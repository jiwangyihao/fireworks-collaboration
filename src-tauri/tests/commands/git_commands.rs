//! Git 命令函数测试
//!
//! 测试 `app::commands::git` 模块中的辅助函数

use fireworks_collaboration_lib::app::commands::git::parse_git_host;

// =========================================================================
// parse_git_host tests
// =========================================================================

#[test]
fn test_parse_git_host_https() {
    assert_eq!(
        parse_git_host("https://github.com/user/repo.git").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_https_no_suffix() {
    assert_eq!(
        parse_git_host("https://github.com/user/repo").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_http() {
    assert_eq!(
        parse_git_host("http://gitlab.example.com/group/project").unwrap(),
        "gitlab.example.com"
    );
}

#[test]
fn test_parse_git_host_https_custom() {
    assert_eq!(
        parse_git_host("https+custom://github.com/user/repo.git").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_https_with_userinfo() {
    assert_eq!(
        parse_git_host("https://user:pass@github.com/user/repo.git").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_https_with_port() {
    assert_eq!(
        parse_git_host("https://github.com:443/user/repo.git").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_ssh_classic() {
    assert_eq!(
        parse_git_host("git@github.com:user/repo.git").unwrap(),
        "github.com"
    );
}

#[test]
fn test_parse_git_host_ssh_protocol() {
    assert_eq!(
        parse_git_host("ssh://git@gitlab.com/user/repo.git").unwrap(),
        "gitlab.com"
    );
}

#[test]
fn test_parse_git_host_unsupported_scheme() {
    let result = parse_git_host("ftp://example.com/repo");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unsupported Git URL format"));
}

#[test]
fn test_parse_git_host_invalid_url() {
    let result = parse_git_host("not a url at all");
    assert!(result.is_err());
}

#[test]
fn test_parse_git_host_subdomain() {
    assert_eq!(
        parse_git_host("https://api.github.com/repos").unwrap(),
        "api.github.com"
    );
}
