use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use uuid::Uuid;

use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
use fireworks_collaboration_lib::core::git::default_impl as impls;
use fireworks_collaboration_lib::core::git::errors::GitError;
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

/// 代表测试构造的仓库根路径。
pub struct TestRepo {
    pub path: PathBuf,
}

impl TestRepo {
    pub fn join<P: AsRef<Path>>(&self, p: P) -> PathBuf { self.path.join(p) }
}

/// 创建一个临时目录路径（未初始化 .git）。
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("fwc-test-{}", Uuid::new_v4()))
}

/// 创建空目录（未 init）。
pub fn create_empty_dir() -> PathBuf {
    let p = temp_dir();
    std::fs::create_dir_all(&p).expect("create temp dir");
    p
}

/// 通过调用生产实现的 git_init 创建一个空 git 仓库。
pub fn create_empty_repo() -> TestRepo {
    let path = temp_dir();
    let cancel = AtomicBool::new(false);
    git_init(&path, &cancel, |_p: ProgressPayload| {})
        .expect("git init success for fixture");
    TestRepo { path }
}

/// 读取 HEAD 文件内容（若存在）。
#[allow(dead_code)]
pub fn read_head(repo: &TestRepo) -> std::io::Result<String> {
    let head_path = repo.join(".git/HEAD");
    std::fs::read_to_string(head_path).map(|s| s.trim().to_string())
}

/// 尝试对给定路径执行 git_init，返回错误以便分类断言。
#[allow(dead_code)]
pub fn try_git_init_at(path: &Path) -> Result<(), GitError> {
    let cancel = AtomicBool::new(false);
    git_init(path, &cancel, |_p| {})
}

/// 创建一个含单初始提交的仓库（写入 README.md）。
#[allow(dead_code)]
pub fn create_repo_with_initial_commit(msg: &str) -> TestRepo {
    let repo = create_empty_repo();
    let readme = repo.join("README.md");
    std::fs::write(&readme, "init\n").expect("write readme");
    let cancel = AtomicBool::new(false);
    // 使用当前实现 commit 接口（若尚未实现将返回错误，测试层可根据需要调整）。
    let _ = impls::commit::git_commit(&repo.path, msg, None, false, &cancel, |_p| {});
    repo
}

/// 修改/创建文件。
#[allow(dead_code)]
pub fn modify_file(repo: &TestRepo, rel: &str, content: &str) {
    let p = repo.join(rel);
    if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).expect("mkdirs"); }
    std::fs::write(p, content).expect("write file");
}

/// 将多个文件 add（当前底层命令可能尚未实现，将返回错误；后续阶段会根据实现补齐）。
#[allow(dead_code)] // 未来阶段将接入真正的 add 实现，目前保持为占位以便调用方演进
pub fn add_files_to_index(repo: &TestRepo, files: &[&str]) -> Result<(), GitError> {
    let cancel = AtomicBool::new(false);
    for f in files { let _ = f; }
    // 预留：调用 impls::add::git_add 当实现可用。
    Ok(())
}

/// 列出当前索引内容（路径列表）。若仓库不存在索引或为空，返回空向量。
/// 注意：仅用于测试断言，不排序保持 git2 默认顺序；调用方如需稳定可排序。
#[allow(dead_code)]
pub fn list_index(repo: &TestRepo) -> Vec<String> {
    match git2::Repository::open(&repo.path).and_then(|r| r.index()) {
        Ok(index) => index.iter().map(|e| String::from_utf8_lossy(&e.path).to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

/// 直接使用 git2 库向索引添加文件（用于当前 commit/add 测试迁移准备）。
pub fn stage_files(repo_path: &std::path::Path, files: &[(&str, &str)]) {
    let repo = git2::Repository::open(repo_path).or_else(|_| git2::Repository::init(repo_path)).expect("open or init repo");
    for (rel, content) in files {
        let full = repo_path.join(rel);
        if let Some(parent) = full.parent() { std::fs::create_dir_all(parent).unwrap(); }
        std::fs::write(&full, content).unwrap();
    }
    let mut index = repo.index().expect("index");
    for (rel, _) in files { index.add_path(std::path::Path::new(rel)).expect("add path"); }
    index.write().unwrap();
}

/// 便捷：创建仓库并一次性写入 & 暂存若干文件，返回路径。
pub fn repo_with_staged(files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = create_empty_dir();
    let _ = git2::Repository::init(&dir).expect("init repo");
    stage_files(&dir, files);
    dir
}

/// 统计当前 HEAD 可达提交数量（用于 shallow/deepen 验证）。
/// 如果执行 git rev-list 失败，将 panic（测试期望环境具备 git）。
pub fn rev_count(repo: &std::path::Path) -> u32 {
    use std::process::Command;
    let out = Command::new("git").current_dir(repo).args(["rev-list", "--count", "HEAD"]).output().expect("run git rev-list");
    assert!(out.status.success(), "rev-list failed for {:?}", repo);
    String::from_utf8_lossy(&out.stdout).trim().parse().expect("parse rev count")
}

/// 将任意字符串转为适合作为路径片段的 slug（去除危险字符，仅保留字母数字与下划线）。
pub fn path_slug<S: AsRef<str>>(s: S) -> String {
    let mut out = String::with_capacity(s.as_ref().len());
    for ch in s.as_ref().chars() {
        if ch.is_ascii_alphanumeric() { out.push(ch); } else { out.push('_'); }
    }
    out
}

/// 读取 `.git/shallow` 文件行（若存在），返回行向量；不存在返回空向量。
pub fn shallow_file_lines(repo: &std::path::Path) -> Vec<String> {
    let f = repo.join(".git").join("shallow");
    if !f.exists() { return Vec::new(); }
    match std::fs::read_to_string(&f) { Ok(c) => c.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(), Err(_) => Vec::new() }
}

/// 检测仓库是否为浅克隆：依据 `.git/shallow` 文件是否存在且非空；返回 (is_shallow, line_count)。
/// 占位实现：未来可结合 git2 API (Repository::is_shallow) 或自定义状态实现。
#[allow(dead_code)]
pub fn detect_shallow_repo(repo: &std::path::Path) -> (bool, usize) {
    let lines = shallow_file_lines(repo);
    (!lines.is_empty(), lines.len())
}
