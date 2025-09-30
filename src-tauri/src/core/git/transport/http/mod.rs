// Split from previous single-file http.rs into smaller modules without changing behavior.
// Public surface from transport remains:
// - struct CustomHttpsSubtransport (used by register.rs)
// - fn set_push_auth_header_value (re-exported to transport::)

mod auth;
mod fallback;
mod stream;
mod subtransport;
mod util;

pub use auth::set_push_auth_header_value;
pub(super) use subtransport::CustomHttpsSubtransport;

/// HTTP 操作类型（smart 协议的四种阶段），仅限本模块及子模块使用。
pub(super) enum HttpOp {
    // GET /info/refs?service=git-upload-pack
    InfoRefsUpload,
    // POST /git-upload-pack
    UploadPack,
    // GET /info/refs?service=git-receive-pack
    InfoRefsReceive,
    // POST /git-receive-pack
    ReceivePack,
}

pub(super) enum TransferKind {
    Chunked,
    Length,
    Eof,
}

#[cfg(not(feature = "tauri-app"))]
pub mod testing {
    //! Aggregates HTTP transport testing helpers for integration tests.
    pub use super::fallback::testing::{
        classify_and_count_fallback, inject_fake_failure, inject_real_failure,
        reset_fallback_counters, reset_injected_failures, snapshot_fallback_counters,
    };
    pub use super::subtransport::testing::TestSubtransport;
}
