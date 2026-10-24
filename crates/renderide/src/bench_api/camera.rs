//! Camera clear load-op and render-texture double-buffer workloads.

use crate::gpu_pools::pools::render_texture::{claim_double_buffer_swap, swap_ping_pong};

/// Double-buffered cameras swapped by one [`DoubleBufferWorkload::steady_frame`] sample.
pub const DOUBLE_BUFFER_CAMERAS: usize = 32;

/// Cameras folded by one [`camera_clear_load_op_checksum`] sample.
pub const CAMERA_CLEAR_LOAD_OP_CAMERAS: usize = crate::passes::CAMERA_CLEAR_LOAD_OP_CAMERAS;

/// One synthetic camera for [`camera_clear_load_op_checksum`].
pub use crate::passes::CameraClearLoadOpInput;

/// Seeded, non-periodic load-op inputs. Build this once, outside the timed loop.
#[must_use]
pub fn camera_clear_load_op_inputs(cameras: usize) -> Vec<CameraClearLoadOpInput> {
    crate::passes::camera_clear_load_op_inputs(cameras)
}

/// Allocation-free checksum of per-camera color and depth load ops.
///
/// Each input is passed through [`std::hint::black_box`] inside the loop.
#[must_use]
pub fn camera_clear_load_op_checksum(inputs: &[CameraClearLoadOpInput]) -> u32 {
    crate::passes::camera_clear_load_op_checksum(inputs)
}

/// Per-frame double-buffer bookkeeping for cameras that already own a second color image.
///
/// The measured path is the same claim-once scan and three-slot swap the renderer runs after the
/// history image exists. It does not create wgpu textures; that allocation happens once, outside
/// the steady frame.
pub struct DoubleBufferWorkload {
    scratch: Vec<i32>,
    published_texture: Vec<u64>,
    previous_texture: Vec<u64>,
    published_view: Vec<u64>,
    previous_view: Vec<u64>,
    published_generation: Vec<u64>,
    previous_generation: Vec<u64>,
}

impl DoubleBufferWorkload {
    /// Builds `DOUBLE_BUFFER_CAMERAS` already-allocated ping-pong slots and reserves the scratch.
    #[must_use]
    pub fn standard() -> Self {
        let cameras = DOUBLE_BUFFER_CAMERAS;
        let mut scratch = Vec::with_capacity(cameras);
        let mut published_texture = Vec::with_capacity(cameras);
        let mut previous_texture = Vec::with_capacity(cameras);
        let mut published_view = Vec::with_capacity(cameras);
        let mut previous_view = Vec::with_capacity(cameras);
        let mut published_generation = Vec::with_capacity(cameras);
        let mut previous_generation = Vec::with_capacity(cameras);
        for index in 0..cameras {
            let id = u64::try_from(index).unwrap_or(0);
            published_texture.push(id);
            previous_texture.push(id.wrapping_add(1_000));
            published_view.push(id.wrapping_add(2_000));
            previous_view.push(id.wrapping_add(3_000));
            published_generation.push(id.wrapping_add(4_000));
            previous_generation.push(id.wrapping_add(5_000));
        }
        // Pay the scratch growth before the measured loop.
        for index in 0..cameras {
            let asset_id = i32::try_from(index).unwrap_or(0);
            let _ = claim_double_buffer_swap(&mut scratch, asset_id);
        }
        scratch.clear();
        Self {
            scratch,
            published_texture,
            previous_texture,
            published_view,
            previous_view,
            published_generation,
            previous_generation,
        }
    }

    /// Claims each camera once and swaps its three slots. Returns the published generation sum.
    #[must_use]
    pub fn steady_frame(&mut self) -> u64 {
        self.scratch.clear();
        let mut checksum = 0_u64;
        for index in 0..self.published_generation.len() {
            let asset_id = i32::try_from(index).unwrap_or(0);
            if !claim_double_buffer_swap(&mut self.scratch, asset_id) {
                continue;
            }
            swap_ping_pong(
                &mut self.published_texture[index],
                &mut self.previous_texture[index],
            );
            swap_ping_pong(
                &mut self.published_view[index],
                &mut self.previous_view[index],
            );
            swap_ping_pong(
                &mut self.published_generation[index],
                &mut self.previous_generation[index],
            );
            checksum = checksum.wrapping_add(self.published_generation[index]);
        }
        checksum
    }

    /// Capacity of the reused id scratch after [`Self::standard`].
    #[must_use]
    pub fn scratch_capacity(&self) -> usize {
        self.scratch.capacity()
    }
}
