//! Fills one dynamic uniform slot while the arena shard lock is held.

use std::sync::Arc;

use super::super::super::embedded_material_bind_error::EmbeddedMaterialBindError;
use super::super::super::layout::StemMaterialLayout;
use super::super::super::texture_pools::EmbeddedTexturePools;
use super::super::super::uniform_pack::{
    MaterialUniformPackMetadata, UniformPackTextureContext,
    build_embedded_uniform_bytes_with_material_defaults,
};
use super::super::EmbeddedMaterialBindResources;
use super::arena::non_zero_u64;
use super::key::{MaterialUniformArenaSlotBinding, MaterialUniformCacheKey};
use crate::frame_upload_batch::GraphUploadSink;
use crate::materials::host_data::{MaterialPropertyLookupIds, MaterialPropertyStore};

/// Uniform arena lookup and upload request for [`super::super::EmbeddedMaterialBindResources`].
pub(in super::super) struct EmbeddedUniformArenaRequest<'a> {
    pub(in super::super) uploads: GraphUploadSink<'a>,
    pub(in super::super) stem: &'a str,
    pub(in super::super) shader_variant_bits: Option<u32>,
    pub(in super::super) layout: &'a Arc<StemMaterialLayout>,
    pub(in super::super) uniform_key: &'a MaterialUniformCacheKey,
    pub(in super::super) mutation_gen: u64,
    pub(in super::super) store: &'a MaterialPropertyStore,
    pub(in super::super) lookup: MaterialPropertyLookupIds,
    pub(in super::super) pools: &'a EmbeddedTexturePools<'a>,
    pub(in super::super) primary_texture_2d: i32,
    pub(in super::super) texture_state_sig: u64,
}

impl EmbeddedMaterialBindResources {
    /// Opens the uniform-arena recording frame. Call once before graph recording.
    pub(crate) fn begin_uniform_arena_frame(&self) {
        self.uniform_retire_clock.begin_frame();
    }

    /// Callback for `Queue::on_submitted_work_done`. Only records the open frame id.
    ///
    /// Pending ranges are promoted later on the recording thread. Doing wgpu work inside
    /// this callback can deadlock against `queue.write_buffer`.
    pub(crate) fn uniform_arena_submission_callback(
        &self,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        let frame = self.uniform_retire_clock.open_frame();
        if frame == 0 {
            return None;
        }
        let clock = Arc::clone(&self.uniform_retire_clock);
        Some(Box::new(move || {
            clock.mark_completed(frame);
        }))
    }
}

impl EmbeddedMaterialBindResources {
    /// Shared dynamic uniform arena slot for embedded `@group(1)` material constants.
    ///
    /// Refreshes bytes when [`MaterialPropertyStore`] mutates, when texture-derived uniform state
    /// changes, or when the arena grows to a new backing buffer generation.
    ///
    /// `resolve_binding`, the uniform byte build, the `GraphUploadSink::write_buffer` call, and
    /// `mark_written` all run under one arena shard lock. Splitting them across two critical
    /// sections allowed `slot.last_written_(generation|texture_state_sig)` to record a
    /// `(mutation_gen, texture_state_sig)` snapshot that disagreed with the bytes actually
    /// uploaded if any concurrent caller mutated relevant state between the resolve and the
    /// mark - a later cache hit could then skip a needed rewrite and let one material draw
    /// with another material's uniforms. `write_buffer` is a deferred queue push that does no
    /// GPU synchronization, so holding the shard lock across it is cheap.
    ///
    /// The arena is sharded by [`MaterialUniformCacheKey`] so concurrent rayon recording workers
    /// hitting distinct keys land on distinct shards and never block one another. A given key
    /// always routes to the same shard, so the per-shard slot table and `buffer_generation`
    /// counter remain self-consistent.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the arena shard lock is intentionally held across resolve_binding, the uniform byte build, write_buffer, and mark_written so slot tracking cannot disagree with the uploaded bytes"
    )]
    pub(in super::super) fn get_or_update_embedded_uniform_arena_slot(
        &self,
        req: EmbeddedUniformArenaRequest<'_>,
    ) -> Result<MaterialUniformArenaSlotBinding, EmbeddedMaterialBindError> {
        profiling::scope!("materials::embedded_uniform_arena");
        let EmbeddedUniformArenaRequest {
            uploads,
            stem,
            shader_variant_bits,
            layout,
            uniform_key,
            mutation_gen,
            store,
            lookup,
            pools,
            primary_texture_2d,
            texture_state_sig,
        } = req;
        let uniform_size = non_zero_u64(
            layout
                .reflected
                .material_uniform
                .as_ref()
                .ok_or_else(|| {
                    EmbeddedMaterialBindError::from(format!(
                        "stem {stem}: uniform block missing (shader has no material uniform)"
                    ))
                })?
                .total_size
                .into(),
        )?;
        let tex_ctx = UniformPackTextureContext {
            pools,
            primary_texture_2d,
        };

        profiling::scope!("materials::embedded_uniform_arena_critical_section");
        let mut arena = self.uniform_arena_shard(uniform_key).lock();
        let (binding, needs_write) =
            arena.resolve_binding(*uniform_key, uniform_size, mutation_gen, texture_state_sig)?;
        if needs_write {
            profiling::scope!("materials::embedded_uniform_arena_write");
            let quantize_toggle_floats =
                stem.starts_with(crate::materials::custom_shader::CUSTOM_STEM_PREFIX);
            if let Some(plan) = layout.uniform_pack_plan.as_ref() {
                if !plan.pack_into(
                    store,
                    lookup,
                    &tex_ctx,
                    shader_variant_bits,
                    &mut arena.pack_scratch,
                ) {
                    return Err(EmbeddedMaterialBindError::from(format!(
                        "stem {stem}: uniform block missing (shader has no material uniform)"
                    )));
                }
                uploads.write_buffer(
                    binding.buffer.as_ref(),
                    u64::from(binding.dynamic_offset),
                    &arena.pack_scratch,
                );
                diag_log_material_bytes(
                    stem,
                    layout,
                    &arena.pack_scratch,
                    binding.dynamic_offset,
                    shader_variant_bits,
                );
            } else {
                let uniform_bytes = build_embedded_uniform_bytes_with_material_defaults(
                    &layout.reflected,
                    layout.ids.as_ref(),
                    &MaterialUniformPackMetadata {
                        value_spaces: &layout.uniform_value_spaces,
                        material_defaults: &layout.uniform_default_by_field,
                        quantize_toggle_floats,
                    },
                    store,
                    lookup,
                    &tex_ctx,
                    shader_variant_bits,
                )
                .ok_or_else(|| {
                    format!("stem {stem}: uniform block missing (shader has no material uniform)")
                })?;
                uploads.write_buffer(
                    binding.buffer.as_ref(),
                    u64::from(binding.dynamic_offset),
                    &uniform_bytes,
                );
                diag_log_material_bytes(
                    stem,
                    layout,
                    &uniform_bytes,
                    binding.dynamic_offset,
                    shader_variant_bits,
                );
            }
            arena.mark_written(uniform_key, &binding, mutation_gen, texture_state_sig);
        } else {
            profiling::scope!("materials::embedded_uniform_arena_hit");
        }
        Ok(binding)
    }
}

fn diag_log_material_bytes(
    stem: &str,
    layout: &StemMaterialLayout,
    bytes: &[u8],
    dynamic_offset: u32,
    variant_bits: Option<u32>,
) {
    let frame_log = crate::gpu::warp_black_diag::frame_log_armed();
    let bind_probe = crate::gpu::warp_material_bind_probe::armed();
    if !frame_log && !bind_probe {
        return;
    }
    if bind_probe {
        crate::gpu::warp_material_bind_probe::note_uniform(stem, dynamic_offset, bytes);
    }
    if !frame_log {
        return;
    }
    let fields = layout
        .reflected
        .material_uniform
        .as_ref()
        .map(|block| &block.fields);
    let offset_of =
        |name: &str| fields.and_then(|fields| fields.get(name).map(|field| field.offset));
    crate::gpu::warp_black_diag_probe::log_material_bytes(
        &crate::gpu::warp_black_diag_probe::MaterialBytes {
            stem,
            bytes,
            dynamic_offset,
            variant_bits,
            color_offset: offset_of("_Color"),
            metallic_offset: offset_of("_Metallic"),
            gloss_offset: offset_of("_Glossiness"),
            variant_offset: offset_of("_RenderideVariantBits"),
        },
    );
}
