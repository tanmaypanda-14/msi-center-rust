//! Privileged helper for the EC fan curve.
//!
//! Installed setuid root, because `msi-ec` exposes raw EC writes only to root
//! (`debug/ec_set`). It does exactly one job: write the seven fan-speed bytes
//! at `0x72..0x78` and put the EC into "advanced" fan mode so that table is
//! the active one.
//!
//! There is deliberately no address/value interface. The caller can choose a
//! fan speed, nothing else, so the worst a compromised caller achieves is a
//! loud fan or a slow one — never a write to an unrelated EC register.
//!
//! No environment variables, no PATH lookup, no files opened by name from the
//! caller: every path below is absolute and compiled in.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// EC address of the first of the seven fan-speed points.
const SPEED_BASE: u8 = 0x72;
/// MSI's EC keeps seven speed points (6 temperature breakpoints).
const POINTS: usize = 7;

const MEC: &str = "/sys/devices/platform/msi-ec";
const EC_SET: &str = "/sys/devices/platform/msi-ec/debug/ec_set";
const FAN_MODE: &str = "/sys/devices/platform/msi-ec/fan_mode";

/// Refuse to bake a curve that pins the fan off; the EC would then let the
/// CPU throttle into its thermal limit instead of cooling it.
const MANUAL_MIN: u8 = 20;

const USAGE: &str = "\
usage: msi-fan-helper manual <20-100>
       msi-fan-helper curve <s0,s1,s2,s3,s4,s5,s6>
       msi-fan-helper check

  manual  set every curve point to one speed, so the fan holds that speed
  curve   restore seven explicit speed points (each 0-100)
  check   exit 0 when the EC write path is available, print why when it is not

Requires msi-ec loaded with debug=1 (packaged as /usr/lib/modprobe.d/msi-ec.conf).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("manual") => args
            .get(1)
            .ok_or_else(|| "manual needs a percentage".to_string())
            .and_then(|raw| parse_percent(raw))
            .and_then(|percent| apply(&[percent; POINTS]).map(|()| format!("manual {percent}%"))),
        Some("curve") => args
            .get(1)
            .ok_or_else(|| "curve needs seven comma-separated values".to_string())
            .and_then(|raw| parse_curve(raw))
            .and_then(|curve| apply(&curve).map(|()| format!("curve {curve:?}"))),
        Some("check") => check(),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(message) => {
            println!("ok: {message}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("msi-fan-helper: {err}");
            ExitCode::FAILURE
        }
    }
}

fn check() -> Result<String, String> {
    if !Path::new(MEC).is_dir() {
        return Err("msi-ec is not loaded".to_string());
    }
    if !Path::new(EC_SET).exists() {
        return Err(format!(
            "{EC_SET} is missing - load msi-ec with debug=1 \
             (packaged as /usr/lib/modprobe.d/msi-ec.conf) and reload it"
        ));
    }
    Ok("EC write path available".to_string())
}

/// Write the seven speed points, then latch them by selecting advanced mode.
fn apply(speeds: &[u8; POINTS]) -> Result<(), String> {
    check()?;

    for (offset, speed) in speeds.iter().enumerate() {
        ec_write(SPEED_BASE + offset as u8, *speed)?;
    }

    // The table is only consulted in advanced mode; writing the mode byte is
    // also what makes the EC re-read it right away.
    fs::write(FAN_MODE, "advanced")
        .map_err(|e| format!("selecting advanced fan mode failed: {e}"))?;

    Ok(())
}

fn ec_write(addr: u8, value: u8) -> Result<(), String> {
    let payload = format!("{addr:02x}={value:02x}");
    fs::write(EC_SET, payload).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "not permitted - msi-fan-helper must be installed setuid root".to_string()
        }
        _ => format!("writing EC {addr:#04x} failed: {e}"),
    })
}

fn parse_percent(raw: &str) -> Result<u8, String> {
    let value: u8 = raw
        .trim()
        .parse()
        .map_err(|_| format!("{raw:?} is not a number"))?;
    if !(MANUAL_MIN..=100).contains(&value) {
        return Err(format!(
            "manual speed {value} is out of range ({MANUAL_MIN}-100)"
        ));
    }
    Ok(value)
}

fn parse_curve(raw: &str) -> Result<[u8; POINTS], String> {
    let mut out = [0u8; POINTS];
    let mut seen = 0;
    for (index, part) in raw.split(',').enumerate() {
        if index >= POINTS {
            return Err(format!("expected {POINTS} values, got more"));
        }
        out[index] = part
            .trim()
            .parse::<u8>()
            .map_err(|_| format!("{:?} is not a number", part.trim()))?;
        if out[index] > 100 {
            return Err(format!("speed {} is above 100", out[index]));
        }
        seen += 1;
    }
    if seen != POINTS {
        return Err(format!("expected {POINTS} values, got {seen}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_speed_is_bounded() {
        assert_eq!(parse_percent("20").unwrap(), 20);
        assert_eq!(parse_percent("100").unwrap(), 100);
        assert!(parse_percent("19").is_err());
        assert!(parse_percent("101").is_err());
        assert!(parse_percent("loud").is_err());
    }

    #[test]
    fn curve_needs_exactly_seven_points() {
        assert_eq!(
            parse_curve("0,50,60,65,75,75,100").unwrap(),
            [0, 50, 60, 65, 75, 75, 100]
        );
        assert!(parse_curve("0,50,60").is_err());
        assert!(parse_curve("0,50,60,65,75,75,100,50").is_err());
        assert!(parse_curve("0,50,60,65,75,75,101").is_err());
        assert!(parse_curve("0,50,60,65,75,75,x").is_err());
    }

    #[test]
    fn registers_are_the_documented_fan_curve() {
        // Guard against a refactor silently moving the write target.
        let last = SPEED_BASE + (POINTS - 1) as u8;
        assert_eq!((SPEED_BASE, last), (0x72, 0x78));
    }
}
