use crate::{
    communication::server_client::ServerClient,
    models::auth::{AuthError, LoginRequest, LoginResponse},
};
pub(crate) struct LoginService<C: ServerClient> {
    client: C,
}
impl<C: ServerClient> LoginService<C> {
    pub(crate) fn new(client: C) -> Self {
        Self { client }
    }
    pub(crate) fn validate(request: &LoginRequest) -> Result<(), &'static str> {
        if request.username.trim().is_empty() {
            return Err("Enter your username or email.");
        }
        if request.password.is_empty() {
            return Err("Enter your password.");
        }
        Ok(())
    }
    pub(crate) fn authenticate(&self, request: LoginRequest) -> Result<LoginResponse, AuthError> {
        // Preference only until secure token storage exists. No credentials persisted.
        let _remember_me = request.remember_me;
        self.client.authenticate(&request)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct FakeClient;
    impl ServerClient for FakeClient {
        fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError> {
            if request.password == "password" {
                Ok(LoginResponse {
                    username: request.username.clone(),
                    access_token: None,
                })
            } else {
                Err(AuthError::InvalidCredentials)
            }
        }
    }
    fn request(username: &str, password: &str) -> LoginRequest {
        LoginRequest {
            username: username.into(),
            password: password.into(),
            remember_me: false,
        }
    }
    #[test]
    fn validates_missing_fields_without_trimming_password() {
        assert!(LoginService::<FakeClient>::validate(&request("  ", "password")).is_err());
        assert!(LoginService::<FakeClient>::validate(&request("player", "")).is_err());
        assert!(LoginService::<FakeClient>::validate(&request("player", " ")).is_ok());
    }
    #[test]
    fn forwards_success_and_failure() {
        let service = LoginService::new(FakeClient);
        assert!(service.authenticate(request("player", "password")).is_ok());
        assert!(matches!(
            service.authenticate(request("player", "wrong")),
            Err(AuthError::InvalidCredentials)
        ));
    }
}
