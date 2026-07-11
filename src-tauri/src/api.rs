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

/// HTTP client for the Hermes API
pub struct HermesApi {
    client: reqwest::Client,
}

impl HermesApi {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
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
                Err(_) => ConnectionStatus::Idle,
            },
            Ok(resp) if resp.status().is_client_error() => ConnectionStatus::Idle,
            _ => ConnectionStatus::Disconnected,
        }
    }

    /// Check if a specific session is currently busy (has an active run)
    pub async fn check_session_busy(
        &self,
        base_url: &str,
        _api_key: &str,
    ) -> bool {
        // Try /v1/runs to see if any run is in_progress
        let url = format!("{}/v1/runs", base_url.trim_end_matches('/'));
        let req = self.client.get(&url);
        let req = if !_api_key.is_empty() {
            req.header("Authorization", format!("Bearer {}", _api_key))
        } else {
            req
        };

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(data) = body.get("data").and_then(|v| v.as_array()) {
                        return data.iter().any(|r| {
                            r.get("status").and_then(|s| s.as_str()) == Some("in_progress")
                        });
                    }
                }
            }
        }
        false
    }

    /// Send a chat message and return the response text
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
}