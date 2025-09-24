// 全局 dead_code 宽泛放行已移除：若有临时占位请在最小作用域内标注。
//! 公共测试工具聚合入口（Post refactor stabilized）
//!
//! 设计目标：
//! 1. 聚合高频测试构造/矩阵/DSL，避免在各主题测试文件中反复显式 `use crate::common::<module>`。
//! 2. 提供 **稳定的最小 re-export 前置层 (prelude)**，后续新增辅助函数在原模块内演进，不破坏现有引用。
//! 3. （已移除）init()/ensure_init() 兼容层，直接在需要处调用 `test_env::init_test_env()`。
//! 4. 保持向后兼容：原有 `pub mod <name>` 仍然导出，旧路径引用不受影响。
//! 5. 与《TESTS_REFACTOR_HANDOFF.md》文档保持一致：仅在确有 ≥2 文件重复时上移抽象。
//!
//! 后续演进（参考技术债）：
//! - 引入结构化事件枚举后，在 prelude 中追加 `StructuredEvent` / `EventTag` 枚举 re-export。
//! - 对 shallow/partial/fetch 对象计数 & capability 校验增加统一 helper。
//!
//! 使用方式：
//! ```rust
//! use crate::common::prelude::*;
//! 
//! #[test]
//! fn example() {
//!     test_env::init_test_env();
//!     let repo = fixtures::create_empty_repo();
//!     // ... proceed with test logic
//! }
//! ```
//!
//! 若仅需少量特定模块，可继续 `use crate::common::fixtures::create_empty_repo;`，无强制要求使用 prelude。

// ---- 子模块公开 ----
pub mod fixtures;
pub mod test_env;
pub mod repo_factory;
pub mod git_helpers;
pub mod git_scenarios;
pub mod event_assert;
pub mod shallow_matrix;        // 12.5: 浅克隆 / 深度矩阵
pub mod partial_filter_matrix; // 12.6: partial clone filter 矩阵
pub mod partial_filter_support; // 支撑能力判定
pub mod retry_matrix;          // 12.9: push & retry 矩阵
pub mod http_override_stub;    // 12.10: http override cases & stub
pub mod pipeline;              // 12.15: pipeline DSL (e2e scaffolding)
pub mod task_wait;             // 12.18: 任务等待辅助
pub mod i18n;                  // 12.19: 简化 i18n fixture/translate for tests

// （移除 TEST_COMMON_VERSION / init / ensure_init 兼容层）

// ---- 通用描述 Trait 与辅助断言 ----
/// 为各参数矩阵提供统一描述接口，用于：
/// * 参数化测试名称 / slug
/// * 去重校验（防止等价 case 重复）
/// * 日志/调试输出聚合
pub trait CaseDescribe { fn describe(&self) -> String; }

/// 验证一组实现 `CaseDescribe` 的 case 描述唯一性，并返回描述集合（便于后续使用）。
pub fn assert_unique_describe<T: CaseDescribe>(cases: &[T]) -> Vec<String> {
	use std::collections::HashSet;
	let mut set = HashSet::new();
	let mut out = Vec::with_capacity(cases.len());
	for c in cases { let d = c.describe(); assert!(set.insert(d.clone()), "duplicate describe(): {}", d); out.push(d); }
	out
}

/// 将描述转为 slug：保留字母数字与 - _，其它字符替换为 '-'; 多个连续 '-' 去重。
pub fn describe_slug<T: CaseDescribe>(c: &T) -> String {
	let raw = c.describe();
	let mut out = String::with_capacity(raw.len());
	let mut last_dash = false;
	for ch in raw.chars() {
		let keep = ch.is_ascii_alphanumeric() || ch=='-' || ch=='_';
		if keep { out.push(ch); last_dash = ch=='-'; }
		else {
			if !last_dash { out.push('-'); last_dash=true; }
		}
	}
	// 修剪首尾 '-'
	while out.starts_with('-') { out.remove(0); }
	while out.ends_with('-') { out.pop(); }
	out
}

// ---- Prelude：高频类型/函数再导出 ----
pub mod prelude {
	//! 常用测试工具集：单一 `use` 导入。
	//! 约定：仅 re-export "高频 + 稳定" API；不导出尚在重构中的低频细节。
	//! 增量新增保持向后兼容，不移除已存在条目。
	#![allow(unused_imports)] // re-export 供外部使用，当前 crate 内部未直接引用属预期；集中 suppress。

	// 基础初始化：直接使用 test_env::init_test_env(); 不再通过统一 wrapper。
	// Pipeline DSL re-export
	pub use super::pipeline::{PipelineSpec, PipelineBuilder, PipelineStepKind, PipelineConfig, FaultKind, run_pipeline, run_pipeline_with};

	// Fixtures
	pub use super::fixtures::{
		TestRepo,
		create_empty_repo,
		create_empty_dir,
		repo_with_staged,
		stage_files,
		write_files,
		path_slug,
	};
	// Repo factory helpers (rev_count moved here)
	pub use super::repo_factory::rev_count;

	// Git 场景 / Retry
	pub use super::git_scenarios::{
		GitOp,
		CloneParams, CloneOutcome, run_clone,
		FetchParams, FetchOutcome, run_fetch,
		PushRetrySpec, PushRetryOutcome, PushResultKind, run_push_with_retry,
	};
	pub use super::retry_matrix::{RetryCase, BackoffKind, PolicyOverride, compute_backoff_sequence, retry_cases};

	// 错误分类 assertions
	pub use super::git_helpers::{
		error_category,
		assert_err_category, expect_err_category,
		assert_err_in, expect_err_in, map_err_category,
	};

	// 事件断言 DSL
	pub use super::event_assert::{
		EventPhase, EventTag, default_tag_mapper, tagify, tagify_with,
		expect_subsequence, expect_tags_subsequence,
		assert_terminal_exclusive, assert_contains_phases, assert_last_phase_contains,
	};

	// Matrices
	pub use super::shallow_matrix::{ShallowCase, shallow_cases};
	pub use super::partial_filter_matrix::{
		PartialFilterOp, PartialFilterKind, PartialFilterCase,
		partial_filter_cases_for, clone_partial_filter_cases, fetch_partial_filter_cases, all_partial_filter_cases,
	};
	pub use super::partial_filter_support::{SupportLevel, PartialFilterOutcome, assess_partial_filter};

	// HTTP Override stub
	pub use super::http_override_stub::{
		HttpOverrideCase, FollowMode, IdempotentFlag, MaxEventsCase,
		http_override_cases, http_override_cases_for,
		run_http_override,
	};

	// 通用 Trait / helper
	pub use super::{CaseDescribe, assert_unique_describe, describe_slug};

	// i18n 简化工具（测试侧 fixture）
	pub use super::i18n::{translate, locale_keys};

	// 任务等待辅助
	pub use super::task_wait::wait_until_task_done;
}
