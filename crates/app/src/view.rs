//! Snapshot → window properties.
//!
//! All number formatting happens here; the Slint files only lay out strings.

use msi_core::{MsiControl, Result as CoreResult, Snapshot};
use slint::ComponentHandle;

use crate::ui::{AppState, AppWindow};

/// Re-read the EC and push it into the window.
pub fn refresh(ui: &AppWindow, backend: &dyn MsiControl) {
    match backend.snapshot() {
        Ok(snapshot) => {
            apply(ui, &snapshot);
            ui.global::<AppState>().set_status("".into());
        }
        Err(err) => ui.global::<AppState>().set_status(format!("{err}").into()),
    }
}

/// Apply one write, then refresh; `ok` is shown when the write succeeds.
pub fn finish(ui: &AppWindow, backend: &dyn MsiControl, result: CoreResult<()>, ok: &str) {
    match result {
        Ok(()) => {
            refresh(ui, backend);
            ui.global::<AppState>().set_status(ok.into());
        }
        Err(err) => ui.global::<AppState>().set_status(format!("{err}").into()),
    }
}

pub fn apply(ui: &AppWindow, s: &Snapshot) {
    let state = ui.global::<AppState>();

    state.set_model_name(s.model.as_str().into());
    state.set_subtitle(
        if s.ec_firmware.is_empty() {
            "EC firmware unknown".to_owned()
        } else {
            format!("EC firmware {}", s.ec_firmware)
        }
        .into(),
    );

    state.set_cpu_temp(temp_text(s.cpu_temp_c).into());
    state.set_fan_rpm(rpm_text(s.fan1_rpm).into());
    state.set_fan_level(
        s.fan1_percent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_owned())
            .into(),
    );

    state.set_battery_percent(s.battery_percent.unwrap_or(0) as i32);
    state.set_battery(
        s.battery_percent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_owned())
            .into(),
    );
    state.set_battery_limit(
        s.battery_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_owned())
            .into(),
    );

    state.set_ac_online(s.ac_online);
    state.set_cooler_boost(s.cooler_boost);
    state.set_webcam_blocked(s.webcam_blocked.unwrap_or(false));

    match s.win_key_left {
        Some(left) => {
            state.set_win_key_left(left);
            state.set_fn_win_label(
                if left {
                    "Swapped - Fn right, Win left"
                } else {
                    "Standard - Fn left, Win right"
                }
                .into(),
            );
        }
        None => {
            state.set_win_key_left(false);
            state.set_fn_win_label("not exposed by this EC".into());
        }
    }

    if let Some(mode) = s.shift_mode {
        state.set_shift_mode(mode.as_str().into());
    }
    if let Some(mode) = s.fan_mode {
        state.set_fan_mode(mode.as_str().into());
    }

    let level = s.keyboard_brightness.unwrap_or(0);
    let max = s.keyboard_brightness_max.unwrap_or(0);
    state.set_kbd_level(level as i32);
    state.set_kbd_max(max as i32);
    state.set_kbd_label(
        if max > 0 {
            format!("{level} of {max}")
        } else {
            "not exposed by this EC".to_owned()
        }
        .into(),
    );
}

fn temp_text(v: Option<f32>) -> String {
    match v {
        Some(t) => format!("{t:.0}°C"),
        None => "--".to_owned(),
    }
}

fn rpm_text(v: Option<u32>) -> String {
    match v {
        Some(r) => r.to_string(),
        None => "--".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperatures_are_rounded_to_whole_degrees() {
        assert_eq!(temp_text(Some(49.4)), "49°C");
        assert_eq!(temp_text(Some(49.6)), "50°C");
        assert_eq!(temp_text(None), "--");
    }

    #[test]
    fn rpm_is_plain_digits() {
        assert_eq!(rpm_text(Some(2500)), "2500");
        assert_eq!(rpm_text(None), "--");
    }
}
