//! Real backend: the `msi-ec` platform driver plus battery/LED sysfs.
//!
//! Register semantics come from the MS-15HK EC as documented by MControlCenter
//! and confirmed on this machine:
//!
//! * fan tachometer   `0xCC/0xCD` (2-byte big-endian tick counter), `rpm = 480000 / ticks`
//! * fan level        `0x71`, exported by the driver as `cpu/realtime_fan_speed` (not RPM)
//! * fan curve        `0x6A..0x6F` temperatures, `0x72..0x78` speeds (EC only)
//! * shift mode       `eco` / `comfort` / `sport`
//! * fan mode         `auto` / `silent` / `advanced`
//!
//! The raw EC file is opened read-only and only when the driver exposes no
//! equivalent; the only writes to raw EC memory go through the setuid
//! `msi-fan-helper`, which can touch the fan-speed registers and nothing else.

use crate::backend::{Error, MsiControl, Result};
use crate::model::{FanCurve, FanMode, ShiftMode, Snapshot};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;

const MEC: &str = "/sys/devices/platform/msi-ec";
const BAT: &str = "/sys/class/power_supply/BAT1";
const ADP: &str = "/sys/class/power_supply/ADP1";
const KBD: &str = "/sys/class/leds/msiacpi::kbd_backlight";
const DMI: &str = "/sys/class/dmi/id";
const RAW_EC_CANDIDATES: [&str; 2] = ["/sys/kernel/debug/ec/ec0/io", "/dev/ec"];

/// setuid helper that owns the fan-curve writes. Checked in order, so a
/// tarball install into /usr/local/bin works as well as the package.
const FAN_HELPERS: [&str; 2] = [
    "/usr/bin/msi-fan-helper",
    "/usr/local/bin/msi-fan-helper",
];
/// Development override, so the app can be pointed at a freshly built helper.
const FAN_HELPER_ENV: &str = "MSI_CENTER_FAN_HELPER";

/// Lowest manual speed the helper accepts; below this the EC would let the CPU
/// throttle rather than cool it.
const MANUAL_MIN: u8 = 20;

/// Fan tachometer register pair.
const FAN_TICK_HI: u64 = 0xCC;
/// Ticks-to-RPM constant used by MControlCenter for this EC generation.
const FAN_TICK_CONST: u32 = 480_000;
/// Anything above this is a torn/garbage read rather than a real fan speed.
const FAN_RPM_CEILING: u32 = 15_000;

/// Fan-curve tables in EC memory.
const CURVE_TEMPS: u8 = 0x6A;
const CURVE_SPEEDS: u8 = 0x72;

pub struct MsiEcBackend {
    root: PathBuf,
    raw_ec: Option<PathBuf>,
}

impl MsiEcBackend {
    /// `None` when the platform driver is not loaded.
    pub fn probe() -> Option<Self> {
        let root = PathBuf::from(MEC);
        if !root.is_dir() {
            return None;
        }
        let raw_ec = RAW_EC_CANDIDATES
            .iter()
            .map(PathBuf::from)
            .find(|p| is_readable(p));
        Some(Self { root, raw_ec })
    }

    pub fn new() -> Result<Self> {
        Self::probe().ok_or_else(|| {
            Error::NotPresent(
                "the msi-ec kernel module is not loaded (modprobe msi-ec)".to_string(),
            )
        })
    }

    fn read_str(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.root.join(rel))
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn read_u32(&self, path: &Path) -> Option<u32> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    fn read_bool(&self, rel: &str) -> Option<bool> {
        self.read_str(rel)
            .map(|v| matches!(v.as_str(), "on" | "1" | "true"))
    }

    fn write_attr(&self, path: &Path, value: &str) -> Result<()> {
        if !path.exists() {
            return Err(Error::Unsupported("attribute not exposed by this EC"));
        }
        fs::write(path, value).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => Error::PermissionDenied {
                path: path.display().to_string(),
            },
            _ => Error::Io(e),
        })
    }

    fn write_mec(&self, rel: &str, value: &str) -> Result<()> {
        self.write_attr(&self.root.join(rel), value)
    }

    /// Real fan RPM from the EC tick counter.
    ///
    /// Preferred source is the driver's `debug/ec_dump` (msi-ec HEAD and later,
    /// no `ec_sys` needed). Older drivers only expose the raw EC image through
    /// debugfs, which is what the fallback reads. `None` when neither works.
    fn fan_rpm(&self) -> Option<u32> {
        if let Some(ticks) = self.ec_dump_ticks() {
            return Some(rpm_from_ticks(ticks));
        }

        let path = self.raw_ec.as_ref()?;
        let mut file = fs::File::open(path).ok()?;
        let mut buf = [0u8; 2];
        file.seek(SeekFrom::Start(FAN_TICK_HI)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(rpm_from_ticks(u16::from_be_bytes(buf)))
    }

    /// Tachometer ticks parsed out of `debug/ec_dump`, if the driver has it.
    fn ec_dump_ticks(&self) -> Option<u16> {
        let dump = fs::read_to_string(self.root.join("debug/ec_dump")).ok()?;
        ticks_from_ec_dump(&dump)
    }

    /// `len` bytes of the raw EC image at `addr`, read-only.
    fn read_ec(&self, addr: u8, len: usize) -> Option<Vec<u8>> {
        let path = self.raw_ec.as_ref()?;
        let mut file = fs::File::open(path).ok()?;
        file.seek(SeekFrom::Start(u64::from(addr))).ok()?;
        let mut buf = vec![0u8; len];
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    /// The live fan-curve table, when the EC image is readable. Needs no
    /// privileges on this board: the tmpfiles rule grants group read.
    fn fan_curve(&self) -> Option<FanCurve> {
        let temps = self.read_ec(CURVE_TEMPS, 6)?;
        let speeds = self.read_ec(CURVE_SPEEDS, 7)?;
        Some(FanCurve {
            temps: temps.try_into().ok()?,
            speeds: speeds.try_into().ok()?,
        })
    }

    /// A manual speed needs both the helper and the driver's raw-write node.
    fn manual_fan_available(&self) -> bool {
        helper_path().exists() && self.root.join("debug/ec_set").exists()
    }

    /// Remember the curve as it was before the first manual override, so
    /// "restore" puts back what this EC actually shipped with.
    fn save_curve_once(&self) {
        let Some(path) = state_file() else {
            return;
        };
        if path.exists() {
            return;
        }
        let Some(curve) = self.fan_curve() else {
            return;
        };
        if curve.flat_percent().is_some() {
            return; // already an override; never overwrite the original
        }
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(path, curve.speeds_csv());
    }

    fn saved_curve(&self) -> Option<FanCurve> {
        let raw = fs::read_to_string(state_file()?).ok()?;
        parse_speeds(&raw).map(|speeds| FanCurve {
            speeds,
            ..FanCurve::FACTORY
        })
    }

    fn battery_path(&self, name: &str) -> PathBuf {
        Path::new(BAT).join(name)
    }
}

/// The first installed helper: `/usr/bin/msi-fan-helper`, falling back to
/// `/usr/local/bin`, or the path in `MSI_CENTER_FAN_HELPER` while developing.
/// The helper itself never reads the environment.
fn helper_path() -> PathBuf {
    if let Some(path) = std::env::var_os(FAN_HELPER_ENV).map(PathBuf::from) {
        if path.exists() {
            return path;
        }
    }
    FAN_HELPERS
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(FAN_HELPERS[0]))
}

/// `$XDG_STATE_HOME/msi-center/fan-curve.orig`, per the XDG base-dir spec.
fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(base.join("msi-center").join("fan-curve.orig"))
}

fn parse_speeds(raw: &str) -> Option<[u8; 7]> {
    let mut out = [0u8; 7];
    let mut seen = 0;
    for (index, part) in raw.trim().split(',').enumerate() {
        if index >= 7 {
            return None;
        }
        out[index] = part.trim().parse().ok()?;
        seen += 1;
    }
    (seen == 7).then_some(out)
}

/// Run the setuid helper and surface its stderr verbatim.
fn run_helper(args: &[&str]) -> Result<()> {
    let path = helper_path();
    if !path.exists() {
        return Err(Error::Backend(format!(
            "{} is not installed - reinstall msi-center to get the fan helper",
            path.display()
        )));
    }

    let output = Command::new(&path).args(args).output().map_err(Error::Io)?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    Err(Error::Backend(if detail.is_empty() {
        format!("{} exited with {}", path.display(), output.status)
    } else {
        detail.to_owned()
    }))
}

fn is_readable(path: &Path) -> bool {
    fs::File::open(path).is_ok()
}

impl MsiControl for MsiEcBackend {
    fn name(&self) -> &'static str {
        "msi-ec"
    }

    fn snapshot(&self) -> Result<Snapshot> {
        let mut snap = Snapshot {
            model: read_trimmed(Path::new(DMI).join("product_name").as_path()),
            ..Snapshot::default()
        };
        snap.ec_firmware = self.read_str("fw_version").unwrap_or_default();

        snap.shift_mode = self
            .read_str("shift_mode")
            .as_deref()
            .and_then(ShiftMode::parse);
        snap.fan_mode = self
            .read_str("fan_mode")
            .as_deref()
            .and_then(FanMode::parse);
        snap.cooler_boost = self.read_bool("cooler_boost").unwrap_or(false);
        snap.webcam_blocked = self.read_bool("webcam_block");
        snap.win_key_left = self.read_str("win_key").map(|position| position == "left");

        snap.cpu_temp_c = self
            .read_u32(self.root.join("cpu/realtime_temperature").as_path())
            .map(|v| v as f32);

        // NOTE: realtime_fan_speed is the EC's thermal/fan *level* at 0x71,
        // not RPM. Kept separate from fan1_rpm so the UI never conflates them.
        snap.fan1_percent = self
            .read_u32(self.root.join("cpu/realtime_fan_speed").as_path())
            .map(|v| v.min(100) as u8);

        snap.fan1_rpm = self.fan_rpm();
        snap.fan_curve = self.fan_curve();
        snap.manual_fan_available = self.manual_fan_available();

        snap.battery_percent = self
            .read_u32(self.battery_path("capacity").as_path())
            .map(|v| v.min(100) as u8);
        snap.battery_limit = self
            .read_u32(self.battery_path("charge_control_end_threshold").as_path())
            .map(|v| v.min(100) as u8);
        snap.ac_online = self
            .read_u32(Path::new(ADP).join("online").as_path())
            .map(|v| v == 1)
            .unwrap_or(false);

        let kbd = Path::new(KBD);
        snap.keyboard_brightness = self
            .read_u32(kbd.join("brightness").as_path())
            .map(|v| v.min(255) as u8);
        snap.keyboard_brightness_max = self
            .read_u32(kbd.join("max_brightness").as_path())
            .map(|v| v.min(255) as u8);

        Ok(snap)
    }

    fn set_shift_mode(&self, mode: ShiftMode) -> Result<()> {
        self.write_mec("shift_mode", mode.as_str())
    }

    fn set_fan_mode(&self, mode: FanMode) -> Result<()> {
        self.write_mec("fan_mode", mode.as_str())
    }

    fn set_cooler_boost(&self, on: bool) -> Result<()> {
        self.write_mec("cooler_boost", if on { "on" } else { "off" })
    }

    fn set_manual_fan(&self, percent: u8) -> Result<()> {
        if !(MANUAL_MIN..=100).contains(&percent) {
            return Err(Error::Backend(format!(
                "manual fan speed {percent}% is out of range ({MANUAL_MIN}-100)"
            )));
        }
        self.save_curve_once();
        run_helper(&["manual", &percent.to_string()])
    }

    fn restore_fan_curve(&self) -> Result<()> {
        let curve = self.saved_curve().unwrap_or(FanCurve::FACTORY);
        run_helper(&["curve", &curve.speeds_csv()])
    }

    fn set_battery_limit(&self, percent: u8) -> Result<()> {
        if !(50..=100).contains(&percent) {
            return Err(Error::Backend(format!(
                "charge limit {percent}% out of range"
            )));
        }
        self.write_attr(
            self.battery_path("charge_control_end_threshold").as_path(),
            &percent.to_string(),
        )
    }

    fn set_keyboard_brightness(&self, level: u8) -> Result<()> {
        self.write_attr(
            Path::new(KBD).join("brightness").as_path(),
            &level.to_string(),
        )
    }

    fn set_webcam_block(&self, blocked: bool) -> Result<()> {
        self.write_mec("webcam_block", if blocked { "on" } else { "off" })
    }

    fn set_win_key_left(&self, left: bool) -> Result<()> {
        self.write_mec("win_key", if left { "left" } else { "right" })
    }
}

fn read_trimmed(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// `480000 / ticks`, with the sanity guard shared by both RPM sources.
fn rpm_from_ticks(ticks: u16) -> u32 {
    if ticks == 0 {
        return 0; // fan stopped
    }
    let rpm = FAN_TICK_CONST / u32::from(ticks);
    if rpm > FAN_RPM_CEILING {
        return 0;
    }
    rpm
}

/// Pull the tachometer byte pair out of msi-ec's `debug/ec_dump` table.
///
/// The table looks like:
/// ```text
/// | 0xc_ | 00 00 00 00 00 00 2e 01 00 00 00 00 00 8a 00 00  |............|
/// ```
fn ticks_from_ec_dump(dump: &str) -> Option<u16> {
    let row = dump
        .lines()
        .find(|line| line.trim_start().starts_with("| 0xc_ |"))?;

    // | 0xc_ | <16 hex bytes> |ascii|
    let cells = row.split('|').nth(2)?;
    let bytes: Vec<u8> = cells
        .split_whitespace()
        .map(|cell| u8::from_str_radix(cell, 16))
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    if bytes.len() < 16 {
        return None;
    }

    Some(u16::from_be_bytes([bytes[12], bytes[13]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "\
|      | _0 _1 _2 _3 _4 _5 _6 _7 _8 _9 _a _b _c _d _e _f
|------+------------------------------------------------
| 0x0_ | 00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f  |................|
| 0xc_ | 00 00 00 00 00 00 2e 01 00 00 00 00 00 8a 00 00  |............|
";

    #[test]
    fn reads_ticks_from_the_c_row() {
        assert_eq!(ticks_from_ec_dump(DUMP), Some(0x008a));
    }

    #[test]
    fn missing_row_is_not_an_error() {
        assert_eq!(ticks_from_ec_dump("nonsense"), None);
    }

    #[test]
    fn converts_ticks_to_rpm() {
        assert_eq!(rpm_from_ticks(0), 0);
        assert_eq!(rpm_from_ticks(120), 4000);
        // 480000 / 1 is nonsense and must not reach the UI
        assert_eq!(rpm_from_ticks(1), 0);
    }

    #[test]
    fn parses_seven_speed_points() {
        assert_eq!(
            parse_speeds("0,50,60,65,75,75,100\n"),
            Some([0, 50, 60, 65, 75, 75, 100])
        );
        assert_eq!(parse_speeds("0,50,60"), None);
        assert_eq!(parse_speeds("0,50,60,65,75,75,100,1"), None);
        assert_eq!(parse_speeds("0,50,60,65,75,75,x"), None);
    }
}
