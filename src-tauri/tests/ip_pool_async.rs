use fireworks_collaboration_lib::core::ip_pool::global::pick_best_async;

#[tokio::test]
async fn pick_best_async_does_not_panic_and_returns_selection() {
    let host = "github.com";
    let port = 443u16;
    let sel = pick_best_async(host, port).await;
    assert_eq!(sel.host(), host, "host should match");
    assert_eq!(sel.port(), port, "port should match");
    // Strategy may be SystemDefault or Cached depending on cache state; just ensure it's one of them
    assert!(sel.is_system_default() || sel.selected().is_some());
}
