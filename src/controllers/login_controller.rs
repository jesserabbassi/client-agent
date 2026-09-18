use crate::{
    communication::server_client::DevelopmentServerClient,
    models::auth::{AuthError, AuthState, LoginRequest},
    services::login_service::LoginService,
    ui::ClientView,
};
use slint::ComponentHandle;
use std::sync::Arc;
pub(crate) fn bind(ui: &ClientView, on_authenticated: fn(&ClientView, String)) {
    present(ui, AuthState::Idle, "");
    ui.set_development_mode(cfg!(feature = "mock-auth"));
    let service = Arc::new(LoginService::new(DevelopmentServerClient));
    let weak = ui.as_weak();
    ui.on_login_requested(move |username, password, remember_me| {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_busy() || ui.get_authenticated() {
            return;
        }
        let request = LoginRequest {
            username: username.trim().into(),
            password: password.into(),
            remember_me,
        };
        if let Err(message) = LoginService::<DevelopmentServerClient>::validate(&request) {
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
                        let _token = response.access_token;
                        present(
                            &ui,
                            AuthState::Authenticated,
                            &format!("Welcome, {}. Authentication successful.", response.username),
                        );
                        on_authenticated(&ui, response.username);
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
    ui.set_connection_problem(
        matches!(
            state,
            AuthState::ServerUnavailable | AuthState::NetworkError
        ) || !cfg!(feature = "mock-auth"),
    );
    ui.set_status_text(
        match state {
            AuthState::Loading => "Connecting…",
            AuthState::ServerUnavailable => "Server unavailable",
            AuthState::NetworkError => "Connection lost",
            _ if cfg!(feature = "mock-auth") => "Connected to Ninety Server · MOCK",
            _ => "Server unavailable · backend not configured",
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
