//! Parsing of an Omarchy `colors.toml`.
//!
//! The file is a flat TOML document of `key = "#rrggbb"` pairs. We parse the
//! subset we need instead of pulling a TOML implementation into the binary,
//! and we keep unknown keys so future Omarchy releases do not break us.

use std::collections::HashMap;

use crate::color::Rgb;
use crate::Error;

/// Light or dark, as declared by the theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

impl Mode {
    pub fn parse(value: &str) -> Self {
        match value
            .trim()
            .trim_matches(['"', '\''])
            .to_ascii_lowercase()
            .as_str()
        {
            "light" => Self::Light,
            _ => Self::Dark,
        }
    }

    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// The raw palette of one Omarchy theme.
#[derive(Clone, Debug)]
pub struct Palette {
    mode: Mode,
    colors: HashMap<String, Rgb>,
}

impl Palette {
    /// Parse the contents of a `colors.toml`.
    ///
    /// Unknown keys are kept; unreadable values are ignored. Only `background`
    /// is required, because every other token has a sensible derivation.
    pub fn parse(src: &str) -> Result<Self, Error> {
        let mut mode = Mode::default();
        let mut colors = HashMap::new();

        for (line_no, raw) in src.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value);
            if key == "mode" {
                mode = Mode::parse(value);
            } else if let Some(rgb) = Rgb::from_hex(value) {
                colors.insert(key.to_owned(), rgb);
            } else if key == "background" || key == "foreground" {
                return Err(Error::Malformed {
                    line: line_no + 1,
                    detail: format!("{key} is not a hex colour: {value:?}"),
                });
            }
        }

        if !colors.contains_key("background") {
            return Err(Error::Malformed {
                line: 0,
                detail: "no `background` key".to_owned(),
            });
        }
        Ok(Self { mode, colors })
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// First key that exists among `keys`, in preference order.
    pub fn get(&self, keys: &[&str]) -> Option<Rgb> {
        keys.iter().find_map(|key| self.colors.get(*key).copied())
    }

    /// The palette used when no Omarchy theme can be read.
    ///
    /// These are the colours the app shipped with before it learned to follow
    /// the system theme, so an offline machine keeps a familiar look.
    pub fn fallback() -> Self {
        let mut colors = HashMap::new();
        for (key, value) in [
            ("background", "#0a0c10"),
            ("dark_background", "#07090c"),
            ("darker_background", "#05070a"),
            ("lighter_background", "#1b2230"),
            ("foreground", "#e9eef7"),
            ("bright_foreground", "#ffffff"),
            ("dark_foreground", "#aeb9cc"),
            ("muted", "#8592a6"),
            ("accent", "#e01b2e"),
            ("selection", "#3a1220"),
            ("green", "#2fbf71"),
            ("yellow", "#f0a02a"),
            ("red", "#ff5345"),
            ("blue", "#5aa9e6"),
        ] {
            if let Some(rgb) = Rgb::from_hex(value) {
                colors.insert(key.to_owned(), rgb);
            }
        }
        Self {
            mode: Mode::Dark,
            colors,
        }
    }
}

/// Drop a trailing `# comment`, keeping `#` inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' | '\'' => {
                quote = match quote {
                    Some(q) if q == ch => None,
                    None => Some(ch),
                    other => other,
                };
            }
            '#' if quote.is_none() => return &line[..i],
            _ => {}
        }
    }
    line
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    const OSAKA_JADE: &str = r##"
mode = "dark"

accent = "#509475"
selection = "#32473B"
muted = "#53685B"

background = "#111c18" # darker than the accents
dark_background = "#0c1512"
foreground = "#C1C497"

red = "#FF5345"
green = "#549e6a"
yellow = "#E5C736"
blue = "#509475"
"##;

    #[test]
    fn parses_osaka_jade() {
        let palette = Palette::parse(OSAKA_JADE).expect("valid palette");
        assert_eq!(palette.mode(), Mode::Dark);
        assert_eq!(palette.get(&["background"]), Rgb::from_hex("#111c18"));
        assert_eq!(palette.get(&["accent"]), Rgb::from_hex("#509475"));
        assert_eq!(palette.get(&["red"]), Rgb::from_hex("#ff5345"));
    }

    #[test]
    fn hash_inside_quotes_is_not_a_comment() {
        let palette = Palette::parse("background = \"#111c18\"\n").expect("valid palette");
        assert_eq!(
            palette.get(&["background"]),
            Some(Rgb::new(0x11, 0x1c, 0x18))
        );
    }

    #[test]
    fn missing_background_is_an_error() {
        let err = Palette::parse("accent = \"#509475\"\n").unwrap_err();
        assert!(err.to_string().contains("background"));
    }

    #[test]
    fn unreadable_background_is_an_error() {
        assert!(Palette::parse("background = garbage\n").is_err());
    }

    #[test]
    fn light_mode_is_recognised() {
        let palette = Palette::parse("mode = \"light\"\nbackground = \"#fffcf0\"\n").unwrap();
        assert_eq!(palette.mode(), Mode::Light);
    }

    #[test]
    fn unknown_keys_and_junk_are_ignored() {
        let palette =
            Palette::parse("background = \"#111c18\"\nsome_future_key = \"whatever\"\n[]\n")
                .unwrap();
        assert_eq!(palette.get(&["some_future_key"]), None);
    }
}
