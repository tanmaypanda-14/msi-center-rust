//! Where Omarchy keeps the active theme, and the small config bits around it.
//!
//! Omarchy keeps the applied theme in `~/.local/state/omarchy/current/theme`
//! (a directory, or a symlink in older releases) with `theme.name` beside it.
//! It does not honour `XDG_STATE_HOME`, so neither do we; `MSI_CENTER_THEME_DIR`
//! overrides the location for tests and for other desktops that keep a
//! compatible palette.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Font, DEFAULT_BASE_SIZE, DEFAULT_FAMILY};

pub const COLORS_FILE: &str = "colors.toml";
pub const SHELL_FILE: &str = "shell.toml";
pub const NAME_FILE: &str = "theme.name";

/// Directory of the applied theme.
pub fn state_theme_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("MSI_CENTER_THEME_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".local/state/omarchy/current")
            .join("theme"),
    )
}

/// Directory of a theme by name: user overlay first, then the stock themes.
pub fn named_theme_dir(name: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let omarchy =
        std::env::var_os("OMARCHY_PATH").unwrap_or_else(|| OsString::from("/usr/share/omarchy"));

    let mut candidates = Vec::with_capacity(3);
    if let Some(home) = home {
        candidates.push(home.join(".config/omarchy/themes").join(name));
    }
    candidates.push(PathBuf::from(omarchy).join("themes").join(name));

    candidates
        .into_iter()
        .find(|dir| dir.join(COLORS_FILE).is_file())
}

/// The theme's name, from the `theme.name` file Omarchy writes beside the
/// theme directory, or from the directory name as a fallback.
pub fn theme_name(dir: &Path) -> Option<String> {
    if let Some(parent) = dir.parent() {
        if let Ok(name) = std::fs::read_to_string(parent.join(NAME_FILE)) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    dir.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}

/// Typography from the theme's `shell.toml`, with the app defaults for
/// anything it does not set. `MSI_CENTER_FONT` wins over the family.
pub fn font(dir: &Path) -> Font {
    let base_size = std::fs::read_to_string(dir.join(SHELL_FILE))
        .ok()
        .and_then(|src| ini_get(&src, "font", "base-size"))
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|size| (8.0..=28.0).contains(size))
        .unwrap_or(DEFAULT_BASE_SIZE);

    let family = std::env::var("MSI_CENTER_FONT")
        .ok()
        .filter(|family| !family.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_FAMILY.to_owned());

    Font { family, base_size }
}

/// Minimal `[section] key = value` lookup for the flat numbers we read.
fn ini_get(src: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for raw in src.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name.trim() == section;
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches(['"', '\'']).to_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct TempTheme(PathBuf);

    impl TempTheme {
        fn new(colors: &str, name: Option<&str>, shell: Option<&str>) -> Self {
            // Mirror Omarchy: `current/theme.name` beside `current/theme/`.
            let state = std::env::temp_dir().join(format!(
                "msi-theme-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let dir = state.join("theme");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(COLORS_FILE), colors).unwrap();
            if let Some(name) = name {
                std::fs::write(state.join(NAME_FILE), name).unwrap();
            }
            if let Some(shell) = shell {
                std::fs::write(dir.join(SHELL_FILE), shell).unwrap();
            }
            Self(state)
        }

        fn theme_dir(&self) -> PathBuf {
            self.0.join("theme")
        }
    }

    impl Drop for TempTheme {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const COLORS: &str = "mode = \"dark\"\nbackground = \"#111c18\"\nforeground = \"#C1C497\"\n";

    #[test]
    fn name_file_wins_over_directory_name() {
        let theme = TempTheme::new(COLORS, Some("osaka-jade\n"), None);
        assert_eq!(
            theme_name(&theme.theme_dir()).as_deref(),
            Some("osaka-jade")
        );
    }

    #[test]
    fn directory_name_is_the_fallback() {
        let theme = TempTheme::new(COLORS, None, None);
        assert_eq!(theme_name(&theme.theme_dir()).as_deref(), Some("theme"));
    }

    #[test]
    fn base_size_comes_from_the_font_section() {
        let shell = "[bar]\nbase-size = 99\n\n[font]\n# the root of the scale\nbase-size = 14\n";
        let theme = TempTheme::new(COLORS, Some("x"), Some(shell));
        assert_eq!(font(&theme.theme_dir()).base_size, 14.0);
    }

    #[test]
    fn missing_shell_file_uses_the_default_scale() {
        let theme = TempTheme::new(COLORS, Some("x"), None);
        assert_eq!(font(&theme.theme_dir()).base_size, DEFAULT_BASE_SIZE);
    }

    #[test]
    fn absurd_base_size_is_rejected() {
        let theme = TempTheme::new(COLORS, Some("x"), Some("[font]\nbase-size = 400\n"));
        assert_eq!(font(&theme.theme_dir()).base_size, DEFAULT_BASE_SIZE);
    }
}
