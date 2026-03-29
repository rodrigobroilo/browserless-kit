use crate::client::Config;
use serde_json::json;
use std::fs;

/// Screenshot: URL or HTML file → PNG
pub async fn screenshot(
    config: &Config,
    url: Option<String>,
    html_file: Option<String>,
    output: &str,
    width: u32,
    height: u32,
    full_page: bool,
    wait_for: Option<String>,
    delay: Option<u64>,
    timeout: u64,
    json_output: bool,
) -> Result<(), String> {
    let mut body = json!({
        "options": {
            "type": "png",
            "fullPage": full_page,
        },
        "viewport": {
            "width": width,
            "height": height,
        }
    });

    // HTML file takes precedence over URL
    if let Some(html_path) = html_file {
        let html_content = fs::read_to_string(&html_path)
            .map_err(|e| format!("Failed to read HTML file '{}': {e}", html_path))?;
        body["html"] = json!(html_content);
    } else if let Some(ref u) = url {
        body["url"] = json!(u);
    } else {
        return Err("Either <url> or --html <file> is required".to_string());
    }

    if let Some(selector) = wait_for {
        body["waitForSelector"] = json!({
            "selector": selector,
            "timeout": timeout * 1000,
        });
    }

    if let Some(ms) = delay {
        // Use gotoOptions.waitUntil + addScriptTag to implement delay
        body["gotoOptions"] = json!({
            "waitUntil": "networkidle2",
        });
        body["addScriptTag"] = json!([{
            "content": format!("await new Promise(r => setTimeout(r, {ms}));")
        }]);
    }

    let bytes = config.post_bytes("/screenshot", &body, timeout).await?;
    fs::write(output, &bytes)
        .map_err(|e| format!("Failed to write to '{}': {e}", output))?;

    if json_output {
        println!("{}", json!({
            "output": output,
            "size_bytes": bytes.len(),
            "width": width,
            "height": height,
        }));
    } else {
        println!("✅ Screenshot saved to {} ({} bytes)", output, bytes.len());
    }
    Ok(())
}

/// Content extraction
pub async fn content(
    config: &Config,
    url: &str,
    _format: &str,
    timeout: u64,
    json_output: bool,
) -> Result<(), String> {
    let body = json!({
        "url": url,
    });

    let text = config.post_text("/content", &body, timeout).await?;

    if json_output {
        println!("{}", json!({
            "url": url,
            "content": text,
            "length": text.len(),
        }));
    } else {
        println!("{text}");
    }
    Ok(())
}

/// PDF generation
pub async fn pdf(
    config: &Config,
    url: &str,
    output: &str,
    landscape: bool,
    format: &str,
    timeout: u64,
    json_output: bool,
) -> Result<(), String> {
    let body = json!({
        "url": url,
        "options": {
            "landscape": landscape,
            "format": format,
            "printBackground": true,
            "margin": {
                "top": "0.4in",
                "bottom": "0.4in",
                "left": "0.4in",
                "right": "0.4in",
            }
        }
    });

    let bytes = config.post_bytes("/pdf", &body, timeout).await?;
    fs::write(output, &bytes)
        .map_err(|e| format!("Failed to write to '{}': {e}", output))?;

    if json_output {
        println!("{}", json!({
            "output": output,
            "size_bytes": bytes.len(),
        }));
    } else {
        println!("✅ PDF saved to {} ({} bytes)", output, bytes.len());
    }
    Ok(())
}

/// Scrape elements
pub async fn scrape(
    config: &Config,
    url: &str,
    selector: &str,
    attributes: &str,
    timeout: u64,
    json_output: bool,
) -> Result<(), String> {
    let _props: Vec<&str> = attributes.split(',').map(|s| s.trim()).collect();

    // Browserless v2 uses different scrape schema
    let body = json!({
        "url": url,
        "elements": [{
            "selector": selector,
        }]
    });

    let result = config.post_json("/scrape", &body, timeout).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    } else {
        // Pretty-print results
        if let Some(data) = result.get("data") {
            if let Some(arr) = data.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    if let Some(results) = item.get("results").and_then(|r| r.as_array()) {
                        println!("Selector: {selector} ({} results)", results.len());
                        println!("{}", "─".repeat(60));
                        for (j, r) in results.iter().enumerate() {
                            if let Some(obj) = r.as_object() {
                                print!("  [{}] ", j + 1);
                                for (key, val) in obj {
                                    let v = val.as_str().unwrap_or("").trim();
                                    if !v.is_empty() {
                                        print!("{key}={} ", truncate(v, 100));
                                    }
                                }
                                println!();
                            }
                        }
                    } else {
                        println!("[{i}] {item}");
                    }
                }
            }
        } else {
            println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
        }
    }
    Ok(())
}

/// Health check
pub async fn health(config: &Config, timeout: u64, json_output: bool) -> Result<(), String> {
    let result = config.get_json("/pressure", timeout).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    } else {
        println!("🏥 Browserless Health");
        println!("{}", "─".repeat(40));

        // Browserless v2 pressure response
        if let Some(pressure) = result.as_object() {
            for (key, val) in pressure {
                println!("  {key}: {val}");
            }
        }
        println!("\n✅ Browserless is online");
    }
    Ok(())
}

/// HTTP fetch through browserless (via /content endpoint with JS execution)
pub async fn fetch(
    config: &Config,
    url: &str,
    method: &str,
    headers: &[String],
    body_data: Option<&str>,
    _cookie_session: Option<&str>,
    timeout: u64,
    json_output: bool,
) -> Result<(), String> {
    // Build fetch JS to run inside the browser context
    let mut header_obj = json!({});
    for h in headers {
        if let Some((k, v)) = h.split_once(':') {
            header_obj[k.trim()] = json!(v.trim());
        }
    }

    let fetch_opts = json!({
        "method": method,
        "headers": header_obj,
        "body": if method == "GET" || method == "HEAD" { serde_json::Value::Null } else { json!(body_data) },
        "credentials": "include",
    });

    // Use /function endpoint to run fetch inside Chrome
    let js_code = format!(
        r#"
        export default async function({{ page }}) {{
            const resp = await page.evaluate(async () => {{
                const r = await fetch({url_json}, {opts_json});
                const text = await r.text();
                return {{
                    status: r.status,
                    statusText: r.statusText,
                    headers: Object.fromEntries(r.headers.entries()),
                    body: text,
                }};
            }});
            return resp;
        }}
        "#,
        url_json = serde_json::to_string(url).unwrap(),
        opts_json = serde_json::to_string(&fetch_opts).unwrap(),
    );

    let req_body = json!({
        "code": js_code,
        "context": {}
    });

    // Try /function first, fall back to /content for simple GETs
    if method == "GET" && headers.is_empty() && body_data.is_none() {
        // Simple case: just use /content
        let content_body = json!({
            "url": url,
        });
        let text = config.post_text("/content", &content_body, timeout).await?;
        if json_output {
            println!("{}", json!({
                "url": url,
                "status": 200,
                "body": text,
            }));
        } else {
            println!("{text}");
        }
        return Ok(());
    }

    // Complex case: use /function
    let result = config.post_text("/function", &req_body, timeout).await?;
    if json_output {
        // Try to parse as JSON
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
            println!("{}", serde_json::to_string_pretty(&parsed).unwrap_or(result));
        } else {
            println!("{}", json!({"raw": result}));
        }
    } else {
        // Extract body from response if JSON
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
            if let Some(body) = parsed.get("body").and_then(|b| b.as_str()) {
                println!("{body}");
            } else {
                println!("{}", serde_json::to_string_pretty(&parsed).unwrap_or(result));
            }
        } else {
            println!("{result}");
        }
    }
    Ok(())
}

/// Simple proxy: GET URL through browserless
pub async fn proxy(config: &Config, url: &str, timeout: u64, json_output: bool) -> Result<(), String> {
    fetch(config, url, "GET", &[], None, None, timeout, json_output).await
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
