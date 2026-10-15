use crate::shared::TrailTextureMode;

/// Generated particle mesh id tag for billboard quads.
const BILLBOARD_MESH_KIND: i32 = 1;
/// Generated particle mesh id tag for stretch trail ribbons.
const TRAIL_STRETCH_MESH_KIND: i32 = 2;
/// Generated particle mesh id tag for tiled trail ribbons.
const TRAIL_TILE_MESH_KIND: i32 = 3;
/// Generated particle mesh id tag for distributed trail ribbons.
const TRAIL_DISTRIBUTE_MESH_KIND: i32 = 4;
/// Generated particle mesh id tag for per-segment repeated trail ribbons.
const TRAIL_REPEAT_MESH_KIND: i32 = 5;
/// Number of generated mesh ids reserved per source render-buffer asset.
const GENERATED_MESH_KIND_STRIDE: i64 = 8;

pub(crate) fn billboard_render_buffer_mesh_asset_id(asset_id: i32) -> Option<i32> {
    generated_mesh_asset_id(asset_id, BILLBOARD_MESH_KIND)
}

/// Returns the generated mesh asset id for a trail-buffer texture mode.
pub(crate) fn trail_render_buffer_mesh_asset_id(
    asset_id: i32,
    mode: TrailTextureMode,
) -> Option<i32> {
    let kind = match mode {
        TrailTextureMode::Stretch => TRAIL_STRETCH_MESH_KIND,
        TrailTextureMode::Tile => TRAIL_TILE_MESH_KIND,
        TrailTextureMode::DistributePerSegment => TRAIL_DISTRIBUTE_MESH_KIND,
        TrailTextureMode::RepeatPerSegment => TRAIL_REPEAT_MESH_KIND,
    };
    generated_mesh_asset_id(asset_id, kind)
}

/// Returns all generated mesh ids owned by a point render-buffer asset.
pub(crate) fn point_render_buffer_generated_mesh_ids(asset_id: i32) -> impl Iterator<Item = i32> {
    std::iter::once(billboard_render_buffer_mesh_asset_id(asset_id)).flatten()
}

/// Returns all generated mesh ids owned by a trail render-buffer asset.
pub(crate) fn trail_render_buffer_generated_mesh_ids(asset_id: i32) -> impl Iterator<Item = i32> {
    [
        trail_render_buffer_mesh_asset_id(asset_id, TrailTextureMode::Stretch),
        trail_render_buffer_mesh_asset_id(asset_id, TrailTextureMode::Tile),
        trail_render_buffer_mesh_asset_id(asset_id, TrailTextureMode::DistributePerSegment),
        trail_render_buffer_mesh_asset_id(asset_id, TrailTextureMode::RepeatPerSegment),
    ]
    .into_iter()
    .flatten()
}

/// Returns whether `asset_id` belongs to the generated PhotonDust mesh id range.
pub(crate) fn is_generated_particle_mesh_asset_id(asset_id: i32) -> bool {
    generated_mesh_kind(asset_id).is_some()
}

/// Returns whether `asset_id` is a generated PhotonDust billboard mesh id.
pub(crate) fn is_generated_billboard_mesh_asset_id(asset_id: i32) -> bool {
    generated_mesh_kind(asset_id) == Some(BILLBOARD_MESH_KIND)
}

/// Returns whether `asset_id` is a generated PhotonDust trail mesh id.
#[cfg(test)]
pub(crate) fn is_generated_trail_mesh_asset_id(asset_id: i32) -> bool {
    matches!(
        generated_mesh_kind(asset_id),
        Some(
            TRAIL_STRETCH_MESH_KIND
                | TRAIL_TILE_MESH_KIND
                | TRAIL_DISTRIBUTE_MESH_KIND
                | TRAIL_REPEAT_MESH_KIND
        )
    )
}

fn generated_mesh_asset_id(source_asset_id: i32, kind: i32) -> Option<i32> {
    if source_asset_id < 0 || !(0..GENERATED_MESH_KIND_STRIDE as i32).contains(&kind) {
        return None;
    }
    let encoded = i64::from(source_asset_id)
        .checked_mul(GENERATED_MESH_KIND_STRIDE)?
        .checked_add(i64::from(kind))?
        .checked_add(2)?;
    let id = -encoded;
    (id >= i64::from(i32::MIN) && id <= -2).then_some(id as i32)
}

fn generated_mesh_kind(asset_id: i32) -> Option<i32> {
    if asset_id >= -1 {
        return None;
    }
    let encoded = i64::from(asset_id).checked_neg()?;
    let payload = encoded.checked_sub(2)?;
    let kind = (payload % GENERATED_MESH_KIND_STRIDE) as i32;
    matches!(
        kind,
        BILLBOARD_MESH_KIND
            | TRAIL_STRETCH_MESH_KIND
            | TRAIL_TILE_MESH_KIND
            | TRAIL_DISTRIBUTE_MESH_KIND
            | TRAIL_REPEAT_MESH_KIND
    )
    .then_some(kind)
}
