//! Release versions.
//!
//! Tags look like `v0.2.3-3`: a dotted numeric version plus an Arch `pkgrel`.
//! A missing `pkgrel` sorts before any `pkgrel` of the same base, which is what
//! makes `0.2.3-2` older than `0.2.3-3`.

use std::cmp::Ordering;
use std::fmt;

/// A dotted version with optional `pkgrel`, compared numerically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    base: Vec<u64>,
    release: u64,
}

impl Version {
    /// Parse `0.2.3`, `0.2.3-3`, or the same with a leading `v`.
    ///
    /// Trailing zeroes are dropped, so `1.0` and `1.0.0` are equal.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text
            .strip_prefix('v')
            .or_else(|| text.strip_prefix('V'))
            .unwrap_or(text);

        let (base, release) = match text.split_once('-') {
            Some((base, release)) => (base, release.parse().ok()?),
            None => (text, 0),
        };

        let mut base: Vec<u64> = base
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if base.is_empty() || base.len() > 8 {
            return None;
        }
        while base.len() > 1 && base.last() == Some(&0) {
            base.pop();
        }

        Some(Self { base, release })
    }

    /// The version without its `pkgrel`.
    pub fn base(&self) -> &[u64] {
        &self.base
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let width = self.base.len().max(other.base.len());
        for index in 0..width {
            let left = self.base.get(index).copied().unwrap_or(0);
            let right = other.base.get(index).copied().unwrap_or(0);
            match left.cmp(&right) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        self.release.cmp(&other.release)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let base = self
            .base
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".");
        if self.release == 0 {
            f.write_str(&base)
        } else {
            write!(f, "{base}-{}", self.release)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text:?} should parse"))
    }

    #[test]
    fn parses_release_tags() {
        assert_eq!(v("v0.2.3-3").to_string(), "0.2.3-3");
        assert_eq!(v("0.2.3").to_string(), "0.2.3");
        assert_eq!(v(" 1.0.0 ").to_string(), "1");
    }

    #[test]
    fn compares_each_component_numerically() {
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert_eq!(v("1.0"), v("1.0.0"));
    }

    #[test]
    fn pkgrel_sorts_after_its_base() {
        assert!(v("0.2.3-3") > v("0.2.3"));
        assert!(v("0.2.3-10") > v("0.2.3-9"));
        assert!(v("0.2.4-1") > v("0.2.3-9"));
    }

    #[test]
    fn base_ignores_pkgrel() {
        assert_eq!(v("0.2.3-3").base(), v("0.2.3").base());
        assert!(v("0.2.3-3").base() < v("0.2.4-1").base());
    }

    #[test]
    fn rejects_garbage() {
        for text in ["", "v", "release", "1.2.x", "1:0.2.3-1", "1.2.3-beta"] {
            assert!(Version::parse(text).is_none(), "{text:?} should not parse");
        }
    }
}
