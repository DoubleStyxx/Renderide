//! CPU workload for the custom-shader keyword variant cache.
//!
//! Setup installs one keyword-bearing shader and composes mask 1 into the cache. Timed samples
//! then take one of three production paths: `variant_wgsl` on that resident mask, a cache miss
//! plus `KeywordVariantCache::insert` of an already composed program, or `compose_variant_wgsl`
//! with no cache write.

use std::sync::Arc;

const SHADER_KEY: &str = "bench-keyword-variant";
const HIT_MASK: u32 = 0b01;

const KEYWORD_WGSL: &str = r#"//#keyword _EMISSION
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
    return rg::retain_globals_additive(mat._Color);
#endif
}
"#;

/// Hit, miss, and compose paths for one stem's keyword variant cache.
pub struct KeywordVariantCacheWorkload {
    inner: Option<VariantInner>,
    setup_error: Option<String>,
}

struct VariantInner {
    stem: String,
    source: Arc<str>,
    keywords: Arc<[String]>,
    packed: Arc<str>,
    hit_mask: u32,
    next_miss: u32,
}

impl KeywordVariantCacheWorkload {
    /// Installs the bench shader, clears any previous variants, and caches mask 1.
    #[must_use]
    pub fn standard() -> Self {
        match VariantInner::prepare() {
            Ok(inner) => Self {
                inner: Some(inner),
                setup_error: None,
            },
            Err(error) => Self {
                inner: None,
                setup_error: Some(error),
            },
        }
    }

    /// Why [`Self::standard`] did not build a workload, when setup failed.
    #[must_use]
    pub fn setup_error(&self) -> Option<&str> {
        self.setup_error.as_deref()
    }

    /// Resolves the resident mask through `variant_wgsl`.
    #[must_use]
    pub fn cache_hit(&self) -> u64 {
        let Some(inner) = self.inner.as_ref() else {
            return 0;
        };
        match super::variant_wgsl(&inner.stem, inner.hit_mask) {
            Ok(Some(wgsl)) => fold_wgsl(wgsl.as_ref()),
            Ok(None) | Err(_) => 0,
        }
    }

    /// Looks up a mask that is not cached, then inserts the precomposed program.
    ///
    /// Compose is not included. Each sample uses a new mask, so once the cache holds
    /// 16 entries the insert evicts the oldest.
    #[must_use]
    pub fn cache_miss(&mut self) -> u64 {
        let Some(inner) = self.inner.as_mut() else {
            return 0;
        };
        if inner.next_miss == 0 || inner.next_miss == inner.hit_mask {
            inner.next_miss = inner.next_miss.wrapping_add(1);
        }
        let mask = inner.next_miss;
        inner.next_miss = inner.next_miss.wrapping_add(1);
        insert_miss(&inner.stem, mask, &inner.packed)
    }

    /// Composes mask 1 with naga-oil. Does not read or write the variant cache.
    #[must_use]
    pub fn compose(&self) -> u64 {
        let Some(inner) = self.inner.as_ref() else {
            return 0;
        };
        match super::keywords::compose_variant_wgsl(
            inner.source.as_ref(),
            &inner.stem,
            false,
            inner.keywords.as_ref(),
            inner.hit_mask,
        ) {
            Ok(wgsl) => fold_wgsl(wgsl.as_ref()),
            Err(_) => 0,
        }
    }
}

impl VariantInner {
    fn prepare() -> Result<Self, String> {
        let installed = super::install(SHADER_KEY, KEYWORD_WGSL)?;
        let stem = installed.default_stem;
        clear_variants(&stem);
        let registry = super::registry().read();
        let Some(target) = registry.targets.get(&stem) else {
            drop(registry);
            return Err(format!("custom stem {stem} is not installed"));
        };
        let source = Arc::clone(&target.source);
        let keywords = Arc::clone(&target.keywords);
        drop(registry);
        if keywords.is_empty() {
            return Err(format!("custom stem {stem} has no keywords"));
        }
        let packed = super::variant_wgsl(&stem, HIT_MASK)?
            .ok_or_else(|| format!("custom stem {stem} did not cache keyword mask {HIT_MASK}"))?;
        Ok(Self {
            stem,
            source,
            keywords,
            packed,
            hit_mask: HIT_MASK,
            next_miss: HIT_MASK.wrapping_add(1),
        })
    }
}

fn clear_variants(stem: &str) {
    let mut registry = super::registry().write();
    if let Some(target) = registry.targets.get_mut(stem) {
        target.variants = super::keywords::KeywordVariantCache::default();
    }
    drop(registry);
}

fn insert_miss(stem: &str, mask: u32, packed: &Arc<str>) -> u64 {
    let inserted = {
        let mut registry = super::registry().write();
        let inserted = if let Some(target) = registry.targets.get_mut(stem) {
            let already_cached = target.variants.get(mask).is_some();
            if already_cached {
                false
            } else {
                target.variants.insert(mask, Arc::clone(packed));
                true
            }
        } else {
            false
        };
        drop(registry);
        inserted
    };
    if inserted {
        fold_wgsl(packed.as_ref()).wrapping_add(u64::from(mask))
    } else {
        0
    }
}

fn fold_wgsl(wgsl: &str) -> u64 {
    let mut acc = u64::try_from(wgsl.len()).unwrap_or(0);
    if let Some(byte) = wgsl.as_bytes().first().copied() {
        acc = acc.wrapping_add(u64::from(byte));
    }
    acc
}
