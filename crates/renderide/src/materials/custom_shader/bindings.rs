//! `@group(1)` checks for a custom material before it is published.
//!
//! Host materials already arrive as floats, float4s, and textures on
//! [`crate::shared::MaterialsUpdateBatch`]. A custom shader may only bind types that block can
//! fill, plus the renderer-owned `u32` variant field. Anything else is a shader upload error.

use super::passes::OwnedMaterialDefault;
use crate::materials::wgsl_reflect::identifier_names::unescape_property_name;
use crate::materials::{ReflectedRasterLayout, ReflectedUniformScalarKind};

pub(super) const BINDING_REJECTION_PREFIX: &str = "custom shader binding rejected:";

pub(super) fn binding_rejected(message: impl AsRef<str>) -> String {
    format!("{BINDING_REJECTION_PREFIX} {}", message.as_ref())
}

/// Returns whether `error` came from [`validate_custom_material_bindings`].
pub(super) fn is_binding_rejection(error: &str) -> bool {
    error.contains(BINDING_REJECTION_PREFIX)
}

/// Rejects `@group(1)` shapes the host material block cannot supply.
pub(super) fn validate_custom_material_bindings(
    reflected: &ReflectedRasterLayout,
    defaults: &[OwnedMaterialDefault],
) -> Result<(), String> {
    let uniform_binding = reflected
        .material_uniform
        .as_ref()
        .map(|uniform| uniform.binding);
    for entry in &reflected.material_entries {
        match entry.ty {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                ..
            } => {
                if uniform_binding != Some(entry.binding) {
                    return Err(binding_rejected(format!(
                        "uniform @binding({}) is not the material property block",
                        entry.binding
                    )));
                }
            }
            wgpu::BindingType::Texture { view_dimension, .. } => {
                if !matches!(
                    view_dimension,
                    wgpu::TextureViewDimension::D2
                        | wgpu::TextureViewDimension::D2Array
                        | wgpu::TextureViewDimension::D3
                        | wgpu::TextureViewDimension::Cube
                ) {
                    let name = binding_name(reflected, entry.binding);
                    return Err(binding_rejected(format!(
                        "texture `{name}` @binding({}) has an unsupported dimension",
                        entry.binding
                    )));
                }
            }
            wgpu::BindingType::Sampler(_) => {}
            _ => {
                return Err(binding_rejected(format!(
                    "unsupported @group(1) @binding({})",
                    entry.binding
                )));
            }
        }
    }
    if let Some(uniform) = reflected.material_uniform.as_ref() {
        for (field_name, field) in &uniform.fields {
            if field.kind == ReflectedUniformScalarKind::Unsupported {
                return Err(binding_rejected(format!(
                    "uniform `{}` has an unsupported type",
                    unescape_property_name(field_name)
                )));
            }
            if matches!(
                field.kind,
                ReflectedUniformScalarKind::F32Array | ReflectedUniformScalarKind::Vec4Array
            ) && field.array_stride < 16
            {
                return Err(binding_rejected(format!(
                    "uniform `{}` array stride {} is below the uniform alignment of 16",
                    unescape_property_name(field_name),
                    field.array_stride
                )));
            }
        }
        for default in defaults {
            let field = uniform
                .fields
                .iter()
                .find(|(field_name, _)| unescape_property_name(field_name) == default.property);
            let Some((_, field)) = field else {
                return Err(binding_rejected(format!(
                    "//#mat_default `{}` does not match a reflected uniform field",
                    default.property
                )));
            };
            let matches_kind = if default.is_vec4 {
                field.kind == ReflectedUniformScalarKind::Vec4
            } else {
                field.kind == ReflectedUniformScalarKind::F32
            };
            if !matches_kind {
                return Err(binding_rejected(format!(
                    "//#mat_default `{}` type does not match reflected field {:?}",
                    default.property, field.kind
                )));
            }
        }
    } else if !defaults.is_empty() {
        return Err(binding_rejected(
            "//#mat_default requires a reflected material uniform block",
        ));
    }
    Ok(())
}

fn binding_name(reflected: &ReflectedRasterLayout, binding: u32) -> String {
    reflected
        .material_group1_names
        .get(&binding)
        .map(|name| unescape_property_name(name).to_string())
        .unwrap_or_else(|| format!("binding {binding}"))
}
