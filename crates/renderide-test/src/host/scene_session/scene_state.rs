//! Builds scene-state shared memory and pumps lockstep until the renderer has consumed it.

use std::path::Path;
use std::time::{Duration, Instant};

use glam::{Quat, Vec3, Vec4};
use renderide_shared::buffer::SharedMemoryBufferDescriptor;
use renderide_shared::ipc::HostDualQueueIpc;
use renderide_shared::packing::extras::SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES;
use renderide_shared::packing::memory_packable::MemoryPackable;
use renderide_shared::packing::memory_packer::MemoryPacker;
use renderide_shared::shared::{
    BoneAssignment, CAMERA_STATE_HOST_ROW_BYTES, CameraClearMode, CameraProjection,
    CameraRenderablesUpdate, CameraState, LIGHT_STATE_HOST_ROW_BYTES, LightRenderablesUpdate,
    LightState, LightType, MeshRenderablesUpdate, MeshRendererState, MotionVectorMode,
    RenderBoundingBox, RenderRect, RenderSH2, RenderSpaceUpdate, RenderTransform, ShadowCastMode,
    ShadowType, SkinnedMeshBoundsUpdate, SkinnedMeshRenderablesUpdate, TransformsUpdate,
};
use renderide_shared::wire_writer::mesh_renderers::{
    encode_additions, encode_mesh_states, encode_packed_material_ids,
};
use renderide_shared::wire_writer::transforms::{TransformPoseRow, encode_transform_pose_updates};
use renderide_shared::{SharedMemoryWriter, SharedMemoryWriterConfig};

use crate::error::HarnessError;

use super::super::lockstep::LockstepDriver;
use super::consts::{asset_ids, timing};

/// One mesh renderer submitted by a test scene.
#[derive(Clone, Debug)]
pub(super) struct SceneRenderable {
    /// Dense transform index this renderer attaches to.
    pub transform_id: i32,
    /// Local transform pose for the renderer.
    pub pose: RenderTransform,
    /// Uploaded mesh asset id.
    pub mesh_asset_id: i32,
    /// Material asset id bound to the renderer.
    pub material_asset_id: i32,
    /// Unity-style renderer sorting order.
    pub sorting_order: i32,
    /// Host shadow-caster mode. Existing unlit scenes leave this off.
    pub shadow_cast_mode: ShadowCastMode,
}

/// One one-bone skinned mesh renderer (an "avatar") submitted by a test scene.
///
/// The renderer node doubles as the root bone and the only bone, so [`Self::pose`] places the
/// whole mesh. `posed_bounds` is the object-space AABB the host would send each frame.
#[derive(Clone, Debug)]
pub(super) struct SceneSkinned {
    /// Dense transform index used as renderer node, root bone and bone 0.
    pub transform_id: i32,
    /// Local transform pose for the node (and bone).
    pub pose: RenderTransform,
    /// Uploaded skinned mesh asset id.
    pub mesh_asset_id: i32,
    /// Material asset id bound to the renderer.
    pub material_asset_id: i32,
    /// Host shadow-caster mode.
    pub shadow_cast_mode: ShadowCastMode,
    /// Posed object-space bounds sent through `SkinnedMeshRenderablesUpdate.bounds_updates`.
    pub posed_bounds: RenderBoundingBox,
}

/// One regular light submitted by a test scene.
#[derive(Clone, Debug)]
pub(super) struct SceneLight {
    /// Dense transform index this light attaches to.
    pub transform_id: i32,
    /// Local transform pose for the light.
    pub pose: RenderTransform,
    /// Light state; [`build_scene_state`] rewrites `renderable_index` to match submission order.
    pub state: LightState,
}

/// One enabled secondary camera that renders the scene into a host render texture.
///
/// Frame-bench only (`secondary_camera_copy`). Rows are sent through
/// `RenderSpaceUpdate.cameras_update`; the viewport is always the full target.
#[derive(Clone, Debug)]
pub(super) struct SceneCamera {
    /// Dense transform index this camera attaches to.
    pub transform_id: i32,
    /// Local (world, no parent) transform pose for the camera.
    pub pose: RenderTransform,
    /// Render texture asset the camera writes (allocated with `SetRenderTextureFormat`).
    pub render_texture_asset_id: i32,
    /// Host clear mode (`Nothing` / `Depth` exercise the clear-load seed path).
    pub clear_mode: CameraClearMode,
    /// Unity camera depth: secondary cameras render in ascending order.
    pub depth: f32,
}

/// Full scene payload written into one render-space update.
#[derive(Clone, Debug)]
pub(super) struct SceneSubmission {
    /// World-space camera pose used as `RenderSpaceUpdate.root_transform`.
    pub camera_world_pose: RenderTransform,
    /// Mesh renderers to add and update.
    pub renderables: Vec<SceneRenderable>,
    /// Regular lights to add and update.
    pub lights: Vec<SceneLight>,
    /// One-bone skinned renderers to add and update.
    pub skinned: Vec<SceneSkinned>,
}

impl SceneSubmission {
    /// Builds the historical one-mesh scene used by the original smoke cases.
    pub fn single_mesh(mesh_asset_id: i32, material_asset_id: i32) -> Self {
        Self {
            camera_world_pose: default_camera_world_pose(),
            renderables: vec![SceneRenderable {
                transform_id: 0,
                pose: identity_transform(),
                mesh_asset_id,
                material_asset_id,
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            }],
            lights: Vec::new(),
            skinned: Vec::new(),
        }
    }
}

/// Holds the scene SHM writer alive so the renderer can keep reading the descriptor over many
/// lockstep ticks. [`Drop`] releases the shared-memory mapping.
pub(super) struct SceneState {
    /// Live writer keeping the scene-state SHM region alive.
    writer: SharedMemoryWriter,
    /// Region bytes retained with the writer so the one-shot scene delta remains readable until
    /// the renderer consumes it.
    regions: SceneSharedMemoryRegions,
    /// Packed offsets for [`Self::rewrite`].
    layout: SceneSharedMemoryLayout,
    /// Secondary cameras whose transform rows are part of the pose region.
    cameras: Vec<SceneCamera>,
}

#[derive(Clone, Debug)]
struct SceneSharedMemoryRegions {
    pose_updates_bytes: Vec<u8>,
    mesh_additions_bytes: Vec<u8>,
    mesh_states_bytes: Vec<u8>,
    packed_material_ids_bytes: Vec<u8>,
    light_additions_bytes: Vec<u8>,
    light_states_bytes: Vec<u8>,
    skinned_additions_bytes: Vec<u8>,
    skinned_states_bytes: Vec<u8>,
    skinned_material_ids_bytes: Vec<u8>,
    bone_assignments_bytes: Vec<u8>,
    bone_indexes_bytes: Vec<u8>,
    skinned_bounds_bytes: Vec<u8>,
    camera_additions_bytes: Vec<u8>,
    camera_states_bytes: Vec<u8>,
}

impl SceneSharedMemoryRegions {
    #[cfg(test)]
    fn build(scene: &SceneSubmission) -> Self {
        Self::build_with_cameras(scene, &[])
    }

    fn build_with_cameras(scene: &SceneSubmission, cameras: &[SceneCamera]) -> Self {
        let pose_updates_bytes = encode_transform_pose_updates(&pose_rows(scene, cameras));
        let mesh_additions_bytes = encode_additions(
            &scene
                .renderables
                .iter()
                .map(|renderable| renderable.transform_id)
                .collect::<Vec<_>>(),
        );
        let mesh_states_bytes = encode_mesh_states(&mesh_state_rows(scene));
        let packed_material_ids_bytes = encode_packed_material_ids(
            &scene
                .renderables
                .iter()
                .map(|renderable| renderable.material_asset_id)
                .collect::<Vec<_>>(),
        );
        let light_additions_bytes = encode_additions(
            &scene
                .lights
                .iter()
                .map(|light| light.transform_id)
                .collect::<Vec<_>>(),
        );
        let light_states_bytes = encode_light_states(&light_state_rows(scene));
        let skinned = encode_skinned_regions(scene);
        Self {
            pose_updates_bytes,
            mesh_additions_bytes,
            mesh_states_bytes,
            packed_material_ids_bytes,
            light_additions_bytes,
            light_states_bytes,
            skinned_additions_bytes: skinned.additions,
            skinned_states_bytes: skinned.states,
            skinned_material_ids_bytes: skinned.material_ids,
            bone_assignments_bytes: skinned.bone_assignments,
            bone_indexes_bytes: skinned.bone_indexes,
            skinned_bounds_bytes: skinned.bounds,
            camera_additions_bytes: encode_camera_additions(cameras),
            camera_states_bytes: encode_camera_states(cameras),
        }
    }

    const fn total_bytes(&self) -> usize {
        self.pose_updates_bytes.len()
            + self.mesh_additions_bytes.len()
            + self.mesh_states_bytes.len()
            + self.packed_material_ids_bytes.len()
            + self.light_additions_bytes.len()
            + self.light_states_bytes.len()
            + self.skinned_additions_bytes.len()
            + self.skinned_states_bytes.len()
            + self.skinned_material_ids_bytes.len()
            + self.bone_assignments_bytes.len()
            + self.bone_indexes_bytes.len()
            + self.skinned_bounds_bytes.len()
            + self.camera_additions_bytes.len()
            + self.camera_states_bytes.len()
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneSharedMemoryLayout {
    pose_updates: SharedMemoryBufferDescriptor,
    mesh_additions: SharedMemoryBufferDescriptor,
    mesh_states: SharedMemoryBufferDescriptor,
    packed_material_ids: SharedMemoryBufferDescriptor,
    light_additions: SharedMemoryBufferDescriptor,
    light_states: SharedMemoryBufferDescriptor,
    skinned_additions: SharedMemoryBufferDescriptor,
    skinned_states: SharedMemoryBufferDescriptor,
    skinned_material_ids: SharedMemoryBufferDescriptor,
    bone_assignments: SharedMemoryBufferDescriptor,
    bone_indexes: SharedMemoryBufferDescriptor,
    skinned_bounds: SharedMemoryBufferDescriptor,
    camera_additions: SharedMemoryBufferDescriptor,
    camera_states: SharedMemoryBufferDescriptor,
}

impl SceneSharedMemoryLayout {
    fn pack_back_to_back(buffer_id: i32, regions: &SceneSharedMemoryRegions) -> Self {
        let capacity = regions.total_bytes() as i32;
        let mut offset = 0i32;
        let pose_updates = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.pose_updates_bytes.len(),
        );
        let mesh_additions = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.mesh_additions_bytes.len(),
        );
        let mesh_states = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.mesh_states_bytes.len(),
        );
        let packed_material_ids = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.packed_material_ids_bytes.len(),
        );
        let light_additions = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.light_additions_bytes.len(),
        );
        let light_states = descriptor_for_region(
            buffer_id,
            capacity,
            &mut offset,
            regions.light_states_bytes.len(),
        );
        let mut region = |len: usize| descriptor_for_region(buffer_id, capacity, &mut offset, len);
        let skinned_additions = region(regions.skinned_additions_bytes.len());
        let skinned_states = region(regions.skinned_states_bytes.len());
        let skinned_material_ids = region(regions.skinned_material_ids_bytes.len());
        let bone_assignments = region(regions.bone_assignments_bytes.len());
        let bone_indexes = region(regions.bone_indexes_bytes.len());
        let skinned_bounds = region(regions.skinned_bounds_bytes.len());
        let camera_additions = region(regions.camera_additions_bytes.len());
        let camera_states = region(regions.camera_states_bytes.len());
        Self {
            pose_updates,
            mesh_additions,
            mesh_states,
            packed_material_ids,
            light_additions,
            light_states,
            skinned_additions,
            skinned_states,
            skinned_material_ids,
            bone_assignments,
            bone_indexes,
            skinned_bounds,
            camera_additions,
            camera_states,
        }
    }
}

/// Builds the scene-state SHM region and latches a `RenderSpaceUpdate` into the lockstep driver.
pub(super) fn build_scene_state(
    prefix: &str,
    backing_dir: &Path,
    scene: &SceneSubmission,
    lockstep: &mut LockstepDriver,
) -> Result<SceneState, HarnessError> {
    build_scene_state_with_cameras(prefix, backing_dir, scene, &[], lockstep)
}

/// [`build_scene_state`] plus secondary cameras sent in the same `RenderSpaceUpdate`.
///
/// Every camera's render texture must already be resident (see
/// `asset_upload::create_render_texture`); the renderer skips cameras whose target is missing.
pub(super) fn build_scene_state_with_cameras(
    prefix: &str,
    backing_dir: &Path,
    scene: &SceneSubmission,
    cameras: &[SceneCamera],
    lockstep: &mut LockstepDriver,
) -> Result<SceneState, HarnessError> {
    let regions = SceneSharedMemoryRegions::build_with_cameras(scene, cameras);
    let total_bytes = regions.total_bytes();
    let cfg = SharedMemoryWriterConfig {
        prefix: prefix.to_string(),
        destroy_on_drop: true,
        dir_override: Some(backing_dir.to_path_buf()),
    };
    let mut writer = SharedMemoryWriter::open(cfg, asset_ids::SCENE_STATE_BUFFER, total_bytes)
        .map_err(|e| {
            HarnessError::QueueOptions(format!("open scene-state SHM (cap={total_bytes}): {e}"))
        })?;

    let layout =
        SceneSharedMemoryLayout::pack_back_to_back(asset_ids::SCENE_STATE_BUFFER, &regions);
    write_scene_regions(&mut writer, &layout, &regions)?;
    writer.flush();

    let render_space = build_render_space_update(scene, cameras, &layout);
    lockstep.set_render_space(Some(render_space));

    Ok(SceneState {
        writer,
        regions,
        layout,
        cameras: cameras.to_vec(),
    })
}

impl SceneState {
    /// Rewrites caster poses and latches a pose-only render-space delta.
    ///
    /// The lockstep host sends each delta once. Replaying mesh or light additions would allocate
    /// a second copy of every caster and light, so this path updates the existing transform rows
    /// and leaves the addition lists untouched.
    pub(super) fn rewrite(
        &mut self,
        scene: &SceneSubmission,
        lockstep: &mut LockstepDriver,
    ) -> Result<(), HarnessError> {
        let pose_updates_bytes = encode_transform_pose_updates(&pose_rows(scene, &self.cameras));
        if pose_updates_bytes.len() != self.regions.pose_updates_bytes.len() {
            return Err(HarnessError::QueueOptions(
                "scene rewrite changed the pose region size".to_string(),
            ));
        }
        write_region(
            &mut self.writer,
            self.layout.pose_updates,
            &pose_updates_bytes,
            "pose_updates",
        )?;
        self.writer.flush();
        self.regions.pose_updates_bytes = pose_updates_bytes;
        lockstep.set_render_space(Some(pose_only_render_space_update(
            scene,
            &self.cameras,
            &self.layout,
        )));
        Ok(())
    }
}

/// Pumps the lockstep until at least one `FrameSubmitData` carrying the scene has been enqueued.
/// Returns the `frame_index` of that submission so callers can log it.
pub(super) fn ensure_scene_submitted(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    timeout: Duration,
) -> Result<i32, HarnessError> {
    let deadline = Instant::now() + timeout;
    let frame_index_before = lockstep.current_frame_index();
    while Instant::now() < deadline {
        let tick = lockstep.tick(queues);
        if tick.frame_submits_sent > 0 {
            return Ok(frame_index_before);
        }
        std::thread::sleep(timing::SCENE_SUBMIT_POLL);
    }
    Err(HarnessError::AssetAckTimeout(
        deadline.elapsed(),
        "renderer never sent FrameStartData after scene was loaded",
    ))
}

/// Builds an identity transform.
pub(super) fn identity_transform() -> RenderTransform {
    RenderTransform {
        position: Vec3::ZERO,
        scale: Vec3::ONE,
        rotation: Quat::IDENTITY,
    }
}

/// Builds the default camera pose used by the original integration harness.
pub(super) fn default_camera_world_pose() -> RenderTransform {
    RenderTransform {
        position: Vec3::new(0.0, 0.0, -3.0),
        scale: Vec3::ONE,
        rotation: Quat::IDENTITY,
    }
}

/// Builds a no-shadow directional light row suitable for CI golden cases.
pub(super) fn directional_light(color: [f32; 3], intensity: f32) -> LightState {
    light_state(LightType::Directional, color, intensity, 50.0, 30.0)
}

/// Builds a no-shadow point light row suitable for CI golden cases.
pub(super) fn point_light(color: [f32; 3], intensity: f32, range: f32) -> LightState {
    light_state(LightType::Point, color, intensity, range, 30.0)
}

/// Builds a hard-shadow spot light. `resolution` is the atlas override, or `-1` for quality.
pub(super) fn shadowed_spot_light(
    color: [f32; 3],
    intensity: f32,
    range: f32,
    spot_angle: f32,
    resolution: i32,
) -> LightState {
    let mut light = light_state(LightType::Spot, color, intensity, range, spot_angle);
    light.shadow_type = ShadowType::Hard;
    light.shadow_strength = 1.0;
    light.shadow_near_plane = 0.05;
    light.shadow_bias = 0.02;
    light.shadow_map_resolution_override = resolution;
    light
}

/// Builds a hard-shadow point light. `resolution` is the atlas override, or `-1` for quality.
pub(super) fn shadowed_point_light(
    color: [f32; 3],
    intensity: f32,
    range: f32,
    resolution: i32,
) -> LightState {
    let mut light = light_state(LightType::Point, color, intensity, range, 30.0);
    light.shadow_type = ShadowType::Hard;
    light.shadow_strength = 1.0;
    light.shadow_near_plane = 0.05;
    light.shadow_bias = 0.02;
    light.shadow_map_resolution_override = resolution;
    light
}

fn light_state(
    light_type: LightType,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    spot_angle: f32,
) -> LightState {
    LightState {
        renderable_index: 0,
        intensity,
        range,
        spot_angle,
        color: Vec4::new(color[0], color[1], color[2], 1.0),
        shadow_strength: 0.0,
        shadow_near_plane: 0.1,
        shadow_map_resolution_override: -1,
        shadow_bias: 0.05,
        shadow_normal_bias: 0.4,
        cookie_texture_asset_id: -1,
        r#type: light_type,
        shadow_type: ShadowType::None,
        _padding: [0; 2],
    }
}

fn pose_rows(scene: &SceneSubmission, cameras: &[SceneCamera]) -> Vec<TransformPoseRow> {
    scene
        .renderables
        .iter()
        .map(|renderable| TransformPoseRow {
            transform_id: renderable.transform_id,
            pose: renderable.pose,
        })
        .chain(scene.lights.iter().map(|light| TransformPoseRow {
            transform_id: light.transform_id,
            pose: light.pose,
        }))
        .chain(scene.skinned.iter().map(|avatar| TransformPoseRow {
            transform_id: avatar.transform_id,
            pose: avatar.pose,
        }))
        .chain(cameras.iter().map(|camera| TransformPoseRow {
            transform_id: camera.transform_id,
            pose: camera.pose,
        }))
        .collect()
}

fn mesh_state_rows(scene: &SceneSubmission) -> Vec<MeshRendererState> {
    scene
        .renderables
        .iter()
        .enumerate()
        .map(|(index, renderable)| MeshRendererState {
            renderable_index: index as i32,
            mesh_asset_id: renderable.mesh_asset_id,
            material_count: 1,
            material_property_block_count: 0,
            sorting_order: renderable.sorting_order,
            shadow_cast_mode: renderable.shadow_cast_mode,
            motion_vector_mode: MotionVectorMode::NoMotion,
            _padding: [0; 2],
        })
        .collect()
}

/// Encoded skinned-mesh shared-memory regions for one scene.
struct SkinnedRegions {
    additions: Vec<u8>,
    states: Vec<u8>,
    material_ids: Vec<u8>,
    bone_assignments: Vec<u8>,
    bone_indexes: Vec<u8>,
    bounds: Vec<u8>,
}

fn encode_skinned_regions(scene: &SceneSubmission) -> SkinnedRegions {
    if scene.skinned.is_empty() {
        return SkinnedRegions {
            additions: Vec::new(),
            states: Vec::new(),
            material_ids: Vec::new(),
            bone_assignments: Vec::new(),
            bone_indexes: Vec::new(),
            bounds: Vec::new(),
        };
    }
    let states: Vec<MeshRendererState> = scene
        .skinned
        .iter()
        .enumerate()
        .map(|(index, avatar)| MeshRendererState {
            renderable_index: index as i32,
            mesh_asset_id: avatar.mesh_asset_id,
            material_count: 1,
            material_property_block_count: 0,
            sorting_order: 0,
            shadow_cast_mode: avatar.shadow_cast_mode,
            motion_vector_mode: MotionVectorMode::NoMotion,
            _padding: [0; 2],
        })
        .collect();
    let assignments: Vec<BoneAssignment> = scene
        .skinned
        .iter()
        .enumerate()
        .map(|(index, avatar)| BoneAssignment {
            renderable_index: index as i32,
            root_bone_transform_id: avatar.transform_id,
            bone_count: 1,
        })
        .collect();
    let bone_indexes: Vec<u8> = scene
        .skinned
        .iter()
        .flat_map(|avatar| avatar.transform_id.to_le_bytes())
        .collect();
    let mut bounds =
        vec![0u8; (scene.skinned.len() + 1) * SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES];
    for (index, avatar) in scene.skinned.iter().enumerate() {
        let start = index * SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES;
        let mut packer = MemoryPacker::new(
            &mut bounds[start..start + SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES],
        );
        SkinnedMeshBoundsUpdate {
            renderable_index: index as i32,
            local_bounds: avatar.posed_bounds,
        }
        .pack(&mut packer);
    }
    let sentinel = scene.skinned.len() * SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES;
    let mut packer = MemoryPacker::new(&mut bounds[sentinel..]);
    SkinnedMeshBoundsUpdate {
        renderable_index: -1,
        local_bounds: RenderBoundingBox::default(),
    }
    .pack(&mut packer);
    SkinnedRegions {
        additions: encode_additions(
            &scene
                .skinned
                .iter()
                .map(|avatar| avatar.transform_id)
                .collect::<Vec<_>>(),
        ),
        states: encode_mesh_states(&states),
        material_ids: encode_packed_material_ids(
            &scene
                .skinned
                .iter()
                .map(|avatar| avatar.material_asset_id)
                .collect::<Vec<_>>(),
        ),
        bone_assignments: bytemuck::cast_slice(&assignments).to_vec(),
        bone_indexes,
        bounds,
    }
}

fn light_state_rows(scene: &SceneSubmission) -> Vec<LightState> {
    scene
        .lights
        .iter()
        .enumerate()
        .map(|(index, light)| LightState {
            renderable_index: index as i32,
            ..light.state
        })
        .collect()
}

fn descriptor_for_region(
    buffer_id: i32,
    buffer_capacity: i32,
    offset: &mut i32,
    len: usize,
) -> SharedMemoryBufferDescriptor {
    let desc = SharedMemoryBufferDescriptor {
        buffer_id,
        buffer_capacity,
        offset: *offset,
        length: len as i32,
    };
    *offset += len as i32;
    desc
}

fn write_scene_regions(
    writer: &mut SharedMemoryWriter,
    layout: &SceneSharedMemoryLayout,
    regions: &SceneSharedMemoryRegions,
) -> Result<(), HarnessError> {
    write_region(
        writer,
        layout.pose_updates,
        &regions.pose_updates_bytes,
        "pose_updates",
    )?;
    write_region(
        writer,
        layout.mesh_additions,
        &regions.mesh_additions_bytes,
        "mesh additions",
    )?;
    write_region(
        writer,
        layout.mesh_states,
        &regions.mesh_states_bytes,
        "mesh_states",
    )?;
    write_region(
        writer,
        layout.packed_material_ids,
        &regions.packed_material_ids_bytes,
        "packed_material_ids",
    )?;
    write_region(
        writer,
        layout.light_additions,
        &regions.light_additions_bytes,
        "light additions",
    )?;
    write_region(
        writer,
        layout.light_states,
        &regions.light_states_bytes,
        "light_states",
    )?;
    let skinned_regions = [
        (
            layout.skinned_additions,
            &regions.skinned_additions_bytes,
            "skinned additions",
        ),
        (
            layout.skinned_states,
            &regions.skinned_states_bytes,
            "skinned mesh_states",
        ),
        (
            layout.skinned_material_ids,
            &regions.skinned_material_ids_bytes,
            "skinned materials",
        ),
        (
            layout.bone_assignments,
            &regions.bone_assignments_bytes,
            "bone_assignments",
        ),
        (
            layout.bone_indexes,
            &regions.bone_indexes_bytes,
            "bone_transform_indexes",
        ),
        (
            layout.skinned_bounds,
            &regions.skinned_bounds_bytes,
            "skinned bounds",
        ),
    ];
    for (desc, bytes, label) in skinned_regions {
        write_region(writer, desc, bytes, label)?;
    }
    write_region(
        writer,
        layout.camera_additions,
        &regions.camera_additions_bytes,
        "camera additions",
    )?;
    write_region(
        writer,
        layout.camera_states,
        &regions.camera_states_bytes,
        "camera states",
    )?;
    Ok(())
}

fn write_region(
    writer: &mut SharedMemoryWriter,
    desc: SharedMemoryBufferDescriptor,
    bytes: &[u8],
    label: &str,
) -> Result<(), HarnessError> {
    writer
        .write_at(desc.offset as usize, bytes)
        .map_err(|e| HarnessError::QueueOptions(format!("write {label}: {e}")))
}

fn pose_only_render_space_update(
    scene: &SceneSubmission,
    cameras: &[SceneCamera],
    layout: &SceneSharedMemoryLayout,
) -> RenderSpaceUpdate {
    let mut update = build_render_space_update(scene, cameras, layout);
    update.mesh_renderers_update = None;
    update.lights_update = None;
    update.cameras_update = None;
    update
}

fn build_render_space_update(
    scene: &SceneSubmission,
    cameras: &[SceneCamera],
    layout: &SceneSharedMemoryLayout,
) -> RenderSpaceUpdate {
    RenderSpaceUpdate {
        id: asset_ids::RENDER_SPACE,
        is_active: true,
        is_overlay: false,
        is_private: false,
        root_transform: scene.camera_world_pose,
        view_position_is_external: false,
        override_view_position: false,
        skybox_material_asset_id: -1,
        ambient_light: RenderSH2::default(),
        overriden_view_transform: RenderTransform::default(),
        transforms_update: Some(TransformsUpdate {
            target_transform_count: target_transform_count(scene, cameras),
            removals: SharedMemoryBufferDescriptor::default(),
            parent_updates: SharedMemoryBufferDescriptor::default(),
            pose_updates: layout.pose_updates,
        }),
        mesh_renderers_update: Some(MeshRenderablesUpdate {
            mesh_states: layout.mesh_states,
            mesh_materials_and_property_blocks: layout.packed_material_ids,
            removals: SharedMemoryBufferDescriptor::default(),
            additions: layout.mesh_additions,
        }),
        skinned_mesh_renderers_update: (!scene.skinned.is_empty()).then_some(
            SkinnedMeshRenderablesUpdate {
                bounds_updates: layout.skinned_bounds,
                bone_assignments: layout.bone_assignments,
                bone_transform_indexes: layout.bone_indexes,
                mesh_states: layout.skinned_states,
                mesh_materials_and_property_blocks: layout.skinned_material_ids,
                removals: SharedMemoryBufferDescriptor::default(),
                additions: layout.skinned_additions,
                ..SkinnedMeshRenderablesUpdate::default()
            },
        ),
        lights_update: (!scene.lights.is_empty()).then_some(LightRenderablesUpdate {
            states: layout.light_states,
            removals: SharedMemoryBufferDescriptor::default(),
            additions: layout.light_additions,
        }),
        cameras_update: (!cameras.is_empty()).then_some(CameraRenderablesUpdate {
            transform_ids: SharedMemoryBufferDescriptor::default(),
            states: layout.camera_states,
            removals: SharedMemoryBufferDescriptor::default(),
            additions: layout.camera_additions,
        }),
        camera_portals_update: None,
        reflection_probes_update: None,
        reflection_probe_sh2_taks: None,
        layers_update: None,
        billboard_buffers_update: None,
        mesh_render_buffers_update: None,
        trail_renderers_update: None,
        lights_buffer_renderers_update: None,
        render_transform_overrides_update: None,
        render_material_overrides_update: None,
        blit_to_displays_update: None,
        lod_group_update: None,
        gaussian_splat_renderers_update: None,
        reflection_probe_render_tasks: Vec::new(),
    }
}

/// First transform index not used by `scene` (secondary cameras attach from here).
pub(super) fn next_free_transform_id(scene: &SceneSubmission) -> i32 {
    target_transform_count(scene, &[])
}

fn target_transform_count(scene: &SceneSubmission, cameras: &[SceneCamera]) -> i32 {
    pose_rows(scene, cameras)
        .iter()
        .map(|row| row.transform_id)
        .max()
        .map_or(0, |max_id| max_id + 1)
}

fn encode_light_states(rows: &[LightState]) -> Vec<u8> {
    let row_size = LIGHT_STATE_HOST_ROW_BYTES;
    let mut out = vec![0u8; (rows.len() + 1) * row_size];
    for (index, row) in rows.iter().enumerate() {
        pack_light_state_row(&mut out[index * row_size..(index + 1) * row_size], *row);
    }
    let sentinel = LightState {
        renderable_index: -1,
        ..LightState::default()
    };
    let start = rows.len() * row_size;
    pack_light_state_row(&mut out[start..start + row_size], sentinel);
    out
}

fn pack_light_state_row(dest: &mut [u8], mut row: LightState) {
    let mut packer = MemoryPacker::new(dest);
    row.pack(&mut packer);
}

fn encode_camera_additions(cameras: &[SceneCamera]) -> Vec<u8> {
    if cameras.is_empty() {
        return Vec::new();
    }
    encode_additions(
        &cameras
            .iter()
            .map(|camera| camera.transform_id)
            .collect::<Vec<_>>(),
    )
}

/// Perspective, full-viewport, enabled (flags bit 0) camera row for a host render texture.
fn camera_state_row(index: usize, camera: &SceneCamera) -> CameraState {
    CameraState {
        renderable_index: index as i32,
        field_of_view: 60.0,
        orthographic_size: 1.0,
        near_clip: 0.05,
        far_clip: 100.0,
        background_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        viewport: RenderRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        depth: camera.depth,
        render_texture_asset_id: camera.render_texture_asset_id,
        selective_render_count: 0,
        exclude_render_count: 0,
        clear_mode: camera.clear_mode,
        projection: CameraProjection::Perspective,
        flags: 1,
    }
}

fn encode_camera_states(cameras: &[SceneCamera]) -> Vec<u8> {
    if cameras.is_empty() {
        return Vec::new();
    }
    let row_size = CAMERA_STATE_HOST_ROW_BYTES;
    let mut out = vec![0u8; (cameras.len() + 1) * row_size];
    for (index, camera) in cameras.iter().enumerate() {
        let mut row = camera_state_row(index, camera);
        let mut packer = MemoryPacker::new(&mut out[index * row_size..(index + 1) * row_size]);
        row.pack(&mut packer);
    }
    let mut sentinel = CameraState {
        renderable_index: -1,
        ..CameraState::default()
    };
    let start = cameras.len() * row_size;
    let mut packer = MemoryPacker::new(&mut out[start..start + row_size]);
    sentinel.pack(&mut packer);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_submission_single_mesh_matches_historical_region_sizes() {
        let scene = SceneSubmission::single_mesh(2, 4);
        let regions = SceneSharedMemoryRegions::build(&scene);
        assert_eq!(regions.pose_updates_bytes.len(), 44);
        assert_eq!(regions.mesh_additions_bytes.len(), 8);
        assert_eq!(regions.mesh_states_bytes.len(), 48);
        assert_eq!(regions.packed_material_ids_bytes.len(), 4);
        assert_eq!(regions.light_additions_bytes.len(), 4);
        assert_eq!(regions.light_states_bytes.len(), LIGHT_STATE_HOST_ROW_BYTES);
    }

    #[test]
    fn multi_renderable_scene_uses_transform_ids_and_material_order() {
        let scene = SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables: vec![
                SceneRenderable {
                    transform_id: 2,
                    pose: identity_transform(),
                    mesh_asset_id: 10,
                    material_asset_id: 20,
                    sorting_order: 0,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
                SceneRenderable {
                    transform_id: 4,
                    pose: identity_transform(),
                    mesh_asset_id: 11,
                    material_asset_id: 21,
                    sorting_order: 3,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
            ],
            lights: Vec::new(),
            skinned: Vec::new(),
        };
        let regions = SceneSharedMemoryRegions::build(&scene);
        assert_eq!(regions.mesh_additions_bytes.len(), 12);
        assert_eq!(regions.mesh_states_bytes.len(), 72);
        assert_eq!(regions.packed_material_ids_bytes.len(), 8);
        assert_eq!(target_transform_count(&scene, &[]), 5);
    }

    #[test]
    fn skinned_regions_are_sized_and_terminated() {
        let avatar = |transform_id| SceneSkinned {
            transform_id,
            pose: identity_transform(),
            mesh_asset_id: 3,
            material_asset_id: 4,
            shadow_cast_mode: ShadowCastMode::On,
            posed_bounds: RenderBoundingBox::default(),
        };
        let scene = SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables: Vec::new(),
            lights: Vec::new(),
            skinned: vec![avatar(7), avatar(9)],
        };
        let regions = SceneSharedMemoryRegions::build(&scene);
        assert_eq!(regions.skinned_additions_bytes.len(), 12);
        assert_eq!(regions.skinned_states_bytes.len(), 72);
        assert_eq!(regions.skinned_material_ids_bytes.len(), 8);
        assert_eq!(regions.bone_assignments_bytes.len(), 24);
        assert_eq!(regions.bone_indexes_bytes.len(), 8);
        assert_eq!(
            regions.skinned_bounds_bytes.len(),
            3 * SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES
        );
        assert_eq!(target_transform_count(&scene, &[]), 10);
    }

    #[test]
    fn camera_regions_are_indexed_terminated_and_count_transforms() {
        let camera = |transform_id, clear_mode, depth| SceneCamera {
            transform_id,
            pose: default_camera_world_pose(),
            render_texture_asset_id: 900 + transform_id,
            clear_mode,
            depth,
        };
        let cameras = [
            camera(1, CameraClearMode::Depth, 0.0),
            camera(2, CameraClearMode::Nothing, 1.0),
        ];
        let scene = SceneSubmission::single_mesh(2, 4);
        let regions = SceneSharedMemoryRegions::build_with_cameras(&scene, &cameras);
        assert_eq!(regions.camera_additions_bytes.len(), 12);
        assert_eq!(
            regions.camera_states_bytes.len(),
            3 * CAMERA_STATE_HOST_ROW_BYTES
        );
        let row_index = |row: usize| {
            let start = row * CAMERA_STATE_HOST_ROW_BYTES;
            i32::from_le_bytes(
                regions.camera_states_bytes[start..start + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!([row_index(0), row_index(1), row_index(2)], [0, 1, -1]);
        assert_eq!(target_transform_count(&scene, &cameras), 3);
        let no_cameras = SceneSharedMemoryRegions::build(&scene);
        assert!(no_cameras.camera_additions_bytes.is_empty());
        assert!(no_cameras.camera_states_bytes.is_empty());
    }

    #[test]
    fn light_state_rows_are_indexed_and_terminated() {
        let scene = SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables: Vec::new(),
            lights: vec![
                SceneLight {
                    transform_id: 0,
                    pose: identity_transform(),
                    state: point_light([1.0, 0.5, 0.25], 2.0, 3.0),
                },
                SceneLight {
                    transform_id: 1,
                    pose: identity_transform(),
                    state: directional_light([0.5, 0.5, 1.0], 1.0),
                },
            ],
            skinned: Vec::new(),
        };
        let bytes = encode_light_states(&light_state_rows(&scene));
        assert_eq!(bytes.len(), 3 * LIGHT_STATE_HOST_ROW_BYTES);
        assert_eq!(i32::from_le_bytes(bytes[0..4].try_into().unwrap()), 0);
        assert_eq!(
            i32::from_le_bytes(
                bytes[LIGHT_STATE_HOST_ROW_BYTES..LIGHT_STATE_HOST_ROW_BYTES + 4]
                    .try_into()
                    .unwrap()
            ),
            1
        );
        assert_eq!(
            i32::from_le_bytes(
                bytes[2 * LIGHT_STATE_HOST_ROW_BYTES..2 * LIGHT_STATE_HOST_ROW_BYTES + 4]
                    .try_into()
                    .unwrap()
            ),
            -1
        );
    }
}
