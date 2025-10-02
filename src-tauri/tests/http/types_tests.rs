use fireworks_collaboration_lib::core::http::types::{
    HttpResponseOutput, RedirectInfo, TimingInfo,
};
use std::collections::HashMap;

#[test]
fn test_roundtrip_serde() {
    let out = HttpResponseOutput {
        ok: true,
        status: 200,
        headers: HashMap::from([("content-type".into(), "text/plain".into())]),
        body_base64: "SGVsbG8=".into(),
        used_fake_sni: false,
        ip: Some("1.2.3.4".into()),
        timing: TimingInfo {
            connect_ms: 1,
            tls_ms: 2,
            first_byte_ms: 3,
            total_ms: 4,
        },
        redirects: vec![RedirectInfo {
            status: 301,
            location: "https://example.com".into(),
            count: 1,
        }],
        body_size: 5,
    };
    let s = serde_json::to_string(&out).unwrap();
    let back: HttpResponseOutput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.status, 200);
    assert_eq!(back.timing.total_ms, 4);
    assert_eq!(back.redirects.len(), 1);
}
