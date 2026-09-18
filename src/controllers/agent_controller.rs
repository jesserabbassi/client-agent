use slint::ComponentHandle;

use crate::ui::{ClientView, StationItem};

/// Wire feature actions to the application window.
pub(crate) fn bind(ui: &ClientView) {
    super::login_controller::bind(ui, show_dashboard);
    super::stations_controller::bind(ui);
    super::booking_controller::bind(ui);
    let weak = ui.as_weak();
    ui.on_booking_requested(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_authenticated()
                && ui.get_show_stations()
                && !ui.get_show_booking()
                && ui.get_selected_station() >= 0
                && ui.get_station_detail().available
            {
                super::booking_controller::clear(&ui);
                ui.set_show_booking(true);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_stations_requested(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_authenticated() {
                ui.set_show_stations(true);
                ui.set_show_booking(false);
                super::booking_controller::clear(&ui);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_dashboard_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_show_stations(false);
            ui.set_show_booking(false);
            super::booking_controller::clear(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_logout_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_authenticated(false);
            ui.set_show_stations(false);
            ui.set_show_booking(false);
            super::booking_controller::clear(&ui);
            ui.set_selected_station(-1);
            ui.set_station_detail(StationItem::default());
            ui.set_player_name("".into());
            ui.set_password("".into());
            ui.set_message("".into());
        }
    });
}

fn show_dashboard(ui: &ClientView, username: String) {
    ui.set_player_name(username.into());
    ui.set_authenticated(true);
}
