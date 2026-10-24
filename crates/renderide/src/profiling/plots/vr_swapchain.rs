//! Tracy plot for the VR swapchain GPU wait.

use std::time::Duration;

use super::tracy_plot::tracy_plot;

/// Records, in milliseconds, how long the driver thread blocked in the VR swapchain
/// GPU wait before `xrReleaseSwapchainImage`.
///
/// Zero when the wait is skipped (`never`, `auto` that did not match, an empty
/// projection batch, or a device that is already lost). The plot name is a Tracy
/// contract; do not rename it.
pub fn plot_vr_swapchain_gpu_wait_ms(wait: Duration) {
    tracy_plot!("vr_swapchain_gpu_wait_ms", wait.as_secs_f64() * 1000.0);
}
