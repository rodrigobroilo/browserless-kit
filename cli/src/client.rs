use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT};
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

pub struct Config {
    pub base_url: String,
    pub token: String,
    pub cf_client_id: String,
    pub cf_client_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let base_url = std::env::var("BROWSERLESS_URL")
            .map_err(|_| "BROWSERLESS_URL not set")?;
        let token = std::env::var("BROWSERLESS_TOKEN")
            .map_err(|_| "BROWSERLESS_TOKEN not set")?;
        let cf_client_id = std::env::var("CF_ACCESS_CLIENT_ID").unwrap_or_default();
        let cf_client_secret = std::env::var("CF_ACCESS_CLIENT_SECRET").unwrap_or_default();

        Ok(Self { base_url, token, cf_client_id, cf_client_secret })
    }

    pub fn endpoint(&self, path: &str) -> String {
        let sep = if path.contains('?') { "&" } else { "?" };
        format!("{}{}{sep}token={}", self.base_url.trim_end_matches('/'), path, self.token)
    }

    /// WebSocket URL for CDP
    pub fn ws_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        let ws_base = if base.starts_with("https://") {
            base.replacen("https://", "wss://", 1)
        } else if base.starts_with("http://") {
            base.replacen("http://", "ws://", 1)
        } else {
            format!("wss://{base}")
        };
        format!("{ws_base}?token={}", self.token)
    }

    fn cf_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if !self.cf_client_id.is_empty() {
            if let Ok(v) = HeaderValue::from_str(&self.cf_client_id) {
                headers.insert("CF-Access-Client-Id", v);
            }
        }
        if !self.cf_client_secret.is_empty() {
            if let Ok(v) = HeaderValue::from_str(&self.cf_client_secret) {
                headers.insert("CF-Access-Client-Secret", v);
            }
        }
        headers.insert(USER_AGENT, HeaderValue::from_static(UA));
        headers
    }

    pub fn http_client(&self, timeout_secs: u64) -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .default_headers(self.cf_headers())
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))
    }

    /// Build headers for tungstenite WebSocket (CF-Access + User-Agent)
    pub fn ws_headers(&self) -> Vec<(String, String)> {
        let mut hdrs = vec![
            ("User-Agent".to_string(), UA.to_string()),
        ];
        if !self.cf_client_id.is_empty() {
            hdrs.push(("CF-Access-Client-Id".to_string(), self.cf_client_id.clone()));
        }
        if !self.cf_client_secret.is_empty() {
            hdrs.push(("CF-Access-Client-Secret".to_string(), self.cf_client_secret.clone()));
        }
        hdrs
    }

    /// POST JSON, return raw bytes (for binary responses like screenshots/PDFs)
    pub async fn post_bytes(&self, path: &str, body: &serde_json::Value, timeout: u64) -> Result<Vec<u8>, String> {
        let client = self.http_client(timeout)?;
        let url = self.endpoint(path);

        let resp = client.post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {text}"));
        }

        resp.bytes().await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Read body failed: {e}"))
    }

    /// POST JSON, return text
    pub async fn post_text(&self, path: &str, body: &serde_json::Value, timeout: u64) -> Result<String, String> {
        let client = self.http_client(timeout)?;
        let url = self.endpoint(path);

        let resp = client.post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {text}"));
        }
        Ok(text)
    }

    /// POST JSON, return parsed JSON
    pub async fn post_json(&self, path: &str, body: &serde_json::Value, timeout: u64) -> Result<serde_json::Value, String> {
        let text = self.post_text(path, body, timeout).await?;
        serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}\nBody: {text}"))
    }

    /// GET, return text
    pub async fn get_text(&self, path: &str, timeout: u64) -> Result<String, String> {
        let client = self.http_client(timeout)?;
        let url = self.endpoint(path);

        let resp = client.get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {text}"));
        }
        Ok(text)
    }

    /// GET, return parsed JSON
    pub async fn get_json(&self, path: &str, timeout: u64) -> Result<serde_json::Value, String> {
        let text = self.get_text(path, timeout).await?;
        serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}\nBody: {text}"))
    }
}
