//! Compute dispatch and workgroup caps.

use super::GpuLimits;

impl GpuLimits {
    /// `max_compute_workgroups_per_dimension` for dispatch validation.
    #[inline]
    pub fn max_compute_workgroups_per_dimension(&self) -> u32 {
        self.wgpu.max_compute_workgroups_per_dimension
    }

    /// `max_compute_invocations_per_workgroup` for shader workgroup-size validation.
    #[cfg(test)]
    #[inline]
    pub fn max_compute_invocations_per_workgroup(&self) -> u32 {
        self.wgpu.max_compute_invocations_per_workgroup
    }

    /// `max_compute_workgroup_size_x` for shader workgroup-size validation.
    #[cfg(test)]
    #[inline]
    pub fn max_compute_workgroup_size_x(&self) -> u32 {
        self.wgpu.max_compute_workgroup_size_x
    }

    /// `max_compute_workgroup_size_y` for shader workgroup-size validation.
    #[cfg(test)]
    #[inline]
    pub fn max_compute_workgroup_size_y(&self) -> u32 {
        self.wgpu.max_compute_workgroup_size_y
    }

    /// `max_compute_workgroup_size_z` for shader workgroup-size validation.
    #[cfg(test)]
    #[inline]
    pub fn max_compute_workgroup_size_z(&self) -> u32 {
        self.wgpu.max_compute_workgroup_size_z
    }

    /// Returns `true` if `(x,y,z)` dispatch dimensions are within per-axis limits.
    #[must_use]
    #[inline]
    pub fn compute_dispatch_fits(&self, x: u32, y: u32, z: u32) -> bool {
        let m = self.wgpu.max_compute_workgroups_per_dimension;
        x <= m && y <= m && z <= m
    }

    /// Returns `true` if a `@workgroup_size(x, y, z)` declaration fits the device's per-axis caps
    /// and total invocation cap.
    #[must_use]
    #[cfg(test)]
    #[inline]
    pub fn workgroup_size_fits(&self, x: u32, y: u32, z: u32) -> bool {
        x <= self.wgpu.max_compute_workgroup_size_x
            && y <= self.wgpu.max_compute_workgroup_size_y
            && z <= self.wgpu.max_compute_workgroup_size_z
            && u64::from(x) * u64::from(y) * u64::from(z)
                <= u64::from(self.wgpu.max_compute_invocations_per_workgroup)
    }
}
