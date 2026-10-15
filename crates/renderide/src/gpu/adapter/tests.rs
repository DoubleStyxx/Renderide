//! Unit tests for adapter selection and MSAA clamping policy.

#[cfg(test)]
mod msaa_clamp_tests {
    use super::super::msaa_support::clamp_msaa_request_to_supported;

    #[test]
    fn clamp_off_stays_off() {
        assert_eq!(clamp_msaa_request_to_supported(0, &[2, 4, 8]), 1);
        assert_eq!(clamp_msaa_request_to_supported(1, &[2, 4, 8]), 1);
    }

    #[test]
    fn clamp_upgrades_when_two_missing() {
        // Same situation as Rgba8UnormSrgb on some Vulkan drivers: only 4+ is valid.
        assert_eq!(clamp_msaa_request_to_supported(2, &[4, 8]), 4);
        assert_eq!(clamp_msaa_request_to_supported(3, &[4, 8]), 4);
    }

    #[test]
    fn clamp_exact_tier_preserved() {
        assert_eq!(clamp_msaa_request_to_supported(4, &[2, 4, 8]), 4);
    }

    #[test]
    fn clamp_falls_back_to_max_when_above_all_tiers() {
        assert_eq!(clamp_msaa_request_to_supported(16, &[4, 8]), 8);
    }

    #[test]
    fn clamp_empty_supported_means_off() {
        assert_eq!(clamp_msaa_request_to_supported(4, &[]), 1);
    }

    #[test]
    fn clamp_empty_stereo_tiers_forces_off_even_for_valid_desktop_requests() {
        // Models the case where the device lacks MULTISAMPLE_ARRAY /
        // TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES: `msaa_supported_sample_counts_stereo` returns
        // an empty `Vec`, and any MSAA request must silently collapse to 1x for the stereo path.
        for r in [2u32, 3, 4, 8, 16] {
            assert_eq!(clamp_msaa_request_to_supported(r, &[]), 1);
        }
    }
}

#[cfg(test)]
mod msaa_stereo_feature_gate_tests {
    /// Mirrors the gate used inside `msaa_supported_sample_counts_stereo`. When either feature is
    /// missing the stereo supported list must be empty regardless of per-format sample counts, so
    /// [`super::super::msaa_support::clamp_msaa_request_to_supported`] can silently
    /// fall back to 1x via the empty-slice rule (see `clamp_empty_supported_means_off`).
    fn stereo_feature_gate_passes(features: wgpu::Features) -> bool {
        let required = wgpu::Features::MULTISAMPLE_ARRAY
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        features.contains(required)
    }

    #[test]
    fn gate_requires_both_features() {
        assert!(!stereo_feature_gate_passes(wgpu::Features::empty()));
        assert!(!stereo_feature_gate_passes(
            wgpu::Features::MULTISAMPLE_ARRAY
        ));
        assert!(!stereo_feature_gate_passes(
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        ));
    }

    #[test]
    fn gate_passes_when_both_present() {
        let feats = wgpu::Features::MULTISAMPLE_ARRAY
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        assert!(stereo_feature_gate_passes(feats));
    }

    #[test]
    fn gate_ignores_unrelated_features() {
        let feats = wgpu::Features::MULTIVIEW | wgpu::Features::FLOAT32_FILTERABLE;
        assert!(!stereo_feature_gate_passes(feats));
    }
}

#[cfg(test)]
mod power_preference_score_tests {
    use super::super::selection::power_preference_score;
    use wgpu::{DeviceType, PowerPreference};

    #[test]
    fn high_performance_prefers_discrete_over_integrated() {
        assert!(
            power_preference_score(DeviceType::DiscreteGpu, PowerPreference::HighPerformance)
                < power_preference_score(
                    DeviceType::IntegratedGpu,
                    PowerPreference::HighPerformance,
                )
        );
    }

    #[test]
    fn low_power_prefers_integrated_over_discrete() {
        assert!(
            power_preference_score(DeviceType::IntegratedGpu, PowerPreference::LowPower)
                < power_preference_score(DeviceType::DiscreteGpu, PowerPreference::LowPower)
        );
    }

    #[test]
    fn cpu_and_other_rank_below_real_gpus() {
        for pref in [PowerPreference::HighPerformance, PowerPreference::LowPower] {
            let cpu = power_preference_score(DeviceType::Cpu, pref);
            let other = power_preference_score(DeviceType::Other, pref);
            let discrete = power_preference_score(DeviceType::DiscreteGpu, pref);
            let integrated = power_preference_score(DeviceType::IntegratedGpu, pref);
            assert!(cpu > discrete && cpu > integrated, "Cpu rank too high");
            assert!(
                other > discrete && other > integrated,
                "Other rank too high"
            );
        }
    }

    #[test]
    fn virtual_gpu_ranks_below_real_gpus_but_above_cpu() {
        for pref in [PowerPreference::HighPerformance, PowerPreference::LowPower] {
            let virt = power_preference_score(DeviceType::VirtualGpu, pref);
            let cpu = power_preference_score(DeviceType::Cpu, pref);
            let discrete = power_preference_score(DeviceType::DiscreteGpu, pref);
            assert!(virt > discrete);
            assert!(virt < cpu);
        }
    }
}
