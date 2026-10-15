//! CPU microbenchmarks for Renderide helpers that do not need Resonite or a GPU.
//!
//! ```text
//! cargo bench -p renderide-bench
//! ```
//!
//! GPU-device benches, when they exist, compile only with `--features gpu` and are not part of
//! the default command.

pub use renderide::bench_api::{
    BUFFER_POOL_SLOTS, BufferPoolWorkload, CULL_BATCH, CullWorkload, TRANSFORM_BATCH,
    TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
};

#[cfg(test)]
mod tests {
    use super::{
        BUFFER_POOL_SLOTS, BufferPoolWorkload, CULL_BATCH, CullWorkload, TransformWorkload,
        WorldMatrixWorkload,
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
    fn buffer_pool_reuse_returns_the_same_entries() {
        let mut workload = BufferPoolWorkload::standard();
        let first = workload.reuse_cycle();
        let second = workload.reuse_cycle();
        let reused_ids = (0..BUFFER_POOL_SLOTS).fold(0_usize, usize::wrapping_add);
        assert_eq!(first, reused_ids);
        assert_eq!(second, reused_ids);
    }
}
