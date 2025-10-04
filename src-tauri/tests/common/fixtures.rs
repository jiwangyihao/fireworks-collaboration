use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use uuid::Uuid;

use fireworks_collaboration_lib::core::git::default_impl as impls;
use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

/// 代表测试构造的仓库根路径。
#[allow(dead_code)]
pub struct TestRepo {
    pub path: PathBuf,
}

#[allow(dead_code)]
impl TestRepo {
    pub fn join<P: AsRef<Path>>(&self, p: P) -> PathBuf {
        self.path.join(p)
    }
}

/// 内部：生成唯一临时目录路径（不会自动创建）
fn unique_temp(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fwc-{prefix}-{}", Uuid::new_v4()))
}

/// 创建一个临时目录路径（未初始化 .git）。
pub fn temp_dir() -> PathBuf {
    unique_temp("test")
}

/// 创建空目录（未 init）。
pub fn create_empty_dir() -> PathBuf {
    let p = temp_dir();
    std::fs::create_dir_all(&p).expect("create temp dir");
    p
}

/// 通过调用生产实现的 `git_init` 创建一个空 git 仓库。
#[allow(dead_code)]
pub fn create_empty_repo() -> TestRepo {
    let path = temp_dir();
    let cancel = AtomicBool::new(false);
    git_init(&path, &cancel, |_p: ProgressPayload| {}).expect("git init success for fixture");
    TestRepo { path }
}

/// 若路径尚未是 git 仓库则执行 init，返回 Repository。
pub fn ensure_repo(path: &Path) -> git2::Repository {
    match git2::Repository::open(path) {
        Ok(r) => r,
        Err(_) => git2::Repository::init(path).expect("init repo"),
    }
}

/// 创建一个含单初始提交的仓库（写入 README.md）。
#[allow(dead_code)]
pub fn create_repo_with_initial_commit(msg: &str) -> TestRepo {
    let repo = create_empty_repo();
    write_files(&repo.path, &[("README.md", "init\n")]).expect("write readme");
    let cancel = AtomicBool::new(false);
    let _ = impls::commit::git_commit(&repo.path, msg, None, false, &cancel, |_p| {});
    repo
}

/// 批量写文件（必要时递归创建目录）。
pub fn write_files(repo_path: &Path, files: &[(&str, &str)]) -> std::io::Result<()> {
    for (rel, content) in files {
        let full = repo_path.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
    }
    Ok(())
}

/// 直接使用 git2 库向索引添加文件（用于当前 commit/add 测试迁移准备）。
pub fn stage_files(repo_path: &Path, files: &[(&str, &str)]) {
    let repo = ensure_repo(repo_path);
    write_files(repo_path, files).expect("write files for stage");
    let mut index = repo.index().expect("index");
    for (rel, _) in files {
        index.add_path(Path::new(rel)).expect("add path");
    }
    index.write().unwrap();
}

/// 便捷：写入并提交一组文件（需要仓库已初始化）。返回提交是否成功 (Err 透传 `GitError`)。
/// 用途：branch/checkout 等测试快速追加提交，无需重复展开 add + commit 细节。
#[allow(dead_code)]
pub fn commit_files(
    repo_path: &Path,
    files: &[(&str, &str)],
    message: &str,
    allow_empty: bool,
) -> Result<(), fireworks_collaboration_lib::core::git::errors::GitError> {
    use fireworks_collaboration_lib::core::git::default_impl::{add::git_add, commit::git_commit};
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use std::sync::atomic::AtomicBool;
    // 写文件
    write_files(repo_path, files).expect("write commit files");
    let cancel = AtomicBool::new(false);
    // add
    let add_list: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    git_add(repo_path, &add_list, &cancel, |_p: ProgressPayload| {})?;
    // commit
    git_commit(repo_path, message, None, allow_empty, &cancel, |_p| {})
}

/// 便捷：创建仓库并一次性写入 & 暂存若干文件，返回路径。
pub fn repo_with_staged(files: &[(&str, &str)]) -> PathBuf {
    let dir = create_empty_dir();
    stage_files(&dir, files);
    dir
}

/// 将任意字符串转为适合作为路径片段的 slug（去除危险字符，仅保留字母数字与下划线）。
pub fn path_slug<S: AsRef<str>>(s: S) -> String {
    let mut out = String::with_capacity(s.as_ref().len());
    for ch in s.as_ref().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == '-' {
            out.push('-');
        } else {
            out.push('_');
        }
    }
    out
}

/// 读取 `.git/shallow` 文件行（若存在），返回行向量；不存在返回空向量。
#[allow(dead_code)]
pub fn shallow_file_lines(repo: &Path) -> Vec<String> {
    let f = repo.join(".git").join("shallow");
    if !f.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&f) {
        Ok(c) => c
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests_fixtures_smoke {
    use super::*;

    #[test]
    fn slug_replacement() {
        assert_eq!(path_slug("A b/c-1"), "A_b_c-1");
    }

    #[test]
    fn staged_repo_basic() {
        let dir = repo_with_staged(&[("a.txt", "A"), ("b/b.txt", "B")]);
        let repo = ensure_repo(&dir);
        let idx = repo.index().unwrap();
        assert!(idx.iter().any(|e| e.path == b"a.txt"));
        assert!(idx.iter().any(|e| e.path == b"b/b.txt"));
    }

    #[test]
    fn write_files_helper() {
        let dir = create_empty_dir();
        write_files(&dir, &[("x/y.txt", "hi"), ("z.txt", "ok")]).unwrap();
        assert!(dir.join("x/y.txt").exists());
        assert!(dir.join("z.txt").exists());
    }
}
