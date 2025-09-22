use fireworks_collaboration_lib::events::structured::*;

// 基本契约快照：构造每种事件变体并序列化为 JSON，确保 schema 未被意外修改。
// 若未来字段新增/重命名，请同步更新本测试中的期望 JSON，作为显式契约变更记录。
#[test]
fn strategy_and_related_event_contract_snapshot() {
    let samples = vec![
        Event::Task(TaskEvent::Started { id: "id1".into(), kind: "GitClone".into() }),
        Event::Task(TaskEvent::Completed { id: "id1".into() }),
        Event::Task(TaskEvent::Failed { id: "id1".into(), category: "Protocol".into(), code: Some("x".into()), message: "m".into() }),
        Event::Policy(PolicyEvent::RetryApplied { id: "id2".into(), code: "retry_strategy_override_applied".into(), changed: vec!["max".into(),"factor".into()] }),
        Event::Transport(TransportEvent::PartialFilterCapability { id: "id3".into(), supported: true }),
        Event::Transport(TransportEvent::PartialFilterUnsupported { id: "id3".into(), requested: "blob: none".into() }),
        Event::Transport(TransportEvent::PartialFilterFallback { id: "id3".into(), shallow: true, message: "partial_filter_fallback".into() }),
        Event::Strategy(StrategyEvent::HttpApplied { id: "id4".into(), follow: true, max_redirects: 5 }),
        Event::Strategy(StrategyEvent::TlsApplied { id: "id4".into(), insecure_skip_verify: false, skip_san_whitelist: false }),
        Event::Strategy(StrategyEvent::Conflict { id: "id4".into(), kind: "http".into(), message: "followRedirects=false => force maxRedirects=0 (was 5)".into() }),
        Event::Strategy(StrategyEvent::Summary { id: "id5".into(), kind: "GitClone".into(), http_follow: true, http_max: 5, retry_max: 6, retry_base_ms: 300, retry_factor: 1.5, retry_jitter: true, tls_insecure: false, tls_skip_san: false, applied_codes: vec!["http_strategy_override_applied".into()], filter_requested: false }),
        Event::Strategy(StrategyEvent::AdaptiveTlsRollout { id: "id6".into(), kind: "GitClone".into(), percent_applied: 42, sampled: true }),
        Event::Strategy(StrategyEvent::IgnoredFields { id: "id7".into(), kind: "GitClone".into(), top_level: vec!["extraTop".into()], nested: vec!["http.AAA".into(),"tls.BBB".into()] }),
    ];
    let mut json_lines = vec![];
    for evt in samples {
        let s = serde_json::to_string(&evt).expect("serialize");
        json_lines.push(s);
    }
    // 稳定顺序字符串拼接（不带空格）
    let joined = json_lines.join("\n");
    let expected = r#"{"type":"Task","data":{"Started":{"id":"id1","kind":"GitClone"}}}
{"type":"Task","data":{"Completed":{"id":"id1"}}}
{"type":"Task","data":{"Failed":{"id":"id1","category":"Protocol","code":"x","message":"m"}}}
{"type":"Policy","data":{"RetryApplied":{"id":"id2","code":"retry_strategy_override_applied","changed":["max","factor"]}}}
{"type":"Transport","data":{"PartialFilterCapability":{"id":"id3","supported":true}}}
{"type":"Transport","data":{"PartialFilterUnsupported":{"id":"id3","requested":"blob: none"}}}
{"type":"Transport","data":{"PartialFilterFallback":{"id":"id3","shallow":true,"message":"partial_filter_fallback"}}}
{"type":"Strategy","data":{"HttpApplied":{"id":"id4","follow":true,"max_redirects":5}}}
{"type":"Strategy","data":{"TlsApplied":{"id":"id4","insecure_skip_verify":false,"skip_san_whitelist":false}}}
{"type":"Strategy","data":{"Conflict":{"id":"id4","kind":"http","message":"followRedirects=false => force maxRedirects=0 (was 5)"}}}
{"type":"Strategy","data":{"Summary":{"id":"id5","kind":"GitClone","http_follow":true,"http_max":5,"retry_max":6,"retry_base_ms":300,"retry_factor":1.5,"retry_jitter":true,"tls_insecure":false,"tls_skip_san":false,"applied_codes":["http_strategy_override_applied"],"filter_requested":false}}}
{"type":"Strategy","data":{"AdaptiveTlsRollout":{"id":"id6","kind":"GitClone","percent_applied":42,"sampled":true}}}
{"type":"Strategy","data":{"IgnoredFields":{"id":"id7","kind":"GitClone","top_level":["extraTop"],"nested":["http.AAA","tls.BBB"]}}}"#;
    assert_eq!(joined, expected, "structured event contract changed; update expected snapshot if intentional");
}
