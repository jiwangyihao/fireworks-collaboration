use std::path::PathBuf;

/// 快速创建包含若干顺序提交的本地仓库，返回 (tempdir, path)。
/// commits: (&str, &str) => (文件名, 内容)，依次追加提交。
pub fn build_repo(commits: &[(&str,&str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp repo");
    let dir_path = dir.path().to_path_buf();
    let repo = git2::Repository::init(&dir_path).expect("init repo");
    for (fname, content) in commits {
        std::fs::write(dir_path.join(fname), content).expect("write file");
        let mut index = repo.index().expect("index");
        index.add_path(std::path::Path::new(fname)).expect("add");
        index.write().expect("write index");
        let tree_id = index.write_tree().expect("tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        let sig = repo.signature().expect("sig");
        repo.commit(Some("HEAD"), &sig, &sig, &format!("commit:{}", fname), &tree, &[]).expect("commit");
    }
    (dir, dir_path)
}
