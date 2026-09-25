//! ASP.NET Core SignalR JSON protocol v1 over negotiated WebSockets.
//! Errors are deliberately static: URLs, tokens and remote error bodies must not
//! appear in logs. This client never accepts server-to-client commands.
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{collections::VecDeque, time::Duration};
use tokio::{
    net::TcpStream,
    time::{Instant, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
use url::Url;

pub(crate) type Result<T> = std::result::Result<T, &'static str>;
pub(crate) const MAX_OUTBOUND_BYTES: usize = 30 * 1024;
const MAX_INBOUND_BYTES: usize = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const SERVER_TIMEOUT: Duration = Duration::from_secs(45);
pub(crate) const ACK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Records {
    partial: Vec<u8>,
    ready: VecDeque<Value>,
}

impl Records {
    fn push(&mut self, text: &str) -> Result<()> {
        if self.partial.len() + text.len() > MAX_INBOUND_BYTES {
            return Err("SignalR inbound message too large");
        }
        self.partial.extend_from_slice(text.as_bytes());
        while let Some(end) = self.partial.iter().position(|&b| b == 0x1e) {
            if self.ready.len() >= 128 {
                return Err("too many SignalR records");
            }
            let value =
                serde_json::from_slice(&self.partial[..end]).map_err(|_| "invalid SignalR JSON")?;
            self.ready.push_back(value);
            self.partial.drain(..=end);
        }
        Ok(())
    }
}

pub(crate) struct ServerClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    records: Records,
    pub last_received: Instant,
}

impl ServerClient {
    pub async fn connect(hub: &Url, token: Option<&str>) -> Result<Self> {
        timeout(Duration::from_secs(20), Self::connect_inner(hub, token))
            .await
            .map_err(|_| "SignalR connect timeout")?
    }

    async fn connect_inner(hub: &Url, token: Option<&str>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(IO_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "HTTP client initialization failed")?;
        let mut negotiate = hub.clone();
        negotiate.set_path(&format!("{}/negotiate", hub.path().trim_end_matches('/')));
        negotiate
            .query_pairs_mut()
            .append_pair("negotiateVersion", "1");
        let mut request = client.post(negotiate).body("");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| "SignalR negotiate request failed")?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 => {
                return Err("SignalR authentication rejected; check or rotate the agent token");
            }
            _ => return Err("SignalR negotiate HTTP error"),
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "SignalR negotiate body failed")?
        {
            if body.len() + chunk.len() > MAX_INBOUND_BYTES {
                return Err("SignalR negotiate response too large");
            }
            body.extend_from_slice(&chunk);
        }
        let negotiated: Value =
            serde_json::from_slice(&body).map_err(|_| "invalid SignalR negotiate response")?;
        let connection_token = negotiation_token(&negotiated)?;
        let mut websocket_url = hub.clone();
        websocket_url
            .set_scheme(if hub.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|_| "invalid WebSocket URL")?;
        websocket_url
            .query_pairs_mut()
            .append_pair("id", connection_token);
        let mut request = websocket_url
            .as_str()
            .into_client_request()
            .map_err(|_| "invalid WebSocket request")?;
        if let Some(token) = token {
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {token}")
                    .parse()
                    .map_err(|_| "invalid bearer token")?,
            );
        }
        let limits = WebSocketConfig::default()
            .max_message_size(Some(MAX_INBOUND_BYTES))
            .max_frame_size(Some(MAX_INBOUND_BYTES));
        let (socket, _) = tokio_tungstenite::connect_async_with_config(request, Some(limits), true)
            .await
            .map_err(|_| "SignalR WebSocket connection failed")?;
        let mut result = Self {
            socket,
            records: Records::default(),
            last_received: Instant::now(),
        };
        result
            .send_json(json!({"protocol":"json", "version":1}))
            .await?;
        let handshake = timeout(IO_TIMEOUT, result.receive())
            .await
            .map_err(|_| "SignalR handshake timeout")??;
        if !handshake.is_object()
            || handshake.get("error").is_some()
            || handshake.get("type").is_some()
        {
            return Err("SignalR handshake rejected");
        }
        Ok(result)
    }

    pub async fn send_json(&mut self, value: Value) -> Result<()> {
        let record = encode(&value)?;
        self.send_record(record).await
    }

    pub async fn send_record(&mut self, record: String) -> Result<()> {
        timeout(IO_TIMEOUT, self.socket.send(Message::Text(record.into())))
            .await
            .map_err(|_| "SignalR send timeout")?
            .map_err(|_| "SignalR send failed")
    }

    /// Cancellation-safe: partial records and queued coalesced messages live in self.
    pub async fn receive(&mut self) -> Result<Value> {
        loop {
            if let Some(value) = self.records.ready.pop_front() {
                return Ok(value);
            }
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    self.records.push(&text)?;
                    // Only completed hub records count as liveness, not an endless
                    // stream of incomplete JSON or WebSocket control frames.
                    if !self.records.ready.is_empty() {
                        self.last_received = Instant::now();
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    timeout(IO_TIMEOUT, self.socket.flush())
                        .await
                        .map_err(|_| "WebSocket pong timeout")?
                        .map_err(|_| "WebSocket pong failed")?;
                }
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) | None => return Err("SignalR connection closed"),
                Some(Err(_)) => return Err("SignalR receive failed"),
                _ => return Err("unexpected SignalR binary message"),
            }
        }
    }

    pub async fn close(&mut self) {
        let _ = timeout(Duration::from_secs(1), self.socket.close(None)).await;
    }
}

fn negotiation_token(value: &Value) -> Result<&str> {
    if value.get("error").is_some() {
        return Err("SignalR negotiation rejected");
    }
    if value.get("url").is_some() {
        return Err(
            "SignalR service redirects are not supported; configure a direct ASP.NET Core hub",
        );
    }
    if !value["availableTransports"]
        .as_array()
        .is_some_and(|transports| {
            transports.iter().any(|t| {
                t["transport"] == "WebSockets"
                    && t["transferFormats"]
                        .as_array()
                        .is_some_and(|f| f.iter().any(|v| v == "Text"))
            })
        })
    {
        return Err("SignalR backend does not offer WebSockets with Text");
    }
    let key = match value["negotiateVersion"].as_u64().unwrap_or(0) {
        0 => "connectionId",
        1 => "connectionToken",
        _ => return Err("unsupported negotiation version"),
    };
    value[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or("missing SignalR connection token")
}

pub(crate) fn encode(value: &Value) -> Result<String> {
    let mut record = serde_json::to_string(value).map_err(|_| "SignalR serialization failed")?;
    record.push('\u{1e}');
    if record.len() > MAX_OUTBOUND_BYTES {
        return Err("telemetry payload exceeds limit");
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_fragmented_and_coalesced_records_and_limits_memory() {
        let mut records = Records::default();
        records.push("{}\u{1e}{\"type\":").unwrap();
        assert_eq!(records.ready.pop_front(), Some(json!({})));
        records.push("6}\u{1e}{\"type\":7}\u{1e}").unwrap();
        assert_eq!(records.ready.pop_front(), Some(json!({"type":6})));
        assert_eq!(records.ready.pop_front(), Some(json!({"type":7})));
        assert!(records.push(&"x".repeat(MAX_INBOUND_BYTES + 1)).is_err());
        assert!(Records::default().push("invalid\u{1e}").is_err());
        assert!(Records::default().push(&"{}\u{1e}".repeat(129)).is_err());
    }
    #[test]
    fn negotiation_uses_secret_token_and_requires_supported_transport() {
        let mut reply = json!({"negotiateVersion":1,"connectionId":"public","connectionToken":"secret", "availableTransports":[{"transport":"WebSockets", "transferFormats":["Text"]}]});
        assert_eq!(negotiation_token(&reply).unwrap(), "secret");
        reply["negotiateVersion"] = json!(0);
        assert_eq!(negotiation_token(&reply).unwrap(), "public");
        reply["availableTransports"] = json!([]);
        assert!(negotiation_token(&reply).is_err());
        assert!(encode(&json!({"blob":"x".repeat(MAX_OUTBOUND_BYTES)})).is_err());
    }
}
