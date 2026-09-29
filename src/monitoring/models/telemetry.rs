use super::peripheral_status::PeripheralStatus;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ConnectionStatus {
    Online,
    Offline,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Telemetry {
    pub timestamp: u64,
    pub cpu_usage: f32,
    pub gpu_usage: Option<f32>,
    pub ram_usage: f32,
    pub cpu_temperature: Option<f32>,
    pub gpu_temperature: Option<u32>,
    /// Maximum reported GPU fan percentage; not CPU fan RPM.
    pub fan_speed: Option<u32>,
    pub network_status: ConnectionStatus,
    pub network_latency_ms: Option<u64>,
    pub connection_dropped: bool,
    pub peripherals: Option<Vec<PeripheralStatus>>,
    pub details_truncated: bool,
    #[serde(flatten)]
    pub hardware: HardwareSnapshot,
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HardwareSnapshot {
    pub sampled_at_unix_ms: u64,
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub uptime_seconds: u64,
    pub cpu: Cpu,
    pub ram: Ram,
    pub gpus: Vec<Gpu>,
    pub gpu_provider: &'static str,
    pub temperatures: Vec<Temperature>,
    pub running_processes: Vec<Process>,
    pub process_count: usize,
    pub processes_truncated: bool,
    /// Windows: visible top-level windows, deduplicated by PID. No window titles.
    /// Other platforms: null, rather than mislabelling background processes as apps.
    pub opened_apps: Option<Vec<Process>>,
    pub opened_app_count: Option<usize>,
    pub opened_apps_truncated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cpu {
    pub brand: String,
    pub vendor: String,
    pub usage_percent: f32,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub cores: Vec<CpuCore>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuCore {
    pub name: String,
    pub usage_percent: f32,
    pub frequency_mhz: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ram {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Process {
    pub pid: u32,
    pub name: String,
    /// Normalized over all logical CPUs: 0..100, like Windows Task Manager.
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Temperature {
    pub label: String,
    pub celsius: Option<f32>,
    pub critical_celsius: Option<f32>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Gpu {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub driver_version: Option<String>,
    pub source: String,
    pub utilization_percent: Option<u32>,
    pub memory_utilization_percent: Option<u32>,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_free_bytes: Option<u64>,
    pub shared_memory_used_bytes: Option<u64>,
    pub temperature_celsius: Option<u32>,
    pub fan_percent: Option<u32>,
    pub power_milliwatts: Option<u32>,
    pub power_limit_milliwatts: Option<u32>,
    pub graphics_clock_mhz: Option<u32>,
    pub memory_clock_mhz: Option<u32>,
    pub pcie_generation: Option<u32>,
    pub pcie_width: Option<u32>,
    pub performance_state: Option<String>,
}
