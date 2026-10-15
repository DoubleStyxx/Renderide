//! Mutable builders for world-mesh instance plans.

use crate::materials::ShaderPermutation;
use crate::render_phase::RenderPhaseSet;
use crate::world_mesh::draw_prep::WorldMeshDrawItem;

use super::batch_window::{BatchWindow, build_group};
use super::prepass::{depth_prepass_group_eligible, phase_is_pre_skybox_forward};
use super::scratch::InstancePlanScratch;
use super::{
    DrawGroup, InstancePlan, MergedGroupedWindowGroup, PendingSubmissionGroup, WorldMeshPhase,
    debug_assert_plan_group_order,
};

/// Mutable output and scratch buffers used while building one [`InstancePlan`].
pub(super) struct InstancePlanBuilder {
    /// Per-draw slab order emitted for the frame.
    slab_layout: Vec<usize>,
    /// Named phase queues emitted for the frame.
    phases: RenderPhaseSet<WorldMeshPhase, DrawGroup>,
    /// Shader permutation used to decide phase mirrors that depend on material pass metadata.
    shader_perm: ShaderPermutation,
    /// Reusable grouping scratch for one batch-key window.
    scratch: InstancePlanScratch,
}

impl InstancePlanBuilder {
    /// Creates a builder sized for `draw_count` sorted draws.
    pub(super) fn with_capacity(draw_count: usize, shader_perm: ShaderPermutation) -> Self {
        Self {
            slab_layout: Vec::with_capacity(draw_count),
            phases: RenderPhaseSet::new(),
            shader_perm,
            scratch: InstancePlanScratch::default(),
        }
    }

    /// Emits all groups for one same-batch-key window.
    pub(super) fn process_window(&mut self, draws: &[WorldMeshDrawItem], window: BatchWindow) {
        if window.singleton {
            self.emit_singletons(draws, window);
        } else {
            self.emit_grouped_window(draws, window);
        }
    }

    /// Emits pre-merged groups for a large same-batch-key window.
    pub(super) fn emit_merged_grouped_window(
        &mut self,
        draws: &[WorldMeshDrawItem],
        phase: WorldMeshPhase,
        groups: Vec<MergedGroupedWindowGroup>,
    ) {
        for merged in groups {
            let group = build_group(
                &mut self.slab_layout,
                merged.representative_draw_idx,
                &merged.members,
            );
            self.queue_group_to_phase(draws, phase, group);
        }
    }

    /// Emits groups already merged by resolved material submission compatibility.
    pub(super) fn emit_submission_groups(
        &mut self,
        draws: &[WorldMeshDrawItem],
        groups: Vec<PendingSubmissionGroup>,
    ) {
        for pending in groups {
            let group = build_group(
                &mut self.slab_layout,
                pending.representative_draw_idx,
                &pending.members,
            );
            self.queue_group_to_phase(draws, pending.phase, group);
        }
    }

    /// Produces the final plan after debug-validating group order.
    pub(super) fn finish(self) -> InstancePlan {
        let plan = InstancePlan {
            slab_layout: self.slab_layout,
            phases: self.phases,
        };
        debug_assert_plan_group_order(&plan);
        plan
    }

    fn emit_singletons(&mut self, draws: &[WorldMeshDrawItem], window: BatchWindow) {
        for draw_idx in window.range {
            let group = build_group(&mut self.slab_layout, draw_idx, &[draw_idx]);
            self.queue_group_to_phase(draws, window.phase, group);
        }
    }

    fn emit_grouped_window(&mut self, draws: &[WorldMeshDrawItem], window: BatchWindow) {
        self.scratch.rebuild(draws, window.range.clone());
        for group_idx in 0..self.scratch.group_count() {
            let group = {
                let members = self.scratch.group_members(group_idx);
                let representative = self.scratch.group_representative(group_idx);
                build_group(&mut self.slab_layout, representative, members)
            };
            self.queue_group_to_phase(draws, window.phase, group);
        }
    }

    fn queue_group_to_phase(
        &mut self,
        draws: &[WorldMeshDrawItem],
        phase: WorldMeshPhase,
        group: DrawGroup,
    ) {
        self.phases.phase_mut(phase).push(group.clone());
        if phase_is_pre_skybox_forward(phase) {
            self.phases
                .phase_mut(WorldMeshPhase::ViewNormals)
                .push(group.clone());
            if depth_prepass_group_eligible(draws, &self.slab_layout, &group, self.shader_perm) {
                self.phases.phase_mut(WorldMeshPhase::DepthOnly).push(group);
            }
        }
    }
}
