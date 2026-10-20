//! Domain model for the MSI Center replacement.
//!
//! Mirrors what MSI Center shows on a Modern 15 B7M (board MS-15HK): power
//! scenario, fan behaviour, thermals, battery care. Hardware specifics live
//! behind [`MsiControl`](crate::backend::MsiControl).

/// MSI "User Scenario" / power mode, as the EC names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftMode {
    /// Super Battery
    Eco,
    /// Balanced
    Comfort,
    /// High Performance
    Sport,
}

impl ShiftMode {
    pub const ALL: [Self; 3] = [Self::Eco, Self::Comfort, Self::Sport];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eco => "eco",
            Self::Comfort => "comfort",
            Self::Sport => "sport",
        }
    }

    /// Label MSI Center uses in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Eco => "Super Battery",
            Self::Comfort => "Balanced",
            Self::Sport => "High Performance",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "eco" => Some(Self::Eco),
            "comfort" => Some(Self::Comfort),
            "sport" => Some(Self::Sport),
            _ => None,
        }
    }
}

/// EC fan mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanMode {
    Auto,
    Silent,
    /// MControlCenter's "Advanced" — fan curve tables are active.
    Advanced,
}

impl FanMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Silent, Self::Advanced];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Silent => "silent",
            Self::Advanced => "advanced",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Silent => "Silent",
            Self::Advanced => "Advanced",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "auto" => Some(Self::Auto),
            "silent" => Some(Self::Silent),
            "advanced" => Some(Self::Advanced),
            _ => None,
        }
    }
}

/// Everything the dashboard needs in one shot.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub model: String,
    pub ec_firmware: String,
    pub shift_mode: Option<ShiftMode>,
    pub fan_mode: Option<FanMode>,
    pub cooler_boost: bool,
    pub cpu_temp_c: Option<f32>,
    pub fan1_rpm: Option<u32>,
    pub fan1_percent: Option<u8>,
    pub battery_percent: Option<u8>,
    pub battery_limit: Option<u8>,
    pub ac_online: bool,
    pub keyboard_brightness: Option<u8>,
    pub keyboard_brightness_max: Option<u8>,
    pub webcam_blocked: Option<bool>,
}

impl Snapshot {
    /// True when the msi-ec platform device was found at all.
    pub fn present(&self) -> bool {
        self.shift_mode.is_some() || self.fan_mode.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_modes_round_trip() {
        for mode in ShiftMode::ALL {
            assert_eq!(ShiftMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(ShiftMode::parse(" turbo "), None);
    }

    #[test]
    fn fan_modes_round_trip() {
        for mode in FanMode::ALL {
            assert_eq!(FanMode::parse(mode.as_str()), Some(mode));
        }
    }
}
