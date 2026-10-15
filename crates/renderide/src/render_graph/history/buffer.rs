//! Buffer half-slot types for persistent ping-pong buffer history.

/// Buffer history slot declaration.
#[derive(Clone, Debug)]
pub struct BufferHistorySpec {
    /// Debug label used for the allocated `wgpu::Buffer`.
    pub label: &'static str,
    /// Byte size.
    pub size: u64,
    /// Buffer usage flags.
    pub usage: wgpu::BufferUsages,
}

/// Ping-pong buffer history slot.
pub struct BufferHistorySlot {
    pub(super) spec: BufferHistorySpec,
    pub(super) pair: [Option<wgpu::Buffer>; 2],
}

impl BufferHistorySlot {
    pub(super) fn ensure(&mut self, device: &wgpu::Device) {
        for slot in &mut self.pair {
            if slot.is_none() {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(self.spec.label),
                    size: self.spec.size.max(1),
                    usage: self.spec.usage,
                    mapped_at_creation: false,
                });
                crate::profiling::note_resource_churn!(Buffer, "render_graph::history_buffer");
                *slot = Some(buffer);
            }
        }
    }

    /// Borrows a half of the ping-pong pair; returns [`None`] until the first
    /// [`crate::render_graph::history::HistoryRegistry::ensure_resources`] call has allocated it.
    pub fn half(&self, index: usize) -> Option<&wgpu::Buffer> {
        self.pair.get(index)?.as_ref()
    }
}

#[cfg(test)]
pub(super) fn buffer_specs_equivalent(a: &BufferHistorySpec, b: &BufferHistorySpec) -> bool {
    a.label == b.label && a.size == b.size && a.usage == b.usage
}
