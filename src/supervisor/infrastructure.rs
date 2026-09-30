use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use super::models::{StationStatus, StationTelemetry};

/// Backend endpoint is intentionally supplied by the application at runtime.
/// No route, DTO, or hub name is defined here until backend contracts exist.
#[derive(Clone, Default)]
pub struct ApiClient {
    base_url: Option<String>,
}

impl ApiClient {
    pub fn from_config(base_url: Option<String>) -> Self {
        Self { base_url }
    }
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }
}

#[derive(Clone)]
pub struct SignalRClient {
    sender: broadcast::Sender<StationTelemetry>,
    connected: Arc<Mutex<bool>>,
}

impl Default for SignalRClient {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(128);
        Self {
            sender,
            connected: Arc::new(Mutex::new(false)),
        }
    }
}

impl SignalRClient {
    pub fn connect(&self) {
        if let Ok(mut state) = self.connected.lock() {
            *state = true;
        }
    }
    pub fn disconnect(&self) {
        if let Ok(mut state) = self.connected.lock() {
            *state = false;
        }
    }
    pub fn is_connected(&self) -> bool {
        self.connected.lock().map(|state| *state).unwrap_or(false)
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StationTelemetry> {
        self.sender.subscribe()
    }
    pub fn publish_mock(&self, telemetry: StationTelemetry) {
        let _ = self.sender.send(telemetry);
    }
}

#[derive(Clone, Default)]
pub struct LocalStorage {
    values: Arc<Mutex<HashMap<String, String>>>,
}

impl LocalStorage {
    pub fn save(&self, key: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut values) = self.values.lock() {
            values.insert(key.into(), value.into());
        }
    }
    pub fn load(&self, key: &str) -> Option<String> {
        self.values.lock().ok()?.get(key).cloned()
    }
    pub fn clear(&self, key: &str) {
        if let Ok(mut values) = self.values.lock() {
            values.remove(key);
        }
    }
}

pub fn mock_telemetry(station_id: impl Into<String>, tick: u64) -> StationTelemetry {
    StationTelemetry {
        station_id: station_id.into(),
        hostname: Some("supervisor-station".into()),
        cpu_usage: Some(42.0 + (tick % 20) as f32),
        ram_usage: Some(56.0),
        disk_usage: Some(37.0),
        network_upload: Some(87.0),
        network_download: Some(245.0),
        cpu_temperature: Some(60.0),
        gpu_usage: Some(68.0),
        gpu_temperature: Some(64.0),
        status: StationStatus::Online,
        timestamp: format!("tick-{tick}"),
    }
}
