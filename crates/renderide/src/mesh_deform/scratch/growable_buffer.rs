//! GPU buffer recipe with on-demand power-of-two growth for per-frame mesh deform scratch slabs.
//!
//! The pattern is shared by every [`super::MeshDeformScratch`] buffer: a static descriptor names
//! the buffer's label, usage flags, and minimum size; [`GrowableBuffer::create`] allocates it; and
//! [`GrowableBuffer::ensure`] replaces it with a larger buffer when a write exceeds capacity.

/// Static description of a growable scratch buffer: label, usage, and minimum size floor.
///
/// Centralises the per-buffer recipe so [`super::MeshDeformScratch::new`] and the [`Self::ensure`]
/// growth path share one buffer descriptor and one log message format.
pub(super) struct GrowableBuffer {
    pub(super) label: &'static str,
    pub(super) usage: wgpu::BufferUsages,
    /// Floor below which the buffer is never sized. Matches the `.max(N)` literals from the
    /// per-call buffer descriptors.
    pub(super) min_size: u64,
}

impl GrowableBuffer {
    /// Creates a new GPU buffer sized to `max(requested, self.min_size)`.
    pub(super) fn create(&self, device: &wgpu::Device, requested: u64) -> wgpu::Buffer {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(self.label),
            size: requested.max(self.min_size),
            usage: self.usage,
            mapped_at_creation: false,
        });
        crate::profiling::note_resource_churn!(Buffer, "mesh_deform::scratch_buffer");
        buffer
    }

    /// Ensures `buf` is at least `need_bytes` long, growing to the next power of two.
    ///
    /// Returns `false` (and logs a warning) when growth would exceed `max_buffer_size`. The buffer
    /// is left unchanged in that case so the caller can fall back to a smaller dispatch.
    pub(super) fn ensure(
        &self,
        device: &wgpu::Device,
        buf: &mut wgpu::Buffer,
        need_bytes: u64,
        max_buffer_size: u64,
    ) -> bool {
        if need_bytes <= buf.size() {
            return true;
        }
        let next = need_bytes.next_power_of_two().max(self.min_size);
        if next > max_buffer_size {
            logger::warn!(
                "mesh deform scratch: {} would need {} bytes (max_buffer_size={})",
                self.label,
                next,
                max_buffer_size
            );
            return false;
        }
        *buf = self.create(device, next);
        true
    }
}

/// Bytes per skinning palette matrix (column-major `mat4`).
pub(super) const BONE_MATRIX_BYTES: u64 = 64;

/// Bone palette: storage + copy-dst, sized to a single `mat4` minimum so an empty palette is still
/// addressable.
pub(super) const BONE_MATRICES: GrowableBuffer = GrowableBuffer {
    label: "mesh_deform_bone_palette",
    usage: wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST),
    min_size: BONE_MATRIX_BYTES,
};

/// Blendshape scatter params: 32-byte dynamic-uniform slots packed back-to-back.
pub(super) const BLENDSHAPE_PARAMS: GrowableBuffer = GrowableBuffer {
    label: "mesh_deform_blendshape_params",
    usage: wgpu::BufferUsages::UNIFORM.union(wgpu::BufferUsages::COPY_DST),
    min_size: 32,
};

/// Skin dispatch params: 256-byte aligned dynamic-uniform slots, one per skinning dispatch.
pub(super) const SKIN_DISPATCH: GrowableBuffer = GrowableBuffer {
    label: "mesh_deform_skin_dispatch",
    usage: wgpu::BufferUsages::UNIFORM.union(wgpu::BufferUsages::COPY_DST),
    min_size: 256,
};

/// Dummy read-only storage used when an optional shader input attribute path is disabled.
pub(super) const DUMMY_VEC4_READ: GrowableBuffer = GrowableBuffer {
    label: "mesh_deform_dummy_vec4_read",
    usage: wgpu::BufferUsages::STORAGE,
    min_size: 16,
};

/// Dummy writable storage used when an optional shader output attribute path is disabled.
pub(super) const DUMMY_VEC4_WRITE: GrowableBuffer = GrowableBuffer {
    label: "mesh_deform_dummy_vec4_write",
    usage: wgpu::BufferUsages::STORAGE,
    min_size: 16,
};
