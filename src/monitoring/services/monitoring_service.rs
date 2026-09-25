use crate::monitoring::{
    hardware::{
        hardware_monitor::HardwareMonitor, network_monitor::NetworkMonitor,
        peripheral_monitor::PeripheralMonitor,
    },
    models::telemetry::Telemetry,
};

/// Application service from the monitoring architecture. Never performs network IO.
pub(crate) struct MonitoringService {
    hardware_monitor: HardwareMonitor,
    peripheral_monitor: PeripheralMonitor,
    network_monitor: NetworkMonitor,
}

impl MonitoringService {
    pub fn new(max_processes: usize, network_monitor: NetworkMonitor) -> Self {
        Self {
            hardware_monitor: HardwareMonitor::new(max_processes),
            peripheral_monitor: PeripheralMonitor::new(),
            network_monitor,
        }
    }
    pub fn collect_telemetry(&mut self) -> Telemetry {
        let hardware = self.hardware_monitor.collect();
        let peripherals = self
            .peripheral_monitor
            .get_connected_devices()
            .map(|mut devices| {
                devices.extend(
                    self.peripheral_monitor
                        .detect_disconnected_devices(&devices),
                );
                devices
            });
        Telemetry {
            timestamp: hardware.sampled_at_unix_ms,
            cpu_usage: self.get_cpu_usage(),
            gpu_usage: self.get_gpu_usage(),
            ram_usage: self.hardware_monitor.get_ram_usage(),
            cpu_temperature: self.get_temperature(),
            gpu_temperature: self.hardware_monitor.get_gpu_temperature(),
            fan_speed: self.get_fan_speed(),
            network_status: self.network_monitor.get_connection_status(),
            network_latency_ms: self.network_monitor.get_latency(),
            connection_dropped: self.network_monitor.detect_connection_drop(),
            peripherals,
            details_truncated: false,
            hardware,
        }
    }
    pub fn get_cpu_usage(&self) -> f32 {
        self.hardware_monitor.get_cpu_usage()
    }
    pub fn get_gpu_usage(&self) -> Option<f32> {
        self.hardware_monitor.get_gpu_usage()
    }
    pub fn get_temperature(&self) -> Option<f32> {
        self.hardware_monitor.get_cpu_temperature()
    }
    pub fn get_fan_speed(&self) -> Option<u32> {
        self.hardware_monitor.get_fan_speed()
    }
}
