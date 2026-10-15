//! GPU adapter power-preference enum.

use crate::labeled_enum;

labeled_enum! {
    /// Preferred GPU power mode for future adapter selection (stored; changing at runtime may
    /// require re-initialization).
    pub enum PowerPreferenceSetting: "GPU power preference" {
        default => HighPerformance;

        /// Maps to [`wgpu::PowerPreference::LowPower`].
        LowPower => {
            persist: "low_power",
            label: "Low power",
            aliases: ["low"],
        },
        /// Maps to [`wgpu::PowerPreference::HighPerformance`].
        HighPerformance => {
            persist: "high_performance",
            label: "High performance",
            aliases: ["high", "performance"],
        },
    }
}

impl PowerPreferenceSetting {
    /// Stable string for TOML / UI (`low_power` / `high_performance`). Historical alias for
    /// [`Self::persist_str`].
    #[cfg(test)]
    pub fn as_persist_str(self) -> &'static str {
        self.persist_str()
    }

    /// Parses case-insensitive persisted or UI tokens. Historical alias for
    /// [`Self::parse_persist`].
    #[cfg(test)]
    pub fn from_persist_str(s: &str) -> Option<Self> {
        Self::parse_persist(s)
    }

    /// Maps the persisted setting to the corresponding [`wgpu::PowerPreference`] used by adapter
    /// selection.
    pub fn to_wgpu(self) -> wgpu::PowerPreference {
        match self {
            Self::LowPower => wgpu::PowerPreference::LowPower,
            Self::HighPerformance => wgpu::PowerPreference::HighPerformance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PowerPreferenceSetting;

    #[test]
    fn power_preference_from_persist_str() {
        assert_eq!(
            PowerPreferenceSetting::HighPerformance.as_persist_str(),
            "high_performance"
        );
        assert_eq!(
            PowerPreferenceSetting::from_persist_str("low_power"),
            Some(PowerPreferenceSetting::LowPower)
        );
        assert_eq!(
            PowerPreferenceSetting::from_persist_str("LOW"),
            Some(PowerPreferenceSetting::LowPower)
        );
        assert_eq!(
            PowerPreferenceSetting::from_persist_str("performance"),
            Some(PowerPreferenceSetting::HighPerformance)
        );
        assert_eq!(
            PowerPreferenceSetting::from_persist_str("high_performance"),
            Some(PowerPreferenceSetting::HighPerformance)
        );
        assert_eq!(PowerPreferenceSetting::from_persist_str(""), None);
    }
}
