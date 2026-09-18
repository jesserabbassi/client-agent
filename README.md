# Ninety Gaming House — Client Agent

Native Rust + Slint gaming client with development login, a sample dashboard, available-station selection, and date/time booking previews.

## Run

Run `cargo run` from the project directory. On this machine, first use `source "$HOME/.cargo/env"` if Rust is not on PATH.

Development credentials: **player / password**. The default `mock-auth` feature is development-only, including release builds. `cargo run --no-default-features` fails authentication closed; no real backend is configured.

## Architecture

The source follows the architecture diagram, organized by responsibility:

```text
src/
  main.rs
  ui/
    mod.rs                    Generated Slint bindings
    app.slint                 Root window and screen composition
    login/                    Login screen, form, and controls
    dashboard/                Dashboard presentation
    stations/                 Station grid and details
    booking/                  Date/time form and summary
  controllers/
    mod.rs                    Application binding API
    agent_controller.rs       Navigation, feature wiring, logout
    login_controller.rs       Login callbacks and error presentation
    stations_controller.rs    Selection and station view-model mapping
    booking_controller.rs     Preview callbacks and result presentation
    tests.rs                  Headless flow and rendering tests
  services/
    login_service.rs          Input validation and authentication delegation
    booking_service.rs        Calendar/time validation and pricing
  communication/
    server_client.rs          Authentication transport and development mock
  repositories/
    station_repository.rs     Existing local sample station catalog
  models/
    auth.rs                   Authentication requests, responses, states, errors
    station.rs                Station data independent of Slint
```

Each Rust layer has a `mod.rs`. Services, repositories, communication, and models have no Slint dependency in their source. Controllers translate data into UI properties and handle callbacks. The station repository currently supplies mock data only; no database or persistence is implemented.

The diagram's future Windows service, session, heartbeat, telemetry, and persistent event/session repositories are not implemented yet. Add them to their corresponding layers when their behavior is built. This layered layout supersedes the earlier feature-first organization.

## Current behavior

- Authentication runs in a short-lived worker with 700 ms simulated latency. Duplicate submissions are blocked; results return through the Slint event loop. There is no polling.
- Login opens the dashboard. Available Stations displays eight sample PCs; available PCs can be selected to see specifications. Offline, occupied, and maintenance PCs cannot be selected.
- Choose **Choose date & time**, enter a future date as YYYY-MM-DD, select a same-day whole-hour range in UTC, then **Review booking**. Pricing uses integer cents.
- Editing booking inputs clears the quote. Returning to stations resets the form; logout clears the selection and preview.
- The dashboard and station catalog are sample data. Booking does not check slot availability, reserve a PC, or take payment.
- Remember me is only an in-memory preference. No passwords or tokens are persisted or logged.
- The root window has a minimum logical size of 1120×830. Windows is the intended deployment platform; Windows runtime validation is still pending.

## Verification

```sh
cargo fmt --check
cargo check
cargo test
cargo test --no-default-features
```

Fault simulation on macOS/Linux:

```sh
NINETY_MOCK_MODE=unavailable cargo test
NINETY_MOCK_MODE=network-error cargo test
```

Set `NINETY_TEST_CAPTURE_DIR` to an existing directory when running tests to export PPM render captures. Integration coverage exercises login, selection, navigation, booking validation, logout, re-login, and renders at gaming resolutions plus the minimum window size.

Both direct dependencies are required: `slint` at runtime and `slint-build` for UI compilation. Real authentication transport must stay off the UI thread and implement timeouts. Secure OS token storage and branch-local timezone behavior remain future backend integration work.

After reviewing a booking, **Confirm demo booking** opens a simulation confirmation with a preview reference. Details are revalidated before confirmation; repeated clicks keep the same reference. No payment, reservation, email, or persistence occurs. Leaving the flow clears the preview.
