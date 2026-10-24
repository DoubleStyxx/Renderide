//! Hierarchical depth (Hi-Z) occlusion culling subsystem.
//!
//! Owns GPU pyramid state per logical view ([`ViewId`]), CPU readback snapshots, and
//! temporal view/projection data used by [`crate::passes::WorldMeshForwardOpaquePass`] and
//! [`crate::passes::HiZBuildPass`].

use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::camera::ViewId;
use crate::cull_contract::HiZTemporalState;
use crate::gpu::OutputDepthMode;
use crate::hi_z_cpu::HiZCullData;
use crate::history_texture::HistoryTextureMipViews;
use crate::occlusion::gpu::{HiZBuildRecord, HiZGpuState, HiZHistoryTarget, encode_hi_z_build};
use crate::occlusion::hysteresis::{
    CasterKey, CasterRecord, ViewHysteresisPlan, VisibilityCullBuffers, VisibilityFrameScratch,
    VisibilityHistory, VisibilityRecycle, VisibilityScratch,
};

/// Depth source, layout, and logical view for [`OcclusionSystem::encode_hi_z_build_pass`].
pub(crate) struct HiZBuildInput<'a> {
    /// Depth attachment view (desktop 2D or multiview array) sampled for mip0.
    pub depth_view: &'a wgpu::TextureView,
    /// Registry-owned ping-pong history texture that receives the pyramid.
    pub history_texture: &'a wgpu::Texture,
    /// Registry-owned per-layer/per-mip views for [`Self::history_texture`].
    pub history_mip_views: &'a HistoryTextureMipViews,
    /// Full framebuffer extent in pixels (matches the depth attachment).
    pub extent: (u32, u32),
    /// Desktop single-view vs stereo depth array layout.
    pub mode: OutputDepthMode,
}

/// GPU pyramid, CPU readback ring, and temporal cull snapshots for Hi-Z occlusion.
pub struct OcclusionSystem {
    /// Main window / OpenXR multiview Hi-Z (desktop and stereo layouts).
    main: Arc<Mutex<HiZGpuState>>,
    /// Per logical secondary-camera view pyramids (single-view desktop layout each).
    offscreen: Mutex<HashMap<ViewId, Arc<Mutex<HiZGpuState>>>>,
    /// Per-view classification from the previous tested frame, plus a spare map.
    visibility: Mutex<HashMap<ViewId, VisibilitySlot>>,
    /// Plan list reused across frames. A nested call uses a fresh list instead of waiting.
    hysteresis_plans: Mutex<Vec<Option<ViewHysteresisPlan>>>,
}

/// Published history, the spare map, and the pooled cull buffers for one view.
struct VisibilitySlot {
    published: VisibilityHistory,
    spare: HashMap<CasterKey, CasterRecord>,
    /// Previous frame skipped history because the prepared caster count was over the cap.
    suspended: bool,
    /// Serial-plan buffers returned by [`OcclusionSystem::restore_frame_scratch`].
    frame_scratch: VisibilityFrameScratch,
    /// Motions, disocclusion rectangles, and shard vectors drained by recycle.
    cull_buffers: VisibilityCullBuffers,
}

fn empty_visibility_slot() -> VisibilitySlot {
    VisibilitySlot {
        published: VisibilityHistory::default(),
        spare: HashMap::new(),
        suspended: false,
        frame_scratch: VisibilityFrameScratch::default(),
        cull_buffers: VisibilityCullBuffers::default(),
    }
}

impl Default for OcclusionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl OcclusionSystem {
    /// Creates an empty occlusion system with no pyramid data.
    pub fn new() -> Self {
        Self {
            main: Arc::new(Mutex::new(HiZGpuState::default())),
            offscreen: Mutex::new(HashMap::new()),
            visibility: Mutex::new(HashMap::new()),
            hysteresis_plans: Mutex::new(Vec::new()),
        }
    }

    /// Lends the reused per-frame plan list.
    ///
    /// The list is cleared by the caller at the start of the next frame. A nested call, or a
    /// second caller that already holds the list, gets a fresh vector so this does not panic or
    /// deadlock.
    pub(crate) fn with_hysteresis_plans<R>(
        &self,
        use_plans: impl FnOnce(&mut Vec<Option<ViewHysteresisPlan>>) -> R,
    ) -> R {
        if let Some(mut plans) = self.hysteresis_plans.try_lock() {
            return use_plans(&mut plans);
        }
        let mut plans = Vec::new();
        use_plans(&mut plans)
    }

    /// Returns the mutex-wrapped Hi-Z slot for `view`, creating it when needed.
    pub(crate) fn ensure_hi_z_state(&self, view: ViewId) -> Arc<Mutex<HiZGpuState>> {
        match view {
            ViewId::Main => self.main.clone(),
            ViewId::MainOverlay
            | ViewId::SecondaryCamera(_)
            | ViewId::CameraPortal(_)
            | ViewId::CameraRenderTask(_)
            | ViewId::Camera360RenderTaskFace(_)
            | ViewId::ReflectionProbeRenderTask(_) => {
                let mut offscreen = self.offscreen.lock();
                offscreen
                    .entry(view)
                    .or_insert_with(|| Arc::new(Mutex::new(HiZGpuState::default())))
                    .clone()
            }
        }
    }

    /// Returns the existing mutex-wrapped Hi-Z slot for `view` without creating one.
    fn hi_z_state_slot(&self, view: ViewId) -> Option<Arc<Mutex<HiZGpuState>>> {
        match view {
            ViewId::Main => Some(self.main.clone()),
            ViewId::MainOverlay
            | ViewId::SecondaryCamera(_)
            | ViewId::CameraPortal(_)
            | ViewId::CameraRenderTask(_)
            | ViewId::Camera360RenderTaskFace(_)
            | ViewId::ReflectionProbeRenderTask(_) => self.offscreen.lock().get(&view).cloned(),
        }
    }

    /// Hi-Z occlusion data cloned from the **previous** frame's pyramid readback, matching `mode`.
    pub(crate) fn hi_z_cull_data(
        &self,
        mode: OutputDepthMode,
        view: ViewId,
    ) -> Option<HiZCullData> {
        let slot = self.hi_z_state_slot(view)?;
        let state = slot.lock();
        match view {
            ViewId::Main => match mode {
                OutputDepthMode::DesktopSingle => state
                    .desktop
                    .as_ref()
                    .map(|s| HiZCullData::Desktop(s.clone())),
                OutputDepthMode::StereoArray { .. } => {
                    state.stereo.as_ref().map(|s| HiZCullData::Stereo {
                        left: s.left.clone(),
                        right: s.right.clone(),
                    })
                }
            },
            ViewId::MainOverlay
            | ViewId::SecondaryCamera(_)
            | ViewId::CameraPortal(_)
            | ViewId::CameraRenderTask(_)
            | ViewId::Camera360RenderTaskFace(_)
            | ViewId::ReflectionProbeRenderTask(_) => state
                .desktop
                .as_ref()
                .map(|s| HiZCullData::Desktop(s.clone())),
        }
    }

    /// Retires all Hi-Z state owned by `view`.
    ///
    /// Main-view state persists for the life of the renderer; secondary views are removed when
    /// the view graph changes.
    pub(crate) fn retire_view(&self, view: ViewId) -> bool {
        let removed = match view {
            ViewId::Main => false,
            ViewId::MainOverlay
            | ViewId::SecondaryCamera(_)
            | ViewId::CameraPortal(_)
            | ViewId::CameraRenderTask(_)
            | ViewId::Camera360RenderTaskFace(_)
            | ViewId::ReflectionProbeRenderTask(_) => self.offscreen.lock().remove(&view).is_some(),
        };
        self.visibility.lock().remove(&view);
        removed
    }

    /// Takes the published classification, spare map, and pooled buffers for `view`.
    ///
    /// Missing history is an empty snapshot: every caster is treated as newly revealed.
    /// [`Self::store_visibility_history`] puts the history, spare map, and cull buffers back.
    /// [`Self::restore_frame_scratch`] puts the serial-plan buffers back before the cull, because
    /// those buffers are not needed while the parallel cull reads the plan.
    pub(crate) fn begin_visibility(&self, view: ViewId) -> VisibilityScratch {
        let mut slots = self.visibility.lock();
        let slot = slots.entry(view).or_insert_with(empty_visibility_slot);
        let scratch = VisibilityScratch {
            history: std::mem::take(&mut slot.published),
            spare: std::mem::take(&mut slot.spare),
            suspended: slot.suspended,
            frame_scratch: std::mem::take(&mut slot.frame_scratch),
            cull_buffers: std::mem::take(&mut slot.cull_buffers),
        };
        drop(slots);
        scratch
    }

    /// Returns the serial-plan buffers taken by [`Self::begin_visibility`].
    ///
    /// The view's slot already exists because begin created it. Retiring the view drops the slot,
    /// including these buffers.
    pub(crate) fn restore_frame_scratch(
        &self,
        view: ViewId,
        frame_scratch: VisibilityFrameScratch,
    ) {
        let mut slots = self.visibility.lock();
        if let Some(slot) = slots.get_mut(&view) {
            slot.frame_scratch = frame_scratch;
        }
        drop(slots);
    }

    /// Stores the classification and pooled buffers produced by this frame's fine cull.
    ///
    /// `suspended` records that this frame did not attach history confirmation, so the next frame
    /// stays off until the prepared count falls through the resume band. The spare map and the
    /// cull buffers are the ones [`VisibilityRecycle`] returned, so their allocations stay on the
    /// view.
    pub(crate) fn store_visibility_history(
        &self,
        view: ViewId,
        suspended: bool,
        recycled: VisibilityRecycle,
    ) {
        let mut slots = self.visibility.lock();
        let slot = slots.entry(view).or_insert_with(empty_visibility_slot);
        slot.suspended = suspended;
        slot.published = recycled.history;
        slot.spare = recycled.spare;
        slot.cull_buffers = recycled.buffers;
        drop(slots);
    }

    /// Number of live secondary-view Hi-Z slots.
    #[cfg(test)]
    pub(crate) fn secondary_view_count(&self) -> usize {
        self.offscreen.lock().len()
    }

    /// Records Hi-Z GPU work into `encoder` (staging copy included).
    pub(crate) fn encode_hi_z_build_pass(
        &self,
        record: HiZBuildRecord<'_>,
        state_slot: &Mutex<HiZGpuState>,
        input: HiZBuildInput<'_>,
        profiler: Option<&crate::profiling::GpuProfilerHandle>,
    ) {
        profiling::scope!("hi_z::build");
        let mut state = state_slot.lock();
        encode_hi_z_build(
            record,
            input.depth_view,
            HiZHistoryTarget {
                texture: input.history_texture,
                mip_views: input.history_mip_views,
            },
            input.extent,
            input.mode,
            &mut state,
            profiler,
        );
    }

    /// Drains completed Hi-Z `map_async` readbacks into CPU snapshots for [`Self::hi_z_cull_data`]
    /// and promotes any submit-done readback slots into fresh `map_async` requests on the main thread.
    ///
    /// Non-blocking: uses bounded [`wgpu::Device::poll`] calls; if a read is not ready, prior
    /// snapshots are kept. The second poll gives newly-started maps a chance to complete in the
    /// same frame-start drain, avoiding an extra tick of CPU Hi-Z latency after the GPU-completion
    /// callback has already fired.
    ///
    /// The poll runs **before** any [`HiZGpuState`] lock so the
    /// [`wgpu::Queue::on_submitted_work_done`] callback installed by
    /// [`crate::render_graph::compiled::exec::CompiledRenderGraph::execute_multi_view`]
    /// (which itself locks the per-view [`HiZGpuState`]) can execute without re-entering
    /// a lock held by this function. That callback only marks the encoded readback slot as
    /// submit-done; the actual `map_async` runs here via
    /// [`crate::occlusion::gpu::HiZGpuState::start_ready_maps`], so no
    /// wgpu call is issued from inside the device-poll callback (which would risk deadlocks
    /// with wgpu's internal queue-write locks -- observed as a futex hang inside
    /// `queue.write_texture` during asset upload).
    pub fn hi_z_begin_frame_readback(&self, device: &wgpu::Device) {
        profiling::scope!("hi_z::readback_drain");
        let _ = device.poll(wgpu::PollType::Poll);
        self.drain_completed_hi_z_maps();
        self.start_ready_hi_z_maps();
        let _ = device.poll(wgpu::PollType::Poll);
        self.drain_completed_hi_z_maps();
    }

    /// Cancels all pending Hi-Z staging maps after GPU recovery.
    pub(crate) fn clear_pending_hi_z_readbacks(&self) {
        {
            let mut main = self.main.lock();
            main.clear_pending();
        };
        let offscreen = self.offscreen.lock();
        for slot in offscreen.values() {
            let mut state = slot.lock();
            state.clear_pending();
        }
    }

    fn drain_completed_hi_z_maps(&self) {
        {
            let mut main = self.main.lock();
            main.drain_completed_map_async();
        };
        let offscreen = self.offscreen.lock();
        for slot in offscreen.values() {
            let mut state = slot.lock();
            state.drain_completed_map_async();
        }
    }

    fn start_ready_hi_z_maps(&self) {
        {
            let mut main = self.main.lock();
            main.start_ready_maps();
        };
        let offscreen = self.offscreen.lock();
        for slot in offscreen.values() {
            let mut state = slot.lock();
            state.start_ready_maps();
        }
    }

    /// View/projection snapshot from the **previous** world forward pass (for Hi-Z occlusion tests).
    pub(crate) fn hi_z_temporal_snapshot(&self, view: ViewId) -> Option<HiZTemporalState> {
        self.hi_z_state_slot(view)?.lock().temporal.clone()
    }

    /// Stores a prepared temporal snapshot for next-frame Hi-Z occlusion tests.
    pub(crate) fn store_hi_z_temporal_for_next_frame(
        &self,
        temporal: HiZTemporalState,
        state_slot: &Mutex<HiZGpuState>,
    ) {
        let mut state = state_slot.lock();
        state.temporal = Some(temporal);
    }
}

impl crate::occlusion::OcclusionGraphHook for OcclusionSystem {
    fn ensure_hi_z_state(&self, view: ViewId) -> Arc<Mutex<HiZGpuState>> {
        OcclusionSystem::ensure_hi_z_state(self, view)
    }

    fn encode_hi_z_build_pass(
        &self,
        record: HiZBuildRecord<'_>,
        state_slot: &Mutex<HiZGpuState>,
        input: HiZBuildInput<'_>,
        profiler: Option<&crate::profiling::GpuProfilerHandle>,
    ) {
        OcclusionSystem::encode_hi_z_build_pass(self, record, state_slot, input, profiler);
    }

    fn store_hi_z_temporal_for_next_frame(
        &self,
        temporal: HiZTemporalState,
        state_slot: &Mutex<HiZGpuState>,
    ) {
        OcclusionSystem::store_hi_z_temporal_for_next_frame(self, temporal, state_slot);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::OcclusionSystem;
    use crate::camera::ViewId;
    use crate::scene::RenderSpaceId;

    /// Builds a secondary-camera view id for occlusion tests.
    fn secondary_view(render_space_id: i32, renderable_index: i32) -> ViewId {
        ViewId::secondary_camera(RenderSpaceId(render_space_id), renderable_index)
    }

    #[test]
    fn ensure_hi_z_state_reuses_slots_per_view() {
        let system = OcclusionSystem::new();
        let main_a = system.ensure_hi_z_state(ViewId::Main);
        let main_b = system.ensure_hi_z_state(ViewId::Main);
        let offscreen_a = system.ensure_hi_z_state(secondary_view(17, 0));
        let offscreen_b = system.ensure_hi_z_state(secondary_view(17, 0));

        assert!(Arc::ptr_eq(&main_a, &main_b));
        assert!(Arc::ptr_eq(&offscreen_a, &offscreen_b));
        assert!(!Arc::ptr_eq(&main_a, &offscreen_a));
    }

    #[test]
    fn ensure_hi_z_state_is_thread_safe_for_shared_view() {
        let system = Arc::new(OcclusionSystem::new());

        let first = (0..8)
            .map(|_| {
                let system = Arc::clone(&system);
                std::thread::spawn(move || system.ensure_hi_z_state(secondary_view(99, 0)))
            })
            .map(|thread| thread.join().expect("thread should finish"))
            .reduce(|first, next| {
                assert!(Arc::ptr_eq(&first, &next));
                first
            })
            .expect("at least one slot");

        let again = system.ensure_hi_z_state(secondary_view(99, 0));
        assert!(Arc::ptr_eq(&first, &again));
    }

    /// Retiring one secondary view preserves other secondary view Hi-Z slots.
    #[test]
    fn retire_view_removes_only_target_slot() {
        let system = OcclusionSystem::new();
        let retired = secondary_view(99, 0);
        let surviving = secondary_view(99, 1);
        let _ = system.ensure_hi_z_state(retired);
        let _ = system.ensure_hi_z_state(surviving);

        assert_eq!(system.secondary_view_count(), 2);
        assert!(system.retire_view(retired));
        assert_eq!(system.secondary_view_count(), 1);
        assert!(system.hi_z_state_slot(retired).is_none());
        assert!(system.hi_z_state_slot(surviving).is_some());
    }
}
