//! Secondary-camera rows (`RenderSpaceUpdate.cameras_update`) for the frame-bench
//! `secondary_camera_copy` scene.

use glam::Vec4;
use renderide_shared::packing::memory_packable::MemoryPackable;
use renderide_shared::packing::memory_packer::MemoryPacker;
use renderide_shared::shared::{
    CAMERA_STATE_HOST_ROW_BYTES, CameraClearMode, CameraProjection, CameraState, RenderRect,
    RenderTransform,
};
use renderide_shared::wire_writer::mesh_renderers::encode_additions;

/// One enabled secondary camera that renders the scene into a host render texture.
///
/// Frame-bench only (`secondary_camera_copy`). Rows are sent through
/// `RenderSpaceUpdate.cameras_update`; the viewport is always the full target.
#[derive(Clone, Debug)]
pub(in crate::host::scene_session) struct SceneCamera {
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

pub(super) fn encode_camera_additions(cameras: &[SceneCamera]) -> Vec<u8> {
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

pub(super) fn encode_camera_states(cameras: &[SceneCamera]) -> Vec<u8> {
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
    use super::super::{
        SceneSharedMemoryRegions, SceneSubmission, default_camera_world_pose,
        target_transform_count,
    };
    use super::*;

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
}
