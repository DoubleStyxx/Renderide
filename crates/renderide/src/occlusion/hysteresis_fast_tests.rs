#[cfg(test)]
mod fast_path_tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use glam::Vec3;
    use hashbrown::{HashMap, HashSet};
    use rayon::prelude::*;

    use super::*;

    struct DrawCall {
        caster: Option<CasterRef>,
        aabb_min: Vec3,
        aabb_max: Vec3,
        facts: OcclusionFacts,
    }

    struct Frame {
        present: Vec<PresentCaster>,
        calls: Vec<DrawCall>,
        parallel: bool,
    }

    struct Row {
        key: CasterKey,
        class: VisibilityClass,
        aabb_min: Vec3,
        aabb_max: Vec3,
        mesh_asset_id: i32,
    }

    struct Snap {
        draws: Vec<bool>,
        rows: Vec<Row>,
        armed: bool,
    }

    fn key_in(space: i32, index: u64, skinned: bool) -> CasterKey {
        CasterKey::new(
            RenderSpaceId(space),
            usize::try_from(index).unwrap_or(usize::MAX),
            index,
            i32::try_from(index).unwrap_or(i32::MAX),
            skinned,
        )
    }

    fn key(index: u64) -> CasterKey {
        key_in(1, index, false)
    }

    fn bounds(index: u64) -> (Vec3, Vec3) {
        let min = Vec3::new(index as f32 * 0.5, 0.0, 2.0);
        (min, min + Vec3::splat(0.4))
    }

    fn keep() -> OcclusionFacts {
        OcclusionFacts::keep()
    }

    fn covered() -> OcclusionFacts {
        OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [None, None],
        }
    }

    fn covered_in_hole() -> OcclusionFacts {
        OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [Some(hole()), None],
        }
    }

    fn hole() -> ScreenRect {
        ScreenRect::from_uv((0.2, 0.2), (0.4, 0.4)).expect("disocclusion hole")
    }

    fn caster(index: u64, mesh: i32) -> CasterRef {
        CasterRef {
            key: key(index),
            mesh_asset_id: mesh,
        }
    }

    fn present_of(entries: &[(u64, i32)]) -> Vec<PresentCaster> {
        entries
            .iter()
            .map(|(index, mesh)| {
                let (aabb_min, aabb_max) = bounds(*index);
                PresentCaster {
                    key: key(*index),
                    mesh_asset_id: *mesh,
                    aabb_min,
                    aabb_max,
                    space_id: RenderSpaceId(1),
                }
            })
            .collect()
    }

    fn call(index: u64, facts: OcclusionFacts) -> DrawCall {
        let (aabb_min, aabb_max) = bounds(index);
        DrawCall {
            caster: Some(caster(index, 1)),
            aabb_min,
            aabb_max,
            facts,
        }
    }

    fn visible_history(entries: &[(u64, i32)]) -> VisibilityHistory {
        let mut casters = HashMap::new();
        for (index, mesh) in entries {
            let (min, max) = bounds(*index);
            casters.insert(
                key(*index),
                CasterRecord::new(VisibilityClass::Visible, min, max, *mesh),
            );
        }
        VisibilityHistory::from_casters(casters)
    }

    fn project(_min: Vec3, _max: Vec3) -> Option<ScreenRect> {
        Some(hole())
    }

    fn apply(
        history: VisibilityHistory,
        spare: HashMap<CasterKey, CasterRecord>,
        mut buffers: VisibilityCullBuffers,
        frame: &Frame,
        enable_fast: bool,
    ) -> (
        Snap,
        VisibilityHistory,
        HashMap<CasterKey, CasterRecord>,
        VisibilityCullBuffers,
    ) {
        let layout = if enable_fast {
            prepare_fast_visible(&history, &frame.present, &mut buffers)
        } else {
            None
        };
        let mut present_keys = HashSet::new();
        buffers.stale_rects.clear();
        if layout.is_none() {
            let mut movers = Vec::new();
            plan_disocclusion(
                &history,
                &frame.present,
                &mut present_keys,
                |_| true,
                &mut movers,
            );
            append_disocclusion_rects(&mut buffers.stale_rects, &movers, project);
        }
        let mut plan = VisibilityHysteresis::from_parts(history, spare, buffers);
        let armed = layout.is_some();
        plan.install_fast(layout);
        let draws = if frame.parallel {
            let flags: Vec<AtomicBool> = (0..frame.calls.len())
                .map(|_| AtomicBool::new(false))
                .collect();
            frame
                .calls
                .par_iter()
                .enumerate()
                .for_each(|(index, call)| {
                    flags[index].store(
                        plan.allows_draw(call.caster, call.aabb_min, call.aabb_max, call.facts),
                        Ordering::Relaxed,
                    );
                });
            flags
                .iter()
                .map(|flag| flag.load(Ordering::Relaxed))
                .collect()
        } else {
            frame
                .calls
                .iter()
                .map(|call| plan.allows_draw(call.caster, call.aabb_min, call.aabb_max, call.facts))
                .collect()
        };
        let recycled = plan.recycle();
        let mut rows: Vec<Row> = recycled
            .history
            .iter()
            .map(|(key, record)| Row {
                key: *key,
                class: record.class,
                aabb_min: record.aabb_min,
                aabb_max: record.aabb_max,
                mesh_asset_id: record.mesh_asset_id,
            })
            .collect();
        rows.sort_by_key(|row| {
            (
                row.key.space_id,
                row.key.renderable_index,
                row.key.instance_id,
                row.key.node_id,
                row.key.skinned,
            )
        });
        (
            Snap { draws, rows, armed },
            recycled.history,
            recycled.spare,
            recycled.buffers,
        )
    }

    fn run(initial: VisibilityHistory, frames: &[Frame], enable_fast: bool) -> Vec<Snap> {
        let mut history = initial;
        let mut spare = HashMap::new();
        let mut buffers = VisibilityCullBuffers::default();
        let mut snaps = Vec::new();
        for frame in frames {
            let (snap, next, next_spare, next_buffers) =
                apply(history, spare, buffers, frame, enable_fast);
            history = next;
            spare = next_spare;
            buffers = next_buffers;
            snaps.push(snap);
        }
        snaps
    }

    fn assert_same(fast: &[Snap], slow: &[Snap]) {
        assert_eq!(fast.len(), slow.len());
        for (index, (fast, slow)) in fast.iter().zip(slow).enumerate() {
            assert_eq!(fast.draws, slow.draws, "frame {index} draws");
            assert_eq!(fast.rows.len(), slow.rows.len(), "frame {index} row count");
            for (fast_row, slow_row) in fast.rows.iter().zip(&slow.rows) {
                assert_eq!(fast_row.key, slow_row.key, "frame {index} key");
                assert_eq!(fast_row.class, slow_row.class, "frame {index} class");
                assert_eq!(fast_row.aabb_min, slow_row.aabb_min, "frame {index} min");
                assert_eq!(fast_row.aabb_max, slow_row.aabb_max, "frame {index} max");
                assert_eq!(
                    fast_row.mesh_asset_id, slow_row.mesh_asset_id,
                    "frame {index} mesh"
                );
            }
        }
    }

    fn class_of(snap: &Snap, index: u64) -> Option<VisibilityClass> {
        snap.rows
            .iter()
            .find(|row| row.key == key(index))
            .map(|row| row.class)
    }

    #[test]
    fn fast_matches_slow_while_every_caster_stays_visible() {
        let entries = [(0, 1), (1, 1), (2, 1), (3, 1)];
        let frames = [
            Frame {
                present: present_of(&entries),
                calls: (0..4).map(|index| call(index, keep())).collect(),
                parallel: false,
            },
            Frame {
                present: present_of(&entries),
                calls: (0..4).map(|index| call(index, keep())).collect(),
                parallel: false,
            },
            Frame {
                present: present_of(&entries),
                calls: (0..4).map(|index| call(index, keep())).collect(),
                parallel: false,
            },
        ];
        let initial = visible_history(&entries);
        let fast = run(initial, &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast.iter().all(|snap| snap.armed));
        assert!(slow.iter().all(|snap| !snap.armed));
        assert_same(&fast, &slow);
        assert!(fast[2].draws.iter().all(|draw| *draw));
        assert!(
            fast[2]
                .rows
                .iter()
                .all(|row| row.class == VisibilityClass::Visible)
        );
    }

    #[test]
    fn fast_matches_slow_across_occlusion_cull_and_release() {
        let entries = [(0, 1), (1, 1), (2, 1)];
        let all = present_of(&entries);
        let without_two = present_of(&[(0, 1), (1, 1)]);
        let frames = [
            Frame {
                present: all.clone(),
                calls: vec![call(0, keep()), call(1, keep()), call(2, keep())],
                parallel: false,
            },
            Frame {
                present: all.clone(),
                calls: vec![call(0, covered()), call(1, keep()), call(2, keep())],
                parallel: false,
            },
            Frame {
                present: all.clone(),
                calls: vec![call(0, covered()), call(1, keep()), call(2, keep())],
                parallel: false,
            },
            Frame {
                present: without_two,
                calls: vec![call(0, covered_in_hole()), call(1, keep())],
                parallel: false,
            },
            Frame {
                present: all,
                calls: vec![call(0, keep()), call(1, keep()), call(2, covered())],
                parallel: false,
            },
        ];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast[0].armed && fast[1].armed);
        assert!(
            !fast[2].armed,
            "a remembered occlusion leaves the fast path"
        );
        assert_same(&fast, &slow);
        assert!(fast[1].draws[0]);
        assert_eq!(class_of(&fast[1], 0), Some(VisibilityClass::Occluded));
        assert!(
            !fast[2].draws[0],
            "confirmed occlusion culls on the next frame"
        );
        assert!(fast[3].draws[0], "disocclusion releases the cull");
        assert_eq!(class_of(&fast[3], 0), Some(VisibilityClass::Visible));
        assert!(
            class_of(&fast[3], 2).is_none(),
            "the absent caster is dropped"
        );
        assert!(fast[4].draws[2], "a returned caster is newly revealed");
        assert_eq!(class_of(&fast[4], 2), Some(VisibilityClass::Occluded));
    }

    #[test]
    fn fast_matches_slow_for_mixed_slots_and_a_moved_slot() {
        let entries = [(0, 1)];
        let (min, max) = bounds(0);
        let moved_min = min + Vec3::splat(0.25);
        let moved_max = max + Vec3::splat(0.25);
        let frames = [
            Frame {
                present: present_of(&entries),
                calls: vec![
                    call(0, covered()),
                    DrawCall {
                        caster: Some(caster(0, 1)),
                        aabb_min: min,
                        aabb_max: max,
                        facts: keep(),
                    },
                ],
                parallel: false,
            },
            Frame {
                present: present_of(&entries),
                calls: vec![
                    call(0, covered()),
                    DrawCall {
                        caster: Some(CasterRef {
                            key: key(0),
                            mesh_asset_id: 7,
                        }),
                        aabb_min: moved_min,
                        aabb_max: moved_max,
                        facts: keep(),
                    },
                ],
                parallel: false,
            },
        ];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast[0].armed);
        assert_same(&fast, &slow);
        assert_eq!(class_of(&fast[0], 0), Some(VisibilityClass::Visible));
        assert_eq!(fast[0].rows[0].aabb_min, min);
        assert_eq!(class_of(&fast[1], 0), Some(VisibilityClass::Visible));
        assert_eq!(fast[1].rows[0].mesh_asset_id, 7);
        assert_eq!(fast[1].rows[0].aabb_min, min.min(moved_min));
        assert_eq!(fast[1].rows[0].aabb_max, max.max(moved_max));
    }

    #[test]
    fn fast_matches_slow_when_a_prepared_caster_misses_the_frustum() {
        let entries = [(0, 1), (1, 1), (2, 1)];
        let frames = [
            Frame {
                present: present_of(&entries),
                calls: vec![call(0, keep()), call(1, keep())],
                parallel: false,
            },
            Frame {
                present: present_of(&entries),
                calls: vec![call(0, keep()), call(1, keep()), call(2, covered())],
                parallel: false,
            },
        ];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast[0].armed);
        assert!(!fast[1].armed, "the dropped row is new on the next frame");
        assert_same(&fast, &slow);
        assert!(class_of(&fast[0], 2).is_none());
        assert!(fast[1].draws[2]);
        assert_eq!(class_of(&fast[1], 2), Some(VisibilityClass::Occluded));
    }

    #[test]
    fn fast_matches_slow_when_views_change_count() {
        let main_entries = [(0, 1), (1, 1), (2, 1)];
        let main_frames = [
            Frame {
                present: present_of(&main_entries),
                calls: (0..3).map(|index| call(index, keep())).collect(),
                parallel: false,
            },
            Frame {
                present: present_of(&[(0, 1)]),
                calls: vec![call(0, keep())],
                parallel: false,
            },
        ];
        let secondary_frames = [Frame {
            present: vec![PresentCaster {
                key: key_in(4, 0, true),
                mesh_asset_id: 3,
                aabb_min: bounds(0).0,
                aabb_max: bounds(0).1,
                space_id: RenderSpaceId(4),
            }],
            calls: vec![DrawCall {
                caster: Some(CasterRef {
                    key: key_in(4, 0, true),
                    mesh_asset_id: 3,
                }),
                aabb_min: bounds(0).0,
                aabb_max: bounds(0).1,
                facts: keep(),
            }],
            parallel: false,
        }];
        let secondary_history = {
            let (min, max) = bounds(0);
            let mut casters = HashMap::new();
            casters.insert(
                key_in(4, 0, true),
                CasterRecord::new(VisibilityClass::Visible, min, max, 3),
            );
            VisibilityHistory::from_casters(casters)
        };
        let fast_main = run(visible_history(&main_entries), &main_frames, true);
        let slow_main = run(visible_history(&main_entries), &main_frames, false);
        let fast_secondary = run(secondary_history, &secondary_frames, true);
        let slow_secondary = run(
            {
                let (min, max) = bounds(0);
                let mut casters = HashMap::new();
                casters.insert(
                    key_in(4, 0, true),
                    CasterRecord::new(VisibilityClass::Visible, min, max, 3),
                );
                VisibilityHistory::from_casters(casters)
            },
            &secondary_frames,
            false,
        );
        assert!(fast_main[0].armed);
        assert!(
            !fast_main[1].armed,
            "shrinking the view punches disocclusion"
        );
        assert!(fast_secondary[0].armed);
        assert_same(&fast_main, &slow_main);
        assert_same(&fast_secondary, &slow_secondary);
        assert_eq!(fast_main[1].rows.len(), 1);
        assert_eq!(fast_secondary[0].rows.len(), 1);
        assert!(fast_secondary[0].rows[0].key.skinned);
    }

    #[test]
    fn fast_matches_slow_for_parallel_calls() {
        let entries: Vec<(u64, i32)> = (0..32).map(|index| (index, 1)).collect();
        let mut calls: Vec<DrawCall> = (0..32).map(|index| call(index, keep())).collect();
        calls[7] = call(7, covered());
        calls.push(call(3, keep()));
        let frames = [Frame {
            present: present_of(&entries),
            calls,
            parallel: true,
        }];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast[0].armed);
        assert_same(&fast, &slow);
        assert!(fast[0].draws.iter().all(|draw| *draw));
        assert_eq!(class_of(&fast[0], 7), Some(VisibilityClass::Occluded));
        assert_eq!(class_of(&fast[0], 3), Some(VisibilityClass::Visible));
    }

    #[test]
    fn fast_matches_slow_when_parallel_duplicates_hide_a_miss() {
        let entries: Vec<(u64, i32)> = (0..32).map(|index| (index, 1)).collect();
        let mut calls: Vec<DrawCall> = (0..31).map(|index| call(index, keep())).collect();
        calls.push(call(0, keep()));
        let frames = [Frame {
            present: present_of(&entries),
            calls,
            parallel: true,
        }];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(fast[0].armed);
        assert_same(&fast, &slow);
        assert!(
            class_of(&fast[0], 31).is_none(),
            "the untested caster is dropped"
        );
        assert_eq!(fast[0].rows.len(), 31);
    }

    #[test]
    fn fast_matches_slow_when_indices_are_not_dense() {
        let entries = [(0, 1), (2, 1)];
        let frames = [Frame {
            present: present_of(&entries),
            calls: vec![call(0, keep()), call(2, keep())],
            parallel: false,
        }];
        let fast = run(visible_history(&entries), &frames, true);
        let slow = run(visible_history(&entries), &frames, false);
        assert!(
            !fast[0].armed,
            "a hole in the renderer index stays on the full update"
        );
        assert_same(&fast, &slow);
        assert_eq!(fast[0].rows.len(), 2);
    }

    #[test]
    fn history_map_capacity_survives_a_fast_frame() {
        let entries = [(0, 1), (1, 1), (2, 1)];
        let frame = Frame {
            present: present_of(&entries),
            calls: (0..3).map(|index| call(index, keep())).collect(),
            parallel: false,
        };
        let (first, history, spare, buffers) = apply(
            visible_history(&entries),
            HashMap::new(),
            VisibilityCullBuffers::default(),
            &frame,
            true,
        );
        assert!(first.armed);
        let pointer = history
            .casters
            .keys()
            .next()
            .map(std::ptr::from_ref)
            .expect("history row");
        let capacity = history.casters.capacity();
        assert!(capacity >= 3);
        let (second, history, _, _) = apply(history, spare, buffers, &frame, true);
        assert!(second.armed);
        let second_pointer = history
            .casters
            .keys()
            .next()
            .map(std::ptr::from_ref)
            .expect("history row");
        assert_eq!(second_pointer, pointer);
        assert_eq!(history.casters.capacity(), capacity);
    }

    #[test]
    fn previously_occluded_absence_is_not_a_disoccluder() {
        let (min, max) = bounds(0);
        let mut casters = HashMap::new();
        casters.insert(
            key(0),
            CasterRecord::new(VisibilityClass::Occluded, min, max, 1),
        );
        let (visible_min, visible_max) = bounds(1);
        casters.insert(
            key(1),
            CasterRecord::new(VisibilityClass::Visible, visible_min, visible_max, 1),
        );
        let initial = VisibilityHistory::from_casters(casters);
        let frames = [Frame {
            present: present_of(&[(1, 1)]),
            calls: vec![call(1, covered())],
            parallel: false,
        }];
        let fast = run(initial, &frames, true);
        let slow = run(
            {
                let mut casters = HashMap::new();
                casters.insert(
                    key(0),
                    CasterRecord::new(VisibilityClass::Occluded, min, max, 1),
                );
                casters.insert(
                    key(1),
                    CasterRecord::new(VisibilityClass::Visible, visible_min, visible_max, 1),
                );
                VisibilityHistory::from_casters(casters)
            },
            &frames,
            false,
        );
        assert!(!fast[0].armed);
        assert_same(&fast, &slow);
        assert_eq!(class_of(&fast[0], 1), Some(VisibilityClass::Occluded));
        assert!(class_of(&fast[0], 0).is_none());
    }
}
