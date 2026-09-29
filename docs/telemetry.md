# Client hardware monitoring

This implements the Rust client side of the NinetyBackend monitoring contract. `tests/signalr-fixture/` is a disposable legacy fixture; the production hub is `NinetyBackend/Infrastructure/SignalR/AgentHub.cs`.

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
NINETY_TELEMETRY_HUB_URL=https://api.example.com/hubs/agents
NINETY_STATION_ID=00000000-0000-0000-0000-000000000000
NINETY_TELEMETRY_INTERVAL_MS=2000
NINETY_TELEMETRY_MAX_PROCESSES=128
NINETY_TELEMETRY_TOKEN_FILE=C:\\ProgramData\\Ninety\\agent-token
```

Provision the Agent through the backend, write the returned Agent JWT to the configured token file, set the assigned station ID, and set `NINETY_TELEMETRY_ENABLED=true`. Restart the application to apply configuration. Setting it to `true` while the server is unavailable exercises automatic reconnect; there is no modal error and the UI remains usable. Telemetry is independent of player login/logout.

Configuration comes from `.env` in the working directory, falling back to `.env` beside the executable. `NINETY_ENV_FILE` selects an explicit path. Process environment variables override file values. Parsing does not mutate global environment variables. A Python virtual environment is not needed for this Rust application.

| Setting | Default / constraint |
| --- | --- |
| `NINETY_TELEMETRY_ENABLED` | `.env` sets `false`; if omitted, a configured URL enables telemetry. `true`/`false` or `1`/`0`. |
| `NINETY_TELEMETRY_HUB_URL` | Required to connect. HTTPS; HTTP allowed only for loopback development. No query, fragment or embedded credentials. |
| `NINETY_STATION_ID` | Required when enabled; must be the assigned GamingStation UUID. |
| `NINETY_TELEMETRY_INTERVAL_MS` | `2000`; allowed range 1000–60000 ms. |
| `NINETY_TELEMETRY_MAX_PROCESSES` | `128`; range 0–512, applied separately to processes and opened apps. `0` disables both lists. |
| `NINETY_TELEMETRY_TOKEN` | Optional inline Agent JWT cookie value. |
| `NINETY_TELEMETRY_TOKEN_FILE` | Optional token file; takes precedence over inline token and is reread on reconnect. |

Tokens are machine credentials, unrelated to mock player authentication. Token issuance/refresh is the responsibility of the future provisioning/backend integration. The client does not disable TLS certificate validation or log credentials, raw remote errors, URLs or telemetry payloads. Logs use `tracing` and stderr; configure `RUST_LOG=ninety_client_agent=info` in the deployment environment. A windowed Windows deployment must arrange a log sink if persistent diagnostics are required.

## Backend SignalR contract

The client connects to `/hubs/agents`, invokes `ConnectAgent(stationId)`, sends `SendTelemetry(TelemetryDto)`, and invokes `Heartbeat()` every 15 seconds:

```json
{
  "stationId": "00000000-0000-0000-0000-000000000000",
  "cpuUsage": 0,
  "gpuUsage": 0,
  "ramUsage": 0,
  "cpuTemperature": 0,
  "gpuTemperature": 0,
  "fanSpeed": 0,
  "networkStatus": "Connected",
  "timestamp": "2026-01-01T00:00:00.000Z"
}
```

Unsupported Rust readings are mapped to zero because the backend DTO requires numeric values. Server-side identity comes only from the JWT `agent_id` claim; `stationId` must match the persisted Agent-to-Station assignment. Errors or missing acknowledgements cause reconnect with backoff.

The existing backend authorizes the Agent JWT, validates the station assignment in `ConnectAgent`, tracks active connections, updates heartbeat/presence, and persists telemetry through `IMonitoringService`.

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
