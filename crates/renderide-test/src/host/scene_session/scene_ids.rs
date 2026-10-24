//! Dense per-index asset, buffer and request ids for scene-DSL cases, offset from the
//! reserved bases in [`super::consts::asset_ids`].

use super::consts::asset_ids;

pub(super) const fn scene_mesh_id(index: i32) -> i32 {
    asset_ids::SCENE_MESH_BASE + index
}

pub(super) const fn scene_material_id(index: i32) -> i32 {
    asset_ids::SCENE_MATERIAL_BASE + index
}

pub(super) const fn scene_shader_id(index: i32) -> i32 {
    asset_ids::SCENE_SHADER_BASE + index
}

pub(super) const fn scene_texture_id(index: i32) -> i32 {
    asset_ids::SCENE_TEXTURE_BASE + index
}

pub(super) const fn scene_mesh_buffer_id(index: i32) -> i32 {
    asset_ids::SCENE_MESH_BUFFER_BASE + index
}

pub(super) const fn scene_texture_buffer_id(index: i32) -> i32 {
    asset_ids::SCENE_TEXTURE_BUFFER_BASE + index
}

pub(super) const fn scene_material_update_buffer_id(index: i32) -> i32 {
    asset_ids::SCENE_MATERIAL_UPDATE_BUFFER_BASE + index * 4
}

pub(super) const fn scene_update_batch_id(index: i32) -> i32 {
    asset_ids::SCENE_MATERIAL_UPDATE_BATCH_BASE + index
}

pub(super) const fn scene_property_request_id(index: i32) -> i32 {
    asset_ids::SCENE_PROPERTY_ID_REQUEST_BASE + index
}
