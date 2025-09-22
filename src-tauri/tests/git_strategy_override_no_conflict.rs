use std::fs; use fireworks_collaboration_lib::events::emitter::peek_captured_events;
use fireworks_collaboration_lib::core::tasks::registry::test_emit_clone_with_override;

#[test]
fn no_conflict_http_tls_override() {
  std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
  // create a local repo (only needed to mimic realistic path though helper does not access filesystem)
  let tmp_src = tempfile::tempdir().unwrap();
  let repo = git2::Repository::init(tmp_src.path()).unwrap();
  let fp = tmp_src.path().join("z.txt"); fs::write(&fp, "nc").unwrap();
  let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("z.txt")).unwrap(); index.write().unwrap();
  let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
  let override_json = serde_json::json!({
    "http": {"followRedirects": true, "maxRedirects": 5},
    "tls": {"insecureSkipVerify": true, "skipSanWhitelist": false}
  });
  // Ensure baseline config has insecure=false so override triggers change deterministically
  if let Ok(mut cfg) = fireworks_collaboration_lib::core::config::loader::load_or_init() {
      if cfg.tls.insecure_skip_verify { cfg.tls.insecure_skip_verify = false; let _ = fireworks_collaboration_lib::core::config::loader::save(&cfg); }
  }
  let id = uuid::Uuid::new_v4();
  test_emit_clone_with_override(tmp_src.path().to_string_lossy().as_ref(), id, override_json);
  let ev = peek_captured_events();
  let mut conflict_found=false; let mut http_applied=0; let mut tls_applied=0;
  for (topic,p) in ev { if topic=="task://error" && p.contains(&id.to_string()) { if p.contains("\"code\":\"strategy_override_conflict\"") { conflict_found=true; } if p.contains("\"code\":\"http_strategy_override_applied\"") { http_applied+=1; } if p.contains("\"code\":\"tls_strategy_override_applied\"") { tls_applied+=1; } } }
  assert_eq!(http_applied,0, "http values unchanged -> no http applied event");
  assert_eq!(tls_applied,1, "tls insecure changed -> one tls applied event");
  assert!(!conflict_found, "no conflict expected for normalized inputs");
}
