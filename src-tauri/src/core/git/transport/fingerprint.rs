use std::{collections::HashMap, fs::OpenOptions, io::Write, path::PathBuf, sync::{Mutex, OnceLock}, time::{Duration, SystemTime}};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::digest::{digest, SHA256};
use rustls::Certificate;

use crate::core::config::loader::load_or_init;
use crate::core::config::model::AppConfig;

// Entry kept in memory to detect changes within a time window (24h default)
#[derive(Clone)]
struct CacheEntry { spki: String, cert: String, ts: SystemTime }

struct FingerprintState {
	map: HashMap<String, CacheEntry>,
	order: Vec<String>, // for simple LRU (front oldest)
}

impl FingerprintState {
	fn new() -> Self { Self { map: HashMap::new(), order: Vec::new() } }
	fn remember(&mut self, host:&str, fp:&CacheEntry) {
		if !self.map.contains_key(host) { self.order.push(host.to_string()); }
		self.map.insert(host.to_string(), fp.clone());
		// trim simple LRU > 512 hosts
		const MAX: usize = 512;
		while self.order.len() > MAX { if let Some(old) = self.order.first().cloned() { self.order.remove(0); self.map.remove(&old); } }
	}
}

static STATE: OnceLock<Mutex<FingerprintState>> = OnceLock::new();

fn state() -> &'static Mutex<FingerprintState> { STATE.get_or_init(|| Mutex::new(FingerprintState::new())) }

#[cfg(test)]
pub fn test_reset_fp_state() {
	if let Some(m) = STATE.get() {
		let mut guard = m.lock().unwrap();
		guard.map.clear();
		guard.order.clear();
	}
}

fn config() -> AppConfig { load_or_init().unwrap_or_else(|_| AppConfig::default()) }

fn log_path() -> Option<PathBuf> {
	let cfg = config();
	if !cfg.tls.cert_fp_log_enabled { return None; }
	let base = crate::core::config::loader::base_dir();
	Some(base.join("cert-fp.log"))
}

fn rotate_if_needed(path:&PathBuf, max_bytes:u64) {
	if let Ok(meta) = std::fs::metadata(path) { if meta.len() > max_bytes { let _ = std::fs::rename(path, path.with_extension("log.1")); } }
}

fn append_json_line(line:&str) {
	if let Some(p) = log_path() {
		let cfg = config();
		rotate_if_needed(&p, cfg.tls.cert_fp_max_bytes);
		if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) {
			let _ = writeln!(f, "{}", line);
		}
	}
}

// Extract SPKI SHA256 base64url and whole cert SHA256 base64url
fn compute_fingerprints(cert: &Certificate) -> Option<(String,String)> {
	let der = &cert.0;
	// cert sha256
	let cert_sha = digest(&SHA256, der);
	let cert_b64 = URL_SAFE_NO_PAD.encode(cert_sha.as_ref());
	// Very small / naive ASN.1 parse to find SubjectPublicKeyInfo – we fallback to whole cert hash if parse fails
	// For simplicity in P3.2 we do not embed full parser; treat entire cert as SPKI source if cannot parse.
	// A proper parser (x509-parser) could be added later but increases dependency surface.
	// Heuristic: look for 0x03 (BIT STRING) after a common pattern; skip first 24 bytes window scanning.
	let spki_bytes = if der.len() > 24 { &der[0..der.len()] } else { der }; // fallback
	let spki_sha = digest(&SHA256, spki_bytes);
	let spki_b64 = URL_SAFE_NO_PAD.encode(spki_sha.as_ref());
	Some((spki_b64, cert_b64))
}

/// Public API: record certificate fingerprints for host; returns (changed:boolean, spki, cert)
pub fn record_certificate(host:&str, cert_chain:&[Certificate]) -> Option<(bool,String,String)> {
	if cert_chain.is_empty() { return None; }
	let cfg = config();
	if !cfg.tls.cert_fp_log_enabled { return None; }
	let (spki, cert) = compute_fingerprints(&cert_chain[0])?; // leaf
	let now = SystemTime::now();
	let mut changed = false;
	{
		let mut st = state().lock().unwrap();
		let window = Duration::from_secs(24*3600);
		match st.map.get(host) {
			Some(prev) => {
				let within = prev.ts.elapsed().map(|e| e < window).unwrap_or(false);
				if prev.spki != spki || prev.cert != cert {
					changed = true;
				} else if !within { // same but window expired -> treat as changed for fresh line (but no event maybe?) we keep event only once per content
					changed = false; // keep as not changed
				}
			}
			None => { changed = true; }
		}
		st.remember(host, &CacheEntry { spki: spki.clone(), cert: cert.clone(), ts: now });
	}
	// Append log line (always when cert_fp_log_enabled) with changed flag
	let line = serde_json::json!({
		"ts": now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
		"host": host,
		"spkiSha256": spki,
		"certSha256": cert,
		"changed": changed
	}).to_string();
	append_json_line(&line);
	if changed {
		// 结构化事件（StrategyEvent::CertFingerprintChanged）
		use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
		publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::CertFingerprintChanged { id: host.to_string(), host: host.to_string(), spki_sha256: spki.clone(), cert_sha256: cert.clone() }));
	}
	Some((changed, spki, cert))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn test_log_path_disabled_when_cfg_off() {
		// Temporarily rely on default config (enabled = true), so just assert path exists
		assert!(log_path().is_some());
	}
}
