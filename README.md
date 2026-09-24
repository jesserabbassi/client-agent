# Ninety Gaming House — Client Agent

Native Rust + Slint client. After login, the application contains exactly three pages: **Dashboard**, **Games**, and **Wallet**.

## Run

```sh
source "$HOME/.cargo/env" # If Rust is not already on PATH
cargo run
```

Development credentials: **player / password**, followed by demo verification code **123456**. The default `mock-auth` feature is development-only, including release builds. `cargo run --no-default-features` fails authentication closed.

## Pages

- **Dashboard:** use **Start a session** to play immediately without a prior reservation (choose 1–3 hours; an available demo station is assigned), or reserve one of four demo stations for 1–3 hours, see reservations, and start/end a session. Sessions cost 6.00 € per reserved hour, charged once when starting. Early finishes do not refund the fixed price.
- **Games:** starting a session opens the game picker. Choose or change the session’s game, then end the session from Games or Dashboard. Game selection is demo state; game launching is not connected.
- **Wallet:** shared balance, demo top-ups from 1.00–500.00 €, and transaction history for credits and session charges. No real money is moved.

The sidebar contains only these three pages. Login and logout are retained. Reservations are for the next available slot in this local demo. Only one session can run at a time; end it before logging out. State survives logout in this single-demo-account app and resets when the app closes. No station availability service, scheduled bookings, automatic session expiry, or payment backend is connected.

## Layered architecture

```text
src/
  main.rs
  ui/
    app.slint          Root window and authentication boundary
    shell.slint        Shared header, three-page navigation and footer
    login/             Login presentation and controls
    dashboard/         Dashboard presentation
    games/             Games catalog presentation
    wallet/            Wallet presentation
    mod.rs             Generated Slint bindings
  controllers/
    activity_controller.rs Reservations, sessions and demo wallet state
    agent_controller.rs  Navigation, login wiring and logout
    login_controller.rs  Authentication callbacks and UI state
    tests.rs             Headless flow and rendering checks
    mod.rs
  services/
    login_service.rs     Credential validation and transport delegation
    monitoring_service.rs Coordinates hardware, peripheral and network monitors
    monitoring_runtime.rs Background worker lifecycle
    hardware_monitor/    CPU, RAM, GPU, processes and platform providers
    peripheral_monitor.rs Input device connection changes
    network_monitor.rs   Hub connectivity and latency
    mod.rs
  infrastructure/
    telemetry_sender.rs  Latest-value queue, periodic send and reconnect policy
    telemetry_config.rs  .env and deployment configuration
  communication/
    auth_client.rs       Authentication transport and development mock
    server_client.rs     SignalR negotiation, WebSocket transport and JSON framing
    mod.rs
  models/
    auth.rs              Authentication data and states
    telemetry.rs         Telemetry data contract
    peripheral_status.rs Peripheral connection model
    mod.rs
  repositories/
    mod.rs               Layer reserved for future backend data access
```

Authentication runs in a short-lived worker, then updates Slint through its event loop. Duplicate submissions are blocked. No polling loops are used. Credentials are not persisted or logged. Remember me is an in-memory preference only.

## Verification

```sh
cargo fmt --check
cargo check
cargo test
cargo test --no-default-features
```

Set `NINETY_TEST_CAPTURE_DIR` to an existing directory to export PPM render captures. Tests cover login validation, worker completion, three-page navigation, signed-out navigation protection, logout and re-login, and rendering at 1120×830, 1920×1080, and 2560×1440.

Fault simulation on macOS/Linux:

```sh
NINETY_MOCK_MODE=unavailable cargo test
NINETY_MOCK_MODE=network-error cargo test
```

Client-side SignalR telemetry is implemented; its production backend is not included. Game launching and payments remain demo-only. Windows runtime validation remains pending.

## Hardware monitoring and telemetry

The monitoring module follows the supplied service/infrastructure/model architecture: `MonitoringService` coordinates `HardwareMonitor`, `PeripheralMonitor` and `NetworkMonitor`; `TelemetrySender` queues and sends through the SignalR `ServerClient`.

The hub URL and settings live in `.env` (tracked template: `.env.example`). The local placeholder is `http://localhost:5000/hubs/telemetry`, with `NINETY_TELEMETRY_ENABLED=false` while the backend is not ready. Set the real URL, a unique `NINETY_AGENT_ID`, the machine credential and `NINETY_TELEMETRY_ENABLED=true` when connecting a backend. Collection and communication run on separate background threads, independently of Slint and player login.

See [telemetry architecture, configuration, data contract and tests](docs/telemetry.md) for supported GPU metrics, app/peripheral collection, reconnect behavior and offline presence semantics.


## OTP verification

Successful password validation opens a dedicated OTP screen before dashboard access. Enter the demo code **123456**, then choose **Verify & continue** (or press Enter). Incorrect codes display an error; five failed attempts require returning to login. Back to login and logout clear the pending verification state. Dashboard, Games, and Wallet navigation remains blocked until verification succeeds.

This is a development-only simulation, not production MFA: the code is fixed, no email/SMS is sent, and no server challenge or code expiry is implemented. Mock-disabled builds reject verification. Production integration must issue and verify expiring, single-use challenges and enforce attempt limits on the server.

The screen is in `src/ui/otp/`, callbacks in `src/controllers/otp_controller.rs`, and code validation in `src/services/otp_service.rs`.
