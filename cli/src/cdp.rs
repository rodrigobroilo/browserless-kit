use crate::client::Config;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Custom TLS certificate verifier that accepts all certs (needed for proxy MITM)
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

/// Run a CDP script file (JSON-lines format)
pub async fn run_script(config: &Config, script_path: &str, timeout_ms: u64) -> Result<(), String> {
    let script_content = fs::read_to_string(script_path)
        .map_err(|e| format!("Failed to read script '{}': {e}", script_path))?;

    let commands: Vec<Value> = script_content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
        .map(|l| serde_json::from_str(l))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Invalid JSON in script: {e}"))?;

    // Check if we need to tunnel through a proxy
    let proxy_url = std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("ALL_PROXY"))
        .or_else(|_| std::env::var("all_proxy"))
        .ok();

    let ws_url_str = config.ws_url();
    let cf_headers = config.ws_headers();

    let mut request = ws_url_str.clone().into_client_request()
        .map_err(|e| format!("Invalid WebSocket URL: {e}"))?;

    for (key, value) in &cf_headers {
        request.headers_mut().insert(
            reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("Invalid header name '{}': {e}", key))?,
            reqwest::header::HeaderValue::from_str(value)
                .map_err(|e| format!("Invalid header value: {e}"))?,
        );
    }

    let (ws_stream, _) = if let Some(proxy) = proxy_url {
        // Tunnel through HTTP CONNECT proxy
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let proxy_uri: url::Url = proxy.parse()
            .map_err(|e| format!("Invalid proxy URL: {e}"))?;
        let proxy_host = proxy_uri.host_str().unwrap_or("127.0.0.1");
        let proxy_port = proxy_uri.port().unwrap_or(3128);

        let target_host = std::env::var("BROWSERLESS_HOST").unwrap_or_else(|_| panic!("BROWSERLESS_HOST env var is required"));
        let target_port = 443;

        // Connect to proxy
        let proxy_stream = timeout(
            Duration::from_millis(timeout_ms),
            TcpStream::connect(format!("{proxy_host}:{proxy_port}")),
        ).await
            .map_err(|_| "Proxy connection timed out")?
            .map_err(|e| format!("Proxy connection failed: {e}"))?;

        // Send CONNECT request
        let connect_req = format!(
            "CONNECT {target_host}:{target_port} HTTP/1.1\r\nHost: {target_host}:{target_port}\r\n\r\n"
        );

        let (mut rd, mut wr) = tokio::io::split(proxy_stream);
        wr.write_all(connect_req.as_bytes()).await
            .map_err(|e| format!("CONNECT send failed: {e}"))?;

        // Read CONNECT response
        let mut buf = [0u8; 1024];
        let n = rd.read(&mut buf).await
            .map_err(|e| format!("CONNECT response read failed: {e}"))?;
        let response = String::from_utf8_lossy(&buf[..n]);
        if !response.contains("200") {
            return Err(format!("CONNECT failed: {response}"));
        }

        // Reunite the split halves
        let proxy_stream = rd.unsplit(wr);

        // TLS handshake over the tunnel — accept all certs (proxy may MITM)
        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(NoVerifier))
            .with_no_client_auth();

        let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(tls_config));

        timeout(
            Duration::from_millis(timeout_ms),
            tokio_tungstenite::client_async_tls_with_config(
                request,
                proxy_stream,
                None,
                Some(connector),
            ),
        )
        .await
        .map_err(|_| "WebSocket TLS handshake timed out")?
        .map_err(|e| format!("WebSocket connection failed: {e}"))?
    } else {
        // Direct connection (no proxy)
        timeout(
            Duration::from_millis(timeout_ms),
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| "WebSocket connection timed out")?
        .map_err(|e| format!("WebSocket connection failed: {e}"))?
    };

    let (mut write, mut read) = ws_stream.split();

    // Create a new target (page) via browser-level CDP
    let mut cmd_id = 1u64;
    let create_target = json!({
        "id": cmd_id,
        "method": "Target.createTarget",
        "params": {"url": "about:blank"}
    });
    write.send(Message::Text(create_target.to_string())).await
        .map_err(|e| format!("WebSocket send failed: {e}"))?;
    let create_resp = wait_for_response(&mut read, cmd_id, timeout_ms).await?;
    let target_id = create_resp.get("result")
        .and_then(|r| r.get("targetId"))
        .and_then(|t| t.as_str())
        .ok_or("Failed to get targetId from createTarget response")?
        .to_string();
    cmd_id += 1;

    // Attach to the target to get a session
    let attach = json!({
        "id": cmd_id,
        "method": "Target.attachToTarget",
        "params": {"targetId": target_id, "flatten": true}
    });
    write.send(Message::Text(attach.to_string())).await
        .map_err(|e| format!("WebSocket send failed: {e}"))?;
    let attach_resp = wait_for_response(&mut read, cmd_id, timeout_ms).await?;
    let session_id = attach_resp.get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .ok_or("Failed to get sessionId from attachToTarget response")?
        .to_string();
    cmd_id += 1;

    let mut results: Vec<Value> = Vec::new();

    // Helper to build CDP messages with sessionId for the target page
    let build_msg = |id: u64, method: &str, params: Value| -> Value {
        json!({
            "id": id,
            "sessionId": session_id,
            "method": method,
            "params": params,
        })
    };

    for cmd in &commands {
        let method = cmd.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = cmd.get("params").cloned().unwrap_or(json!({}));

        // Handle special built-in commands
        match method {
            "wait" => {
                let ms = params.get("ms").and_then(|m| m.as_u64()).unwrap_or(1000);
                tokio::time::sleep(Duration::from_millis(ms)).await;
                results.push(json!({"waited_ms": ms}));
                continue;
            }
            "wait_for_selector" => {
                // Use Runtime.evaluate to wait for selector
                let selector = params.get("selector").and_then(|s| s.as_str()).unwrap_or("body");
                let wait_timeout = params.get("timeout").and_then(|t| t.as_u64()).unwrap_or(10000);
                let js = format!(
                    r#"new Promise((resolve, reject) => {{
                        const el = document.querySelector('{selector}');
                        if (el) return resolve(true);
                        const obs = new MutationObserver(() => {{
                            if (document.querySelector('{selector}')) {{
                                obs.disconnect();
                                resolve(true);
                            }}
                        }});
                        obs.observe(document.body, {{childList: true, subtree: true}});
                        setTimeout(() => {{ obs.disconnect(); reject('timeout'); }}, {wait_timeout});
                    }})"#
                );
                let eval_msg = build_msg(cmd_id, "Runtime.evaluate", json!({
                    "expression": js,
                    "awaitPromise": true,
                }));
                write.send(Message::Text(eval_msg.to_string())).await
                    .map_err(|e| format!("WebSocket send failed: {e}"))?;
                cmd_id += 1;

                let result = wait_for_response(&mut read, cmd_id - 1, timeout_ms).await?;
                results.push(result);
                continue;
            }
            "get_cookies" => {
                let cookie_msg = build_msg(cmd_id, "Network.getCookies", params.clone());
                write.send(Message::Text(cookie_msg.to_string())).await
                    .map_err(|e| format!("WebSocket send failed: {e}"))?;
                cmd_id += 1;

                let result = wait_for_response(&mut read, cmd_id - 1, timeout_ms).await?;
                results.push(result);
                continue;
            }
            "click" => {
                let selector = params.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                let js = format!(
                    r#"document.querySelector('{selector}')?.click(); true"#
                );
                let eval_msg = build_msg(cmd_id, "Runtime.evaluate", json!({"expression": js}));
                write.send(Message::Text(eval_msg.to_string())).await
                    .map_err(|e| format!("WebSocket send failed: {e}"))?;
                cmd_id += 1;

                let result = wait_for_response(&mut read, cmd_id - 1, timeout_ms).await?;
                results.push(result);
                continue;
            }
            "type_text" => {
                let selector = params.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                let text = params.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let js = format!(
                    r#"(() => {{
                        const el = document.querySelector('{selector}');
                        if (el) {{
                            el.value = {text_json};
                            el.dispatchEvent(new Event('input', {{bubbles: true}}));
                            el.dispatchEvent(new Event('change', {{bubbles: true}}));
                        }}
                        return !!el;
                    }})()"#,
                    text_json = serde_json::to_string(text).unwrap(),
                );
                let eval_msg = build_msg(cmd_id, "Runtime.evaluate", json!({"expression": js}));
                write.send(Message::Text(eval_msg.to_string())).await
                    .map_err(|e| format!("WebSocket send failed: {e}"))?;
                cmd_id += 1;

                let result = wait_for_response(&mut read, cmd_id - 1, timeout_ms).await?;
                results.push(result);
                continue;
            }
            "screenshot" => {
                let screenshot_msg = build_msg(cmd_id, "Page.captureScreenshot", json!({"format": "png"}));
                write.send(Message::Text(screenshot_msg.to_string())).await
                    .map_err(|e| format!("WebSocket send failed: {e}"))?;
                cmd_id += 1;

                let result = wait_for_response(&mut read, cmd_id - 1, timeout_ms).await?;

                // Save screenshot if output path specified in params
                if let Some(output) = params.get("output").and_then(|o| o.as_str()) {
                    if let Some(data) = result.get("result").and_then(|r| r.get("data")).and_then(|d| d.as_str()) {
                        use base64::Engine;
                        let bytes = base64::engine::general_purpose::STANDARD.decode(data)
                            .map_err(|e| format!("Base64 decode failed: {e}"))?;
                        fs::write(output, &bytes)
                            .map_err(|e| format!("Failed to write screenshot: {e}"))?;
                        println!("✅ CDP screenshot saved to {output} ({} bytes)", bytes.len());
                    }
                }
                results.push(result);
                continue;
            }
            _ => {}
        }

        // Standard CDP command
        let msg = build_msg(cmd_id, method, params);

        write.send(Message::Text(msg.to_string())).await
            .map_err(|e| format!("WebSocket send failed: {e}"))?;

        let result = wait_for_response(&mut read, cmd_id, timeout_ms).await?;
        results.push(result);
        cmd_id += 1;
    }

    // Print all results
    let output = json!({
        "commands_executed": commands.len(),
        "results": results,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());

    // Close WebSocket
    let _ = write.close().await;
    Ok(())
}

/// Wait for a CDP response matching the given ID
async fn wait_for_response(
    read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    expected_id: u64,
    timeout_ms: u64,
) -> Result<Value, String> {
    let deadline = Duration::from_millis(timeout_ms);

    loop {
        match timeout(deadline, read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    // Check if this is our response
                    if let Some(id) = parsed.get("id").and_then(|i| i.as_u64()) {
                        if id == expected_id {
                            return Ok(parsed);
                        }
                    }
                    // Otherwise it's an event — skip
                }
            }
            Ok(Some(Ok(_))) => continue, // Binary or other message
            Ok(Some(Err(e))) => return Err(format!("WebSocket error: {e}")),
            Ok(None) => return Err("WebSocket closed unexpectedly".to_string()),
            Err(_) => return Err(format!("Timed out waiting for CDP response ({}ms)", timeout_ms)),
        }
    }
}
