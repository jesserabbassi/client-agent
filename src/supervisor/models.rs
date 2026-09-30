use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum StationStatus {
    Online,
    Offline,
    Available,
    Gaming,
    Reserved,
    Locked,
    Maintenance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StationTelemetry {
    pub station_id: String,
    pub hostname: Option<String>,
    pub cpu_usage: Option<f32>,
    pub ram_usage: Option<f32>,
    pub disk_usage: Option<f32>,
    pub network_upload: Option<f32>,
    pub network_download: Option<f32>,
    pub cpu_temperature: Option<f32>,
    pub gpu_usage: Option<f32>,
    pub gpu_temperature: Option<f32>,
    pub status: StationStatus,
    pub timestamp: String,
}
