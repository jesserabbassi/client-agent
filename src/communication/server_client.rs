use crate::models::auth::{AuthError, LoginRequest, LoginResponse};
/// Transport boundary: called only from a request worker, never the UI thread.
pub(crate) trait ServerClient: Send + Sync {
    fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError>;
}
pub(crate) struct DevelopmentServerClient;
impl ServerClient for DevelopmentServerClient {
    fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError> {
        if !cfg!(feature = "mock-auth") {
            return Err(AuthError::ServerUnavailable);
        }
        // Development-only simulated latency and fault injection.
        std::thread::sleep(std::time::Duration::from_millis(700));
        match std::env::var("NINETY_MOCK_MODE").as_deref() {
            Ok("unavailable") => return Err(AuthError::ServerUnavailable),
            Ok("network-error") => return Err(AuthError::NetworkError),
            _ => {}
        }
        mock_credentials(request)
    }
}

fn mock_credentials(request: &LoginRequest) -> Result<LoginResponse, AuthError> {
    if request.username == "player" && request.password == "password" {
        Ok(LoginResponse {
            username: request.username.clone(),
            access_token: None,
        })
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "mock-auth"))]
    #[test]
    fn disabled_mock_fails_closed() {
        let request = LoginRequest {
            username: "player".into(),
            password: "password".into(),
            remember_me: false,
        };
        assert!(matches!(
            DevelopmentServerClient.authenticate(&request),
            Err(AuthError::ServerUnavailable)
        ));
    }

    #[test]
    fn development_credentials_are_exact_and_issue_no_token() {
        let good = LoginRequest {
            username: "player".into(),
            password: "password".into(),
            remember_me: true,
        };
        let response = mock_credentials(&good).expect("valid mock credentials");
        assert_eq!(response.username, "player");
        assert!(response.access_token.is_none());
        for (username, password) in [
            ("Player", "password"),
            ("player", "wrong"),
            ("player", "password "),
        ] {
            let bad = LoginRequest {
                username: username.into(),
                password: password.into(),
                remember_me: false,
            };
            assert!(matches!(
                mock_credentials(&bad),
                Err(AuthError::InvalidCredentials)
            ));
        }
    }
}
