use crate::{
    communication::signalr::server_client::{ACK_TIMEOUT, Result, SERVER_TIMEOUT, ServerClient},
    infrastructure::telemetry_config::Config,
    monitoring::{
        hardware::network_monitor::NetworkMonitor,
        models::telemetry::{ConnectionStatus, Telemetry, now_ms},
    },
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::watch,
    time::{Instant, MissedTickBehavior},
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendTelemetry {
    station_id: String,
    cpu_usage: f64,
    gpu_usage: f64,
    ram_usage: f64,
    cpu_temperature: f64,
    gpu_temperature: f64,
    fan_speed: f64,
    network_status: &'static str,
    timestamp: String,
}

struct TelemetrySender {
    server_client: ServerClient,
    queued: Option<Arc<Telemetry>>,
}

impl TelemetrySender {
    fn queue(&mut self, telemetry: Option<Arc<Telemetry>>) {
        self.queued = telemetry;
    }

    async fn invoke(&mut self, invocation_id: &str, target: &str, arguments: Value) -> Result<()> {
        self.server_client
            .send_json(json!({
                "type": 1,
                "invocationId": invocation_id,
                "target": target,
                "arguments": arguments,
            }))
            .await
    }
}

pub(crate) async fn run(
    config: Arc<Config>,
    samples: watch::Receiver<Option<Arc<Telemetry>>>,
    mut stop: watch::Receiver<bool>,
    network_monitor: NetworkMonitor,
) {
    let mut failures = 0u32;
    loop {
        if *stop.borrow() {
            break;
        }
        let token_config = config.clone();
        let attempt = async {
            let token = tokio::task::spawn_blocking(move || token_config.token())
                .await
                .map_err(|_| "token provider failed")??;
            ServerClient::connect(&config.hub, token.as_deref()).await
        };
        let connection = tokio::select! {
            biased;
            _ = stop.changed() => break,
            result = attempt => result,
        };
        let outcome = match connection {
            Ok(connection) => {
                network_monitor.mark_online();
                let mut sender = TelemetrySender {
                    server_client: connection,
                    queued: None,
                };
                tracing::info!("Telemetry SignalR connection established");
                session_loop(&mut sender, &config, &samples, &mut stop, &network_monitor).await
            }
            Err(error) => Err(error),
        };
        match outcome {
            Ok(()) => {
                network_monitor.mark_offline();
                break;
            }
            Err(error) => {
                network_monitor.mark_offline();
                let delay = backoff(failures, rand::random::<f64>());
                failures = failures.saturating_add(1);
                tracing::warn!(
                    reason = error,
                    retry_ms = delay.as_millis() as u64,
                    "Telemetry disconnected; retrying"
                );
                tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = tokio::time::sleep(delay) => {},
                }
            }
        }
    }
    tracing::info!("Telemetry network worker stopped");
}

fn backoff(failures: u32, jitter: f64) -> Duration {
    let ceiling = (1u64 << failures.min(5)).min(30) as f64;
    Duration::from_secs_f64(ceiling * (0.5 + 0.5 * jitter.clamp(0.0, 1.0)))
}

async fn session_loop(
    sender: &mut TelemetrySender,
    config: &Config,
    samples: &watch::Receiver<Option<Arc<Telemetry>>>,
    stop: &mut watch::Receiver<bool>,
    network_monitor: &NetworkMonitor,
) -> Result<()> {
    sender
        .invoke(
            "connect",
            "ConnectAgent",
            json!([config.station_id.to_string()]),
        )
        .await?;
    await_completion(sender, "connect").await?;

    let mut send_tick = tokio::time::interval(config.interval);
    send_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut heartbeat_tick = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut ping_tick = tokio::time::interval(Duration::from_secs(10));
    ping_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut pending: Option<(String, Instant)> = None;
    loop {
        let server_deadline = sender.server_client.last_received + SERVER_TIMEOUT;
        let ack_deadline = pending
            .as_ref()
            .map(|(_, sent)| *sent + ACK_TIMEOUT)
            .unwrap_or_else(|| Instant::now() + SERVER_TIMEOUT);
        tokio::select! {
            biased;
            _ = stop.changed() => { sender.server_client.close().await; return Ok(()); }
            _ = tokio::time::sleep_until(server_deadline) => return Err("SignalR server heartbeat timed out"),
            _ = tokio::time::sleep_until(ack_deadline), if pending.is_some() => return Err("SignalR invocation acknowledgement timed out"),
            message = sender.server_client.receive() => {
                let message = message?;
                match message["type"].as_u64() {
                    Some(3) if pending.as_ref().is_some_and(|(id, _)| message["invocationId"].as_str() == Some(id)) => {
                        if message.get("error").is_some() { return Err("backend rejected SignalR invocation"); }
                        if let Some((_, sent)) = pending.take() { network_monitor.record_latency(sent.elapsed()); }
                    }
                    Some(6) => {}
                    Some(7) => {
                        if message["allowReconnect"].as_bool() == Some(true) { return Err("backend requested reconnect"); }
                        sender.server_client.close().await;
                        return Ok(());
                    }
                    Some(1) => {}
                    _ => return Err("unexpected SignalR hub message"),
                }
            }
            _ = ping_tick.tick() => sender.server_client.send_json(json!({"type": 6})).await?,
            _ = heartbeat_tick.tick(), if pending.is_none() => {
                sender.invoke("heartbeat", "Heartbeat", json!([])).await?;
                pending = Some(("heartbeat".into(), Instant::now()));
            }
            _ = send_tick.tick(), if pending.is_none() => {
                if samples.has_changed().is_err() { return Err("telemetry collector is unavailable"); }
                let snapshot = samples.borrow().clone().filter(|s| {
                    now_ms().saturating_sub(s.hardware.sampled_at_unix_ms) <= (config.interval.as_millis() * 3) as u64
                });
                sender.queue(snapshot);
                let Some(telemetry) = sender.queued.as_deref() else { continue; };
                let dto = map_telemetry(config.station_id, telemetry);
                sender.invoke("telemetry", "SendTelemetry", json!([dto])).await?;
                pending = Some(("telemetry".into(), Instant::now()));
            }
        }
    }
}

async fn await_completion(sender: &mut TelemetrySender, invocation_id: &str) -> Result<()> {
    loop {
        let message = sender.server_client.receive().await?;
        match message["type"].as_u64() {
            Some(3) if message["invocationId"].as_str() == Some(invocation_id) => {
                return if message.get("error").is_some() {
                    Err("backend rejected ConnectAgent")
                } else {
                    Ok(())
                };
            }
            Some(6) | Some(1) => {}
            _ => return Err("unexpected ConnectAgent response"),
        }
    }
}

fn map_telemetry(station_id: uuid::Uuid, telemetry: &Telemetry) -> BackendTelemetry {
    let timestamp = DateTime::<Utc>::from_timestamp_millis(telemetry.timestamp as i64)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    BackendTelemetry {
        station_id: station_id.to_string(),
        cpu_usage: telemetry.cpu_usage as f64,
        gpu_usage: telemetry.gpu_usage.unwrap_or_default() as f64,
        ram_usage: telemetry.ram_usage as f64,
        cpu_temperature: telemetry.cpu_temperature.unwrap_or_default() as f64,
        gpu_temperature: telemetry.gpu_temperature.unwrap_or_default() as f64,
        fan_speed: telemetry.fan_speed.unwrap_or_default() as f64,
        network_status: match telemetry.network_status {
            ConnectionStatus::Online => "Connected",
            ConnectionStatus::Offline => "Disconnected",
        },
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::{
        hardware::network_monitor::NetworkMonitor, services::monitoring_service::MonitoringService,
    };

    #[test]
    fn maps_existing_snapshot_to_backend_dto() {
        let station = uuid::Uuid::new_v4();
        let telemetry = MonitoringService::new(0, NetworkMonitor::new()).collect_telemetry();
        let dto = map_telemetry(station, &telemetry);
        let value = serde_json::to_value(dto).unwrap();
        assert_eq!(value["stationId"], station.to_string());
        assert!(value["timestamp"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn exponential_retry_is_jittered_and_capped() {
        assert_eq!(backoff(0, 0.0), Duration::from_millis(500));
        assert_eq!(backoff(1, 1.0), Duration::from_secs(2));
        assert_eq!(backoff(99, 1.0), Duration::from_secs(30));
        assert_eq!(backoff(99, 0.0), Duration::from_secs(15));
    }
}
