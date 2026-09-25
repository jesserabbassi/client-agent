#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod auth;
mod communication;
mod infrastructure;
mod monitoring;
mod ui;

use slint::ComponentHandle;
use std::{fs::OpenOptions, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_path = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ninety-client-agent.log");
    let log_file = OpenOptions::new().create(true).append(true).open(&log_path)?;
    let _ = tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ninety_client_agent=info".into()),
        )
        .try_init();
    tracing::info!(path = ?log_path, "client started");
    let ui = ui::ClientView::new()?;
    agent::controllers::bind(&ui);
    let _telemetry = match monitoring::services::monitoring_runtime::TelemetryAgent::start_from_env() {
        Ok(agent) => agent,
        Err(reason) => {
            tracing::error!(reason, "Telemetry could not start");
            None
        }
    };
    ui.run()?;
    Ok(())
}
