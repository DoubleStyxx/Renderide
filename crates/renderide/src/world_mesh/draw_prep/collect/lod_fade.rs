//! Screen-space LOD cross-fade decisions.
//!
//! The result is a [`LodFadeDecision`] stored by value on each draw item. The pair of LOD
//! weights is not a side `Vec`: two `f32`s live in this `Copy` value, and the visibility map
//! keeps at most a handful of inline slots before any heap spill.

/// Shader weight that leaves every fragment covered.
///
/// `0` is the historical zero value of the per-draw padding slot, so draws collected with
/// cross-fade off keep the same uniform bits as before this field existed.
pub(super) const LOD_FADE_DISABLED: f32 = 0.0;

/// Coverage at or above this value is treated as fully the current LOD.
const FULLY_CURRENT: f32 = 1.0;
/// Band and coverage values inside this epsilon collapse to a hard LOD switch.
const FADE_EDGE_EPS: f32 = 1.0e-4;
/// Inline fade slots per render space. A transitioning pair fits without a heap allocation.
const INLINE_FADE_SLOTS: usize = 8;

/// Which LOD rows to draw, and the signed stipple weight for each.
///
/// `primary_weight` / `partner_weight` are `0` when the shader should not stipple.
/// A positive weight `t` keeps fragments whose Bayer threshold is below `t`.
/// A negative weight `-t` keeps the complementary fragments.
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

/// One renderer ordinal and its signed stipple weight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct LodFadeSlot {
    /// Renderer ordinal inside the render space.
    ordinal: u32,
    /// Signed stipple weight. `0` is not stored.
    weight: f32,
}

/// Per-space fade weights. The inline array covers a normal LOD pair without allocating.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LodFadeTable {
    /// First fading renderers. Unused slots stay zero.
    inline: [LodFadeSlot; INLINE_FADE_SLOTS],
    /// Number of live entries in [`Self::inline`].
    inline_len: u8,
    /// Renderers past the inline capacity. Empty, and unallocated, for a normal pair.
    overflow: Vec<LodFadeSlot>,
}

impl Default for LodFadeTable {
    fn default() -> Self {
        Self {
            inline: [LodFadeSlot::default(); INLINE_FADE_SLOTS],
            inline_len: 0,
            overflow: Vec::new(),
        }
    }
}

impl LodFadeTable {
    /// Records `weight` for `ordinal`. A zero weight is not stored.
    pub(super) fn record(&mut self, ordinal: usize, weight: f32) {
        if weight == LOD_FADE_DISABLED || !weight.is_finite() {
            return;
        }
        let ordinal = u32::try_from(ordinal).unwrap_or(u32::MAX);
        if let Some(slot) = self.slot_mut(ordinal) {
            slot.weight = weight;
            return;
        }
        let slot = LodFadeSlot { ordinal, weight };
        let len = self.inline_len as usize;
        if len < INLINE_FADE_SLOTS {
            self.inline[len] = slot;
            self.inline_len = self.inline_len.saturating_add(1);
            return;
        }
        self.overflow.push(slot);
    }

    /// Signed stipple weight for `ordinal`, or [`LOD_FADE_DISABLED`] when it is not fading.
    pub(super) fn weight(&self, ordinal: usize) -> f32 {
        let Ok(ordinal) = u32::try_from(ordinal) else {
            return LOD_FADE_DISABLED;
        };
        self.slots()
            .find(|slot| slot.ordinal == ordinal)
            .map(|slot| slot.weight)
            .unwrap_or(LOD_FADE_DISABLED)
    }

    /// Heap capacity of the overflow list. Zero when the inline slots were enough.
    #[cfg(test)]
    pub(super) fn overflow_capacity(&self) -> usize {
        self.overflow.capacity()
    }

    fn slot_mut(&mut self, ordinal: u32) -> Option<&mut LodFadeSlot> {
        let len = self.inline_len as usize;
        if let Some(slot) = self.inline[..len]
            .iter_mut()
            .find(|slot| slot.ordinal == ordinal)
        {
            return Some(slot);
        }
        self.overflow
            .iter_mut()
            .find(|slot| slot.ordinal == ordinal)
    }

    fn slots(&self) -> impl Iterator<Item = &LodFadeSlot> {
        let len = self.inline_len as usize;
        self.inline[..len].iter().chain(self.overflow.iter())
    }
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
    fn a_fading_pair_does_not_allocate_the_overflow_list() {
        let mut table = LodFadeTable::default();
        table.record(0, 0.5);
        table.record(1, -0.5);
        table.record(0, 0.25);

        assert_eq!(table.weight(0), 0.25);
        assert_eq!(table.weight(1), -0.5);
        assert_eq!(table.weight(2), LOD_FADE_DISABLED);
        assert_eq!(table.overflow_capacity(), 0);
        assert!(!std::mem::needs_drop::<LodFadeDecision>());
    }

    #[test]
    fn overflow_allocates_only_after_the_inline_slots_are_full() {
        let mut table = LodFadeTable::default();
        for ordinal in 0..INLINE_FADE_SLOTS {
            table.record(ordinal, 0.4);
        }
        assert_eq!(table.overflow_capacity(), 0);
        table.record(INLINE_FADE_SLOTS, -0.4);
        assert!(table.overflow_capacity() > 0);
        assert_eq!(table.weight(INLINE_FADE_SLOTS), -0.4);
    }
}
