//! `//#keyword` shader defs for custom materials.
//!
//! Keywords are declared in source order. Bit `i` of the variant mask is keyword `i`, which is
//! the declaration order and not a Unity keyword bit index. A host shader-variant bit or a
//! nonzero float property of the same name turns that bit on. The composed WGSL for each mask is
//! cached per stem.

use std::collections::VecDeque;
use std::sync::Arc;

use hashbrown::HashMap;

use super::compose::compose_keyword_variant_wgsl;

/// Maximum `//#keyword` directives on one custom shader.
pub(super) const MAX_CUSTOM_KEYWORDS: usize = 8;
/// Maximum composed keyword variants retained for one stem.
pub(crate) const MAX_CUSTOM_KEYWORD_VARIANTS: usize = 16;

#[derive(Default)]
pub(super) struct KeywordVariantCache {
    entries: HashMap<u32, Arc<str>>,
    order: VecDeque<u32>,
}

impl KeywordVariantCache {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn get(&mut self, mask: u32) -> Option<Arc<str>> {
        let wgsl = self.entries.get(&mask)?.clone();
        if let Some(index) = self.order.iter().position(|stored| *stored == mask) {
            self.order.remove(index);
            self.order.push_back(mask);
        }
        Some(wgsl)
    }

    pub(super) fn insert(&mut self, mask: u32, wgsl: Arc<str>) {
        if self.entries.contains_key(&mask) {
            self.entries.insert(mask, wgsl);
            let _ = self.get(mask);
            return;
        }
        while self.entries.len() >= MAX_CUSTOM_KEYWORD_VARIANTS {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            } else {
                break;
            }
        }
        self.entries.insert(mask, wgsl);
        self.order.push_back(mask);
    }
}

/// Parses `//#keyword NAME` directives in source order.
pub(super) fn parse_keywords(source: &str, file: &str) -> Result<Vec<String>, String> {
    let mut keywords = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_no = line_idx + 1;
        let Some(rest) = line.trim_start().strip_prefix("//#keyword") else {
            continue;
        };
        if rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let mut tokens = rest.split_whitespace();
        let name = tokens
            .next()
            .ok_or_else(|| format!("{file}:{line_no}: `//#keyword` requires a name"))?;
        if tokens.next().is_some() {
            return Err(format!("{file}:{line_no}: `//#keyword` accepts one name"));
        }
        if !is_keyword_name(name) {
            return Err(format!(
                "{file}:{line_no}: `//#keyword` name `{name}` is not an identifier"
            ));
        }
        if keywords.iter().any(|existing| existing == name) {
            return Err(format!("{file}:{line_no}: duplicate `//#keyword` `{name}`"));
        }
        if keywords.len() >= MAX_CUSTOM_KEYWORDS {
            return Err(format!(
                "{file}:{line_no}: custom shaders accept at most {MAX_CUSTOM_KEYWORDS} `//#keyword` directives"
            ));
        }
        keywords.push(name.to_string());
    }
    Ok(keywords)
}

fn is_keyword_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Enabled keyword names for `mask`. Bit `i` selects `keywords[i]`.
pub(super) fn enabled_keywords(keywords: &[String], mask: u32) -> Vec<String> {
    keywords
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1u32 << index) != 0)
        .map(|(_, name)| name.clone())
        .collect()
}

/// `true` when `name` is a toggle-style float (`_EMISSION`) rather than a scalar such as `_Speed`.
pub(crate) fn is_toggle_style_property_name(name: &str) -> bool {
    let mut chars = name.chars();
    if chars.next() != Some('_') {
        return false;
    }
    let mut saw_body = false;
    for c in chars {
        if !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            return false;
        }
        saw_body = true;
    }
    saw_body
}

/// Composes one keyword variant of `source`.
pub(super) fn compose_variant_wgsl(
    source: &str,
    stem: &str,
    multiview: bool,
    keywords: &[String],
    mask: u32,
) -> Result<Arc<str>, String> {
    let enabled = enabled_keywords(keywords, mask);
    let label = format!("custom-keyword:{stem}:{mask}");
    let wgsl = compose_keyword_variant_wgsl(source, &label, multiview, &enabled)?;
    Ok(Arc::from(wgsl))
}

#[cfg(test)]
mod tests {
    use super::super::{
        install, keyword_mask, keyword_variant_cache_len, remove, target_wgsl, variant_wgsl,
    };

    const KEYWORD_WGSL: &str = r#"
//#keyword _EMISSION
//#keyword _ALPHATEST_ON

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef _EMISSION
    return rg::retain_globals_additive(vec4<f32>(1.0, 0.0, 0.0, 1.0));
#else
    return rg::retain_globals_additive(vec4<f32>(0.0, 1.0, 0.0, 1.0));
#endif
}
"#;

    struct RemoveKey(&'static str);

    impl RemoveKey {
        fn prepare(key: &'static str) -> Self {
            let _ = remove(key);
            Self(key)
        }
    }

    impl Drop for RemoveKey {
        fn drop(&mut self) {
            let _ = remove(self.0);
        }
    }

    #[test]
    fn keyword_ifdef_changes_composed_wgsl_and_mask() {
        let _guard = RemoveKey::prepare("kw-ifdef");
        let installed = install("kw-ifdef", KEYWORD_WGSL).expect("install");
        let stem = installed.default_stem.as_str();
        let base = target_wgsl(stem).expect("base wgsl");
        assert!(
            base.contains("vec4<f32>(0f, 1f, 0f, 1f)"),
            "mask 0 keeps the else branch"
        );
        assert!(
            !base.contains("vec4<f32>(1f, 0f, 0f, 1f)"),
            "mask 0 drops the emission branch"
        );

        let emission = variant_wgsl(stem, 0b01).expect("variant").expect("wgsl");
        assert!(
            emission.contains("vec4<f32>(1f, 0f, 0f, 1f)"),
            "emission bit composes the ifdef branch"
        );
        assert!(
            !emission.contains("vec4<f32>(0f, 1f, 0f, 1f)"),
            "emission bit drops the else branch"
        );

        assert_eq!(keyword_mask(stem, Some(0b10), |_| None), Some(0b10));
        assert_eq!(
            keyword_mask(stem, None, |name| { (name == "_EMISSION").then_some(2.0) }),
            Some(0b01)
        );
        assert_eq!(
            keyword_mask(stem, Some(0b10), |name| {
                (name == "_EMISSION").then_some(1.0)
            }),
            Some(0b11)
        );
        assert!(keyword_mask("unlit_default", Some(1), |_| Some(1.0)).is_none());
    }

    #[test]
    fn keyword_variant_cache_stays_bounded() {
        let _guard = RemoveKey::prepare("kw-cache");
        let source = r#"
//#keyword _K0
//#keyword _K1
//#keyword _K2
//#keyword _K3
//#keyword _K4

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    var tone = 0.0;
#ifdef _K0
    tone = tone + 1.0;
#endif
#ifdef _K1
    tone = tone + 2.0;
#endif
#ifdef _K2
    tone = tone + 4.0;
#endif
#ifdef _K3
    tone = tone + 8.0;
#endif
#ifdef _K4
    tone = tone + 16.0;
#endif
    return rg::retain_globals_additive(vec4<f32>(tone, 0.0, 0.0, 1.0));
}
"#;
        let installed = install("kw-cache", source).expect("install");
        let stem = installed.default_stem.as_str();
        for mask in 1u32..=17 {
            variant_wgsl(stem, mask)
                .expect("compose")
                .expect("cached variant");
        }
        assert_eq!(keyword_variant_cache_len(stem), 16);
        variant_wgsl(stem, 1)
            .expect("recompose")
            .expect("evicted variant");
        assert_eq!(keyword_variant_cache_len(stem), 16);
    }
}
