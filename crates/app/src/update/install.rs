//! The parts of the updater that touch the system: network, checksums,
//! pacman, and the binary on disk.
//!
//! Downloads are always checked against the release manifest before they are
//! used. Nothing from the release is ever executed as a script.

use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, IsTerminal};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use super::release::{Asset, Sums};
use super::version::Version;
use super::{Error, Result};

const REPO: &str = "tanmaypanda-14/msi-center-rust";
const PACKAGE: &str = "msi-center";
const LATEST: &str = "https://github.com/tanmaypanda-14/msi-center-rust/releases/latest";
const CURL: [&str; 8] = [
    "-fsSL",
    "--proto",
    "=https",
    "--tlsv1.2",
    "--retry",
    "3",
    "--connect-timeout",
    "15",
];

fn asset_url(name: &str) -> String {
    format!("https://github.com/{REPO}/releases/latest/download/{name}")
}

/// Resolve the tag of the latest release without downloading the API payload.
pub fn latest_version() -> Result<Version> {
    let output = curl(&[
        OsStr::new("-o"),
        OsStr::new("/dev/null"),
        OsStr::new("-w"),
        OsStr::new("%{url_effective}"),
        OsStr::new(LATEST),
    ])?;

    let url = String::from_utf8_lossy(&output.stdout);
    let tag = url.trim().rsplit('/').next().unwrap_or_default();
    Version::parse(tag).ok_or_else(|| Error::Message(format!("unexpected release tag {tag:?}")))
}

/// Fetch and parse the release manifest.
pub fn sums() -> Result<Sums> {
    let url = asset_url("SHA256SUMS");
    let output = curl(&[OsStr::new(&url)])?;
    Sums::parse(&String::from_utf8_lossy(&output.stdout))
}

/// Download one asset into `dir` and return the local path.
pub fn download(asset: &Asset, dir: &Path) -> Result<PathBuf> {
    let path = dir.join(&asset.name);
    let url = asset_url(&asset.name);
    curl(&[OsStr::new("-o"), path.as_os_str(), OsStr::new(&url)])?;
    Ok(path)
}

/// Compare a downloaded file with the checksum from the manifest.
pub fn verify(asset: &Asset, path: &Path) -> Result<()> {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .map_err(|source| Error::Spawn {
            program: "sha256sum".to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(failed("sha256sum", &output));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let actual = text.split_whitespace().next().unwrap_or_default();
    if actual.eq_ignore_ascii_case(&asset.sha256) {
        Ok(())
    } else {
        Err(Error::Message(format!(
            "checksum mismatch for {}: expected {}, got {actual}",
            asset.name, asset.sha256
        )))
    }
}

/// The installed version when the running binary belongs to the package.
pub fn package_version(exe: &Path) -> Result<Option<Version>> {
    let owner = match Command::new("pacman").arg("-Qoq").arg(exe).output() {
        Ok(output) => output,
        // Not an Arch machine, or pacman is not installed: tarball semantics.
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Spawn {
                program: "pacman".to_owned(),
                source,
            })
        }
    };
    if !owner.status.success() {
        return Ok(None);
    }

    let owned = String::from_utf8_lossy(&owner.stdout);
    if !owned.lines().any(|line| line.trim() == PACKAGE) {
        return Ok(None);
    }

    let query = Command::new("pacman")
        .args(["-Q", PACKAGE])
        .output()
        .map_err(|source| Error::Spawn {
            program: "pacman".to_owned(),
            source,
        })?;
    if !query.status.success() {
        return Err(failed("pacman", &query));
    }

    let text = String::from_utf8_lossy(&query.stdout);
    let version = text
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| Error::Message("pacman did not report a version".to_owned()))?;
    Version::parse(version)
        .map(Some)
        .ok_or_else(|| Error::Message(format!("could not parse the installed version {version:?}")))
}

/// Install a verified package so pacman remains the owner of the file.
pub fn install_package(archive: &Path) -> Result<()> {
    run_elevated(
        "pacman",
        &[
            OsStr::new("-U"),
            OsStr::new("--noconfirm"),
            archive.as_os_str(),
        ],
    )
}

/// Extract the tarball and return the binary inside it.
pub fn extract_binary(archive: &Path, dir: &Path) -> Result<PathBuf> {
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(archive)
        .arg("-C")
        .arg(dir)
        .status()
        .map_err(|source| Error::Spawn {
            program: "tar".to_owned(),
            source,
        })?;
    if !status.success() {
        return Err(Error::Message(
            "could not extract the release tarball".to_owned(),
        ));
    }

    let direct = dir.join(PACKAGE);
    if direct.is_file() {
        return Ok(direct);
    }
    for entry in fs::read_dir(dir).map_err(|source| Error::Io {
        action: format!("could not read {}", dir.display()),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Io {
            action: format!("could not read {}", dir.display()),
            source,
        })?;
        let candidate = entry.path().join(PACKAGE);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(Error::Message(format!(
        "the release tarball does not contain {PACKAGE}"
    )))
}

/// Replace the running binary, atomically when its directory is writable.
pub fn replace_binary(new_binary: &Path, exe: &Path) -> Result<()> {
    if try_replace(new_binary, exe)? {
        return Ok(());
    }
    run_elevated(
        "install",
        &[
            OsStr::new("-Dm755"),
            new_binary.as_os_str(),
            exe.as_os_str(),
        ],
    )
}

/// A private temporary directory, removed when the update finishes.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Result<Self> {
        let base = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);

        for attempt in 0..100 {
            let path = base.join(format!(
                "msi-center-update-{}-{nanos}-{attempt}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(Error::Io {
                        action: format!("could not create {}", path.display()),
                        source,
                    })
                }
            }
        }

        Err(Error::Message(
            "could not create a temporary directory".to_owned(),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn curl(args: &[&OsStr]) -> Result<Output> {
    let output = Command::new("curl")
        .args(CURL)
        .args(args)
        .output()
        .map_err(|source| Error::Spawn {
            program: "curl".to_owned(),
            source,
        })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(failed("curl", &output))
    }
}

fn try_replace(new_binary: &Path, exe: &Path) -> Result<bool> {
    let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) else {
        return Ok(false);
    };
    let staged = dir.join(format!(
        ".{}.new.{}",
        name.to_string_lossy(),
        std::process::id()
    ));

    match fs::copy(new_binary, &staged) {
        Ok(_) => {}
        Err(err)
            if err.kind() == ErrorKind::PermissionDenied
                || err.kind() == ErrorKind::ReadOnlyFilesystem =>
        {
            return Ok(false)
        }
        Err(source) => {
            return Err(Error::Io {
                action: format!("could not stage {}", staged.display()),
                source,
            })
        }
    }

    let _ = fs::set_permissions(&staged, fs::Permissions::from_mode(0o755));
    if let Err(source) = fs::rename(&staged, exe) {
        let _ = fs::remove_file(&staged);
        if source.kind() == ErrorKind::PermissionDenied {
            return Ok(false);
        }
        return Err(Error::Io {
            action: format!("could not replace {}", exe.display()),
            source,
        });
    }

    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Elevation {
    /// Already root.
    Direct,
    /// Ask polkit (graphical session without a terminal).
    Pkexec,
    /// Ask sudo (terminal session).
    Sudo,
}

fn run_elevated(program: &str, args: &[&OsStr]) -> Result<()> {
    let elevation = elevation()?;
    let mut command = match elevation {
        Elevation::Direct => Command::new(program),
        Elevation::Pkexec => {
            let mut command = Command::new("pkexec");
            command.arg(program);
            command
        }
        Elevation::Sudo => {
            let mut command = Command::new("sudo");
            command.arg("--").arg(program);
            command
        }
    };

    command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command.status().map_err(|source| Error::Spawn {
        program: program.to_owned(),
        source,
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::Message(format!("{program} exited with {status}")))
    }
}

fn elevation() -> Result<Elevation> {
    if is_root() {
        return Ok(Elevation::Direct);
    }
    if std::io::stdin().is_terminal() && in_path("sudo") {
        return Ok(Elevation::Sudo);
    }

    let graphical =
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if graphical && in_path("pkexec") {
        return Ok(Elevation::Pkexec);
    }
    if in_path("sudo") {
        return Ok(Elevation::Sudo);
    }

    Err(Error::Message(
        "this update needs root: re-run it with sudo, or install pkexec".to_owned(),
    ))
}

fn is_root() -> bool {
    fs::metadata("/proc/self")
        .map(|metadata| metadata.uid() == 0)
        .unwrap_or(false)
}

fn in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|dir| {
                let candidate = dir.join(program);
                candidate.is_file()
                    && candidate
                        .metadata()
                        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn failed(program: &str, output: &Output) -> Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no details");
    Error::Message(format!("{program} failed: {}", detail.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builds_asset_urls_from_the_repository() {
        assert_eq!(
            asset_url("SHA256SUMS"),
            "https://github.com/tanmaypanda-14/msi-center-rust/releases/latest/download/SHA256SUMS"
        );
    }

    #[test]
    fn temporary_directories_are_private_and_removed() {
        let path = {
            let dir = TempDir::new().unwrap();
            let path = dir.path().to_path_buf();
            assert!(path.is_dir());
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o700);
            path
        };
        assert!(!path.exists());
    }

    #[test]
    fn path_lookup_finds_the_shell() {
        assert!(in_path("sh"));
        assert!(!in_path("definitely-not-a-real-program"));
    }

    #[test]
    fn verification_accepts_the_manifest_checksum_and_rejects_others() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("asset");
        fs::write(&file, b"msi-center").unwrap();

        let output = Command::new("sha256sum").arg(&file).output().unwrap();
        let sha256 = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let asset = Asset {
            name: "asset".to_owned(),
            sha256,
        };
        assert!(verify(&asset, &file).is_ok());

        let wrong = Asset {
            name: "asset".to_owned(),
            sha256: "0".repeat(64),
        };
        assert!(verify(&wrong, &file).is_err());
    }

    #[test]
    fn extracts_and_replaces_a_tarball_binary() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("msi-center-0.2.3-x86_64-linux");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("msi-center"), b"new binary").unwrap();

        let archive = dir.path().join("release.tar.gz");
        let status = Command::new("tar")
            .args(["-czf"])
            .arg(&archive)
            .arg("-C")
            .arg(dir.path())
            .arg("msi-center-0.2.3-x86_64-linux")
            .status()
            .unwrap();
        assert!(status.success());

        let extracted = extract_binary(&archive, dir.path()).unwrap();
        assert_eq!(fs::read(&extracted).unwrap(), b"new binary");

        let installed = dir.path().join("msi-center");
        fs::write(&installed, b"old binary").unwrap();
        replace_binary(&extracted, &installed).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"new binary");
        assert_eq!(
            fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}
