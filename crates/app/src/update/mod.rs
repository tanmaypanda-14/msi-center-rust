//! `msi-center --update`: install a newer GitHub release.
//!
//! The updater never executes a downloaded script. It resolves the latest tag,
//! downloads the artifact named in `SHA256SUMS`, verifies the checksum, and
//! then installs it according to how this copy was installed:
//!
//! * a pacman-owned binary gets the release package (`pacman -U`), so the
//!   package database stays authoritative;
//! * a tarball install gets its binary replaced atomically, with elevation
//!   when the target directory is not writable.
//!
//! The kernel driver, permissions, and the Omarchy widget are upgraded by
//! `install.sh`; they change independently of the app.

mod install;
mod release;
mod version;

use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};

use release::{Asset, Sums};
use version::Version;

/// Anything that stops the update before the new version is in place.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("could not run {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("{action}: {source}")]
    Io {
        action: String,
        #[source]
        source: io::Error,
    },
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub dry_run: bool,
    pub assume_yes: bool,
}

/// Run the command and return a process exit code.
pub fn run(options: Options) -> i32 {
    match perform(options) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn perform(options: Options) -> Result<()> {
    println!("==> checking for updates");
    let current = Current::detect()?;
    println!(
        "    installed  {} ({})",
        current.version,
        current.kind.label()
    );

    let latest = install::latest_version()?;
    println!("    latest     v{latest}");

    if !current.is_older_than(&latest) {
        if current.version == latest {
            println!("\nmsi-center {} is up to date.", current.version);
        } else {
            println!(
                "\nmsi-center {} is up to date (latest release v{latest}).",
                current.version
            );
        }
        return Ok(());
    }

    if current.kind == Kind::Binary && is_development_build(&current.exe) {
        return Err(Error::Message(format!(
            "refusing to replace the development build at {}; install a release first",
            current.exe.display()
        )));
    }

    println!("\n==> update available: {} -> {latest}", current.version);
    let sums = install::sums()?;
    let asset = current.kind.asset(&sums).ok_or_else(|| {
        Error::Message(format!(
            "release v{latest} has no suitable asset in SHA256SUMS"
        ))
    })?;
    println!("    artifact   {}", asset.name);
    println!("    method     {}", current.kind.method());

    if options.dry_run {
        println!("\ndry run: nothing was downloaded or installed.");
        return Ok(());
    }

    if !options.assume_yes {
        if !io::stdin().is_terminal() {
            return Err(Error::Message(
                "not a terminal; re-run with --yes to update unattended".to_owned(),
            ));
        }
        if !confirm()? {
            println!("update cancelled");
            return Ok(());
        }
    }

    let dir = install::TempDir::new()?;
    println!("\n==> downloading {}", asset.name);
    let archive = install::download(asset, dir.path())?;
    println!("==> verifying SHA256SUMS");
    install::verify(asset, &archive)?;

    match current.kind {
        Kind::Package => {
            println!("==> installing with pacman");
            install::install_package(&archive)?;
        }
        Kind::Binary => {
            println!("==> extracting {}", asset.name);
            let binary = install::extract_binary(&archive, dir.path())?;
            println!("==> replacing {}", current.exe.display());
            install::replace_binary(&binary, &current.exe)?;
        }
    }

    println!("\nmsi-center updated to {latest}.");
    Ok(())
}

fn confirm() -> Result<bool> {
    print!("Update now? [Y/n] ");
    io::stdout().flush().map_err(|source| Error::Io {
        action: "could not write to stdout".to_owned(),
        source,
    })?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|source| Error::Io {
            action: "could not read the answer".to_owned(),
            source,
        })?;
    Ok(matches!(
        answer.trim(),
        "" | "y" | "Y" | "yes" | "Yes" | "YES"
    ))
}

/// How this copy of the app was installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Owned by the `msi-center` pacman package.
    Package,
    /// A standalone binary from the release tarball.
    Binary,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Package => "pacman package",
            Self::Binary => "standalone binary",
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::Package => "pacman -U (the package stays authoritative)",
            Self::Binary => "replace the binary in place",
        }
    }

    fn asset(self, sums: &Sums) -> Option<&Asset> {
        match self {
            Self::Package => sums.get(release::APP_PACKAGE),
            Self::Binary => sums.tarball(),
        }
    }
}

struct Current {
    version: Version,
    kind: Kind,
    exe: PathBuf,
}

impl Current {
    fn detect() -> Result<Self> {
        let exe = std::env::current_exe().map_err(|source| Error::Io {
            action: "could not locate the running binary".to_owned(),
            source,
        })?;

        if let Some(version) = install::package_version(&exe)? {
            return Ok(Self {
                version,
                kind: Kind::Package,
                exe,
            });
        }

        let version = Version::parse(env!("CARGO_PKG_VERSION")).ok_or_else(|| {
            Error::Message(format!(
                "could not parse the built-in version {:?}",
                env!("CARGO_PKG_VERSION")
            ))
        })?;
        Ok(Self {
            version,
            kind: Kind::Binary,
            exe,
        })
    }

    /// A tarball install only carries the binary, so a packaging-only release
    /// (`0.2.3-3` over `0.2.3`) does not need a new download.
    fn is_older_than(&self, latest: &Version) -> bool {
        match self.kind {
            Kind::Package => self.version < *latest,
            Kind::Binary => self.version.base() < latest.base(),
        }
    }
}

/// `target/debug/msi-center` and `target/release/msi-center` are build outputs.
fn is_development_build(exe: &Path) -> bool {
    let mut parts = exe.components().rev();
    let file = matches!(parts.next(), Some(Component::Normal(name)) if name == "msi-center");
    let profile = matches!(
        parts.next(),
        Some(Component::Normal(name)) if name == "debug" || name == "release"
    );
    let target = matches!(parts.next(), Some(Component::Normal(name)) if name == "target");
    file && profile && target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn package_installs_compare_the_pkgrel_too() {
        let current = Current {
            version: version("0.2.3-2"),
            kind: Kind::Package,
            exe: PathBuf::from("/usr/bin/msi-center"),
        };
        assert!(current.is_older_than(&version("0.2.3-3")));
        assert!(!current.is_older_than(&version("0.2.3-2")));
    }

    #[test]
    fn binary_installs_compare_only_the_base_version() {
        let current = Current {
            version: version("0.2.3"),
            kind: Kind::Binary,
            exe: PathBuf::from("/usr/local/bin/msi-center"),
        };
        assert!(!current.is_older_than(&version("0.2.3-3")));
        assert!(current.is_older_than(&version("0.2.4-1")));
    }

    #[test]
    fn development_builds_are_recognised() {
        assert!(is_development_build(Path::new(
            "/home/me/msi-center-rust/target/debug/msi-center"
        )));
        assert!(is_development_build(Path::new(
            "/home/me/msi-center-rust/target/release/msi-center"
        )));
        assert!(!is_development_build(Path::new("/usr/bin/msi-center")));
        assert!(!is_development_build(Path::new("/home/me/bin/msi-center")));
    }
}
