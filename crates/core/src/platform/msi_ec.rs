//! Real backend: the `msi-ec` platform driver plus battery/LED sysfs.
//!
//! Register semantics come from the MS-15HK EC as documented by MControlCenter
//! and confirmed on this machine:
//!
//! * fan tachometer   `0xCC/0xCD` (2-byte big-endian tick counter), `rpm = 480000 / ticks`
//! * fan level        `0x71`, exported by the driver as `cpu/realtime_fan_speed` (not RPM)
//! * shift mode       `eco` / `comfort` / `sport`
//! * fan mode         `auto` / `silent` / `advanced`
//!
//! The raw EC file is opened read-only and only when the driver exposes no
//! equivalent; nothing here writes to raw EC memory.

use crate::backend::{Error, MsiControl, Result};
use crate::model::{FanMode, ShiftMode, Snapshot};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MEC: &str = "/sys/devices/platform/msi-ec";
const BAT: &str = "/sys/class/power_supply/BAT1";
const ADP: &str = "/sys/class/power_supply/ADP1";
const KBD: &str = "/sys/class/leds/msiacpi::kbd_backlight";
const DMI: &str = "/sys/class/dmi/id";
const RAW_EC_CANDIDATES: [&str; 2] = ["/sys/kernel/debug/ec/ec0/io", "/dev/ec"];

/// Fan tachometer register pair.
const FAN_TICK_HI: u64 = 0xCC;
/// Ticks-to-RPM constant used by MControlCenter for this EC generation.
const FAN_TICK_CONST: u32 = 480_000;
/// Anything above this is a torn/garbage read rather than a real fan speed.
const FAN_RPM_CEILING: u32 = 15_000;

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

    /// Real fan RPM from the EC tick counter, or `None` when the raw EC is not
    /// readable (then the UI simply hides the RPM row).
    fn fan_rpm(&self) -> Option<u32> {
        let path = self.raw_ec.as_ref()?;
        let mut file = fs::File::open(path).ok()?;
        let mut buf = [0u8; 2];
        file.seek(SeekFrom::Start(FAN_TICK_HI)).ok()?;
        file.read_exact(&mut buf).ok()?;
        let ticks = u32::from(u16::from_be_bytes(buf));
        if ticks == 0 {
            return Some(0); // fan stopped
        }
        let rpm = FAN_TICK_CONST / ticks;
        if rpm > FAN_RPM_CEILING {
            return Some(0);
        }
        Some(rpm)
    }

    fn battery_path(&self, name: &str) -> PathBuf {
        Path::new(BAT).join(name)
    }
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

        snap.cpu_temp_c = self
            .read_u32(self.root.join("cpu/realtime_temperature").as_path())
            .map(|v| v as f32);

        // NOTE: realtime_fan_speed is the EC's thermal/fan *level* at 0x71,
        // not RPM. Kept separate from fan1_rpm so the UI never conflates them.
        snap.fan1_percent = self
            .read_u32(self.root.join("cpu/realtime_fan_speed").as_path())
            .map(|v| v.min(100) as u8);

        snap.fan1_rpm = self.fan_rpm();

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
}

fn read_trimmed(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
