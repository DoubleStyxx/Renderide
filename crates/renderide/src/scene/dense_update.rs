//! Helpers for negative-terminated dense renderable update slabs.

/// Row count assigned to one dense fixup worker chunk.
const FIXUP_PARALLEL_CHUNK_ROWS: usize = 32;

/// Minimum slab length required before transform-removal fixups switch from a
/// serial loop to a rayon parallel iterator.
///
/// All scene per-renderable fixups (static/skinned meshes, layer assignments,
/// render-transform/material overrides) share this threshold so the choice
/// remains consistent and easy to retune in one place. Below this length the
/// thread-pool dispatch overhead outweighs the per-element work.
pub(crate) const FIXUP_PARALLEL_MIN: usize = FIXUP_PARALLEL_CHUNK_ROWS * 2;

/// Iterates non-negative entries until the host terminator.
pub(crate) fn non_negative_i32s(values: &[i32]) -> impl Iterator<Item = i32> + '_ {
    values.iter().copied().take_while(|&value| value >= 0)
}

/// Applies host dense-index removals with `swap_remove` semantics.
pub(crate) fn swap_remove_dense_indices<T>(rows: &mut Vec<T>, removals: &[i32]) {
    for raw in non_negative_i32s(removals) {
        let idx = raw as usize;
        if idx < rows.len() {
            rows.swap_remove(idx);
        }
    }
}

/// Applies host dense-index removals with `swap_remove` semantics.
/// Additionally performs an update operation on moved elements.
pub(crate) fn swap_remove_dense_indices_with_update<T>(
    rows: &mut Vec<T>,
    removals: &[i32],
    update: impl Fn(&mut T, i32),
) {
    for raw in non_negative_i32s(removals) {
        let idx = raw as usize;
        if idx < rows.len() {
            rows.swap_remove(idx);
            if let Some(moved) = rows.get_mut(idx) {
                update(moved, raw);
            }
        }
    }
}

/// Pushes one row for each non-negative host addition id.
/// Additionally provides the index at which the element was inserted
/// to the builder method.
pub(crate) fn push_dense_additions<T>(
    rows: &mut Vec<T>,
    additions: &[i32],
    mut build: impl FnMut(i32, i32) -> T,
) {
    for id in non_negative_i32s(additions) {
        rows.push(build(id, rows.len().try_into().unwrap_or(-1)));
    }
}

/// Removes transform ids invalidated by a dense transform removal.
pub(crate) fn retain_live_transform_ids(ids: &mut Vec<i32>) {
    ids.retain(|&id| id >= 0);
}

/// Calls `update_row` for every entry in `rows`, fanning out to the rayon pool when
/// `rows.len() >= FIXUP_PARALLEL_MIN` and falling back to a serial loop otherwise.
///
/// All scene transform-removal fixup sweeps share this dispatch policy: the per-row work is
/// usually a single index rewrite, so the rayon path is only worth its dispatch cost above
/// [`FIXUP_PARALLEL_MIN`].
pub(crate) fn for_each_row_with_par_dispatch<T, F>(rows: &mut [T], update_row: F)
where
    T: Send,
    F: Fn(&mut T) + Sync + Send,
{
    if rows.len() >= FIXUP_PARALLEL_MIN {
        use rayon::prelude::*;
        rows.par_iter_mut()
            .with_min_len(FIXUP_PARALLEL_CHUNK_ROWS)
            .for_each(update_row);
    } else {
        for row in rows.iter_mut() {
            update_row(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        for_each_row_with_par_dispatch, push_dense_additions, swap_remove_dense_indices,
        swap_remove_dense_indices_with_update,
    };

    #[test]
    fn removals_stop_at_negative_terminator() {
        let mut rows = vec![10, 20, 30];

        swap_remove_dense_indices(&mut rows, &[1, -1, 0]);

        assert_eq!(rows, vec![10, 30]);
    }

    #[test]
    fn removals_with_mutations() {
        let mut rows = vec![vec![10], vec![20], vec![30], vec![40]];

        swap_remove_dense_indices_with_update(&mut rows, &[1], |v: &mut Vec<i32>, index| {
            v.push(index);
        });

        assert_eq!(rows, vec![vec![10], vec![40, 1], vec![30]]);
    }

    #[test]
    fn additions_stop_at_negative_terminator() {
        let mut rows = vec![1];

        push_dense_additions(&mut rows, &[2, 3, -1, 4], |id, index| 10 * index + id);

        assert_eq!(rows, vec![1, 12, 23]);
    }

    #[test]
    fn dispatch_serial_path_visits_each_row() {
        let mut rows = vec![1, 2, 3];

        for_each_row_with_par_dispatch(&mut rows, |row| *row += 10);

        assert_eq!(rows, vec![11, 12, 13]);
    }

    #[test]
    fn dispatch_parallel_path_visits_each_row() {
        let mut rows: Vec<i32> = (0..(super::FIXUP_PARALLEL_MIN as i32)).collect();

        for_each_row_with_par_dispatch(&mut rows, |row| *row *= 2);

        let expected: Vec<i32> = (0..(super::FIXUP_PARALLEL_MIN as i32))
            .map(|n| n * 2)
            .collect();
        assert_eq!(rows, expected);
    }
}
