//! Shader variant bitmask uniform packing tests.

use super::super::*;
use super::common::*;

use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
use crate::materials::embedded::texture_pools::EmbeddedTexturePools;
use crate::materials::host_data::{
    MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::{ReflectedRasterLayout, ReflectedUniformScalarKind};

const RENDERIDE_VARIANT_BITS_FIELD: &str = "_RenderideVariantBits";
const VARIANT_BITS_OFFSET: usize = 32;
const TEST_VARIANT_BITS: u32 = 0x2202;
const TEST_OTHER_U32_BITS: u32 = 0x1;

fn empty_tex_ctx<'a>(pools: &'a EmbeddedTexturePools<'a>) -> UniformPackTextureContext<'a> {
    UniformPackTextureContext {
        pools,
        primary_texture_2d: -1,
    }
}

fn empty_pools() -> (
    crate::gpu_pools::TexturePool,
    crate::gpu_pools::Texture3dPool,
    crate::gpu_pools::CubemapPool,
    crate::gpu_pools::RenderTexturePool,
    crate::gpu_pools::VideoTexturePool,
) {
    empty_texture_pools()
}

fn unlit_variant_bits_reflection() -> (
    ReflectedRasterLayout,
    StemEmbeddedPropertyIds,
    PropertyIdRegistry,
) {
    reflected_with_uniform_fields_for_stem(
        "unlit_default",
        &[
            ("_Color", ReflectedUniformScalarKind::Vec4, 16, 0),
            ("_RightEye_ST", ReflectedUniformScalarKind::Vec4, 16, 16),
            (
                RENDERIDE_VARIANT_BITS_FIELD,
                ReflectedUniformScalarKind::U32,
                4,
                VARIANT_BITS_OFFSET as u32,
            ),
        ],
    )
}

#[test]
fn explicit_unlit_variant_bits_pack_reserved_u32_field() {
    let (reflected, ids, _registry) = unlit_variant_bits_reflection();
    let store = MaterialPropertyStore::new();

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };
    let bytes = build_embedded_uniform_bytes_with_value_spaces(
        &reflected,
        &ids,
        &MaterialUniformValueSpaces::default(),
        &store,
        lookup(70),
        &empty_tex_ctx(&pools),
        Some(TEST_VARIANT_BITS),
    )
    .expect("uniform bytes");

    assert_eq!(read_u32_at(&bytes, VARIANT_BITS_OFFSET), TEST_VARIANT_BITS);
}

#[test]
fn explicit_zero_unlit_variant_overrides_texture_and_tint_fallbacks() {
    let (reflected, ids, registry) = unlit_variant_bits_reflection();
    let mut store = MaterialPropertyStore::new();
    store.set_material(
        71,
        registry.intern("_Tex"),
        MaterialPropertyValue::Texture(123),
    );
    store.set_material(
        71,
        registry.intern("_Color"),
        MaterialPropertyValue::Float4([0.25, 1.0, 1.0, 1.0]),
    );

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };

    let bytes = build_embedded_uniform_bytes_with_value_spaces(
        &reflected,
        &ids,
        &MaterialUniformValueSpaces::default(),
        &store,
        lookup(71),
        &empty_tex_ctx(&pools),
        Some(0),
    )
    .expect("uniform bytes");

    assert_eq!(read_u32_at(&bytes, VARIANT_BITS_OFFSET), 0);
}

#[test]
fn missing_unlit_variant_bits_pack_zero_even_with_material_properties() {
    let (reflected, ids, registry) = unlit_variant_bits_reflection();
    let mut store = MaterialPropertyStore::new();
    store.set_material(
        72,
        registry.intern("_Tex"),
        MaterialPropertyValue::Texture(123),
    );
    store.set_material(
        72,
        registry.intern("_Color"),
        MaterialPropertyValue::Float4([0.25, 1.0, 1.0, 1.0]),
    );

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };

    let bytes = build_embedded_uniform_bytes_with_value_spaces(
        &reflected,
        &ids,
        &MaterialUniformValueSpaces::default(),
        &store,
        lookup(72),
        &empty_tex_ctx(&pools),
        None,
    )
    .expect("uniform bytes");

    assert_eq!(read_u32_at(&bytes, VARIANT_BITS_OFFSET), 0);
}

#[test]
fn unlit_real_color_property_still_packs_as_vec4() {
    let (reflected, ids, registry) = reflected_with_uniform_fields_for_stem(
        "unlit_default",
        &[
            ("_Color", ReflectedUniformScalarKind::Vec4, 16, 0),
            (
                RENDERIDE_VARIANT_BITS_FIELD,
                ReflectedUniformScalarKind::U32,
                4,
                16,
            ),
        ],
    );
    let mut store = MaterialPropertyStore::new();
    let color = [0.25, 0.5, 0.75, 0.8];
    store.set_material(
        76,
        registry.intern("_Color"),
        MaterialPropertyValue::Float4(color),
    );

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };

    let bytes = build_embedded_uniform_bytes_with_value_spaces(
        &reflected,
        &ids,
        &MaterialUniformValueSpaces::default(),
        &store,
        lookup(76),
        &empty_tex_ctx(&pools),
        Some(TEST_VARIANT_BITS),
    )
    .expect("uniform bytes");

    assert_eq!(read_f32x4(&bytes, 0), color);
    assert_eq!(read_u32_at(&bytes, 16), TEST_VARIANT_BITS);
}

#[test]
fn unrelated_u32_uniforms_stay_zero() {
    let (reflected, ids, _registry) = reflected_with_uniform_fields_for_stem(
        "unlit_default",
        &[("_OtherFlags", ReflectedUniformScalarKind::U32, 4, 0)],
    );
    let store = MaterialPropertyStore::new();

    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };

    let bytes = build_embedded_uniform_bytes_with_value_spaces(
        &reflected,
        &ids,
        &MaterialUniformValueSpaces::default(),
        &store,
        lookup(77),
        &empty_tex_ctx(&pools),
        Some(TEST_OTHER_U32_BITS),
    )
    .expect("uniform bytes");

    assert_eq!(read_u32_at(&bytes, 0), 0);
}
