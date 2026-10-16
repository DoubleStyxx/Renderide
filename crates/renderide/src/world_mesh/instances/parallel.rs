//! Parallel instance-plan window planning and partial-plan merging.

use std::ops::Range;

use hashbrown::HashMap;
use rayon::prelude::*;

#[cfg(test)]
use crate::cpu_parallelism::current_reference_worker_count;
use crate::cpu_parallelism::{
    ParallelAdmission, admit_render_command_items, reference_worker_count,
};
use crate::materials::ShaderPermutation;
use crate::world_mesh::draw_prep::WorldMeshDrawItem;

use super::batch_window::{BatchWindow, next_batch_window};
use super::builder::InstancePlanBuilder;
use super::scratch::{MeshSubmeshKey, mesh_submesh_key};
use super::{
    DrawGroup, INSTANCE_PLAN_PARALLEL_CHUNKS_PER_TASK, INSTANCE_PLAN_PARALLEL_MAX_WINDOWS_PER_TASK,
    INSTANCE_PLAN_PARALLEL_MIN_SINGLE_WINDOW_DRAWS, INSTANCE_PLAN_PARALLEL_MIN_WINDOWS,
    INSTANCE_PLAN_PARALLEL_WINDOW_DRAW_CHUNK, InstancePlan, LocalGroupedWindowChunk,
    LocalGroupedWindowGroup, MergedGroupedWindowGroup, WorldMeshPhase,
    debug_assert_plan_group_order,
};

/// Returns whether instance planning should use its active Rayon pool.
#[cfg(test)]
pub(super) fn should_parallelize_instance_plan(draw_count: usize, window_count: usize) -> bool {
    should_parallelize_instance_plan_with_workers(
        draw_count,
        window_count,
        current_reference_worker_count(),
    )
}

/// Returns the admission decision for one large same-batch-key window.
pub(super) fn single_window_admission(draw_count: usize, worker_count: usize) -> ParallelAdmission {
    let admission = admit_render_command_items(draw_count, worker_count);
    if draw_count >= INSTANCE_PLAN_PARALLEL_MIN_SINGLE_WINDOW_DRAWS && admission.is_parallel() {
        admission
    } else {
        ParallelAdmission::Serial
    }
}

/// Returns whether instance planning has enough work and workers to split batch windows.
#[cfg(test)]
pub(super) fn should_parallelize_instance_plan_with_workers(
    draw_count: usize,
    window_count: usize,
    worker_count: usize,
) -> bool {
    instance_plan_admission(draw_count, window_count, worker_count).is_parallel()
}

/// Returns the admission decision for splitting batch windows across workers.
pub(super) fn instance_plan_admission(
    draw_count: usize,
    window_count: usize,
    worker_count: usize,
) -> ParallelAdmission {
    let draw_admission = admit_render_command_items(draw_count, worker_count);
    if window_count >= INSTANCE_PLAN_PARALLEL_MIN_WINDOWS
        && draw_admission.is_parallel()
        && parallel_window_chunk_count_with_workers(window_count, worker_count) >= 2
    {
        ParallelAdmission::Parallel {
            chunk_size: INSTANCE_PLAN_PARALLEL_CHUNKS_PER_TASK,
        }
    } else {
        ParallelAdmission::Serial
    }
}

/// Returns how many worker chunks `window_count` produces for a known worker count.
pub(super) fn parallel_window_chunk_count_with_workers(
    window_count: usize,
    worker_count: usize,
) -> usize {
    window_count.div_ceil(parallel_window_chunk_size_with_workers(
        window_count,
        worker_count,
    ))
}

/// Returns the batch-window chunk size to use with the active Rayon pool.
pub(super) fn parallel_window_chunk_size(window_count: usize) -> usize {
    parallel_window_chunk_size_with_workers(window_count, rayon::current_num_threads())
}

/// Returns an adaptive batch-window chunk size capped to avoid over-coalescing work.
pub(super) fn parallel_window_chunk_size_with_workers(
    window_count: usize,
    worker_count: usize,
) -> usize {
    let worker_count = reference_worker_count(worker_count);
    window_count
        .div_ceil(worker_count)
        .clamp(1, INSTANCE_PLAN_PARALLEL_MAX_WINDOWS_PER_TASK)
}

pub(super) fn build_plan_serial(
    draws: &[WorldMeshDrawItem],
    supports_base_instance: bool,
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    let mut builder = InstancePlanBuilder::with_capacity(draws.len(), shader_perm);
    let mut i = 0usize;
    while i < draws.len() {
        let window = next_batch_window(draws, i, supports_base_instance);
        i = window.range.end;
        builder.process_window(draws, window);
    }

    builder.finish()
}

pub(super) fn collect_batch_windows(
    draws: &[WorldMeshDrawItem],
    supports_base_instance: bool,
) -> Vec<BatchWindow> {
    let mut windows = Vec::new();
    let mut i = 0usize;
    while i < draws.len() {
        let window = next_batch_window(draws, i, supports_base_instance);
        i = window.range.end;
        windows.push(window);
    }
    windows
}

pub(super) fn build_plan_from_windows_serial(
    draws: &[WorldMeshDrawItem],
    windows: &[BatchWindow],
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    let mut builder = InstancePlanBuilder::with_capacity(draws.len(), shader_perm);
    for window in windows {
        builder.process_window(draws, window.clone());
    }
    builder.finish()
}

pub(super) fn build_plan_parallel(
    draws: &[WorldMeshDrawItem],
    windows: &[BatchWindow],
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    profiling::scope!("mesh::build_plan_parallel_windows");
    let partials: Vec<_> = {
        profiling::scope!("mesh::build_plan_parallel_windows::worker_chunks");
        let window_chunk_size = parallel_window_chunk_size(windows.len());
        windows
            .par_chunks(window_chunk_size)
            .with_min_len(INSTANCE_PLAN_PARALLEL_CHUNKS_PER_TASK)
            .map(|chunk| {
                profiling::scope!("mesh::build_plan_window_chunk_worker");
                let draw_count = chunk.iter().map(|window| window.range.len()).sum::<usize>();
                let mut builder = InstancePlanBuilder::with_capacity(draw_count, shader_perm);
                for window in chunk {
                    builder.process_window(draws, window.clone());
                }
                builder.finish()
            })
            .collect()
    };
    {
        profiling::scope!("mesh::build_plan_parallel_windows::merge");
        merge_partial_instance_plans(draws.len(), partials)
    }
}

/// Builds a plan for one large same-batch-key window by splitting the window itself.
pub(super) fn build_plan_from_large_window_parallel(
    draws: &[WorldMeshDrawItem],
    window: &BatchWindow,
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    profiling::scope!("mesh::build_plan_parallel_large_window");
    if window.singleton {
        build_singleton_window_parallel(draws, window, shader_perm)
    } else {
        build_grouped_window_parallel(draws, window, shader_perm)
    }
}

/// Builds singleton groups for a large window in ordered draw chunks.
pub(super) fn build_singleton_window_parallel(
    draws: &[WorldMeshDrawItem],
    window: &BatchWindow,
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    let ranges = window_draw_chunks(window.range.clone());
    let partials: Vec<_> = ranges
        .par_iter()
        .with_min_len(INSTANCE_PLAN_PARALLEL_CHUNKS_PER_TASK)
        .map(|range| {
            profiling::scope!("mesh::build_plan_singleton_window_chunk_worker");
            let mut builder = InstancePlanBuilder::with_capacity(range.len(), shader_perm);
            builder.process_window(
                draws,
                BatchWindow {
                    range: range.clone(),
                    phase: window.phase,
                    singleton: true,
                },
            );
            builder.finish()
        })
        .collect();
    merge_partial_instance_plans(draws.len(), partials)
}

/// Builds grouped draw batches for a large window by reducing worker-local mesh/submesh groups.
pub(super) fn build_grouped_window_parallel(
    draws: &[WorldMeshDrawItem],
    window: &BatchWindow,
    shader_perm: ShaderPermutation,
) -> InstancePlan {
    let ranges = window_draw_chunks(window.range.clone());
    let chunks = ranges
        .par_iter()
        .with_min_len(INSTANCE_PLAN_PARALLEL_CHUNKS_PER_TASK)
        .map(|range| {
            profiling::scope!("mesh::build_plan_grouped_window_chunk_worker");
            collect_grouped_window_chunk(draws, range.clone())
        })
        .collect::<Vec<_>>();
    let groups = merge_grouped_window_chunks(chunks);
    let mut builder = InstancePlanBuilder::with_capacity(window.range.len(), shader_perm);
    builder.emit_merged_grouped_window(draws, window.phase, groups);
    builder.finish()
}

/// Splits one batch-window draw range into deterministic draw chunks.
pub(super) fn window_draw_chunks(range: Range<usize>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let end = (start + INSTANCE_PLAN_PARALLEL_WINDOW_DRAW_CHUNK).min(range.end);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

/// Builds worker-local mesh/submesh groups for a draw range.
pub(super) fn collect_grouped_window_chunk(
    draws: &[WorldMeshDrawItem],
    range: Range<usize>,
) -> LocalGroupedWindowChunk {
    let mut group_index: HashMap<MeshSubmeshKey, usize> = HashMap::new();
    let mut groups: Vec<LocalGroupedWindowGroup> = Vec::new();
    for draw_idx in range {
        let key = mesh_submesh_key(&draws[draw_idx]);
        if let Some(&group_idx) = group_index.get(&key) {
            groups[group_idx].members.push(draw_idx);
        } else {
            let group_idx = groups.len();
            group_index.insert(key, group_idx);
            groups.push(LocalGroupedWindowGroup {
                key,
                representative_draw_idx: draw_idx,
                members: vec![draw_idx],
            });
        }
    }
    LocalGroupedWindowChunk { groups }
}

/// Merges worker-local groups in draw-chunk order, preserving serial first-seen group order.
pub(super) fn merge_grouped_window_chunks(
    chunks: Vec<LocalGroupedWindowChunk>,
) -> Vec<MergedGroupedWindowGroup> {
    let mut group_index: HashMap<MeshSubmeshKey, usize> = HashMap::new();
    let mut groups: Vec<MergedGroupedWindowGroup> = Vec::new();
    for chunk in chunks {
        for mut local in chunk.groups {
            if let Some(&group_idx) = group_index.get(&local.key) {
                groups[group_idx].members.append(&mut local.members);
            } else {
                let group_idx = groups.len();
                group_index.insert(local.key, group_idx);
                groups.push(MergedGroupedWindowGroup {
                    representative_draw_idx: local.representative_draw_idx,
                    members: local.members,
                });
            }
        }
    }
    groups
}

pub(super) fn merge_partial_instance_plans(
    draw_count: usize,
    mut partials: Vec<InstancePlan>,
) -> InstancePlan {
    let mut plan = InstancePlan::with_capacity(draw_count);

    for partial in &mut partials {
        let slab_offset = plan.slab_layout.len() as u32;
        for phase in WorldMeshPhase::ALL {
            append_groups_with_slab_offset(
                plan.phase_mut(phase),
                std::mem::take(partial.phase_mut(phase)),
                slab_offset,
            );
        }
        plan.slab_layout.append(&mut partial.slab_layout);
    }

    debug_assert_plan_group_order(&plan);
    plan
}

pub(super) fn append_groups_with_slab_offset(
    target: &mut Vec<DrawGroup>,
    groups: Vec<DrawGroup>,
    slab_offset: u32,
) {
    target.extend(groups.into_iter().map(|mut group| {
        group.instance_range =
            group.instance_range.start + slab_offset..group.instance_range.end + slab_offset;
        group
    }));
}
