use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::http::client::HttpClient;
use fireworks_collaboration_lib::core::http::types::HttpRequestInput;
use std::collections::HashMap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_http_client_simple_get_success() {
    // 1. Start WireMock server
    let mock_server = MockServer::start().await;

    // 2. Setup mock expectation
    Mock::given(method("GET"))
        .and(path("/hello"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello World"))
        .mount(&mock_server)
        .await;

    // 3. Setup client
    let config = AppConfig::default();
    let client = HttpClient::new(config);

    // 4. Create request input
    let url = format!("{}/hello", mock_server.uri());
    let input = HttpRequestInput {
        url,
        method: "GET".to_string(),
        headers: HashMap::new(),
        body_base64: None,
        timeout_ms: 1000,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };

    // 5. Execute
    let response = client.send(input).await.expect("request failed");

    // 6. Assert
    assert!(response.ok);
    assert_eq!(response.status, 200);

    // Decode base64 body (HttpClient returns base64 encoded body)
    let body_bytes = BASE64.decode(&response.body_base64).expect("decode base64");
    let body_str = String::from_utf8(body_bytes).expect("utf8");
    assert_eq!(body_str, "Hello World");
}

#[tokio::test]
async fn test_http_client_404_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let config = AppConfig::default();
    let client = HttpClient::new(config);

    let url = format!("{}/missing", mock_server.uri());
    let input = HttpRequestInput {
        url,
        method: "GET".to_string(),
        headers: HashMap::new(),
        body_base64: None,
        timeout_ms: 1000,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };

    let response = client.send(input).await.expect("request failed");

    assert!(!response.ok);
    assert_eq!(response.status, 404);
}

#[tokio::test]
async fn test_http_client_post_with_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&mock_server)
        .await;

    let config = AppConfig::default();
    let client = HttpClient::new(config);

    let url = format!("{}/data", mock_server.uri());
    let body_content = "some data";
    let body_b64 = BASE64.encode(body_content);

    let input = HttpRequestInput {
        url,
        method: "POST".to_string(),
        headers: HashMap::new(),
        body_base64: Some(body_b64),
        timeout_ms: 1000,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };

    let response = client.send(input).await.expect("request failed");

    assert!(response.ok);
    assert_eq!(response.status, 201);
}
