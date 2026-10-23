//! CPU microbenchmarks for Renderide helpers that do not need Resonite or a GPU.
//!
//! ```text
//! cargo bench -p renderide-bench
//! ```
//!
//! GPU-device benches, when they exist, compile only with `--features gpu` and are not part of
//! the default command.

pub use renderide::bench_api::{
    BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload, CULL_BATCH,
    CullWorkload, HYSTERESIS_BATCH, HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload,
    HysteresisHistoryWorkload, KeywordVariantCacheWorkload, MATERIAL_UNIFORM_BATCH,
    MaterialUniformUpdateWorkload, SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload,
    TRANSFORM_BATCH, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
};

#[cfg(test)]
mod tests {
    use super::{
        BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload,
        CULL_BATCH, CullWorkload, HYSTERESIS_BATCH, HYSTERESIS_HISTORY_COUNTS,
        HysteresisCullWorkload, HysteresisHistoryWorkload, KeywordVariantCacheWorkload,
        MaterialUniformUpdateWorkload, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
    };

    #[test]
    fn transform_fold_depends_on_every_pose() {
        let folded = TransformWorkload::standard().fold_matrices();
        assert!(folded.is_finite());
        assert!(folded.abs() > 0.0);
    }

    #[test]
    fn cull_keeps_some_boxes_and_rejects_others() {
        let visible = CullWorkload::standard().visible_count();
        assert!(visible > 0);
        assert!(visible < u32::try_from(CULL_BATCH).unwrap_or(u32::MAX));
    }

    #[test]
    fn world_matrix_recompute_is_stable_for_the_first_child() {
        let mut workload = WorldMatrixWorkload::standard();
        let first = workload.recompute();
        let second = workload.recompute();
        assert!((first - 1.0).abs() < 1.0e-4);
        assert!((second - first).abs() < 1.0e-4);
    }

    #[test]
    fn world_matrix_reverse_chain_recompute_matches_depth() {
        let mut workload = WorldMatrixWorkload::reverse_chain(WORLD_MATRIX_NODES);
        let expected = (WORLD_MATRIX_NODES - 2) as f32;
        let first = workload.recompute();
        let second = workload.recompute();
        assert!((first - expected).abs() < 1.0e-3);
        assert!((second - first).abs() < 1.0e-4);
    }

    #[test]
    fn buffer_pool_reuse_returns_the_same_entries() {
        let mut workload = BufferPoolWorkload::standard();
        let first = workload.reuse_cycle();
        let second = workload.reuse_cycle();
        let reused_ids = (0..BUFFER_POOL_SLOTS).fold(0_usize, usize::wrapping_add);
        assert_eq!(first, reused_ids);
        assert_eq!(second, reused_ids);
    }

    #[test]
    fn material_uniform_repack_tracks_color_changes() {
        let mut workload = MaterialUniformUpdateWorkload::standard();
        let error = workload.setup_error().unwrap_or("");
        assert!(error.is_empty(), "{error}");
        let first = workload.pack_changed_properties();
        let second = workload.pack_changed_properties();
        assert!(first > 0);
        assert_ne!(first, second);
    }

    #[test]
    fn one_material_repack_folds_the_full_block() {
        let mut workload = MaterialUniformUpdateWorkload::standard();
        let error = workload.setup_error().unwrap_or("");
        assert!(error.is_empty(), "{error}");
        let first = workload.repack_one();
        let second = workload.repack_one();
        assert!(first > 0);
        assert_ne!(first, second);
    }

    #[test]
    fn bind_group_cache_hits_are_stable_and_rebuild_after_invalidate() {
        let workload = BindGroupChurnWorkload::standard();
        let error = workload.setup_error().unwrap_or("");
        assert!(error.is_empty(), "{error}");
        let hits = workload.cache_hits();
        let again = workload.cache_hits();
        assert!(hits >= u64::try_from(BIND_GROUP_BATCH).unwrap_or(0));
        assert_eq!(hits, again);
        let rebuilt = workload.miss_rebuild();
        assert!(rebuilt >= u64::try_from(BIND_GROUP_BATCH).unwrap_or(0));
        assert_eq!(workload.cache_hits(), hits);
    }

    #[test]
    fn keyword_variant_cache_hit_miss_and_compose() {
        let mut workload = KeywordVariantCacheWorkload::standard();
        let error = workload.setup_error().unwrap_or("");
        assert!(error.is_empty(), "{error}");
        let hit = workload.cache_hit();
        assert!(hit > 0);
        assert_eq!(workload.cache_hit(), hit);
        let miss = workload.cache_miss();
        let miss_again = workload.cache_miss();
        assert!(miss > 0);
        assert!(miss_again > 0);
        assert_ne!(miss, miss_again);
        assert_eq!(workload.cache_hit(), hit);
        let composed = workload.compose();
        assert!(composed > 0);
    }

    #[test]
    fn hysteresis_keeps_disoccluded_casters_and_culls_the_stable_quarter() {
        let kept = HysteresisCullWorkload::standard().kept_count();
        let batch = u32::try_from(HYSTERESIS_BATCH).unwrap_or(u32::MAX);
        assert_eq!(kept, batch / 4 * 3);
    }

    #[test]
    fn hysteresis_history_stable_culls_a_quarter_and_motion_keeps_draws() {
        let count = 1_000;
        let mut stable = HysteresisHistoryWorkload::new(count, false);
        let stable_kept = stable.step();
        assert_eq!(stable_kept, u32::try_from(count / 4 * 3).unwrap_or(0));
        let mut moving = HysteresisHistoryWorkload::new(count, true);
        let moving_kept = moving.step();
        assert_eq!(moving_kept, u32::try_from(count).unwrap_or(0));
        assert_eq!(HYSTERESIS_HISTORY_COUNTS[0], 8_192);
        assert!(HYSTERESIS_HISTORY_COUNTS.contains(&10_000));
        assert!(HYSTERESIS_HISTORY_COUNTS.contains(&100_000));
    }
}
