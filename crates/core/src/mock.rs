//! Deterministic backend for development and tests (used when msi-ec is absent,
//! e.g. inside the build container).

use crate::backend::{MsiControl, Result};
use crate::model::{FanCurve, FanMode, ShiftMode, Snapshot};
use std::sync::Mutex;

pub struct MockBackend {
    shift: Mutex<ShiftMode>,
    fan: Mutex<FanMode>,
    cooler: Mutex<bool>,
    manual: Mutex<Option<u8>>,
    limit: Mutex<u8>,
    kbd: Mutex<u8>,
    win_left: Mutex<bool>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            shift: Mutex::new(ShiftMode::Comfort),
            fan: Mutex::new(FanMode::Advanced),
            cooler: Mutex::new(false),
            manual: Mutex::new(None),
            limit: Mutex::new(80),
            kbd: Mutex::new(1),
            win_left: Mutex::new(false),
        }
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MsiControl for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn snapshot(&self) -> Result<Snapshot> {
        let manual = *self.manual.lock().unwrap();
        Ok(Snapshot {
            model: "MSI Modern 15 B7M (mock)".into(),
            ec_firmware: "15HKEMS1.104".into(),
            shift_mode: Some(*self.shift.lock().unwrap()),
            fan_mode: Some(*self.fan.lock().unwrap()),
            cooler_boost: *self.cooler.lock().unwrap(),
            cpu_temp_c: Some(56.0),
            fan1_rpm: Some(2875),
            fan1_percent: Some(manual.unwrap_or(55)),
            fan_curve: Some(match manual {
                Some(percent) => FanCurve {
                    speeds: [percent; 7],
                    ..FanCurve::FACTORY
                },
                None => FanCurve::FACTORY,
            }),
            manual_fan_available: true,
            battery_percent: Some(87),
            battery_limit: Some(*self.limit.lock().unwrap()),
            ac_online: true,
            keyboard_brightness: Some(*self.kbd.lock().unwrap()),
            keyboard_brightness_max: Some(3),
            webcam_blocked: Some(false),
            win_key_left: Some(*self.win_left.lock().unwrap()),
        })
    }

    fn set_shift_mode(&self, mode: ShiftMode) -> Result<()> {
        *self.shift.lock().unwrap() = mode;
        Ok(())
    }

    fn set_fan_mode(&self, mode: FanMode) -> Result<()> {
        *self.fan.lock().unwrap() = mode;
        Ok(())
    }

    fn set_cooler_boost(&self, on: bool) -> Result<()> {
        *self.cooler.lock().unwrap() = on;
        Ok(())
    }

    fn set_manual_fan(&self, percent: u8) -> Result<()> {
        *self.manual.lock().unwrap() = Some(percent);
        *self.fan.lock().unwrap() = FanMode::Advanced;
        Ok(())
    }

    fn restore_fan_curve(&self) -> Result<()> {
        *self.manual.lock().unwrap() = None;
        Ok(())
    }

    fn set_battery_limit(&self, percent: u8) -> Result<()> {
        *self.limit.lock().unwrap() = percent;
        Ok(())
    }

    fn set_keyboard_brightness(&self, level: u8) -> Result<()> {
        *self.kbd.lock().unwrap() = level;
        Ok(())
    }

    fn set_webcam_block(&self, _blocked: bool) -> Result<()> {
        Ok(())
    }

    fn set_win_key_left(&self, left: bool) -> Result<()> {
        *self.win_left.lock().unwrap() = left;
        Ok(())
    }
}
