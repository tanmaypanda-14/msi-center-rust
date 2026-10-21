//! Bind the resolved Omarchy theme to the Slint `Theme` global.
//!
//! This is the only place that knows both the theme crate's [`Rgb`] and
//! Slint's colour type; pages just read `Theme.*` properties.

use msi_theme::{Rgb, Theme as SystemTheme};
use slint::{Color, ComponentHandle};

use crate::ui::{AppWindow, Theme as UiTheme};

/// Push a theme into the window. Called once, before `run()`: the UI follows
/// the system theme at startup.
pub fn apply(ui: &AppWindow, theme: &SystemTheme) {
    let g = ui.global::<UiTheme>();
    let t = &theme.tokens;

    g.set_bg(color(t.bg));
    g.set_rail(color(t.rail));
    g.set_surface(color(t.surface));
    g.set_surface_alt(color(t.surface_alt));
    g.set_border(color(t.border));
    g.set_border_strong(color(t.border_strong));

    g.set_text(color(t.text));
    g.set_text_bright(color(t.text_bright));
    g.set_muted(color(t.muted));
    g.set_faint(color(t.faint));

    g.set_accent(color(t.accent));
    g.set_accent_hover(color(t.accent_hover));
    g.set_accent_soft(color(t.accent_soft));
    g.set_on_accent(color(t.on_accent));

    g.set_ok(color(t.ok));
    g.set_warn(color(t.warn));
    g.set_danger(color(t.danger));
    g.set_info(color(t.info));

    g.set_font_family(theme.font.family.as_str().into());

    // Omarchy's type scale is rooted at 12px; keep the app's proportions and
    // follow the user's font size within a sane range.
    let scale = (theme.font.base_size / msi_theme::DEFAULT_BASE_SIZE).clamp(0.9, 1.4);
    g.set_fs_caption(11.0 * scale);
    g.set_fs_small(12.0 * scale);
    g.set_fs_body(13.0 * scale);
    g.set_fs_title(21.0 * scale);
    g.set_fs_value(32.0 * scale);
    g.set_fs_display(40.0 * scale);
}

fn color(c: Rgb) -> Color {
    Color::from_rgb_u8(c.r, c.g, c.b)
}
