//! HTTP request commands and utilities.

use std::collections::HashMap;
use tauri::State;

use crate::core::http::{
    client::HttpClient,
    types::{HttpRequestInput, HttpResponseOutput, RedirectInfo},
};

use super::super::types::SharedConfig;

/// Redact authorization headers for logging purposes.
pub(crate) fn redact_auth_in_headers(
    mut headers: HashMap<String, String>,
    mask: bool,
) -> HashMap<String, String> {
    if !mask {
        return headers;
    }

    // Case-insensitive matching for Authorization header
    for (k, _v) in headers.clone().iter() {
        if k.eq_ignore_ascii_case("authorization") {
            headers.insert(k.clone(), "REDACTED".into());
        }
    }

    headers
}

/// Classify error messages into categories.
///
/// Returns a tuple of (category, message) for better error handling.
pub(crate) fn classify_error_msg(e: &str) -> (&'static str, String) {
    let msg = e.to_string();

    if msg.contains("SAN whitelist mismatch") {
        ("Verify", msg)
    } else if msg.contains("tls handshake") {
        ("Tls", msg)
    } else if msg.contains("connect timeout")
        || msg.contains("connect error")
        || msg.contains("read body")
    {
        ("Network", msg)
    } else if msg.contains("only https")
        || msg.contains("invalid URL")
        || msg.contains("url host missing")
    {
        ("Input", msg)
    } else {
        ("Internal", msg)
    }
}

/// Validate and parse URL for HTTP requests.
fn validate_url(url: &str) -> Result<(hyper::Uri, String), String> {
    let parsed = url
        .parse::<hyper::Uri>()
        .map_err(|e| format!("Input: invalid URL - {}", e))?;

    if parsed.scheme_str() != Some("https") {
        return Err("Input: only https is supported".into());
    }

    let host = parsed
        .host()
        .ok_or_else(|| "Input: url host missing".to_string())?;

    Ok((parsed.clone(), host.to_string()))
}

/// Process redirect response.
fn process_redirect(
    headers: &HashMap<String, String>,
    current_url: &str,
) -> Result<String, String> {
    let location = headers
        .get("location")
        .ok_or_else(|| "Network: redirect without Location header".to_string())?;

    // Parse and construct next URL (resolve relative paths)
    let base = current_url
        .parse::<url::Url>()
        .map_err(|e| format!("Internal: url parse - {}", e))?;

    let next_url = base
        .join(location)
        .map_err(|e| format!("Input: bad redirect location - {}", e))?
        .to_string();

    Ok(next_url)
}

/// Update request for redirect based on status code.
fn update_request_for_redirect(input: &mut HttpRequestInput, status: u16, next_url: String) {
    input.url = next_url;

    match status {
        // 301, 302, 303: Change to GET, remove body
        301 | 302 | 303 => {
            input.method = "GET".into();
            input.body_base64 = None;
        }
        // 307, 308: Keep method and body
        307 | 308 => {
            // No changes needed
        }
        _ => {}
    }
}

/// Make an HTTP request with optional redirect following.
///
/// This command sends an HTTPS request and can optionally follow redirects.
/// All requests are subject to SAN whitelist validation.
///
/// # Parameters
/// - `input`: Request configuration including URL, method, headers, body, and redirect settings
///
/// # Returns
/// Response data including status, headers, body, and redirect chain (if any)
#[tauri::command(rename_all = "camelCase")]
pub async fn http_fake_request(
    input: HttpRequestInput,
    cfg: State<'_, SharedConfig>,
) -> Result<HttpResponseOutput, String> {
    // Clone config to avoid holding lock across await
    let cfg_val = {
        let g = cfg.lock().map_err(|e| e.to_string())?;
        g.clone()
    };

    // Early validation of URL and host whitelist
    let mut current_url = input.url.clone();
    validate_url(&current_url)?;

    // Log request with redacted headers
    let redacted =
        redact_auth_in_headers(input.headers.clone(), cfg_val.logging.auth_header_masked);
    tracing::info!(
        target = "http",
        method = %input.method,
        url = %input.url,
        headers = ?redacted,
        "HTTP request started"
    );

    // Create HTTP client
    let client = HttpClient::new(cfg_val.clone());
    let follow = input.follow_redirects;
    let max_redirects = input.max_redirects;
    let mut redirects: Vec<RedirectInfo> = Vec::new();
    let mut attempt_input = input.clone();

    // Handle redirects in a loop
    for i in 0..=max_redirects as u16 {
        match client.send(attempt_input.clone()).await {
            Ok(mut out) => {
                let status = out.status;
                let is_redirect = matches!(status, 301 | 302 | 303 | 307 | 308);

                // If not a redirect or not following redirects, return
                if !is_redirect || !follow {
                    let redirect_count = redirects.len();
                    out.redirects = redirects;
                    tracing::info!(
                        target = "http",
                        status = status,
                        redirects = redirect_count,
                        "HTTP request completed"
                    );
                    return Ok(out);
                }

                // Check if we've reached the redirect limit
                if i as u8 >= max_redirects {
                    return Err(format!("Network: too many redirects (>{})", max_redirects));
                }

                // Process redirect
                let next_url = process_redirect(&out.headers, &current_url)?;

                tracing::debug!(
                    target = "http",
                    status = status,
                    from = %current_url,
                    to = %next_url,
                    "Following redirect"
                );

                redirects.push(RedirectInfo {
                    status,
                    location: next_url.clone(),
                    count: (i as u8) + 1,
                });

                // Update request for next iteration
                update_request_for_redirect(&mut attempt_input, status, next_url.clone());
                current_url = next_url;
            }
            Err(e) => {
                let (cat, msg) = classify_error_msg(&e.to_string());
                tracing::error!(
                    target = "http",
                    category = cat,
                    error = %msg,
                    "HTTP request failed"
                );
                return Err(format!("{}: {}", cat, msg));
            }
        }
    }

    Err("Network: redirect loop reached without resolution".into())
}
