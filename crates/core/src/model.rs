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

/// The EC fan curve: six temperature breakpoints and seven speed points.
///
/// The EC picks `speeds[0]` below `temps[0]`, `speeds[i]` between
/// `temps[i-1]` and `temps[i]`, and `speeds[6]` above `temps[5]`. The table is
/// only consulted in [`FanMode::Advanced`]. Not exposed by msi-ec directly:
/// it lives at EC `0x6A..0x6F` (temps) and `0x72..0x78` (speeds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanCurve {
    pub temps: [u8; 6],
    pub speeds: [u8; 7],
}

impl FanCurve {
    /// What this board's EC ships with, from the MS-15HK analysis.
    pub const FACTORY: Self = Self {
        temps: [54, 58, 62, 66, 77, 93],
        speeds: [0, 50, 60, 65, 75, 75, 100],
    };

    /// `Some(percent)` when every point holds the same speed, i.e. the table
    /// was replaced by a manual override.
    pub fn flat_percent(&self) -> Option<u8> {
        let first = self.speeds[0];
        self.speeds.iter().all(|s| *s == first).then_some(first)
    }

    /// Comma-separated speed points, the helper's `curve` argument.
    pub fn speeds_csv(&self) -> String {
        self.speeds
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
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
    /// Current EC fan-curve table, when the EC image is readable.
    pub fan_curve: Option<FanCurve>,
    /// `true` when a manual speed can actually be written (helper + EC path).
    pub manual_fan_available: bool,
    pub battery_percent: Option<u8>,
    pub battery_limit: Option<u8>,
    pub ac_online: bool,
    pub keyboard_brightness: Option<u8>,
    pub keyboard_brightness_max: Option<u8>,
    pub webcam_blocked: Option<bool>,
    /// `true` when the Win key sits left of Fn, i.e. the two are swapped.
    pub win_key_left: Option<bool>,
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

    #[test]
    fn a_flat_curve_is_a_manual_override() {
        let manual = FanCurve {
            speeds: [60; 7],
            ..FanCurve::FACTORY
        };
        assert_eq!(manual.flat_percent(), Some(60));
        assert_eq!(FanCurve::FACTORY.flat_percent(), None);
    }

    #[test]
    fn speeds_serialise_for_the_helper() {
        assert_eq!(
            FanCurve::FACTORY.speeds_csv(),
            "0,50,60,65,75,75,100"
        );
    }

    #[test]
    fn factory_curve_matches_the_ec_dump() {
        // Captured from 0x6A..0x6F / 0x72..0x78 on 15HKEMS1.104.
        assert_eq!(FanCurve::FACTORY.temps, [54, 58, 62, 66, 77, 93]);
        assert_eq!(FanCurve::FACTORY.speeds, [0, 50, 60, 65, 75, 75, 100]);
    }
}
