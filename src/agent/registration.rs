use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{thread, time::Duration};

#[derive(Clone, Debug)]
pub(crate) struct RegistrationInfo {
    pub agent_id: String,
    pub station_id: String,
    pub bootstrap_secret: String,
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    #[serde(rename = "stationId")]
    station_id: &'a str,
    #[serde(rename = "machineName")]
    machine_name: &'a str,
    #[serde(rename = "version")]
    version: &'a str,
}

#[derive(Deserialize)]
struct RegisterResponse {
    #[serde(rename = "agentId")]
    agent_id: String,
    #[serde(rename = "stationId")]
    station_id: String,
    #[serde(rename = "bootstrapSecret")]
    bootstrap_secret: String,
}

#[derive(Serialize)]
struct BootstrapRequest<'a> {
    #[serde(rename = "bootstrapSecret")]
    bootstrap_secret: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    #[serde(rename = "accessToken")]
    pub(crate) access_token: String,
    #[serde(rename = "refreshToken")]
    pub(crate) refresh_token: String,
}

pub(crate) fn base_url() -> Result<String, &'static str> {
    crate::infrastructure::local_env::load();
    std::env::var("NINETY_BASE_URL")
        .or_else(|_| std::env::var("NINETY_API_BASE_URL"))
        .map(|v| v.trim_end_matches('/').to_owned())
        .map_err(|_| "Backend URL is not configured.")
}

pub(crate) fn register(
    station_id: &str,
    machine_name: &str,
    version: &str,
) -> Result<RegistrationInfo, String> {
    let url = format!("{}/api/Agent/register", base_url().map_err(str::to_owned)?);
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Unable to initialize the network client.".to_owned())?
        .post(url)
        .json(&RegisterRequest { station_id, machine_name, version })
        .send()
        .map_err(|_| "Backend unavailable. Check the network connection.".to_owned())?;
    if response.status().is_success() {
        let value = response.json::<RegisterResponse>().map_err(|_| "Backend returned an invalid registration response.".to_owned())?;
        return Ok(RegistrationInfo { agent_id: value.agent_id, station_id: value.station_id, bootstrap_secret: value.bootstrap_secret });
    }
    let status = response.status().as_u16();
    Err(match status { 404 => "Station not found.".into(), 409 => "Station already has an active or pending Agent registration.".into(), 401 | 403 => "Registration requires supervisor authorization on the backend.".into(), _ => "Unable to register this Station. Check the Station ID and try again.".into() })
}

pub(crate) fn exchange(info: &RegistrationInfo) -> Result<TokenResponse, String> {
    let url = format!("{}/api/Agent/{}/bootstrap", base_url().map_err(str::to_owned)?, info.agent_id);
    let response = Client::builder().timeout(Duration::from_secs(20)).build().map_err(|_| "Unable to initialize the network client.".to_owned())?
        .post(url).json(&BootstrapRequest { bootstrap_secret: &info.bootstrap_secret }).send()
        .map_err(|_| "Backend unavailable while checking approval.".to_owned())?;
    if response.status().is_success() { response.json().map_err(|_| "Backend returned an invalid credential response.".to_owned()) }
    else if response.status().as_u16() == 401 { Err("PENDING".into()) }
    else if response.status().as_u16() == 409 { Err("Registration rejected by the supervisor.".into()) }
    else if response.status().as_u16() == 403 { Err("Registration revoked by the supervisor.".into()) }
    else if response.status().as_u16() == 404 { Err("Agent registration no longer exists.".into()) }
    else { Err("Approval check failed. Please retry.".into()) }
}

pub(crate) fn poll_until_approved(info: RegistrationInfo, on_result: impl FnOnce(Result<TokenResponse, String>) + Send + 'static) {
    thread::spawn(move || {
        let mut callback = Some(on_result);
        let mut delay = Duration::from_secs(3);
        for _ in 0..120 {
        match exchange(&info) {
            Ok(tokens) => { callback.take().expect("registration callback")(Ok(tokens)); return; }
            Err(error) if error == "PENDING" => { thread::sleep(delay); delay = (delay * 2).min(Duration::from_secs(30)); }
            Err(error) => { callback.take().expect("registration callback")(Err(error)); return; }
        }
        }
        callback.take().expect("registration callback")(Err("Approval polling timed out. Retry registration.".into()));
    });
}
