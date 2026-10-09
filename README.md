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
- **Games:** starting a session opens the game picker. Choosing a game launches its installed executable or launcher; the side strip appears after the launch request succeeds. If a launcher is missing, the game picker shows the error and stays open.
- Choosing a game during an active session switches the full agent to a narrow side strip above ordinary windows. On macOS its width is calculated from the display's reported physical size to be about 3 cm. On Windows it uses a 150 logical pixel width and the active monitor's work area. The strip shows elapsed time and time remaining, updated every second. **Open** restores the previous window size, position, and maximized or fullscreen state; **End** stops the timer and restores the dashboard.
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

The login screen also has **Create an account** with first name, last name, email, password, and confirmation. The client sends `firstName`, `lastName`, `email`, and `password` to the ASP.NET backend at `POST /api/Auth/register`, then opens the registration OTP screen when the backend requests verification. Run the backend with `dotnet run --project ../backend/NinetyBackend --launch-profile http` from this client directory, or set `NINETY_BASE_URL` to its URL. The client `.env` points to `http://127.0.0.1:5268`. The backend stores accounts in its configured PostgreSQL database and sends OTP codes using its SMTP configuration.

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

Client-side SignalR telemetry is implemented; its production backend is not included. Payments remain demo-only. Windows runtime validation remains pending.

Game launch paths can be set in `.env` with `NINETY_GAME_CS2`, `NINETY_GAME_VALORANT`, `NINETY_GAME_LOL`, `NINETY_GAME_DOTA2`, `NINETY_GAME_FC24`, and `NINETY_GAME_ROCKET_LEAGUE`. Use an absolute executable path or a macOS `.app` bundle. Optional arguments use a JSON array in the corresponding `_ARGS` setting. Counter-Strike 2 and Dota 2 use their Steam app IDs when no path is configured and Steam is installed. The client starts the launcher and does not verify that the game finishes loading.

To configure a game on a station, add its absolute executable path to that station's client `.env` and restart the client. Windows paths can use forward slashes, for example `NINETY_GAME_ROCKET_LEAGUE="C:/Games/Rocket League/RocketLeague.exe"`; macOS apps can use a bundle path such as `NINETY_GAME_LOL="/Applications/Your Game.app"`. Quote paths containing spaces. Launchers that need command-line arguments can use a JSON string array in the matching `_ARGS` variable. The configured game must be installed on the same computer as the client.

| Game | `.env` setting | Default launcher |
| --- | --- | --- |
| Counter-Strike 2 | `NINETY_GAME_CS2` | Steam, if installed |
| Valorant | `NINETY_GAME_VALORANT` | Configure path |
| League of Legends | `NINETY_GAME_LOL` | Configure path |
| Dota 2 | `NINETY_GAME_DOTA2` | Steam, if installed |
| EA FC 24 | `NINETY_GAME_FC24` | Configure path |
| Rocket League | `NINETY_GAME_ROCKET_LEAGUE` | Configure path |

## Hardware monitoring and telemetry

The monitoring module follows the supplied service/infrastructure/model architecture: `MonitoringService` coordinates `HardwareMonitor`, `PeripheralMonitor` and `NetworkMonitor`; `TelemetrySender` queues and sends through the SignalR `ServerClient`.

The hub URL and settings live in `.env` (tracked template: `.env.example`). The production hub is `/hubs/agents`. Set the assigned `NINETY_STATION_ID`, an Agent JWT token file, and `NINETY_TELEMETRY_ENABLED=true` when connecting a backend. The JWT contains the server-issued Agent identity; the client does not choose or transmit an arbitrary Agent ID. Collection and communication run on separate background threads, independently of Slint and player login.

See [telemetry architecture, configuration, data contract and tests](docs/telemetry.md) for supported GPU metrics, app/peripheral collection, reconnect behavior and offline presence semantics.


## OTP verification

Successful password validation opens a dedicated OTP screen before dashboard access. Enter the demo code **123456**, then choose **Verify & continue** (or press Enter). Incorrect codes display an error; five failed attempts require returning to login. Back to login and logout clear the pending verification state. Dashboard, Games, and Wallet navigation remains blocked until verification succeeds.

This is a development-only simulation, not production MFA: the code is fixed, no email/SMS is sent, and no server challenge or code expiry is implemented. Mock-disabled builds reject verification. Production integration must issue and verify expiring, single-use challenges and enforce attempt limits on the server.

The screen is in `src/ui/otp/`, callbacks in `src/controllers/otp_controller.rs`, and code validation in `src/services/otp_service.rs`.
