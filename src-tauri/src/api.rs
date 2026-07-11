use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Minimal representation of /health/detailed
#[derive(Debug, Deserialize)]
pub struct HealthDetailed {
    pub status: String,
    pub gateway_state: Option<String>,
    pub active_agents: Option<u32>,
    pub gateway_busy: Option<bool>,
}

/// Response from the chat streaming endpoint
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Option<Vec<ChatChoice>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub delta: Option<ChatDelta>,
    pub message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatDelta {
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub content: Option<String>,
}

/// HTTP client for the Hermes API
pub struct HermesApi {
    client: reqwest::Client,
}

impl HermesApi {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Poll /health/detailed and return a normalized status
    pub async fn poll_status(&self, base_url: &str, api_key: &str) -> ConnectionStatus {
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
                    let gateway_state = body
                        .get("gateway_state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let active = body.get("active_agents").and_then(|v| v.as_u64()).unwrap_or(0);
                    let busy = body.get("gateway_busy").and_then(|v| v.as_bool()).unwrap_or(false);

                    if active > 0 || busy {
                        ConnectionStatus::Busy
                    } else if gateway_state == "running" {
                        ConnectionStatus::Idle
                    } else {
                        ConnectionStatus::Disconnected
                    }
                }
                Err(_) => ConnectionStatus::Idle, // health ok but parse failed -> assume alive
            },
            Ok(resp) if resp.status().is_client_error() => {
                // 401/403 -> server reachable but auth failed -> still "connected" in a sense
                ConnectionStatus::Idle
            }
            _ => ConnectionStatus::Disconnected,
        }
    }

    /// Send a chat message and return the streaming URL or first response
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

        let body = serde_json::json!({
            "message": message,
            "stream": false,
        });

        let req = self.client.post(&url).json(&body);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("Request failed: {}", e))?;
        let text = resp.text().await.map_err(|e| format!("Read failed: {}", e))?;
        Ok(text)
    }

    /// Create a new session
    pub async fn create_session(&self, base_url: &str, api_key: &str) -> Result<String, String> {
        let url = format!("{}/api/sessions", base_url.trim_end_matches('/'));
        let req = self.client.post(&url);
        let req = if !api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", api_key))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| format!("Session create failed: {}", e))?;
        let body: Value = resp.json().await.map_err(|e| format!("JSON parse failed: {}", e))?;
        body.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No session id in response".to_string())
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
}
