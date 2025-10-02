use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingInfo {
    pub connect_ms: u32,
    pub tls_ms: u32,
    pub first_byte_ms: u32,
    pub total_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectInfo {
    pub status: u16,
    pub location: String,
    pub count: u8,
}

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
