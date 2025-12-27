//! OAuth command integration tests (Logic Verification)

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fireworks_collaboration_lib::app::commands::oauth::{
    parse_oauth_callback, start_oauth_server_core,
};
use fireworks_collaboration_lib::app::types::OAuthState;

#[test]
fn test_parse_oauth_callback() {
    let req =
        "GET /auth/callback?code=123&state=abc&error=bad&error_description=bad%20desc HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code.as_deref(), Some("123"));
    assert_eq!(data.state.as_deref(), Some("abc"));
    assert_eq!(data.error.as_deref(), Some("bad"));
    assert_eq!(data.error_description.as_deref(), Some("bad desc"));

    let req2 = "GET /auth/callback?code=456 HTTP/1.1";
    let data2 = parse_oauth_callback(req2);
    assert_eq!(data2.code.as_deref(), Some("456"));
    assert!(data2.state.is_none());
}

#[tokio::test]
async fn test_start_oauth_server_logic() {
    // 1. Setup State
    let state: OAuthState = Arc::new(Mutex::new(None));

    // 2. Start Server
    let port_res = start_oauth_server_core(state.clone()).await;
    assert!(port_res.is_ok());
    let port = port_res.unwrap();
    assert!(port > 0);

    // 3. Send Request
    let addr = format!("127.0.0.1:{}", port);

    tokio::task::spawn_blocking(move || {
        // Wait a bit for server to accept
        std::thread::sleep(Duration::from_millis(100));

        if let Ok(mut stream) = TcpStream::connect(addr.clone()) {
            let req = b"GET /auth/callback?code=789 HTTP/1.1\r\n\r\n";
            stream.write_all(req).unwrap();

            let mut resp = String::new();
            stream.read_to_string(&mut resp).unwrap();

            assert!(resp.contains("200 OK"));
        } else {
            panic!("Failed to connect to {}", addr);
        }
    })
    .await
    .unwrap();

    // 4. Verify State
    // Wait for thread to process
    tokio::time::sleep(Duration::from_millis(200)).await;

    let lock = state.lock().unwrap();
    assert!(lock.is_some());
    let data = lock.as_ref().unwrap();
    assert_eq!(data.code.as_deref(), Some("789"));
}
