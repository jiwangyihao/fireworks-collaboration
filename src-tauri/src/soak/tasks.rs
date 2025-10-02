use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::core::tasks::model::TaskState;
use crate::core::tasks::{TaskKind, TaskRegistry};
use crate::events::structured::MemoryEventBus;

use super::aggregator::SoakAggregator;

use std::path::Path;

/// Execute a git push task and wait for completion.
pub fn run_push_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    repo: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let dest_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("push repo path invalid UTF-8"))?
        .to_string();

    let (id, token) = registry.create(TaskKind::GitPush {
        dest: dest_str.clone(),
        remote: Some("origin".to_string()),
        refspecs: None,
        username: None,
        password: None,
        strategy_override: None,
    });

    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        let dest = dest_str;
        async move {
            registry.spawn_git_push_task(None, id, token, dest, Some("origin".to_string()), None, None, None, None)
        }
    });

    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await push task")?;

    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("push snapshot missing"))?
        .state;

    aggregator.record_task("GitPush", state.clone());
    aggregator.process_events(bus.take_all());

    Ok(state)
}

/// Execute a git fetch task and wait for completion.
pub fn run_fetch_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    repo: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let repo_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("fetch repo path invalid UTF-8"))?
        .to_string();

    let (id, token) = registry.create(TaskKind::GitFetch {
        repo: "".to_string(),
        dest: repo_str.clone(),
        depth: None,
        filter: None,
        strategy_override: None,
    });

    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        async move {
            registry.spawn_git_fetch_task_with_opts(
                None,
                id,
                token,
                "".to_string(),
                repo_str,
                None,
                None,
                None,
                None,
            )
        }
    });

    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await fetch task")?;

    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("fetch snapshot missing"))?
        .state;

    aggregator.record_task("GitFetch", state.clone());
    aggregator.process_events(bus.take_all());

    Ok(state)
}

/// Execute a git clone task and wait for completion.
pub fn run_clone_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    origin: &Path,
    dest: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let origin_str = origin
        .to_str()
        .ok_or_else(|| anyhow!("origin path invalid UTF-8"))?
        .to_string();
    let dest_str = dest
        .to_str()
        .ok_or_else(|| anyhow!("dest path invalid UTF-8"))?
        .to_string();

    let (id, token) = registry.create(TaskKind::GitClone {
        repo: origin_str.clone(),
        dest: dest_str.clone(),
        depth: None,
        filter: None,
        strategy_override: None,
    });

    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        async move {
            registry.spawn_git_clone_task_with_opts(
                None,
                id,
                token,
                origin_str,
                dest_str,
                None,
                None,
                None,
            )
        }
    });

    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await clone task")?;

    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("clone snapshot missing"))?
        .state;

    aggregator.record_task("GitClone", state.clone());
    aggregator.process_events(bus.take_all());

    Ok(state)
}
