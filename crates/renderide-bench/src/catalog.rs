//! Known CPU micro-bench ids and framebench scenarios for `list` and acceptance checks.
//!
//! The CPU list must match `benches/cpu_hot.rs` and `benches/transparent_sort.rs`. A unit test
//! below counts the entries; when a bench is added, update this table in the same PR.

/// One criterion bench: criterion writes `target/criterion/<group>/<id>/new/estimates.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuBenchId {
    /// Criterion group name (`benchmark_group`).
    pub group: &'static str,
    /// Criterion function id (`bench_function`).
    pub id: &'static str,
}

impl CpuBenchId {
    /// Path fragment `group/id` used in reports and criterion directories.
    #[must_use]
    pub fn path(&self) -> String {
        format!("{}/{}", self.group, self.id)
    }
}

/// Every `cpu_hot` and `transparent_sort` id the front door expects after a full CPU run.
pub const CPU_BENCH_IDS: &[CpuBenchId] = &[
    CpuBenchId {
        group: "transforms",
        id: "trs_and_degenerate_scale",
    },
    CpuBenchId {
        group: "culling",
        id: "homogeneous_clip_aabb",
    },
    CpuBenchId {
        group: "culling",
        id: "skinned_mesh_world_geometry",
    },
    CpuBenchId {
        group: "hysteresis_classification",
        id: "decide_visibility",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "8192_quarter_occluded",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "8192_after_motion_all_visible",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "10000_stable",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "10000_moving",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "50000_stable",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "50000_moving",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "100000_stable",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "100000_moving",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "8192_all_visible",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "10000_all_visible_over_cap",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "50000_all_visible_over_cap",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "100000_all_visible_over_cap",
    },
    CpuBenchId {
        group: "hysteresis_history",
        id: "676_of_1600_superset",
    },
    CpuBenchId {
        group: "world_matrices",
        id: "forest_recompute",
    },
    CpuBenchId {
        group: "world_matrices",
        id: "reverse_chain_recompute",
    },
    CpuBenchId {
        group: "world_matrices",
        id: "motion_history_publish",
    },
    CpuBenchId {
        group: "world_matrices",
        id: "motion_history_publish_100k_sparse",
    },
    CpuBenchId {
        group: "world_matrices",
        id: "motion_history_publish_100k_dense",
    },
    CpuBenchId {
        group: "material_uniforms",
        id: "property_change_pack",
    },
    CpuBenchId {
        group: "material_uniforms",
        id: "one_material_repack",
    },
    CpuBenchId {
        group: "bind_groups",
        id: "cache_hit",
    },
    CpuBenchId {
        group: "bind_groups",
        id: "miss_rebuild",
    },
    CpuBenchId {
        group: "keyword_variants",
        id: "cache_hit",
    },
    CpuBenchId {
        group: "keyword_variants",
        id: "cache_miss",
    },
    CpuBenchId {
        group: "keyword_variants",
        id: "compose",
    },
    CpuBenchId {
        group: "custom_stem_lookup",
        id: "frame_reads",
    },
    CpuBenchId {
        group: "shadow_churn",
        id: "static_repaint",
    },
    CpuBenchId {
        group: "shadow_churn",
        id: "cold_plan",
    },
    CpuBenchId {
        group: "shadow_churn",
        id: "static_repaint_revisions",
    },
    CpuBenchId {
        group: "render_texture_double_buffer",
        id: "steady_swap",
    },
    CpuBenchId {
        group: "camera_clear",
        id: "load_op_checksum",
    },
    CpuBenchId {
        group: "buffer_pool",
        id: "free_list_reuse",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "master_1k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "recompute_1k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "precomputed_1k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "snap_1k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "master_10k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "recompute_10k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "precomputed_10k",
    },
    CpuBenchId {
        group: "transparent_sort",
        id: "snap_10k",
    },
];

/// Framebench scenarios the front door lists (defaults plus opt-in extras).
pub const FRAME_SCENARIOS: &[&str] = &[
    "unlit_sphere",
    "multi_primitive_unlit",
    "pbs_lit_material_matrix",
    "many_static_meshes",
    "lights_shadows",
    "many_shadowed_point_lights",
    "offscreen_skinned_avatars",
    "onscreen_skinned_avatars",
    "secondary_camera_copy",
    "secondary_camera_double_buffer",
];

#[cfg(test)]
mod tests {
    use super::{CPU_BENCH_IDS, FRAME_SCENARIOS};
    use std::collections::HashSet;

    #[test]
    fn cpu_catalog_has_unique_paths_and_expected_count() {
        let mut seen = HashSet::new();
        for entry in CPU_BENCH_IDS {
            assert!(seen.insert(entry.path()), "duplicate {}", entry.path());
        }
        // 36 cpu_hot + 8 transparent_sort.
        assert_eq!(CPU_BENCH_IDS.len(), 44);
        assert!(CPU_BENCH_IDS.iter().any(|e| e.group == "transparent_sort"));
        assert!(CPU_BENCH_IDS.iter().any(|e| e.group == "culling"));
    }

    #[test]
    fn frame_catalog_has_unique_names() {
        let mut seen = HashSet::new();
        for name in FRAME_SCENARIOS {
            assert!(seen.insert(*name), "duplicate {name}");
            assert!(!name.is_empty());
        }
        assert_eq!(FRAME_SCENARIOS.len(), 10);
        assert!(FRAME_SCENARIOS.contains(&"unlit_sphere"));
        assert!(FRAME_SCENARIOS.contains(&"many_shadowed_point_lights"));
    }
}
