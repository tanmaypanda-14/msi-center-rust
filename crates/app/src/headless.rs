//! Execution of the headless commands. Each returns a process exit code.

use msi_core::{FanMode, MsiControl, ShiftMode};
use msi_theme::Theme;

/// `--snapshot`: print everything the app can read.
pub fn snapshot(backend: &dyn MsiControl) -> i32 {
    let snapshot = match backend.snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    println!("backend      {}", backend.name());
    println!("model        {}", snapshot.model);
    println!("ec firmware  {}", snapshot.ec_firmware);
    println!("shift mode   {:?}", snapshot.shift_mode);
    println!("fan mode     {:?}", snapshot.fan_mode);
    println!("cooler boost {}", snapshot.cooler_boost);
    println!("cpu temp     {:?}", snapshot.cpu_temp_c);
    println!("fan rpm      {:?}", snapshot.fan1_rpm);
    println!("fan level    {:?}", snapshot.fan1_percent);
    println!(
        "fan curve    {}",
        snapshot
            .fan_curve
            .map(|curve| curve.speeds_csv())
            .unwrap_or_else(|| "not readable".to_owned())
    );
    println!(
        "manual fan   {}",
        if snapshot.manual_fan_available {
            "available"
        } else {
            "unavailable"
        }
    );
    println!("battery      {:?}%", snapshot.battery_percent);
    println!("charge limit {:?}%", snapshot.battery_limit);
    println!("ac online    {}", snapshot.ac_online);
    println!(
        "kbd light    {:?}/{:?}",
        snapshot.keyboard_brightness, snapshot.keyboard_brightness_max
    );
    println!("webcam block {:?}", snapshot.webcam_blocked);
    println!(
        "fn/win swap  {}",
        match snapshot.win_key_left {
            Some(true) => "swapped (Fn right, Win left)",
            Some(false) => "standard (Fn left, Win right)",
            None => "not exposed",
        }
    );
    0
}

/// `--set key=value`: one write, no window.
pub fn set(backend: &dyn MsiControl, key: &str, value: &str) -> i32 {
    let result = match key {
        "shift" => ShiftMode::parse(value)
            .ok_or_else(|| msi_core::Error::Backend(format!("unknown shift mode {value:?}")))
            .and_then(|mode| backend.set_shift_mode(mode)),
        "fan" => FanMode::parse(value)
            .ok_or_else(|| msi_core::Error::Backend(format!("unknown fan mode {value:?}")))
            .and_then(|mode| backend.set_fan_mode(mode)),
        "cooler" => match bool_arg(value) {
            Some(on) => backend.set_cooler_boost(on),
            None => bad("boolean", value),
        },
        "fanlevel" => match value.parse::<u8>() {
            Ok(percent) => backend.set_manual_fan(percent),
            Err(_) => bad("percent", value),
        },
        "fanrestore" => backend.restore_fan_curve(),
        "battery" => match value.parse::<u8>() {
            Ok(percent) => backend.set_battery_limit(percent),
            Err(_) => bad("percent", value),
        },
        "kbd" => match value.parse::<u8>() {
            Ok(level) => backend.set_keyboard_brightness(level),
            Err(_) => bad("level", value),
        },
        "fnswap" => match value {
            "on" | "swapped" => backend.set_win_key_left(true),
            "off" | "standard" => backend.set_win_key_left(false),
            other => bad("boolean", other),
        },
        "webcam" => match value {
            "on" | "block" => backend.set_webcam_block(true),
            "off" | "unblock" => backend.set_webcam_block(false),
            other => bad("boolean", other),
        },
        other => return fail(&format!("unknown key {other:?}")),
    };

    match result {
        Ok(()) => {
            println!("ok: {key}={value}");
            0
        }
        Err(err) => fail(&err.to_string()),
    }
}

/// `--theme`: show what the UI will draw with.
pub fn theme(theme: &Theme) -> i32 {
    print!("{}", theme.describe());
    0
}

fn bool_arg(value: &str) -> Option<bool> {
    match value {
        "on" | "true" | "1" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn bad(expected: &str, value: &str) -> msi_core::Result<()> {
    Err(msi_core::Error::Backend(format!(
        "expected {expected}, got {value:?}"
    )))
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    1
}
