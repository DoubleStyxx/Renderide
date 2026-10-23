//! Fingerprint of the inputs that decide a shadow frame plan.
//!
//! Shadow caster collection is not camera-culled, so a static world presents the same casters and
//! lights every submission. Rebuilding per-face visibility and re-recording every atlas layer in
//! that case is the steady-state cost behind multi-light frame-time drops.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::backend::HostShadowQuality;
use crate::camera::ViewId;
use crate::gpu::{GpuLight, GpuLimits};
use crate::materials::{RasterFrontFace, RasterPipelineKind, RasterPrimitiveTopology};
use crate::shared::ShadowCastMode;
use crate::world_mesh::{WorldMeshDrawItem, WorldMeshDrawPlan};

use super::super::manager::FrameResourceManager;
use super::shadow_draw_casts;

/// Retained shadow-plan identity and the light assignments that go with it.
#[derive(Debug)]
pub(in crate::backend::frame_resource_manager) struct ShadowPlanCache {
    fingerprint: u64,
    rendered_fingerprint: AtomicU64,
    /// Atlas submits for the prepare that is in progress. Reset at the start of each prepare.
    atlas_passes_this_frame: AtomicU32,
    /// Successful shadow-atlas submits since the manager was created.
    atlas_passes_total: AtomicU64,
    assignments: Vec<CachedShadowAssignment>,
    /// Reused buffer for restoring light assignments without a per-frame allocation.
    assignment_scratch: Vec<CachedShadowAssignment>,
}

impl Default for ShadowPlanCache {
    fn default() -> Self {
        Self {
            fingerprint: 0,
            rendered_fingerprint: AtomicU64::new(0),
            atlas_passes_this_frame: AtomicU32::new(0),
            atlas_passes_total: AtomicU64::new(0),
            assignments: Vec::new(),
            assignment_scratch: Vec::new(),
        }
    }
}

impl ShadowPlanCache {
    pub(super) fn invalidate(&mut self) {
        self.fingerprint = 0;
        self.assignments.clear();
    }

    pub(super) fn remember(&mut self, fingerprint: u64, assignments: Vec<CachedShadowAssignment>) {
        self.fingerprint = fingerprint;
        self.assignments = assignments;
    }

    pub(super) fn matches(&self, fingerprint: u64) -> bool {
        fingerprint != 0 && self.fingerprint == fingerprint
    }

    pub(super) fn maps_need_render(&self, fingerprint: u64, casters_need_redraw: bool) -> bool {
        casters_need_redraw || self.rendered_fingerprint.load(Ordering::Relaxed) != fingerprint
    }

    pub(super) fn note_rendered(&self, fingerprint: u64) {
        self.rendered_fingerprint
            .store(fingerprint, Ordering::Relaxed);
    }

    /// Records that the retained plan's maps were drawn for the current fingerprint.
    pub(super) fn note_current_rendered(&self) {
        let fingerprint = self.fingerprint;
        if fingerprint != 0 {
            self.note_rendered(fingerprint);
        }
    }

    /// Clears the per-frame atlas-pass count at the start of shadow planning.
    pub(super) fn begin_atlas_pass_frame(&self) {
        self.atlas_passes_this_frame.store(0, Ordering::Relaxed);
    }

    /// Counts one submitted shadow atlas. One logical pass, not one per layer.
    pub(super) fn record_atlas_pass(&self) {
        self.atlas_passes_this_frame.store(1, Ordering::Relaxed);
        self.atlas_passes_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Per-frame atlas submits, then the process total.
    pub(super) fn atlas_pass_counts(&self) -> (u32, u64) {
        (
            self.atlas_passes_this_frame.load(Ordering::Relaxed),
            self.atlas_passes_total.load(Ordering::Relaxed),
        )
    }

    /// Forgets a successful draw so the next submission re-renders the atlas.
    pub(in crate::backend::frame_resource_manager) fn clear_rendered(&self) {
        self.rendered_fingerprint.store(0, Ordering::Relaxed);
    }

    /// Copies the retained assignments into the reused scratch buffer.
    ///
    /// The caller must [`Self::return_assignment_scratch`] so the next hit does not allocate.
    pub(super) fn take_assignment_copy(&mut self) -> Vec<CachedShadowAssignment> {
        let mut scratch = std::mem::take(&mut self.assignment_scratch);
        scratch.clear();
        scratch.extend_from_slice(&self.assignments);
        scratch
    }

    pub(super) fn return_assignment_scratch(&mut self, scratch: Vec<CachedShadowAssignment>) {
        self.assignment_scratch = scratch;
    }
}

/// Mesh upload generation visible to shadow planning.
///
/// Production passes the asset-transfer queue. Depth-only shadow shaders do not sample the
/// material property store, so that store is not a shadow-plan input. Tests and the
/// zero-revision planner bench pass [`ZeroShadowContentRevisions`].
pub(crate) trait ShadowContentRevisions {
    /// Generation of the resident mesh buffers for `mesh_asset_id`.
    fn mesh_upload_generation(&self, mesh_asset_id: i32) -> u64;
}

/// Revisions that never change. Used when the caller has no upload queue.
#[cfg(any(test, feature = "bench-api"))]
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::backend) struct ZeroShadowContentRevisions;

#[cfg(any(test, feature = "bench-api"))]
impl ShadowContentRevisions for ZeroShadowContentRevisions {
    fn mesh_upload_generation(&self, _mesh_asset_id: i32) -> u64 {
        0
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedShadowAssignment {
    pub(super) view_id: ViewId,
    pub(super) light_index: usize,
    pub(super) start: u32,
    pub(super) count: u32,
}

/// Shadow inputs that must match before a previous plan can be retained.
pub(super) struct ShadowInputSignature {
    pub(super) fingerprint: u64,
    pub(super) casters_need_redraw: bool,
}

pub(super) fn shadow_input_signature<'a, I, R>(
    manager: &FrameResourceManager,
    quality: HostShadowQuality,
    views: I,
    revisions: &R,
) -> ShadowInputSignature
where
    I: IntoIterator<Item = (ViewId, &'a WorldMeshDrawPlan)>,
    R: ShadowContentRevisions,
{
    profiling::scope!("render::prepare_shadow_frame::fingerprint");
    let mut fp = Fingerprint::new();
    hash_quality(&mut fp, quality);
    hash_limits(&mut fp, manager.limits.as_deref());
    let mut casters_need_redraw = false;
    for (view_id, draw_plan) in views {
        view_id.hash(&mut fp);
        let Some(collection) = draw_plan.as_prefetched() else {
            fp.mix_u64(0);
            continue;
        };
        let mut draws = DrawFingerprint::default();
        for item in &collection.items {
            if !shadow_draw_casts(item) {
                continue;
            }
            casters_need_redraw |= caster_needs_redraw(item);
            draws.mix(hash_caster(item, revisions));
        }
        draws.write(&mut fp);
        hash_lights(&mut fp, view_lights(manager, view_id));
    }
    ShadowInputSignature {
        fingerprint: fp.into_u64(),
        casters_need_redraw,
    }
}

pub(super) fn snapshot_shadow_assignments(
    manager: &FrameResourceManager,
    views: &[ViewId],
) -> Vec<CachedShadowAssignment> {
    let mut assignments = Vec::new();
    for view_id in views {
        let Some(lights) = manager.per_view_lights.get(*view_id) else {
            continue;
        };
        for (light_index, light) in lights.lights.iter().enumerate() {
            assignments.push(CachedShadowAssignment {
                view_id: *view_id,
                light_index,
                start: light.shadow_view_start,
                count: light.shadow_view_count,
            });
        }
    }
    assignments
}

fn view_lights(manager: &FrameResourceManager, view_id: ViewId) -> &[GpuLight] {
    manager
        .per_view_lights
        .get(view_id)
        .map(|lights| lights.lights.as_slice())
        .unwrap_or(&[])
}

fn caster_needs_redraw(item: &WorldMeshDrawItem) -> bool {
    item.skinned || item.world_space_deformed || item.blendshape_deformed
}

fn hash_quality(fp: &mut Fingerprint, quality: HostShadowQuality) {
    fp.mix_u64(u64::from(quality.per_pixel_lights));
    fp.mix_u64(u64::from(quality.cascade_count));
    fp.mix_u64(u64::from(quality.tile_resolution));
    fp.mix_u64(u64::from(f32_key(quality.shadow_distance)));
}

fn hash_limits(fp: &mut Fingerprint, limits: Option<&GpuLimits>) {
    let Some(limits) = limits else {
        fp.mix_u64(0);
        return;
    };
    fp.mix_u64(1);
    fp.mix_u64(u64::from(limits.supports_base_instance));
    fp.mix_u64(u64::from(limits.max_texture_dimension_2d()));
    fp.mix_u64(u64::from(limits.max_texture_array_layers()));
}

fn hash_lights(fp: &mut Fingerprint, lights: &[GpuLight]) {
    fp.mix_u64(u64::try_from(lights.len()).unwrap_or(u64::MAX));
    for light in lights {
        hash_light(fp, light);
    }
}

fn hash_light(fp: &mut Fingerprint, light: &GpuLight) {
    fp.mix_u64(u64::from(light.light_type));
    hash_f32s(fp, &light.position);
    hash_f32s(fp, &light.direction);
    fp.mix_u64(u64::from(f32_key(light.range)));
    fp.mix_u64(u64::from(f32_key(light.spot_cos_half_angle)));
    fp.mix_u64(u64::from(light.shadow_type));
    fp.mix_u64(u64::from(f32_key(light.shadow_near_plane)));
    fp.mix_u64(u64::from(f32_key(light.shadow_bias)));
    fp.mix_u64(u64::from(f32_key(light.shadow_normal_bias)));
    fp.mix_u64(u64::from(light.shadow_map_resolution));
    let casts = u64::from(light.shadow_type != 0 && light.shadow_strength > 0.0);
    fp.mix_u64(casts);
}

/// Hashes one shadow caster.
///
/// Caster policy in this key is whether the draw casts, its cast mode, and its pipeline kind.
/// Those fields live on the draw. Depth-only shadow shaders (`world_mesh_shadow_caster.wgsl`
/// and the radial variant) have an empty `fs_main` and do not sample material property blocks,
/// so an animated material float does not change this key. Material asset id stays so a material
/// swap still misses. Mesh upload generation stays because encode reads the live mesh-pool
/// buffers for `mesh_asset_id`. Front face, topology, and cull mode stay because the depth
/// pipeline is built from them.
fn hash_caster(item: &WorldMeshDrawItem, revisions: &impl ShadowContentRevisions) -> u64 {
    let mut fp = Fingerprint::new();
    fp.mix_u64(item.instance_id.0);
    fp.mix_u64(signed_key(item.node_id));
    fp.mix_u64(u64::try_from(item.slot_index).unwrap_or(u64::MAX));
    fp.mix_u64(signed_key(item.mesh_asset_id));
    fp.mix_u64(revisions.mesh_upload_generation(item.mesh_asset_id));
    fp.mix_u64(u64::from(item.first_index));
    fp.mix_u64(u64::from(item.index_count));
    fp.mix_u64(u64::from(shadow_draw_casts(item)));
    fp.mix_u64(shadow_cast_mode_key(item.shadow_cast_mode));
    hash_pipeline(&mut fp, &item.batch_key.pipeline);
    fp.mix_u64(signed_key(item.batch_key.material_asset_id));
    fp.mix_u64(u64::from(item.skinned));
    fp.mix_u64(u64::from(item.world_space_deformed));
    fp.mix_u64(u64::from(item.blendshape_deformed));
    fp.mix_u64(stack_key(item));
    fp.mix_u64(front_face_key(item.batch_key.front_face));
    fp.mix_u64(topology_key(item.batch_key.primitive_topology));
    fp.mix_u64(cull_key(item));
    hash_matrix(&mut fp, item.rigid_world_matrix);
    hash_aabb(&mut fp, item.world_aabb);
    fp.into_u64()
}

fn signed_key(value: i32) -> u64 {
    u64::from(value.cast_unsigned())
}

fn shadow_cast_mode_key(mode: ShadowCastMode) -> u64 {
    match mode {
        ShadowCastMode::Off => 0,
        ShadowCastMode::On => 1,
        ShadowCastMode::ShadowOnly => 2,
        ShadowCastMode::DoubleSided => 3,
    }
}

fn hash_pipeline(fp: &mut Fingerprint, pipeline: &RasterPipelineKind) {
    match pipeline {
        RasterPipelineKind::Null => fp.mix_u64(0),
        RasterPipelineKind::EmbeddedStem(stem) => {
            fp.mix_u64(1);
            stem.hash(fp);
        }
    }
}

fn stack_key(item: &WorldMeshDrawItem) -> u64 {
    item.material_stack_order.map_or(u64::MAX, |order| {
        u64::try_from(order.first_stacked_slot_index).unwrap_or(u64::MAX)
    })
}

fn front_face_key(front_face: RasterFrontFace) -> u64 {
    match front_face {
        RasterFrontFace::Clockwise => 0,
        RasterFrontFace::CounterClockwise => 1,
    }
}

fn topology_key(topology: RasterPrimitiveTopology) -> u64 {
    match topology {
        RasterPrimitiveTopology::TriangleList => 0,
        RasterPrimitiveTopology::PointList => 1,
    }
}

fn cull_key(item: &WorldMeshDrawItem) -> u64 {
    if item.shadow_cast_mode == ShadowCastMode::DoubleSided {
        return 0;
    }
    match item
        .batch_key
        .render_state
        .resolved_cull_mode(Some(wgpu::Face::Back))
    {
        None => 1,
        Some(wgpu::Face::Front) => 2,
        Some(wgpu::Face::Back) => 3,
    }
}

fn hash_matrix(fp: &mut Fingerprint, matrix: Option<glam::Mat4>) {
    let Some(matrix) = matrix else {
        fp.mix_u64(0);
        return;
    };
    fp.mix_u64(1);
    for value in matrix.to_cols_array() {
        fp.mix_u64(u64::from(f32_key(value)));
    }
}

fn hash_aabb(fp: &mut Fingerprint, aabb: Option<(glam::Vec3, glam::Vec3)>) {
    let Some((min, max)) = aabb else {
        fp.mix_u64(0);
        return;
    };
    fp.mix_u64(1);
    hash_f32s(fp, &min.to_array());
    hash_f32s(fp, &max.to_array());
}

fn hash_f32s(fp: &mut Fingerprint, values: &[f32]) {
    for value in values {
        fp.mix_u64(u64::from(f32_key(*value)));
    }
}

/// Canonical bits for a finite float. `-0.0` and non-finite values share one key so sign-only
/// noise does not miss the cache.
fn f32_key(value: f32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let bits = value.to_bits();
    if bits == 0x8000_0000 { 0 } else { bits }
}

#[derive(Default)]
struct DrawFingerprint {
    count: u64,
    sum: u64,
    xor: u64,
}

impl DrawFingerprint {
    fn mix(&mut self, item_hash: u64) {
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.wrapping_add(item_hash);
        self.xor ^= item_hash.wrapping_mul(0x9E37_79B1_85EB_CA87);
    }

    fn write(&self, fp: &mut Fingerprint) {
        fp.mix_u64(self.count);
        fp.mix_u64(self.sum);
        fp.mix_u64(self.xor);
    }
}

struct Fingerprint(u64);

impl Fingerprint {
    fn new() -> Self {
        Self(0x5348_4144_4F57_0001)
    }

    fn mix_u64(&mut self, value: u64) {
        self.0 = self.0.wrapping_mul(0x9E37_79B1_85EB_CA87) ^ value;
    }

    fn into_u64(self) -> u64 {
        self.0
    }
}

impl Hasher for Fingerprint {
    fn write(&mut self, bytes: &[u8]) {
        let mut offset = 0;
        while offset < bytes.len() {
            let mut chunk = [0u8; 8];
            let end = (offset + 8).min(bytes.len());
            let len = end - offset;
            chunk[..len].copy_from_slice(&bytes[offset..end]);
            self.mix_u64(u64::from_le_bytes(chunk));
            offset = end;
        }
        self.mix_u64(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
