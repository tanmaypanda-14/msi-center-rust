//! Connect the window's commands to the backend, and keep it fresh.

use std::sync::Arc;
use std::time::Duration;

use msi_core::{FanMode, MsiControl, ShiftMode};
use slint::{ComponentHandle, Timer, TimerMode};

use crate::history::History;
use crate::ui::{AppState, AppWindow};
use crate::view::finish;

/// Wire every callback in [`AppState`] and start the one-second refresh.
///
/// The returned timer must be kept alive for as long as the window runs.
pub fn connect(ui: &AppWindow, backend: Arc<dyn MsiControl>) -> Timer {
    on_set_shift(ui, backend.clone());
    on_set_fan(ui, backend.clone());
    on_toggle_cooler(ui, backend.clone());
    on_set_battery_limit(ui, backend.clone());
    on_cycle_keyboard(ui, backend.clone());
    on_toggle_fnswap(ui, backend.clone());
    on_toggle_webcam(ui, backend.clone());
    start_refresh(ui, backend)
}

fn on_set_shift(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_set_shift(move |mode| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(mode) = ShiftMode::parse(&mode) else {
            return;
        };
        let result = backend.set_shift_mode(mode);
        finish(
            &ui,
            backend.as_ref(),
            result,
            &format!("Power mode set to {}", mode.as_str()),
        );
    });
}

fn on_set_fan(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_set_fan(move |mode| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(mode) = FanMode::parse(&mode) else {
            return;
        };
        let result = backend.set_fan_mode(mode);
        finish(
            &ui,
            backend.as_ref(),
            result,
            &format!("Fan mode set to {}", mode.as_str()),
        );
    });
}

fn on_toggle_cooler(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_toggle_cooler(move || {
        let Some(ui) = weak.upgrade() else { return };
        let next = !backend
            .snapshot()
            .map(|snapshot| snapshot.cooler_boost)
            .unwrap_or(false);
        let result = backend.set_cooler_boost(next);
        let message = if next {
            "Cooler Boost on"
        } else {
            "Cooler Boost off"
        };
        finish(&ui, backend.as_ref(), result, message);
    });
}

fn on_set_battery_limit(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>()
        .on_set_battery_limit(move |percent| {
            let Some(ui) = weak.upgrade() else { return };
            let percent = percent.clamp(50, 100) as u8;
            let result = backend.set_battery_limit(percent);
            finish(
                &ui,
                backend.as_ref(),
                result,
                &format!("Charge limit set to {percent}%"),
            );
        });
}

fn on_cycle_keyboard(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_cycle_keyboard(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Ok(snapshot) = backend.snapshot() else {
            return;
        };
        let max = snapshot.keyboard_brightness_max.unwrap_or(0) as u16;
        if max == 0 {
            ui.global::<AppState>()
                .set_status("Keyboard backlight is not exposed by this EC".into());
            return;
        }
        let current = snapshot.keyboard_brightness.unwrap_or(0) as u16;
        let next = (current + 1) % (max + 1);
        let result = backend.set_keyboard_brightness(next as u8);
        finish(
            &ui,
            backend.as_ref(),
            result,
            &format!("Keyboard backlight: {next} of {max}"),
        );
    });
}

fn on_toggle_fnswap(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_toggle_fnswap(move || {
        let Some(ui) = weak.upgrade() else { return };
        let next = !backend
            .snapshot()
            .ok()
            .and_then(|snapshot| snapshot.win_key_left)
            .unwrap_or(false);
        let result = backend.set_win_key_left(next);
        let message = if next {
            "Fn/Win swapped"
        } else {
            "Fn/Win restored"
        };
        finish(&ui, backend.as_ref(), result, message);
    });
}

fn on_toggle_webcam(ui: &AppWindow, backend: Arc<dyn MsiControl>) {
    let weak = ui.as_weak();
    ui.global::<AppState>().on_toggle_webcam(move || {
        let Some(ui) = weak.upgrade() else { return };
        let next = !backend
            .snapshot()
            .ok()
            .and_then(|snapshot| snapshot.webcam_blocked)
            .unwrap_or(false);
        let result = backend.set_webcam_block(next);
        let message = if next {
            "Webcam blocked"
        } else {
            "Webcam unblocked"
        };
        finish(&ui, backend.as_ref(), result, message);
    });
}

/// One refresh per second: EC reads are cheap and the window only redraws when
/// a value actually changes.
fn start_refresh(ui: &AppWindow, backend: Arc<dyn MsiControl>) -> Timer {
    let timer = Timer::default();
    let weak = ui.as_weak();
    let mut history = History::new();
    let mut quiet_ticks = 0u32;

    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let Some(ui) = weak.upgrade() else { return };
        match backend.snapshot() {
            Ok(snapshot) => {
                crate::view::apply(&ui, &snapshot);
                let state = ui.global::<AppState>();
                if let Some(temp) = snapshot.cpu_temp_c {
                    history.push(temp);
                    state.set_temp_plot(history.plot_path().into());
                }
                // Let a user-facing message linger for a few seconds.
                if !state.get_status().is_empty() {
                    quiet_ticks += 1;
                    if quiet_ticks >= 5 {
                        state.set_status("".into());
                        quiet_ticks = 0;
                    }
                }
            }
            Err(err) => ui.global::<AppState>().set_status(format!("{err}").into()),
        }
    });

    timer
}
