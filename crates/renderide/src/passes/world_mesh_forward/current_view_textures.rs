//! Debug HUD: current-view 2D texture asset ids derived from sorted world-mesh draws.

use crate::materials::EmbeddedMaterialBindResources;
use crate::materials::MaterialSystem;
use crate::materials::RasterPipelineKind;
use crate::materials::host_data::MaterialPropertyStore;
use crate::world_mesh::draw_prep::WorldMeshDrawItem;

/// Texture2D asset ids bound for one embedded-stem draw (from reflection layout).
fn per_material_texture2d_asset_ids_for_draw(
    bind: &EmbeddedMaterialBindResources,
    stem: &str,
    store: &MaterialPropertyStore,
    item: &WorldMeshDrawItem,
) -> Vec<i32> {
    bind.texture2d_asset_ids_for_stem(stem, store, item.lookup_ids)
}

/// Appends texture ids for embedded-stem draws into `out` in draw order (may contain duplicates).
fn append_per_pass_texture2d_asset_ids_from_draws(
    materials: &MaterialSystem,
    draws: &[WorldMeshDrawItem],
    out: &mut Vec<i32>,
) {
    let Some(bind) = materials.embedded_material_bind() else {
        return;
    };
    let Some(registry) = materials.material_registry() else {
        return;
    };
    let store = materials.material_property_store();
    for item in draws {
        if !matches!(item.batch_key.pipeline, RasterPipelineKind::EmbeddedStem(_)) {
            continue;
        }
        let Some(stem) = registry.stem_for_shader_asset(item.batch_key.shader_asset_id) else {
            continue;
        };
        out.extend(per_material_texture2d_asset_ids_for_draw(
            bind, stem, store, item,
        ));
    }
}

/// Sort-then-dedup in O(n log n) instead of the O(n^2) `Vec::contains` linear scan.
///
/// Sort order is implementation-defined (numeric ascending) since the debug HUD only needs the
/// set of bound textures, not the original draw-order sequence.
fn dedup_visible_texture_asset_ids(ids: &mut Vec<i32>) {
    ids.sort_unstable();
    ids.dedup();
}

/// Fills `out` with the deduplicated Texture2D asset ids referenced by embedded materials in the
/// current sorted draw list. `out` is cleared before filling so its capacity survives across
/// frames when the caller reuses the same buffer.
pub(super) fn current_view_texture2d_asset_ids_from_draws(
    materials: &MaterialSystem,
    draws: &[WorldMeshDrawItem],
    out: &mut Vec<i32>,
) {
    out.clear();
    append_per_pass_texture2d_asset_ids_from_draws(materials, draws, out);
    dedup_visible_texture_asset_ids(out);
}
