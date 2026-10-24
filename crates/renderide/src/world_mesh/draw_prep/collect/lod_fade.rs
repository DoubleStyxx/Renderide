//! Screen-space LOD cross-fade decisions.
//!
//! The result is a [`LodFadeDecision`] stored by value on each draw item. Per-renderer weights
//! live in a dense ordinal array owned by the view. The array is allocated only when a renderer
//! in that view records a non-zero weight, and it is dropped with the view. There is no
//! thread-local pool, so a buffer acquired on one worker cannot migrate onto another.

use std::sync::Arc;

use hashbrown::{HashMap, HashSet};

use crate::materials::RasterPipelineKind;
use crate::scene::{
    LodEntry, LodRendererKind, LodRendererRef, RenderSpaceId, RenderSpaceView, StaticMeshRenderer,
};
use crate::shared::ShadowCastMode;

use super::super::prepared_renderables::{
    FramePreparedDraw, FramePreparedLodGroup, FramePreparedRenderables, LodStippleIndex,
};
use super::DrawCollectionInputs;
use super::lod::{
    LodVisibility, ResolvedLodEntry, skinned_renderer_for_lod_ref, static_renderer_for_lod_ref,
};

/// Shader weight that leaves every fragment covered.
///
/// `0` is the historical zero value of the per-draw padding slot, so draws collected with
/// cross-fade off keep the same uniform bits as before this field existed.
pub(super) const LOD_FADE_DISABLED: f32 = 0.0;

/// Composed stem of `shaders/materials/unlit.wgsl`, the only fragment shader that stipples.
///
/// PBS and the other unlit-family stems still draw solid fragments. Fading those groups would
/// overdraw and punch depth holes, so they keep the single-LOD swap.
const LOD_STIPPLE_STEM: &str = "unlit_default";
/// Forward stem selected when an unlit draw's fade weight is non-zero.
const LOD_STIPPLE_FADE_STEM: &str = "unlit_fade_default";
/// Multiview form of [`LOD_STIPPLE_FADE_STEM`].
const LOD_STIPPLE_FADE_MULTIVIEW_STEM: &str = "unlit_fade_multiview";

/// Coverage at or above this value is treated as fully the current LOD.
const FULLY_CURRENT: f32 = 1.0;
/// Band and coverage values inside this epsilon collapse to a hard LOD switch.
const FADE_EDGE_EPS: f32 = 1.0e-4;

/// Which LOD rows to draw, and the signed stipple weight for each.
///
/// `primary_weight` / `partner_weight` are `0` when the shader should not stipple.
/// A positive weight `t` keeps fragments whose Bayer threshold is below `t`.
/// A negative weight `-t` keeps the complementary fragments. The negative weight is also
/// the partner row: shadow collection forces that row off so a caster draws one LOD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LodFadeDecision {
    /// LOD row that owns the higher-detail side of this choice.
    pub(crate) primary: u32,
    /// Coarser LOD row drawn during a transition.
    pub(crate) partner: Option<u32>,
    /// Signed stipple weight for [`Self::primary`].
    pub(crate) primary_weight: f32,
    /// Signed stipple weight for [`Self::partner`].
    pub(crate) partner_weight: f32,
}

impl LodFadeDecision {
    /// One LOD row, with stippling disabled.
    pub(crate) fn single(index: usize) -> Self {
        Self {
            primary: u32::try_from(index).unwrap_or(u32::MAX),
            partner: None,
            primary_weight: LOD_FADE_DISABLED,
            partner_weight: LOD_FADE_DISABLED,
        }
    }

    /// `true` when both LOD rows must be drawn.
    #[cfg(any(test, feature = "bench-api"))]
    pub(crate) fn draws_partner(self) -> bool {
        self.partner.is_some()
    }
}

/// Dense per-ordinal stipple weights for one render space in one view.
///
/// The buffer is owned by the view. [`LodVisibility`] allocates it only when a renderer records
/// a non-zero weight, so a view with no group in a fade band does not fill or allocate. Dropping
/// the view frees the buffer on whichever thread owns the view; nothing is returned to a
/// thread-local pool.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LodFadeTable {
    /// `weights[ordinal]` is the signed stipple weight. `0` means the shader is unchanged.
    weights: Vec<f32>,
}

impl LodFadeTable {
    /// Zero-fills a scratch buffer that can address `len` renderer ordinals.
    ///
    /// Callers must not call this for a view that has no fading renderer.
    pub(super) fn acquire(len: usize) -> Self {
        if len == 0 {
            return Self::default();
        }
        let mut weights = Vec::with_capacity(len);
        weights.resize(len, 0.0);
        Self { weights }
    }

    /// `true` when this view has not allocated a weight buffer.
    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    /// Records `weight` for `ordinal`. A zero weight is left as the cleared default.
    ///
    /// Returns whether a non-zero weight was stored. Callers use that to mark the view as fading
    /// without a second scan of the table. An empty table does not grow: the caller allocates
    /// once, before the first non-zero record.
    pub(super) fn record(&mut self, ordinal: usize, weight: f32) -> bool {
        if weight == LOD_FADE_DISABLED || !weight.is_finite() {
            return false;
        }
        if let Some(slot) = self.weights.get_mut(ordinal) {
            *slot = weight;
            return true;
        }
        false
    }

    /// Signed stipple weight for `ordinal`, or [`LOD_FADE_DISABLED`] when it is not fading.
    #[inline]
    pub(super) fn weight(&self, ordinal: usize) -> f32 {
        self.weights
            .get(ordinal)
            .copied()
            .unwrap_or(LOD_FADE_DISABLED)
    }

    /// Scratch capacity. Zero until [`Self::acquire`] runs for this view.
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.weights.capacity()
    }
}

/// Points a non-zero unlit fade at the stippling shader. A zero weight keeps the historical stem.
pub(super) fn select_unlit_fade_pipeline(pipeline: &mut RasterPipelineKind, lod_fade: f32) {
    if lod_fade == 0.0 || !lod_fade.is_finite() {
        return;
    }
    let RasterPipelineKind::EmbeddedStem(stem) = pipeline else {
        return;
    };
    let next = match stem.as_ref() {
        LOD_STIPPLE_STEM => LOD_STIPPLE_FADE_STEM,
        "unlit_multiview" => LOD_STIPPLE_FADE_MULTIVIEW_STEM,
        _ => return,
    };
    *pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(next));
}

/// Unity-style mesh LOD bias. Non-finite and non-positive values fall back to `1`.
pub(super) fn biased_relative_height(relative_height: f32, mesh_lod_bias: f32) -> f32 {
    let bias = if mesh_lod_bias.is_finite() && mesh_lod_bias > 0.0 {
        mesh_lod_bias
    } else {
        1.0
    };
    relative_height.max(0.0) * bias
}

/// Fraction of the current LOD still covered at `effective_height`.
///
/// `1` means the transition band has not started. `0` means the next LOD is fully covered.
/// The band is `fade_width` times the span from `previous_height` down to `selected_height`,
/// sitting on the low side of the current LOD's range.
pub(crate) fn lod_cross_fade_coverage(
    selected_height: f32,
    previous_height: f32,
    fade_width: f32,
    has_next: bool,
    effective_height: f32,
    cross_fade: bool,
) -> f32 {
    if !cross_fade || !has_next {
        return FULLY_CURRENT;
    }
    let current = sanitize_unit_height(selected_height);
    let previous = sanitize_unit_height(previous_height).max(current);
    let width = if fade_width.is_finite() {
        fade_width.clamp(0.0, FULLY_CURRENT)
    } else {
        0.0
    };
    let band = width * (previous - current);
    if band <= FADE_EDGE_EPS {
        return FULLY_CURRENT;
    }
    let fade_start = current + band;
    if effective_height >= fade_start {
        return FULLY_CURRENT;
    }
    ((effective_height - current) / band).clamp(0.0, FULLY_CURRENT)
}

/// Builds the draw choice for one selected LOD row.
pub(crate) fn lod_fade_decision(selected: usize, has_next: bool, coverage: f32) -> LodFadeDecision {
    if !has_next || coverage >= FULLY_CURRENT - FADE_EDGE_EPS {
        return LodFadeDecision::single(selected);
    }
    if coverage <= FADE_EDGE_EPS {
        return LodFadeDecision::single(selected.saturating_add(1));
    }
    LodFadeDecision {
        primary: u32::try_from(selected).unwrap_or(u32::MAX),
        partner: u32::try_from(selected.saturating_add(1)).ok(),
        primary_weight: coverage,
        partner_weight: -coverage,
    }
}

/// Clamps a host transition height into the unit interval.
pub(super) fn sanitize_unit_height(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, FULLY_CURRENT)
    } else {
        0.0
    }
}

/// `true` when `stem` is the unlit shader that discards faded fragments.
pub(super) fn material_stem_stipples_lod(stem: Option<&str>) -> bool {
    stem == Some(LOD_STIPPLE_STEM)
}

/// `true` when every stem is the stippling unlit shader and the group is non-empty.
#[cfg(test)]
fn every_material_stem_stipples<'a>(stems: impl IntoIterator<Item = Option<&'a str>>) -> bool {
    let mut saw = false;
    for stem in stems {
        saw = true;
        if !material_stem_stipples_lod(stem) {
            return false;
        }
    }
    saw
}

/// Shadow mode for one draw. A partner row (`lod_fade < 0`) does not cast.
pub(super) fn shadow_cast_mode_for_lod_fade(mode: ShadowCastMode, lod_fade: f32) -> ShadowCastMode {
    if lod_fade < 0.0 {
        ShadowCastMode::Off
    } else {
        mode
    }
}

/// `true` when `material_asset_id` routes to [`LOD_STIPPLE_STEM`].
pub(super) fn material_id_stipples(ctx: &DrawCollectionInputs<'_>, material_asset_id: i32) -> bool {
    let stem = ctx
        .materials
        .dict
        .shader_asset_for_material(material_asset_id)
        .and_then(|shader_id| ctx.materials.router.stem_for_shader_asset(shader_id));
    material_stem_stipples_lod(stem)
}

/// `true` when every prepared draw in `group` is a color unlit material.
///
/// The answer is an O(1) lookup in an index built once per prepared snapshot, from one pass over
/// the draws. Callers must invoke this only for a group already inside a fade band.
pub(super) fn prepared_group_stipples(
    ctx: &DrawCollectionInputs<'_>,
    group: &FramePreparedLodGroup,
) -> bool {
    let Some(prepared) = ctx.caches.prepared else {
        return false;
    };
    if let Some(index) = prepared.lod_stipple().get() {
        return index.group_stipples(group.space_id, group.scene_group_index);
    }
    let built = build_lod_stipple_index(ctx, prepared);
    prepared
        .lod_stipple()
        .get_or_init(|| built)
        .group_stipples(group.space_id, group.scene_group_index)
}

/// One draw pass over cross-fade members, then one bool per cross-fade group.
///
/// Ordinals that are not in a cross-fade group are skipped, so the material-stem lookup does not
/// walk the whole prepared list once per renderer. The resulting bool is what view selection
/// reads, and only for a group whose height is inside the fade band.
fn build_lod_stipple_index(
    ctx: &DrawCollectionInputs<'_>,
    prepared: &FramePreparedRenderables,
) -> LodStippleIndex {
    let mut members = HashSet::new();
    for group in prepared.lod_groups() {
        if !group.cross_fade {
            continue;
        }
        for lod in &group.lods {
            for renderer in &lod.renderers {
                members.insert((group.space_id, renderer.renderer_ordinal));
            }
        }
    }
    // `1` means every draw seen so far is color unlit. `2` means one draw cannot stipple.
    let mut ordinal_state: HashMap<(RenderSpaceId, usize), u8> = HashMap::new();
    if !members.is_empty() {
        for draw in prepared.draws() {
            let key = (draw.space_id, draw.renderer_ordinal);
            if !members.contains(&key) {
                continue;
            }
            let ok = draw_stipples(ctx, draw);
            match ordinal_state.get_mut(&key) {
                Some(state) if !ok => *state = 2,
                Some(_) => {}
                None => {
                    ordinal_state.insert(key, if ok { 1 } else { 2 });
                }
            }
        }
    }
    let mut by_group = HashMap::new();
    for group in prepared.lod_groups() {
        if !group.cross_fade {
            continue;
        }
        let stipples = group_ordinals_stipple(&ordinal_state, group);
        by_group.insert((group.space_id, group.scene_group_index), stipples);
    }
    LodStippleIndex::new(by_group)
}

fn draw_stipples(ctx: &DrawCollectionInputs<'_>, draw: &FramePreparedDraw) -> bool {
    draw.shadow_cast_mode != ShadowCastMode::ShadowOnly
        && material_id_stipples(ctx, draw.material_asset_id)
}

fn group_ordinals_stipple(
    ordinal_state: &HashMap<(RenderSpaceId, usize), u8>,
    group: &FramePreparedLodGroup,
) -> bool {
    let mut saw = false;
    for lod in &group.lods {
        for renderer in &lod.renderers {
            saw = true;
            if ordinal_state
                .get(&(group.space_id, renderer.renderer_ordinal))
                .copied()
                != Some(1)
            {
                return false;
            }
        }
    }
    saw
}

/// Chooses one LOD or a fading pair from the row that passed the height test.
pub(super) fn fade_decision_for_rows(
    selected_index: usize,
    lod_count: usize,
    source: Option<&[LodEntry]>,
    measured_height: Option<f32>,
    cross_fade: bool,
) -> LodFadeDecision {
    let Some(effective_height) = measured_height else {
        return LodFadeDecision::single(selected_index);
    };
    let Some(source) = source else {
        return LodFadeDecision::single(selected_index);
    };
    let Some(selected) = source.get(selected_index) else {
        return LodFadeDecision::single(selected_index);
    };
    let previous_height = if selected_index == 0 {
        1.0
    } else {
        source
            .get(selected_index - 1)
            .map(|lod| lod.screen_relative_transition_height)
            .unwrap_or(1.0)
    };
    let has_next = selected_index.saturating_add(1) < lod_count.min(source.len());
    let coverage = lod_cross_fade_coverage(
        selected.screen_relative_transition_height,
        previous_height,
        selected.fade_transition_width,
        has_next,
        effective_height,
        cross_fade,
    );
    lod_fade_decision(selected_index, has_next, coverage)
}

/// Fade choice for a prepared group, using widths copied at prepare time.
///
/// This does not look up the scene space. The caller passes `allow_fade` only after the group's
/// stored `cross_fade` flag is set, so a view with cross-fade off never reaches here.
pub(super) fn fade_decision_for_prepared_group(
    group: &FramePreparedLodGroup,
    selected_index: usize,
    measured_height: Option<f32>,
    allow_fade: bool,
) -> LodFadeDecision {
    let Some(effective_height) = measured_height else {
        return LodFadeDecision::single(selected_index);
    };
    let Some(selected) = group.lods.get(selected_index) else {
        return LodFadeDecision::single(selected_index);
    };
    let previous_height = if selected_index == 0 {
        1.0
    } else {
        group
            .lods
            .get(selected_index - 1)
            .map(|lod| lod.screen_relative_transition_height)
            .unwrap_or(1.0)
    };
    let has_next = selected_index.saturating_add(1) < group.lods.len();
    let coverage = lod_cross_fade_coverage(
        selected.screen_relative_transition_height,
        previous_height,
        selected.fade_transition_width,
        has_next,
        effective_height,
        allow_fade,
    );
    lod_fade_decision(selected_index, has_next, coverage)
}

/// Marks the prepared renderers chosen by `decision` and stores non-zero stipple weights.
pub(super) fn commit_prepared_decision(
    visibility: &mut LodVisibility,
    group: &FramePreparedLodGroup,
    decision: &LodFadeDecision,
) {
    mark_prepared_row(
        visibility,
        group,
        decision.primary as usize,
        decision.primary_weight,
    );
    if let Some(partner) = decision.partner {
        mark_prepared_row(visibility, group, partner as usize, decision.partner_weight);
    }
}

fn mark_prepared_row(
    visibility: &mut LodVisibility,
    group: &FramePreparedLodGroup,
    index: usize,
    weight: f32,
) {
    let Some(lod) = group.lods.get(index) else {
        return;
    };
    for renderer in &lod.renderers {
        visibility.mark_selected_ordinal(group.space_id, renderer.renderer_ordinal);
        visibility.record_fade_ordinal(group.space_id, renderer.renderer_ordinal, weight);
    }
}

/// Marks the resolved renderers chosen by `decision` and stores non-zero stipple weights.
pub(super) fn commit_resolved_decision(
    visibility: &mut LodVisibility,
    space_id: RenderSpaceId,
    resolved: &[ResolvedLodEntry],
    decision: &LodFadeDecision,
) {
    mark_resolved_row(
        visibility,
        space_id,
        resolved,
        decision.primary as usize,
        decision.primary_weight,
    );
    if let Some(partner) = decision.partner {
        mark_resolved_row(
            visibility,
            space_id,
            resolved,
            partner as usize,
            decision.partner_weight,
        );
    }
}

/// `true` when every renderer in the group draws color through the stippling unlit shader.
pub(super) fn scene_group_can_cross_fade(
    ctx: &DrawCollectionInputs<'_>,
    space_id: RenderSpaceId,
    space: RenderSpaceView<'_>,
    lods: &[LodEntry],
) -> bool {
    let mut saw = false;
    for lod in lods {
        for renderer_ref in &lod.renderers {
            let Some(base) = lod_ref_base(space, renderer_ref) else {
                return false;
            };
            let Some(renderable_index) = lod_ref_index(space, renderer_ref) else {
                return false;
            };
            if !renderer_materials_stipple(
                ctx,
                space_id,
                renderer_ref.kind == LodRendererKind::Skinned,
                renderable_index,
                base,
            ) {
                return false;
            }
            saw = true;
        }
    }
    saw
}

fn lod_ref_base<'a>(
    space: RenderSpaceView<'a>,
    renderer_ref: &LodRendererRef,
) -> Option<&'a StaticMeshRenderer> {
    match renderer_ref.kind {
        LodRendererKind::Static => static_renderer_for_lod_ref(space, renderer_ref),
        LodRendererKind::Skinned => {
            skinned_renderer_for_lod_ref(space, renderer_ref).map(|renderer| &renderer.base)
        }
    }
}

fn lod_ref_index(space: RenderSpaceView<'_>, renderer_ref: &LodRendererRef) -> Option<usize> {
    match renderer_ref.kind {
        LodRendererKind::Static => space
            .static_mesh_renderers()
            .iter()
            .position(|renderer| renderer.instance_id == renderer_ref.instance_id),
        LodRendererKind::Skinned => space
            .skinned_mesh_renderers()
            .iter()
            .position(|renderer| renderer.base.instance_id == renderer_ref.instance_id),
    }
}

fn renderer_materials_stipple(
    ctx: &DrawCollectionInputs<'_>,
    space_id: RenderSpaceId,
    skinned: bool,
    renderable_index: usize,
    renderer: &StaticMeshRenderer,
) -> bool {
    if !renderer.emits_visible_color_draws() {
        return false;
    }
    let material_id = |slot_index: usize, fallback: i32| {
        ctx.scene_assets
            .scene
            .overridden_material_asset_id(
                space_id,
                ctx.view.render_context,
                skinned,
                renderable_index,
                slot_index,
            )
            .unwrap_or(fallback)
    };
    if renderer.material_slots.is_empty() {
        return renderer
            .primary_material_asset_id
            .is_some_and(|primary| material_id_stipples(ctx, material_id(0, primary)));
    }
    renderer
        .material_slots
        .iter()
        .enumerate()
        .all(|(slot_index, slot)| {
            material_id_stipples(ctx, material_id(slot_index, slot.material_asset_id))
        })
}

fn mark_resolved_row(
    visibility: &mut LodVisibility,
    space_id: RenderSpaceId,
    resolved: &[ResolvedLodEntry],
    index: usize,
    weight: f32,
) {
    let Some(lod) = resolved.get(index) else {
        return;
    };
    for instance_id in &lod.renderers {
        visibility.mark_selected(space_id, *instance_id);
        let Some(ordinal) = visibility.instance_ordinal(space_id, *instance_id) else {
            continue;
        };
        visibility.record_fade_ordinal(space_id, ordinal, weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_fade_off_is_a_single_lod_with_stipple_disabled() {
        let coverage = lod_cross_fade_coverage(0.2, 1.0, 1.0, true, 0.5, false);
        let decision = lod_fade_decision(0, true, coverage);

        assert!((coverage - 1.0).abs() < 1.0e-6);
        assert_eq!(decision, LodFadeDecision::single(0));
        assert!(!decision.draws_partner());
    }

    #[test]
    fn mid_transition_draws_both_lods_with_complementary_weights() {
        let coverage = lod_cross_fade_coverage(0.2, 1.0, 0.5, true, 0.4, true);
        let decision = lod_fade_decision(0, true, coverage);

        assert!((coverage - 0.5).abs() < 1.0e-5, "coverage {coverage}");
        assert_eq!(decision.primary, 0);
        assert_eq!(decision.partner, Some(1));
        assert!((decision.primary_weight - 0.5).abs() < 1.0e-5);
        assert!((decision.partner_weight + 0.5).abs() < 1.0e-5);
    }

    #[test]
    fn outside_the_band_and_past_the_last_lod_draw_one_row() {
        let above = lod_cross_fade_coverage(0.2, 1.0, 0.25, true, 0.9, true);
        assert!(
            (above - 1.0).abs() < 1.0e-6,
            "height above the band stays on the current LOD"
        );
        assert_eq!(
            lod_fade_decision(0, true, above),
            LodFadeDecision::single(0)
        );

        let at_end = lod_cross_fade_coverage(0.05, 0.2, 1.0, false, 0.1, true);
        assert!((at_end - 1.0).abs() < 1.0e-6);
        assert_eq!(
            lod_fade_decision(1, false, at_end),
            LodFadeDecision::single(1)
        );
    }

    #[test]
    fn zero_fade_width_does_not_open_a_band() {
        let coverage = lod_cross_fade_coverage(0.2, 1.0, 0.0, true, 0.2, true);
        assert!((coverage - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn decision_is_copy_and_holds_no_heap_pointer() {
        let decision = lod_fade_decision(0, true, 0.5);
        let copied = decision;
        assert!(copied.draws_partner());
        assert!(!std::mem::needs_drop::<LodFadeDecision>());
        assert!(size_of::<LodFadeDecision>() <= 24);
    }

    #[test]
    fn non_unlit_stems_keep_a_single_lod() {
        assert!(material_stem_stipples_lod(Some("unlit_default")));
        assert!(!material_stem_stipples_lod(Some("pbsmetallic_default")));
        assert!(!material_stem_stipples_lod(Some("ui_unlit_default")));
        assert!(!material_stem_stipples_lod(Some("textunlit_default")));
        assert!(!material_stem_stipples_lod(None));
        assert!(every_material_stem_stipples([
            Some("unlit_default"),
            Some("unlit_default"),
        ]));
        assert!(!every_material_stem_stipples([
            Some("unlit_default"),
            Some("pbsmetallic_default"),
        ]));
        assert!(!every_material_stem_stipples(std::iter::empty()));
    }

    #[test]
    fn partner_lod_does_not_cast_a_second_shadow() {
        assert_eq!(
            shadow_cast_mode_for_lod_fade(ShadowCastMode::On, -0.4),
            ShadowCastMode::Off
        );
        assert_eq!(
            shadow_cast_mode_for_lod_fade(ShadowCastMode::DoubleSided, -0.4),
            ShadowCastMode::Off
        );
        assert_eq!(
            shadow_cast_mode_for_lod_fade(ShadowCastMode::On, 0.4),
            ShadowCastMode::On
        );
        assert_eq!(
            shadow_cast_mode_for_lod_fade(ShadowCastMode::On, 0.0),
            ShadowCastMode::On
        );
    }

    #[test]
    fn fade_weight_scratch_stays_empty_until_a_view_allocates_it() {
        let idle = LodFadeTable::default();
        assert!(idle.is_empty());
        assert_eq!(idle.capacity(), 0);
        let mut untouched = LodFadeTable::default();
        assert!(!untouched.record(0, 0.4));
        assert_eq!(untouched.capacity(), 0);

        let mut view = LodFadeTable::acquire(64);
        let capacity = view.capacity();
        assert!(!view.record(0, 0.0));
        assert!(!view.record(99, 0.4));
        for ordinal in 0..64 {
            assert!(view.record(ordinal, if ordinal % 2 == 0 { 0.35 } else { -0.35 }));
        }
        assert_eq!(
            view.capacity(),
            capacity,
            "recording must not grow the buffer"
        );
        assert_eq!(view.weight(63), -0.35);
        assert_eq!(view.weight(64), LOD_FADE_DISABLED);

        rayon::scope(|scope| {
            scope.spawn(move |_| drop(view));
        });
        let after_other_thread = LodFadeTable::default();
        assert_eq!(
            after_other_thread.capacity(),
            0,
            "dropping a view buffer on another thread must not allocate scratch for the next view"
        );
    }

    #[test]
    fn nonzero_fade_selects_the_unlit_fade_pipeline() {
        use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

        let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
            material_asset_id: 1,
            property_block: None,
            skinned: false,
            sorting_order: 0,
            mesh_asset_id: 1,
            node_id: 1,
            slot_index: 0,
            collect_order: 0,
            alpha_blended: false,
        });
        item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(LOD_STIPPLE_STEM));
        select_unlit_fade_pipeline(&mut item.batch_key.pipeline, 0.0);
        assert!(matches!(
            &item.batch_key.pipeline,
            RasterPipelineKind::EmbeddedStem(stem) if stem.as_ref() == LOD_STIPPLE_STEM
        ));
        select_unlit_fade_pipeline(&mut item.batch_key.pipeline, -0.4);
        assert!(matches!(
            &item.batch_key.pipeline,
            RasterPipelineKind::EmbeddedStem(stem) if stem.as_ref() == LOD_STIPPLE_FADE_STEM
        ));
        item.batch_key.pipeline =
            RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default"));
        select_unlit_fade_pipeline(&mut item.batch_key.pipeline, 0.4);
        assert!(matches!(
            &item.batch_key.pipeline,
            RasterPipelineKind::EmbeddedStem(stem) if stem.as_ref() == "pbsmetallic_default"
        ));
    }
}
