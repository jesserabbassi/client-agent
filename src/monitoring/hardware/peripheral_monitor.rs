use crate::monitoring::models::peripheral_status::PeripheralStatus;
use std::collections::BTreeMap;

pub(crate) struct PeripheralMonitor {
    previous: BTreeMap<String, PeripheralStatus>,
}
impl PeripheralMonitor {
    pub fn new() -> Self {
        Self {
            previous: BTreeMap::new(),
        }
    }

    pub fn get_connected_devices(&self) -> Option<Vec<PeripheralStatus>> {
        #[cfg(windows)]
        {
            super::hardware_monitor::windows::connected_peripherals()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Failed/unsupported scans do not falsely disconnect every device.
    /// Disconnections are included for one collection interval, not stored forever.
    pub fn detect_disconnected_devices(
        &mut self,
        current: &[PeripheralStatus],
    ) -> Vec<PeripheralStatus> {
        let next: BTreeMap<_, _> = current
            .iter()
            .map(|d| (d.device_id.clone(), d.clone()))
            .collect();
        let removed = self
            .previous
            .iter()
            .filter(|(id, _)| !next.contains_key(*id))
            .map(|(_, d)| {
                let mut d = d.clone();
                d.connected = false;
                d
            })
            .collect();
        self.previous = next;
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unplug_is_reported_once_and_replug_is_connected() {
        let mut monitor = PeripheralMonitor::new();
        let keyboard = PeripheralStatus {
            device_id: "kbd1".into(),
            device_name: "Keyboard".into(),
            device_type: "keyboard".into(),
            connected: true,
        };
        assert!(
            monitor
                .detect_disconnected_devices(&[keyboard.clone()])
                .is_empty()
        );
        let removed = monitor.detect_disconnected_devices(&[]);
        assert_eq!(removed.len(), 1);
        assert!(!removed[0].connected);
        assert!(monitor.detect_disconnected_devices(&[]).is_empty());
        assert!(monitor.detect_disconnected_devices(&[keyboard]).is_empty());
    }
}
