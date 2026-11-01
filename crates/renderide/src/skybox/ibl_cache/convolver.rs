//! Standalone GGX convolver for caller-owned cubemap textures.

use std::sync::Arc;

use crate::gpu::GpuContext;
use crate::profiling::GpuProfilerHandle;

use super::encode::{
    ConvolveEncodeContext, DownsampleEncodeContext, StitchEncodeContext, encode_convolve_mips,
    encode_downsample_mips, encode_stitch_mip,
};
use super::errors::SkyboxIblConvolveError;
use super::key::source_max_lod;
use super::pipeline::ComputePipeline;
use super::pipeline_store::{PipelineSlot, PipelineStore};
use super::resources::{
    PendingBakeResources, copy_cube_mip0, create_full_array_sample_view, create_ibl_cube,
};

/// Resources produced while encoding convolve passes and retained until submit completion.
pub(crate) struct SkyboxIblConvolveResources {
    _resources: PendingBakeResources,
    _source_sample_view: Arc<wgpu::TextureView>,
    _sampler: Arc<wgpu::Sampler>,
    _pipelines: Vec<wgpu::ComputePipeline>,
}

impl SkyboxIblConvolveResources {
    /// Moves every handle the encoded passes reference into a driver-thread retain set.
    ///
    /// The batch may still be queued when the caller times out and drops the convolver's
    /// stack frame. The driver thread drops this set only after `Queue::submit` returns.
    pub(crate) fn into_retained(self) -> crate::gpu::GpuRetainedResources {
        let mut retained = crate::gpu::GpuRetainedResources::new();
        for texture in self._resources.textures {
            retained.retain_texture(texture.as_ref().clone());
        }
        retained.retain_buffers(self._resources.buffers);
        for bind_group in self._resources.bind_groups {
            retained.retain_bind_group(bind_group);
        }
        retained.retain_texture_views(self._resources.texture_views);
        for view in self._resources.source_views {
            retained.retain_texture_view(view.as_ref().clone());
        }
        if let Some(view) = self._resources.source_sample_view {
            retained.retain_texture_view(view.as_ref().clone());
        }
        retained.retain_texture_view(self._source_sample_view.as_ref().clone());
        retained.retain_sampler(self._sampler.as_ref().clone());
        for pipeline in self._pipelines {
            retained.retain_compute_pipeline(pipeline);
        }
        retained
    }
}

/// Texture set used while convolving a caller-owned cubemap.
struct ConvolverTextures {
    /// Stitched source radiance mip pyramid.
    source_cube: super::resources::IblCubeTexture,
    /// Scratch target for source mip generation before stitching.
    source_scratch_cube: super::resources::IblCubeTexture,
    /// Scratch target for filtered mip generation before stitching.
    filtered_scratch_cube: super::resources::IblCubeTexture,
    /// Full-mip 2D-array view of [`Self::source_cube`].
    source_sample_view: Arc<wgpu::TextureView>,
}

impl ConvolverTextures {
    /// Allocates transient textures and a sample view for one convolve operation.
    fn create(device: &wgpu::Device, face_size: u32, mip_levels: u32) -> Self {
        let source_cube = create_ibl_cube(
            device,
            "skybox_ibl_existing_source_cube",
            face_size,
            mip_levels,
        );
        let source_scratch_cube = create_ibl_cube(
            device,
            "skybox_ibl_existing_source_scratch_cube",
            face_size,
            mip_levels,
        );
        let filtered_scratch_cube = create_ibl_cube(
            device,
            "skybox_ibl_existing_filtered_scratch_cube",
            face_size,
            mip_levels,
        );
        let source_sample_view = Arc::new(create_full_array_sample_view(
            source_cube.texture.as_ref(),
            mip_levels,
        ));
        Self {
            source_cube,
            source_scratch_cube,
            filtered_scratch_cube,
            source_sample_view,
        }
    }

    /// Retains transient textures and views until the caller's submit completes.
    fn retain_transient(&self, resources: &mut PendingBakeResources) {
        resources.textures.push(self.source_cube.texture.clone());
        resources
            .textures
            .push(self.source_scratch_cube.texture.clone());
        resources
            .textures
            .push(self.filtered_scratch_cube.texture.clone());
        resources.source_sample_view = Some(self.source_sample_view.clone());
    }
}

/// Pipelines and sampler a caller-owned cubemap convolve records into.
struct PreparedConvolvePipelines<'a> {
    sampler: Arc<wgpu::Sampler>,
    convolve: &'a ComputePipeline,
    downsample: &'a ComputePipeline,
    stitch: &'a ComputePipeline,
}

/// Minimal GGX convolver for caller-owned cubemap textures.
#[derive(Default)]
pub(crate) struct SkyboxIblConvolver {
    pipelines: PipelineStore,
}

impl SkyboxIblConvolver {
    /// Creates an empty convolver with lazily-built GPU resources.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Builds the convolve, downsample, and stitch pipelines before recording.
    fn prepare_convolve_pipelines(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<PreparedConvolvePipelines<'_>, SkyboxIblConvolveError> {
        let sampler = self.pipelines.ensure_sampler(device);
        self.ensure_convolve_slot(PipelineSlot::Convolve, device, "skybox_ibl_convolve_params")?;
        self.ensure_convolve_slot(PipelineSlot::Downsample, device, "skybox_ibl_downsample")?;
        self.ensure_convolve_slot(PipelineSlot::Stitch, device, "skybox_ibl_stitch")?;
        Ok(PreparedConvolvePipelines {
            sampler,
            convolve: self.convolve_slot(PipelineSlot::Convolve, "skybox_ibl_convolve_params")?,
            downsample: self.convolve_slot(PipelineSlot::Downsample, "skybox_ibl_downsample")?,
            stitch: self.convolve_slot(PipelineSlot::Stitch, "skybox_ibl_stitch")?,
        })
    }

    fn ensure_convolve_slot(
        &mut self,
        slot: PipelineSlot,
        device: &wgpu::Device,
        shader: &'static str,
    ) -> Result<(), SkyboxIblConvolveError> {
        self.pipelines
            .ensure(slot, device)
            .map_err(|_err| SkyboxIblConvolveError::MissingShader(shader))?;
        Ok(())
    }

    fn convolve_slot(
        &self,
        slot: PipelineSlot,
        shader: &'static str,
    ) -> Result<&ComputePipeline, SkyboxIblConvolveError> {
        self.pipelines
            .get(slot)
            .map_err(|_err| SkyboxIblConvolveError::MissingShader(shader))
    }

    /// Encodes GGX convolve passes for mips `1..mip_levels` of `texture`.
    pub(crate) fn encode_existing_cube_mips(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        face_size: u32,
        mip_levels: u32,
        profiler: Option<&GpuProfilerHandle>,
    ) -> Result<SkyboxIblConvolveResources, SkyboxIblConvolveError> {
        profiling::scope!("skybox_ibl::encode_existing_cube_mips");
        let prepared = self.prepare_convolve_pipelines(gpu.device())?;

        let textures = ConvolverTextures::create(gpu.device(), face_size, mip_levels);
        let mut resources = PendingBakeResources::default();
        textures.retain_transient(&mut resources);
        copy_cube_mip0(
            encoder,
            texture,
            textures.source_scratch_cube.texture.as_ref(),
            face_size,
            profiler,
        );
        encode_stitch_mip(
            StitchEncodeContext {
                device: gpu.device(),
                encoder,
                pipeline: prepared.stitch,
                src_texture: textures.source_scratch_cube.texture.as_ref(),
                dst_texture: textures.source_cube.texture.as_ref(),
                mip: 0,
                dst_size: face_size,
                profiler,
                label: "skybox_ibl stitch existing source mip0",
                profiler_label: "skybox_ibl::stitch_existing_source_mip0".to_string(),
            },
            &mut resources,
        );
        encode_stitch_mip(
            StitchEncodeContext {
                device: gpu.device(),
                encoder,
                pipeline: prepared.stitch,
                src_texture: textures.source_cube.texture.as_ref(),
                dst_texture: texture,
                mip: 0,
                dst_size: face_size,
                profiler,
                label: "skybox_ibl stitch existing output mip0",
                profiler_label: "skybox_ibl::stitch_existing_output_mip0".to_string(),
            },
            &mut resources,
        );
        encode_downsample_mips(
            DownsampleEncodeContext {
                device: gpu.device(),
                encoder,
                pipeline: prepared.downsample,
                stitch_pipeline: prepared.stitch,
                texture: textures.source_cube.texture.as_ref(),
                scratch_texture: textures.source_scratch_cube.texture.as_ref(),
                face_size,
                mip_levels,
                profiler,
            },
            &mut resources,
        );
        encode_convolve_mips(
            ConvolveEncodeContext {
                device: gpu.device(),
                encoder,
                pipeline: prepared.convolve,
                stitch_pipeline: prepared.stitch,
                texture,
                scratch_texture: textures.filtered_scratch_cube.texture.as_ref(),
                src_view: textures.source_sample_view.as_ref(),
                sampler: prepared.sampler.as_ref(),
                face_size,
                mip_levels,
                src_max_lod: source_max_lod(mip_levels),
                profiler,
            },
            &mut resources,
        );
        Ok(SkyboxIblConvolveResources {
            _resources: resources,
            _source_sample_view: textures.source_sample_view,
            _sampler: prepared.sampler,
            _pipelines: vec![
                prepared.convolve.pipeline.clone(),
                prepared.downsample.pipeline.clone(),
                prepared.stitch.pipeline.clone(),
            ],
        })
    }
}
