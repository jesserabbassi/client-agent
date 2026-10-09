use crate::auth::{
    models::auth::{AuthError, LoginRequest, LoginResponse, OtpRequest},
    services::auth_client::{AuthClient, RegisterRequest},
};
pub(crate) struct LoginService<C: AuthClient> {
    client: C,
}
impl<C: AuthClient> LoginService<C> {
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
        self.client.authenticate(&request)
    }
    pub(crate) fn verify_otp(&self, request: OtpRequest) -> Result<LoginResponse, AuthError> {
        self.client.verify_otp(&request)
    }
    pub(crate) fn register(&self, request: &RegisterRequest) -> Result<LoginResponse, AuthError> {
        self.client.register(request)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    struct FakeClient;
    impl AuthClient for FakeClient {
        fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError> {
            if request.password == "password" {
                Ok(LoginResponse {
                    user_id: "test-user".into(),
                    username: request.username.clone(),
                    requires_otp: false,
                    access_token: None,
                    otp_purpose: None,
                })
            } else {
                Err(AuthError::InvalidCredentials)
            }
        }
        fn verify_otp(&self, _request: &OtpRequest) -> Result<LoginResponse, AuthError> {
            Err(AuthError::InvalidOtp)
        }
        fn register(
            &self,
            _request: &super::super::auth_client::RegisterRequest,
        ) -> Result<LoginResponse, AuthError> {
            Ok(LoginResponse {
                user_id: "test-user".into(),
                username: "player@example.com".into(),
                requires_otp: true,
                access_token: None,
                otp_purpose: Some(0),
            })
        }
        fn refresh(&self, _refresh_token: &str) -> Result<Value, AuthError> {
            Ok(Value::Null)
        }
        fn logout(&self, _access_token: &str) -> Result<(), AuthError> {
            Ok(())
        }
        fn profile(&self, _access_token: &str) -> Result<Value, AuthError> {
            Ok(Value::Null)
        }
        fn change_password(
            &self,
            _access_token: &str,
            _current: &str,
            _new: &str,
        ) -> Result<Value, AuthError> {
            Ok(Value::Null)
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
