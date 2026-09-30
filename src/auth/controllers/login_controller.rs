use crate::{
    auth::{
        models::auth::{AuthError, AuthState, LoginRequest},
        services::{
            auth_client::{RegisterRequest, ServerAuthClient},
            login_service::LoginService,
        },
    },
    ui::ClientView,
};
use slint::ComponentHandle;
use std::sync::Arc;
pub(crate) fn bind(ui: &ClientView, on_authenticated: fn(&ClientView, String, String)) {
    present(ui, AuthState::Idle, "");
    ui.set_development_mode(false);
    let service = match ServerAuthClient::from_env() {
        Ok(client) => Arc::new(LoginService::new(client)),
        Err(_) => {
            present(ui, AuthState::ServerUnavailable, "Set NINETY_BASE_URL to connect to the auth server.");
            return;
        }
    };
    let weak = ui.as_weak();
    let register_service = Arc::clone(&service);
    let register_weak = ui.as_weak();
    ui.on_register_requested(move |email, password, confirmation| {
        let Some(ui) = register_weak.upgrade() else {
            return;
        };
        if ui.get_busy() || ui.get_authenticated() || ui.get_otp_pending() {
            return;
        }
        let email = email.trim().to_string();
        if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
            present(&ui, AuthState::Idle, "Enter a valid email address.");
            return;
        }
        if password.len() < 8 {
            present(
                &ui,
                AuthState::Idle,
                "Use a password with at least 8 characters.",
            );
            return;
        }
        if password != confirmation {
            present(&ui, AuthState::Idle, "Passwords do not match.");
            return;
        }
        present(&ui, AuthState::Loading, "Creating your account…");
        ui.set_password("".into());
        let handle = ui.as_weak();
        let service = Arc::clone(&register_service);
        let worker = std::thread::Builder::new()
            .name("ninety-register".into())
            .spawn(move || {
                let result = service.register(&RegisterRequest {
                    email,
                    password: password.into(),
                });
                let _ = handle.upgrade_in_event_loop(move |ui| match result {
                    Ok(()) => present(
                        &ui,
                        AuthState::Idle,
                        "Account created. Sign in to continue.",
                    ),
                    Err(AuthError::InvalidCredentials) => present(
                        &ui,
                        AuthState::Idle,
                        "Registration was rejected. Check your details or use another email.",
                    ),
                    Err(error) => {
                        let (state, message) = error.presentation();
                        present(&ui, state, message);
                    }
                });
            });
        if worker.is_err() {
            present(
                &ui,
                AuthState::NetworkError,
                "Unable to start registration. Please try again.",
            );
        }
    });
    ui.on_login_requested(move |username, password, remember_me| {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_busy() || ui.get_authenticated() || ui.get_otp_pending() {
            return;
        }
        let request = LoginRequest {
            username: username.trim().into(),
            password: password.into(),
            remember_me,
        };
        if let Err(message) = LoginService::<ServerAuthClient>::validate(&request) {
            present(&ui, AuthState::Idle, message);
            return;
        }
        present(&ui, AuthState::Loading, "Verifying your credentials…");
        ui.set_password("".into());
        let handle = ui.as_weak();
        let service = Arc::clone(&service);
        let worker = std::thread::Builder::new()
            .name("ninety-auth".into())
            .spawn(move || {
                let result = service.authenticate(request);
                // Window closure while authenticating is safe.
                let _ = handle.upgrade_in_event_loop(move |ui| match result {
                    Ok(response) => {
                        let username = response.username;
                        let user_id = response.user_id;
                        let requires_otp = response.requires_otp;
                        let _access_token = response.access_token;
                        if requires_otp {
                            present(
                                &ui,
                                AuthState::OtpRequired,
                                &format!("Welcome, {username}. Enter your verification code."),
                            );
                            on_authenticated(&ui, username, user_id);
                        } else {
                            ui.set_player_name(username.into());
                            ui.set_authenticated(true);
                            present(&ui, AuthState::Idle, "Welcome back.");
                        }
                    }
                    Err(error) => {
                        let (state, message) = error.presentation();
                        present(&ui, state, message);
                    }
                });
            });
        if worker.is_err() {
            present(
                &ui,
                AuthState::NetworkError,
                "Unable to start authentication. Please try again.",
            );
        }
    });
}
fn present(ui: &ClientView, state: AuthState, message: &str) {
    ui.set_busy(state == AuthState::Loading);
    ui.set_message(message.into());
    ui.set_connection_problem(matches!(
        state,
        AuthState::ServerUnavailable | AuthState::NetworkError
    ));
    ui.set_status_text(
        match state {
            AuthState::Loading => "Connecting…",
            AuthState::ServerUnavailable => "Server unavailable",
            AuthState::NetworkError => "Connection lost",
            _ => "Connected to Ninety Server",
        }
        .into(),
    );
}

impl AuthError {
    fn presentation(&self) -> (AuthState, &'static str) {
        match self {
            Self::InvalidCredentials => (
                AuthState::InvalidCredentials,
                "Incorrect username or password. Try again.",
            ),
            Self::ServerUnavailable => (
                AuthState::ServerUnavailable,
                "Unable to reach Ninety server. Please try again.",
            ),
            Self::NetworkError => (
                AuthState::NetworkError,
                "Connection lost. Please try again.",
            ),
            Self::InvalidOtp => (
                AuthState::InvalidCredentials,
                "Incorrect verification code. Try again.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_distinct_states() {
        assert_eq!(
            AuthError::ServerUnavailable.presentation().0,
            AuthState::ServerUnavailable
        );
        assert_eq!(
            AuthError::NetworkError.presentation().0,
            AuthState::NetworkError
        );
    }
}
