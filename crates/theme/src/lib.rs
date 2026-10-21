//! Omarchy theme ingestion for the MSI Center UI.
//!
//! Omarchy writes the active theme to a fixed location and exposes its palette
//! as `colors.toml`. This crate reads that file and resolves it into semantic
//! [`Tokens`] the Slint UI can bind to, so the window follows the desktop
//! theme without knowing anything about colour names or TOML.
//!
//! The interface is deliberately small:
//!
//! * [`load`] — the theme to draw with. Never fails; falls back to the app's
//!   own palette when the machine is not running Omarchy.
//! * [`Theme::load_from`] — the same for an explicit theme directory, which is
//!   also the seam the tests use.
//! * [`Theme::tokens`] / [`Theme::font`] — what the UI consumes.
//!
//! Everything else (paths, parsing, contrast fixing) is implementation.
//!
//! ```
//! let theme = msi_theme::Theme::from_colors_toml(
//!     "osaka-jade",
//!     "background = \"#111c18\"\nforeground = \"#C1C497\"\naccent = \"#509475\"\n",
//! )
//! .unwrap();
//! assert_eq!(theme.name, "osaka-jade");
//! assert_eq!(theme.tokens.accent.to_hex(), "#509475");
//! ```

mod color;
mod error;
mod palette;
mod paths;
mod tokens;

use std::path::Path;

pub use color::Rgb;
pub use error::{Error, Result};
pub use palette::{Mode, Palette};
pub use paths::{named_theme_dir, state_theme_dir};
pub use tokens::Tokens;

/// Font family used when the theme does not name one.
pub const DEFAULT_FAMILY: &str = "Adwaita Sans";
/// Omarchy's default type-scale root.
pub const DEFAULT_BASE_SIZE: f32 = 12.0;

/// Typography for the window, taken from the theme's `shell.toml` when it has
/// one and from the app defaults otherwise.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub family: String,
    /// Root of the type scale, in px.
    pub base_size: f32,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: DEFAULT_FAMILY.to_owned(),
            base_size: DEFAULT_BASE_SIZE,
        }
    }
}

/// Where the resolved tokens came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// An Omarchy theme directory was read successfully.
    Omarchy,
    /// The app's built-in palette, used when no theme could be read.
    Fallback,
}

/// One resolved theme.
#[derive(Clone, Debug)]
pub struct Theme {
    /// Human-readable name, e.g. `osaka-jade`.
    pub name: String,
    pub mode: Mode,
    pub tokens: Tokens,
    pub font: Font,
    pub source: Source,
}

impl Theme {
    /// The theme to draw with: `MSI_CENTER_THEME` if set, otherwise the active
    /// Omarchy theme, otherwise the built-in palette.
    pub fn load() -> Self {
        if let Some(name) = std::env::var_os("MSI_CENTER_THEME") {
            if let Some(theme) = named_theme_dir(Path::new(&name))
                .as_deref()
                .and_then(Self::try_load)
            {
                return theme;
            }
            eprintln!(
                "msi-center: theme {:?} not found, using the active Omarchy theme",
                name
            );
        }

        let Some(dir) = state_theme_dir() else {
            return Self::fallback();
        };
        match Self::load_from(&dir) {
            Ok(theme) => theme,
            Err(err) => {
                // A missing directory means "not an Omarchy machine": that is
                // normal and quiet. A broken file is worth a line on stderr.
                if dir.join(paths::COLORS_FILE).exists() {
                    eprintln!("msi-center: {err}; using the built-in palette");
                }
                Self::fallback()
            }
        }
    }

    /// Read the theme in `dir` (which must contain `colors.toml`).
    pub fn load_from(dir: &Path) -> Result<Self> {
        let colors =
            std::fs::read_to_string(dir.join(paths::COLORS_FILE)).map_err(|source| Error::Io {
                path: dir.join(paths::COLORS_FILE),
                source,
            })?;
        let name = paths::theme_name(dir).unwrap_or_else(|| "unknown".to_owned());
        let mut theme = Self::from_colors_toml(&name, &colors)?;
        theme.font = paths::font(dir);
        Ok(theme)
    }

    /// Resolve a `colors.toml` that is already in memory.
    pub fn from_colors_toml(name: &str, src: &str) -> Result<Self> {
        let palette = Palette::parse(src)?;
        Ok(Self {
            name: name.to_owned(),
            mode: palette.mode(),
            tokens: Tokens::resolve(&palette),
            font: Font::default(),
            source: Source::Omarchy,
        })
    }

    /// The built-in palette, for machines without Omarchy.
    pub fn fallback() -> Self {
        let palette = Palette::fallback();
        Self {
            name: "msi".to_owned(),
            mode: palette.mode(),
            tokens: Tokens::resolve(&palette),
            font: Font::default(),
            source: Source::Fallback,
        }
    }

    fn try_load(dir: &Path) -> Option<Self> {
        if !dir.join(paths::COLORS_FILE).is_file() {
            return None;
        }
        Self::load_from(dir).ok()
    }

    /// One-block summary used by `msi-center --theme`.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("theme        {} ({:?})\n", self.name, self.mode));
        out.push_str(&format!(
            "source       {}\n",
            match self.source {
                Source::Omarchy => "omarchy",
                Source::Fallback => "built-in",
            }
        ));
        out.push_str(&format!(
            "font         {} @ {}px\n",
            self.font.family, self.font.base_size
        ));
        for line in self.tokens.to_lines() {
            out.push_str("             ");
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_self_consistent() {
        let theme = Theme::fallback();
        assert_eq!(theme.source, Source::Fallback);
        assert!(theme.tokens.text.contrast(theme.tokens.surface) >= 4.5);
        assert_ne!(theme.tokens.accent, theme.tokens.bg);
    }

    #[test]
    fn describe_lists_tokens() {
        let text = Theme::fallback().describe();
        assert!(text.contains("theme"));
        assert!(text.contains("accent"));
    }
}
