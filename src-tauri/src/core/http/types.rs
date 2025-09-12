use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingInfo { pub connect_ms: u32, pub tls_ms: u32, pub first_byte_ms: u32, pub total_ms: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectInfo { pub status: u16, pub location: String, pub count: u8 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestInput {
  pub url: String,
  pub method: String,
  pub headers: HashMap<String, String>,
  pub body_base64: Option<String>,
  pub timeout_ms: u64,
  pub force_real_sni: bool,
  pub follow_redirects: bool,
  pub max_redirects: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseOutput {
  pub ok: bool,
  pub status: u16,
  pub headers: HashMap<String, String>,
  pub body_base64: String,
  pub used_fake_sni: bool,
  pub ip: Option<String>,
  pub timing: TimingInfo,
  pub redirects: Vec<RedirectInfo>,
  pub body_size: usize,
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_roundtrip_serde() {
    let out = HttpResponseOutput{
      ok: true,
      status: 200,
      headers: HashMap::from([("content-type".into(), "text/plain".into())]),
      body_base64: "SGVsbG8=".into(),
      used_fake_sni: false,
      ip: Some("1.2.3.4".into()),
      timing: TimingInfo{ connect_ms:1, tls_ms:2, first_byte_ms:3, total_ms:4 },
      redirects: vec![RedirectInfo{ status: 301, location: "https://example.com".into(), count:1}],
      body_size: 5,
    };
    let s = serde_json::to_string(&out).unwrap();
    let back: HttpResponseOutput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.status, 200);
    assert_eq!(back.timing.total_ms, 4);
    assert_eq!(back.redirects.len(), 1);
  }
}
