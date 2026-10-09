use crate::auth::models::auth::{AuthError, LoginRequest, LoginResponse, OtpRequest};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub(crate) trait AuthClient: Send + Sync {
    fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError>;
    fn verify_otp(&self, request: &OtpRequest) -> Result<LoginResponse, AuthError>;
    fn register(&self, request: &RegisterRequest) -> Result<LoginResponse, AuthError>;
    fn refresh(&self, refresh_token: &str) -> Result<Value, AuthError>;
    fn logout(&self, access_token: &str) -> Result<(), AuthError>;
    fn profile(&self, access_token: &str) -> Result<Value, AuthError>;
    fn change_password(
        &self,
        access_token: &str,
        current: &str,
        new: &str,
    ) -> Result<Value, AuthError>;
}

pub(crate) struct ServerAuthClient {
    client: Client,
    base_url: String,
    paths: EndpointPaths,
}
#[derive(Clone)]
struct EndpointPaths {
    register: String,
    login: String,
    verify_otp: String,
    refresh: String,
    logout: String,
    profile: String,
    change_password: String,
}
impl ServerAuthClient {
    pub(crate) fn from_env() -> Result<Self, AuthError> {
        crate::infrastructure::local_env::load();
        let base_url = std::env::var("NINETY_BASE_URL")
            .or_else(|_| std::env::var("NINETY_API_BASE_URL"))
            .map_err(|_| AuthError::ServerUnavailable)?
            .trim_end_matches('/')
            .to_owned();
        if base_url.is_empty() {
            tracing::error!("NINETY_BASE_URL is empty");
            return Err(AuthError::ServerUnavailable);
        }
        let local_tls = std::env::var("NINETY_ALLOW_INVALID_TLS").as_deref() == Ok("true");
        // Login can wait for the backend to dispatch the email OTP.
        let mut builder = Client::builder().timeout(Duration::from_secs(60));
        if local_tls {
            tracing::warn!("TLS certificate verification disabled by NINETY_ALLOW_INVALID_TLS");
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder.build().map_err(|error| {
            tracing::error!(%error, "could not create HTTP client");
            AuthError::NetworkError
        })?;
        let path = |key: &str, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
        Ok(Self {
            client,
            base_url,
            paths: EndpointPaths {
                register: path("NINETY_AUTH_REGISTER_PATH", "/api/Auth/register"),
                login: path("NINETY_AUTH_LOGIN_PATH", "/api/Auth/login"),
                verify_otp: path("NINETY_AUTH_VERIFY_OTP_PATH", "/api/Auth/verify-otp"),
                refresh: path("NINETY_AUTH_REFRESH_PATH", "/api/Auth/refresh"),
                logout: path("NINETY_AUTH_LOGOUT_PATH", "/api/Auth/logout"),
                profile: path("NINETY_AUTH_PROFILE_PATH", "/api/Auth/profile"),
                change_password: path(
                    "NINETY_AUTH_CHANGE_PASSWORD_PATH",
                    "/api/Auth/change-password",
                ),
            },
        })
    }
    fn parse(response: reqwest::blocking::Response) -> Result<LoginResponse, AuthError> {
        match response.status().as_u16() {
            401 => Err(AuthError::InvalidCredentials),
            200..=299 => response
                .json::<BackendAuthResponse>()
                .map(Into::into)
                .map_err(|_| AuthError::NetworkError),
            400 => Err(AuthError::InvalidOtp),
            503 => Err(AuthError::EmailDelivery),
            _ => Err(AuthError::ServerUnavailable),
        }
    }
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<reqwest::blocking::Response, AuthError> {
        tracing::info!(method = %method, path, "auth request started");
        let mut request = self
            .client
            .request(method, format!("{}{}", self.base_url, path));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        request.send().map_err(|error| {
            tracing::error!(%error, path, "auth request failed");
            AuthError::NetworkError
        })
    }
    fn parse_value(response: reqwest::blocking::Response) -> Result<Value, AuthError> {
        match response.status().as_u16() {
            401 | 403 => Err(AuthError::InvalidCredentials),
            200..=299 => response.json().map_err(|_| AuthError::NetworkError),
            _ => Err(AuthError::ServerUnavailable),
        }
    }
}
#[derive(Serialize)]
pub(crate) struct RegisterRequest {
    #[serde(rename = "firstName")]
    pub(crate) first_name: String,
    #[serde(rename = "lastName")]
    pub(crate) last_name: String,
    #[serde(rename = "email")]
    pub(crate) email: String,
    #[serde(rename = "password")]
    pub(crate) password: String,
}
#[derive(Serialize)]
struct LoginBody<'a> {
    #[serde(rename = "Email")]
    email: &'a str,
    #[serde(rename = "Password")]
    password: &'a str,
}
#[derive(Serialize)]
struct OtpBody<'a> {
    #[serde(rename = "UserId")]
    user_id: &'a str,
    #[serde(rename = "Code")]
    code: &'a str,
    #[serde(rename = "Purpose")]
    purpose: u8,
}
#[derive(Deserialize)]
struct BackendAuthResponse {
    #[serde(rename = "userId", alias = "id")]
    user_id: String,
    #[serde(rename = "email", alias = "username")]
    email: String,
    #[serde(rename = "requiresOtp", alias = "requiresOTP", alias = "otpRequired")]
    requires_otp: bool,
    #[serde(rename = "accessToken", alias = "token")]
    access_token: Option<String>,
    #[serde(rename = "otpPurpose")]
    otp_purpose: Option<u8>,
}
impl From<BackendAuthResponse> for LoginResponse {
    fn from(v: BackendAuthResponse) -> Self {
        Self {
            user_id: v.user_id,
            username: v.email,
            requires_otp: v.requires_otp,
            access_token: v.access_token,
            otp_purpose: v.otp_purpose,
        }
    }
}
impl AuthClient for ServerAuthClient {
    fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError> {
        tracing::info!(path = self.paths.login.as_str(), "login request started");
        let r = self
            .client
            .post(format!("{}{}", self.base_url, self.paths.login))
            .json(&LoginBody {
                email: &request.username,
                password: &request.password,
            })
            .send()
            .map_err(|error| {
                tracing::error!(%error, path = self.paths.login.as_str(), "login request failed");
                AuthError::NetworkError
            })?;
        tracing::info!(status = %r.status(), path = self.paths.login.as_str(), "login response received");
        Self::parse(r)
    }
    fn verify_otp(&self, request: &OtpRequest) -> Result<LoginResponse, AuthError> {
        let r = self
            .client
            .post(format!("{}{}", self.base_url, self.paths.verify_otp))
            .json(&OtpBody {
                user_id: &request.user_id,
                code: &request.code,
                purpose: request.purpose,
            })
            .send()
            .map_err(|_| AuthError::NetworkError)?;
        Self::parse(r)
    }
    fn register(&self, request: &RegisterRequest) -> Result<LoginResponse, AuthError> {
        let response = self.request(
            reqwest::Method::POST,
            &self.paths.register,
            None,
            Some(serde_json::to_value(request).unwrap_or_default()),
        )?;
        match response.status().as_u16() {
            200..=299 => response
                .json::<BackendAuthResponse>()
                .map(Into::into)
                .map_err(|_| AuthError::NetworkError),
            400 | 409 => Err(AuthError::InvalidCredentials),
            503 => Err(AuthError::EmailDelivery),
            _ => Err(AuthError::ServerUnavailable),
        }
    }
    fn refresh(&self, refresh_token: &str) -> Result<Value, AuthError> {
        Self::parse_value(self.request(
            reqwest::Method::POST,
            &self.paths.refresh,
            None,
            Some(serde_json::json!({"refreshToken": refresh_token})),
        )?)
    }
    fn logout(&self, access_token: &str) -> Result<(), AuthError> {
        Self::parse_value(self.request(
            reqwest::Method::POST,
            &self.paths.logout,
            Some(access_token),
            None,
        )?)
        .map(|_| ())
    }
    fn profile(&self, access_token: &str) -> Result<Value, AuthError> {
        Self::parse_value(self.request(
            reqwest::Method::GET,
            &self.paths.profile,
            Some(access_token),
            None,
        )?)
    }
    fn change_password(
        &self,
        access_token: &str,
        current: &str,
        new: &str,
    ) -> Result<Value, AuthError> {
        Self::parse_value(self.request(
            reqwest::Method::POST,
            &self.paths.change_password,
            Some(access_token),
            Some(serde_json::json!({"currentPassword": current, "newPassword": new})),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::RegisterRequest;

    #[test]
    fn register_payload_matches_backend_dto() {
        let payload = serde_json::to_value(RegisterRequest {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            email: "ada@example.com".into(),
            password: "example-password".into(),
        })
        .expect("serialize registration");
        assert_eq!(
            payload,
            serde_json::json!({
                "firstName": "Ada",
                "lastName": "Lovelace",
                "email": "ada@example.com",
                "password": "example-password"
            })
        );
    }
}
