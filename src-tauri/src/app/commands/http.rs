//! HTTP request commands and utilities.

use std::collections::HashMap;
use tauri::State;

use crate::core::http::{
    client::HttpClient,
    types::{HttpRequestInput, HttpResponseOutput, RedirectInfo},
};

use super::super::types::SharedConfig;

/// Redact authorization headers for logging purposes.
pub fn redact_auth_in_headers(
    mut headers: HashMap<String, String>,
    mask: bool,
) -> HashMap<String, String> {
    if !mask {
        return headers;
    }

    // Case-insensitive matching for sensitive headers
    for (k, _v) in headers.clone().iter() {
        if k.eq_ignore_ascii_case("authorization")
            || k.eq_ignore_ascii_case("cookie")
            || k.eq_ignore_ascii_case("proxy-authorization")
        {
            headers.insert(k.clone(), "REDACTED".into());
        }
    }

    headers
}

/// Classify error messages into categories.
///
/// Returns a tuple of (category, message) for better error handling.
pub fn classify_error_msg(e: &str) -> (&'static str, String) {
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
pub fn validate_url(url: &str) -> Result<(hyper::Uri, String), String> {
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
pub fn process_redirect(
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
pub fn update_request_for_redirect(input: &mut HttpRequestInput, status: u16, next_url: String) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_redact_auth_in_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "secret".to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("authorization".to_string(), "another-secret".to_string());

        // Masking ON
        let redacted = redact_auth_in_headers(headers.clone(), true);
        assert_eq!(redacted.get("Authorization").unwrap(), "REDACTED");
        assert_eq!(redacted.get("authorization").unwrap(), "REDACTED");
        assert_eq!(redacted.get("Content-Type").unwrap(), "application/json");

        // Masking OFF
        let normal = redact_auth_in_headers(headers, false);
        assert_eq!(normal.get("Authorization").unwrap(), "secret");
        assert_eq!(normal.get("authorization").unwrap(), "another-secret");
    }

    #[test]
    fn test_classify_error_msg() {
        assert_eq!(
            classify_error_msg("SAN whitelist mismatch"),
            ("Verify", "SAN whitelist mismatch".to_string())
        );
        assert_eq!(
            classify_error_msg("tls handshake error"),
            ("Tls", "tls handshake error".to_string())
        );
        assert_eq!(
            classify_error_msg("connect timeout"),
            ("Network", "connect timeout".to_string())
        );
        assert_eq!(
            classify_error_msg("only https is supported"),
            ("Input", "only https is supported".to_string())
        );
        assert_eq!(
            classify_error_msg("random unexpected error"),
            ("Internal", "random unexpected error".to_string())
        );
    }

    #[test]
    fn test_validate_url() {
        // Valid HTTPS
        let (uri, host) = validate_url("https://github.com/test").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(uri.to_string(), "https://github.com/test");

        // Invalid: only HTTP
        let res_http = validate_url("http://example.com");
        assert!(res_http.is_err());
        assert!(res_http.unwrap_err().contains("only https"));

        // Invalid URL
        let res_invalid = validate_url("not a url");
        assert!(res_invalid.is_err());
        assert!(res_invalid.unwrap_err().contains("invalid URL"));

        // Missing host (rejected by hyper parse or our check)
        let res_no_host = validate_url("https:///path");
        assert!(res_no_host.is_err());
        let err = res_no_host.unwrap_err();
        assert!(err.contains("invalid URL") || err.contains("host missing"));
    }

    #[test]
    fn test_process_redirect() {
        let mut headers = HashMap::new();
        headers.insert("location".to_string(), "/new-path".to_string());
        let current_url = "https://example.com/old-path";

        let next = process_redirect(&headers, current_url).unwrap();
        assert_eq!(next, "https://example.com/new-path");

        // Absolute location
        let mut headers_abs = HashMap::new();
        headers_abs.insert(
            "location".to_string(),
            "https://other.com/target".to_string(),
        );
        let next_abs = process_redirect(&headers_abs, current_url).unwrap();
        assert_eq!(next_abs, "https://other.com/target");

        // Missing location
        let res_missing = process_redirect(&HashMap::new(), current_url);
        assert!(res_missing.is_err());
        assert!(res_missing.unwrap_err().contains("without Location header"));
    }

    #[test]
    fn test_update_request_for_redirect() {
        let mut input = HttpRequestInput {
            url: "https://old.com".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body_base64: Some("data".to_string()),
            timeout_ms: 5000,
            force_real_sni: false,
            follow_redirects: true,
            max_redirects: 5,
        };

        // 302 should change to GET and clear body
        update_request_for_redirect(&mut input, 302, "https://new.com".to_string());
        assert_eq!(input.url, "https://new.com");
        assert_eq!(input.method, "GET");
        assert!(input.body_base64.is_none());

        // 307 should keep method and body
        let mut input_307 = HttpRequestInput {
            url: "https://old.com".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body_base64: Some("data".to_string()),
            ..input
        };
        update_request_for_redirect(&mut input_307, 307, "https://new.com".to_string());
        assert_eq!(input_307.method, "POST");
        assert_eq!(input_307.body_base64.as_deref(), Some("data"));
    }
}
