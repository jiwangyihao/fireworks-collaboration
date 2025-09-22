//! pipeline: E2E 流水线 DSL 脚手架 (12.15 前置)
//! 目标：为 e2e_public_git 聚合文件提供统一的步骤描述 & 执行函数，
//! 先以占位/模拟形式存在，后续再接入真实 GitService / 事件收集。
//!
//! 设计原则：
//!  * 声明式：PipelineSpec 列出步骤序列；
//!  * 幂等执行：每个步骤独立返回结构，允许后续缓存 clone 结果；
//!  * 事件抽象：当前仅返回字符串锚点，未来接入结构化事件或真实 TaskEvent。
//!  * 失败策略：占位阶段不模拟复杂错误，保留接口字段。
//!
//! 后续增强 (计划)：
//!  * 引入共享远端仓库 fixture + 本地缓存目录
//!  * 支持超时、取消、错误注入 (fault injection)
//!  * 事件收集改为结构化枚举 + tag DSL

use std::path::PathBuf;
use std::process::Command;

// ---- 新增：Pipeline 配置 & 故障注入 ----
#[derive(Debug, Clone, Default)]
pub struct PipelineConfig {
    pub remote_commits: usize,          // 远端初始提交数量
    pub enable_real: bool,              // 是否执行真实 git 操作
    pub faults: Vec<FaultKind>,         // 故障注入集合
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind { ForcePushFailure }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStepKind { Clone, Modify, Commit, Push, Fetch }

#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub kind: PipelineStepKind,
    pub desc: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct PipelineSpec {
    pub steps: Vec<PipelineStep>,
}

impl PipelineSpec {
    pub fn new(steps: Vec<PipelineStep>) -> Self { Self { steps } }
    pub fn basic_clone_build_push() -> Self {
        Self::new(vec![
            PipelineStep { kind: PipelineStepKind::Clone, desc: "clone" },
            PipelineStep { kind: PipelineStepKind::Modify, desc: "modify" },
            PipelineStep { kind: PipelineStepKind::Commit, desc: "commit" },
            PipelineStep { kind: PipelineStepKind::Push, desc: "push" },
            PipelineStep { kind: PipelineStepKind::Fetch, desc: "fetch" },
        ])
    }
    pub fn read_only() -> Self {
        Self::new(vec![
            PipelineStep { kind: PipelineStepKind::Clone, desc: "clone" },
            PipelineStep { kind: PipelineStepKind::Fetch, desc: "fetch" },
        ])
    }
}

#[derive(Debug, Default)]
pub struct PipelineOutcome {
    pub events: Vec<String>,
    pub workdir: Option<PathBuf>,
    pub failed: bool,
    pub remote_dir: Option<PathBuf>,
    pub commit_count_before: Option<u32>,
    pub commit_count_after: Option<u32>,
}

pub fn run_pipeline(spec: &PipelineSpec) -> PipelineOutcome {
    let mut out = PipelineOutcome::default();
    for step in &spec.steps {
        match step.kind {
            PipelineStepKind::Clone => {
                // 占位：创建临时目录并记录事件
                let dir = std::env::temp_dir().join(format!("fwc-pipeline-{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&dir).expect("create pipeline temp dir");
                out.workdir = Some(dir);
                out.events.push("pipeline:clone:start".into());
                out.events.push("pipeline:clone:complete".into());
            }
            PipelineStepKind::Modify => {
                out.events.push("pipeline:modify:file_changed".into());
            }
            PipelineStepKind::Commit => {
                out.events.push("pipeline:commit:create".into());
            }
            PipelineStepKind::Push => {
                out.events.push("pipeline:push:start".into());
                out.events.push("pipeline:push:success".into());
            }
            PipelineStepKind::Fetch => {
                out.events.push("pipeline:fetch:start".into());
                out.events.push("pipeline:fetch:complete".into());
            }
        }
    }
    out
}

// ---- 真实执行支持 ----
// 建立裸仓库并创建指定数量提交（使用工作克隆临时目录）。
fn create_bare_remote_with_commits(n: usize) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("fwc-remote-src-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("mkdir tmp");
    run(Command::new("git").arg("init").arg(&tmp));
    for i in 1..=n.max(1) { // 至少一个提交
        let f = tmp.join(format!("f{}.txt", i));
        std::fs::write(&f, format!("c{}\n", i)).unwrap();
        run(Command::new("git").current_dir(&tmp).args(["add", "."]));
        run(Command::new("git").current_dir(&tmp).args(["commit", "-m", &format!("c{}", i)]));
    }
    // 裸仓库
    let bare = std::env::temp_dir().join(format!("fwc-remote-bare-{}", uuid::Uuid::new_v4()));
    run(Command::new("git").args(["clone", "--bare", tmp.to_string_lossy().as_ref(), bare.to_string_lossy().as_ref()]));
    bare
}

fn run(cmd: &mut Command) { let status = cmd.status().expect("run command"); assert!(status.success(), "command failed: {:?}", cmd); }

fn git_rev_count(repo: &PathBuf) -> u32 {
    let out = Command::new("git").current_dir(repo).args(["rev-list", "--count", "HEAD"]).output().expect("rev-list");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

pub fn run_pipeline_with(spec: &PipelineSpec, cfg: &PipelineConfig) -> PipelineOutcome {
    if !cfg.enable_real { return run_pipeline(spec); }
    let mut out = PipelineOutcome::default();
    // 准备远端
    let remote = create_bare_remote_with_commits(cfg.remote_commits);
    out.remote_dir = Some(remote.clone());
    let mut local: Option<PathBuf> = None;
    for step in &spec.steps {
        match step.kind {
            PipelineStepKind::Clone => {
                let dir = std::env::temp_dir().join(format!("fwc-pipeline-real-{}", uuid::Uuid::new_v4()));
                run(Command::new("git").args(["clone", remote.to_string_lossy().as_ref(), dir.to_string_lossy().as_ref()]));
                out.commit_count_before = Some(git_rev_count(&dir));
                local = Some(dir.clone());
                out.workdir = Some(dir);
                out.events.push("pipeline:clone:complete".into());
            }
            PipelineStepKind::Modify => {
                if let Some(l) = &local { std::fs::write(l.join("new.txt"), "data\n").unwrap(); out.events.push("pipeline:modify:file_changed".into()); }
            }
            PipelineStepKind::Commit => {
                if let Some(l) = &local {
                    run(Command::new("git").current_dir(l).args(["add", "."]));
                    run(Command::new("git").current_dir(l).args(["commit", "-m", "pipeline_commit"]));
                    out.events.push("pipeline:commit:create".into());
                }
            }
            PipelineStepKind::Push => {
                let mut force_fail = cfg.faults.iter().any(|f| matches!(f, FaultKind::ForcePushFailure));
                if let Some(l) = &local {
                    if force_fail { // 改 remote URL 指向不存在以触发失败
                        run(Command::new("git").current_dir(l).args(["remote", "set-url", "origin", "file:///non/existent/remote"]));
                    }
                    let status = Command::new("git").current_dir(l).args(["push", "origin", "HEAD:refs/heads/master"]).status().expect("push");
                    if status.success() && !force_fail { out.events.push("pipeline:push:success".into()); } else { out.failed = true; out.events.push("pipeline:push:failed".into()); }
                }
            }
            PipelineStepKind::Fetch => {
                if let Some(l) = &local {
                    let status = Command::new("git").current_dir(l).args(["fetch", "origin"]).status().expect("fetch");
                    if status.success() { out.events.push("pipeline:fetch:complete".into()); } else { out.failed = true; out.events.push("pipeline:fetch:failed".into()); }
                    out.commit_count_after = Some(git_rev_count(l));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests_pipeline_smoke {
    use super::*;
    #[test]
    fn smoke_basic_pipeline() {
        let spec = PipelineSpec::basic_clone_build_push();
        let out = run_pipeline(&spec);
        assert!(out.events.iter().any(|e| e.contains("clone:complete")));
        assert!(out.events.iter().any(|e| e.contains("push:success")));
    }
}
