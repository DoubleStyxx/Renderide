//! Naga validation and Renderide shader contract checks.

use std::collections::BTreeMap;

use naga::back::wgsl::WriterFlags;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{
    Binding, EntryPoint, FunctionArgument, FunctionResult, Handle, Interpolation, Sampling,
    ShaderStage, Type, TypeInner,
};

use super::directives::BuildPassDirective;
use super::error::BuildError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EntryIoSlot {
    ty: Handle<Type>,
    interpolation: Option<Interpolation>,
    sampling: Option<Sampling>,
    blend_src: Option<u32>,
    per_primitive: bool,
}

/// Checks that `module` declares the entry points required by `passes`.
pub(super) fn validate_entry_points(
    module: &naga::Module,
    label: &str,
    passes: &[BuildPassDirective],
) -> Result<(), BuildError> {
    if passes.is_empty() {
        let has_compute = module
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Compute);
        if has_compute {
            return Ok(());
        }
        let has_vs = module
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Vertex && e.name == "vs_main");
        let has_any_fs = module
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Fragment);
        if !has_vs || !has_any_fs {
            return Err(BuildError::Message(format!(
                "{label}: expected a vs_main vertex entry point and at least one @fragment \
                 entry point (vertex={has_vs} fragment={has_any_fs})",
            )));
        }
        return Ok(());
    }
    for pass in passes {
        let has_vs = module
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Vertex && e.name == pass.vertex_entry.as_str());
        let has_fs = module
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Fragment && e.name == pass.fragment_entry.as_str());
        if !has_vs || !has_fs {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) expected entry points {} and {} (vertex={has_vs} fragment={has_fs})",
                pass.name, pass.pass_type, pass.vertex_entry, pass.fragment_entry
            )));
        }
    }
    Ok(())
}

/// Checks that every declared pass has compatible vertex output and fragment input locations.
pub(super) fn validate_pass_interfaces(
    module: &naga::Module,
    label: &str,
    passes: &[BuildPassDirective],
) -> Result<(), BuildError> {
    for pass in passes {
        let Some(vertex) = find_entry_point(module, ShaderStage::Vertex, &pass.vertex_entry) else {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) missing vertex entry point {}",
                pass.name, pass.pass_type, pass.vertex_entry
            )));
        };
        let Some(fragment) = find_entry_point(module, ShaderStage::Fragment, &pass.fragment_entry)
        else {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) missing fragment entry point {}",
                pass.name, pass.pass_type, pass.fragment_entry
            )));
        };
        validate_pass_interface_pair(module, label, pass, vertex, fragment)?;
    }
    Ok(())
}

fn find_entry_point<'a>(
    module: &'a naga::Module,
    stage: ShaderStage,
    name: &str,
) -> Option<&'a EntryPoint> {
    module
        .entry_points
        .iter()
        .find(|entry| entry.stage == stage && entry.name == name)
}

fn validate_pass_interface_pair(
    module: &naga::Module,
    label: &str,
    pass: &BuildPassDirective,
    vertex: &EntryPoint,
    fragment: &EntryPoint,
) -> Result<(), BuildError> {
    let vertex_outputs = collect_entry_output_locations(module, vertex, label)?;
    let fragment_inputs = collect_entry_input_locations(module, fragment, label)?;
    for (location, fragment_slot) in fragment_inputs {
        let Some(vertex_slot) = vertex_outputs.get(&location) else {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) fragment entry {} reads @location({location}), \
                 but vertex entry {} does not write it",
                pass.name, pass.pass_type, fragment.name, vertex.name
            )));
        };
        if module.types[vertex_slot.ty].inner != module.types[fragment_slot.ty].inner {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) @location({location}) type mismatch between vertex {} ({}) \
                 and fragment {} ({})",
                pass.name,
                pass.pass_type,
                vertex.name,
                type_label(module, vertex_slot.ty),
                fragment.name,
                type_label(module, fragment_slot.ty)
            )));
        }
        if vertex_slot.interpolation != fragment_slot.interpolation
            || vertex_slot.sampling != fragment_slot.sampling
            || vertex_slot.blend_src != fragment_slot.blend_src
            || vertex_slot.per_primitive != fragment_slot.per_primitive
        {
            return Err(BuildError::Message(format!(
                "{label}: pass `{}` ({:?}) @location({location}) interpolation mismatch between \
                 vertex {} ({}) and fragment {} ({})",
                pass.name,
                pass.pass_type,
                vertex.name,
                io_slot_label(*vertex_slot),
                fragment.name,
                io_slot_label(fragment_slot)
            )));
        }
    }
    Ok(())
}

fn collect_entry_input_locations(
    module: &naga::Module,
    entry: &EntryPoint,
    label: &str,
) -> Result<BTreeMap<u32, EntryIoSlot>, BuildError> {
    let mut slots = BTreeMap::new();
    let owner = format!("{label}: fragment entry {} input", entry.name);
    for arg in &entry.function.arguments {
        collect_argument_locations(module, arg, &owner, &mut slots)?;
    }
    Ok(slots)
}

fn collect_entry_output_locations(
    module: &naga::Module,
    entry: &EntryPoint,
    label: &str,
) -> Result<BTreeMap<u32, EntryIoSlot>, BuildError> {
    let mut slots = BTreeMap::new();
    let owner = format!("{label}: vertex entry {} output", entry.name);
    if let Some(result) = entry.function.result.as_ref() {
        collect_result_locations(module, result, &owner, &mut slots)?;
    }
    Ok(slots)
}

fn collect_argument_locations(
    module: &naga::Module,
    arg: &FunctionArgument,
    owner: &str,
    slots: &mut BTreeMap<u32, EntryIoSlot>,
) -> Result<(), BuildError> {
    collect_locations(module, arg.ty, arg.binding.as_ref(), owner, slots)
}

fn collect_result_locations(
    module: &naga::Module,
    result: &FunctionResult,
    owner: &str,
    slots: &mut BTreeMap<u32, EntryIoSlot>,
) -> Result<(), BuildError> {
    collect_locations(module, result.ty, result.binding.as_ref(), owner, slots)
}

fn collect_locations(
    module: &naga::Module,
    ty: Handle<Type>,
    binding: Option<&Binding>,
    owner: &str,
    slots: &mut BTreeMap<u32, EntryIoSlot>,
) -> Result<(), BuildError> {
    if let Some(binding) = binding {
        insert_location_slot(owner, ty, binding, slots)?;
        return Ok(());
    }
    if let TypeInner::Struct { members, .. } = &module.types[ty].inner {
        for member in members {
            collect_locations(module, member.ty, member.binding.as_ref(), owner, slots)?;
        }
    }
    Ok(())
}

fn insert_location_slot(
    owner: &str,
    ty: Handle<Type>,
    binding: &Binding,
    slots: &mut BTreeMap<u32, EntryIoSlot>,
) -> Result<(), BuildError> {
    let Binding::Location {
        location,
        interpolation,
        sampling,
        blend_src,
        per_primitive,
    } = binding
    else {
        return Ok(());
    };
    let slot = EntryIoSlot {
        ty,
        interpolation: *interpolation,
        sampling: *sampling,
        blend_src: *blend_src,
        per_primitive: *per_primitive,
    };
    if slots.insert(*location, slot).is_some() {
        return Err(BuildError::Message(format!(
            "{owner} declares duplicate @location({location})"
        )));
    }
    Ok(())
}

fn type_label(module: &naga::Module, ty: Handle<Type>) -> String {
    let ty = &module.types[ty];
    ty.name.clone().unwrap_or_else(|| format!("{:?}", ty.inner))
}

fn io_slot_label(slot: EntryIoSlot) -> String {
    format!(
        "interpolation={:?} sampling={:?} blend_src={:?} per_primitive={}",
        slot.interpolation, slot.sampling, slot.blend_src, slot.per_primitive
    )
}

/// Canonical material control property names that must never appear in material uniforms.
const MATERIAL_CONTROL_PROPERTY_NAMES: &[&str] = &[
    "_SrcBlend",
    "_SrcBlendBase",
    "_SrcBlendAdd",
    "_DstBlend",
    "_DstBlendBase",
    "_DstBlendAdd",
    "_ZWrite",
    "_ZTest",
    "_Cull",
    "_Culling",
    "_Stencil",
    "_StencilComp",
    "_StencilOp",
    "_StencilFail",
    "_StencilZFail",
    "_StencilReadMask",
    "_StencilWriteMask",
    "_ColorMask",
    "_OffsetFactor",
    "_OffsetUnits",
    "_RenderQueue",
];

/// Rejects any material whose `@group(1) @binding(0)` uniform contains material control fields.
pub(super) fn validate_no_pipeline_state_uniform_fields(
    module: &naga::Module,
    label: &str,
) -> Result<(), BuildError> {
    for (_, var) in module.global_variables.iter() {
        let Some(binding) = &var.binding else {
            continue;
        };
        if binding.group != 1 || binding.binding != 0 {
            continue;
        }
        if !matches!(var.space, naga::AddressSpace::Uniform) {
            continue;
        }
        let ty = &module.types[var.ty];
        let TypeInner::Struct { ref members, .. } = ty.inner else {
            continue;
        };
        for member in members {
            let Some(name) = member.name.as_deref() else {
                continue;
            };
            if MATERIAL_CONTROL_PROPERTY_NAMES.contains(&name) {
                let struct_name = ty.name.as_deref().unwrap_or("<unnamed>");
                return Err(BuildError::Message(format!(
                    "{label}: material uniform struct `{struct_name}` declares material-control \
                     field `{name}` at @group(1) @binding(0). Material-control properties \
                     flow through MaterialBlendMode + MaterialRenderState or draw ordering; \
                     remove the field from the WGSL struct."
                )));
            }
            if let Some(canonical) = confusing_material_control_property_name(name) {
                let struct_name = ty.name.as_deref().unwrap_or("<unnamed>");
                return Err(BuildError::Message(format!(
                    "{label}: material uniform struct `{struct_name}` declares field `{name}` \
                     at @group(1) @binding(0), which looks like material-control property \
                     `{canonical}`. Material-control properties flow through MaterialBlendMode + \
                     MaterialRenderState or draw ordering; fix the typo or remove the field from \
                     the WGSL struct."
                )));
            }
        }
    }
    Ok(())
}

fn confusing_material_control_property_name(name: &str) -> Option<&'static str> {
    MATERIAL_CONTROL_PROPERTY_NAMES
        .iter()
        .copied()
        .find(|canonical| {
            name.eq_ignore_ascii_case(canonical) || edit_distance_at_most_one(name, canonical)
        })
}

fn edit_distance_at_most_one(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }

    let mut ai = 0;
    let mut bi = 0;
    let mut edits = 0;
    while ai < a.len() && bi < b.len() {
        if a[ai] == b[bi] {
            ai += 1;
            bi += 1;
            continue;
        }

        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Less => bi += 1,
            std::cmp::Ordering::Equal => {
                ai += 1;
                bi += 1;
            }
            std::cmp::Ordering::Greater => ai += 1,
        }
    }

    edits + (a.len() - ai) + (b.len() - bi) <= 1
}

/// Validates a naga module and flattens it back to WGSL.
pub(super) fn module_to_wgsl(module: &naga::Module, label: &str) -> Result<String, BuildError> {
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator
        .validate(module)
        .map_err(|e| BuildError::Message(format!("validate {label}: {e}")))?;
    naga::back::wgsl::write_string(module, &info, WriterFlags::EXPLICIT_TYPES)
        .map_err(|e| BuildError::Message(format!("wgsl out {label}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with_material_field(field: &str) -> naga::Module {
        let wgsl = format!(
            r#"
struct TestMaterial {{
    {field}: f32,
}}

@group(1) @binding(0)
var<uniform> mat: TestMaterial;
"#
        );
        naga::front::wgsl::parse_str(&wgsl).expect("test WGSL parses")
    }

    fn validation_error_for_field(field: &str) -> String {
        let module = module_with_material_field(field);
        validate_no_pipeline_state_uniform_fields(&module, "test_shader")
            .expect_err("field should be rejected")
            .to_string()
    }

    #[test]
    fn rejects_exact_material_control_uniform_fields() {
        for field in ["_Culling", "_RenderQueue"] {
            let err = validation_error_for_field(field);
            assert!(
                err.contains(field),
                "{field} error should name field: {err}"
            );
            assert!(
                err.contains("material-control"),
                "{field} error should identify material-control state: {err}"
            );
        }
    }

    #[test]
    fn rejects_likely_material_control_uniform_typos() {
        let zwrite = validation_error_for_field("_Zwrite");
        assert!(
            zwrite.contains("_ZWrite"),
            "case typo error should name canonical field: {zwrite}"
        );

        let cull = validation_error_for_field("_Cul");
        assert!(
            cull.contains("_Cull"),
            "edit-distance typo error should name canonical field: {cull}"
        );
    }

    #[test]
    fn accepts_similar_shader_data_field_names() {
        let module = module_with_material_field("_ColorMask_ST");
        validate_no_pipeline_state_uniform_fields(&module, "test_shader")
            .expect("texture transform field is shader data, not pipeline state");
    }
}
