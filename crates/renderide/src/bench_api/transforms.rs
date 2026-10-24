use glam::{Quat, Vec3};

use crate::scene::{
    WorldTransformCache, compute_world_matrices_for_space, render_matrix_has_degenerate_scale,
    render_transform_has_degenerate_scale, render_transform_to_matrix,
};
use crate::shared::RenderTransform;

/// Local TRS rows folded by one [`TransformWorkload::fold_matrices`] sample.
pub const TRANSFORM_BATCH: usize = 4_096;

/// Hierarchy nodes solved by one [`WorldMatrixWorkload::recompute`] sample.
pub const WORLD_MATRIX_NODES: usize = 4_096;

/// Sibling group width for [`WorldMatrixWorkload`]. Index 0 of each group is a root.
const WORLD_MATRIX_GROUP: usize = 32;

/// Local poses for the TRS-to-matrix and degenerate-scale helpers.
#[derive(Clone, Debug)]
pub struct TransformWorkload {
    rows: Vec<RenderTransform>,
}

impl TransformWorkload {
    /// Builds [`TRANSFORM_BATCH`] varied poses.
    #[must_use]
    pub fn standard() -> Self {
        Self::varied(TRANSFORM_BATCH)
    }

    /// Builds `count` poses with distinct translation and yaw.
    #[must_use]
    pub fn varied(count: usize) -> Self {
        let rows = (0..count)
            .map(|index| {
                let step = u16::try_from(index % 3_600).unwrap_or(0);
                RenderTransform {
                    position: Vec3::new(f32::from(step) * 0.01, 0.25, -1.5),
                    scale: Vec3::new(1.0, if index.is_multiple_of(17) { 0.0 } else { 1.0 }, 1.0),
                    rotation: Quat::from_rotation_y(f32::from(step) * 0.001),
                }
            })
            .collect();
        Self { rows }
    }

    /// Folds TRS matrix conversion and the degenerate-scale checks into one scalar.
    #[must_use]
    pub fn fold_matrices(&self) -> f32 {
        let mut acc = 0.0_f32;
        for row in &self.rows {
            let matrix = render_transform_to_matrix(row);
            let degenerate = u8::from(render_transform_has_degenerate_scale(row))
                + u8::from(render_matrix_has_degenerate_scale(matrix));
            acc += matrix.w_axis.x + matrix.x_axis.x + f32::from(degenerate);
        }
        acc
    }
}

/// Parent-chain world-matrix solve over a reused world-transform cache.
#[derive(Debug)]
pub struct WorldMatrixWorkload {
    nodes: Vec<RenderTransform>,
    parents: Vec<i32>,
    cache: WorldTransformCache,
}

impl WorldMatrixWorkload {
    /// Builds [`WORLD_MATRIX_NODES`] nodes grouped into roots plus direct children.
    #[must_use]
    pub fn standard() -> Self {
        Self::forest(WORLD_MATRIX_NODES)
    }

    /// Builds `count` nodes. Every [`WORLD_MATRIX_GROUP`]th node is a root.
    #[must_use]
    pub fn forest(count: usize) -> Self {
        let mut nodes = Vec::with_capacity(count);
        let mut parents = Vec::with_capacity(count);
        for index in 0..count {
            let grouped = index % WORLD_MATRIX_GROUP == 0;
            let parent = if grouped {
                -1
            } else {
                i32::try_from(index - (index % WORLD_MATRIX_GROUP)).unwrap_or(-1)
            };
            let group = index / WORLD_MATRIX_GROUP;
            let x = if grouped {
                f32::from(u16::try_from(group).unwrap_or(0))
            } else {
                1.0
            };
            nodes.push(RenderTransform {
                position: Vec3::new(x, 0.0, 0.0),
                scale: Vec3::ONE,
                rotation: Quat::IDENTITY,
            });
            parents.push(parent);
        }
        Self {
            nodes,
            parents,
            cache: WorldTransformCache::default(),
        }
    }

    /// Builds one parent chain stored child-before-parent.
    ///
    /// Node `i` parents `i + 1`, and the last node is the root. Bulk rebuild classifies this
    /// shape on every full recompute; the chain is long enough that a per-depth sweep is quadratic.
    #[must_use]
    pub fn reverse_chain(count: usize) -> Self {
        let mut nodes = Vec::with_capacity(count);
        let mut parents = Vec::with_capacity(count);
        for index in 0..count {
            let is_root = index + 1 == count;
            nodes.push(RenderTransform {
                position: Vec3::new(if is_root { 0.0 } else { 1.0 }, 0.0, 0.0),
                scale: Vec3::ONE,
                rotation: Quat::IDENTITY,
            });
            let parent = if is_root {
                -1
            } else {
                i32::try_from(index + 1).unwrap_or(-1)
            };
            parents.push(parent);
        }
        Self {
            nodes,
            parents,
            cache: WorldTransformCache::default(),
        }
    }

    /// Invalidates the cache and solves world matrices.
    ///
    /// Returns the world-space X of node 1 (child of root 0), which is `1` for [`Self::forest`].
    /// For [`Self::reverse_chain`] that node sits `count - 2` steps below the root, so the value
    /// is `count - 2` when every step translates `+1` on X.
    #[must_use]
    pub fn recompute(&mut self) -> f32 {
        self.cache.computed.fill(false);
        if compute_world_matrices_for_space(0, &self.nodes, &self.parents, &mut self.cache).is_err()
        {
            return 0.0;
        }
        self.cache
            .world_matrices
            .get(1)
            .map(|matrix| matrix.w_axis.x)
            .unwrap_or(0.0)
    }

    /// Publishes rigid-motion history for `nodes` into reused previous-matrix storage.
    ///
    /// Returns that storage's capacity. After the first publish for a fixed node count, further
    /// publishes must return the same capacity. `nodes` is the object-mode subset; it is not
    /// allocated inside the method.
    #[must_use]
    pub fn publish_motion_history(&mut self, nodes: &[usize]) -> usize {
        self.cache.seal_motion_history(nodes);
        self.cache.publish_previous_world_matrices(nodes);
        self.cache.previous_world_matrix_capacity()
    }
}
