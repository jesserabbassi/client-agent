#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod communication;
mod controllers;
mod infrastructure;
mod models;
mod repositories;
mod services;
mod ui;

use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ninety_client_agent=info".into()),
        )
        .try_init();
    let ui = ui::ClientView::new()?;
    controllers::bind(&ui);
    let _telemetry = match services::monitoring_runtime::TelemetryAgent::start_from_env() {
        Ok(agent) => agent,
        Err(reason) => {
            tracing::error!(reason, "Telemetry could not start");
            None
        }
    };
    ui.run()?;
    Ok(())
}
