//! A tiny RGB colour type with the few operations the token resolver needs.
//!
//! Deliberately dependency-free: the workspace ships a single small binary,
//! and every operation here is a handful of integer operations.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#rrggbb`, `rrggbb`, `#rgb` or `#rrggbbaa` (alpha is dropped).
    pub fn from_hex(input: &str) -> Option<Self> {
        let hex = input.trim().strip_prefix('#').unwrap_or(input.trim());
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        match hex.len() {
            3 => {
                let mut out = [0u8; 3];
                for (i, slot) in out.iter_mut().enumerate() {
                    let digit = u8::from_str_radix(&hex[i..i + 1], 16).ok()?;
                    *slot = digit * 17;
                }
                Some(Self::new(out[0], out[1], out[2]))
            }
            6 | 8 => Some(Self::new(channel(0)?, channel(2)?, channel(4)?)),
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Linear interpolation towards `other`, with `t` clamped to `0.0..=1.0`.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self::new(
            blend(self.r, other.r),
            blend(self.g, other.g),
            blend(self.b, other.b),
        )
    }

    /// Relative luminance, WCAG 2.x.
    pub fn luminance(self) -> f32 {
        fn channel(v: u8) -> f32 {
            let v = v as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG contrast ratio between two colours, `1.0..=21.0`.
    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Move `self` towards `target` until it reaches `min` contrast against
    /// `background`. Returns `self` unchanged when it already passes.
    pub fn at_least_contrast(self, background: Self, min: f32, target: Self) -> Self {
        if self.contrast(background) >= min {
            return self;
        }
        // Ten steps is enough to cover any WCAG threshold while keeping the
        // result as close to the theme's own colour as possible.
        (1..=10)
            .map(|step| self.mix(target, step as f32 / 10.0))
            .find(|candidate| candidate.contrast(background) >= min)
            .unwrap_or(target)
    }

    /// The more readable of two candidates on top of `self`.
    pub fn readable_on(self, candidates: [Self; 2]) -> Self {
        let [a, b] = candidates;
        if a.contrast(self) >= b.contrast(self) {
            a
        } else {
            b
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_hex_forms_omarchy_writes() {
        assert_eq!(Rgb::from_hex("#509475"), Some(Rgb::new(0x50, 0x94, 0x75)));
        assert_eq!(Rgb::from_hex("509475"), Some(Rgb::new(0x50, 0x94, 0x75)));
        assert_eq!(Rgb::from_hex("#fff"), Some(Rgb::WHITE));
        assert_eq!(Rgb::from_hex("#509475ff"), Some(Rgb::new(0x50, 0x94, 0x75)));
        assert_eq!(Rgb::from_hex("#12345"), None);
        assert_eq!(Rgb::from_hex("nope"), None);
    }

    #[test]
    fn mix_clamps_and_rounds() {
        let black = Rgb::BLACK;
        let white = Rgb::WHITE;
        assert_eq!(black.mix(white, 0.5), Rgb::new(128, 128, 128));
        assert_eq!(black.mix(white, 2.0), white);
        assert_eq!(white.mix(black, -1.0), white);
    }

    #[test]
    fn contrast_bounds() {
        assert!((Rgb::WHITE.contrast(Rgb::BLACK) - 21.0).abs() < 0.01);
        assert!((Rgb::WHITE.contrast(Rgb::WHITE) - 1.0).abs() < 0.001);
        assert!(Rgb::WHITE.contrast(Rgb::new(0x50, 0x94, 0x75)) > 1.0);
    }

    #[test]
    fn contrast_boost_reaches_the_threshold() {
        let bg = Rgb::new(0x11, 0x1c, 0x18);
        let faint = Rgb::new(0x53, 0x68, 0x5b);
        assert!(faint.contrast(bg) < 3.0);
        let lifted = faint.at_least_contrast(bg, 3.0, Rgb::new(0xc1, 0xc4, 0x97));
        assert!(lifted.contrast(bg) >= 3.0);
    }

    #[test]
    fn readable_on_prefers_the_higher_contrast_candidate() {
        let accent = Rgb::new(0x50, 0x94, 0x75);
        assert_eq!(accent.readable_on([Rgb::WHITE, Rgb::BLACK]), Rgb::BLACK);
    }
}
