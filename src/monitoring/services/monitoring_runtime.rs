use super::monitoring_service::MonitoringService;
use crate::{
    communication::signalr::server_client::Result,
    infrastructure::{telemetry_config::Config, telemetry_sender::run},
    monitoring::{hardware::network_monitor::NetworkMonitor, models::telemetry::Telemetry},
};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::watch;

pub(crate) struct TelemetryAgent {
    stop: watch::Sender<bool>,
    collect_stop: mpsc::Sender<()>,
    network: Option<thread::JoinHandle<()>>,
}

impl TelemetryAgent {
    pub fn start_from_env() -> Result<Option<Self>> {
        let Some(config) = Config::from_env()? else {
            tracing::info!("Telemetry disabled by configuration or missing hub URL");
            return Ok(None);
        };
        Self::start(config).map(Some)
    }

    pub(crate) fn start(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let (samples_tx, samples_rx) = watch::channel(None::<Arc<Telemetry>>);
        let (collect_stop, collect_shutdown) = mpsc::channel();
        let collector_config = config.clone();
        let network_monitor = NetworkMonitor::new();
        let collector_network = network_monitor.clone();
        thread::Builder::new()
            .name("telemetry-collector".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(|| {
                    let mut collector =
                        MonitoringService::new(collector_config.max_processes, collector_network);
                    loop {
                        match collect_shutdown.recv_timeout(collector_config.interval) {
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            _ => break,
                        }
                        if samples_tx
                            .send(Some(Arc::new(collector.collect_telemetry())))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                if outcome.is_err() {
                    tracing::error!("Telemetry collector stopped unexpectedly");
                }
            })
            .map_err(|_| "cannot start telemetry collector")?;
        let (stop, shutdown) = watch::channel(false);
        let network = thread::Builder::new()
            .name("telemetry-network".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        tracing::error!("Cannot initialize telemetry runtime");
                        return;
                    }
                };
                runtime.block_on(run(config, samples_rx, shutdown, network_monitor));
                // A stuck filesystem token read must not prevent process shutdown.
                runtime.shutdown_timeout(Duration::from_secs(1));
            })
            .map_err(|_| "cannot start telemetry network worker")?;
        Ok(Self {
            stop,
            collect_stop,
            network: Some(network),
        })
    }
}

impl Drop for TelemetryAgent {
    fn drop(&mut self) {
        let _ = self.collect_stop.send(());
        let _ = self.stop.send(true);
        // Main drops this after ui.run() returns. No callback ever joins workers.
        if let Some(network) = self.network.take() {
            if network.join().is_err() {
                tracing::error!("Telemetry network worker panicked");
            }
        }
    }
}
