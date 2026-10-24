//! CPU microbenchmarks for Renderide helpers that do not need Resonite or a GPU.
//!
//! ```text
//! cargo bench -p renderide-bench
//! ```
//!
//! GPU-device benches, when they exist, compile only with `--features gpu` and are not part of
//! the default command.

pub use renderide::bench_api::{
    BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload,
    CAMERA_CLEAR_LOAD_OP_CAMERAS, CULL_BATCH, CullWorkload, HYSTERESIS_BATCH,
    HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload, HysteresisHistoryWorkload,
    KeywordVariantCacheWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
    SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload, TRANSFORM_BATCH,
    TRANSPARENT_SORT_1K, TRANSPARENT_SORT_10K, TransformWorkload, TransparentSortWorkload,
    WORLD_MATRIX_NODES, WorldMatrixWorkload, camera_clear_load_op_checksum,
    camera_clear_load_op_inputs,
};

#[cfg(test)]
mod tests {
    use super::{
        BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload,
        CAMERA_CLEAR_LOAD_OP_CAMERAS, CULL_BATCH, CullWorkload, HYSTERESIS_BATCH,
        HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload, HysteresisHistoryWorkload,
        KeywordVariantCacheWorkload, MaterialUniformUpdateWorkload, TransformWorkload,
        TransparentSortWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
        camera_clear_load_op_checksum, camera_clear_load_op_inputs,
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

    #[test]
    fn camera_clear_load_op_checksum_is_stable() {
        let inputs = camera_clear_load_op_inputs(CAMERA_CLEAR_LOAD_OP_CAMERAS);
        let first = camera_clear_load_op_checksum(&inputs);
        let second = camera_clear_load_op_checksum(&inputs);
        assert_eq!(first, second);
        assert_ne!(first, 0);
        assert_ne!(
            camera_clear_load_op_checksum(&inputs[..128]),
            camera_clear_load_op_checksum(&inputs[128..256])
        );
    }

    #[test]
    fn precomputed_and_recomputed_transparent_sorts_agree() {
        let workload = TransparentSortWorkload::mixed(64);
        let precomputed = workload.clone().sort_precomputed();
        let recomputed = workload.sort_recomputing_depth();
        assert_ne!(precomputed, 0);
        assert_eq!(precomputed, recomputed);
    }

    #[test]
    fn snap_siblings_checksum_is_stable() {
        let workload = TransparentSortWorkload::jittered_canvas_planes(64);
        let first = workload.clone().snap_siblings();
        let second = workload.snap_siblings();
        assert_ne!(first, 0);
        assert_eq!(first, second);
    }

    #[test]
    fn master_farthest_corner_sort_can_disagree_with_the_plane_key() {
        let workload = TransparentSortWorkload::plane_versus_corner();
        let precomputed = workload.clone().sort_precomputed();
        let master = workload.sort_master();
        assert_ne!(precomputed, 0);
        assert_ne!(
            precomputed, master,
            "canvas plane keys and farthest-corner keys must be able to swap"
        );
    }
}
