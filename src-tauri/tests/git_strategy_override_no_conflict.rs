use std::fs;
use fireworks_collaboration_lib::core::tasks::registry::test_emit_clone_with_override;
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_test_event_bus, clear_test_event_bus, Event as StructuredEvent, StrategyEvent};
use std::sync::Arc;

#[test]
fn no_conflict_http_tls_override() {
  std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
  // create a local repo (only needed to mimic realistic path though helper does not access filesystem)
  let tmp_src = tempfile::tempdir().unwrap();
  let repo = git2::Repository::init(tmp_src.path()).unwrap();
  let fp = tmp_src.path().join("z.txt"); fs::write(&fp, "nc").unwrap();
  let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("z.txt")).unwrap(); index.write().unwrap();
  let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
  // 动态读取当前全局配置，构造与现值相反的 insecure_skip_verify 以确保产生变更 (http 不变, tls 变)
  let base_cfg = fireworks_collaboration_lib::core::config::loader::load_or_init().expect("load base cfg");
  let flip_insecure = !base_cfg.tls.insecure_skip_verify;
  let override_json = serde_json::json!({
    "http": {"follow_redirects": base_cfg.http.follow_redirects, "max_redirects": base_cfg.http.max_redirects},
    "tls": {"insecure_skip_verify": flip_insecure, "skip_san_whitelist": base_cfg.tls.skip_san_whitelist}
  });
  let id = uuid::Uuid::new_v4();
  // 安装独立结构化事件总线以捕获策略事件
  let s_bus = MemoryEventBus::new();
  set_test_event_bus(Arc::new(s_bus.clone()));
  test_emit_clone_with_override(tmp_src.path().to_string_lossy().as_ref(), id, override_json);
  // 仅使用结构化事件：应存在一个 TlsApplied 且无 HttpApplied
  let structured = s_bus.snapshot();
  let mut s_http=0; let mut s_tls=0; let mut s_summary=0; let mut conflicts=0;
  for e in &structured {
    match e {
      StructuredEvent::Strategy(StrategyEvent::HttpApplied { id: sid, .. }) if sid==&id.to_string() => s_http+=1,
      StructuredEvent::Strategy(StrategyEvent::TlsApplied { id: sid, .. }) if sid==&id.to_string() => s_tls+=1,
      StructuredEvent::Strategy(StrategyEvent::Summary { id: sid, .. }) if sid==&id.to_string() => s_summary+=1,
      StructuredEvent::Strategy(StrategyEvent::Conflict { id: sid, .. }) if sid==&id.to_string() => conflicts+=1,
      _=>{}
    }
  }
  eprintln!("captured structured events: http={} tls={} summary={} conflicts={}", s_http,s_tls,s_summary,conflicts);
  assert_eq!(s_http, 0, "http unchanged -> no HttpApplied");
  assert_eq!(s_tls, 1, "tls flipped -> one TlsApplied");
  assert!(s_summary>=1, "expected summary");
  assert_eq!(conflicts, 0, "no conflict expected");
  clear_test_event_bus();
}
