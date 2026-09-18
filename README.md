# Ninety Gaming House — Client Agent

Native Rust + Slint client. After login, the application contains exactly three pages: **Dashboard**, **Games**, and **Wallet**.

## Run

```sh
source "$HOME/.cargo/env" # If Rust is not already on PATH
cargo run
```

Development credentials: **player / password**, followed by demo verification code **123456**. The default `mock-auth` feature is development-only, including release builds. `cargo run --no-default-features` fails authentication closed.

## Pages

- **Dashboard:** sample playtime, games played, wallet balance, popular games, recent activity, and links to Games and Wallet.
- **Games:** sample game catalog with category highlighting. Game launching is not connected.
- **Wallet:** sample balance, bonus, loyalty points, and transaction history. No real payments or top-ups are performed.

The sidebar contains only these three pages. Login and logout are retained. The former station and booking flows and their associated code were removed.

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
    agent_controller.rs  Navigation, login wiring and logout
    login_controller.rs  Authentication callbacks and UI state
    tests.rs             Headless flow and rendering checks
    mod.rs
  services/
    login_service.rs     Credential validation and transport delegation
    mod.rs
  communication/
    server_client.rs     Authentication transport and development mock
    mod.rs
  models/
    auth.rs              Authentication data and states
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

No real backend, game launcher, or payment integration is included. Windows runtime validation remains pending.


## OTP verification

Successful password validation opens a dedicated OTP screen before dashboard access. Enter the demo code **123456**, then choose **Verify & continue** (or press Enter). Incorrect codes display an error; five failed attempts require returning to login. Back to login and logout clear the pending verification state. Dashboard, Games, and Wallet navigation remains blocked until verification succeeds.

This is a development-only simulation, not production MFA: the code is fixed, no email/SMS is sent, and no server challenge or code expiry is implemented. Mock-disabled builds reject verification. Production integration must issue and verify expiring, single-use challenges and enforce attempt limits on the server.

The screen is in `src/ui/otp/`, callbacks in `src/controllers/otp_controller.rs`, and code validation in `src/services/otp_service.rs`.
