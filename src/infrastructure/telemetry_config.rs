use std::{path::PathBuf, time::Duration};
use url::Url;

pub(crate) struct Config {
    pub hub: Url,
    pub station_id: uuid::Uuid,
    pub interval: Duration,
    pub max_processes: usize,
    token: Option<String>,
    token_file: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Result<Option<Self>, &'static str> {
        // Parse into a map without mutating process environment (Rust 2024).
        // Deployment environment variables always override .env values.
        let path = std::env::var_os("NINETY_ENV_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let local = PathBuf::from(".env");
                if local.exists() {
                    local
                } else {
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|p| p.join(".env")))
                        .unwrap_or(local)
                }
            });
        let values = match dotenvy::from_path_iter(path) {
            Ok(iter) => iter
                .collect::<std::result::Result<std::collections::HashMap<_, _>, _>>()
                .map_err(|e| {
            let msg = format!("invalid telemetry .env file: {}", e);
            Box::leak(msg.into_boxed_str()) as &str // Convertit la String en &'static str
        })?,
            Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                std::collections::HashMap::new()
            }
            Err(_) => return Err("cannot read telemetry .env file"),
        };
        Self::read(|name| {
            std::env::var(name)
                .ok()
                .or_else(|| values.get(name).cloned())
        })
    }

    pub(super) fn read(get: impl Fn(&str) -> Option<String>) -> Result<Option<Self>, &'static str> {
        match get("NINETY_TELEMETRY_ENABLED").as_deref() {
            Some("false" | "0") => return Ok(None),
            None | Some("true" | "1") => {}
            _ => return Err("NINETY_TELEMETRY_ENABLED must be true or false"),
        }
        let Some(hub) = get("NINETY_TELEMETRY_HUB_URL") else {
            return Ok(None);
        };
        let hub = Url::parse(&hub).map_err(|_| "invalid telemetry hub URL")?;
        let local = matches!(hub.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(hub.scheme() == "https" || hub.scheme() == "http" && local)
            || hub.host_str().is_none()
            || !hub.username().is_empty()
            || hub.password().is_some()
            || hub.query().is_some()
            || hub.fragment().is_some()
        {
            return Err(
                "hub URL must be HTTPS (HTTP allowed on loopback), with no credentials, query or fragment",
            );
        }
        let station_id = get("NINETY_STATION_ID")
            .ok_or("NINETY_STATION_ID is required")?
            .parse()
            .map_err(|_| "NINETY_STATION_ID must be a valid UUID")?;
        let number = |key, default, min, max| -> Result<u64, &'static str> {
            let n = get(key)
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|_| "invalid telemetry numeric setting")?
                .unwrap_or(default);
            if !(min..=max).contains(&n) {
                return Err("telemetry numeric setting out of range");
            }
            Ok(n)
        };
        Ok(Some(Self {
            hub,
            station_id,
            interval: Duration::from_millis(number(
                "NINETY_TELEMETRY_INTERVAL_MS",
                2000,
                1000,
                60000,
            )?),
            max_processes: number("NINETY_TELEMETRY_MAX_PROCESSES", 128, 0, 512)? as usize,
            token: get("NINETY_TELEMETRY_TOKEN"),
            token_file: get("NINETY_TELEMETRY_TOKEN_FILE").map(PathBuf::from),
        }))
    }

    /// Reread on each reconnect so provisioning can rotate an expiring token.
    /// The caller runs this file access on a blocking worker.
    pub fn token(&self) -> Result<Option<String>, &'static str> {
        use std::io::Read;
        let token = if let Some(path) = &self.token_file {
            let mut value = String::new();
            std::fs::File::open(path)
                .map_err(|_| "cannot open telemetry token file")?
                .take(16385)
                .read_to_string(&mut value)
                .map_err(|_| "cannot read telemetry token file")?;
            Some(value.trim().to_owned())
        } else {
            self.token.clone()
        };
        if let Some(value) = &token {
            if value.is_empty()
                || value.len() > 16384
                || !value.bytes().all(|b| b.is_ascii_graphic())
            {
                return Err("invalid telemetry bearer token");
            }
        }
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_is_opt_in_and_validates_security_and_bounds() {
        assert!(Config::read(|_| None).unwrap().is_none());
        assert!(
            Config::read(|key| match key {
                "NINETY_TELEMETRY_ENABLED" => Some("false".into()),
                "NINETY_TELEMETRY_HUB_URL" => Some("not-ready".into()),
                _ => None,
            })
            .unwrap()
            .is_none()
        );
        assert!(
            Config::read(|key| (key == "NINETY_TELEMETRY_ENABLED").then(|| "invalid".into()))
                .is_err()
        );
        for url in [
            "http://example.com/hub",
            "https://user:secret@example.com/hub",
            "https://example.com/hub?token=secret",
            "file:///tmp/hub",
        ] {
            assert!(
                Config::read(|key| match key {
                    "NINETY_TELEMETRY_HUB_URL" => Some(url.into()),
                    "NINETY_STATION_ID" => Some("22222222-2222-2222-2222-222222222222".into()),
                    _ => None,
                })
                .is_err()
            );
        }
        for interval in ["0", "999", "60001", "bad"] {
            assert!(
                Config::read(|key| match key {
                    "NINETY_TELEMETRY_HUB_URL" => Some("http://localhost/hub".into()),
                    "NINETY_STATION_ID" => Some("22222222-2222-2222-2222-222222222222".into()),
                    "NINETY_TELEMETRY_INTERVAL_MS" => Some(interval.into()),
                    _ => None,
                })
                .is_err()
            );
        }
        assert!(
            Config::read(|key| match key {
                "NINETY_TELEMETRY_HUB_URL" => Some("https://example.com/hub".into()),
                "NINETY_STATION_ID" => Some("22222222-2222-2222-2222-222222222222".into()),
                _ => None,
            })
            .unwrap()
            .is_some()
        );
    }
}
