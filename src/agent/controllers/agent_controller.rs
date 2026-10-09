use crate::{
    auth::controllers::{login_controller, otp_controller},
    monitoring::controllers::activity_controller,
    ui::{ClientView, Page},
};
use slint::ComponentHandle;
use crate::agent::registration;
use crate::ui::RegistrationState;

pub(crate) fn bind(ui: &ClientView) {
    activity_controller::bind(ui);
    otp_controller::bind(ui);
    login_controller::bind(ui, otp_controller::begin);
    bind_registration(ui);
    let weak = ui.as_weak();
    ui.on_open_full_agent(move || {
        if let Some(ui) = weak.upgrade() {
            crate::ui::leave_compact(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_navigate(move |page| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_authenticated() {
                ui.set_page(page);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_logout_requested(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.global::<crate::ui::Activity>().get_session_active() {
                ui.global::<crate::ui::Activity>()
                    .set_message("End your active session before logging out.".into());
                ui.set_page(Page::Dashboard);
                return;
            }
            otp_controller::reset(&ui);
            ui.set_authenticated(false);
            ui.set_page(Page::Dashboard);
            ui.set_player_name("".into());
            ui.set_password("".into());
            ui.set_message("".into());
        }
    });
}

fn bind_registration(ui: &ClientView) {
    ui.set_machine_name(std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "UNKNOWN NODE".into()).into());
    ui.set_agent_version(env!("CARGO_PKG_VERSION").into());
    if let Ok(Some(session)) = crate::agent::session::restore() {
        ui.set_station_id(session.station_id.to_string().into());
        ui.set_registration_state(RegistrationState::Registering);
        ui.set_message("Restoring secure Agent credentials...".into());
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let result = crate::agent::session::refresh_access_token();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    match result {
                        Ok(Some(_)) => {
                            ui.set_registration_state(RegistrationState::Approved);
                            ui.set_message("Secure Agent session restored.".into());
                            ui.set_setup_complete(true);
                        }
                        Ok(None) => {
                            ui.set_registration_state(RegistrationState::Error);
                            ui.set_message("Agent credentials are missing. Re-register this station.".into());
                            ui.set_setup_complete(false);
                        }
                        Err(error) if error_is_auth_failure(&error) => {
                            ui.set_registration_state(RegistrationState::Error);
                            ui.set_message("Agent credentials expired or were revoked. Re-register this station.".into());
                            ui.set_setup_complete(false);
                        }
                        Err(_) => {
                            ui.set_message("Backend temporarily unavailable. Retrying connection in the background.".into());
                            ui.set_setup_complete(true);
                        }
                    }
                }
            });
        });
        return;
    }
    let weak = ui.as_weak();
    ui.on_station_registration_requested(move |station_id| {
        let Some(ui) = weak.upgrade() else { return; };
        let station_id = station_id.trim().to_owned();
        if uuid::Uuid::parse_str(&station_id).is_err() {
            ui.set_registration_state(RegistrationState::Error);
            ui.set_message("Enter a valid Station ID.".into());
            return;
        }
        ui.set_registration_state(RegistrationState::Registering);
        ui.set_message("".into());
        let machine = ui.get_machine_name().to_string();
        let version = ui.get_agent_version().to_string();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let result = registration::register(&station_id, &machine, &version);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return; };
                match result {
                    Ok(info) => {
                        ui.set_registration_state(RegistrationState::Pending);
                        ui.set_message("Registration request sent. Waiting for supervisor approval.".into());
                        let weak = ui.as_weak();
                        let agent_id = info.agent_id.clone();
                        let station_id = info.station_id.clone();
                        registration::poll_until_approved(info, move |result| {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = weak.upgrade() {
                                    match result {
                                        Ok(tokens) => match crate::agent::session::persist(&agent_id, &station_id, &tokens) {
                                            Ok(_) => { ui.set_registration_state(RegistrationState::Approved); ui.set_message("Station approved. Secure setup complete.".into()); ui.set_station_id(station_id.into()); ui.set_setup_complete(true); }
                                            Err(error) => { ui.set_registration_state(RegistrationState::Error); ui.set_message(error.into()); }
                                        },
                                        Err(error) => { ui.set_registration_state(RegistrationState::Error); ui.set_message(error.into()); }
                                    }
                                }
                            });
                        });
                    }
                    Err(error) => { ui.set_registration_state(RegistrationState::Error); ui.set_message(error.into()); }
                }
            });
        });
    });
}

fn error_is_auth_failure(error: &str) -> bool {
    error.contains("rejected") || error.contains("identity is missing") || error.contains("invalid")
}
