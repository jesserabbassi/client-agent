use crate::{
    communication::server_client::{
        self as signalr, ACK_TIMEOUT, Result, SERVER_TIMEOUT, ServerClient,
    },
    infrastructure::telemetry_config::Config,
    models::telemetry::{Envelope, Telemetry, now_ms},
    services::network_monitor::NetworkMonitor,
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::watch,
    time::{Instant, MissedTickBehavior},
};

/// Network actor: owns the ServerClient and a replaceable latest-value queue.
struct TelemetrySender {
    server_client: ServerClient,
    queued: Option<Arc<Telemetry>>,
}
impl TelemetrySender {
    fn queue(&mut self, telemetry: Option<Arc<Telemetry>>) {
        self.queued = telemetry;
    }
    async fn send(
        &mut self,
        config: &Config,
        session: &str,
        sequence: u64,
        status: &str,
    ) -> Result<()> {
        self.server_client
            .send_record(payload(
                config,
                session,
                sequence,
                status,
                self.queued.as_deref(),
            )?)
            .await
    }
}

pub(crate) async fn run(
    config: Arc<Config>,
    samples: watch::Receiver<Option<Arc<Telemetry>>>,
    mut stop: watch::Receiver<bool>,
    network_monitor: NetworkMonitor,
) {
    let session = uuid::Uuid::new_v4().to_string();
    let mut sequence = 0u64;
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
                session_loop(
                    &mut sender,
                    &config,
                    &session,
                    &mut sequence,
                    &samples,
                    &mut stop,
                    &mut failures,
                    &network_monitor,
                )
                .await
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
    session: &str,
    sequence: &mut u64,
    samples: &watch::Receiver<Option<Arc<Telemetry>>>,
    stop: &mut watch::Receiver<bool>,
    failures: &mut u32,
    network_monitor: &NetworkMonitor,
) -> Result<()> {
    let mut send_tick = tokio::time::interval(config.interval);
    send_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
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
            _ = stop.changed() => {
                // Best effort, bounded. Power loss/network loss is detected by the
                // backend's last-seen timeout, never by an impossible offline send.
                let _ = tokio::time::timeout(Duration::from_secs(2), async {
                    *sequence += 1;
                    let record = payload(config, session, *sequence, "offline", None)?;
                    sender.server_client.send_record(record).await?;
                    sender.server_client.close().await;
                    Ok::<_, &'static str>(())
                }).await;
                return Ok(());
            }
            _ = tokio::time::sleep_until(server_deadline) => return Err("SignalR server heartbeat timed out"),
            _ = tokio::time::sleep_until(ack_deadline), if pending.is_some() => return Err("telemetry acknowledgement timed out"),
            message = sender.server_client.receive() => {
                let message = message?;
                match message["type"].as_u64() {
                    Some(3) => {
                        if pending.as_ref().is_some_and(|(id, _)| message["invocationId"].as_str() == Some(id.as_str())) {
                            if message.get("error").is_some() { return Err("backend rejected telemetry invocation"); }
                            if let Some((_, sent)) = &pending { network_monitor.record_latency(sent.elapsed()); }
                            pending = None;
                            *failures = 0;
                        }
                    }
                    Some(6) => {},
                    Some(7) => {
                        if message["allowReconnect"].as_bool() == Some(true) { return Err("backend requested reconnect"); }
                        tracing::warn!("Backend closed SignalR with reconnect disabled");
                        sender.server_client.close().await;
                        return Ok(());
                    }
                    Some(1) => {
                        if let Some(id) = message["invocationId"].as_str() {
                            sender.server_client.send_json(json!({"type":3,"invocationId":id,"error":"This agent supports telemetry only"})).await?;
                        }
                    }
                    _ => return Err("unexpected SignalR hub message"),
                }
            }
            _ = ping_tick.tick() => sender.server_client.send_json(json!({"type":6})).await?,
            _ = send_tick.tick(), if pending.is_none() => {
                if samples.has_changed().is_err() { return Err("telemetry collector is unavailable"); }
                let snapshot = samples.borrow().clone();
                // Do not replay stale measurements after a long outage/collector stall.
                let snapshot = snapshot.filter(|s| now_ms().saturating_sub(s.hardware.sampled_at_unix_ms) <= (config.interval.as_millis() * 3) as u64);
                *sequence += 1;
                sender.queue(snapshot);
                sender.send(config, session, *sequence, "online").await?;
                pending = Some((sequence.to_string(), Instant::now()));
            }
        }
    }
}

fn payload(
    config: &Config,
    session: &str,
    sequence: u64,
    status: &str,
    snapshot: Option<&Telemetry>,
) -> Result<String> {
    let mut snapshot = snapshot.cloned();
    loop {
        let envelope = Envelope {
            schema_version: 1,
            agent_id: &config.agent_id,
            session_id: session,
            sequence,
            sent_at_unix_ms: now_ms(),
            status,
            telemetry: snapshot.as_ref(),
        };
        let invocation: Value = json!({"type":1,"invocationId":sequence.to_string(),"target":config.method,"arguments":[envelope]});
        match signalr::encode(&invocation) {
            Ok(record) => return Ok(record),
            Err(error) => {
                let Some(telemetry) = &mut snapshot else {
                    return Err(error);
                };
                telemetry.details_truncated = true;
                let sample = &mut telemetry.hardware;
                if !sample.running_processes.is_empty() {
                    sample
                        .running_processes
                        .truncate(sample.running_processes.len() / 2);
                    sample.processes_truncated = true;
                } else if let Some(apps) = &mut sample.opened_apps
                    && !apps.is_empty()
                {
                    apps.truncate(apps.len() / 2);
                    sample.opened_apps_truncated = true;
                } else if let Some(devices) = &mut telemetry.peripherals
                    && !devices.is_empty()
                {
                    devices.truncate(devices.len() / 2);
                } else if !sample.temperatures.is_empty() {
                    sample.temperatures.truncate(sample.temperatures.len() / 2);
                } else if !sample.cpu.cores.is_empty() {
                    sample.cpu.cores.truncate(sample.cpu.cores.len() / 2);
                } else if !sample.gpus.is_empty() {
                    sample.gpus.truncate(sample.gpus.len() / 2);
                } else {
                    return Err(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        monitoring_runtime::TelemetryAgent, monitoring_service::MonitoringService,
    };

    fn test_config(hub: &str) -> Config {
        Config::read(|key| match key {
            "NINETY_TELEMETRY_HUB_URL" => Some(hub.into()),
            "NINETY_AGENT_ID" => Some("integration-pc".into()),
            "NINETY_TELEMETRY_TOKEN" => Some("fixture-token".into()),
            "NINETY_TELEMETRY_INTERVAL_MS" => Some("1000".into()),
            _ => None,
        })
        .unwrap()
        .unwrap()
    }

    #[test]
    fn payload_trims_large_process_lists_and_preserves_counts() {
        let config = test_config("http://localhost/telemetry");
        let mut snapshot = MonitoringService::new(1, NetworkMonitor::new()).collect_telemetry();
        snapshot.hardware.running_processes = (0..512)
            .map(|pid| crate::models::telemetry::Process {
                pid,
                name: "a".repeat(160),
                cpu_percent: 1.0,
                memory_bytes: 123,
            })
            .collect();
        snapshot.hardware.process_count = 512;
        let text = payload(&config, "session", 1, "online", Some(&snapshot)).unwrap();
        assert!(text.len() <= signalr::MAX_OUTBOUND_BYTES);
        let value: Value = serde_json::from_str(text.trim_end_matches('\u{1e}')).unwrap();
        let sample = &value["arguments"][0]["telemetry"];
        assert_eq!(sample["processCount"], 512);
        assert_eq!(sample["processesTruncated"], true);
        assert!(sample["runningProcesses"].as_array().unwrap().len() < 512);
    }

    #[test]
    fn payload_bounds_peripherals_and_keeps_scalar_measurements() {
        let config = test_config("http://localhost/telemetry");
        let mut snapshot = MonitoringService::new(0, NetworkMonitor::new()).collect_telemetry();
        snapshot.peripherals = Some(
            (0..256)
                .map(|i| crate::models::peripheral_status::PeripheralStatus {
                    device_id: format!("{i}-{}", "x".repeat(4096)),
                    device_name: "HID device".into(),
                    device_type: "hid".into(),
                    connected: true,
                })
                .collect(),
        );
        let text = payload(&config, "session", 1, "online", Some(&snapshot)).unwrap();
        assert!(text.len() <= signalr::MAX_OUTBOUND_BYTES);
        let value: Value = serde_json::from_str(text.trim_end_matches('\u{1e}')).unwrap();
        assert_eq!(value["arguments"][0]["telemetry"]["detailsTruncated"], true);
        assert_eq!(
            value["arguments"][0]["telemetry"]["ram"]["totalBytes"],
            snapshot.hardware.ram.total_bytes
        );
    }

    #[test]
    fn shutdown_cancels_unavailable_backend_without_waiting_for_retry() {
        let agent = TelemetryAgent::start(test_config("http://127.0.0.1:1/telemetry")).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let started = std::time::Instant::now();
        drop(agent);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    /// Run against the local ASP.NET Core fixture described in docs/telemetry.md.
    #[test]
    #[ignore = "requires the local ASP.NET Core SignalR fixture"]
    fn aspnet_core_telemetry_reconnect_and_shutdown() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        let base = std::env::var("NINETY_TEST_SIGNALR_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:5187".into());
        let config = test_config(&format!("{base}/telemetry"));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = reqwest::Client::new();
        runtime.block_on(async {
            client
                .post(format!("{base}/reset"))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
            let agent = TelemetryAgent::start(config).unwrap();
            async fn wait_state(
                client: &reqwest::Client,
                base: &str,
                predicate: impl Fn(&Value) -> bool,
            ) -> Value {
                tokio::time::timeout(Duration::from_secs(15), async {
                    loop {
                        let state: Value = client
                            .get(format!("{base}/state"))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                        if predicate(&state) {
                            return state;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .expect("fixture condition timed out")
            }
            let first = wait_state(&client, &base, |s| {
                s["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|m| m["envelope"]["telemetry"].is_object())
                    .count()
                    >= 2
            })
            .await;
            let messages = first["messages"].as_array().unwrap();
            let connection_id = messages.last().unwrap()["connectionId"].clone();
            let session = messages.last().unwrap()["envelope"]["sessionId"].clone();
            assert!(messages.iter().all(|m| m["envelope"]["status"] == "online"));
            client
                .post(format!("{base}/disconnect"))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
            let reconnected = wait_state(&client, &base, |s| {
                s["messages"].as_array().unwrap().iter().any(|m| {
                    m["connectionId"] != connection_id && m["envelope"]["telemetry"].is_object()
                })
            })
            .await;
            let last = reconnected["messages"].as_array().unwrap().last().unwrap();
            assert_eq!(last["envelope"]["sessionId"], session);
            assert!(
                last["envelope"]["telemetry"]["cpu"]["logicalCores"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
            drop(agent);
            let state = wait_state(&client, &base, |s| {
                s["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["envelope"]["status"] == "offline")
            })
            .await;
            let messages = state["messages"].as_array().unwrap();
            for pair in messages.windows(2) {
                assert!(
                    pair[0]["envelope"]["sequence"].as_u64().unwrap()
                        < pair[1]["envelope"]["sequence"].as_u64().unwrap()
                );
            }
        });
    }
    #[test]
    fn exponential_retry_is_jittered_and_capped() {
        assert_eq!(backoff(0, 0.0), Duration::from_millis(500));
        assert_eq!(backoff(1, 1.0), Duration::from_secs(2));
        assert_eq!(backoff(99, 1.0), Duration::from_secs(30));
        assert_eq!(backoff(99, 0.0), Duration::from_secs(15));
    }
}
