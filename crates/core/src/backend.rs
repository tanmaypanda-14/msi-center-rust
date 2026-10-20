//! The seam between the UI and the Embedded Controller.

use crate::model::{FanMode, ShiftMode, Snapshot};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("msi-ec is not available: {0}")]
    NotPresent(String),
    #[error("unsupported by this EC: {0}")]
    Unsupported(&'static str),
    #[error("{path} is not writable - check the msi-ec udev rule")]
    PermissionDenied { path: String },
    #[error("backend failure: {0}")]
    Backend(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Everything the app can do to the machine.
pub trait MsiControl: Send + Sync {
    /// Short identifier, e.g. `msi-ec`, `mock`.
    fn name(&self) -> &'static str;

    /// Read the full current state in one pass.
    fn snapshot(&self) -> Result<Snapshot>;

    fn set_shift_mode(&self, mode: ShiftMode) -> Result<()>;
    fn set_fan_mode(&self, mode: FanMode) -> Result<()>;
    fn set_cooler_boost(&self, on: bool) -> Result<()>;

    fn set_battery_limit(&self, _percent: u8) -> Result<()> {
        Err(Error::Unsupported("battery charge limit"))
    }

    fn set_keyboard_brightness(&self, _level: u8) -> Result<()> {
        Err(Error::Unsupported("keyboard backlight"))
    }

    fn set_webcam_block(&self, _blocked: bool) -> Result<()> {
        Err(Error::Unsupported("webcam block"))
    }

    fn set_win_key_left(&self, _left: bool) -> Result<()> {
        Err(Error::Unsupported("Fn/Win swap"))
    }
}
