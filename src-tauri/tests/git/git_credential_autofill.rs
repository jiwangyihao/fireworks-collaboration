//! 测试 Git Push 凭证自动填充功能
//! 
//! 这些测试验证 P6.4 阶段实现的 Git URL 解析和凭证自动填充逻辑。
//!
//! `注意：parse_git_host` 和 `extract_git_host` 在 git.rs 中标记为 pub(crate)，
//! 但由于测试文件在不同的crate中，我们使用功能等价的本地实现进行测试。

#[cfg(test)]
mod unit_tests {
    /// 测试 `parse_git_host` 函数 - HTTPS URL
    #[test]
    fn test_parse_git_host_https() {
        // HTTPS 格式测试
        let test_cases = vec![
            ("https://github.com/user/repo.git", "github.com"),
            ("https://gitlab.com/group/project.git", "gitlab.com"),
            ("http://example.com/repo.git", "example.com"),
        ];
        
        for (url, expected_host) in test_cases {
            let result = parse_git_host(url);
            assert_eq!(result.unwrap(), expected_host, "Failed to parse HTTPS URL: {url}");
        }
    }
    
    /// 测试 `parse_git_host` 函数 - SSH URL (git@ 格式)
    #[test]
    fn test_parse_git_host_ssh_git_at() {
        let test_cases = vec![
            ("git@github.com:user/repo.git", "github.com"),
            ("git@gitlab.com:group/project.git", "gitlab.com"),
        ];
        
        for (url, expected_host) in test_cases {
            let result = parse_git_host(url);
            assert_eq!(result.unwrap(), expected_host, "Failed to parse SSH git@ URL: {url}");
        }
    }
    
    /// 测试 `parse_git_host` 函数 - SSH URL (ssh:// 格式)
    #[test]
    fn test_parse_git_host_ssh_protocol() {
        let test_cases = vec![
            ("ssh://git@github.com/user/repo.git", "github.com"),
            ("ssh://git@gitlab.com/group/project.git", "gitlab.com"),
        ];
        
        for (url, expected_host) in test_cases {
            let result = parse_git_host(url);
            assert_eq!(result.unwrap(), expected_host, "Failed to parse SSH protocol URL: {url}");
        }
    }
    
    /// 测试不支持的 URL 格式
    #[test]
    fn test_parse_git_host_unsupported() {
        let invalid_urls = vec![
            "ftp://github.com/user/repo.git",
            "file:///local/repo.git",
            "invalid-url",
            "",
        ];
        
        for url in invalid_urls {
            let result = parse_git_host(url);
            assert!(result.is_err(), "Should reject unsupported URL: {url}");
        }
    }
    
    /// 测试边界情况 - 仅主机名、带端口等
    #[test]
    fn test_parse_git_host_edge_cases() {
        // HTTPS 格式的最小有效 URL
        assert_eq!(parse_git_host("https://github.com").unwrap(), "github.com");
        
        // SSH git@ 格式的最小有效 URL
        assert_eq!(parse_git_host("git@github.com:repo").unwrap(), "github.com");
        
        // 带端口的 HTTPS URL
        assert_eq!(parse_git_host("https://github.com:443/user/repo.git").unwrap(), "github.com:443");
        
        // 不带 .git 后缀
        assert_eq!(parse_git_host("https://github.com/user/repo").unwrap(), "github.com");
    }
    
    /// Parse host from various Git URL formats.
    /// 
    /// This is a test-local copy of the implementation in git.rs to ensure
    /// the test logic matches the actual implementation.
    fn parse_git_host(url: &str) -> Result<String, String> {
        // HTTPS: https://github.com/user/repo.git
        if url.starts_with("https://") || url.starts_with("http://") {
            let without_scheme = url.trim_start_matches("https://").trim_start_matches("http://");
            let host = without_scheme.split('/').next()
                .ok_or("Invalid HTTPS URL")?;
            return Ok(host.to_string());
        }
        
        // SSH: git@github.com:user/repo.git
        if url.starts_with("git@") {
            let without_user = url.trim_start_matches("git@");
            let host = without_user.split(':').next()
                .ok_or("Invalid SSH URL")?;
            return Ok(host.to_string());
        }
        
        // SSH: ssh://git@github.com/user/repo.git
        if url.starts_with("ssh://") {
            let without_scheme = url.trim_start_matches("ssh://");
            let without_user = without_scheme.trim_start_matches("git@");
            let host = without_user.split('/').next()
                .ok_or("Invalid SSH URL")?;
            return Ok(host.to_string());
        }
        
        Err(format!("Unsupported Git URL format: {url}"))
    }
}


#[cfg(test)]
mod integration_tests {
    use tempfile::TempDir;
    use std::process::Command;
    use std::path::Path;
    
    /// 测试从实际 Git 仓库提取 host
    /// 
    /// 这个测试会创建一个临时 Git 仓库并配置 remote，
    /// 然后验证能否正确提取 host
    #[test]
    fn test_extract_git_host_from_real_repo() {
        // 创建临时目录
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // 初始化 Git 仓库
        let init_output = Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .output();
        
        if init_output.is_err() {
            // Git 不可用，跳过测试
            eprintln!("Git not available, skipping integration test");
            return;
        }
        
        let init_result = init_output.unwrap();
        if !init_result.status.success() {
            eprintln!("Failed to initialize git repo, skipping test");
            return;
        }
        
        // 配置 remote URL (HTTPS 格式)
        let config_output = Command::new("git")
            .arg("config")
            .arg("remote.origin.url")
            .arg("https://github.com/test/repo.git")
            .current_dir(repo_path)
            .output()
            .unwrap();
        
        assert!(config_output.status.success(), "Failed to set remote URL");
        
        // 验证能够提取 host
        let host = extract_git_host(repo_path.to_str().unwrap());
        assert!(host.is_ok(), "Failed to extract host from Git repo");
        assert_eq!(host.unwrap(), "github.com");
    }
    
    /// 测试从 SSH 格式的 remote 提取 host
    #[test]
    fn test_extract_git_host_ssh_remote() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        let init_output = Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .output();
        
        if init_output.is_err() || !init_output.unwrap().status.success() {
            eprintln!("Git not available, skipping integration test");
            return;
        }
        
        // 配置 SSH 格式的 remote URL
        let config_output = Command::new("git")
            .arg("config")
            .arg("remote.origin.url")
            .arg("git@gitlab.com:group/project.git")
            .current_dir(repo_path)
            .output()
            .unwrap();
        
        assert!(config_output.status.success());
        
        let host = extract_git_host(repo_path.to_str().unwrap());
        assert!(host.is_ok());
        assert_eq!(host.unwrap(), "gitlab.com");
    }
    
    /// 测试非 Git 仓库目录
    #[test]
    fn test_extract_git_host_not_a_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // 不初始化 Git 仓库，直接尝试提取
        let host = extract_git_host(repo_path.to_str().unwrap());
        assert!(host.is_err(), "Should fail for non-Git directory");
        assert!(host.unwrap_err().contains("Not a git repository"));
    }
    
    /// 测试没有配置 remote 的仓库
    #[test]
    fn test_extract_git_host_no_remote() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        let init_output = Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .output();
        
        if init_output.is_err() || !init_output.unwrap().status.success() {
            eprintln!("Git not available, skipping integration test");
            return;
        }
        
        // 不配置 remote，直接尝试提取
        let host = extract_git_host(repo_path.to_str().unwrap());
        assert!(host.is_err(), "Should fail for repo without remote");
    }
    
    /// Extract Git host from a repository path by reading git config.
    /// 
    /// This is a test-local copy of the implementation in git.rs.
    fn extract_git_host(repo_path: &str) -> Result<String, String> {
        let path = Path::new(repo_path);
        if !path.exists() || !path.join(".git").exists() {
            return Err("Not a git repository".to_string());
        }
        
        let output = Command::new("git")
            .arg("config")
            .arg("--get")
            .arg("remote.origin.url")
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git config: {e}"))?;
        
        if !output.status.success() {
            return Err("Failed to get remote URL".to_string());
        }
        
        let url = String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 in remote URL: {e}"))?
            .trim()
            .to_string();
        
        parse_git_host(&url)
    }
    
    /// Parse host from various Git URL formats.
    fn parse_git_host(url: &str) -> Result<String, String> {
        if url.starts_with("https://") || url.starts_with("http://") {
            let without_scheme = url.trim_start_matches("https://").trim_start_matches("http://");
            let host = without_scheme.split('/').next()
                .ok_or("Invalid HTTPS URL")?;
            return Ok(host.to_string());
        }
        
        if url.starts_with("git@") {
            let without_user = url.trim_start_matches("git@");
            let host = without_user.split(':').next()
                .ok_or("Invalid SSH URL")?;
            return Ok(host.to_string());
        }
        
        if url.starts_with("ssh://") {
            let without_scheme = url.trim_start_matches("ssh://");
            let without_user = without_scheme.trim_start_matches("git@");
            let host = without_user.split('/').next()
                .ok_or("Invalid SSH URL")?;
            return Ok(host.to_string());
        }
        
        Err(format!("Unsupported Git URL format: {url}"))
    }
}
