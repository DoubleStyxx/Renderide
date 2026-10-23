#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::Mat4;
    use rayon::prelude::*;

    use super::*;
    use crate::camera::reverse_z_perspective;

    fn record(class: VisibilityClass, min: Vec3, max: Vec3) -> CasterRecord {
        CasterRecord {
            class,
            aabb_min: min,
            aabb_max: max,
            mesh_asset_id: 1,
        }
    }

    fn aabb() -> (Vec3, Vec3) {
        (Vec3::new(-0.5, -0.5, 2.0), Vec3::new(0.5, 0.5, 3.0))
    }

    fn inputs<'a>(previous: Option<&'a CasterRecord>, occludes: bool) -> HistoryInputs<'a> {
        HistoryInputs {
            previous,
            caster_moved: false,
            overlaps_disocclusion: false,
            hi_z_occludes: occludes,
            other_occluder: false,
        }
    }

    fn caster_ref(key: CasterKey) -> CasterRef {
        CasterRef {
            key,
            mesh_asset_id: 1,
        }
    }

    fn perspective_clip(near: f32, far: f32) -> DepthClip {
        DepthClip {
            near,
            far,
            orthographic: false,
        }
    }

    fn ndc_from_view_depth(depth: f32, clip: DepthClip) -> f32 {
        (clip.far * clip.near / depth - clip.near) / (clip.far - clip.near)
    }

    #[test]
    fn newly_revealed_caster_is_drawn_even_when_stale_hiz_occludes() {
        let decision = decide_visibility(inputs(None, true));
        assert_eq!(decision.action, HistoryAction::DrawRemembered);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn moving_occluder_keeps_previously_occluded_caster() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.overlaps_disocclusion = true;
        let decision = decide_visibility(query);
        assert_eq!(decision.action, HistoryAction::Unconfirmed);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn camera_cut_keeps_previously_occluded_caster() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.hi_z_occludes = false;
        let decision = decide_visibility(query);
        assert_eq!(decision.action, HistoryAction::Unconfirmed);
        assert_ne!(decision.action, HistoryAction::Culled);
    }

    #[test]
    fn stable_occluded_caster_stays_culled() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let decision = decide_visibility(inputs(Some(&previous), true));
        assert_eq!(decision.action, HistoryAction::Culled);
        assert_eq!(decision.next_class, VisibilityClass::Occluded);
    }

    #[test]
    fn visible_caster_is_not_rejected_by_its_own_depth() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Visible, min, max);
        let decision = decide_visibility(inputs(Some(&previous), true));
        assert_eq!(decision.action, HistoryAction::DrawRemembered);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn moved_caster_behind_stale_hiz_is_drawn() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.caster_moved = true;
        assert_eq!(decide_visibility(query).action, HistoryAction::Unconfirmed);
    }

    #[test]
    fn separate_occluder_schedules_visible_caster_for_the_next_frame() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Visible, min, max);
        let mut query = inputs(Some(&previous), true);
        query.other_occluder = true;
        let decision = decide_visibility(query);
        assert_eq!(decision.action, HistoryAction::DrawRemembered);
        assert_eq!(decision.next_class, VisibilityClass::Occluded);
    }

    #[test]
    fn probe_disocclusion_never_culls_a_visible_caster() {
        let (min, max) = aabb();
        let occluded = record(VisibilityClass::Occluded, min, max);
        let visible = record(VisibilityClass::Visible, min, max);
        let cases = [
            ("newly revealed", inputs(None, true)),
            ("moving occluder", {
                let mut query = inputs(Some(&occluded), true);
                query.overlaps_disocclusion = true;
                query
            }),
            ("camera cut", {
                let mut query = inputs(Some(&occluded), true);
                query.hi_z_occludes = false;
                query
            }),
            ("self occlusion", inputs(Some(&visible), true)),
            ("caster slid out", {
                let mut query = inputs(Some(&occluded), true);
                query.caster_moved = true;
                query
            }),
        ];
        for (name, query) in cases {
            let decision = decide_visibility(query);
            assert_ne!(
                decision.action,
                HistoryAction::Culled,
                "{name} culled a caster the current frame can still show"
            );
        }
    }

    #[test]
    fn frame_sink_keeps_disoccluded_caster_and_culls_stable_one() {
        let (min, max) = aabb();
        let mut history = VisibilityHistory::default();
        let hidden = CasterKey::new(RenderSpaceId(1), 3, 9, 4, false);
        let revealed = CasterKey::new(RenderSpaceId(1), 4, 10, 5, false);
        history
            .casters
            .insert(hidden, record(VisibilityClass::Occluded, min, max));
        let history = Arc::new(history);
        let frame = VisibilityHysteresis::new(history, HashMap::new(), HashMap::new(), Vec::new());
        let facts = OcclusionFacts {
            occluded: true,
            other_occluder: false,
            rects: [None, None],
        };
        assert!(!frame.allows_draw(Some(caster_ref(hidden)), min, max, facts));
        assert!(frame.allows_draw(Some(caster_ref(revealed)), min, max, facts));
        let committed = frame.finish();
        assert_eq!(committed.len(), 2);
        assert_eq!(
            committed.get(&hidden).map(|record| record.class),
            Some(VisibilityClass::Occluded)
        );
        assert_eq!(
            committed.get(&revealed).map(|record| record.class),
            Some(VisibilityClass::Visible)
        );
    }

    #[test]
    fn visible_slot_wins_when_material_slots_share_a_key() {
        let (min, max) = aabb();
        let key = CasterKey::new(RenderSpaceId(1), 3, 9, 4, false);
        let frame = VisibilityHysteresis::new(
            Arc::new(VisibilityHistory::default()),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );
        let occluded = OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [None, None],
        };
        let visible = OcclusionFacts::keep();
        assert!(frame.allows_draw(Some(caster_ref(key)), min, max, occluded));
        assert!(frame.allows_draw(
            Some(CasterRef {
                key,
                mesh_asset_id: 7,
            }),
            min + Vec3::splat(0.2),
            max + Vec3::splat(0.2),
            visible,
        ));
        let committed = frame.finish();
        let stored = committed.get(&key).expect("merged caster");
        assert_eq!(stored.class, VisibilityClass::Visible);
        assert_eq!(stored.mesh_asset_id, 7);
        assert_eq!(stored.aabb_min, min);
        assert_eq!(stored.aabb_max, max + Vec3::splat(0.2));
    }

    #[test]
    fn spare_map_capacity_survives_finish() {
        let mut spare = HashMap::with_capacity(64);
        spare.insert(
            CasterKey::new(RenderSpaceId(1), 0, 0, 0, false),
            record(VisibilityClass::Visible, Vec3::ZERO, Vec3::ONE),
        );
        spare.clear();
        let capacity = spare.capacity();
        let frame = VisibilityHysteresis::new(
            Arc::new(VisibilityHistory::default()),
            spare,
            HashMap::new(),
            Vec::new(),
        );
        let committed = frame.finish();
        assert!(committed.capacity() >= capacity);
    }

    #[test]
    fn parallel_updates_merge_into_one_history() {
        let frame = VisibilityHysteresis::new(
            Arc::new(VisibilityHistory::default()),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );
        let facts = OcclusionFacts::keep();
        let (min, max) = aabb();
        {
            let frame = &frame;
            (0..64_u64).into_par_iter().for_each(|index| {
                let key = CasterKey::new(
                    RenderSpaceId(1),
                    usize::try_from(index).unwrap_or(0),
                    index,
                    i32::try_from(index).unwrap_or(i32::MAX),
                    false,
                );
                assert!(frame.allows_draw(Some(caster_ref(key)), min, max, facts));
            });
        }
        assert_eq!(frame.finish().len(), 64);
    }

    #[test]
    fn distant_caster_uses_a_linear_margin_with_a_close_near_plane() {
        let clip = perspective_clip(0.01, 1_000.0);
        let caster_depth = 20.0;
        let caster_ndc = ndc_from_view_depth(caster_depth, clip);
        let within_margin = ndc_from_view_depth(19.7, clip);
        let separate = ndc_from_view_depth(19.5, clip);
        assert!(
            (separate - caster_ndc).abs() < 0.02,
            "a 0.02 reverse-Z NDC gap cannot see a half-meter separation at 20 m"
        );
        assert!(!covering_depth_is_other_occluder(
            caster_ndc,
            within_margin,
            clip
        ));
        assert!(covering_depth_is_other_occluder(caster_ndc, separate, clip));
        assert!(!covering_depth_is_other_occluder(
            caster_ndc,
            separate,
            DepthClip {
                near: f32::NAN,
                far: 1_000.0,
                orthographic: false,
            }
        ));
    }

    #[test]
    fn screen_motion_pad_is_zero_when_stable_and_open_when_unbounded() {
        let near = 0.01;
        let far = 100.0;
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, -5.0), Vec3::ZERO, Vec3::Y);
        let narrow = reverse_z_perspective(1.0, 90.0_f32.to_radians(), near, far) * view;
        let wide = reverse_z_perspective(1.0, 60.0_f32.to_radians(), near, far) * view;
        let min = Vec3::splat(-0.25);
        let max = Vec3::splat(0.25);
        let stable = screen_motion_uv_pad(narrow, narrow, min, max).expect("stable projection");
        assert!(stable < 1.0e-4);
        let changed = screen_motion_uv_pad(narrow, wide, min, max).expect("fov projection");
        assert!(changed > 0.01);
        let behind = reverse_z_perspective(1.0, 90.0_f32.to_radians(), near, far)
            * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 10.0), Vec3::Y);
        assert!(screen_motion_uv_pad(narrow, behind, min, max).is_none());
    }

    #[test]
    fn bounds_motion_uses_the_world_epsilon() {
        let (min, max) = aabb();
        assert!(!bounds_moved((min, max), (min, max)));
        assert!(bounds_moved((min, max), (min + Vec3::splat(0.05), max)));
    }

    #[test]
    fn disocclusion_rects_cover_old_and_new_footprints() {
        let mover = MovedCaster {
            space_id: RenderSpaceId(1),
            previous_aabb: (Vec3::ZERO, Vec3::ONE),
            current_aabb: (Vec3::splat(2.0), Vec3::splat(3.0)),
        };
        let rects = disocclusion_rects(&[mover], |min, _max| {
            let anchor = min.x.clamp(0.0, 1.0);
            ScreenRect::from_uv((anchor, 0.2), (anchor, 0.4))
        });
        assert_eq!(rects.len(), 2);
        let probe = ScreenRect {
            min_u: 0.0,
            min_v: 0.25,
            max_u: 0.0,
            max_v: 0.3,
        };
        assert!(rects[0].overlaps(probe));
    }

    #[test]
    fn absent_visible_caster_and_mesh_swap_are_disoccluders() {
        let (min, max) = aabb();
        let hidden = CasterKey::new(RenderSpaceId(1), 1, 1, 1, false);
        let swapped = CasterKey::new(RenderSpaceId(1), 2, 2, 2, false);
        let steady = CasterKey::new(RenderSpaceId(1), 3, 3, 3, false);
        let occluded = CasterKey::new(RenderSpaceId(1), 4, 4, 4, false);
        let mut history = VisibilityHistory::default();
        history
            .casters
            .insert(hidden, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(swapped, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(steady, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(occluded, record(VisibilityClass::Occluded, min, max));
        let present = [
            PresentCaster {
                key: swapped,
                mesh_asset_id: 9,
                aabb_min: min,
                aabb_max: max,
                space_id: RenderSpaceId(1),
            },
            PresentCaster {
                key: steady,
                mesh_asset_id: 1,
                aabb_min: min,
                aabb_max: max,
                space_id: RenderSpaceId(1),
            },
        ];
        let movers = plan_disocclusion(&history, &present, |_| true);
        assert!(movers.iter().any(|mover| {
            mover.previous_aabb == (min, max) && mover.current_aabb == (min, max)
        }));
        assert!(
            movers
                .iter()
                .any(|mover| mover.space_id == RenderSpaceId(1))
        );
        assert_eq!(movers.len(), 2);
    }

    #[test]
    fn history_confirmation_turns_off_above_the_cap_and_stays_off_through_the_band() {
        assert!(history_confirmation_applies(
            Some(HISTORY_CASTER_BUDGET),
            false
        ));
        assert!(!history_confirmation_applies(None, false));
        assert!(!history_confirmation_applies(
            Some(HISTORY_CASTER_BUDGET),
            true
        ));
        assert!(!history_confirmation_applies(
            Some(HISTORY_CASTER_RESUME + 1),
            true
        ));
        assert!(history_confirmation_applies(
            Some(HISTORY_CASTER_RESUME),
            true
        ));
        assert!(history_within_budget(HISTORY_CASTER_BUDGET));
        assert!(!history_within_budget(HISTORY_CASTER_BUDGET + 1));
    }

    #[test]
    fn parallax_fails_open_past_eight_and_when_the_cover_is_farther() {
        let clip = perspective_clip(0.01, 10_000.0);
        let caster = ndc_from_view_depth(16.0, clip);
        let close_cover = ndc_from_view_depth(1.0, clip);
        assert!(
            parallax_adjusted_pad(0.02, caster, Some(close_cover), true, clip).is_none(),
            "a depth ratio of 16 must not cull"
        );
        let modest_caster = ndc_from_view_depth(10.0, clip);
        let just_over = ndc_from_view_depth(10.0 / 8.01, clip);
        assert!(parallax_adjusted_pad(0.02, modest_caster, Some(just_over), true, clip).is_none());
        let farther = ndc_from_view_depth(40.0, clip);
        let near_caster = ndc_from_view_depth(5.0, clip);
        assert!(parallax_adjusted_pad(0.02, near_caster, Some(farther), true, clip).is_none());
        assert!(parallax_adjusted_pad(0.02, near_caster, Some(0.0), true, clip).is_none());
        assert!(parallax_adjusted_pad(0.02, near_caster, None, true, clip).is_none());
    }

    #[test]
    fn parallax_scales_a_modest_closer_cover_and_ignores_ratio_when_visible() {
        let clip = perspective_clip(0.01, 10_000.0);
        let caster = ndc_from_view_depth(10.0, clip);
        let cover = ndc_from_view_depth(4.0, clip);
        let scaled =
            parallax_adjusted_pad(0.02, caster, Some(cover), true, clip).expect("ratio 2.5");
        assert!((scaled - 0.05).abs() < 1.0e-4);
        let at_cap = ndc_from_view_depth(10.0 / 8.0, clip);
        let capped =
            parallax_adjusted_pad(0.02, caster, Some(at_cap), true, clip).expect("ratio 8");
        assert!((capped - 0.16).abs() < 1.0e-3);
        let visible = parallax_adjusted_pad(
            0.02,
            caster,
            Some(ndc_from_view_depth(0.1, clip)),
            false,
            clip,
        )
        .expect("visible samples keep the base pad");
        assert!((visible - 0.02).abs() < 1.0e-6);
    }
}
