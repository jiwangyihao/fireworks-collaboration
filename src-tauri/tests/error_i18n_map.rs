#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::git::default_impl::helpers::map_git2_error;
use fireworks_collaboration_lib::core::git::errors::ErrorCategory;

#[test]
fn chinese_connection_error_classified_as_network() {
    // Construct a git2::Error manually via custom message (using generic error code/class)
    let err = git2::Error::from_str("无法 连接 到 服务器: 超时");
    let cat = map_git2_error(&err);
    assert!(matches!(cat, ErrorCategory::Network), "expected Network got {:?}", cat);
}
