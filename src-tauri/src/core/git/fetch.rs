use std::path::Path;
use gix::bstr::ByteSlice as _;

/// 使用 gix 在阻塞环境中执行 fetch。
///
/// 约定：
/// - `dest` 必须已经是一个 Git 仓库目录；
/// - `repo_url` 可为空字符串：为空时等同于 `git fetch`（自动选择默认远程）；
///   非空时等同于 `git fetch <name-or-url>`，可为远程名或 URL。
/// - 进度使用 Discard；取消通过 `should_interrupt` 协作。
/// 进度回调签名：phase, percent, objects, bytes, total_hint
pub type ProgressCb<'a> = dyn FnMut(&str, u32, Option<u64>, Option<u64>, Option<u64>) + 'a;

/// 带进度回调的阻塞式 fetch。
pub fn fetch_blocking_with_progress<'a>(
    repo_url: &str,
    dest: &Path,
    should_interrupt: &std::sync::atomic::AtomicBool,
    mut on_progress: Box<ProgressCb<'a>>,
    preset: Option<&str>,
) -> Result<(), String> {
    // 快速校验：dest 必须是现有 Git 仓库（存在 .git 目录或为裸仓库路径）
    // 目前按工作区仓库场景处理：要求存在 .git 目录
    let git_dir = dest.join(".git");
    if !git_dir.exists() {
        return Err("dest is not a git repository (missing .git)".into());
    }

    // 打开现有仓库
    let repo = gix::open(dest).map_err(|e| format!("open repo: {}", e))?;

    // 选择远程：
    // - 空字符串 => None（自动选择默认远程或 HEAD 配置）
    // - 否则根据 name/url 解析
    let name_or_url = if repo_url.trim().is_empty() {
        None
    } else {
        Some(gix::bstr::BStr::new(repo_url))
    };

    let remote = repo
        .find_fetch_remote(name_or_url)
        .map_err(|e| format!("find fetch remote: {}", e))?;

    // 建立连接（阻塞网络客户端）
    let con = remote
        .connect(gix::remote::Direction::Fetch)
        .map_err(|e| format!("remote connect: {}", e))?;

    // 通知进入 Negotiating 阶段
    on_progress("Negotiating", 0, None, None, None);

    // 构建 refmap（若远程完全无 fetch refspec，可能返回 MissingRefSpecs）
    // 组装 refmap 选项：当远程无 fetch refspec 或用户显式选择预设时，使用 extra_refspecs 注入常见映射
    let mut opts: gix::remote::ref_map::Options = Default::default();
    if let Some(kind) = preset {
        let mut specs: Vec<gix::refspec::RefSpec> = Vec::new();
        let parse = |s: &str| -> Result<gix::refspec::RefSpec, String> {
            gix::refspec::parse(
                s.as_bytes().as_bstr(),
                gix::refspec::parse::Operation::Fetch,
            )
            .map(|owned| owned.to_owned())
            .map_err(|e| format!("parse refspec: {}", e))
        };
        match kind {
            "branches" => {
                specs.push(parse("+refs/heads/*:refs/remotes/origin/*")?);
            }
            "branches+tags" => {
                specs.push(parse("+refs/heads/*:refs/remotes/origin/*")?);
                specs.push(parse("+refs/tags/*:refs/tags/*")?);
            }
            "tags" => {
                specs.push(parse("+refs/tags/*:refs/tags/*")?);
            }
            _ => {}
        }
        opts.extra_refspecs = specs;
    }

    let prep = con
        .prepare_fetch(gix::progress::Discard, opts)
        .map_err(|e| format!("prepare_fetch: {}", e))?;

    // 通知进入 Receiving 阶段
    on_progress("Receiving", 10, None, None, None);

    // 执行接收
    let _outcome = prep
        .receive(gix::progress::Discard, should_interrupt)
        .map_err(|e| format!("receive: {}", e))?;

    Ok(())
}

/// 兼容旧签名：无进度回调
pub fn fetch_blocking(
    repo_url: &str,
    dest: &Path,
    should_interrupt: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    fetch_blocking_with_progress(
        repo_url,
        dest,
        should_interrupt,
        Box::new(|_, _, _, _, _| {}),
        None,
    )
}
