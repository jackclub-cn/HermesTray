use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Connection status reported to the frontend/tray
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "busy")]
    Busy,
}

/// How long an uncorroborated "busy" claim is trusted before it is treated as
/// a stale counter. See [`BusyWatch`].
pub const BUSY_STALE_AFTER_SECS: u64 = 900;

/// The parts of a `/health/detailed` payload that decide busy vs. idle.
///
/// `active_agents` is what Hermes exposes, but it is read from the persisted
/// `gateway_state.json`, which is only rewritten on turn boundaries — a counter
/// left behind by a killed turn can therefore claim "busy" for days. The live
/// counters (`api_server.active_runs`, background queues) are read from the
/// running process and are used to tell a real turn from such a leftover.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HealthSignals {
    pub gateway_running: bool,
    pub active_agents: u64,
    pub gateway_busy: bool,
    pub live_api_runs: u64,
    pub live_background: u64,
}

impl HealthSignals {
    pub fn from_payload(body: &Value) -> Self {
        let gateway_state = body
            .get("gateway_state")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let active_agents = body
            .get("active_agents")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let gateway_busy = body
            .get("gateway_busy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let live_api_runs = body
            .get("api_server")
            .and_then(|v| v.get("active_runs"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let live_background = body
            .get("readiness")
            .and_then(|v| v.get("checks"))
            .and_then(|v| v.get("background_queues"))
            .map(|queues| {
                [
                    "active_api_runs",
                    "process_completions",
                    "active_delegations",
                ]
                .iter()
                .filter_map(|key| queues.get(*key).and_then(|v| v.as_u64()))
                .sum()
            })
            .unwrap_or(0);

        Self {
            gateway_running: gateway_state == "running",
            active_agents,
            gateway_busy,
            live_api_runs,
            live_background,
        }
    }

    /// Does the payload claim that work is in progress?
    pub fn busy_claim(&self) -> bool {
        self.active_agents > 0 || self.gateway_busy
    }

    /// Is there live evidence that work is actually in progress?
    pub fn corroborated(&self) -> bool {
        self.live_api_runs > 0 || self.live_background > 0
    }
}

/// Outcome of aging one busy claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusyState {
    /// The payload does not claim busy — the caller decides idle/disconnected.
    Clear,
    /// Busy and believable.
    Busy,
    /// Busy claimed for too long with no live work behind it: treat as idle.
    Stale,
}

#[derive(Debug)]
struct BusyClaim {
    agents: u64,
    since: Instant,
    reported: bool,
}

/// Ages a busy claim so a stale `active_agents` counter cannot pin the tray on
/// "busy" forever: the claim is trusted while it is young, while the agent
/// count keeps moving, or while live work corroborates it; otherwise it expires
/// after `stale_after`.
#[derive(Debug, Default)]
pub struct BusyWatch {
    claim: Option<BusyClaim>,
}

impl BusyWatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(
        &mut self,
        signals: &HealthSignals,
        now: Instant,
        stale_after: Duration,
    ) -> BusyState {
        if !signals.busy_claim() {
            self.claim = None;
            return BusyState::Clear;
        }
        if signals.corroborated() {
            self.claim = None;
            return BusyState::Busy;
        }

        match self.claim {
            Some(ref mut claim) if claim.agents == signals.active_agents => {
                if now.saturating_duration_since(claim.since) >= stale_after {
                    if !claim.reported {
                        claim.reported = true;
                        eprintln!(
                            "[status] active_agents={} unchanged for {}s with no live work; \
                             treating the counter as stale",
                            claim.agents,
                            now.saturating_duration_since(claim.since).as_secs()
                        );
                    }
                    BusyState::Stale
                } else {
                    BusyState::Busy
                }
            }
            _ => {
                self.claim = Some(BusyClaim {
                    agents: signals.active_agents,
                    since: now,
                    reported: false,
                });
                BusyState::Busy
            }
        }
    }
}

/// Shared across `poll_status` calls: every command builds a fresh
/// `HermesApi`, so the aging state has to live outside the client.
fn busy_watch() -> &'static Mutex<BusyWatch> {
    static WATCH: OnceLock<Mutex<BusyWatch>> = OnceLock::new();
    WATCH.get_or_init(|| Mutex::new(BusyWatch::new()))
}

/// Map a `/health/detailed` payload plus its aged busy state to a status.
fn status_from(signals: &HealthSignals, busy: BusyState) -> ConnectionStatus {
    match busy {
        BusyState::Busy => ConnectionStatus::Busy,
        BusyState::Clear | BusyState::Stale => {
            if signals.gateway_running {
                ConnectionStatus::Idle
            } else {
                ConnectionStatus::Disconnected
            }
        }
    }
}

/// HTTP client for the Hermes API
pub struct HermesApi {
    client: reqwest::Client,
}

impl HermesApi {
    pub fn new() -> Self {
        // Share a single reqwest::Client across all HermesApi instances so the
        // connection pool / keep-alive is reused instead of being torn down on
        // every command. `Client` is cheaply cloneable (Arc inside).
        static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
        let client = CLIENT
            .get_or_init(|| {
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                    .unwrap_or_default()
            })
            .clone();
        Self { client }
    }

    /// Poll /health/detailed and return a normalized status
    pub async fn poll_status(&self, base_url: &str, api_key: &str) -> ConnectionStatus {
        self.poll_status_with(base_url, api_key, busy_watch()).await
    }

    /// Like [`Self::poll_status`] but with its own busy-aging state, so a manual
    /// "test connection" probe cannot reset the background poller's timer.
    pub async fn poll_status_isolated(&self, base_url: &str, api_key: &str) -> ConnectionStatus {
        let watch = Mutex::new(BusyWatch::new());
        self.poll_status_with(base_url, api_key, &watch).await
    }

    async fn poll_status_with(
        &self,
        base_url: &str,
        api_key: &str,
        watch: &Mutex<BusyWatch>,
    ) -> ConnectionStatus {
        let url = format!("{}/health/detailed", base_url.trim_end_matches('/'));
        let req = self.client.get(&url);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        match req.send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                Ok(body) => {
                    let signals = HealthSignals::from_payload(&body);
                    let busy = {
                        let mut watch = watch.lock().unwrap_or_else(|e| e.into_inner());
                        watch.observe(
                            &signals,
                            Instant::now(),
                            Duration::from_secs(BUSY_STALE_AFTER_SECS),
                        )
                    };
                    status_from(&signals, busy)
                }
                Err(_) => ConnectionStatus::Idle,
            },
            Ok(resp) if resp.status().is_client_error() => {
                // A bad/missing API key (401/403) must not read as "idle": the
                // server is reachable but rejecting us, so surface it as down.
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                    || resp.status() == reqwest::StatusCode::FORBIDDEN
                {
                    ConnectionStatus::Disconnected
                } else {
                    ConnectionStatus::Idle
                }
            }
            _ => ConnectionStatus::Disconnected,
        }
    }


    /// Send a chat message (non-streaming) and return the response text
    pub async fn send_message(
        &self,
        base_url: &str,
        api_key: &str,
        session_id: &str,
        message: &str,
    ) -> Result<String, String> {
        let url = format!(
            "{}/api/sessions/{}/chat",
            base_url.trim_end_matches('/'),
            session_id
        );

        // If message is a JSON array or object (multimodal content), use it directly
        // otherwise use it as a plain text message.
        let body = if let Ok(json_value) = serde_json::from_str::<Value>(message) {
            serde_json::json!({
                "message": json_value,
                "stream": false,
            })
        } else {
            serde_json::json!({
                "message": message,
                "stream": false,
            })
        };

        let req = self.client.post(&url).json(&body);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| friendly_error(&e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("Read failed: {}", e))?;

        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status.as_u16(), text));
        }

        // Parse Hermes response: {"message": {"content": "..."}}
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            // Hermes session chat: message.content
            if let Some(content) = json
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                return Ok(content.to_string());
            }
            // OpenAI-style: choices[0].message.content
            if let Some(content) = json
                .get("choices")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                return Ok(content.to_string());
            }
            // OpenAI-style delta: choices[0].delta.content
            if let Some(content) = json
                .get("choices")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                return Ok(content.to_string());
            }
            // Fall back to raw text
            return Ok(text);
        }

        // Plain text response
        Ok(text)
    }

    /// Create a new session
    pub async fn create_session(&self, base_url: &str, api_key: &str) -> Result<String, String> {
        let url = format!("{}/api/sessions", base_url.trim_end_matches('/'));
        let req = self.client.post(&url).json(&serde_json::json!({}));
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("Session create failed: {}", e))?;
        let status = resp.status();
        let body: Value = resp.json().await.map_err(|e| format!("JSON parse failed: {}", e))?;

        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status.as_u16(), body));
        }

        // Response shape: {"session": {"id": "api_xxx"}}
        body.get("session")
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("No session id in response: {}", body))
    }

    /// List sessions
    pub async fn list_sessions(&self, base_url: &str, api_key: &str) -> Result<Vec<Value>, String> {
        let url = format!("{}/api/sessions", base_url.trim_end_matches('/'));
        let req = self.client.get(&url);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("List sessions failed: {}", e))?;
        let body: Value = resp.json().await.map_err(|e| format!("JSON parse failed: {}", e))?;
        Ok(body
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Delete a session
    pub async fn delete_session(&self, base_url: &str, api_key: &str, session_id: &str) -> Result<(), String> {
        let url = format!(
            "{}/api/sessions/{}",
            base_url.trim_end_matches('/'),
            session_id
        );
        let req = self.client.delete(&url);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("Delete session failed: {}", e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP {}: {}", resp.status().as_u16(), resp.text().await.unwrap_or_default()))
        }
    }

    /// Get messages of a session (defensive: tries several common shapes)
    pub async fn get_session_messages(
        &self,
        base_url: &str,
        api_key: &str,
        session_id: &str,
    ) -> Result<Vec<Value>, String> {
        let url = format!(
            "{}/api/sessions/{}/messages",
            base_url.trim_end_matches('/'),
            session_id
        );
        let req = self.client.get(&url);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("Get messages failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}: {}", resp.status().as_u16(), resp.text().await.unwrap_or_default()));
        }
        let body: Value = resp.json().await.map_err(|e| format!("JSON parse failed: {}", e))?;
        Ok(body
            .get("messages")
            .or_else(|| body.get("data"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Send a chat message with SSE streaming. Emits `chat-stream` events to the
    /// frontend as tokens arrive. Returns the full response text.
    ///
    /// Cancellable via `cancel_rx` (broadcast receiver).
    pub async fn send_message_stream(
        &self,
        app: &tauri::AppHandle,
        base_url: &str,
        api_key: &str,
        session_id: &str,
        message: &str,
        mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<String, String> {
        let url = format!(
            "{}/api/sessions/{}/chat",
            base_url.trim_end_matches('/'),
            session_id
        );

        // If message is a JSON array or object (multimodal content), use it directly
        let body = if let Ok(json_value) = serde_json::from_str::<Value>(message) {
            serde_json::json!({
                "message": json_value,
                "stream": true,
            })
        } else {
            serde_json::json!({
                "message": message,
                "stream": true,
            })
        };

        let req = self.client.post(&url).json(&body);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let mut resp = req
            .send()
            .await
            .map_err(|e| friendly_error(&e.to_string()))?;

        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {}: {}", code, text));
        }

        // Accumulate raw bytes (not a String): the network may split a multi-byte
        // UTF-8 char across chunks, and `from_utf8_lossy` on a partial chunk would
        // replace it with U+FFFD. We split on the LF byte (0x0A), which never
        // appears inside a multi-byte UTF-8 sequence, so each line is decodable
        // as a whole.
        let mut buf: Vec<u8> = Vec::new();
        let mut full = String::new();
        // Track the last full content so we can emit only the delta, whether the
        // server sends cumulative content or per-token deltas.
        let mut last_content = String::new();

        let emit = |delta: &str, done: bool| {
            let _ = app.emit(
                "chat-stream",
                serde_json::json!({
                    "session_id": session_id,
                    "delta": delta,
                    "done": done,
                }),
            );
        };

        loop {
            let chunk = tokio::select! {
                _ = cancel_rx.recv() => break,
                chunk = resp.chunk() => chunk,
            };

            let chunk = match chunk {
                Ok(Some(c)) => c,
                Ok(None) => break, // EOF
                Err(e) => {
                    // Mid-stream network error: stop gracefully with what we have
                    let _ = e;
                    break;
                }
            };

            buf.extend_from_slice(&chunk);

            // Process complete lines (split on the LF byte; see buf comment above)
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line = String::from_utf8_lossy(&buf[..pos]).into_owned();
                buf.drain(..=pos);
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        // Flush remaining buffer before returning
                        flush_buffer(&buf, &mut last_content, &mut full, &emit);
                        buf.clear();
                        emit("", true);
                        return Ok(full);
                    }
                    if let Some(content) = extract_content(data) {
                        let delta = if content.starts_with(last_content.as_str()) && !last_content.is_empty() {
                            content[last_content.len()..].to_string()
                        } else {
                            content.clone()
                        };
                        // Guard: if cumulative diff is empty, emit nothing
                        if !delta.is_empty() {
                            full.push_str(&delta);
                            emit(&delta, false);
                        }
                        last_content = content;
                    }
                }
            }
        }

        // Trailing partial line without newline (or cancelled)
        flush_buffer(&buf, &mut last_content, &mut full, &emit);
        emit("", true);

        Ok(full)
    }
}

/// Extract assistant content from an SSE `data:` payload.
/// Handles Hermes `{"message": {"content": ...}}` and OpenAI-style
/// `{"choices": [{"delta"/"message": {"content": ...}}]}` shapes.
fn extract_content(data: &str) -> Option<String> {
    let v: Value = serde_json::from_str(data).ok()?;
    // Hermes session chat
    if let Some(c) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        return Some(c.to_string());
    }
    // Direct {"content": "..."}
    if let Some(c) = v.get("content").and_then(|c| c.as_str()) {
        return Some(c.to_string());
    }
    // OpenAI-style
    if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
        if let Some(first) = choices.first() {
            if let Some(c) = first
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                return Some(c.to_string());
            }
            if let Some(c) = first
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                return Some(c.to_string());
            }
        }
    }
    None
}

/// Flush any buffered partial SSE line (no trailing newline) into the stream.
fn flush_buffer(
    buf: &[u8],
    last_content: &mut String,
    full: &mut String,
    emit: &dyn Fn(&str, bool),
) {
    let raw = String::from_utf8_lossy(buf);
    let line = raw.trim();
    if let Some(data) = line.strip_prefix("data:") {
        let data = data.trim();
        if data == "[DONE]" {
            return;
        }
        if let Some(content) = extract_content(data) {
            let delta = if content.starts_with(last_content.as_str()) && !last_content.is_empty() {
                content[last_content.len()..].to_string()
            } else {
                content.clone()
            };
            if !delta.is_empty() {
                full.push_str(&delta);
                emit(&delta, false);
            }
            *last_content = content;
        }
    }
}

/// Map common network errors to friendlier messages.
fn friendly_error(err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("dns") || e.contains("resolve") || e.contains("failed to lookup") {
        "Cannot resolve host — check the API URL".into()
    } else if e.contains("connection refused") {
        "Connection refused — is Hermes API Server running?".into()
    } else if e.contains("timed out") || e.contains("timeout") {
        "Request timed out".into()
    } else if e.contains("tls") || e.contains("certificate") {
        "TLS error — check the certificate / URL scheme".into()
    } else {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn extract_content_hermes_shape() {
        let data = r#"{"message":{"content":"hello"}}"#;
        assert_eq!(extract_content(data), Some("hello".to_string()));
    }

    #[test]
    fn extract_content_direct_shape() {
        assert_eq!(extract_content(r#"{"content":"world"}"#), Some("world".to_string()));
    }

    #[test]
    fn extract_content_openai_delta() {
        let data = r#"{"choices":[{"delta":{"content":"foo"}}]}"#;
        assert_eq!(extract_content(data), Some("foo".to_string()));
    }

    #[test]
    fn extract_content_openai_message() {
        let data = r#"{"choices":[{"message":{"content":"bar"}}]}"#;
        assert_eq!(extract_content(data), Some("bar".to_string()));
    }

    #[test]
    fn extract_content_returns_none() {
        assert_eq!(extract_content("not json"), None);
        assert_eq!(extract_content(r#"{"other":1}"#), None);
    }

    #[test]
    fn flush_buffer_emits_full_delta_on_first_chunk() {
        let mut last = String::new();
        let mut full = String::new();
        let emitted: RefCell<Vec<String>> = RefCell::new(Vec::new());
        flush_buffer(
            r#"data: {"content":"你好"}"#.as_bytes(),
            &mut last,
            &mut full,
            &|delta, done| {
                assert!(!done);
                emitted.borrow_mut().push(delta.to_string());
            },
        );
        assert_eq!(full, "你好");
        assert_eq!(last, "你好");
        assert_eq!(emitted.borrow().as_slice(), ["你好".to_string()].as_slice());
    }

    #[test]
    fn flush_buffer_emits_only_diff_for_cumulative_content() {
        // Server sends cumulative content: the second flush must emit only the
        // tail beyond the previous content (and byte-slice on a char boundary).
        let mut last = String::from("你好");
        let mut full = String::from("你好");
        let emitted: RefCell<Vec<String>> = RefCell::new(Vec::new());
        flush_buffer(
            r#"data: {"content":"你好世界"}"#.as_bytes(),
            &mut last,
            &mut full,
            &|delta, _| emitted.borrow_mut().push(delta.to_string()),
        );
        assert_eq!(full, "你好世界");
        assert_eq!(last, "你好世界");
        assert_eq!(emitted.borrow().as_slice(), ["世界".to_string()].as_slice());
    }

    #[test]
    fn flush_buffer_ignores_done_marker() {
        let mut last = String::new();
        let mut full = String::new();
        let called = Cell::new(false);
        flush_buffer(b"data: [DONE]", &mut last, &mut full, &|_, _| called.set(true));
        assert!(!called.get());
        assert_eq!(full, "");
    }

    #[test]
    fn flush_buffer_ignores_non_data_lines() {
        let mut last = String::new();
        let mut full = String::new();
        let called = Cell::new(false);
        flush_buffer(b"event: message", &mut last, &mut full, &|_, _| called.set(true));
        assert!(!called.get());
        assert_eq!(full, "");
    }

    #[test]
    fn friendly_error_maps_common_causes() {
        assert!(friendly_error("dns error").contains("Cannot resolve host"));
        assert!(friendly_error("connection refused").contains("Connection refused"));
        assert!(friendly_error("request timed out").contains("timed out"));
        assert!(friendly_error("TLS handshake failed").contains("TLS error"));
        assert_eq!(friendly_error("some other error"), "some other error");
    }
}
#[cfg(test)]
mod busy_watch_tests {
    use super::*;
    use std::time::{Duration, Instant};

    const MIN: u64 = 60;

    fn payload() -> Value {
        serde_json::json!({
            "gateway_state": "running",
            "active_agents": 1,
            "gateway_busy": true,
            "api_server": { "active_runs": 0 },
            "readiness": {
                "checks": {
                    "background_queues": {
                        "active_api_runs": 0,
                        "process_completions": 0,
                        "active_delegations": 0
                    }
                }
            }
        })
    }

    // ── payload parsing ──────────────────────────────────────

    #[test]
    fn parses_busy_claim_and_gateway_state() {
        let s = HealthSignals::from_payload(&payload());
        assert!(s.gateway_running);
        assert_eq!(s.active_agents, 1);
        assert!(s.busy_claim());
        assert!(!s.corroborated());
    }

    #[test]
    fn parses_live_work_evidence() {
        let mut body = payload();
        body["api_server"]["active_runs"] = serde_json::json!(2);
        let s = HealthSignals::from_payload(&body);
        assert_eq!(s.live_api_runs, 2);
        assert!(s.corroborated());

        let mut body = payload();
        body["readiness"]["checks"]["background_queues"]["process_completions"] =
            serde_json::json!(1);
        body["readiness"]["checks"]["background_queues"]["active_delegations"] =
            serde_json::json!(2);
        let s = HealthSignals::from_payload(&body);
        assert_eq!(s.live_background, 3);
        assert!(s.corroborated());
    }

    #[test]
    fn missing_fields_default_to_not_busy() {
        let s = HealthSignals::from_payload(&serde_json::json!({}));
        assert!(!s.gateway_running);
        assert!(!s.busy_claim());
        assert!(!s.corroborated());
    }

    #[test]
    fn gateway_busy_flag_alone_counts_as_claim() {
        let mut body = payload();
        body["active_agents"] = serde_json::json!(0);
        let s = HealthSignals::from_payload(&body);
        assert!(s.busy_claim());
    }

    // ── stale-busy aging ─────────────────────────────────────

    #[test]
    fn uncorroborated_busy_goes_stale_after_timeout() {
        let mut watch = BusyWatch::new();
        let s = HealthSignals::from_payload(&payload());
        let t0 = Instant::now();
        let stale_after = Duration::from_secs(BUSY_STALE_AFTER_SECS);

        assert_eq!(watch.observe(&s, t0, stale_after), BusyState::Busy);
        assert_eq!(
            watch.observe(&s, t0 + Duration::from_secs(14 * MIN), stale_after),
            BusyState::Busy
        );
        assert_eq!(
            watch.observe(&s, t0 + Duration::from_secs(15 * MIN), stale_after),
            BusyState::Stale
        );
        // stays stale while the counter never moves
        assert_eq!(
            watch.observe(&s, t0 + Duration::from_secs(60 * MIN), stale_after),
            BusyState::Stale
        );
    }

    #[test]
    fn changing_agent_count_restarts_the_clock() {
        let mut watch = BusyWatch::new();
        let stale_after = Duration::from_secs(BUSY_STALE_AFTER_SECS);
        let t0 = Instant::now();
        let one = HealthSignals::from_payload(&payload());
        let two = {
            let mut body = payload();
            body["active_agents"] = serde_json::json!(2);
            HealthSignals::from_payload(&body)
        };

        assert_eq!(watch.observe(&one, t0, stale_after), BusyState::Busy);
        let t1 = t0 + Duration::from_secs(14 * MIN);
        assert_eq!(watch.observe(&two, t1, stale_after), BusyState::Busy);
        // 20 min after t0 is only 6 min after the count changed
        assert_eq!(
            watch.observe(&two, t0 + Duration::from_secs(20 * MIN), stale_after),
            BusyState::Busy
        );
        assert_eq!(
            watch.observe(&two, t1 + Duration::from_secs(15 * MIN), stale_after),
            BusyState::Stale
        );
    }

    #[test]
    fn live_work_evidence_keeps_busy_forever() {
        let mut watch = BusyWatch::new();
        let stale_after = Duration::from_secs(BUSY_STALE_AFTER_SECS);
        let mut body = payload();
        body["readiness"]["checks"]["background_queues"]["active_delegations"] =
            serde_json::json!(1);
        let s = HealthSignals::from_payload(&body);
        let t0 = Instant::now();
        assert_eq!(watch.observe(&s, t0, stale_after), BusyState::Busy);
        assert_eq!(
            watch.observe(&s, t0 + Duration::from_secs(120 * MIN), stale_after),
            BusyState::Busy
        );
    }

    #[test]
    fn idle_observation_clears_the_claim() {
        let mut watch = BusyWatch::new();
        let stale_after = Duration::from_secs(BUSY_STALE_AFTER_SECS);
        let busy = HealthSignals::from_payload(&payload());
        let idle = {
            let mut body = payload();
            body["active_agents"] = serde_json::json!(0);
            body["gateway_busy"] = serde_json::json!(false);
            HealthSignals::from_payload(&body)
        };
        let t0 = Instant::now();
        assert_eq!(watch.observe(&busy, t0, stale_after), BusyState::Busy);
        assert_eq!(
            watch.observe(&idle, t0 + Duration::from_secs(10), stale_after),
            BusyState::Clear
        );
        // a fresh claim after an idle gap must start its own clock
        let t1 = t0 + Duration::from_secs(20);
        assert_eq!(watch.observe(&busy, t1, stale_after), BusyState::Busy);
        assert_eq!(
            watch.observe(&busy, t1 + Duration::from_secs(14 * MIN), stale_after),
            BusyState::Busy
        );
    }

    // ── status mapping ───────────────────────────────────────

    #[test]
    fn status_mapping() {
        let running = HealthSignals::from_payload(&payload());
        let mut stopped_body = payload();
        stopped_body["gateway_state"] = serde_json::json!("stopped");
        let stopped = HealthSignals::from_payload(&stopped_body);

        assert_eq!(status_from(&running, BusyState::Busy), ConnectionStatus::Busy);
        assert_eq!(status_from(&running, BusyState::Clear), ConnectionStatus::Idle);
        // a stale claim must degrade to idle instead of a permanent busy icon
        assert_eq!(status_from(&running, BusyState::Stale), ConnectionStatus::Idle);
        assert_eq!(
            status_from(&stopped, BusyState::Clear),
            ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn stale_timeout_is_15_minutes() {
        assert_eq!(BUSY_STALE_AFTER_SECS, 900);
    }
}
