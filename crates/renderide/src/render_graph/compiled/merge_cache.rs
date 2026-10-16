//! Compile-time raster-merge labels and coalesced attachment templates.
//!
//! Recording looks these up instead of rebuilding `String` labels and cloning pass templates
//! on every materialized render pass.

use std::borrow::Cow;

use super::super::pass::{PassKind, PassNode, RenderPassTemplate};
use super::super::schedule::{FrameSchedule, RenderPassMaterializationGroup, ScheduleStep};
use super::helpers;
use super::resource::CompiledPassInfo;

/// Outcome of trying to coalesce one materialization group at graph build time.
pub(in crate::render_graph) enum RasterMergeCacheEntry {
    /// At least one step is not a graph-managed raster pass.
    NotRaster,
    /// A raster step has no compiled attachment template.
    MissingTemplate {
        /// Pass name stored on [`crate::render_graph::error::GraphExecuteError::MissingRasterTemplate`].
        pass: String,
    },
    /// Attachment load/store rules reject merging this group.
    Incompatible,
    /// Static label and coalesced template reused by every frame.
    Ready {
        /// `graph::raster_merge(...)` profiler and wgpu pass label.
        label: String,
        /// Attachment template shared by the merged render pass.
        template: RenderPassTemplate,
    },
}

/// One materialization group cached for the frame record path.
pub(in crate::render_graph) struct CachedRasterMergeGroup {
    /// First schedule-step index in the group.
    pub(super) start_step: usize,
    /// Exclusive schedule-step index after the group.
    pub(super) end_step: usize,
    /// Static merge decision for this group.
    pub(super) entry: RasterMergeCacheEntry,
}

/// Builds the per-group merge cache from the compiled schedule.
pub(in crate::render_graph) fn build_raster_merge_cache(
    schedule: &FrameSchedule,
    passes: &[PassNode],
    pass_info: &[CompiledPassInfo],
) -> Vec<CachedRasterMergeGroup> {
    schedule
        .render_pass_materialization_plan
        .groups
        .iter()
        .copied()
        .map(|group| CachedRasterMergeGroup {
            start_step: group.start_step,
            end_step: group.end_step,
            entry: cache_entry(schedule, passes, pass_info, group),
        })
        .collect()
}

fn cache_entry(
    schedule: &FrameSchedule,
    passes: &[PassNode],
    pass_info: &[CompiledPassInfo],
    group: RenderPassMaterializationGroup,
) -> RasterMergeCacheEntry {
    let Some(steps) = schedule.steps.get(group.start_step..group.end_step) else {
        return RasterMergeCacheEntry::NotRaster;
    };
    if steps.is_empty()
        || !steps.iter().all(|step| {
            passes
                .get(step.pass_idx)
                .is_some_and(|pass| pass.kind() == PassKind::Raster)
        })
    {
        return RasterMergeCacheEntry::NotRaster;
    }
    let mut templates = Vec::with_capacity(steps.len());
    for step in steps {
        let Some(info) = pass_info.get(step.pass_idx) else {
            return RasterMergeCacheEntry::MissingTemplate {
                pass: format!("pass#{}", step.pass_idx),
            };
        };
        let Some(template) = info.raster_template.as_ref() else {
            return RasterMergeCacheEntry::MissingTemplate {
                pass: info.name.clone(),
            };
        };
        templates.push(template);
    }
    let Some(template) = helpers::coalesce_render_pass_template(&templates) else {
        return RasterMergeCacheEntry::Incompatible;
    };
    RasterMergeCacheEntry::Ready {
        label: merge_label(passes, steps),
        template,
    }
}

fn merge_label(passes: &[PassNode], steps: &[ScheduleStep]) -> String {
    let mut label = String::from("graph::raster_merge(");
    for (idx, step) in steps.iter().enumerate() {
        if idx != 0 {
            label.push_str(" + ");
        }
        let profiling_label = passes
            .get(step.pass_idx)
            .map(PassNode::profiling_label)
            .unwrap_or(Cow::Borrowed("pass"));
        label.push_str(profiling_label.as_ref());
    }
    label.push(')');
    label
}
