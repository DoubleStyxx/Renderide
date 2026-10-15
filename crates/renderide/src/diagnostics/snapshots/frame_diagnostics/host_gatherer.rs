//! Throttled `sysinfo` sampling for the debug HUD host CPU / RAM section.

use super::host::HostCpuMemoryHud;
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind,
    System,
};

/// Owns a lazily allocated [`System`] and samples [`HostCpuMemoryHud`] every
/// [`REFRESH_INTERVAL_FRAMES`] frames to limit `sysinfo` work.
pub struct HostHudGatherer {
    system: Option<System>,
    frame_counter: u64,
    pid: Option<Pid>,
    cached: HostCpuMemoryHud,
}

const REFRESH_INTERVAL_FRAMES: u64 = 30;

impl HostHudGatherer {
    /// Creates a gatherer; the first [`Self::snapshot`] may allocate the [`System`].
    pub fn new() -> Self {
        Self {
            system: None,
            frame_counter: 0,
            pid: sysinfo::get_current_pid().ok(),
            cached: HostCpuMemoryHud::default(),
        }
    }

    /// Returns host CPU/RAM plus this process RAM for the current frame (cached between refreshes).
    pub fn snapshot(&mut self) -> HostCpuMemoryHud {
        self.frame_counter = self.frame_counter.wrapping_add(1);

        if self.system.is_none() && sysinfo::IS_SUPPORTED_SYSTEM {
            self.system = Some(System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::everything())
                    .with_memory(MemoryRefreshKind::everything()),
            ));
        }

        let Some(ref mut sys) = self.system else {
            let label = if sysinfo::IS_SUPPORTED_SYSTEM {
                String::new()
            } else {
                "unsupported platform".to_string()
            };
            self.cached = HostCpuMemoryHud {
                cpu_model: label,
                ..Default::default()
            };
            return self.cached.clone();
        };

        if self.frame_counter % REFRESH_INTERVAL_FRAMES == 1 {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            let process_ram = self.pid.and_then(|pid| {
                sys.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                sys.process(pid).map(sysinfo::Process::memory)
            });

            let cpu_model = sys
                .cpus()
                .first()
                .map(|c| c.brand().to_string())
                .unwrap_or_default();

            self.cached = HostCpuMemoryHud {
                cpu_model,
                logical_cpus: sys.cpus().len(),
                cpu_usage_percent: sys.global_cpu_usage(),
                ram_total_bytes: sys.total_memory(),
                ram_used_bytes: sys.used_memory(),
                process_ram_bytes: process_ram,
            };
        }

        self.cached.clone()
    }
}

impl Default for HostHudGatherer {
    fn default() -> Self {
        Self::new()
    }
}
