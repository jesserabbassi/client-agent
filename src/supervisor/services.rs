use super::{infrastructure::SignalRClient, models::StationTelemetry};

#[derive(Clone)]
pub struct MonitoringService {
    signalr: SignalRClient,
}

impl MonitoringService {
    pub fn new(signalr: SignalRClient) -> Self {
        Self { signalr }
    }
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<StationTelemetry> {
        self.signalr.subscribe()
    }
    pub fn connect(&self) {
        self.signalr.connect();
    }
}

#[derive(Clone, Default)]
pub struct RemoteControlService;

impl RemoteControlService {
    pub fn lock_station(&self, _station_id: &str) {}
    pub fn unlock_station(&self, _station_id: &str) {}
    pub fn restart_station(&self, _station_id: &str) {}
    pub fn shutdown_station(&self, _station_id: &str) {}
}
