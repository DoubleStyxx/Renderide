#[test]
fn fast_publish_near_epoch_wrap_drops_untested_and_replaces_bounds() {
    let entries = [(0, 1), (1, 1), (2, 1)];
    let mut initial = visible_history(&entries);
    initial.set_clock(u64::MAX, 1);
    for (_, record) in initial.iter() {
        record.set_seen_epoch(1);
    }
    let steady = Frame {
        present: present_of(&entries),
        calls: (0..3).map(|index| call(index, keep())).collect(),
        parallel: false,
    };
    let mut missed = steady.clone();
    missed.calls.pop();
    let frames = [steady, missed];
    let fast = run(clone_clock(&initial), &frames, true);
    let slow = run(initial, &frames, false);
    assert!(fast[0].published_fast);
    assert_eq!(fast[0].epoch, 2, "the wrap skips the stale stamp");
    assert!(!fast[1].published_fast);
    assert_same(&fast, &slow);
    assert!(
        class_of(&fast[1], 2).is_none(),
        "the untested row is not kept"
    );

    let mut moved = visible_history(&[(0, 1)]);
    moved.set_clock(u64::MAX, 1);
    for (_, record) in moved.iter() {
        record.set_seen_epoch(1);
    }
    let (min, max) = bounds(0);
    let first_min = min + Vec3::splat(0.25);
    let first_max = max + Vec3::splat(0.25);
    let second_min = min + Vec3::splat(0.5);
    let second_max = max + Vec3::splat(0.5);
    let moved_frame = [Frame {
        present: vec![PresentCaster {
            key: key(0),
            mesh_asset_id: 1,
            aabb_min: first_min,
            aabb_max: second_max,
            space_id: RenderSpaceId(1),
        }],
        calls: vec![
            DrawCall {
                caster: Some(caster(0, 1)),
                aabb_min: first_min,
                aabb_max: first_max,
                facts: keep(),
            },
            DrawCall {
                caster: Some(caster(0, 1)),
                aabb_min: second_min,
                aabb_max: second_max,
                facts: keep(),
            },
        ],
        parallel: false,
    }];
    let fast_moved = run(clone_clock(&moved), &moved_frame, true);
    let slow_moved = run(moved, &moved_frame, false);
    assert_same(&fast_moved, &slow_moved);
    assert_eq!(fast_moved[0].rows[0].aabb_min, first_min);
    assert_eq!(fast_moved[0].rows[0].aabb_max, second_max);
    assert_ne!(fast_moved[0].rows[0].aabb_min, min);
}

fn clone_clock(history: &VisibilityHistory) -> VisibilityHistory {
    let mut casters = HashMap::new();
    for (key, record) in history.iter() {
        let copy = CasterRecord::new(
            record.class,
            record.aabb_min,
            record.aabb_max,
            record.mesh_asset_id,
        );
        copy.set_seen_epoch(record.seen_epoch.load(Ordering::Relaxed));
        casters.insert(*key, copy);
    }
    let mut cloned = VisibilityHistory::from_casters(casters);
    cloned.set_clock(history.epoch, history.stamped_epoch);
    cloned
}

#[test]
fn fast_path_refuses_a_reused_slot_and_a_caster_that_changes_space() {
    let entries = [(0, 1)];
    let (min, max) = bounds(0);
    let reused = PresentCaster {
        key: CasterKey::new(RenderSpaceId(1), 0, 1_000, 0, false),
        mesh_asset_id: 1,
        aabb_min: min,
        aabb_max: max,
        space_id: RenderSpaceId(1),
    };
    let reused_call = DrawCall {
        caster: Some(CasterRef {
            key: reused.key,
            mesh_asset_id: 1,
        }),
        aabb_min: min,
        aabb_max: max,
        facts: keep(),
    };
    let frames = [Frame {
        present: vec![reused],
        calls: vec![reused_call],
        parallel: false,
    }];
    let fast = run(visible_history(&entries), &frames, true);
    let slow = run(visible_history(&entries), &frames, false);
    assert!(
        !fast[0].armed,
        "a new renderer at the same index is not the old row"
    );
    assert_same(&fast, &slow);
    assert!(class_of(&fast[0], 0).is_none());
    assert_eq!(fast[0].rows.len(), 1);
    assert_eq!(fast[0].rows[0].key.instance_id, 1_000);
    assert!(!fast[0].rects.is_empty());

    let mut moved = present_of(&entries);
    moved[0].key.space_id = 2;
    moved[0].space_id = RenderSpaceId(2);
    let moved_call = DrawCall {
        caster: Some(CasterRef {
            key: moved[0].key,
            mesh_asset_id: 1,
        }),
        aabb_min: min,
        aabb_max: max,
        facts: keep(),
    };
    let moved_frames = [Frame {
        present: moved,
        calls: vec![moved_call],
        parallel: false,
    }];
    let fast = run(visible_history(&entries), &moved_frames, true);
    let slow = run(visible_history(&entries), &moved_frames, false);
    assert!(!fast[0].armed);
    assert_same(&fast, &slow);
    assert_eq!(fast[0].rows.len(), 1);
    assert_eq!(fast[0].rows[0].key.space_id, 2);
    assert!(!fast[0].rects.is_empty());
}

#[test]
fn fast_path_refuses_mixed_space_and_mixed_skinned_histories() {
    for keys in [
        [key_in(1, 0, false), key_in(2, 1, false)],
        [key_in(1, 0, false), key_in(1, 1, true)],
    ] {
        let mut casters = HashMap::new();
        for key in keys {
            let (min, max) = bounds(key.renderable_index);
            casters.insert(
                key,
                CasterRecord::new(VisibilityClass::Visible, min, max, 1),
            );
        }
        let present = casters
            .keys()
            .map(|key| {
                let (min, max) = bounds(key.renderable_index);
                PresentCaster {
                    key: *key,
                    mesh_asset_id: 1,
                    aabb_min: min,
                    aabb_max: max,
                    space_id: RenderSpaceId(key.space_id),
                }
            })
            .collect();
        let calls = casters
            .keys()
            .map(|key| {
                let (min, max) = bounds(key.renderable_index);
                DrawCall {
                    caster: Some(CasterRef {
                        key: *key,
                        mesh_asset_id: 1,
                    }),
                    aabb_min: min,
                    aabb_max: max,
                    facts: keep(),
                }
            })
            .collect();
        let frames = [Frame {
            present,
            calls,
            parallel: false,
        }];
        let initial = VisibilityHistory::from_casters(casters);
        let fast = run(clone_clock(&initial), &frames, true);
        let slow = run(initial, &frames, false);
        assert!(!fast[0].armed);
        assert_same(&fast, &slow);
    }
}

#[test]
fn fast_table_storage_survives_a_full_update() {
    let entries = [(0, 1), (1, 1), (2, 1)];
    let steady = Frame {
        present: present_of(&entries),
        calls: (0..3).map(|index| call(index, keep())).collect(),
        parallel: false,
    };
    let mut occluded = steady.clone();
    occluded.calls[0] = call(0, covered());
    let (first, history, spare, buffers) = apply(
        visible_history(&entries),
        HashMap::new(),
        VisibilityCullBuffers::default(),
        &steady,
        true,
    );
    assert!(first.published_fast);
    let (pointer, capacity) = fast::row_storage(&buffers.fast_slots);
    assert!(capacity >= 3);
    let (second, history, spare, buffers) = apply(history, spare, buffers, &occluded, true);
    assert!(!second.published_fast);
    let (after_full, after_capacity) = fast::row_storage(&buffers.fast_slots);
    assert_eq!(after_full, pointer);
    assert_eq!(after_capacity, capacity);
    // The occluded row keeps the fast path off. One full frame releases it back to Visible.
    let mut released = steady.clone();
    released.calls[0] = call(0, keep());
    let (release, history, spare, buffers) = apply(history, spare, buffers, &released, true);
    assert!(
        !release.armed,
        "an occluded row still takes the full update"
    );
    let (after_release, release_capacity) = fast::row_storage(&buffers.fast_slots);
    assert_eq!(after_release, pointer);
    assert_eq!(release_capacity, capacity);
    let (third, _, _, buffers) = apply(history, spare, buffers, &steady, true);
    assert!(third.armed);
    assert!(third.published_fast);
    let (after_reuse, reused_capacity) = fast::row_storage(&buffers.fast_slots);
    assert_eq!(after_reuse, pointer);
    assert_eq!(reused_capacity, capacity);
}

#[test]
fn seeded_fast_path_matches_the_full_update() {
    const SEEDS: u64 = 64;
    const FRAMES: usize = 20;
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        let mut world = (0..rng.below(24).max(1))
            .map(|index| Body::at(index, &mut rng))
            .collect::<Vec<_>>();
        let mut fast_history = VisibilityHistory::default();
        let mut slow_history = VisibilityHistory::default();
        let mut fast_spare = HashMap::new();
        let mut slow_spare = HashMap::new();
        let mut fast_buffers = VisibilityCullBuffers::default();
        let mut slow_buffers = VisibilityCullBuffers::default();
        for _frame in 0..FRAMES {
            if rng.chance(2, 5) {
                for body in &mut world {
                    body.mutate(&mut rng);
                }
            }
            let (present, calls) = frame_of(&world, &mut rng);
            let frame = Frame {
                present,
                calls,
                parallel: false,
            };
            let (fast_snap, next_fast, next_fast_spare, next_fast_buffers) = apply(
                std::mem::take(&mut fast_history),
                std::mem::take(&mut fast_spare),
                std::mem::take(&mut fast_buffers),
                &frame,
                true,
            );
            let (slow_snap, next_slow, next_slow_spare, next_slow_buffers) = apply(
                std::mem::take(&mut slow_history),
                std::mem::take(&mut slow_spare),
                std::mem::take(&mut slow_buffers),
                &frame,
                false,
            );
            assert_same(
                std::slice::from_ref(&fast_snap),
                std::slice::from_ref(&slow_snap),
            );
            fast_history = next_fast;
            slow_history = next_slow;
            fast_spare = next_fast_spare;
            slow_spare = next_slow_spare;
            fast_buffers = next_fast_buffers;
            slow_buffers = next_slow_buffers;
        }
    }
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1))
    }

    fn u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.u32() % n
    }

    fn chance(&mut self, numer: u32, denom: u32) -> bool {
        self.below(denom) < numer
    }
}

struct Body {
    index: u64,
    space: i32,
    skinned: bool,
    instance: u64,
    node: i32,
    mesh: i32,
    min: Vec3,
    max: Vec3,
}

impl Body {
    fn at(index: u32, rng: &mut Rng) -> Self {
        let index = u64::from(index);
        let (min, max) = bounds(index);
        Self {
            index,
            space: if rng.chance(1, 4) { 2 } else { 1 },
            skinned: rng.chance(1, 10),
            instance: index,
            node: i32::try_from(index).unwrap_or(i32::MAX),
            mesh: 1,
            min,
            max,
        }
    }

    fn mutate(&mut self, rng: &mut Rng) {
        let roll = rng.below(100);
        if roll < 3 {
            self.min += Vec3::splat(0.25);
            self.max += Vec3::splat(0.25);
        } else if roll < 5 {
            self.min += Vec3::splat(5.0e-5);
            self.max += Vec3::splat(5.0e-5);
        } else if roll < 6 {
            self.mesh = self.mesh.saturating_add(1);
        } else if roll < 7 {
            self.instance = self.instance.saturating_add(1_000);
        } else if roll < 8 {
            self.space = 3 - self.space;
        }
    }

    fn key(&self) -> CasterKey {
        CasterKey::new(
            RenderSpaceId(self.space),
            usize::try_from(self.index).unwrap_or(usize::MAX),
            self.instance,
            self.node,
            self.skinned,
        )
    }
}

fn frame_of(world: &[Body], rng: &mut Rng) -> (Vec<PresentCaster>, Vec<DrawCall>) {
    let mut present = Vec::new();
    let mut calls = Vec::new();
    for body in world {
        if rng.chance(1, 20) {
            continue;
        }
        present.push(PresentCaster {
            key: body.key(),
            mesh_asset_id: body.mesh,
            aabb_min: body.min,
            aabb_max: body.max,
            space_id: RenderSpaceId(body.space),
        });
        if rng.chance(3, 20) {
            continue;
        }
        let occluded = rng.chance(1, 10);
        calls.push(DrawCall {
            caster: Some(CasterRef {
                key: body.key(),
                mesh_asset_id: body.mesh,
            }),
            aabb_min: body.min,
            aabb_max: body.max,
            facts: facts(occluded, rng),
        });
        if rng.chance(1, 10) {
            let shifted = rng.chance(3, 10);
            let min = if shifted {
                body.min - Vec3::splat(0.1)
            } else {
                body.min
            };
            calls.push(DrawCall {
                caster: Some(CasterRef {
                    key: body.key(),
                    mesh_asset_id: body.mesh,
                }),
                aabb_min: min,
                aabb_max: body.max,
                facts: facts(rng.chance(3, 10), rng),
            });
        }
    }
    if rng.chance(1, 20) {
        calls.push(DrawCall {
            caster: None,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ONE,
            facts: keep(),
        });
    }
    (present, calls)
}

fn facts(occluded: bool, rng: &mut Rng) -> OcclusionFacts {
    OcclusionFacts {
        occluded,
        other_occluder: occluded && rng.chance(7, 10),
        rects: [rng.chance(1, 2).then(hole), rng.chance(1, 5).then(hole)],
    }
}
