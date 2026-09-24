use crate::models::telemetry::ConnectionStatus;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct State {
    status: ConnectionStatus,
    latency: Option<u64>,
    drops: u64,
}

/// Observes the actual hub connection, not general Internet connectivity.
#[derive(Clone)]
pub(crate) struct NetworkMonitor {
    state: Arc<Mutex<State>>,
    observed_drops: u64,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                status: ConnectionStatus::Offline,
                latency: None,
                drops: 0,
            })),
            observed_drops: 0,
        }
    }
    pub fn get_connection_status(&self) -> ConnectionStatus {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).status
    }
    pub fn get_latency(&self) -> Option<u64> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).latency
    }
    pub fn detect_connection_drop(&mut self) -> bool {
        let drops = self.state.lock().unwrap_or_else(|e| e.into_inner()).drops;
        let changed = drops != self.observed_drops;
        self.observed_drops = drops;
        changed
    }
    pub fn mark_online(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).status = ConnectionStatus::Online;
    }
    pub fn mark_offline(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.status == ConnectionStatus::Online {
            state.drops = state.drops.saturating_add(1);
        }
        state.status = ConnectionStatus::Offline;
        state.latency = None;
    }
    pub fn record_latency(&self, elapsed: Duration) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).latency =
            Some(elapsed.as_millis() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_a_drop_even_when_reconnected_between_samples() {
        let mut monitor = NetworkMonitor::new();
        let sender = monitor.clone();
        sender.mark_online();
        sender.record_latency(Duration::from_millis(12));
        assert_eq!(monitor.get_latency(), Some(12));
        sender.mark_offline();
        sender.mark_online();
        assert_eq!(monitor.get_connection_status(), ConnectionStatus::Online);
        assert!(monitor.detect_connection_drop());
        assert!(!monitor.detect_connection_drop());
        assert_eq!(monitor.get_latency(), None);
    }
}
