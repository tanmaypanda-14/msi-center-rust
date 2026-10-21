//! Semantic design tokens for the UI.
//!
//! The UI never sees an Omarchy colour name. It asks for `surface`, `muted`,
//! `accent` and so on, which keeps the Slint files independent of any one
//! theme's vocabulary and lets the resolver guarantee contrast.

use crate::color::Rgb;
use crate::palette::Palette;

/// Colours the interface is allowed to use, all resolved from the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tokens {
    /// Window background.
    pub bg: Rgb,
    /// Navigation rail, one step away from the window.
    pub rail: Rgb,
    /// Card background.
    pub surface: Rgb,
    /// Raised background: inputs, segmented controls, hover.
    pub surface_alt: Rgb,
    /// Hairline borders.
    pub border: Rgb,
    /// Border for a focused or selected control.
    pub border_strong: Rgb,

    pub text: Rgb,
    pub text_bright: Rgb,
    /// Secondary text, contrast-checked against `surface`.
    pub muted: Rgb,
    /// Decorative text: axis notes, hints.
    pub faint: Rgb,

    pub accent: Rgb,
    pub accent_hover: Rgb,
    /// Tint behind accent-coloured controls.
    pub accent_soft: Rgb,
    /// Text and glyphs drawn on top of `accent`.
    pub on_accent: Rgb,

    pub ok: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
    pub info: Rgb,
}

impl Tokens {
    /// Resolve a palette into tokens.
    ///
    /// Derived shades are mixed from `background` towards `foreground`, which
    /// gives a usable elevation ramp for both dark and light themes. Explicit
    /// theme keys are used when the theme provides them.
    pub fn resolve(palette: &Palette) -> Self {
        let bg = palette.get(&["background"]).unwrap_or_else(|| {
            Palette::fallback()
                .get(&["background"])
                .unwrap_or(Rgb::BLACK)
        });
        let fg = palette
            .get(&["foreground"])
            .unwrap_or_else(|| bg.readable_on([Rgb::WHITE, Rgb::new(0xdd, 0xdd, 0xdd)]));
        let dark = palette.mode().is_dark();

        let elevate = |t: f32| bg.mix(fg, t);

        let surface = palette
            .get(&["lighter_background"])
            .filter(|_| dark)
            .map(|lighter| bg.mix(lighter, 0.55))
            .unwrap_or_else(|| elevate(0.05));
        let surface_alt = palette
            .get(&["lighter_background"])
            .filter(|_| dark)
            .unwrap_or_else(|| elevate(0.10));

        let rail = palette
            .get(&["dark_background"])
            .filter(|_| dark)
            .unwrap_or_else(|| elevate(0.02));

        let accent = palette.get(&["accent", "blue", "green"]).unwrap_or(fg);
        let accent_hover = accent.mix(fg, 0.18);
        let accent_soft = palette
            .get(&["selection"])
            .map(|selection| bg.mix(selection, 0.75))
            .unwrap_or_else(|| bg.mix(accent, 0.20));

        let text_bright = palette
            .get(&["bright_foreground"])
            .unwrap_or_else(|| fg.mix(Rgb::WHITE, 0.35));

        let muted_raw = palette.get(&["muted"]).unwrap_or_else(|| fg.mix(bg, 0.45));
        let muted = muted_raw.at_least_contrast(surface, 4.0, fg);
        let faint = muted_raw.at_least_contrast(bg, 2.6, fg);

        let ok =
            palette
                .get(&["green"])
                .unwrap_or(accent)
                .at_least_contrast(surface, 3.0, text_bright);
        let warn = palette
            .get(&["orange", "yellow"])
            .unwrap_or(accent_hover)
            .at_least_contrast(surface, 3.0, text_bright);
        let danger = palette.get(&["red"]).unwrap_or(accent);
        let info = palette.get(&["blue", "cyan"]).unwrap_or(accent);

        Self {
            bg,
            rail,
            surface,
            surface_alt,
            border: elevate(0.14),
            border_strong: elevate(0.24),
            text: fg,
            text_bright,
            muted,
            faint,
            accent,
            accent_hover,
            accent_soft,
            on_accent: accent.readable_on([bg, text_bright]),
            ok,
            warn,
            danger,
            info,
        }
    }

    /// A stable `key=value` dump, used by `--theme` diagnostics and tests.
    pub fn to_lines(&self) -> Vec<String> {
        let entries = [
            ("bg", self.bg),
            ("rail", self.rail),
            ("surface", self.surface),
            ("surface-alt", self.surface_alt),
            ("border", self.border),
            ("text", self.text),
            ("muted", self.muted),
            ("accent", self.accent),
            ("ok", self.ok),
            ("warn", self.warn),
            ("danger", self.danger),
        ];
        entries
            .iter()
            .map(|(name, color)| format!("{name:<12} {}", color.to_hex()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osaka_jade() -> Palette {
        Palette::parse(
            r##"
mode = "dark"
accent = "#509475"
selection = "#32473B"
muted = "#53685B"
background = "#111c18"
dark_background = "#0c1512"
lighter_background = "#23372B"
foreground = "#C1C497"
bright_foreground = "#F7E8B2"
red = "#FF5345"
green = "#549e6a"
yellow = "#E5C736"
blue = "#509475"
"##,
        )
        .unwrap()
    }

    #[test]
    fn osaka_jade_maps_to_the_system_colours() {
        let tokens = Tokens::resolve(&osaka_jade());
        assert_eq!(tokens.bg, Rgb::from_hex("#111c18").unwrap());
        assert_eq!(tokens.rail, Rgb::from_hex("#0c1512").unwrap());
        assert_eq!(tokens.accent, Rgb::from_hex("#509475").unwrap());
        assert_eq!(tokens.text, Rgb::from_hex("#c1c497").unwrap());
        assert_eq!(tokens.text_bright, Rgb::from_hex("#f7e8b2").unwrap());
    }

    #[test]
    fn surfaces_are_elevated_but_still_distinct() {
        let tokens = Tokens::resolve(&osaka_jade());
        assert_ne!(tokens.surface, tokens.bg);
        assert_ne!(tokens.surface_alt, tokens.surface);
        assert_ne!(tokens.border, tokens.surface);
        assert!(tokens.surface.contrast(tokens.bg) > 1.0);
    }

    #[test]
    fn secondary_text_stays_readable() {
        let tokens = Tokens::resolve(&osaka_jade());
        assert!(
            tokens.muted.contrast(tokens.surface) >= 4.0,
            "muted contrast was {}",
            tokens.muted.contrast(tokens.surface)
        );
        assert!(tokens.text.contrast(tokens.surface) >= 7.0);
    }

    #[test]
    fn accent_text_is_readable() {
        let tokens = Tokens::resolve(&osaka_jade());
        assert!(tokens.on_accent.contrast(tokens.accent) >= 4.5);
    }

    #[test]
    fn light_themes_also_produce_an_elevation_ramp() {
        let palette = Palette::parse(
            r##"
mode = "light"
background = "#fffcf0"
foreground = "#100f0f"
muted = "#6f6e69"
accent = "#205ea6"
selection = "#e6e4d9"
green = "#66800b"
yellow = "#ad8301"
red = "#af3029"
blue = "#205ea6"
"##,
        )
        .unwrap();
        let tokens = Tokens::resolve(&palette);
        assert_ne!(tokens.surface, tokens.bg);
        assert!(tokens.text.contrast(tokens.surface) >= 7.0);
        assert!(tokens.muted.contrast(tokens.surface) >= 4.0);
    }

    #[test]
    fn a_minimal_palette_still_resolves() {
        let palette = Palette::parse("background = \"#101010\"\n").unwrap();
        let tokens = Tokens::resolve(&palette);
        assert!(tokens.text.contrast(tokens.bg) > 4.0);
        assert_eq!(tokens.accent, tokens.text);
    }
}
