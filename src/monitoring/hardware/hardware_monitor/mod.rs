mod gpu;
#[cfg(windows)]
pub(crate) mod windows;
use crate::monitoring::models::telemetry::*;
use gpu::GpuCollector;
use sysinfo::{Components, ProcessRefreshKind, ProcessesToUpdate, System};

pub(crate) struct HardwareMonitor {
    system: System,
    components: Components,
    gpu: GpuCollector,
    max_processes: usize,
    last_gpus: Vec<Gpu>,
}

impl HardwareMonitor {
    pub fn new(max_processes: usize) -> Self {
        let mut result = Self {
            system: System::new(),
            components: Components::new_with_refreshed_list(),
            gpu: GpuCollector::new(),
            max_processes,
            last_gpus: Vec::new(),
        };
        result.refresh(); // Prime CPU/process deltas; first sample is one interval later.
        result
    }

    fn refresh(&mut self) {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        self.components.refresh(true);
    }

    pub fn collect(&mut self) -> HardwareSnapshot {
        self.refresh();
        let cpus = self.system.cpus();
        let mut processes: Vec<_> = self
            .system
            .processes()
            .iter()
            .map(|(pid, p)| Process {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().chars().take(160).collect(),
                cpu_percent: (p.cpu_usage() / cpus.len().max(1) as f32).clamp(0.0, 100.0),
                memory_bytes: p.memory(),
            })
            .collect();
        processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes).then(a.pid.cmp(&b.pid)));
        let process_count = processes.len();
        #[cfg(windows)]
        let opened_apps = windows::visible_app_pids().map(|pids| {
            processes
                .iter()
                .filter(|p| pids.contains(&p.pid))
                .cloned()
                .collect::<Vec<_>>()
        });
        #[cfg(not(windows))]
        let opened_apps: Option<Vec<Process>> = None;
        let opened_app_count = opened_apps.as_ref().map(Vec::len);
        let opened_apps = opened_apps.map(|mut apps| {
            apps.truncate(self.max_processes);
            apps
        });
        processes.truncate(self.max_processes);
        let (gpus, gpu_provider) = self.gpu.collect();
        self.last_gpus = gpus.clone();
        HardwareSnapshot {
            sampled_at_unix_ms: now_ms(),
            hostname: System::host_name(),
            os: System::long_os_version(),
            uptime_seconds: System::uptime(),
            cpu: Cpu {
                brand: cpus
                    .first()
                    .map(|c| c.brand().to_owned())
                    .unwrap_or_default(),
                vendor: cpus
                    .first()
                    .map(|c| c.vendor_id().to_owned())
                    .unwrap_or_default(),
                usage_percent: self.system.global_cpu_usage(),
                physical_cores: System::physical_core_count(),
                logical_cores: cpus.len(),
                cores: cpus
                    .iter()
                    .map(|c| CpuCore {
                        name: c.name().into(),
                        usage_percent: c.cpu_usage(),
                        frequency_mhz: c.frequency(),
                    })
                    .collect(),
            },
            ram: Ram {
                total_bytes: self.system.total_memory(),
                used_bytes: self.system.used_memory(),
                available_bytes: self.system.available_memory(),
                swap_total_bytes: self.system.total_swap(),
                swap_used_bytes: self.system.used_swap(),
            },
            gpus,
            gpu_provider,
            temperatures: self
                .components
                .iter()
                .map(|c| Temperature {
                    label: c.label().chars().take(160).collect(),
                    celsius: c.temperature(),
                    critical_celsius: c.critical(),
                })
                .collect(),
            running_processes: processes,
            process_count,
            processes_truncated: process_count > self.max_processes,
            opened_apps,
            opened_app_count,
            opened_apps_truncated: opened_app_count.is_some_and(|n| n > self.max_processes),
        }
    }

    pub fn get_cpu_usage(&self) -> f32 {
        self.system.global_cpu_usage()
    }
    pub fn get_gpu_usage(&self) -> Option<f32> {
        self.last_gpus
            .iter()
            .filter_map(|g| g.utilization_percent)
            .max()
            .map(|n| n as f32)
    }
    pub fn get_ram_usage(&self) -> f32 {
        if self.system.total_memory() == 0 {
            return 0.0;
        }
        100.0 * self.system.used_memory() as f32 / self.system.total_memory() as f32
    }
    pub fn get_cpu_temperature(&self) -> Option<f32> {
        self.components
            .iter()
            .filter(|c| {
                let label = c.label().to_ascii_lowercase();
                ["cpu", "core", "package", "tctl", "tdie"]
                    .iter()
                    .any(|name| label.contains(name))
            })
            .filter_map(|c| c.temperature())
            .filter(|n| n.is_finite())
            .reduce(f32::max)
    }
    pub fn get_gpu_temperature(&self) -> Option<u32> {
        self.last_gpus
            .iter()
            .filter_map(|g| g.temperature_celsius)
            .max()
    }
    pub fn get_fan_speed(&self) -> Option<u32> {
        self.last_gpus.iter().filter_map(|g| g.fan_percent).max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_collection_is_serializable_and_respects_process_limit() {
        let sample = HardwareMonitor::new(3).collect();
        assert!(sample.running_processes.len() <= 3);
        assert!(sample.ram.total_bytes >= sample.ram.used_bytes);
        assert!(sample.cpu.logical_cores > 0);
        assert!(serde_json::to_string(&sample).is_ok());
    }
}
