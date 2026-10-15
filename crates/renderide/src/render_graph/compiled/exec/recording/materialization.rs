//! Raster pass materialization and phase-step recording helpers.

use super::super::super::super::context::{PassFrameContext, RasterPassCtx};
use super::super::super::super::error::GraphExecuteError;
use super::super::super::super::pass::{PassKind, PassNode, RenderPassTemplate};
use super::super::super::super::schedule::{RenderPassMaterializationGroup, ScheduleStep};
use super::super::super::helpers;
use super::super::super::{CachedRasterMergeGroup, CompiledRenderGraph, RasterMergeCacheEntry};
use super::{
    PassExecution, PassGpuInputs, PassRecordTargets, PassViewInputs, PhaseRecordingScope,
    update_command_stats,
};
use crate::frame_upload_batch::{FrameUploadBatch, GraphUploadSink};
use crate::render_graph::blackboard::GraphCommandStats;

const INLINE_MERGE_TEMPLATES: usize = 8;
const MERGE_LABEL_STACK_CAP: usize = 512;

/// Borrowed raster templates for one materialization group.
struct MergeTemplateList<'a> {
    inline: [&'a RenderPassTemplate; INLINE_MERGE_TEMPLATES],
    len: usize,
    overflow: Vec<&'a RenderPassTemplate>,
}

impl<'a> MergeTemplateList<'a> {
    fn as_slice(&self) -> &[&'a RenderPassTemplate] {
        if self.overflow.is_empty() {
            &self.inline[..self.len]
        } else {
            self.overflow.as_slice()
        }
    }
}

impl CompiledRenderGraph {
    /// Finds a materialization group that begins at `step_idx`.
    fn materialization_group_starting_at(
        &self,
        step_idx: usize,
    ) -> Option<RenderPassMaterializationGroup> {
        self.schedule
            .render_pass_materialization_plan
            .groups
            .iter()
            .copied()
            .find(|group| group.start_step == step_idx)
    }

    /// Returns the compile-time cache entry for `group`, when this graph was built with one.
    fn cached_raster_merge(
        &self,
        group: RenderPassMaterializationGroup,
    ) -> Option<&CachedRasterMergeGroup> {
        self.raster_merge_cache.iter().find(|cached| {
            cached.start_step == group.start_step && cached.end_step == group.end_step
        })
    }

    /// Records all schedule steps for one pass phase in flat topological order.
    pub(super) fn record_phase_steps<'a>(
        &self,
        scope: PhaseRecordingScope,
        view: PassViewInputs<'a>,
        mut targets: PassRecordTargets<'a, '_, '_>,
        gpu: PassGpuInputs<'a>,
        upload_batch: &FrameUploadBatch,
    ) -> Result<(), GraphExecuteError> {
        self.record_phase_steps_range(
            scope,
            0..self.schedule.steps.len(),
            view,
            targets.reborrow(),
            gpu,
            upload_batch,
        )
    }

    /// Records schedule steps for one pass phase inside a flat step range.
    pub(super) fn record_phase_steps_range<'a>(
        &self,
        scope: PhaseRecordingScope,
        step_range: std::ops::Range<usize>,
        view: PassViewInputs<'a>,
        mut targets: PassRecordTargets<'a, '_, '_>,
        gpu: PassGpuInputs<'a>,
        upload_batch: &FrameUploadBatch,
    ) -> Result<(), GraphExecuteError> {
        let end_step = step_range.end.min(self.schedule.steps.len());
        let mut step_idx = step_range.start.min(end_step);
        while step_idx < end_step {
            let step = self.schedule.steps[step_idx];
            if step.phase != scope.phase {
                step_idx += 1;
                continue;
            }
            if let Some(group) = self.materialization_group_starting_at(step_idx)
                && group.end_step <= end_step
                && self.try_execute_raster_materialization_group(
                    group,
                    scope,
                    view,
                    targets.reborrow(),
                    gpu,
                    upload_batch,
                )?
            {
                step_idx = group.end_step;
                continue;
            }
            self.execute_pass_node(
                PassExecution {
                    pass_idx: step.pass_idx,
                    upload_scope: step.frame_upload_scope(scope.view_idx),
                },
                view,
                targets.reborrow(),
                gpu,
                upload_batch,
            )?;
            step_idx += 1;
        }
        Ok(())
    }

    /// Attempts to record one compatible raster group inside a single `wgpu::RenderPass`.
    pub(super) fn try_execute_raster_materialization_group<'a>(
        &self,
        group: RenderPassMaterializationGroup,
        scope: PhaseRecordingScope,
        view: PassViewInputs<'a>,
        targets: PassRecordTargets<'a, '_, '_>,
        gpu: PassGpuInputs<'a>,
        upload_batch: &FrameUploadBatch,
    ) -> Result<bool, GraphExecuteError> {
        let steps = &self.schedule.steps[group.start_step..group.end_step];
        if steps.len() < 2 || !steps.iter().all(|step| step.phase == scope.phase) {
            return Ok(false);
        }
        let Some(first_step) = steps.first().copied() else {
            return Ok(false);
        };
        match self.cached_raster_merge(group).map(|cached| &cached.entry) {
            Some(RasterMergeCacheEntry::NotRaster) => {
                return Ok(false);
            }
            Some(RasterMergeCacheEntry::MissingTemplate { pass }) => {
                return Err(GraphExecuteError::MissingRasterTemplate { pass: pass.clone() });
            }
            // Incompatible still runs dynamic-state and `should_record` checks so skipped-pass
            // stats match the uncached path. The cached entry only skips the template clone.
            Some(RasterMergeCacheEntry::Ready { .. } | RasterMergeCacheEntry::Incompatible)
            | None => {}
        }

        let PassRecordTargets {
            frame_params,
            blackboard,
            encoder,
        } = targets;
        let uploads =
            GraphUploadSink::new(upload_batch, first_step.frame_upload_scope(scope.view_idx));
        let mut ctx = RasterPassCtx {
            device: gpu.device,
            frame: PassFrameContext::new(&mut frame_params.shared, &mut frame_params.view),
            uploads,
            graph_resources: view.graph_resources,
            blackboard,
            profiler: gpu.profiler,
        };
        let dynamic_state_matches = {
            let Some(template_refs) =
                materialization_template_refs(&self.passes, &self.pass_info, steps)?
            else {
                return Ok(false);
            };
            materialized_group_dynamic_state_matches(
                &self.passes,
                steps,
                template_refs.as_slice(),
                &ctx,
            )
        };
        if !dynamic_state_matches {
            return Ok(false);
        }
        if !self.materialized_pass_should_record(
            first_step,
            &self.passes[first_step.pass_idx],
            &ctx,
        )? {
            update_command_stats(ctx.blackboard, GraphCommandStats::record_skipped_pass);
            return Ok(false);
        }

        let sample_count = helpers::frame_sample_count_from_raster_ctx(&ctx);
        let Some((color_attachments, depth_stencil_attachment, multiview_mask)) =
            resolve_merged_attachments(
                &self.passes,
                &self.pass_info,
                &self.raster_merge_cache,
                group,
                steps,
                first_step,
                &ctx,
                view.graph_resources,
                sample_count,
            )?
        else {
            return Ok(false);
        };
        let mut label_buf = [0u8; MERGE_LABEL_STACK_CAP];
        let mut label_heap = None;
        let merged_profile_label = merge_profile_label(
            &self.passes,
            &self.raster_merge_cache,
            group,
            steps,
            &mut label_buf,
            &mut label_heap,
        );
        let pass_query = ctx
            .profiler
            .map(|p| p.begin_pass_query(merged_profile_label, encoder));
        let timestamp_writes = crate::profiling::render_pass_timestamp_writes(pass_query.as_ref());
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(merged_profile_label),
            color_attachments: color_attachments.as_slice(),
            depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes,
            multiview_mask,
        });
        for (idx, step) in steps.iter().copied().enumerate() {
            let pass = &self.passes[step.pass_idx];
            let upload_scope = step.frame_upload_scope(scope.view_idx);
            let _upload_scope = upload_batch.enter_scope(upload_scope);
            ctx.uploads = GraphUploadSink::new(upload_batch, upload_scope);
            let should_record = if idx == 0 {
                true
            } else {
                self.materialized_pass_should_record(step, pass, &ctx)?
            };
            if !should_record {
                update_command_stats(ctx.blackboard, GraphCommandStats::record_skipped_pass);
                continue;
            }
            self.record_materialized_raster_step(step, pass, &mut ctx, &mut rpass)?;
            update_command_stats(ctx.blackboard, GraphCommandStats::record_raster_pass);
        }
        update_command_stats(ctx.blackboard, GraphCommandStats::record_opened_render_pass);
        drop(rpass);
        if let (Some(p), Some(q)) = (ctx.profiler, pass_query) {
            p.end_query(encoder, q);
        }
        Ok(true)
    }

    /// Runs a materialized pass predicate with declared blackboard access validation.
    fn materialized_pass_should_record(
        &self,
        step: ScheduleStep,
        pass: &PassNode,
        ctx: &RasterPassCtx<'_, '_>,
    ) -> Result<bool, GraphExecuteError> {
        self.validate_blackboard_inputs(step.pass_idx, pass.name(), ctx.blackboard)?;
        self.begin_blackboard_access_validation(step.pass_idx, pass.name(), ctx.blackboard);
        let should_record = pass
            .should_record_raster(ctx)
            .map_err(GraphExecuteError::Pass);
        let access_result = self.finish_blackboard_access_validation(ctx.blackboard);
        let should_record = should_record?;
        access_result?;
        Ok(should_record)
    }

    /// Records one logical raster pass inside a materialized render pass.
    fn record_materialized_raster_step(
        &self,
        step: ScheduleStep,
        pass: &PassNode,
        ctx: &mut RasterPassCtx<'_, '_>,
        rpass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), GraphExecuteError> {
        self.validate_blackboard_inputs(step.pass_idx, pass.name(), ctx.blackboard)?;
        self.begin_blackboard_access_validation(step.pass_idx, pass.name(), ctx.blackboard);
        let record_result = pass
            .record_raster(ctx, rpass)
            .map_err(GraphExecuteError::Pass);
        let access_result = self.finish_blackboard_access_validation(ctx.blackboard);
        record_result?;
        access_result
    }
}

/// Borrows raster templates for a materialization candidate.
fn materialization_template_refs<'a>(
    passes: &[PassNode],
    pass_info: &'a [crate::render_graph::compiled::CompiledPassInfo],
    steps: &[ScheduleStep],
) -> Result<Option<MergeTemplateList<'a>>, GraphExecuteError> {
    if steps.is_empty()
        || !steps.iter().all(|step| {
            passes
                .get(step.pass_idx)
                .is_some_and(|pass| pass.kind() == PassKind::Raster)
        })
    {
        return Ok(None);
    }
    let first = raster_template_ref(pass_info, steps[0].pass_idx)?;
    let mut inline = [first; INLINE_MERGE_TEMPLATES];
    let mut overflow = Vec::new();
    if steps.len() <= INLINE_MERGE_TEMPLATES {
        for (offset, step) in steps.iter().enumerate().skip(1) {
            inline[offset] = raster_template_ref(pass_info, step.pass_idx)?;
        }
        return Ok(Some(MergeTemplateList {
            inline,
            len: steps.len(),
            overflow,
        }));
    }
    overflow.reserve(steps.len());
    for step in steps {
        overflow.push(raster_template_ref(pass_info, step.pass_idx)?);
    }
    Ok(Some(MergeTemplateList {
        inline,
        len: 1,
        overflow,
    }))
}

fn raster_template_ref(
    pass_info: &[crate::render_graph::compiled::CompiledPassInfo],
    pass_idx: usize,
) -> Result<&RenderPassTemplate, GraphExecuteError> {
    pass_info
        .get(pass_idx)
        .and_then(|info| info.raster_template.as_ref())
        .ok_or_else(|| missing_raster_template(pass_info, pass_idx))
}

fn missing_raster_template(
    pass_info: &[crate::render_graph::compiled::CompiledPassInfo],
    pass_idx: usize,
) -> GraphExecuteError {
    let pass = pass_info
        .get(pass_idx)
        .map_or_else(|| format!("pass#{pass_idx}"), |info| info.name.clone());
    GraphExecuteError::MissingRasterTemplate { pass }
}

/// Returns whether dynamic multiview and stencil state match across a merge candidate.
fn materialized_group_dynamic_state_matches(
    passes: &[PassNode],
    steps: &[ScheduleStep],
    templates: &[&RenderPassTemplate],
    ctx: &RasterPassCtx<'_, '_>,
) -> bool {
    let Some((first_step, rest_steps)) = steps.split_first() else {
        return false;
    };
    let Some((first_template, rest_templates)) = templates.split_first() else {
        return false;
    };
    let first_pass = &passes[first_step.pass_idx];
    let multiview = first_pass.multiview_mask_override(ctx, first_template);
    let stencil = first_template
        .depth_stencil_attachment
        .as_ref()
        .and_then(|depth| first_pass.stencil_ops_override(ctx, depth));
    rest_steps
        .iter()
        .zip(rest_templates)
        .all(|(step, template)| {
            let pass = &passes[step.pass_idx];
            pass.multiview_mask_override(ctx, template) == multiview
                && template
                    .depth_stencil_attachment
                    .as_ref()
                    .and_then(|depth| pass.stencil_ops_override(ctx, depth))
                    == stencil
        })
}

fn resolve_merged_attachments<'a>(
    passes: &[PassNode],
    pass_info: &[crate::render_graph::compiled::CompiledPassInfo],
    cache: &[CachedRasterMergeGroup],
    group: RenderPassMaterializationGroup,
    steps: &[ScheduleStep],
    first_step: ScheduleStep,
    ctx: &RasterPassCtx<'_, '_>,
    graph_resources: &'a crate::render_graph::context::GraphResolvedResources,
    sample_count: u32,
) -> Result<
    Option<(
        helpers::ResolvedColorAttachments<'a>,
        Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
        Option<std::num::NonZeroU32>,
    )>,
    GraphExecuteError,
> {
    let cached = cache
        .iter()
        .find(|cached| cached.start_step == group.start_step && cached.end_step == group.end_step);
    let owned = match cached.map(|cached| &cached.entry) {
        Some(RasterMergeCacheEntry::Ready { .. }) => None,
        Some(RasterMergeCacheEntry::Incompatible | RasterMergeCacheEntry::NotRaster) => {
            return Ok(None);
        }
        Some(RasterMergeCacheEntry::MissingTemplate { pass }) => {
            return Err(GraphExecuteError::MissingRasterTemplate { pass: pass.clone() });
        }
        None => {
            let Some(template_refs) = materialization_template_refs(passes, pass_info, steps)?
            else {
                return Ok(None);
            };
            let Some(template) = helpers::coalesce_render_pass_template(template_refs.as_slice())
            else {
                return Ok(None);
            };
            Some(template)
        }
    };
    let merged_template = if let Some(template) = owned.as_ref() {
        template
    } else if let Some(RasterMergeCacheEntry::Ready { template, .. }) =
        cached.map(|cached| &cached.entry)
    {
        template
    } else {
        return Ok(None);
    };
    let first_pass = &passes[first_step.pass_idx];
    let color_attachments = helpers::resolve_color_attachments(
        "render-graph-raster-merged",
        merged_template,
        graph_resources,
        sample_count,
    )?;
    let stencil_ops = merged_template
        .depth_stencil_attachment
        .as_ref()
        .and_then(|depth| first_pass.stencil_ops_override(ctx, depth));
    let depth_stencil_attachment = helpers::resolve_depth_attachment_with_stencil(
        "render-graph-raster-merged",
        merged_template,
        graph_resources,
        sample_count,
        stencil_ops,
    )?;
    let multiview_mask = first_pass.multiview_mask_override(ctx, merged_template);
    Ok(Some((
        color_attachments,
        depth_stencil_attachment,
        multiview_mask,
    )))
}

fn merge_profile_label<'a>(
    passes: &[PassNode],
    cache: &[CachedRasterMergeGroup],
    group: RenderPassMaterializationGroup,
    steps: &[ScheduleStep],
    buf: &'a mut [u8; MERGE_LABEL_STACK_CAP],
    heap: &'a mut Option<String>,
) -> &'a str {
    let cached = cache
        .iter()
        .find(|cached| cached.start_step == group.start_step && cached.end_step == group.end_step);
    let owned =
        if let Some(RasterMergeCacheEntry::Ready { .. }) = cached.map(|cached| &cached.entry) {
            None
        } else {
            Some(materialized_group_profile_label(passes, steps))
        };
    let source = if let Some(label) = owned.as_deref() {
        label
    } else if let Some(RasterMergeCacheEntry::Ready { label, .. }) =
        cached.map(|cached| &cached.entry)
    {
        label.as_str()
    } else {
        "graph::raster_merge"
    };
    if source.len() <= buf.len() {
        buf[..source.len()].copy_from_slice(source.as_bytes());
        return std::str::from_utf8(&buf[..source.len()]).unwrap_or("graph::raster_merge");
    }
    *heap = Some(source.to_owned());
    heap.as_deref().unwrap_or("graph::raster_merge")
}

fn materialized_group_profile_label(passes: &[PassNode], steps: &[ScheduleStep]) -> String {
    let mut label = String::from("graph::raster_merge(");
    for (idx, step) in steps.iter().enumerate() {
        if idx != 0 {
            label.push_str(" + ");
        }
        label.push_str(passes[step.pass_idx].profiling_label().as_ref());
    }
    label.push(')');
    label
}
