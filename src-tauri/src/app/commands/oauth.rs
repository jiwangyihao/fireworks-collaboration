//! OAuth server and related commands.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};
use tauri::State;

use super::super::types::{OAuthCallbackData, OAuthState};

/// Start the OAuth callback server on a dynamically allocated port.
///
/// The server listens for incoming OAuth callback requests
/// and stores the authorization code or error in the shared state.
/// Returns the actual port number that was allocated.
#[tauri::command(rename_all = "camelCase")]
pub async fn start_oauth_server(state: State<'_, OAuthState>) -> Result<u16, String> {
    let oauth_state = Arc::clone(&*state);

    // Bind to port 0 to let the OS allocate an available port
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind OAuth server: {}", e))?;
    
    let port = listener.local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?
        .port();
    
    tracing::info!(target = "oauth", port = port, "OAuth server started on dynamic port");

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let st = Arc::clone(&oauth_state);
                    thread::spawn(move || handle_oauth_request(s, st));
                }
                Err(e) => {
                    tracing::error!(target = "oauth", error = %e, "Failed to accept connection");
                }
            }
        }
    });

    Ok(port)
}

/// Handle an incoming OAuth callback request.
fn handle_oauth_request(mut stream: TcpStream, oauth_state: OAuthState) {
    let mut buf = [0u8; 4096];

    match stream.read(&mut buf) {
        Ok(n) => {
            let req = String::from_utf8_lossy(&buf[..n]);

            if req.starts_with("GET /auth/callback") {
                let data = parse_oauth_callback(&req);

                // Store callback data
                if let Ok(mut s) = oauth_state.lock() {
                    tracing::info!(
                        target = "oauth",
                        code_present = data.code.is_some(),
                        error_present = data.error.is_some(),
                        "OAuth callback received"
                    );
                    *s = Some(data);
                } else {
                    tracing::error!(
                        target = "oauth",
                        "Failed to acquire lock for storing OAuth data"
                    );
                }

                // Send success response
                let body = "<html><body><h1>Authorization Complete</h1><p>You can close this window now.</p></body></html>";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            } else {
                // Not found
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        }
        Err(e) => {
            tracing::error!(target = "oauth", error = %e, "Failed to read from stream");
        }
    }
}

/// Parse OAuth callback parameters from the request.
fn parse_oauth_callback(req: &str) -> OAuthCallbackData {
    let mut data = OAuthCallbackData {
        code: None,
        state: None,
        error: None,
        error_description: None,
    };

    if let Some(q_pos) = req.find('?') {
        let tail = &req[q_pos + 1..];
        let end = tail.find(' ').unwrap_or(tail.len());

        for kv in tail[..end].split('&') {
            if let Some(eq) = kv.find('=') {
                let (k, v) = (&kv[..eq], &kv[eq + 1..]);
                let v = urlencoding::decode(v).unwrap_or_default().to_string();

                match k {
                    "code" => data.code = Some(v),
                    "state" => data.state = Some(v),
                    "error" => data.error = Some(v),
                    "error_description" => data.error_description = Some(v),
                    _ => {}
                }
            }
        }
    }

    data
}

/// Get and clear the OAuth callback data.
///
/// This returns the stored callback data (if any) and clears it from state.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_oauth_callback_data(
    state: State<'_, OAuthState>,
) -> Result<Option<OAuthCallbackData>, String> {
    state
        .lock()
        .map(|mut s| s.take())
        .map_err(|e| format!("Failed to acquire OAuth state lock: {}", e))
}

/// Clear the OAuth callback state without returning data.
#[tauri::command(rename_all = "camelCase")]
pub async fn clear_oauth_state(state: State<'_, OAuthState>) -> Result<(), String> {
    state
        .lock()
        .map(|mut s| {
            *s = None;
        })
        .map_err(|e| format!("Failed to acquire OAuth state lock: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_oauth_callback_tests {
        use super::*;

        #[test]
        fn parses_code_parameter() {
            let req = "GET /auth/callback?code=abc123 HTTP/1.1\r\nHost: localhost";
            let data = parse_oauth_callback(req);

            assert_eq!(data.code, Some("abc123".to_string()));
            assert!(data.state.is_none());
            assert!(data.error.is_none());
        }

        #[test]
        fn parses_code_and_state() {
            let req = "GET /auth/callback?code=abc123&state=xyz789 HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert_eq!(data.code, Some("abc123".to_string()));
            assert_eq!(data.state, Some("xyz789".to_string()));
        }

        #[test]
        fn parses_error_parameters() {
            let req = "GET /auth/callback?error=access_denied&error_description=User%20denied%20access HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert_eq!(data.error, Some("access_denied".to_string()));
            assert_eq!(data.error_description, Some("User denied access".to_string()));
            assert!(data.code.is_none());
        }

        #[test]
        fn handles_url_encoded_values() {
            let req = "GET /auth/callback?code=abc%2B123%3D%3D&state=test%20state HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert_eq!(data.code, Some("abc+123==".to_string()));
            assert_eq!(data.state, Some("test state".to_string()));
        }

        #[test]
        fn handles_no_query_string() {
            let req = "GET /auth/callback HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert!(data.code.is_none());
            assert!(data.state.is_none());
            assert!(data.error.is_none());
            assert!(data.error_description.is_none());
        }

        #[test]
        fn handles_empty_query_string() {
            let req = "GET /auth/callback? HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert!(data.code.is_none());
        }

        #[test]
        fn ignores_unknown_parameters() {
            let req = "GET /auth/callback?code=abc&unknown=value&other=123 HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert_eq!(data.code, Some("abc".to_string()));
        }

        #[test]
        fn handles_malformed_key_value() {
            // Missing '=' should be skipped
            let req = "GET /auth/callback?code=abc&malformed&state=xyz HTTP/1.1";
            let data = parse_oauth_callback(req);

            assert_eq!(data.code, Some("abc".to_string()));
            assert_eq!(data.state, Some("xyz".to_string()));
        }
    }
}
