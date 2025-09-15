// Split transport module without changing public API or logic.
// Public API remains:
// - ensure_registered
// - maybe_rewrite_https_to_custom
// - set_push_auth_header_value

#[path = "http/mod.rs"]
mod http;
mod register;
mod rewrite;

pub use http::set_push_auth_header_value;
pub use register::ensure_registered;
pub use rewrite::maybe_rewrite_https_to_custom;
