//! Per-material custom shader key overrides.
//!
//! `Assign` stores a material asset id -> shader key. Draw resolution uses that key's stem when
//! the program is installed, and leaves the stock route in place until then. `Remove` drops every
//! override for the key. Material unload drops that id.

use std::sync::Arc;

use super::registry;

/// Records `material_asset_id` -> `key`. Returns whether the map changed.
pub(super) fn assign(material_asset_id: i32, key: &str) -> bool {
    let key = super::normalize_shader_key(key);
    if key.is_empty() {
        return false;
    }
    let mut registry = registry().write();
    if registry
        .material_overrides
        .get(&material_asset_id)
        .is_some_and(|existing| existing == &key)
    {
        return false;
    }
    registry.material_overrides.insert(material_asset_id, key);
    true
}

/// Drops the override for `material_asset_id`.
pub(super) fn clear(material_asset_id: i32) -> bool {
    registry()
        .write()
        .material_overrides
        .remove(&material_asset_id)
        .is_some()
}

/// `true` when any material is assigned to `key`.
pub(super) fn key_has_material(key: &str) -> bool {
    let key = super::normalize_shader_key(key);
    let registry = registry().read();
    let found = registry
        .material_overrides
        .values()
        .any(|stored| stored == &key);
    drop(registry);
    found
}

/// Installed default stem for the material's assigned key, when both exist.
pub(super) fn stem_for_material(material_asset_id: i32) -> Option<Arc<str>> {
    let key = {
        let registry = registry().read();
        let key = registry.material_overrides.get(&material_asset_id).cloned();
        drop(registry);
        key
    }?;
    super::default_stem_for_asset_name(&key)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::materials::RasterPipelineKind;
    use crate::materials::custom_shader::{self, apply_material_shader_override};

    const WGSL: &str = include_str!("../../../../../RenderideMod/Shaders/example_solid.wgsl");

    struct Guard(&'static str, i32, i32);

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = custom_shader::remove(self.0);
            let _ = custom_shader::clear_material_override(self.1);
            let _ = custom_shader::clear_material_override(self.2);
        }
    }

    #[test]
    fn assign_routes_one_material_and_keeps_stock_routes() {
        let key = "per-material-rainbow";
        let _guard = Guard(key, 88001, 88002);
        let installed = custom_shader::install(key, WGSL).expect("install");
        let unlit = RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default"));
        assert_eq!(
            custom_shader::pipeline_for_shader_name("Unlit", unlit.clone()),
            unlit,
            "a non-stock key must not replace the Unlit route"
        );
        assert!(custom_shader::default_stem_for_asset_name("unlit").is_none());
        assert_eq!(
            custom_shader::pipeline_for_shader_name(key, unlit.clone()),
            RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem.as_str()))
        );

        let assign = renderide_shared::format_custom_shader_assign(88001, key);
        let payload = renderide_shared::parse_custom_shader_payload(&assign)
            .expect("prefix")
            .expect("header");
        let resolved = custom_shader::resolve_custom_shader_payload(Ok(payload), |_| None);
        assert!(resolved.material_override_changed);
        assert!(resolved.retarget_shader_key.is_none());
        assert!(!resolved.rejected);
        let routed = apply_material_shader_override(88001, unlit.clone());
        let RasterPipelineKind::EmbeddedStem(stem) = routed else {
            panic!("assigned material should use the custom stem");
        };
        assert_eq!(stem.as_ref(), installed.default_stem);
        let generation = custom_shader::source_generation(&stem);
        assert_ne!(generation, 0);
        assert_eq!(apply_material_shader_override(88002, unlit.clone()), unlit);

        let replaced = custom_shader::install(key, &format!("{WGSL}\n")).expect("replace");
        assert_ne!(
            custom_shader::source_generation(&replaced.default_stem),
            generation
        );

        let clear = renderide_shared::format_custom_shader_clear(88001);
        let payload = renderide_shared::parse_custom_shader_payload(&clear)
            .expect("prefix")
            .expect("header");
        let cleared = custom_shader::resolve_custom_shader_payload(Ok(payload), |_| None);
        assert!(cleared.material_override_changed);
        assert_eq!(apply_material_shader_override(88001, unlit.clone()), unlit);

        let _ = custom_shader::resolve_custom_shader_payload(
            Ok(renderide_shared::parse_custom_shader_payload(
                &renderide_shared::format_custom_shader_assign(88001, key),
            )
            .expect("prefix")
            .expect("header")),
            |_| None,
        );
        let _ = custom_shader::resolve_custom_shader_payload(
            Ok(renderide_shared::parse_custom_shader_payload(
                &renderide_shared::format_custom_shader_assign(88002, key),
            )
            .expect("prefix")
            .expect("header")),
            |_| None,
        );
        custom_shader::remove(key).expect("remove");
        assert_eq!(apply_material_shader_override(88001, unlit.clone()), unlit);
        assert_eq!(apply_material_shader_override(88002, unlit.clone()), unlit);
        assert!(!custom_shader::clear_material_override(88001));
    }
}
