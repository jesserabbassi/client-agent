// No Debug: credentials must never enter logs.
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) remember_me: bool,
}
pub(crate) struct LoginResponse {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) requires_otp: bool,
    pub(crate) access_token: Option<String>,
}
pub(crate) struct OtpRequest { pub(crate) user_id: String, pub(crate) code: String }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthState {
    Idle,
    Loading,
    OtpRequired,
    InvalidCredentials,
    ServerUnavailable,
    NetworkError,
    InvalidOtp,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AuthError {
    InvalidCredentials,
    ServerUnavailable,
    NetworkError,
    InvalidOtp,
}
