//! Core model and MSI EC control for the MSI Center replacement.

pub mod backend;
pub mod mock;
pub mod model;
pub mod platform;

pub use backend::{Error, MsiControl, Result};
pub use model::{FanMode, ShiftMode, Snapshot};

/// The best backend for this machine.
pub fn default_backend() -> Box<dyn MsiControl> {
    platform::default_backend()
}
