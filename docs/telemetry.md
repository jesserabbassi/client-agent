# Client hardware monitoring

This implements the client side of the supplied **Gaming Client Agent – Hardware Monitoring Module** architecture. The production backend is not implemented or assumed to exist yet. `tests/signalr-fixture/` is a disposable interoperability test fixture only.

## Architecture

| Diagram component | Rust implementation | Responsibility |
| --- | --- | --- |
| MonitoringService | `src/services/monitoring_service.rs` | Owns the three monitors; produces a `Telemetry` snapshot and delegates summary getters. |
| HardwareMonitor | `src/services/hardware_monitor/` | CPU, GPU, RAM, temperatures, fans, processes and visible Windows apps. GPU and Windows APIs are isolated providers. |
| PeripheralMonitor | `src/services/peripheral_monitor.rs` | Enumerates connected input devices and detects removal between successful scans. |
| NetworkMonitor | `src/services/network_monitor.rs` | Observes actual backend connection state, invocation round-trip latency, and connection drops. |
| Telemetry / PeripheralStatus | `src/models/telemetry.rs`, `src/models/peripheral_status.rs` | Serializable data with explicit units and nullable unsupported metrics. |
| TelemetrySender | `src/infrastructure/telemetry_sender.rs` | Queues the latest snapshot, sends it through `ServerClient`, handles acknowledgements, heartbeat scheduling and reconnect policy. |
| ServerClient | `src/communication/server_client.rs` | ASP.NET Core SignalR negotiation, JSON framing, WebSocket connection and bounded IO. |
| Lifecycle/configuration | `src/services/monitoring_runtime.rs`, `src/infrastructure/telemetry_config.rs` | Starts/stops workers and reads deployment settings. |

`MonitoringService` runs on a dedicated collection thread. A capacity-one watch channel hands snapshots to `TelemetrySender`, which owns `ServerClient` on a separate Tokio network thread. Slow hardware drivers cannot block network heartbeats, and neither worker accesses Slint. Shutdown is signalled after the UI event loop exits. Authentication's pre-existing development transport is now `communication/auth_client.rs`; its behavior is unchanged.

## Environment configuration

The local `.env` is already created with telemetry **disabled** and a **placeholder** hub URL. It is ignored by Git. `.env.example` is the tracked deployment template; copy it to `.env` on another checkout.

```dotenv
NINETY_TELEMETRY_ENABLED=false
NINETY_TELEMETRY_HUB_URL=http://localhost:5000/hubs/telemetry
NINETY_AGENT_ID=customer-pc-001
NINETY_TELEMETRY_METHOD=ReportTelemetry
NINETY_TELEMETRY_INTERVAL_MS=2000
NINETY_TELEMETRY_MAX_PROCESSES=128
```

When the backend is ready, set its actual URL, assign a **unique, stable agent ID per PC**, configure its machine credential, and set `NINETY_TELEMETRY_ENABLED=true`. Restart the application to apply configuration. Setting it to `true` while the server is unavailable exercises automatic reconnect; there is no modal error and the UI remains usable. Telemetry is independent of player login/logout.

Configuration comes from `.env` in the working directory, falling back to `.env` beside the executable. `NINETY_ENV_FILE` selects an explicit path. Process environment variables override file values. Parsing does not mutate global environment variables. A Python virtual environment is not needed for this Rust application.

| Setting | Default / constraint |
| --- | --- |
| `NINETY_TELEMETRY_ENABLED` | `.env` sets `false`; if omitted, a configured URL enables telemetry. `true`/`false` or `1`/`0`. |
| `NINETY_TELEMETRY_HUB_URL` | Required to connect. HTTPS; HTTP allowed only for loopback development. No query, fragment or embedded credentials. |
| `NINETY_AGENT_ID` | Required when enabled, 1–128 bytes. Provision once per PC; do not share IDs. |
| `NINETY_TELEMETRY_METHOD` | `ReportTelemetry`; configure the future hub method here. |
| `NINETY_TELEMETRY_INTERVAL_MS` | `2000`; allowed range 1000–60000 ms. |
| `NINETY_TELEMETRY_MAX_PROCESSES` | `128`; range 0–512, applied separately to processes and opened apps. `0` disables both lists. |
| `NINETY_TELEMETRY_TOKEN` | Optional bearer token sent in HTTP headers for negotiate and WebSocket upgrade. |
| `NINETY_TELEMETRY_TOKEN_FILE` | Optional, takes precedence over inline token; reread on each reconnect to permit externally managed token rotation. |

Tokens are machine credentials, unrelated to mock player authentication. Token issuance/refresh is the responsibility of the future provisioning/backend integration. The client does not disable TLS certificate validation or log credentials, raw remote errors, URLs or telemetry payloads. Logs use `tracing` and stderr; configure `RUST_LOG=ninety_client_agent=info` in the deployment environment. A windowed Windows deployment must arrange a log sink if persistent diagnostics are required.

## Proposed backend contract

The configurable hub method takes **one object**. This is a proposed versioned contract to agree with the backend when it is implemented, not an assertion about an existing API:

```json
{
  "schemaVersion": 1,
  "agentId": "customer-pc-001",
  "sessionId": "uuid-generated-on-agent-start",
  "sequence": 1,
  "sentAtUnixMs": 1800000000000,
  "status": "online",
  "telemetry": null
}
```

`telemetry` is null for startup presence (before the first sample), stale/missing samples, and graceful offline messages. Otherwise it contains the `Telemetry` model, including the diagram's `cpuUsage`, `gpuUsage`, `ramUsage`, `cpuTemperature`, `gpuTemperature`, `fanSpeed`, `networkStatus` and `timestamp`, plus the detailed fields below. Times are UTC Unix milliseconds. `sequence` increases for the lifetime of `sessionId`, including reconnects. The method must return normally so SignalR emits a Completion acknowledgement; it need not return a value. Errors or missing acknowledgements cause reconnect with backoff.

The future backend should authorize the machine identity against `agentId`, track its active session/connection, and ignore stale connection disconnects when a replacement connection is active. Determine offline status from `OnDisconnectedAsync` and/or a server-side last-seen timeout (for example, greater than `max(45 seconds, 3 × configured sample interval)`). An offline message on graceful shutdown is best effort: a powered-off PC or a PC without network access cannot send one. Use server receive time for last-seen decisions, not the PC's potentially skewed clock.

Supported transport: direct ASP.NET Core SignalR, JSON protocol v1, negotiated WebSockets with Text, negotiation versions 0 and 1. Azure SignalR negotiate redirects, fallback SSE/long polling, MessagePack and stateful reconnect are not implemented. Service redirects fail explicitly rather than forwarding credentials to another host. Negotiation and framing follow the [ASP.NET Core transport specification](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/TransportProtocols.md) and [hub protocol specification](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md).

## Measurements and availability

| Data | Implementation / meaning |
| --- | --- |
| CPU | Total and per-logical-core utilization, name/vendor, core counts, frequency MHz. CPU deltas are primed before the first periodic sample. |
| RAM | Total, used, available, swap total and used; all bytes. Summary `ramUsage` is percent. |
| NVIDIA GPU | NVML: identity/name/driver, utilization, memory utilization and bytes, temperature °C, fan %, power and limit mW, graphics/memory clocks MHz, PCIe generation/width and performance state where supported. |
| Windows GPUs | DXGI enumerates physical NVIDIA/AMD/Intel adapters; PDH reads dedicated/shared memory usage and busiest-engine utilization. NVIDIA records are enriched from NVML when available. |
| Temperatures/fans | Available OS sensors and driver readings. Summary CPU temperature is the maximum recognized CPU sensor; GPU summary values are maxima over reported GPUs. `fanSpeed` is GPU fan percentage, not RPM or a CPU fan measurement. |
| Running processes | PID, process name, normalized CPU %, resident memory bytes. Largest-memory processes first; names bounded to 160 characters. No command lines, environment variables, executable paths or window titles. |
| Opened apps | On Windows, visible unowned top-level windows deduplicated by process ID, with process usage. Other platforms return null. This is a window-based definition and may include desktop helper processes. |
| Peripherals | Windows Raw Input keyboards, mice and HID devices: device identity, generic type name and connected flag. Unplugged devices appear with `connected=false` for the next successful sample. No keystrokes or mouse activity are read. Not a complete USB/audio/Bluetooth inventory; non-Windows returns null. |
| Network | Backend hub online/offline state at sample time; invocation completion latency in ms (includes server execution); drop observed since the previous collection. |

Hardware/driver APIs do not expose every metric on every PC. Unsupported readings are **null**, not invented zeroes. An empty GPU list with `gpuProvider="unavailable"` means no supported provider returned a device. On non-Windows systems GPU collection currently requires NVIDIA NVML; Apple/AMD/Intel GPU metrics on those systems are not implemented. Native Windows runtime validation is still required on customer hardware, especially PDH counter availability, device hotplug and multi-GPU configurations.

## Reliability and limits

- Default two-second sampling/sending; one telemetry invocation in flight, 15-second acknowledgement timeout. `networkLatencyMs` measures that invocation's round trip.
- Ten-second SignalR pings and a 45-second inbound hub-record timeout; HTTP/send timeouts of 10 seconds and total connection setup timeout of 20 seconds.
- Exponential reconnect delay with jitter, starting at 0.5–1 second and capped at 15–30 seconds. Successful acknowledgement resets backoff. A deliberate SignalR Close with `allowReconnect=false` stops the network worker until the application restarts.
- Only the latest snapshot is retained; no unbounded backlog, disk spool or replay storm. Samples older than three collection intervals are excluded. Peripheral disconnect events and intermediate changes can be coalesced during outages; this is live state telemetry, not an audit log.
- Outgoing records fit within 30 KiB, below the usual 32 KiB hub receive limit. Lists are reduced when necessary, with `processesTruncated`, `openedAppsTruncated` and `detailsTruncated` flags. Total process/app counts remain available. Incoming message storage is bounded to 64 KiB and 128 queued records.
- Shutdown cancels reconnect waits, attempts offline presence for at most two seconds, and closes the socket. An in-progress bounded network operation may finish first. The collector is signalled to stop and never joined on the UI event loop; an unresponsive native driver cannot hold up that loop.

## Validation

```sh
cargo fmt --check
cargo check
cargo test
cargo test --no-default-features
```

Tests cover configuration validation, bounded SignalR framing, negotiation, payload limits, peripheral removal, network drop tracking, real system collection and shutdown during backend unavailability. They do not require the production backend.

Optional interoperability test using the local .NET 9 fixture (loopback only):

```sh
# Terminal 1
dotnet run --project tests/signalr-fixture/SignalRFixture.csproj -- --urls http://127.0.0.1:5187

# Terminal 2; this test supplies its own configuration, independently of .env
cargo test aspnet_core_telemetry_reconnect_and_shutdown -- --ignored --nocapture
```

The fixture verifies bearer headers, real SignalR negotiation/handshake/completions, periodic hardware payloads, recovery after a forced transport disconnect, increasing sequence numbers and graceful offline presence. It is not started by the application and is not a production backend.
