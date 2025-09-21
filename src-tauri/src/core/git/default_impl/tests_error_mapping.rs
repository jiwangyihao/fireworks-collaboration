#[cfg(test)]
mod tests {
    use git2::{Error, ErrorCode, ErrorClass};
    use crate::core::git::default_impl::helpers::map_git2_error;
    use crate::core::git::errors::ErrorCategory;

    fn mk_err(code: ErrorCode, class: ErrorClass, msg: &str) -> Error { Error::new(code, class, msg) }

    #[test]
    fn mapping_snapshot_matrix() {
        let cases = vec![
            (mk_err(ErrorCode::User, ErrorClass::None, "user canceled"), ErrorCategory::Cancel, "cancel"),
            (mk_err(ErrorCode::Net, ErrorClass::Net, "connection timed out"), ErrorCategory::Network, "timeout"),
            (mk_err(ErrorCode::Net, ErrorClass::Net, "连接 超时"), ErrorCategory::Network, "cn-timeout"),
            (mk_err(ErrorCode::GenericError, ErrorClass::Ssl, "tls handshake failure"), ErrorCategory::Tls, "tls"),
            (mk_err(ErrorCode::GenericError, ErrorClass::Ssl, "certificate verify failed"), ErrorCategory::Verify, "cert"),
            (mk_err(ErrorCode::GenericError, ErrorClass::Http, "HTTP 501"), ErrorCategory::Protocol, "http-class"),
            (mk_err(ErrorCode::Auth, ErrorClass::Http, "401 Unauthorized"), ErrorCategory::Auth, "401"),
            (mk_err(ErrorCode::Auth, ErrorClass::Http, "permission denied"), ErrorCategory::Auth, "perm"),
            (mk_err(ErrorCode::GenericError, ErrorClass::Config, "some internal weird"), ErrorCategory::Internal, "internal"),
        ];
        for (err, expect, tag) in cases { assert_eq!(map_git2_error(&err), expect, "case tag={tag} msg={}", err.message()); }
    }
}
