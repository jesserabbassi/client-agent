#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod communication;
mod controllers;
mod models;
mod repositories;
mod services;
mod ui;

use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = ui::ClientView::new()?;
    controllers::bind(&ui);
    ui.run()?;
    Ok(())
}
