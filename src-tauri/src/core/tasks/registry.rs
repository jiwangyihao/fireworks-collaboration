use std::{collections::HashMap, sync::{Arc, Mutex}, time::SystemTime};
use tokio::{time::{sleep, Duration}, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::events::emitter::{emit_all, AppHandle};
use crate::events::structured::{publish_global, Event as StructuredEvent, PolicyEvent as StructuredPolicyEvent};
use super::retry::{compute_retry_diff, load_retry_plan};
use super::model::{TaskMeta, TaskKind, TaskState, TaskSnapshot, TaskStateEvent, TaskProgressEvent, TaskErrorEvent};
use super::retry::{backoff_delay_ms, is_retryable, categorize};
use crate::core::git::errors::GitError;
use crate::core::config::model::AppConfig;

const EV_STATE: &str = "task://state";
const EV_PROGRESS: &str = "task://progress";
const EV_ERROR: &str = "task://error";

pub struct TaskRegistry {
    inner: Mutex<HashMap<Uuid, TaskMeta>>,
    structured_bus: Mutex<Option<Arc<dyn crate::events::structured::EventBusAny>>>,
}

impl TaskRegistry {
    // T6: 已移除全部 legacy 策略类 TaskErrorEvent（applied/conflict/summary/fallback/adaptive/ignored_fields）。
    // 仅保留结构化 Strategy/Policy/Transport 事件；前端需消费结构化事件。
    pub fn new() -> Self { Self { inner: Mutex::new(HashMap::new()), structured_bus: Mutex::new(None) } }

    /// 测试/调用方可注入专用结构化事件总线（绕过全局/线程局部限制，便于捕获跨线程任务生命周期事件）
    pub fn inject_structured_bus(&self, bus: Arc<dyn crate::events::structured::EventBusAny>) {
        *self.structured_bus.lock().unwrap() = Some(bus);
    }


    fn publish_structured(&self, evt: crate::events::structured::Event) {
        if let Some(bus) = self.structured_bus.lock().unwrap().as_ref() {
            bus.publish(evt.clone());
        }
        crate::events::structured::publish_global(evt);
    }

    pub fn create(&self, kind: TaskKind) -> (Uuid, CancellationToken) {
        let id = Uuid::new_v4();
        let token = CancellationToken::new();
    let meta = TaskMeta { id, kind, state: TaskState::Pending, created_at: SystemTime::now(), cancel_token: token.clone(), fail_reason: None, lifecycle_flags: crate::core::tasks::model::LifecycleFlags::default() };
        self.inner.lock().unwrap().insert(id, meta);
        (id, token)
    }

    pub fn list(&self) -> Vec<TaskSnapshot> { self.inner.lock().unwrap().values().map(TaskSnapshot::from).collect() }
    pub fn snapshot(&self, id: &Uuid) -> Option<TaskSnapshot> { self.inner.lock().unwrap().get(id).map(TaskSnapshot::from) }
    pub fn cancel(&self, id: &Uuid) -> bool { self.inner.lock().unwrap().get(id).map(|m| { m.cancel_token.cancel(); true }).unwrap_or(false) }

    fn with_meta<F: FnOnce(&mut TaskMeta)>(&self, id: &Uuid, f: F) -> Option<TaskMeta> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(m) = guard.get_mut(id) { f(m); Some(m.clone()) } else { None }
    }
    fn emit_state(&self, app:&AppHandle, id:&Uuid) { if let Some(m) = self.inner.lock().unwrap().get(id) { let evt = TaskStateEvent::new(m); emit_all(app, EV_STATE, &evt); } }
    fn set_state_emit(&self, app:&AppHandle, id:&Uuid, s:TaskState){
        if self.with_meta(id, |m| m.state = s).is_some(){ self.emit_state(app, id); }
    }
    fn set_state_noemit(&self, id:&Uuid, s:TaskState){ let _ = self.with_meta(id, |m| m.state = s); }
    fn emit_error_structured(&self, evt:&TaskErrorEvent){
        use crate::events::structured::{Event as StructuredEvent, TaskEvent as StructuredTaskEvent};
        self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Failed { id: evt.task_id.to_string(), category: evt.category.clone(), code: evt.code.clone(), message: evt.message.clone() }));
        // 标记 failed 已发送（幂等）
        let _ = self.with_meta(&evt.task_id, |m| { m.fail_reason = Some(evt.message.clone()); if !m.lifecycle_flags.failed { m.lifecycle_flags.failed = true; } });
    }
    /// 幂等生命周期事件发布
    fn publish_lifecycle_started(&self, id:&Uuid, kind:&str){
        use crate::events::structured::{Event as StructuredEvent, TaskEvent as StructuredTaskEvent};
        let mut should = false;
        let kind_owned = kind.to_string();
        if let Some(_) = self.with_meta(id, |m| { if !m.lifecycle_flags.started { m.lifecycle_flags.started=true; should=true; } }) { if should { self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Started { id:id.to_string(), kind: kind_owned })); } }
    }
    fn publish_lifecycle_completed(&self, id:&Uuid){
        use crate::events::structured::{Event as StructuredEvent, TaskEvent as StructuredTaskEvent};
        let mut should=false; if let Some(_) = self.with_meta(id, |m| { if !m.lifecycle_flags.completed { m.lifecycle_flags.completed=true; should=true; } }) { if should { self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Completed { id:id.to_string() })); } }
    }
    fn publish_lifecycle_canceled(&self, id:&Uuid){
        use crate::events::structured::{Event as StructuredEvent, TaskEvent as StructuredTaskEvent};
        let mut should=false; if let Some(_) = self.with_meta(id, |m| { if !m.lifecycle_flags.canceled { m.lifecycle_flags.canceled=true; should=true; } }) { if should { self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Canceled { id:id.to_string() })); } }
    }
    fn publish_lifecycle_failed_if_needed(&self, id:&Uuid, message:&str){
        // 如果 error 事件已经触发 failed 标记则跳过
        use crate::events::structured::{Event as StructuredEvent, TaskEvent as StructuredTaskEvent};
        let mut should=false; if let Some(_) = self.with_meta(id, |m| { if !m.lifecycle_flags.failed { m.lifecycle_flags.failed=true; should=true; } }) { if should { self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Failed { id:id.to_string(), category: "Unknown".into(), code: None, message: message.to_string() })); } }
    }
    fn emit_error(&self, app:&AppHandle, evt:&TaskErrorEvent) { emit_all(app, EV_ERROR, evt); self.emit_error_structured(evt); }

    pub fn spawn_sleep_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, total_ms: u64) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            this.publish_lifecycle_started(&id, "Sleep");
            let step = 50u64; // 更细颗粒度便于测试
            let mut elapsed = 0u64;
            while elapsed < total_ms {
                if token.is_cancelled(){ match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled)}; this.publish_lifecycle_canceled(&id); return; }
                sleep(Duration::from_millis(step)).await;
                elapsed += step;
                if let Some(app_ref) = &app {
                    let percent = ((elapsed.min(total_ms) as f64 / total_ms as f64) * 100.0) as u32;
                    let prog = TaskProgressEvent { task_id: id, kind: "Sleep".into(), phase: "Running".into(), percent, objects: None, bytes: None, total_hint: None, retried_times: None };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }
            }
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }
            this.publish_lifecycle_completed(&id);
        })
    }

    /// 启动 Git Clone 任务（阻塞线程执行），支持取消与基本进度事件
    /// Decide whether to emit a partial filter fallback event.
    /// Placeholder capability model: always unsupported for now. Once real capability
    /// detection is available we plug it in here. Returns the optional (message, shallow_mode)
    /// where shallow_mode=true means depth retained (depth+filter) and false means full.
    pub fn decide_partial_fallback(depth_applied: Option<u32>, filter_requested: Option<&str>, capability_supported: bool) -> Option<(String, bool)> {
        if filter_requested.is_none() { return None; }
        if capability_supported { return None; }
        let shallow = depth_applied.is_some();
        let msg = if shallow { "partial filter unsupported; fallback=shallow (depth retained)".to_string() } else { "partial filter unsupported; fallback=full".to_string() };
        Some((msg, shallow))
    }

    fn runtime_config() -> AppConfig {
        // 尝试加载持久化配置；失败时回退默认并应用轻量环境变量覆盖。
        let mut cfg = crate::core::config::loader::load_or_init().unwrap_or_else(|_| AppConfig::default());
        if let Ok(v) = std::env::var("FWC_PARTIAL_FILTER_SUPPORTED") { if v == "1" { cfg.partial_filter_supported = true; } }
    // Backward compatibility: older tests / env may use FWC_PARTIAL_FILTER_CAPABLE (treat identically)
    if let Ok(v) = std::env::var("FWC_PARTIAL_FILTER_CAPABLE") { if v == "1" { cfg.partial_filter_supported = true; } }
        cfg
    }

    // T6: 移除所有 legacy gating（FWC_STRATEGY_APPLIED_EVENTS / FWC_LEGACY_STRATEGY_EVENTS）。结构化事件成为唯一来源。

    fn emit_strategy_summary(_app:&Option<AppHandle>, id:Uuid, kind:&str, http:(bool,u8), retry:&super::retry::RetryPlan, tls:(bool,bool), codes:Vec<String>, has_filter:bool) {
        // 仅发送结构化 summary 事件
        crate::events::structured::publish_global(
            crate::events::structured::Event::Strategy(
                crate::events::structured::StrategyEvent::Summary {
                    id: id.to_string(),
                    kind: kind.to_string(),
                    http_follow: http.0,
                    http_max: http.1,
                    retry_max: retry.max,
                    retry_base_ms: retry.base_ms,
                    retry_factor: retry.factor,
                    retry_jitter: retry.jitter,
                    tls_insecure: tls.0,
                    tls_skip_san: tls.1,
                    applied_codes: codes.clone(),
                    filter_requested: has_filter,
                }
            )
        );
    }

    /// Merge HTTP strategy overrides (P2.3b). The override model is parsed earlier (parse_stage == clone/fetch depth/filter parsing, or push parse).
    /// We pass in the global AppConfig (cloned) and return (effective_follow_redirects, effective_max_redirects).
    /// Rules:
    /// 1. Start with global config values.
    /// 2. If override.follow_redirects is Some -> replace.
    /// 3. If override.max_redirects is Some -> clamp to [0, 20] (parse layer already restricted upper bound) and replace.
    /// 4. Log once if any value actually changed, include task kind & id for traceability.
    /// 公共暴露（测试/属性验证使用）：计算 HTTP 覆盖合并 & 冲突归一化结果。
    pub fn apply_http_override(kind: &str, id: &Uuid, global: &AppConfig, override_http: Option<&crate::core::git::default_impl::opts::StrategyHttpOverride>) -> (bool, u8, bool, Option<String>) {
        let mut follow = global.http.follow_redirects;
        let mut max_r = global.http.max_redirects;
        let mut changed = false;
        let mut conflict: Option<String> = None;
        if let Some(o) = override_http {
            if let Some(f) = o.follow_redirects { if f != follow { follow = f; changed = true; } }
            if let Some(m) = o.max_redirects { let m_clamped = (m.min(20)) as u8; if m_clamped != max_r { max_r = m_clamped; changed = true; } }
            // 冲突规则：follow=false 且 max_redirects>0 归一化为 max=0
            if follow == false && max_r > 0 { conflict = Some(format!("followRedirects=false => force maxRedirects=0 (was {})", max_r)); if max_r != 0 { max_r = 0; /* 归一化 */ changed = true; } }
        }
        if changed { tracing::info!(target="strategy", task_kind=%kind, task_id=%id, follow_redirects=%follow, max_redirects=%max_r, "http override applied"); }
        (follow, max_r, changed, conflict)
    }

    /// Merge Retry strategy overrides (P2.3c).
    /// Rules:
    /// 1. Start with global retry config (loaded via config loader earlier by caller or default()).
    /// 2. Each provided field (max/base_ms/factor/jitter) replaces the corresponding value.
    /// 3. Validation (range) 已在 parse 阶段完成 (opts.rs)；此处只做简单 copy。
    /// 4. Return (RetryPlan, changed_flag). If changed=true caller may emit informational event with code=retry_strategy_override_applied.
    /// 公共暴露（测试/属性验证使用）：计算 Retry 覆盖结果与是否有改动。
    pub fn apply_retry_override(global: &crate::core::config::model::RetryCfg, override_retry: Option<&crate::core::git::default_impl::opts::StrategyRetryOverride>) -> (super::retry::RetryPlan, bool) {
        let mut plan: super::retry::RetryPlan = global.clone().into();
        let mut changed = false;
        if let Some(o) = override_retry {
            if let Some(m) = o.max { if m != plan.max { plan.max = m; changed = true; } }
            if let Some(b) = o.base_ms { if b as u64 != plan.base_ms { plan.base_ms = b as u64; changed = true; } }
            if let Some(f) = o.factor { if (f as f64) != plan.factor { plan.factor = f as f64; changed = true; } }
            if let Some(j) = o.jitter { if j != plan.jitter { plan.jitter = j; changed = true; } }
        }
        if changed { tracing::info!(target="strategy", retry_max=plan.max, retry_base_ms=plan.base_ms, retry_factor=plan.factor, retry_jitter=plan.jitter, "retry override applied"); }
        (plan, changed)
    }

    /// Merge TLS strategy overrides (P2.3d).
    /// Rules:
    /// 1. Start from global tls config (san_whitelist is NOT overridden per-task for safety; only boolean toggles allowed).
    /// 2. If insecure_skip_verify provided and different -> replace.
    /// 3. If skip_san_whitelist provided and different -> replace.
    /// 4. Return (insecure_skip_verify, skip_san_whitelist, changed_flag). Caller may emit informational event code=tls_strategy_override_applied when changed.
    /// 公共暴露（测试/属性验证使用）：计算 TLS 覆盖合并 & 冲突归一化结果。
    pub fn apply_tls_override(kind:&str, id:&Uuid, global:&AppConfig, override_tls: Option<&crate::core::git::default_impl::opts::StrategyTlsOverride>) -> (bool, bool, bool, Option<String>) {
        let mut insecure = global.tls.insecure_skip_verify;
        let mut skip_san = global.tls.skip_san_whitelist;
        let mut changed = false;
        let mut conflict: Option<String> = None;
        if let Some(o) = override_tls {
            if let Some(v) = o.insecure_skip_verify { if v != insecure { insecure = v; changed = true; } }
            if let Some(v) = o.skip_san_whitelist { if v != skip_san { skip_san = v; changed = true; } }
            // 冲突规则：insecure=true 时跳过 SAN 白名单意义弱化，规范化 skipSanWhitelist=false
            if insecure && skip_san { conflict = Some("insecureSkipVerify=true normalizes skipSanWhitelist=false".to_string()); if skip_san { skip_san = false; changed = true; }
            }
        }
        if changed { tracing::info!(target="strategy", task_kind=%kind, task_id=%id, insecure_skip_verify=%insecure, skip_san_whitelist=%skip_san, "tls override applied"); }
        (insecure, skip_san, changed, conflict)
    }

    

    pub fn spawn_git_clone_task_with_opts(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, depth: Option<serde_json::Value>, filter: Option<String>, strategy_override: Option<serde_json::Value>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            fn emit_adaptive_tls_timing(id:Uuid, kind:&str){
                use crate::core::git::transport::{tl_snapshot, metrics_enabled};
                use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
                if !metrics_enabled() { return; }
                let snap = tl_snapshot();
                if let Some(t) = snap.timing {
                    publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::AdaptiveTlsTiming { id: id.to_string(), kind: kind.to_string(), used_fake_sni: snap.used_fake.unwrap_or(false), fallback_stage: snap.fallback_stage.unwrap_or("Unknown").to_string(), connect_ms: t.connect_ms, tls_ms: t.tls_ms, first_byte_ms: t.first_byte_ms, total_ms: t.total_ms, cert_fp_changed: snap.cert_fp_changed.unwrap_or(false) }));
                }
            }
            use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitClone");


            // 预发一个开始事件
            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            // 为测试与前端观察提供一个极短缓冲，确保能够看到 Running 状态
            std::thread::sleep(std::time::Duration::from_millis(50));

            // 提前检查取消
            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitClone", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }

            // 参数解析：depth 已在 P2.2b 生效；filter 在 P2.2d 引入占位（当前不真正启用 partial，进行回退提示）
            let parsed_options_res = crate::core::git::default_impl::opts::parse_depth_filter_opts(depth.clone(), filter.clone(), strategy_override.clone());
            // For strategyOverride.http application we need the global config; currently AppConfig is only available in tauri command layer.
            // P2.3b: we don't mutate global config; overrides are per-task ephemeral. For clone/fetch we only log effective values.
            let global_cfg = Self::runtime_config();
            let mut effective_follow_redirects: bool = global_cfg.http.follow_redirects;
            let mut effective_max_redirects: u8 = global_cfg.http.max_redirects;
            let mut retry_plan: super::retry::RetryPlan = global_cfg.retry.clone().into();
            let mut depth_applied: Option<u32> = None;
            let mut effective_insecure_skip_verify: bool = global_cfg.tls.insecure_skip_verify;
            let mut effective_skip_san_whitelist: bool = global_cfg.tls.skip_san_whitelist;
            let mut filter_requested: Option<String> = None; // 记录用户请求的 filter（用于回退信息）
            let mut applied_codes: Vec<String> = vec![];
            if let Err(e) = parsed_options_res {
                // 如果是 unsupported filter 错误，发布结构化 unsupported 事件（即使任务失败）
                let msg_string = e.to_string();
                if msg_string.contains("unsupported filter:") {
                    publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterUnsupported { id: id.to_string(), requested: msg_string.clone() }));
                }
                // 直接作为 Protocol/错误分类失败
                if let Some(app_ref) = &app { let err_evt = TaskErrorEvent::from_parts(id, "GitClone", super::retry::categorize(&e), format!("{}", e), None); this.emit_error(app_ref, &err_evt); }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                return;
            } else {
                if let Some(opts) = parsed_options_res.ok() {
                    // 解析成功若用户请求了 filter，发布 capability 事件（supported 由全局配置判定）
                    if opts.filter.is_some() {
                        publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterCapability { id: id.to_string(), supported: global_cfg.partial_filter_supported }));
                    }
                    // P2.3e: emit ignored fields informational event (once) if any unknown keys present
                        if !opts.ignored_top_level.is_empty() || !opts.ignored_nested.is_empty() {
                        publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::IgnoredFields { id: id.to_string(), kind: "GitClone".into(), top_level: opts.ignored_top_level.clone(), nested: opts.ignored_nested.iter().map(|(s,k)| format!("{}.{k}", s)).collect() }));
                    }
                    depth_applied = opts.depth;
                    if let Some(f) = opts.filter.as_ref() { filter_requested = Some(f.as_str().to_string()); }
                    if let Some(http_over) = opts.strategy_override.as_ref().and_then(|s| s.http.as_ref()) {
                        let (f, m, changed, conflict) = Self::apply_http_override("GitClone", &id, &global_cfg, Some(http_over));
                        effective_follow_redirects = f; effective_max_redirects = m;
                        if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::HttpApplied { id: id.to_string(), follow: f, max_redirects: m })); applied_codes.push("http_strategy_override_applied".into()); }
                        if let Some(conflict_msg) = conflict { publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::Conflict { id: id.to_string(), kind: "http".into(), message: conflict_msg })); }
                    }
                    if let Some(tls_over) = opts.strategy_override.as_ref().and_then(|s| s.tls.as_ref()) {
                        let (ins, skip, changed, conflict) = Self::apply_tls_override("GitClone", &id, &global_cfg, Some(tls_over));
                        effective_insecure_skip_verify = ins; effective_skip_san_whitelist = skip;
                        if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::TlsApplied { id: id.to_string(), insecure_skip_verify: ins, skip_san_whitelist: skip })); applied_codes.push("tls_strategy_override_applied".into()); }
                        if let Some(conflict_msg)=conflict { publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::Conflict { id: id.to_string(), kind: "tls".into(), message: conflict_msg })); }
                    }
                    if let Some(retry_over) = opts.strategy_override.as_ref().and_then(|s| s.retry.as_ref()) {
                        let (plan, changed) = Self::apply_retry_override(&global_cfg.retry, Some(retry_over));
                        retry_plan = plan;
                        if changed { let base_plan = load_retry_plan(); let (diff, _) = compute_retry_diff(&base_plan, &retry_plan); publish_global(StructuredEvent::Policy(StructuredPolicyEvent::RetryApplied { id: id.to_string(), code: "retry_strategy_override_applied".to_string(), changed: diff.changed.into_iter().map(|s| s.to_string()).collect() })); applied_codes.push("retry_strategy_override_applied".into()); }
                    }
                    tracing::info!(target="git", depth=?opts.depth, filter=?opts.filter.as_ref().map(|f| f.as_str()), has_strategy=?opts.strategy_override.is_some(), strategy_http_follow=?effective_follow_redirects, strategy_http_max_redirects=?effective_max_redirects, strategy_tls_insecure=?effective_insecure_skip_verify, strategy_tls_skip_san=?effective_skip_san_whitelist, "git_clone options accepted (depth/filter/strategy parsed)");
                    if let Some((_, shallow)) = Self::decide_partial_fallback(depth_applied, filter_requested.as_deref(), global_cfg.partial_filter_supported) { publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterFallback { id: id.to_string(), shallow, message: "partial_filter_fallback".into() })); }
                    // 汇总事件（无条件发送，便于前端一次性展示）
                    Self::emit_strategy_summary(&app, id, "GitClone", (effective_follow_redirects, effective_max_redirects), &retry_plan, (effective_insecure_skip_verify, effective_skip_san_whitelist), applied_codes.clone(), filter_requested.is_some());
                    // 自适应 TLS rollout 事件（仅当改写发生说明命中采样）
                    if let Some(rewritten) = crate::core::git::transport::maybe_rewrite_https_to_custom(&global_cfg, repo.as_str()) { let _ = rewritten; let percent = global_cfg.http.fake_sni_rollout_percent; publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::AdaptiveTlsRollout { id: id.to_string(), kind: "GitClone".into(), percent_applied: percent as u8, sampled: true })); }
                }
            }

            // Use per-task retry plan (may be overridden)
            let plan = retry_plan.clone();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() { match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }; this.publish_lifecycle_canceled(&id); break; }

                // per-attempt interrupt flag and watcher
                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() {
                        interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });

                // 执行克隆
                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    service
                        .clone_blocking(
                            repo.as_str(),
                            &dest_path,
                            depth_applied,
                            &*interrupt_flag,
                            move |p| {
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitClone", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app {
                            let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None };
                            emit_all(app_ref, EV_PROGRESS, &prog);
                        }
                        emit_adaptive_tls_timing(id, "GitClone");
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        this.publish_lifecycle_completed(&id);
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "clone error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitClone", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        if is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            emit_adaptive_tls_timing(id, "GitClone");
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => { this.set_state_noemit(&id, TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    /// 启动 Git Fetch 任务（阻塞线程执行），支持取消与基本进度事件
    pub fn spawn_git_fetch_task_with_opts(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, preset: Option<String>, depth: Option<serde_json::Value>, filter: Option<String>, strategy_override: Option<serde_json::Value>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            fn emit_adaptive_tls_timing(id:Uuid, kind:&str){
                use crate::core::git::transport::{tl_snapshot, metrics_enabled};
                use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
                if !metrics_enabled() { return; }
                let snap = tl_snapshot();
                if let Some(t) = snap.timing { publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::AdaptiveTlsTiming { id: id.to_string(), kind: kind.to_string(), used_fake_sni: snap.used_fake.unwrap_or(false), fallback_stage: snap.fallback_stage.unwrap_or("Unknown").to_string(), connect_ms: t.connect_ms, tls_ms: t.tls_ms, first_byte_ms: t.first_byte_ms, total_ms: t.total_ms, cert_fp_changed: snap.cert_fp_changed.unwrap_or(false) })); }
            }
            let _ = &preset; // 目前 git2 路径未使用该预设参数
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitFetch");

            // 预发一个开始事件
            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitFetch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }

            // 解析与校验（P2.2a+c）；P2.2e：若用户请求 filter（partial fetch 尚未真正启用）发送非阻断回退事件
            let parsed_options_res = crate::core::git::default_impl::opts::parse_depth_filter_opts(depth.clone(), filter.clone(), strategy_override.clone());
            let global_cfg = Self::runtime_config();
            let mut effective_follow_redirects: bool = global_cfg.http.follow_redirects;
            let mut effective_max_redirects: u8 = global_cfg.http.max_redirects;
            let mut retry_plan: super::retry::RetryPlan = global_cfg.retry.clone().into();
            let mut depth_applied: Option<u32> = None;
            let mut effective_insecure_skip_verify: bool = global_cfg.tls.insecure_skip_verify;
            let mut effective_skip_san_whitelist: bool = global_cfg.tls.skip_san_whitelist;
            let mut filter_requested: Option<String> = None;
            let mut applied_codes: Vec<String> = vec![];
            // applied events legacy gating removed; structured events always emitted
            if let Err(e) = parsed_options_res {
                let msg_string = e.to_string();
                if msg_string.contains("unsupported filter:") {
                    publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterUnsupported { id: id.to_string(), requested: msg_string.clone() }));
                }
                if let Some(app_ref) = &app { let err_evt = TaskErrorEvent::from_parts(id, "GitFetch", super::retry::categorize(&e), format!("{}", e), None); this.emit_error(app_ref, &err_evt); }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                this.emit_error_structured(&TaskErrorEvent { task_id:id, kind:"GitFetch".into(), category:"Runtime".into(), code:Some("fetch_failed".into()), message: format!("fatal: {}", e), retried_times:None });
                return;
            } else if let Ok(opts) = parsed_options_res.as_ref() {
                if opts.filter.is_some() {
                    publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterCapability { id: id.to_string(), supported: global_cfg.partial_filter_supported }));
                }
                if !opts.ignored_top_level.is_empty() || !opts.ignored_nested.is_empty() { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::IgnoredFields { id: id.to_string(), kind: "GitFetch".into(), top_level: opts.ignored_top_level.clone(), nested: opts.ignored_nested.iter().map(|(s,k)| format!("{}.{k}", s)).collect() })); }
                depth_applied = opts.depth; // P2.2c: depth now effective
                if let Some(f) = opts.filter.as_ref() { filter_requested = Some(f.as_str().to_string()); }
                if let Some(http_over) = opts.strategy_override.as_ref().and_then(|s| s.http.as_ref()) {
                    let (f, m, changed, conflict) = Self::apply_http_override("GitFetch", &id, &global_cfg, Some(http_over));
                    effective_follow_redirects = f; effective_max_redirects = m;
                    if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::HttpApplied { id: id.to_string(), follow: f, max_redirects: m })); applied_codes.push("http_strategy_override_applied".into()); }
                        if let Some(_)=conflict { /* fetch http conflict 已通过 clone 路径验证，这里不重复发 structured 冲突事件 */ }
                }
                if let Some(tls_over) = opts.strategy_override.as_ref().and_then(|s| s.tls.as_ref()) {
                    let (ins, skip, changed, conflict) = Self::apply_tls_override("GitFetch", &id, &global_cfg, Some(tls_over));
                    effective_insecure_skip_verify = ins; effective_skip_san_whitelist = skip;
                    if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::TlsApplied { id: id.to_string(), insecure_skip_verify: ins, skip_san_whitelist: skip })); applied_codes.push("tls_strategy_override_applied".into()); }
                    if let Some(_)=conflict { /* 冲突已被 clone 逻辑覆盖示例；此处简化不再重复 */ }
                }
                if let Some(retry_over) = opts.strategy_override.as_ref().and_then(|s| s.retry.as_ref()) {
                    let (plan, changed) = Self::apply_retry_override(&global_cfg.retry, Some(retry_over));
                    retry_plan = plan;
                    if changed { applied_codes.push("retry_strategy_override_applied".into()); }
                }
                tracing::info!(target="git", depth=?opts.depth, filter=?opts.filter.as_ref().map(|f| f.as_str()), has_strategy=?opts.strategy_override.is_some(), strategy_http_follow=?effective_follow_redirects, strategy_http_max_redirects=?effective_max_redirects, strategy_tls_insecure=?effective_insecure_skip_verify, strategy_tls_skip_san=?effective_skip_san_whitelist, "git_fetch options accepted (depth/filter/strategy parsed)");
                if let Some((_, shallow)) = Self::decide_partial_fallback(depth_applied, filter_requested.as_deref(), global_cfg.partial_filter_supported) { publish_global(crate::events::structured::Event::Transport(crate::events::structured::TransportEvent::PartialFilterFallback { id: id.to_string(), shallow, message: "partial_filter_fallback".into() })); }
                Self::emit_strategy_summary(&app, id, "GitFetch", (effective_follow_redirects, effective_max_redirects), &retry_plan, (effective_insecure_skip_verify, effective_skip_san_whitelist), applied_codes.clone(), filter_requested.is_some());
                if let Some(rewritten) = crate::core::git::transport::maybe_rewrite_https_to_custom(&global_cfg, repo.as_str()) { let _ = rewritten; let percent = global_cfg.http.fake_sni_rollout_percent; publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::AdaptiveTlsRollout { id: id.to_string(), kind: "GitFetch".into(), percent_applied: percent as u8, sampled: true })); }
            }

            let plan = retry_plan.clone();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() {
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    break;
                }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() { interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed); }
                });

                // 进入阶段性进度
                if let Some(app_ref) = &app {
                    let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Fetching".into(), percent: 10, objects: None, bytes: None, total_hint: None, retried_times: None };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    service
                        .fetch_blocking(
                            repo.as_str(),
                            &dest_path,
                            depth_applied,
                            &*interrupt_flag,
                            move |p| {
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitFetch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app { let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                        emit_adaptive_tls_timing(id, "GitFetch");
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "fetch error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitFetch", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        if is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            emit_adaptive_tls_timing(id, "GitFetch");
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    // Backward compatible wrappers (P2.2a): tests and existing callers without new params
    pub fn spawn_git_clone_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String) -> JoinHandle<()> {
        self.spawn_git_clone_task_with_opts(app, id, token, repo, dest, None, None, None)
    }

    pub fn spawn_git_fetch_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, preset: Option<String>) -> JoinHandle<()> {
        self.spawn_git_fetch_task_with_opts(app, id, token, repo, dest, preset, None, None, None)
    }

    /// 启动 Git Push 任务（阻塞线程执行），支持取消与阶段事件
    pub fn spawn_git_push_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, remote: Option<String>, refspecs: Option<Vec<String>>, username: Option<String>, password: Option<String>, strategy_override: Option<serde_json::Value>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            fn emit_adaptive_tls_timing(id:Uuid, kind:&str){
                use crate::core::git::transport::{tl_snapshot, metrics_enabled};
                use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
                if !metrics_enabled() { return; }
                let snap = tl_snapshot();
                if let Some(t) = snap.timing { publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::AdaptiveTlsTiming { id: id.to_string(), kind: kind.to_string(), used_fake_sni: snap.used_fake.unwrap_or(false), fallback_stage: snap.fallback_stage.unwrap_or("Unknown").to_string(), connect_ms: t.connect_ms, tls_ms: t.tls_ms, first_byte_ms: t.first_byte_ms, total_ms: t.total_ms, cert_fp_changed: snap.cert_fp_changed.unwrap_or(false) })); }
            }
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitPush");

            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitPush", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }

            // P2.3a: parse strategyOverride early (depth/filter not applicable for push). If invalid => Protocol Fail.
            let mut effective_follow_redirects = None;
            let mut effective_max_redirects = None;
            // 统一初始化重试计划（即使没有 retry override 也使用全局 plan，保证 summary 一定发出）
            let mut retry_plan: super::retry::RetryPlan = Self::runtime_config().retry.clone().into();
            let mut effective_insecure_skip_verify: Option<bool> = None;
            let mut effective_skip_san_whitelist: Option<bool> = None;
            let mut applied_codes: Vec<String> = vec![];
            // applied events legacy gating removed; structured events always emitted
            // parse strategy_override (push 专用)；无论是否存在都需最终 summary
            if let Some(raw) = strategy_override.clone() {
                use crate::core::git::default_impl::opts::parse_strategy_override;
                match parse_strategy_override(Some(raw)) {
                    Err(e) => {
                    if let Some(app_ref) = &app { let err_evt = TaskErrorEvent::from_parts(id, "GitPush", super::retry::categorize(&e), format!("{}", e), None); this.emit_error(app_ref, &err_evt); }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                    return;
                }
                    Ok(parsed_res) => {
                        // P2.3e ignored fields event
                        if !parsed_res.ignored_top_level.is_empty() || !parsed_res.ignored_nested.is_empty() { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::IgnoredFields { id: id.to_string(), kind: "GitPush".into(), top_level: parsed_res.ignored_top_level.clone(), nested: parsed_res.ignored_nested.iter().map(|(s,k)| format!("{}.{k}", s)).collect() })); }
                        if let Some(parsed) = parsed_res.parsed {
                            let global_cfg = Self::runtime_config();
                            if let Some(http_over) = parsed.http.as_ref() {
                                let (f,m,changed, conflict) = Self::apply_http_override("GitPush", &id, &global_cfg, Some(http_over));
                                effective_follow_redirects = Some(f);
                                effective_max_redirects = Some(m);
                                if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::HttpApplied { id: id.to_string(), follow: f, max_redirects: m })); applied_codes.push("http_strategy_override_applied".into()); }
                                if let Some(msg)=conflict { if let Some(app_ref)=&app { let evt = TaskErrorEvent { task_id:id, kind:"GitPush".into(), category:"Protocol".into(), code:Some("strategy_override_conflict".into()), message: format!("http conflict: {}", msg), retried_times:None }; this.emit_error(app_ref,&evt);} }
                            }
                            if let Some(tls_over) = parsed.tls.as_ref() {
                                let (ins, skip, changed, conflict) = Self::apply_tls_override("GitPush", &id, &global_cfg, Some(tls_over));
                                effective_insecure_skip_verify = Some(ins);
                                effective_skip_san_whitelist = Some(skip);
                                if changed { publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::TlsApplied { id: id.to_string(), insecure_skip_verify: ins, skip_san_whitelist: skip })); applied_codes.push("tls_strategy_override_applied".into()); }
                                if let Some(msg)=conflict { if let Some(app_ref)=&app { let evt = TaskErrorEvent { task_id:id, kind:"GitPush".into(), category:"Protocol".into(), code:Some("strategy_override_conflict".into()), message: format!("tls conflict: {}", msg), retried_times:None }; this.emit_error(app_ref,&evt);} }
                            }
                            if let Some(retry_over) = parsed.retry.as_ref() {
                                let (plan_new, changed) = Self::apply_retry_override(&global_cfg.retry, Some(retry_over));
                                if changed { let base_plan = load_retry_plan(); let (diff, _) = compute_retry_diff(&base_plan, &plan_new); publish_global(StructuredEvent::Policy(StructuredPolicyEvent::RetryApplied { id: id.to_string(), code: "retry_strategy_override_applied".to_string(), changed: diff.changed.into_iter().map(|s| s.to_string()).collect() })); applied_codes.push("retry_strategy_override_applied".into()); }
                                retry_plan = plan_new;
                            }
                            tracing::info!(target="strategy", kind="push", has_override=true, http_follow=?effective_follow_redirects, http_max_redirects=?effective_max_redirects, tls_insecure=?effective_insecure_skip_verify, tls_skip_san=?effective_skip_san_whitelist, strategy_override_valid=true, "strategyOverride accepted for push (parse+http/tls apply)");
                            // summary 延后到统一位置发射
                        } else {
                            tracing::info!(target="strategy", kind="push", has_override=true, http_follow=?effective_follow_redirects, http_max_redirects=?effective_max_redirects, tls_insecure=?effective_insecure_skip_verify, tls_skip_san=?effective_skip_san_whitelist, strategy_override_valid=true, "strategyOverride accepted for push (empty object)");
                        }
                    }
                }
            }

            // 若没有 override，对 effective_* 采用全局默认；然后统一发 summary（即使没有任何 appliedCodes）
            tracing::debug!(target="strategy", kind="push", task_id=%id, applied_codes=?applied_codes, "emit push strategy summary");
            let global_after = Self::runtime_config();
            let eff_follow = effective_follow_redirects.unwrap_or(global_after.http.follow_redirects);
            let eff_max = effective_max_redirects.unwrap_or(global_after.http.max_redirects);
            let eff_insecure = effective_insecure_skip_verify.unwrap_or(global_after.tls.insecure_skip_verify);
            let eff_skip = effective_skip_san_whitelist.unwrap_or(global_after.tls.skip_san_whitelist);
            // 去重（防止将来多次同类型 override 重复记录）
            let mut dedup_codes = applied_codes.clone();
            dedup_codes.sort();
            dedup_codes.dedup();
            Self::emit_strategy_summary(&app, id, "GitPush", (eff_follow, eff_max), &retry_plan, (eff_insecure, eff_skip), dedup_codes, false);
            // push adaptive rollout
            if let Some(rewritten) = crate::core::git::transport::maybe_rewrite_https_to_custom(&global_after, dest.as_str()) { let _=rewritten; publish_global(crate::events::structured::Event::Strategy(crate::events::structured::StrategyEvent::AdaptiveTlsRollout { id: id.to_string(), kind: "GitPush".into(), percent_applied: global_after.http.fake_sni_rollout_percent as u8, sampled: true })); }

            let plan = retry_plan; // 已确保存在
            let mut attempt: u32 = 0;
            // 用于检测是否进入上传阶段（进入后不再自动重试）
            let upload_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            loop {
                if token.is_cancelled() { match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }; this.publish_lifecycle_canceled(&id); break; }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                upload_started.store(false, std::sync::atomic::Ordering::Relaxed);
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() { interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed); }
                });

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    let upload_started_cb = std::sync::Arc::clone(&upload_started);
                    // 允许仅提供 token（password）；若 username 为空或缺失，默认使用 "x-access-token"
                    let creds_opt = match (username.as_deref(), password.as_deref()) {
                        (Some(u), Some(p)) if !u.is_empty() => Some((u, p)),
                        (None, Some(p)) => Some(("x-access-token", p)),
                        (Some(u), Some(p)) if u.is_empty() => Some(("x-access-token", p)),
                        _ => None
                    };
                    let refspecs_vec: Option<Vec<String>> = refspecs.clone();
                    let refspecs_opt: Option<Vec<String>> = refspecs_vec;
                    let refspecs_slices: Option<Vec<&str>> = refspecs_opt.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());
                    service
                        .push_blocking(
                            &dest_path,
                            remote.as_deref(),
                            refspecs_slices.as_deref(),
                            creds_opt.map(|(u,p)| (u, p)),
                            &*interrupt_flag,
                            move |p| {
                                if p.phase == "Upload" { upload_started_cb.store(true, std::sync::atomic::Ordering::Relaxed); }
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitPush", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    this.publish_lifecycle_canceled(&id);
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app { let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                        emit_adaptive_tls_timing(id, "GitPush");
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        this.publish_lifecycle_completed(&id);
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "push error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitPush", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        // 仅在尚未进入上传阶段时允许自动重试
                        if !upload_started.load(std::sync::atomic::Ordering::Relaxed) && is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            emit_adaptive_tls_timing(id, "GitPush");
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => { this.set_state_noemit(&id, TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    /// 启动 Git Init 任务：初始化一个新的仓库
    pub fn spawn_git_init_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitInit");
            if token.is_cancelled() { // 取消早退
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitInit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::init::git_init(&dest_path, &*interrupt_flag, move |_p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: "GitInit".into(), phase: "Running".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitInit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitInit", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } }
                }
            }
        })
    }

    /// 启动 Git Add 任务：暂存文件
    pub fn spawn_git_add_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, paths: Vec<String>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitAdd");
            if token.is_cancelled() {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let path_vec = paths.clone();
            let ref_slices: Vec<&str> = path_vec.iter().map(|s| s.as_str()).collect();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::add::git_add(&dest_path, &ref_slices, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitAdd", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } }
                }
            }
        })
    }

    /// 启动 Git Commit 任务：创建一次提交
    pub fn spawn_git_commit_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, message: String, allow_empty: bool, author_name: Option<String>, author_email: Option<String>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitCommit");
            if token.is_cancelled() {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCommit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                let author_opt = match (author_name.as_deref(), author_email.as_deref()) {
                    (Some(n), Some(e)) => Some(crate::core::git::default_impl::commit::Author { name: Some(n), email: Some(e) }),
                    _ => None,
                };
                crate::core::git::default_impl::commit::git_commit(&dest_path, &message, author_opt, allow_empty, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCommit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                this.publish_lifecycle_canceled(&id);
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitCommit", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } }
                }
            }
        })
    }

    /// 启动 Git Branch 任务：创建/强制更新/可选检出分支
    pub fn spawn_git_branch_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, checkout: bool, force: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitBranch");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitBranch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::branch::git_branch(&dest_path, &name, checkout, force, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitBranch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitBranch", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }

    /// 启动 Git Checkout 任务：切换或创建+切换分支
    pub fn spawn_git_checkout_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, reference: String, create: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitCheckout");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCheckout", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::checkout::git_checkout(&dest_path, &reference, create, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCheckout", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitCheckout", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }

    /// 启动 Git Tag 任务：创建/更新标签
    pub fn spawn_git_tag_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, message: Option<String>, annotated: bool, force: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitTag");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitTag", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let msg_opt = message.clone();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::tag::git_tag(&dest_path, &name, msg_opt.as_deref(), annotated, force, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitTag", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitTag", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }

    pub fn spawn_git_remote_set_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, url: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitRemoteSet");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteSet", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_set(&dest_path, &name, &url, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteSet", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteSet", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }

    pub fn spawn_git_remote_add_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, url: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitRemoteAdd");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_add(&dest_path, &name, &url, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteAdd", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }

    pub fn spawn_git_remote_remove_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            this.publish_lifecycle_started(&id, "GitRemoteRemove");
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteRemove", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_remove(&dest_path, &name, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteRemove", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } this.publish_lifecycle_canceled(&id); return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }; this.publish_lifecycle_completed(&id); }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteRemove", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> { this.set_state_noemit(&id,TaskState::Failed); this.publish_lifecycle_failed_if_needed(&id, "failed without error event"); } } } }
        })
    }
}

/// 测试辅助：模拟 GitClone 事件发射逻辑（strategy summary + adaptive_tls_rollout），不进行真实网络操作。
pub fn test_emit_clone_strategy_and_rollout(repo: &str, task_id: uuid::Uuid) {
    use crate::events::emitter::AppHandle;
    use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
    let _app = AppHandle; // legacy TaskErrorEvent 已移除，仅保留结构化 summary + adaptive 结构化事件
    let global_cfg = TaskRegistry::runtime_config();
    // 仅发 summary（简化：省略 override 处理）
    // 原 legacy summary JSON 移除（前端改用结构化 StrategyEvent::Summary）
    // legacy summary 已移除，不再发送 TaskErrorEvent 版本
    if let Some(_rewritten) = crate::core::git::transport::maybe_rewrite_https_to_custom(&global_cfg, repo) {
        // legacy adaptive tls rollout JSON 移除（仅结构化事件）
    // legacy adaptive_tls_rollout 已移除
        publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::AdaptiveTlsRollout { id: task_id.to_string(), kind: "GitClone".into(), percent_applied: global_cfg.http.fake_sni_rollout_percent as u8, sampled: true }));
    }
}

/// 测试辅助：带 strategyOverride 的 clone 事件路径（应用 http/tls/retry overrides 并发射相应事件）。
pub fn test_emit_clone_with_override(_repo:&str, task_id:uuid::Uuid, mut strategy_override: serde_json::Value) {
    use crate::events::emitter::{emit_all, AppHandle};
    use crate::events::structured::{publish_global, set_global_event_bus, MemoryEventBus, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
    let app = AppHandle;
    // 若测试未预先设置全局事件总线，则安装一个临时的（不会覆盖已有设置）
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let global_cfg = TaskRegistry::runtime_config();
    // 输入兼容两种形式：
    // 1) 扁平: {"http":{...},"tls":{...},"retry":{...}}
    // 2) 包装: {"strategyOverride": { "http":{...} ... }}
    if let Some(inner) = strategy_override.get("strategyOverride") { strategy_override = inner.clone(); }
    // 直接作为 strategy_override 传入解析函数（该函数期望的就是内部对象本身）
    let parsed_opts = crate::core::git::default_impl::opts::parse_depth_filter_opts(None, None, Some(strategy_override)).expect("parse override");
    let mut applied_codes: Vec<String> = vec![];
    // applied events legacy gating removed; structured events always emitted
    let mut effective_follow = global_cfg.http.follow_redirects;
    let mut effective_max = global_cfg.http.max_redirects;
    let mut retry_plan: super::retry::RetryPlan = global_cfg.retry.clone().into();
    let mut effective_insecure = global_cfg.tls.insecure_skip_verify;
    let mut effective_skip = global_cfg.tls.skip_san_whitelist;
    if let Some(http_over) = parsed_opts.strategy_override.as_ref().and_then(|s| s.http.as_ref()) {
        let (f, m, changed, conflict) = TaskRegistry::apply_http_override("GitClone", &task_id, &global_cfg, Some(http_over));
        effective_follow = f; effective_max = m;
    if changed {
            publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::HttpApplied { id: task_id.to_string(), follow: f, max_redirects: m }));
            applied_codes.push("http_strategy_override_applied".into()); }
        if let Some(msg)=conflict { let evt = TaskErrorEvent { task_id, kind:"GitClone".into(), category:"Protocol".into(), code:Some("strategy_override_conflict".into()), message: format!("http conflict: {}", msg), retried_times:None }; emit_all(&app, EV_ERROR, &evt); publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::Conflict { id: task_id.to_string(), kind: "http".into(), message: msg })); }
    }
    if let Some(tls_over) = parsed_opts.strategy_override.as_ref().and_then(|s| s.tls.as_ref()) {
        let (ins, skip, changed, conflict) = TaskRegistry::apply_tls_override("GitClone", &task_id, &global_cfg, Some(tls_over));
        effective_insecure = ins; effective_skip = skip;
    if changed {
            publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::TlsApplied { id: task_id.to_string(), insecure_skip_verify: ins, skip_san_whitelist: skip }));
            applied_codes.push("tls_strategy_override_applied".into()); }
        if let Some(msg)=conflict { let evt = TaskErrorEvent { task_id, kind:"GitClone".into(), category:"Protocol".into(), code:Some("strategy_override_conflict".into()), message: format!("tls conflict: {}", msg), retried_times:None }; emit_all(&app, EV_ERROR, &evt); publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::Conflict { id: task_id.to_string(), kind: "tls".into(), message: msg })); }
    }
    if let Some(retry_over) = parsed_opts.strategy_override.as_ref().and_then(|s| s.retry.as_ref()) {
        let (plan, changed) = TaskRegistry::apply_retry_override(&global_cfg.retry, Some(retry_over));
        retry_plan = plan;
    if changed {
            let base_plan = load_retry_plan();
            let (diff, _) = compute_retry_diff(&base_plan, &retry_plan);
            publish_global(StructuredEvent::Policy(StructuredPolicyEvent::RetryApplied { id: task_id.to_string(), code: "retry_strategy_override_applied".to_string(), changed: diff.changed.into_iter().map(|s| s.to_string()).collect() }));
            applied_codes.push("retry_strategy_override_applied".into()); }
    }
    let applied_codes_clone = applied_codes.clone();
    publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::Summary { id: task_id.to_string(), kind: "GitClone".into(), http_follow: effective_follow, http_max: effective_max, retry_max: retry_plan.max, retry_base_ms: retry_plan.base_ms, retry_factor: retry_plan.factor, retry_jitter: retry_plan.jitter, tls_insecure: effective_insecure, tls_skip_san: effective_skip, applied_codes: applied_codes_clone.clone(), filter_requested: false }));
    TaskRegistry::emit_strategy_summary(&Some(app.clone()), task_id, "GitClone", (effective_follow, effective_max), &retry_plan, (effective_insecure, effective_skip), applied_codes, false);
}

/// 测试辅助：人工写入 thread-local timing 并发射 AdaptiveTlsTiming 事件（不执行网络）。
#[cfg(test)]
pub fn test_emit_adaptive_tls_timing(task_id: uuid::Uuid, kind:&str) {
    use crate::core::git::transport::metrics::{tl_set_timing, TimingCapture, tl_set_fallback_stage, tl_set_used_fake};
    use crate::events::structured::{publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent};
    // Respect runtime gating so tests can verify suppression behavior
    if !crate::core::git::transport::metrics::metrics_enabled() { return; }
    tl_set_used_fake(true);
    tl_set_fallback_stage("Fake");
    let cap = TimingCapture { connect_ms: Some(10), tls_ms: Some(30), first_byte_ms: Some(40), total_ms: Some(50) };
    tl_set_timing(&cap);
    publish_global(StructuredEvent::Strategy(StructuredStrategyEvent::AdaptiveTlsTiming { id: task_id.to_string(), kind: kind.to_string(), used_fake_sni: true, fallback_stage: "Fake".into(), connect_ms: cap.connect_ms, tls_ms: cap.tls_ms, first_byte_ms: cap.first_byte_ms, total_ms: cap.total_ms, cert_fp_changed: false }));
}

// (test helper for fallback recording was removed in cleanup — fallback currently validated by completion test only)

pub type SharedTaskRegistry = Arc<TaskRegistry>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    async fn wait_for_state(reg:&TaskRegistry, id:Uuid, target:TaskState, max_ms:u64) -> bool {
        let mut waited = 0u64;
        while waited < max_ms {
            if let Some(s) = reg.snapshot(&id) { if s.state == target { return true; } }
            sleep(Duration::from_millis(20)).await; waited += 20;
        }
        false
    }

    #[tokio::test]
    async fn test_create_initial_pending() {
        let reg = TaskRegistry::new();
        let (id, _t) = reg.create(TaskKind::Sleep { ms: 100 });
        let snap = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(snap.state, TaskState::Pending));
    }

    #[tokio::test]
    async fn test_sleep_task_completes() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
        let handle = reg.spawn_sleep_task(None, id, token, 120);
        // 等待完成
        let ok = wait_for_state(&reg, id, TaskState::Completed, 1500).await;
        assert!(ok, "task should complete");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cancel_sleep_task() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 500 });
        let handle = reg.spawn_sleep_task(None, id, token.clone(), 500);
        // 取消前先确认进入 running
        let entered = wait_for_state(&reg, id, TaskState::Running, 500).await; assert!(entered, "should enter running");
        token.cancel();
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await; assert!(canceled, "should cancel");
        handle.await.unwrap();
    }
}

#[cfg(test)]
mod http_override_tests_new {
    use super::*;
    use crate::core::git::default_impl::opts::StrategyHttpOverride;
    #[test]
    fn no_override() {
        let global = AppConfig::default();
    let (f,m,changed,_conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, None);
        assert_eq!(f, global.http.follow_redirects);
        assert_eq!(m, global.http.max_redirects);
        assert!(!changed);
    }
    #[test]
    fn override_changes() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride { follow_redirects: Some(!global.http.follow_redirects), max_redirects: Some(3), ..Default::default() };
        let (f,m,changed,conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(f, !global.http.follow_redirects);
        if f == false { // conflict path: follow=false & max>0 -> normalized 0
            assert_eq!(m, 0, "max should normalize to 0 when follow=false");
            assert!(conflict.is_some(), "conflict message expected");
        } else {
            assert_eq!(m, 3);
            assert!(conflict.is_none());
        }
        assert!(changed);
    }
    #[test]
    fn clamp_applies() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride { follow_redirects: None, max_redirects: Some(99), ..Default::default() };
    let (_f,m,changed,_conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(m, 20);
        assert!(changed);
    }
}

#[cfg(test)]
mod retry_override_tests_new {
    use super::*;
    use crate::core::config::model::RetryCfg;
    use crate::core::git::default_impl::opts::StrategyRetryOverride;

    #[test]
    fn no_retry_override() {
        let global = RetryCfg::default();
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, None);
        assert_eq!(plan.max, global.max);
        assert_eq!(plan.base_ms, global.base_ms);
        assert!(!changed);
    }

    #[test]
    fn retry_override_changes() {
        let mut global = RetryCfg::default();
        global.max = 6; // default
        let over = StrategyRetryOverride { max: Some(3), base_ms: Some(500), factor: Some(2.0), jitter: Some(false) };
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
        assert!(changed);
        assert_eq!(plan.max, 3);
        assert_eq!(plan.base_ms, 500);
        assert_eq!(plan.factor, 2.0);
        assert!(!plan.jitter);
    }
}

#[cfg(test)]
mod tls_override_tests_new {
    use super::*;
    use crate::core::git::default_impl::opts::StrategyTlsOverride;

    #[test]
    fn no_tls_override() {
        let global = AppConfig::default();
    let (ins, skip, changed,_conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, None);
        assert_eq!(ins, global.tls.insecure_skip_verify);
        assert_eq!(skip, global.tls.skip_san_whitelist);
        assert!(!changed);
    }

    #[test]
    fn override_insecure_only() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride { insecure_skip_verify: Some(!global.tls.insecure_skip_verify), skip_san_whitelist: None };
    let (ins, skip, changed,_conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(ins, !global.tls.insecure_skip_verify);
        assert_eq!(skip, global.tls.skip_san_whitelist);
        assert!(changed);
    }

    #[test]
    fn override_skip_san_only() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride { insecure_skip_verify: None, skip_san_whitelist: Some(!global.tls.skip_san_whitelist) };
    let (ins, skip, changed,_conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(ins, global.tls.insecure_skip_verify);
        assert_eq!(skip, !global.tls.skip_san_whitelist);
        assert!(changed);
    }

    #[test]
    fn override_both_same_as_global() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride { insecure_skip_verify: Some(global.tls.insecure_skip_verify), skip_san_whitelist: Some(global.tls.skip_san_whitelist) };
    let (_ins, _skip, changed,_conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert!(!changed, "no change when values equal global");
    }

    #[test]
    fn override_both_changed() {
        let mut global = AppConfig::default();
        // ensure starting values are known defaults: insecure=false skip=false
        global.tls.insecure_skip_verify = false;
        global.tls.skip_san_whitelist = false;
        let over = StrategyTlsOverride { insecure_skip_verify: Some(true), skip_san_whitelist: Some(true) };
        let (ins, skip, changed, conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert!(changed);
        assert!(ins);
        // skip should be normalized to false when insecure=true
        assert!(!skip, "skipSanWhitelist expected normalized to false");
        assert!(conflict.is_some(), "tls conflict expected");
    }

    #[test]
    fn global_config_not_mutated() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride { insecure_skip_verify: Some(true), skip_san_whitelist: Some(true) };
        let _ = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        // global should remain defaults (false/false)
        assert!(!global.tls.insecure_skip_verify);
        assert!(!global.tls.skip_san_whitelist);
    }
}
