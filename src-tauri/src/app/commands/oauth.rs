//! OAuth server and related commands.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};
use tauri::State;

use super::super::types::{OAuthCallbackData, OAuthState};

/// Start the OAuth callback server on port 3429.
///
/// The server listens for incoming OAuth callback requests
/// and stores the authorization code or error in the shared state.
#[tauri::command]
pub async fn start_oauth_server(state: State<'_, OAuthState>) -> Result<String, String> {
    let oauth_state = Arc::clone(&*state);

    thread::spawn(move || match TcpListener::bind("127.0.0.1:3429") {
        Ok(listener) => {
            tracing::info!(target = "oauth", "OAuth server started on 127.0.0.1:3429");

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
        }
        Err(e) => {
            tracing::error!(target = "oauth", error = %e, "Failed to bind OAuth server");
        }
    });

    Ok("OAuth server started".into())
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
#[tauri::command]
pub async fn get_oauth_callback_data(
    state: State<'_, OAuthState>,
) -> Result<Option<OAuthCallbackData>, String> {
    state
        .lock()
        .map(|mut s| s.take())
        .map_err(|e| format!("Failed to acquire OAuth state lock: {}", e))
}

/// Clear the OAuth callback state without returning data.
#[tauri::command]
pub async fn clear_oauth_state(state: State<'_, OAuthState>) -> Result<(), String> {
    state
        .lock()
        .map(|mut s| {
            *s = None;
        })
        .map_err(|e| format!("Failed to acquire OAuth state lock: {}", e))
}
