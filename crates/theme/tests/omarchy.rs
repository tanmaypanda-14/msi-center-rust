//! End-to-end read of an Omarchy-style theme directory.

use std::path::PathBuf;

use msi_theme::{Source, Theme};

struct Fixture(PathBuf);

impl Fixture {
    fn new(colors: Option<&str>, shell: Option<&str>) -> Self {
        let state = std::env::temp_dir().join(format!(
            "msi-theme-integration-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        let dir = state.join("theme");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(state.join("theme.name"), "tokyo-night\n").unwrap();
        if let Some(colors) = colors {
            std::fs::write(dir.join("colors.toml"), colors).unwrap();
        }
        if let Some(shell) = shell {
            std::fs::write(dir.join("shell.toml"), shell).unwrap();
        }
        Self(state)
    }
}

impl Fixture {
    fn theme_dir(&self) -> PathBuf {
        self.0.join("theme")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TOKYO_NIGHT: &str = r##"
mode = "dark"
accent = "#7aa2f7"
selection = "#283457"
muted = "#565f89"
background = "#1a1b26"
dark_background = "#16161e"
lighter_background = "#292e42"
foreground = "#c0caf5"
bright_foreground = "#ffffff"
red = "#f7768e"
green = "#9ece6a"
yellow = "#e0af68"
blue = "#7aa2f7"
"##;

#[test]
fn reads_an_omarchy_theme_directory() {
    let dir = Fixture::new(Some(TOKYO_NIGHT), Some("[font]\nbase-size = 13\n"));
    let theme = Theme::load_from(&dir.theme_dir()).expect("theme should load");

    assert_eq!(theme.name, "tokyo-night");
    assert_eq!(theme.source, Source::Omarchy);
    assert_eq!(theme.tokens.accent.to_hex(), "#7aa2f7");
    assert_eq!(theme.tokens.bg.to_hex(), "#1a1b26");
    assert_eq!(theme.font.base_size, 13.0);
}

#[test]
fn a_directory_without_colors_is_an_error() {
    let dir = Fixture::new(None, None);
    assert!(Theme::load_from(&dir.theme_dir()).is_err());
}

#[test]
fn malformed_palette_reports_a_line() {
    let dir = Fixture::new(Some("background = \"oops\"\n"), None);
    let err = Theme::load_from(&dir.theme_dir()).unwrap_err().to_string();
    assert!(err.contains("background"), "{err}");
}

#[test]
fn fallback_naming_is_stable() {
    assert_eq!(Theme::fallback().name, "msi");
}
