//! One-shot GPU-resource slots.

use std::sync::OnceLock;

/// One-time GPU object slot.
#[derive(Debug)]
pub(crate) struct OnceGpu<T> {
    slot: OnceLock<T>,
}

impl<T> Default for OnceGpu<T> {
    fn default() -> Self {
        Self {
            slot: OnceLock::new(),
        }
    }
}

impl<T> OnceGpu<T> {
    /// Returns the cached object, creating it with `build` on first use.
    pub(crate) fn get_or_create(&self, build: impl FnOnce() -> T) -> &T {
        self.slot.get_or_init(build)
    }
}
