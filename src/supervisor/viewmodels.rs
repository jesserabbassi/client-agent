use std::collections::HashMap;

use super::{models::StationTelemetry, services::MonitoringService};

#[derive(Default)]
pub struct MonitoringViewModel {
    stations: HashMap<String, StationTelemetry>,
}

impl MonitoringViewModel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn load_telemetry(
        &mut self,
        service: &MonitoringService,
    ) -> tokio::sync::broadcast::Receiver<StationTelemetry> {
        service.connect();
        service.subscribe()
    }
    pub fn update_realtime_data(&mut self, telemetry: StationTelemetry) {
        self.stations
            .insert(telemetry.station_id.clone(), telemetry);
    }
    pub fn stations(&self) -> impl Iterator<Item = &StationTelemetry> {
        self.stations.values()
    }
}
