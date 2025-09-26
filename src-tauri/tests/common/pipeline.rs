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

// ---- 事件常量（集中管理，便于未来结构化事件枚举替换） ----
const EV_CLONE_START: &str = "pipeline:clone:start";
const EV_CLONE_COMPLETE: &str = "pipeline:clone:complete";
const EV_MODIFY_FILE_CHANGED: &str = "pipeline:modify:file_changed";
const EV_COMMIT_CREATE: &str = "pipeline:commit:create";
const EV_PUSH_START: &str = "pipeline:push:start";
const EV_PUSH_SUCCESS: &str = "pipeline:push:success";
const EV_PUSH_FAILED: &str = "pipeline:push:failed";
const EV_FETCH_START: &str = "pipeline:fetch:start";
const EV_FETCH_COMPLETE: &str = "pipeline:fetch:complete";
const EV_FETCH_FAILED: &str = "pipeline:fetch:failed";

fn emit(events: &mut Vec<String>, e: &str) {
    events.push(e.into());
}

// ---- 新增：Pipeline 配置 & 故障注入 ----
#[derive(Debug, Clone, Default)]
pub struct PipelineConfig {
    pub remote_commits: usize,  // 远端初始提交数量
    pub enable_real: bool,      // 是否执行真实 git 操作
    pub faults: Vec<FaultKind>, // 故障注入集合
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    ForcePushFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStepKind {
    /// 克隆远端（或模拟克隆）
    Clone,
    /// 修改工作区文件（模拟新增/变更）
    Modify,
    /// 产生一次提交
    Commit,
    /// 推送到远端
    Push,
    /// 获取远端更新（或刷新状态）
    Fetch,
}

#[derive(Debug, Clone)]
pub struct PipelineStep {
    pub kind: PipelineStepKind,
    #[allow(dead_code)] // 预留：未来用于报告 / 可读性输出（当前测试未直接断言 desc 内容）
    pub desc: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct PipelineSpec {
    pub steps: Vec<PipelineStep>,
}

impl PipelineSpec {
    #[allow(dead_code)] // 预留：外部可直接构造完整步骤序列（当前内部主要走 builder）
    pub fn new(steps: Vec<PipelineStep>) -> Self {
        Self { steps }
    }
    pub fn basic_clone_build_push() -> Self {
        PipelineBuilder::new()
            .clone_step()
            .modify()
            .commit()
            .push()
            .fetch()
            .build()
    }
    #[allow(dead_code)] // 预留：后续可能用于只读校验场景（clone + fetch），当前未被调用
    pub fn read_only() -> Self {
        PipelineBuilder::new().clone_step().fetch().build()
    }
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::new()
    }
}

#[derive(Debug, Default)]
pub struct PipelineBuilder {
    steps: Vec<PipelineStep>,
}
impl PipelineBuilder {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }
    pub fn push_step(mut self, kind: PipelineStepKind, desc: &'static str) -> Self {
        self.steps.push(PipelineStep { kind, desc });
        self
    }
    pub fn clone_step(self) -> Self {
        self.push_step(PipelineStepKind::Clone, "clone")
    }
    pub fn modify(self) -> Self {
        self.push_step(PipelineStepKind::Modify, "modify")
    }
    pub fn commit(self) -> Self {
        self.push_step(PipelineStepKind::Commit, "commit")
    }
    pub fn push(self) -> Self {
        self.push_step(PipelineStepKind::Push, "push")
    }
    pub fn fetch(self) -> Self {
        self.push_step(PipelineStepKind::Fetch, "fetch")
    }
    pub fn build(self) -> PipelineSpec {
        PipelineSpec { steps: self.steps }
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

impl PipelineOutcome {
    pub fn is_failed(&self) -> bool {
        self.failed
    }
    pub fn is_success(&self) -> bool {
        !self.failed
    }
    pub fn commit_delta(&self) -> Option<i32> {
        match (self.commit_count_before, self.commit_count_after) {
            (Some(b), Some(a)) => Some(a as i32 - b as i32),
            _ => None,
        }
    }
    pub fn has_event_prefix(&self, prefix: &str) -> bool {
        self.events.iter().any(|e| e.starts_with(prefix))
    }
}

// ---- 公共断言辅助（供 e2e / 其它聚合复用） ----
/// 断言提交数量至少增长 min_delta。
#[allow(dead_code)]
pub fn assert_commit_growth_at_least(out: &PipelineOutcome, min_delta: i32) {
    let delta = out
        .commit_delta()
        .expect("commit delta should exist in success path");
    assert!(
        delta >= min_delta,
        "expected commit growth >= {min_delta}, got {delta}; before={:?} after={:?}",
        out.commit_count_before,
        out.commit_count_after
    );
}

/// 断言提交数量未变化（delta == 0）。
#[allow(dead_code)]
pub fn assert_commit_unchanged(out: &PipelineOutcome) {
    let delta = out
        .commit_delta()
        .expect("commit delta should exist for unchanged assertion");
    assert_eq!(
        delta, 0,
        "expected commit unchanged (delta=0), got {delta}; before={:?} after={:?}",
        out.commit_count_before, out.commit_count_after
    );
}

/// 失败路径：若 before/after 同时存在则确认未前进；缺失 after 视为允许（fetch 失败等情况）。
#[allow(dead_code)]
pub fn assert_failure_commit_not_advanced(out: &PipelineOutcome) {
    // 优先依据远端仓库判断（push 失败时更符合预期：远端不应前进）
    if let Some(remote) = &out.remote_dir {
        if let Some(b) = out.commit_count_before {
            // b 在 clone 步设置：等于远端初始提交数
            let a_remote = git_rev_count(remote);
            assert_eq!(
                a_remote, b,
                "commit count should not advance on failure: before={b} after={a_remote}"
            );
            return;
        }
    }
    // 回退：若没有远端信息，则比较本地 before/after（仅模拟路径）
    if let (Some(b), Some(a)) = (out.commit_count_before, out.commit_count_after) {
        assert_eq!(
            a, b,
            "commit count should not advance on failure: before={b} after={a}"
        );
    }
}

pub fn run_pipeline(spec: &PipelineSpec) -> PipelineOutcome {
    let mut out = PipelineOutcome::default();
    for step in &spec.steps {
        run_step_simulated(step, &mut out);
    }
    out
}

fn run_step_simulated(step: &PipelineStep, out: &mut PipelineOutcome) {
    match step.kind {
        PipelineStepKind::Clone => {
            let dir = std::env::temp_dir().join(format!("fwc-pipeline-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("create pipeline temp dir");
            out.workdir = Some(dir);
            emit(&mut out.events, EV_CLONE_START);
            emit(&mut out.events, EV_CLONE_COMPLETE);
        }
        PipelineStepKind::Modify => emit(&mut out.events, EV_MODIFY_FILE_CHANGED),
        PipelineStepKind::Commit => emit(&mut out.events, EV_COMMIT_CREATE),
        PipelineStepKind::Push => {
            emit(&mut out.events, EV_PUSH_START);
            emit(&mut out.events, EV_PUSH_SUCCESS);
        }
        PipelineStepKind::Fetch => {
            emit(&mut out.events, EV_FETCH_START);
            emit(&mut out.events, EV_FETCH_COMPLETE);
        }
    }
}

// ---- 真实执行支持 ----
// 建立裸仓库并创建指定数量提交（使用工作克隆临时目录）。
fn create_bare_remote_with_commits(n: usize) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("fwc-remote-src-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("mkdir tmp");
    run(Command::new("git").arg("init").arg(&tmp));
    for i in 1..=n.max(1) {
        // 至少一个提交
        let f = tmp.join(format!("f{}.txt", i));
        std::fs::write(&f, format!("c{}\n", i)).unwrap();
        run(Command::new("git").current_dir(&tmp).args(["add", "."]));
        run(Command::new("git")
            .current_dir(&tmp)
            .args(["commit", "-m", &format!("c{}", i)]));
    }
    // 裸仓库
    let bare = std::env::temp_dir().join(format!("fwc-remote-bare-{}", uuid::Uuid::new_v4()));
    run(Command::new("git").args([
        "clone",
        "--bare",
        tmp.to_string_lossy().as_ref(),
        bare.to_string_lossy().as_ref(),
    ]));
    bare
}

fn run(cmd: &mut Command) {
    let status = cmd.status().expect("run command");
    assert!(status.success(), "command failed: {:?}", cmd);
}

fn git_rev_count(repo: &PathBuf) -> u32 {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("rev-list");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

pub fn run_pipeline_with(spec: &PipelineSpec, cfg: &PipelineConfig) -> PipelineOutcome {
    if !cfg.enable_real {
        return run_pipeline(spec);
    }
    let mut out = PipelineOutcome::default();
    // 准备远端
    let remote = create_bare_remote_with_commits(cfg.remote_commits);
    out.remote_dir = Some(remote.clone());
    let mut local: Option<PathBuf> = None;
    for step in &spec.steps {
        match step.kind {
            PipelineStepKind::Clone => {
                let dir = std::env::temp_dir()
                    .join(format!("fwc-pipeline-real-{}", uuid::Uuid::new_v4()));
                run(Command::new("git").args([
                    "clone",
                    remote.to_string_lossy().as_ref(),
                    dir.to_string_lossy().as_ref(),
                ]));
                out.commit_count_before = Some(git_rev_count(&dir));
                local = Some(dir.clone());
                out.workdir = Some(dir);
                emit(&mut out.events, EV_CLONE_COMPLETE);
            }
            PipelineStepKind::Modify => {
                if let Some(l) = &local {
                    std::fs::write(l.join("new.txt"), "data\n").unwrap();
                    emit(&mut out.events, EV_MODIFY_FILE_CHANGED);
                }
            }
            PipelineStepKind::Commit => {
                if let Some(l) = &local {
                    run(Command::new("git").current_dir(l).args(["add", "."]));
                    run(Command::new("git").current_dir(l).args([
                        "commit",
                        "-m",
                        "pipeline_commit",
                    ]));
                    emit(&mut out.events, EV_COMMIT_CREATE);
                }
            }
            PipelineStepKind::Push => {
                let force_fail = cfg
                    .faults
                    .iter()
                    .any(|f| matches!(f, FaultKind::ForcePushFailure));
                if let Some(l) = &local {
                    if force_fail {
                        // 改 remote URL 指向不存在以触发失败
                        run(Command::new("git").current_dir(l).args([
                            "remote",
                            "set-url",
                            "origin",
                            "file:///non/existent/remote",
                        ]));
                    }
                    emit(&mut out.events, EV_PUSH_START);
                    let status = Command::new("git")
                        .current_dir(l)
                        .args(["push", "origin", "HEAD:refs/heads/master"])
                        .status()
                        .expect("push");
                    if status.success() && !force_fail {
                        emit(&mut out.events, EV_PUSH_SUCCESS);
                    } else {
                        out.failed = true;
                        emit(&mut out.events, EV_PUSH_FAILED);
                    }
                }
            }
            PipelineStepKind::Fetch => {
                if let Some(l) = &local {
                    emit(&mut out.events, EV_FETCH_START);
                    let status = Command::new("git")
                        .current_dir(l)
                        .args(["fetch", "origin"])
                        .status()
                        .expect("fetch");
                    if status.success() {
                        emit(&mut out.events, EV_FETCH_COMPLETE);
                    } else {
                        out.failed = true;
                        emit(&mut out.events, EV_FETCH_FAILED);
                    }
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
        assert!(out.has_event_prefix(EV_CLONE_COMPLETE));
        assert!(out.has_event_prefix(EV_PUSH_SUCCESS));
    }

    #[test]
    fn builder_and_outcome_helpers() {
        let spec = PipelineSpec::builder()
            .clone_step()
            .modify()
            .commit()
            .push()
            .fetch()
            .build();
        let out = run_pipeline(&spec);
        assert!(out.is_success());
        assert!(!out.is_failed());
        assert!(out.has_event_prefix("pipeline:push:"));
    }
}

#[cfg(test)]
mod tests_pipeline_real {
    use super::*;

    // 真实执行（启用 git 命令），并注入故障，覆盖：
    // * PipelineConfig.remote_commits / enable_real / faults 字段读取
    // * FaultKind::ForcePushFailure 枚举变体
    // * run_pipeline_with 分支、EV_PUSH_FAILED / EV_FETCH_FAILED 常量
    // * PipelineOutcome.remote_dir / commit_count_before / commit_count_after 字段读取
    // * PipelineOutcome::commit_delta 方法调用
    #[test]
    fn real_pipeline_force_push_failure() {
        // 配置含 2 个初始提交，开启真实模式，并注入推送失败故障
        let cfg = PipelineConfig {
            remote_commits: 2,
            enable_real: true,
            faults: vec![FaultKind::ForcePushFailure],
        };
        let spec = PipelineSpec::builder()
            .clone_step()
            .modify()
            .commit()
            .push()
            .fetch()
            .build();
        let out = run_pipeline_with(&spec, &cfg);
        // 失败路径：应产生 push 失败 事件，并标记 failed
        assert!(
            out.failed,
            "expect pipeline failed due to injected push failure"
        );
        assert!(out.has_event_prefix(EV_CLONE_COMPLETE));
        assert!(out.has_event_prefix(EV_PUSH_START));
        assert!(
            out.has_event_prefix(EV_PUSH_FAILED),
            "missing push failed event: {:?}",
            out.events
        );
        assert!(out.has_event_prefix(EV_FETCH_START));
        assert!(
            out.has_event_prefix(EV_FETCH_FAILED),
            "missing fetch failed event (remote url broken)"
        );
        // 字段读取（标记使用）
        assert!(out.remote_dir.is_some());
        assert!(out.commit_count_before.is_some());
        // fetch 失败时 commit_count_after 可能为 None
        let _delta = out.commit_delta();
    }

    // 成功路径：无故障，验证 push / fetch 成功事件，并使用 commit_delta。
    #[test]
    fn real_pipeline_success_path() {
        let cfg = PipelineConfig {
            remote_commits: 1,
            enable_real: true,
            faults: vec![],
        };
        let spec = PipelineSpec::builder()
            .clone_step()
            .modify()
            .commit()
            .push()
            .fetch()
            .build();
        let out = run_pipeline_with(&spec, &cfg);
        assert!(out.is_success(), "pipeline should succeed without faults");
        assert!(out.has_event_prefix(EV_PUSH_SUCCESS));
        assert!(out.has_event_prefix(EV_FETCH_COMPLETE));
        // commit_delta 在成功路径下应为 Some(>=0)
        if let Some(delta) = out.commit_delta() {
            assert!(delta >= 0);
        }
    }
}
