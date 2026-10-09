use crate::{
    auth::{
        models::auth::{AuthError, OtpRequest},
        services::{auth_client::ServerAuthClient, login_service::LoginService},
    },
    ui::{ClientView, Page},
};
use slint::ComponentHandle;

pub(crate) fn begin(ui: &ClientView, username: String, user_id: String) {
    ui.set_authenticated(false);
    ui.set_player_name(username.into());
    ui.set_otp_code("".into());
    ui.set_otp_error("".into());
    ui.set_otp_attempts(0);
    ui.set_otp_user_id(user_id.into());
    ui.set_otp_pending(true);
}

pub(crate) fn reset(ui: &ClientView) {
    ui.set_otp_pending(false);
    ui.set_otp_code("".into());
    ui.set_otp_error("".into());
    ui.set_otp_attempts(0);
    ui.set_otp_user_id("".into());
    ui.set_otp_purpose(1);
}

pub(crate) fn bind(ui: &ClientView) {
    let weak = ui.as_weak();
    ui.on_otp_verify(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_otp_pending() || ui.get_authenticated() || ui.get_otp_attempts() >= 5 {
            return;
        }
        let result = ServerAuthClient::from_env()
            .map(LoginService::new)
            .and_then(|service| {
                service.verify_otp(OtpRequest {
                    user_id: ui.get_otp_user_id().to_string(),
                    code: ui.get_otp_code().to_string(),
                    purpose: ui.get_otp_purpose() as u8,
                })
            });
        ui.set_otp_code("".into());
        match result {
            Ok(response) => {
                reset(&ui);
                ui.set_message("".into());
                ui.set_page(Page::Dashboard);
                ui.set_authenticated(true);
                ui.set_player_name(response.username.into());
            }
            Err(AuthError::InvalidOtp) => {
                let attempts = ui.get_otp_attempts() + 1;
                ui.set_otp_attempts(attempts);
                ui.set_otp_error(if attempts >= 5 {
                    "Too many attempts. Return to login and sign in again.".into()
                } else {
                    "Incorrect verification code. Try again.".into()
                });
            }
            Err(_) => ui.set_otp_error(
                "Unable to verify right now. Check the server and try again.".into(),
            ),
        }
    });
    let weak = ui.as_weak();
    ui.on_otp_cancel(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_otp_pending() {
            return;
        }
        reset(&ui);
        ui.set_player_name("".into());
        ui.set_message("".into());
        ui.set_password("".into());
    });
}
