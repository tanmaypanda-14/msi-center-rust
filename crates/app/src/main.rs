//! MSI Center replacement - application shell.
//!
//! One process: a Slint window reading the EC through `msi-core`, refreshing
//! once a second. No webview, no runtime, no background services.

slint::include_modules!();

use msi_core::{default_backend, FanMode, MsiControl, Result as CoreResult, ShiftMode, Snapshot};
use slint::{ComponentHandle, Timer, TimerMode};
use std::sync::Arc;
use std::time::Duration;

fn temp_text(v: Option<f32>) -> String {
    match v {
        Some(t) => format!("{t:.0}°C"),
        None => "--".to_string(),
    }
}

fn rpm_text(v: Option<u32>) -> String {
    match v {
        Some(r) => r.to_string(),
        None => "--".to_string(),
    }
}

fn apply(ui: &AppWindow, s: &Snapshot) {
    ui.set_model_name(s.model.clone().into());

    let fw = if s.ec_firmware.is_empty() {
        "EC firmware unknown".to_string()
    } else {
        format!("EC firmware {}", s.ec_firmware)
    };
    ui.set_subtitle(fw.into());

    ui.set_cpu_temp(temp_text(s.cpu_temp_c).into());
    ui.set_fan_rpm(rpm_text(s.fan1_rpm).into());
    ui.set_fan_level(
        s.fan1_percent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_string())
            .into(),
    );
    ui.set_battery_percent(s.battery_percent.unwrap_or(0) as i32);
    ui.set_battery(
        s.battery_percent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_string())
            .into(),
    );
    ui.set_battery_limit(
        s.battery_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "--".to_string())
            .into(),
    );

    ui.set_ac_online(s.ac_online);
    ui.set_cooler_boost(s.cooler_boost);
    ui.set_webcam_blocked(s.webcam_blocked.unwrap_or(false));

    if let Some(mode) = s.shift_mode {
        ui.set_shift_mode(mode.as_str().into());
    }
    if let Some(mode) = s.fan_mode {
        ui.set_fan_mode(mode.as_str().into());
    }

    let level = s.keyboard_brightness.unwrap_or(0);
    let max = s.keyboard_brightness_max.unwrap_or(0);
    ui.set_kbd_level(level as i32);
    ui.set_kbd_max(max as i32);
    ui.set_kbd_label(
        if max > 0 {
            format!("{} of {}", level, max)
        } else {
            "not exposed by this EC".to_string()
        }
        .into(),
    );
}

/// Re-read the EC and push it into the window.
fn refresh(ui: &AppWindow, backend: &dyn MsiControl) {
    match backend.snapshot() {
        Ok(s) => {
            apply(ui, &s);
            ui.set_status("".into());
        }
        Err(e) => ui.set_status(format!("{e}").into()),
    }
}

/// Apply one write, then refresh; `ok` is shown when the write succeeds.
fn finish(ui: &AppWindow, backend: &dyn MsiControl, result: CoreResult<()>, ok: &str) {
    match result {
        Ok(()) => {
            refresh(ui, backend);
            ui.set_status(ok.into());
        }
        Err(e) => ui.set_status(format!("{e}").into()),
    }
}

fn main() -> std::result::Result<(), slint::PlatformError> {
    let backend: Arc<Box<dyn MsiControl>> = Arc::new(default_backend());

    // Headless check: `msi-center --snapshot` prints what the app sees and exits.
    if std::env::args().any(|a| a == "--snapshot") {
        match backend.snapshot() {
            Ok(s) => {
                println!("backend      {}", backend.name());
                println!("model        {}", s.model);
                println!("ec firmware  {}", s.ec_firmware);
                println!("shift mode   {:?}", s.shift_mode);
                println!("fan mode     {:?}", s.fan_mode);
                println!("cooler boost {}", s.cooler_boost);
                println!("cpu temp     {:?}", s.cpu_temp_c);
                println!("fan rpm      {:?}", s.fan1_rpm);
                println!("fan level    {:?}", s.fan1_percent);
                println!("battery      {:?}%", s.battery_percent);
                println!("charge limit {:?}%", s.battery_limit);
                println!("ac online    {}", s.ac_online);
                println!(
                    "kbd light    {:?}/{:?}",
                    s.keyboard_brightness, s.keyboard_brightness_max
                );
                println!("webcam block {:?}", s.webcam_blocked);
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Headless control: `msi-center --set fan=silent` (also handy for scripts).
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--set") {
        let Some(spec) = args.get(pos + 1) else {
            eprintln!("usage: msi-center --set key=value");
            std::process::exit(2);
        };
        let Some((key, value)) = spec.split_once('=') else {
            eprintln!("expected key=value, got {spec:?}");
            std::process::exit(2);
        };
        let result = match key {
            "shift" => ShiftMode::parse(value)
                .ok_or_else(|| msi_core::Error::Backend(format!("unknown shift mode {value:?}")))
                .and_then(|m| backend.set_shift_mode(m)),
            "fan" => FanMode::parse(value)
                .ok_or_else(|| msi_core::Error::Backend(format!("unknown fan mode {value:?}")))
                .and_then(|m| backend.set_fan_mode(m)),
            "cooler" => match value {
                "on" | "true" | "1" => backend.set_cooler_boost(true),
                "off" | "false" | "0" => backend.set_cooler_boost(false),
                _ => Err(msi_core::Error::Backend(format!("bad bool {value:?}"))),
            },
            "battery" => value
                .parse::<u8>()
                .map_err(|_| msi_core::Error::Backend(format!("bad percent {value:?}")))
                .and_then(|p| backend.set_battery_limit(p)),
            "kbd" => value
                .parse::<u8>()
                .map_err(|_| msi_core::Error::Backend(format!("bad level {value:?}")))
                .and_then(|l| backend.set_keyboard_brightness(l)),
            "webcam" => match value {
                "on" | "block" => backend.set_webcam_block(true),
                "off" | "unblock" => backend.set_webcam_block(false),
                _ => Err(msi_core::Error::Backend(format!("bad bool {value:?}"))),
            },
            other => Err(msi_core::Error::Backend(format!("unknown key {other:?}"))),
        };
        match result {
            Ok(()) => {
                println!("ok: {spec}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }

    let ui = AppWindow::new()?;

    // Give the window a real Wayland app_id so the compositor can apply rules
    // and group it like any other app (must happen before the window is shown).
    let _ = slint::set_xdg_app_id("msi-center");

    // `--page N` opens straight to a page; used for screenshots and demos.
    if let Some(pos) = args.iter().position(|a| a == "--page") {
        if let Some(page) = args.get(pos + 1).and_then(|p| p.parse::<i32>().ok()) {
            ui.set_page(page.clamp(0, 3));
        }
    }

    ui.set_backend_label(backend.name().into());
    refresh(&ui, &**backend);

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_set_shift(move |mode| {
            let Some(ui) = weak.upgrade() else { return };
            let result = match ShiftMode::parse(&mode) {
                Some(m) => backend.set_shift_mode(m),
                None => return,
            };
            finish(
                &ui,
                &**backend,
                result,
                &format!("Power mode set to {}", mode),
            );
        });
    }

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_set_fan(move |mode| {
            let Some(ui) = weak.upgrade() else { return };
            let result = match FanMode::parse(&mode) {
                Some(m) => backend.set_fan_mode(m),
                None => return,
            };
            finish(
                &ui,
                &**backend,
                result,
                &format!("Fan mode set to {}", mode),
            );
        });
    }

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_toggle_cooler(move || {
            let Some(ui) = weak.upgrade() else { return };
            let current = backend
                .snapshot()
                .ok()
                .map(|s| s.cooler_boost)
                .unwrap_or(false);
            let next = !current;
            let result = backend.set_cooler_boost(next);
            let msg = if next {
                "Cooler Boost on"
            } else {
                "Cooler Boost off"
            };
            finish(&ui, &**backend, result, msg);
        });
    }

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_set_battery_limit(move |percent| {
            let Some(ui) = weak.upgrade() else { return };
            let percent = percent.clamp(50, 100) as u8;
            let result = backend.set_battery_limit(percent);
            finish(
                &ui,
                &**backend,
                result,
                &format!("Charge limit set to {percent}%"),
            );
        });
    }

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_cycle_keyboard(move || {
            let Some(ui) = weak.upgrade() else { return };
            let Ok(snapshot) = backend.snapshot() else {
                return;
            };
            let max = snapshot.keyboard_brightness_max.unwrap_or(0) as u16;
            if max == 0 {
                ui.set_status("Keyboard backlight is not exposed by this EC".into());
                return;
            }
            let current = snapshot.keyboard_brightness.unwrap_or(0) as u16;
            let next = (current + 1) % (max + 1);
            let result = backend.set_keyboard_brightness(next as u8);
            finish(
                &ui,
                &**backend,
                result,
                &format!("Keyboard backlight: {next} of {max}"),
            );
        });
    }

    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        ui.on_toggle_webcam(move || {
            let Some(ui) = weak.upgrade() else { return };
            let current = backend
                .snapshot()
                .ok()
                .and_then(|s| s.webcam_blocked)
                .unwrap_or(false);
            let next = !current;
            let result = backend.set_webcam_block(next);
            let msg = if next {
                "Webcam blocked"
            } else {
                "Webcam unblocked"
            };
            finish(&ui, &**backend, result, msg);
        });
    }

    // One refresh per second: EC reads are cheap, and nothing here is animated
    // by the timer, so the window only redraws when a value actually changes.
    let timer = Timer::default();
    {
        let backend = backend.clone();
        let weak = ui.as_weak();
        let mut history: std::collections::VecDeque<f32> = std::collections::VecDeque::new();
        let mut quiet_ticks = 0u32;
        timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let Some(ui) = weak.upgrade() else { return };
            match backend.snapshot() {
                Ok(s) => {
                    apply(&ui, &s);
                    if let Some(temp) = s.cpu_temp_c {
                        if history.len() >= HISTORY_LEN {
                            history.pop_front();
                        }
                        history.push_back(temp);
                        ui.set_temp_plot(plot_path(&history).into());
                    }
                    // Let a user-facing message linger for a few seconds.
                    if !ui.get_status().is_empty() {
                        quiet_ticks += 1;
                        if quiet_ticks >= 5 {
                            ui.set_status("".into());
                            quiet_ticks = 0;
                        }
                    }
                }
                Err(e) => ui.set_status(format!("{e}").into()),
            }
        });
    }

    ui.run()
}

/// Two minutes of one-second samples.
const HISTORY_LEN: usize = 120;

/// Build an SVG-ish polyline in a 1000x100 viewbox, 30-100 °C mapped to the
/// full height. Slint scales it to the card.
fn plot_path(history: &std::collections::VecDeque<f32>) -> String {
    if history.len() < 2 {
        return String::new();
    }
    let step = 1000.0 / (history.len() - 1) as f32;
    let mut path = String::with_capacity(history.len() * 12);
    for (i, temp) in history.iter().enumerate() {
        let x = i as f32 * step;
        let normalized = ((temp.clamp(30.0, 100.0) - 30.0) / 70.0).clamp(0.0, 1.0);
        let y = 100.0 - normalized * 100.0;
        path.push_str(if i == 0 { "M" } else { " L" });
        path.push_str(&format!("{x:.0} {y:.0}"));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn empty_or_single_sample_draws_nothing() {
        let mut h = VecDeque::new();
        assert!(plot_path(&h).is_empty());
        h.push_back(50.0);
        assert!(plot_path(&h).is_empty());
    }

    #[test]
    fn plot_maps_full_scale_to_the_viewbox() {
        let h: VecDeque<f32> = [30.0, 100.0].into_iter().collect();
        assert_eq!(plot_path(&h), "M0 100 L1000 0");
    }

    #[test]
    fn plot_clamps_out_of_range_samples() {
        let h: VecDeque<f32> = [10.0, 120.0, 65.0].into_iter().collect();
        assert_eq!(plot_path(&h), "M0 100 L500 0 L1000 50");
    }
}
