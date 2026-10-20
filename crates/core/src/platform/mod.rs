//! Platform selection.

use crate::backend::MsiControl;

#[cfg(target_os = "linux")]
pub mod msi_ec;

/// The real backend when msi-ec is loaded, otherwise a mock so the UI can be
/// developed anywhere.
pub fn default_backend() -> Box<dyn MsiControl> {
    #[cfg(target_os = "linux")]
    {
        match msi_ec::MsiEcBackend::probe() {
            Some(backend) => Box::new(backend),
            None => Box::new(crate::mock::MockBackend::new()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(crate::mock::MockBackend::new())
    }
}
