use crate::monitoring::models::telemetry::Gpu;
use nvml_wrapper::{
    Nvml,
    enum_wrappers::device::{Clock, TemperatureSensor},
};

pub(crate) struct GpuCollector {
    nvml: Option<Nvml>,
    #[cfg(windows)]
    windows: super::windows::WindowsGpus,
}

impl GpuCollector {
    pub fn new() -> Self {
        Self {
            nvml: Nvml::init().ok(),
            #[cfg(windows)]
            windows: super::windows::WindowsGpus::new(),
        }
    }

    pub fn collect(&mut self) -> (Vec<Gpu>, &'static str) {
        let mut result = Vec::new();
        if let Some(nvml) = &self.nvml {
            let driver = nvml.sys_driver_version().ok();
            for index in 0..nvml.device_count().unwrap_or(0).min(64) {
                let Ok(device) = nvml.device_by_index(index) else {
                    continue;
                };
                let memory = device.memory_info().ok();
                let utilization = device.utilization_rates().ok();
                result.push(Gpu {
                    id: device.uuid().unwrap_or_else(|_| format!("nvml-{index}")),
                    name: device.name().unwrap_or_else(|_| "NVIDIA GPU".into()),
                    vendor: "NVIDIA".into(),
                    driver_version: driver.clone(),
                    source: "nvml".into(),
                    utilization_percent: utilization.as_ref().map(|u| u.gpu),
                    memory_utilization_percent: utilization.as_ref().map(|u| u.memory),
                    memory_total_bytes: memory.as_ref().map(|m| m.total),
                    memory_used_bytes: memory.as_ref().map(|m| m.used),
                    memory_free_bytes: memory.as_ref().map(|m| m.free),
                    temperature_celsius: device.temperature(TemperatureSensor::Gpu).ok(),
                    fan_percent: device.fan_speed(0).ok(),
                    power_milliwatts: device.power_usage().ok(),
                    power_limit_milliwatts: device.power_management_limit().ok(),
                    graphics_clock_mhz: device.clock_info(Clock::Graphics).ok(),
                    memory_clock_mhz: device.clock_info(Clock::Memory).ok(),
                    pcie_generation: device.current_pcie_link_gen().ok(),
                    pcie_width: device.current_pcie_link_width().ok(),
                    performance_state: device.performance_state().ok().map(|s| format!("{s:?}")),
                    ..Gpu::default()
                });
            }
        }
        #[cfg(windows)]
        {
            // Keep NVML's rich metrics; match each identical-name adapter at most once.
            let mut matched = std::collections::HashSet::new();
            for gpu in self.windows.collect() {
                let nvml_match = result
                    .iter()
                    .enumerate()
                    .position(|(i, n)| !matched.contains(&i) && n.name == gpu.name);
                if let Some(i) = nvml_match {
                    matched.insert(i);
                    result[i].shared_memory_used_bytes = gpu.shared_memory_used_bytes;
                } else {
                    result.push(gpu);
                }
            }
        }
        let provider = if result.is_empty() {
            "unavailable"
        } else {
            #[cfg(windows)]
            {
                "dxgi-pdh+nvml"
            }
            #[cfg(not(windows))]
            {
                "nvml"
            }
        };
        (result, provider)
    }
}
