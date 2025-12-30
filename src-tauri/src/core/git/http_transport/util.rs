use crate::core::ip_pool::IpSource;

pub(super) fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    // 返回头部结束后紧随 body 的起始索引（含 CRLFCRLF 整个序列长度）
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

pub(super) fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

pub(super) fn parse_http_header_first_line_and_host(header: &[u8]) -> (String, String, bool) {
    let mut line1 = String::new();
    let mut host = String::new();
    let mut has_auth = false;
    let text = match std::str::from_utf8(header) {
        Ok(s) => s,
        Err(_) => return (line1, host, false),
    };
    for (i, line) in text.split("\r\n").enumerate() {
        if i == 0 {
            line1 = line.trim().to_string();
            continue;
        }
        let mut parts = line.splitn(2, ':');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k.eq_ignore_ascii_case("host") {
                host = v.trim().to_string();
            }
            if k.eq_ignore_ascii_case("authorization") {
                has_auth = true;
            }
        }
    }
    (line1, host, has_auth)
}

pub(super) fn log_body_preview(body: &[u8], host: &str, msg: &str) {
    let n = body.len();
    let hex: String = body.iter().map(|b| format!("{b:02x}")).collect();
    let ascii = body
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect::<String>();
    tracing::debug!(target="git.transport.http", host=%host, bytes=%n, hex=%hex, ascii=%ascii, "{}", msg);
}

pub(super) fn format_ip_sources(sources: &[IpSource]) -> String {
    if sources.is_empty() {
        return "unknown".to_string();
    }
    sources
        .iter()
        .map(|src| match src {
            IpSource::Builtin => "Builtin",
            IpSource::Dns => "Dns",
            IpSource::History => "History",
            IpSource::UserStatic => "UserStatic",
            IpSource::Fallback => "Fallback",
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_double_crlf() {
        let data = b"Header: value\r\n\r\nBody";
        assert_eq!(find_double_crlf(data), Some(17)); // "Header: value" len 13 + 4 = 17

        let none = b"Header: value\r\nBody";
        assert_eq!(find_double_crlf(none), None);

        let partial = b"Header\r\n\r";
        assert_eq!(find_double_crlf(partial), None);
    }

    #[test]
    fn test_parse_http_header_basic() {
        let input = b"HTTP/1.1 200 OK\r\nHost: example.com\r\nAuthorization: Basic 123\r\n\r\n";
        let (line1, host, has_auth) = parse_http_header_first_line_and_host(input);

        assert_eq!(line1, "HTTP/1.1 200 OK");
        assert_eq!(host, "example.com");
        assert!(has_auth);
    }

    #[test]
    fn test_parse_http_header_no_auth() {
        let input = b"GET / HTTP/1.1\r\nHost:  example.org \r\nAccept: */*\r\n\r\n";
        let (line1, host, has_auth) = parse_http_header_first_line_and_host(input);

        assert_eq!(line1, "GET / HTTP/1.1");
        assert_eq!(host, "example.org");
        assert!(!has_auth);
    }

    #[test]
    fn test_parse_http_header_case_insensitive() {
        let input =
            b"POST /git-receive-pack HTTP/1.1\r\nhost: my.git\r\nauthorization: token\r\n\r\n";
        let (line1, host, has_auth) = parse_http_header_first_line_and_host(input);

        assert_eq!(line1, "POST /git-receive-pack HTTP/1.1");
        assert_eq!(host, "my.git");
        assert!(has_auth);
    }
}
