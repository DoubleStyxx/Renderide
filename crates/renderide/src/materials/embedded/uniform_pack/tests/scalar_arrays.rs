//! Scalar array uniform packing tests.

use super::super::*;
use super::common::*;

use crate::materials::ReflectedUniformScalarKind;
use crate::materials::embedded::texture_pools::EmbeddedTexturePools;
use crate::materials::host_data::{
    MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue,
};

fn empty_tex_ctx<'a>(pools: &'a EmbeddedTexturePools<'a>) -> UniformPackTextureContext<'a> {
    UniformPackTextureContext {
        pools,
        primary_texture_2d: -1,
    }
}

#[test]
fn float_array_packs_unsupported_uniform_array_with_scalar_stride() {
    let (reflected, ids, registry) = reflected_with_uniform_fields(&[(
        "_SlicerOffset",
        ReflectedUniformScalarKind::Unsupported,
        64,
        0,
    )]);
    let pid = registry.intern("_SlicerOffset");
    let mut store = MaterialPropertyStore::new();
    store.set_material(
        90,
        pid,
        MaterialPropertyValue::FloatArray(vec![0.25, 0.5, 0.75, 1.0, 2.0]),
    );

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_texture_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };
    let bytes =
        build_embedded_uniform_bytes(&reflected, &ids, &store, lookup(90), &empty_tex_ctx(&pools))
            .expect("uniform bytes");

    assert_eq!(read_f32_at(&bytes, 0), 0.25);
    assert_eq!(read_f32_at(&bytes, 16), 0.5);
    assert_eq!(read_f32_at(&bytes, 32), 0.75);
    assert_eq!(read_f32_at(&bytes, 48), 1.0);
}

#[test]
fn property_block_float_array_overrides_material_float_array() {
    let (reflected, ids, registry) = reflected_with_uniform_fields(&[(
        "_HighlightRange",
        ReflectedUniformScalarKind::Unsupported,
        64,
        0,
    )]);
    let pid = registry.intern("_HighlightRange");
    let mut store = MaterialPropertyStore::new();
    store.set_material(
        91,
        pid,
        MaterialPropertyValue::FloatArray(vec![0.25, 0.5, 0.75, 1.0]),
    );
    store.set_property_block(910, pid, MaterialPropertyValue::FloatArray(vec![3.0, 2.0]));

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_texture_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };
    let bytes = build_embedded_uniform_bytes(
        &reflected,
        &ids,
        &store,
        MaterialPropertyLookupIds {
            material_asset_id: 91,
            mesh_property_block_slot0: Some(910),
            mesh_renderer_property_block_id: None,
        },
        &empty_tex_ctx(&pools),
    )
    .expect("uniform bytes");

    assert_eq!(read_f32_at(&bytes, 0), 3.0);
    assert_eq!(read_f32_at(&bytes, 16), 2.0);
    assert_eq!(read_f32_at(&bytes, 32), 0.0);
    assert_eq!(read_f32_at(&bytes, 48), 0.0);
}
