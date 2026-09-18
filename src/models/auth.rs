// No Debug: credentials must never enter logs.
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) remember_me: bool,
}
pub(crate) struct LoginResponse {
    pub(crate) username: String,
    // Future backend tokens require OS credential storage before persistence.
    pub(crate) access_token: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthState {
    Idle,
    Loading,
    Authenticated,
    InvalidCredentials,
    ServerUnavailable,
    NetworkError,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AuthError {
    InvalidCredentials,
    ServerUnavailable,
    NetworkError,
}
