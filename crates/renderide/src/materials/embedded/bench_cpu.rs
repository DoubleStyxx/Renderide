//! CPU workloads for custom-material uniform updates and bind-group cache churn.
//!
//! Setup installs one custom material through [`crate::materials::custom_shader::install`],
//! reflects the composed WGSL, and fills a [`MaterialPropertyStore`]. Timed samples then call
//! the production packer, uniform-arena slot table, frame-upload byte arena, texture-bind
//! signature, and bind-group LRU. No wgpu device is created.

use crate::frame_contract::OffscreenWriteTarget;
use crate::frame_upload_batch::FrameUploadBatch;
use crate::gpu_pools::{
    CubemapPool, RenderTexturePool, Texture3dPool, TexturePool, VideoTexturePool,
};
use crate::materials::custom_shader::{self, install};
use crate::materials::host_data::{
    MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::reflect_raster_material_wgsl;

use super::layout::{StemEmbeddedPropertyIds, material_uniform_defaults_for_stem, stem_hash};
use super::material_bind::{
    CpuMaterialBindGroupCache, CpuMaterialUniformArena, material_bind_cache_key_for_bench,
};
use super::texture_pools::EmbeddedTexturePools;
use super::texture_resolve::{primary_texture_2d_asset_id, texture_bind_signature};
use super::uniform_pack::{
    MaterialUniformDefaults, MaterialUniformPackMetadata, MaterialUniformValueSpaces,
    UniformPackTextureContext, build_embedded_uniform_bytes_with_material_defaults,
};
use crate::materials::ReflectedRasterLayout;

/// Materials repacked by one [`MaterialUniformUpdateWorkload::pack_changed_properties`] sample.
pub const MATERIAL_UNIFORM_BATCH: usize = 1_024;

/// Bind-group keys visited by one [`BindGroupChurnWorkload`] sample.
pub const BIND_GROUP_BATCH: usize = 1_024;

const CUSTOM_SHADER_KEY: &str = "bench-material-uniform";
const TEX_BINDING: u32 = 1;
const MASK_BINDING: u32 = 3;
/// Settled uniform-arena generation. The bench arena is sized so it does not grow.
const SETTLED_ARENA_GENERATION: u64 = 0;

/// Custom material exercised by the uniform and bind-group benches.
///
/// Field set matches the production packer inputs: an sRGB color, stored floats, `//#mat_default`
/// fallbacks, a `*_ST` identity fallback, `_RenderideVariantBits`, and texture LOD-bias fields.
const BENCH_CUSTOM_MATERIAL_WGSL: &str = r#"//#render_queue Geometry
//#mat_default _Color vec4 1.0 0.35 0.1 1.0
//#mat_default _Cutoff float 0.5
//#mat_default _Tex_ST vec4 1.0 1.0 0.0 0.0

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _Tex_ST: vec4<f32>,
    _MaskTex_ST: vec4<f32>,
    _Cutoff: f32,
    _Smoothness: f32,
    _RenderideVariantBits: u32,
    _Tex_LodBias: f32,
    _MaskTex_LodBias: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _Tex: texture_2d<f32>;
@group(1) @binding(2) var _Tex_sampler: sampler;
@group(1) @binding(3) var _MaskTex: texture_2d<f32>;
@group(1) @binding(4) var _MaskTex_sampler: sampler;

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
    let albedo = textureSampleLevel(_Tex, _Tex_sampler, vec2<f32>(0.5, 0.5), mat._Tex_LodBias);
    let mask = textureSampleLevel(_MaskTex, _MaskTex_sampler, vec2<f32>(0.5, 0.5), mat._MaskTex_LodBias);
    let tint = mat._Color * mat._Cutoff;
    let st = mat._Tex_ST + mat._MaskTex_ST;
    let bits = f32(mat._RenderideVariantBits) * 0.0;
    let rgb = albedo.rgb * mask.rgb * tint.rgb + st.xyz * 0.0;
    return rg::retain_globals_additive(vec4<f32>(rgb, mat._Smoothness + bits));
}
"#;

/// Packs and stages one custom material's uniform block after a property write.
pub struct MaterialUniformUpdateWorkload {
    inner: Option<UniformInner>,
    setup_error: Option<String>,
}

/// Hit and post-invalidation rebuild of the embedded material bind-group cache.
pub struct BindGroupChurnWorkload {
    inner: Option<BindInner>,
    setup_error: Option<String>,
}

struct UniformInner {
    reflected: ReflectedRasterLayout,
    ids: StemEmbeddedPropertyIds,
    value_spaces: MaterialUniformValueSpaces,
    defaults: MaterialUniformDefaults,
    store: MaterialPropertyStore,
    pools: OwnedTexturePools,
    arena: CpuMaterialUniformArena,
    uploads: FrameUploadBatch,
    stem_hash: u64,
    color_pid: i32,
    tick: u32,
}

struct BindInner {
    reflected: ReflectedRasterLayout,
    ids: StemEmbeddedPropertyIds,
    store: MaterialPropertyStore,
    pools: OwnedTexturePools,
    cache: CpuMaterialBindGroupCache,
    stem_hash: u64,
    shader_source_generation: u64,
}

struct OwnedTexturePools {
    texture: TexturePool,
    texture3d: Texture3dPool,
    cubemap: CubemapPool,
    render_texture: RenderTexturePool,
    video_texture: VideoTexturePool,
}

struct PreparedMaterial {
    reflected: ReflectedRasterLayout,
    ids: StemEmbeddedPropertyIds,
    value_spaces: MaterialUniformValueSpaces,
    defaults: MaterialUniformDefaults,
    store: MaterialPropertyStore,
    pools: OwnedTexturePools,
    stem_hash: u64,
    shader_source_generation: u64,
    color_pid: i32,
    block_size: u64,
}

impl MaterialUniformUpdateWorkload {
    /// Installs the bench custom material and primes one uniform slot per material.
    #[must_use]
    pub fn standard() -> Self {
        match PreparedMaterial::prepare(MATERIAL_UNIFORM_BATCH) {
            Ok(prepared) => match UniformInner::from_prepared(prepared) {
                Ok(inner) => Self {
                    inner: Some(inner),
                    setup_error: None,
                },
                Err(error) => Self::failed(error),
            },
            Err(error) => Self::failed(error),
        }
    }

    /// Why [`Self::standard`] did not build a workload, when setup failed.
    #[must_use]
    pub fn setup_error(&self) -> Option<&str> {
        self.setup_error.as_deref()
    }

    /// Writes a new `_Color` on every material, then packs and stages the uniform block.
    ///
    /// Slots are primed, so each sample takes the dirty-slot rewrite path: arena
    /// `needs_write`, [`build_embedded_uniform_bytes_with_material_defaults`], and a copy
    /// into the frame upload arena.
    #[must_use]
    pub fn pack_changed_properties(&mut self) -> u64 {
        let Some(inner) = self.inner.as_mut() else {
            return 0;
        };
        inner.pack_changed()
    }

    fn failed(error: String) -> Self {
        Self {
            inner: None,
            setup_error: Some(error),
        }
    }
}

impl UniformInner {
    fn from_prepared(prepared: PreparedMaterial) -> Result<Self, String> {
        let PreparedMaterial {
            reflected,
            ids,
            value_spaces,
            defaults,
            store,
            pools,
            stem_hash,
            color_pid,
            block_size,
            ..
        } = prepared;
        let mut arena = CpuMaterialUniformArena::for_slots(block_size, MATERIAL_UNIFORM_BATCH);
        for index in 0..MATERIAL_UNIFORM_BATCH {
            let material_id = material_id(index);
            let lookup = lookup_for(material_id);
            let primary = primary_texture_2d_asset_id(&reflected, &ids, &store, lookup);
            let generation = store.mutation_generation(lookup);
            arena
                .prime(stem_hash, material_id, primary, generation, 0)
                .map_err(|err| err.to_string())?;
        }
        Ok(Self {
            reflected,
            ids,
            value_spaces,
            defaults,
            store,
            pools,
            arena,
            uploads: FrameUploadBatch::new(),
            stem_hash,
            color_pid,
            tick: 0,
        })
    }

    fn pack_changed(&mut self) -> u64 {
        let mut checksum = 0_u64;
        for index in 0..MATERIAL_UNIFORM_BATCH {
            checksum = checksum.wrapping_add(self.pack_index(index));
        }
        self.uploads.clear_staged_cpu_payload();
        self.tick = self.tick.wrapping_add(1);
        checksum
    }

    fn pack_index(&mut self, index: usize) -> u64 {
        let material_id = material_id(index);
        let lookup = lookup_for(material_id);
        self.store.set_material(
            material_id,
            self.color_pid,
            MaterialPropertyValue::Float4(color_for(self.tick, index)),
        );
        let mutation_gen = self.store.mutation_generation(lookup);
        let primary = primary_texture_2d_asset_id(&self.reflected, &self.ids, &self.store, lookup);
        let Ok(decision) =
            self.arena
                .begin_rewrite(self.stem_hash, material_id, primary, mutation_gen, 0)
        else {
            return 0;
        };
        let Some(offset) = decision else {
            return 0;
        };
        let Some(bytes) = self.pack_current(lookup, primary) else {
            return 0;
        };
        let staged = u64::try_from(self.uploads.stage_cpu_payload(&bytes)).unwrap_or(0);
        self.arena.finish_rewrite(mutation_gen, 0);
        staged
            .wrapping_add(u64::from(offset))
            .wrapping_add(fold_uniform_bytes(&bytes))
    }

    fn pack_current(&self, lookup: MaterialPropertyLookupIds, primary: i32) -> Option<Vec<u8>> {
        let pools = self.pools.view();
        let tex_ctx = UniformPackTextureContext {
            pools: &pools,
            primary_texture_2d: primary,
        };
        build_embedded_uniform_bytes_with_material_defaults(
            &self.reflected,
            &self.ids,
            &MaterialUniformPackMetadata {
                value_spaces: &self.value_spaces,
                material_defaults: &self.defaults,
            },
            &self.store,
            lookup,
            &tex_ctx,
            None,
        )
    }
}

impl BindGroupChurnWorkload {
    /// Installs the bench custom material and seeds the bind-group LRU.
    #[must_use]
    pub fn standard() -> Self {
        match PreparedMaterial::prepare(BIND_GROUP_BATCH) {
            Ok(prepared) => {
                let inner = BindInner::from_prepared(prepared);
                Self {
                    inner: Some(inner),
                    setup_error: None,
                }
            }
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

    /// Recomputes each texture signature and bind key, then hits the resident LRU entry.
    #[must_use]
    pub fn cache_hits(&self) -> u64 {
        let Some(inner) = self.inner.as_ref() else {
            return 0;
        };
        inner.cache_hits()
    }

    /// Clears the bind-group LRU, then misses and inserts the working set again.
    ///
    /// This is the CPU half of `invalidate_stem_layout` / `clear_bind_cache` followed by the
    /// next frame's cache-miss rebuild. Bind-group object creation stays on the GPU path.
    #[must_use]
    pub fn miss_rebuild(&self) -> u64 {
        let Some(inner) = self.inner.as_ref() else {
            return 0;
        };
        inner.miss_rebuild()
    }
}

impl BindInner {
    fn from_prepared(prepared: PreparedMaterial) -> Self {
        let PreparedMaterial {
            reflected,
            ids,
            store,
            pools,
            stem_hash,
            shader_source_generation,
            ..
        } = prepared;
        let inner = Self {
            reflected,
            ids,
            store,
            pools,
            cache: CpuMaterialBindGroupCache::new(),
            stem_hash,
            shader_source_generation,
        };
        inner.seed();
        inner
    }

    fn cache_hits(&self) -> u64 {
        let mut checksum = 0_u64;
        for index in 0..BIND_GROUP_BATCH {
            let signature = self.signature(index);
            let key = self.key(index, signature);
            if let Some(token) = self.cache.lookup_hit(&key) {
                checksum = checksum.wrapping_add(token).wrapping_add(1);
            }
        }
        checksum
    }

    fn miss_rebuild(&self) -> u64 {
        let dropped = self.cache.invalidate();
        let mut checksum = u64::try_from(dropped).unwrap_or(0);
        for index in 0..BIND_GROUP_BATCH {
            let signature = self.signature(index);
            let key = self.key(index, signature);
            checksum = checksum.wrapping_add(self.cache.rebuild_miss(key, signature));
        }
        checksum
    }

    fn seed(&self) {
        for index in 0..BIND_GROUP_BATCH {
            let signature = self.signature(index);
            let key = self.key(index, signature);
            let _token = self.cache.rebuild_miss(key, signature);
        }
    }

    fn signature(&self, index: usize) -> u64 {
        let lookup = lookup_for(material_id(index));
        let pools = self.pools.view();
        let primary = primary_texture_2d_asset_id(&self.reflected, &self.ids, &self.store, lookup);
        texture_bind_signature(
            &self.reflected,
            &self.ids,
            &self.store,
            lookup,
            &pools,
            primary,
            OffscreenWriteTarget::None,
        )
    }

    fn key(&self, index: usize, signature: u64) -> super::material_bind::MaterialBindCacheKey {
        material_bind_cache_key_for_bench(
            self.stem_hash,
            lookup_for(material_id(index)),
            signature,
            OffscreenWriteTarget::None,
            Some(SETTLED_ARENA_GENERATION),
            self.shader_source_generation,
        )
    }
}

impl PreparedMaterial {
    fn prepare(count: usize) -> Result<Self, String> {
        let installed = install(CUSTOM_SHADER_KEY, BENCH_CUSTOM_MATERIAL_WGSL)?;
        let wgsl = custom_shader::target_wgsl(&installed.default_stem)
            .ok_or_else(|| format!("custom stem {} has no WGSL", installed.default_stem))?;
        let reflected =
            reflect_raster_material_wgsl(wgsl.as_ref()).map_err(|err| err.to_string())?;
        let block_size = reflected
            .material_uniform
            .as_ref()
            .map(|uniform| u64::from(uniform.total_size))
            .ok_or_else(|| format!("stem {} has no material uniform", installed.default_stem))?;
        if block_size == 0 {
            return Err(format!(
                "stem {} reflected a zero-sized material uniform",
                installed.default_stem
            ));
        }
        let registry = PropertyIdRegistry::new();
        let ids = StemEmbeddedPropertyIds::build(&registry, &reflected);
        let color_pid = *ids
            .uniform_field_ids
            .get("_Color")
            .ok_or_else(|| format!("stem {} did not reflect _Color", installed.default_stem))?;
        let cutoff_pid = ids.uniform_field_ids.get("_Cutoff").copied();
        let smoothness_pid = ids.uniform_field_ids.get("_Smoothness").copied();
        let tex_pid = ids
            .texture_binding_property_ids
            .get(&TEX_BINDING)
            .and_then(|pids| pids.first().copied());
        let mask_pid = ids
            .texture_binding_property_ids
            .get(&MASK_BINDING)
            .and_then(|pids| pids.first().copied());
        let value_spaces =
            MaterialUniformValueSpaces::for_stem(&installed.default_stem, &reflected);
        let defaults = material_uniform_defaults_for_stem(&installed.default_stem, &reflected)?;
        let mut store = MaterialPropertyStore::default();
        for index in 0..count {
            let material_id = material_id(index);
            store.set_material(
                material_id,
                color_pid,
                MaterialPropertyValue::Float4(color_for(0, index)),
            );
            if let Some(cutoff_pid) = cutoff_pid {
                store.set_material(material_id, cutoff_pid, MaterialPropertyValue::Float(0.5));
            }
            if let Some(smoothness_pid) = smoothness_pid {
                store.set_material(
                    material_id,
                    smoothness_pid,
                    MaterialPropertyValue::Float(0.35),
                );
            }
            if let Some(tex_pid) = tex_pid {
                store.set_material(
                    material_id,
                    tex_pid,
                    MaterialPropertyValue::Texture(material_id),
                );
            }
            if let Some(mask_pid) = mask_pid {
                store.set_material(
                    material_id,
                    mask_pid,
                    MaterialPropertyValue::Texture(material_id.saturating_add(10_000)),
                );
            }
        }
        Ok(Self {
            stem_hash: stem_hash(&installed.default_stem),
            shader_source_generation: custom_shader::source_generation(&installed.default_stem),
            reflected,
            ids,
            value_spaces,
            defaults,
            store,
            pools: OwnedTexturePools::empty(),
            color_pid,
            block_size,
        })
    }
}

impl OwnedTexturePools {
    fn empty() -> Self {
        Self {
            texture: TexturePool::default_pool(),
            texture3d: Texture3dPool::default_pool(),
            cubemap: CubemapPool::default_pool(),
            render_texture: RenderTexturePool::new(),
            video_texture: VideoTexturePool::new(),
        }
    }

    fn view(&self) -> EmbeddedTexturePools<'_> {
        EmbeddedTexturePools {
            texture: &self.texture,
            texture3d: &self.texture3d,
            cubemap: &self.cubemap,
            render_texture: &self.render_texture,
            video_texture: &self.video_texture,
        }
    }
}

fn lookup_for(material_id: i32) -> MaterialPropertyLookupIds {
    MaterialPropertyLookupIds {
        material_asset_id: material_id,
        mesh_property_block_slot0: None,
        mesh_renderer_property_block_id: None,
    }
}

fn material_id(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX).saturating_add(1)
}

fn color_for(tick: u32, index: usize) -> [f32; 4] {
    let step = u16::try_from(index % 10_000).unwrap_or(0);
    let tick_step = u16::try_from(tick % 997).unwrap_or(0);
    let red = f32::from(tick_step) * 0.001 + f32::from(step) * 0.000_01;
    [red, 0.35, 0.1, 1.0]
}

fn fold_uniform_bytes(bytes: &[u8]) -> u64 {
    let mut acc = u64::try_from(bytes.len()).unwrap_or(0);
    if let Some(head) = bytes.get(..4) {
        let raw = <[u8; 4]>::try_from(head).unwrap_or([0; 4]);
        acc = acc.wrapping_add(u64::from(u32::from_le_bytes(raw)));
    }
    acc
}
