//! The release manifest (`SHA256SUMS`).
//!
//! The manifest is both the asset index and the checksum list, so the updater
//! needs exactly one small download before it knows which artifact to fetch.

use super::{Error, Result};

/// Stable alias for the pacman package, regardless of its version.
pub const APP_PACKAGE: &str = "msi-center-x86_64.pkg.tar.zst";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
}

/// The assets listed in one release.
#[derive(Clone, Debug, Default)]
pub struct Sums {
    assets: Vec<Asset>,
}

impl Sums {
    /// Parse the two-column `sha256sum` format.
    pub fn parse(text: &str) -> Result<Self> {
        let mut assets = Vec::new();

        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut fields = line.split_whitespace();
            let (Some(sha256), Some(name), None) = (fields.next(), fields.next(), fields.next())
            else {
                return Err(Error::Message(format!(
                    "malformed SHA256SUMS line {}",
                    index + 1
                )));
            };

            if !is_sha256(sha256) {
                return Err(Error::Message(format!(
                    "invalid checksum on SHA256SUMS line {}",
                    index + 1
                )));
            }
            if !is_plain_name(name) {
                return Err(Error::Message(format!(
                    "unsafe asset name {name:?} on SHA256SUMS line {}",
                    index + 1
                )));
            }

            assets.push(Asset {
                name: name.to_owned(),
                sha256: sha256.to_ascii_lowercase(),
            });
        }

        if assets.is_empty() {
            return Err(Error::Message("SHA256SUMS lists no assets".to_owned()));
        }
        Ok(Self { assets })
    }

    pub fn get(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }

    /// The versioned tarball used by non-pacman installs.
    pub fn tarball(&self) -> Option<&Asset> {
        self.assets.iter().find(|asset| {
            asset.name.starts_with("msi-center-") && asset.name.ends_with("-x86_64-linux.tar.gz")
        })
    }
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Asset names are used as local file names, so they must stay in one directory.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
bfd6ce11112eee9b9a3591077b307956795426d0002dde404625315e6332764b  install.sh
f5020722d034cfd65b3894983b85b550cc29853de09624bd19404f3a21d04f38  msi-center-x86_64.pkg.tar.zst
4253b3bab8ce798b805661924126438d4b059a14afd0ca261ec880f5c0bb038a  msi-center-0.2.3-x86_64-linux.tar.gz
";

    #[test]
    fn finds_the_package_alias_and_the_tarball() {
        let sums = Sums::parse(SAMPLE).unwrap();
        assert_eq!(
            sums.get(APP_PACKAGE).unwrap().sha256,
            "f5020722d034cfd65b3894983b85b550cc29853de09624bd19404f3a21d04f38"
        );
        assert_eq!(
            sums.tarball().unwrap().name,
            "msi-center-0.2.3-x86_64-linux.tar.gz"
        );
    }

    #[test]
    fn ignores_blank_lines_and_comments() {
        let sums = Sums::parse(&format!("# comment\n\n{SAMPLE}")).unwrap();
        assert!(sums.get(APP_PACKAGE).is_some());
    }

    #[test]
    fn rejects_unsafe_asset_names() {
        let line = format!("{}  ../../etc/passwd\n", "a".repeat(64));
        assert!(Sums::parse(&line).is_err());
        let line = format!("{}  /usr/bin/msi-center\n", "a".repeat(64));
        assert!(Sums::parse(&line).is_err());
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(Sums::parse("").is_err());
        assert!(Sums::parse("not-a-checksum  install.sh\n").is_err());
        assert!(Sums::parse(&format!("{}\n", "a".repeat(64))).is_err());
        assert!(Sums::parse(&format!("{}  install.sh  extra\n", "a".repeat(64))).is_err());
    }
}
