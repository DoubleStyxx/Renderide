//! Lazy-built compute pipelines for SH2 projection.

use std::sync::Arc;

use hashbrown::HashSet;

use super::super::projection_pipeline::{
    ProjectionPipeline, ProjectionPipelineBuildOutcome, ProjectionPipelineKind,
    spawn_projection_pipeline_build,
};

/// Cache of SH2 projection compute pipelines, plus async build bookkeeping.
pub(super) struct ProjectionPipelineCache {
    cubemap: Option<ProjectionPipeline>,
    build_tx: crossbeam_channel::Sender<ProjectionPipelineBuildOutcome>,
    build_rx: crossbeam_channel::Receiver<ProjectionPipelineBuildOutcome>,
    pending_builds: HashSet<ProjectionPipelineKind>,
    failed_builds: HashSet<ProjectionPipelineKind>,
}

impl ProjectionPipelineCache {
    /// Creates an empty pipeline cache with its build-completion channel.
    pub(super) fn new() -> Self {
        let (build_tx, build_rx) = crossbeam_channel::unbounded();
        Self {
            cubemap: None,
            build_tx,
            build_rx,
            pending_builds: HashSet::new(),
            failed_builds: HashSet::new(),
        }
    }

    /// Returns the requested pipeline if it has finished building.
    pub(super) fn get(&self, kind: ProjectionPipelineKind) -> Option<&ProjectionPipeline> {
        match kind {
            ProjectionPipelineKind::Cubemap => self.cubemap.as_ref(),
        }
    }

    /// Drains completed background builds and installs their pipelines.
    pub(super) fn drain_completed_builds(&mut self) {
        while let Ok(outcome) = self.build_rx.try_recv() {
            self.pending_builds.remove(&outcome.kind);
            match outcome.result {
                Ok(pipeline) => {
                    *self.slot_mut(outcome.kind) = Some(pipeline);
                }
                Err(e) => {
                    logger::warn!(
                        "reflection_probe_sh2: projection pipeline build failed for {}: {e}",
                        outcome.kind.stem()
                    );
                    self.failed_builds.insert(outcome.kind);
                }
            }
        }
    }

    /// Returns `Ok(true)` when the pipeline is ready, `Ok(false)` while a build is in flight,
    /// or `Err` when a prior build attempt failed terminally.
    pub(super) fn ensure_ready(
        &mut self,
        device: &Arc<wgpu::Device>,
        kind: ProjectionPipelineKind,
    ) -> Result<bool, String> {
        if self.get(kind).is_some() {
            return Ok(true);
        }
        if self.failed_builds.contains(&kind) {
            return Err(format!(
                "projection pipeline {} previously failed to build",
                kind.stem()
            ));
        }
        if self.pending_builds.contains(&kind) {
            return Ok(false);
        }
        spawn_projection_pipeline_build(kind, device.clone(), self.build_tx.clone())?;
        self.pending_builds.insert(kind);
        Ok(false)
    }

    /// Kicks off background builds for every projection pipeline.
    pub(super) fn pre_warm(&mut self, device: &Arc<wgpu::Device>) {
        for kind in ProjectionPipelineKind::ALL {
            if let Err(e) = self.ensure_ready(device, kind) {
                logger::warn!(
                    "reflection_probe_sh2: projection pipeline pre-warm failed for {}: {e}",
                    kind.stem()
                );
            }
        }
    }

    fn slot_mut(&mut self, kind: ProjectionPipelineKind) -> &mut Option<ProjectionPipeline> {
        match kind {
            ProjectionPipelineKind::Cubemap => &mut self.cubemap,
        }
    }
}
