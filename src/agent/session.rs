use crate::{
    infrastructure::{local_database::LocalDatabase, secure_store},
    agent::registration::{self, TokenResponse},
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub(crate) struct AgentSession {
    pub agent_id: uuid::Uuid,
    pub station_id: uuid::Uuid,
}

static ACCESS_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) fn persist(agent_id: &str, station_id: &str, tokens: &TokenResponse) -> Result<AgentSession, String> {
    let agent_id: uuid::Uuid = agent_id.parse().map_err(|_| "Backend returned an invalid Agent ID.".to_owned())?;
    let station_id: uuid::Uuid = station_id.parse().map_err(|_| "Backend returned an invalid Station ID.".to_owned())?;
    secure_store::set("agent-id", &agent_id.to_string()).map_err(str::to_owned)?;
    secure_store::set("station-id", &station_id.to_string()).map_err(str::to_owned)?;
    secure_store::set("refresh-token", &tokens.refresh_token).map_err(str::to_owned)?;
    LocalDatabase::open().map_err(str::to_owned)?;
    set_access_token(tokens.access_token.clone());
    Ok(AgentSession { agent_id, station_id })
}

pub(crate) fn restore() -> Result<Option<AgentSession>, &'static str> {
    let Some(agent_id) = secure_store::get("agent-id")? else { return Ok(None); };
    let Some(station_id) = secure_store::get("station-id")? else { return Ok(None); };
    let agent_id = agent_id.parse().map_err(|_| "stored Agent ID is invalid")?;
    let station_id = station_id.parse().map_err(|_| "stored Station ID is invalid")?;
    LocalDatabase::open()?;
    Ok(Some(AgentSession { agent_id, station_id }))
}

pub(crate) fn access_token() -> Option<String> {
    ACCESS_TOKEN.get()?.lock().ok()?.clone()
}

pub(crate) fn set_access_token(token: String) {
    let lock = ACCESS_TOKEN.get_or_init(|| Mutex::new(None));
    if let Ok(mut value) = lock.lock() { *value = Some(token); }
}

pub(crate) fn refresh_access_token() -> Result<Option<AgentSession>, String> {
    let Some(refresh_token) = secure_store::get("refresh-token").map_err(str::to_owned)? else {
        return Ok(None);
    };
    let url = format!("{}/api/Agent/token/refresh", registration::base_url().map_err(str::to_owned)?);
    let response = Client::builder().timeout(std::time::Duration::from_secs(20)).build()
        .map_err(|_| "Unable to initialize the network client.".to_owned())?
        .post(url).json(&RefreshRequest { refresh_token }).send()
        .map_err(|_| "Backend unavailable while refreshing Agent credentials.".to_owned())?;
    if !response.status().is_success() {
        return Err("Agent credentials were rejected. Re-registration is required.".to_owned());
    }
    let tokens: TokenResponse = response.json().map_err(|_| "Backend returned invalid refreshed credentials.".to_owned())?;
    let agent_id = secure_store::get("agent-id").map_err(str::to_owned)?.ok_or_else(|| "Agent identity is missing.".to_owned())?;
    let station_id = secure_store::get("station-id").map_err(str::to_owned)?.ok_or_else(|| "Station identity is missing.".to_owned())?;
    secure_store::set("refresh-token", &tokens.refresh_token).map_err(str::to_owned)?;
    set_access_token(tokens.access_token);
    let agent_id = agent_id.parse().map_err(|_| "stored Agent ID is invalid".to_owned())?;
    let station_id = station_id.parse().map_err(|_| "stored Station ID is invalid".to_owned())?;
    Ok(Some(AgentSession { agent_id, station_id }))
}

pub(crate) fn clear() {
    for key in ["agent-id", "station-id", "refresh-token", "database-key"] {
        let _ = secure_store::delete(key);
    }
    if let Some(lock) = ACCESS_TOKEN.get() { if let Ok(mut token) = lock.lock() { *token = None; } }
}

#[derive(Deserialize, Serialize)]
struct RefreshRequest {
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}
