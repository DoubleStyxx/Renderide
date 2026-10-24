//! Bulk versus indexed copies of rigid object-motion history.
//!
//! A contiguous object-mode run that covers at least half the nodes is one `copy_from_slice`.
//! Shorter runs and lists with holes stay indexed so a sparse set does not touch the rest.
//!
//! Callers pass the sorted, deduped object-mode list. A run whose span equals its length is
//! then one contiguous range, so release builds accept it in constant time. Walking the index
//! list here is a bounds-checked pass that costs more than the matrix copy at 100k nodes.

use glam::Mat4;

/// Half the nodes. A contiguous object-mode run at least this large uses a bulk copy.
const DENSE_MOTION_NODE_FRACTION_DENOMINATOR: usize = 2;

/// `true` when `nodes` is large enough that a contiguous run should use the bulk copy.
///
/// Sparse lists return here. The check stays inline so a 64-of-100k publish does not call
/// into the dense-range walk.
#[inline]
pub(super) fn covers_at_least_half(nodes: &[usize], count: usize) -> bool {
    count > 0
        && nodes
            .len()
            .saturating_mul(DENSE_MOTION_NODE_FRACTION_DENOMINATOR)
            >= count
}

/// Returns `nodes` when it is one contiguous run covering at least half of `count`.
///
/// The indexed publish path stays for shorter runs and for lists with holes. `nodes` is the
/// sorted, deduped object-mode set, so a matching span is `first..last + 1` with no gaps.
#[inline]
pub(super) fn dense_motion_range(nodes: &[usize], count: usize) -> Option<std::ops::Range<usize>> {
    let len = nodes.len();
    if !covers_at_least_half(nodes, count) {
        return None;
    }
    let first = *nodes.first()?;
    let last = *nodes.last()?;
    if first >= count || last >= count || last < first || last - first + 1 != len {
        return None;
    }
    // Release trusts the sorted unique list. Debug still rejects a duplicate that preserves
    // the span, such as `[0, 0, 2]`, so that hole is not bulk-copied.
    #[cfg(debug_assertions)]
    if !is_strictly_contiguous(nodes) {
        return None;
    }
    Some(first..last + 1)
}

/// `nodes` is `first, first+1, ...` with no duplicates and no holes.
#[cfg(debug_assertions)]
fn is_strictly_contiguous(nodes: &[usize]) -> bool {
    let Some(mut expected) = nodes.first().copied() else {
        return false;
    };
    for node in nodes.iter().skip(1) {
        let Some(next) = expected.checked_add(1) else {
            return false;
        };
        if *node != next {
            return false;
        }
        expected = next;
    }
    true
}

/// Copies one dense object-mode run. A ready prefix is a single `copy_from_slice`.
#[inline]
pub(super) fn publish_dense(cache: &mut super::WorldTransformCache, range: std::ops::Range<usize>) {
    let start = range.start;
    let end = range.end;
    if start == 0 && end <= cache.motion_history_ready_prefix {
        let super::WorldTransformCache {
            world_matrices,
            previous_world_matrices,
            previous_valid,
            ..
        } = cache;
        previous_world_matrices[start..end].copy_from_slice(&world_matrices[start..end]);
        previous_valid[start..end].fill(true);
        return;
    }
    let super::WorldTransformCache {
        world_matrices,
        previous_world_matrices,
        previous_valid,
        motion_history_ready,
        ..
    } = cache;
    copy_ready_matrices(
        &mut previous_world_matrices[start..end],
        &mut previous_valid[start..end],
        &world_matrices[start..end],
        &motion_history_ready[start..end],
    );
}

/// Seals one dense run. Once `0..end` is ready, later seals of that prefix do no work.
#[inline]
pub(super) fn seal_dense(cache: &mut super::WorldTransformCache, range: std::ops::Range<usize>) {
    let start = range.start;
    let end = range.end;
    if start == 0 && end <= cache.motion_history_ready_prefix {
        return;
    }
    let all_computed = cache.computed[start..end].iter().all(|flag| *flag);
    if all_computed {
        cache.motion_history_ready[start..end].fill(true);
        if start <= cache.motion_history_ready_prefix {
            cache.motion_history_ready_prefix = end.max(cache.motion_history_ready_prefix);
        }
        return;
    }
    let super::WorldTransformCache {
        computed,
        motion_history_ready,
        ..
    } = cache;
    seal_ready_flags(&mut motion_history_ready[start..end], &computed[start..end]);
}

/// Copies a contiguous ready run. Every-ready runs are one `copy_from_slice` plus a fill.
#[inline]
pub(super) fn copy_ready_matrices(
    previous: &mut [Mat4],
    valid: &mut [bool],
    current: &[Mat4],
    ready: &[bool],
) {
    if ready.iter().all(|flag| *flag) {
        previous.copy_from_slice(current);
        valid.fill(true);
        return;
    }
    for (((slot, valid), current), ready) in previous
        .iter_mut()
        .zip(valid.iter_mut())
        .zip(current.iter())
        .zip(ready.iter())
    {
        if *ready {
            *slot = *current;
            *valid = true;
        } else {
            *valid = false;
        }
    }
}

/// Marks a contiguous run ready where the current solve computed that node.
#[inline]
pub(super) fn seal_ready_flags(ready: &mut [bool], computed: &[bool]) {
    if computed.iter().all(|flag| *flag) {
        ready.fill(true);
        return;
    }
    for (slot, computed) in ready.iter_mut().zip(computed.iter()) {
        if *computed {
            *slot = true;
        }
    }
}
