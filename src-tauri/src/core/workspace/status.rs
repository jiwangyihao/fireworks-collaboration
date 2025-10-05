//! Workspace status service and data structures.
//!
//! Provides cached, concurrent repository status collection for workspace repositories,
//! including filtering, sorting, and cache management utilities.

use super::model::{RepositoryEntry, Workspace, WorkspaceConfig};
use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset, Utc};
use git2::{BranchType, Repository as GitRepository, Status as GitStatus, StatusOptions};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tracing::debug;

/// Cached repository status entry with timestamp for TTL validation.
#[derive(Debug, Clone)]
struct CachedRepositoryStatus {
    status: RepositoryStatus,
    computed_at: Instant,
}

#[derive(Debug, Default)]
struct WorkspaceStatusCache {
    entries: HashMap<String, CachedRepositoryStatus>,
}

/// Service responsible for collecting and caching repository status snapshots.
#[derive(Debug)]
pub struct WorkspaceStatusService {
    cache: Mutex<WorkspaceStatusCache>,
    ttl_secs: AtomicU64,
    max_concurrency: AtomicUsize,
    auto_refresh_secs: AtomicU64,
}

impl WorkspaceStatusService {
    /// Create a new status service from workspace configuration.
    pub fn new(config: &WorkspaceConfig) -> Self {
        let ttl = sanitize_ttl(config.status_cache_ttl_secs);
        let concurrency = sanitize_concurrency(config.status_max_concurrency);
        let auto_refresh = config.status_auto_refresh_secs.unwrap_or_default();

        Self {
            cache: Mutex::new(WorkspaceStatusCache::default()),
            ttl_secs: AtomicU64::new(ttl),
            max_concurrency: AtomicUsize::new(concurrency),
            auto_refresh_secs: AtomicU64::new(auto_refresh),
        }
    }

    /// Update service parameters from workspace configuration.
    pub fn update_from_config(&self, config: &WorkspaceConfig) {
        let ttl = sanitize_ttl(config.status_cache_ttl_secs);
        let concurrency = sanitize_concurrency(config.status_max_concurrency);
        let auto_refresh = config.status_auto_refresh_secs.unwrap_or_default();
        self.ttl_secs.store(ttl, AtomicOrdering::Relaxed);
        self.max_concurrency
            .store(concurrency, AtomicOrdering::Relaxed);
        self.auto_refresh_secs
            .store(auto_refresh, AtomicOrdering::Relaxed);
    }

    /// Clear cached status data.
    pub fn clear_cache(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.entries.clear();
        }
    }

    /// Remove cached status for a specific repository.
    pub fn invalidate_repo(&self, repo_id: &str) -> bool {
        if let Ok(mut guard) = self.cache.lock() {
            guard.entries.remove(repo_id).is_some()
        } else {
            false
        }
    }

    fn ttl(&self) -> Duration {
        Duration::from_secs(self.ttl_secs.load(AtomicOrdering::Relaxed))
    }

    fn concurrency(&self) -> usize {
        self.max_concurrency.load(AtomicOrdering::Relaxed).max(1)
    }

    fn auto_refresh(&self) -> Option<u64> {
        match self.auto_refresh_secs.load(AtomicOrdering::Relaxed) {
            0 => None,
            value => Some(value),
        }
    }

    /// Collect repository statuses according to query parameters.
    pub async fn query_statuses(
        &self,
        workspace: &Workspace,
        mut query: StatusQuery,
    ) -> Result<WorkspaceStatusResponse> {
        let ttl = self.ttl();
        let now = Instant::now();
        let workspace_root = workspace.root_path.clone();
        let include_disabled = query.include_disabled;

        let mut selected_repos = Vec::new();
        let mut missing_repo_ids = Vec::new();

        let requested_ids: Option<HashSet<String>> = query
            .repo_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect());

        if let Some(ids) = &requested_ids {
            let existing: HashSet<String> = workspace
                .repositories
                .iter()
                .map(|r| r.id.clone())
                .collect();
            for id in ids {
                if !existing.contains(id) {
                    missing_repo_ids.push(id.clone());
                }
            }
        }

        for repo in &workspace.repositories {
            if !include_disabled && !repo.enabled {
                continue;
            }
            if let Some(ids) = &requested_ids {
                if !ids.contains(&repo.id) {
                    continue;
                }
            }
            selected_repos.push(repo.clone());
        }

        let repo_count = selected_repos.len();

        let mut cached_statuses = Vec::new();
        let mut repos_to_refresh = Vec::new();
        {
            let mut cache_guard = self
                .cache
                .lock()
                .map_err(|e| anyhow!("Failed to lock workspace status cache: {}", e))?;

            if requested_ids.is_none() {
                let active_ids: HashSet<String> =
                    selected_repos.iter().map(|r| r.id.clone()).collect();
                cache_guard.entries.retain(|id, _| active_ids.contains(id));
            }

            let force_refresh = query.force_refresh;
            for repo in &selected_repos {
                match cache_guard.entries.get(&repo.id) {
                    Some(entry)
                        if !force_refresh && now.duration_since(entry.computed_at) < ttl =>
                    {
                        let mut status = entry.status.clone();
                        status.is_cached = true;
                        cached_statuses.push(status);
                    }
                    _ => {
                        repos_to_refresh.push(repo.clone());
                    }
                }
            }
        }

        let refreshed_statuses = collect_statuses_concurrently(
            repos_to_refresh,
            workspace_root.clone(),
            self.concurrency(),
        )
        .await?;

        {
            let mut cache_guard = self
                .cache
                .lock()
                .map_err(|e| anyhow!("Failed to lock workspace status cache: {}", e))?;
            for status in &refreshed_statuses {
                let mut cached = status.clone();
                cached.is_cached = false;
                cache_guard.entries.insert(
                    status.repo_id.clone(),
                    CachedRepositoryStatus {
                        status: cached,
                        computed_at: Instant::now(),
                    },
                );
            }
        }

        let logger_workspace_name = workspace.name.clone();
        debug!(
            target = "workspace::status",
            workspace = %logger_workspace_name,
            total = repo_count,
            refreshed = refreshed_statuses.len(),
            cached = cached_statuses.len(),
            "Collected workspace repository statuses"
        );

        let mut combined = Vec::with_capacity(refreshed_statuses.len() + cached_statuses.len());
        combined.extend(refreshed_statuses.into_iter());
        combined.extend(cached_statuses.into_iter());

        let filter = query.filter.clone();
        let mut filtered: Vec<RepositoryStatus> =
            combined.into_iter().filter(|s| filter.matches(s)).collect();

        if let Some(sort) = query.sort.take() {
            let direction = sort.direction;
            filtered.sort_by(|a, b| {
                let ordering = match sort.field {
                    StatusSortField::Name => case_insensitive_cmp(&a.name, &b.name),
                    StatusSortField::Branch => compare_optional_strings(
                        a.current_branch.as_ref(),
                        b.current_branch.as_ref(),
                    ),
                    StatusSortField::Ahead => a.ahead.cmp(&b.ahead),
                    StatusSortField::Behind => a.behind.cmp(&b.behind),
                    StatusSortField::LastUpdated => a.status_unix_time.cmp(&b.status_unix_time),
                    StatusSortField::LastCommit => a
                        .last_commit_unix_time
                        .unwrap_or(i64::MIN)
                        .cmp(&b.last_commit_unix_time.unwrap_or(i64::MIN)),
                    StatusSortField::WorkingState => working_state_rank(&a.working_state)
                        .cmp(&working_state_rank(&b.working_state)),
                };
                match direction {
                    StatusSortDirection::Asc => ordering,
                    StatusSortDirection::Desc => ordering.reverse(),
                }
            });
        }

        let refreshed_count = filtered.iter().filter(|s| !s.is_cached).count();
        let cached_count = filtered.iter().filter(|s| s.is_cached).count();
        let summary = build_summary(&filtered);

        Ok(WorkspaceStatusResponse {
            generated_at: Utc::now().to_rfc3339(),
            total: filtered.len(),
            refreshed: refreshed_count,
            cached: cached_count,
            cache_ttl_secs: ttl.as_secs(),
            auto_refresh_secs: self.auto_refresh(),
            queried: repo_count,
            missing_repo_ids,
            summary,
            statuses: filtered,
        })
    }
}

/// Request parameters for workspace status queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusQuery {
    #[serde(default)]
    pub repo_ids: Option<Vec<String>>,
    #[serde(default)]
    pub include_disabled: bool,
    #[serde(default)]
    pub force_refresh: bool,
    #[serde(default)]
    pub filter: StatusFilter,
    pub sort: Option<StatusSort>,
}

impl Default for StatusQuery {
    fn default() -> Self {
        Self {
            repo_ids: None,
            include_disabled: false,
            force_refresh: false,
            filter: StatusFilter::default(),
            sort: None,
        }
    }
}

/// Filter options applied to collected repository statuses.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusFilter {
    pub branch: Option<String>,
    pub name_contains: Option<String>,
    pub tags: Option<Vec<String>>,
    pub has_local_changes: Option<bool>,
    pub sync_states: Option<Vec<SyncState>>,
}

impl StatusFilter {
    fn matches(&self, status: &RepositoryStatus) -> bool {
        if let Some(branch_filter) = &self.branch {
            let branch = status
                .current_branch
                .as_ref()
                .map(|b| b.to_lowercase())
                .unwrap_or_default();
            if !branch.contains(&branch_filter.to_lowercase()) {
                return false;
            }
        }

        if let Some(name_filter) = &self.name_contains {
            if !status
                .name
                .to_lowercase()
                .contains(&name_filter.to_lowercase())
            {
                return false;
            }
        }

        if let Some(required_tags) = &self.tags {
            let available: Vec<String> = status.tags.iter().map(|t| t.to_lowercase()).collect();
            for tag in required_tags {
                if !available.contains(&tag.to_lowercase()) {
                    return false;
                }
            }
        }

        if let Some(has_changes) = self.has_local_changes {
            let dirty = matches!(status.working_state, WorkingTreeState::Dirty);
            if dirty != has_changes {
                return false;
            }
        }

        if let Some(sync_states) = &self.sync_states {
            if !sync_states.contains(&status.sync_state) {
                return false;
            }
        }

        true
    }
}

/// Sorting options for repository status results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSort {
    pub field: StatusSortField,
    #[serde(default)]
    pub direction: StatusSortDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusSortField {
    Name,
    Branch,
    Ahead,
    Behind,
    LastUpdated,
    LastCommit,
    WorkingState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StatusSortDirection {
    Asc,
    Desc,
}

impl Default for StatusSortDirection {
    fn default() -> Self {
        StatusSortDirection::Asc
    }
}

/// Workspace repository working tree cleanliness state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkingTreeState {
    Clean,
    Dirty,
    Missing,
    Error,
}

/// Repository synchronization state relative to upstream remote.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Clean,
    Ahead,
    Behind,
    Diverged,
    Detached,
    Unknown,
}

/// Repository status snapshot returned to frontend clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryStatus {
    pub repo_id: String,
    pub name: String,
    pub enabled: bool,
    pub path: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub sync_state: SyncState,

    pub working_state: WorkingTreeState,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicts: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_unix_time: Option<i64>,

    pub status_timestamp: String,
    pub status_unix_time: i64,

    #[serde(default)]
    pub is_cached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Aggregated working tree state counts for a workspace query.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkingStateSummary {
    pub clean: usize,
    pub dirty: usize,
    pub missing: usize,
    pub error: usize,
}

/// Aggregated synchronization state counts for a workspace query.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStateSummary {
    pub clean: usize,
    pub ahead: usize,
    pub behind: usize,
    pub diverged: usize,
    pub detached: usize,
    pub unknown: usize,
}

/// Aggregated status information for quick dashboard summaries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusSummary {
    pub working_states: WorkingStateSummary,
    pub sync_states: SyncStateSummary,
    pub error_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub error_repositories: Vec<String>,
}

/// Query response payload containing aggregated repository statuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusResponse {
    pub generated_at: String,
    pub total: usize,
    pub refreshed: usize,
    pub cached: usize,
    pub cache_ttl_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_refresh_secs: Option<u64>,
    pub queried: usize,
    pub missing_repo_ids: Vec<String>,
    #[serde(default)]
    pub summary: WorkspaceStatusSummary,
    pub statuses: Vec<RepositoryStatus>,
}

async fn collect_statuses_concurrently(
    repos: Vec<RepositoryEntry>,
    workspace_root: PathBuf,
    max_concurrency: usize,
) -> Result<Vec<RepositoryStatus>> {
    if repos.is_empty() {
        return Ok(Vec::new());
    }

    let mut handles: VecDeque<tokio::task::JoinHandle<RepositoryStatus>> = VecDeque::new();
    let mut results = Vec::with_capacity(repos.len());
    let concurrency = max_concurrency.max(1);

    for repo in repos {
        let root = workspace_root.clone();
        let handle = tokio::task::spawn_blocking(move || compute_repository_status(&root, &repo));
        handles.push_back(handle);
        if handles.len() >= concurrency {
            if let Some(handle) = handles.pop_front() {
                let status = handle
                    .await
                    .map_err(|e| anyhow!("Status worker panicked: {}", e))?;
                results.push(status);
            }
        }
    }

    while let Some(handle) = handles.pop_front() {
        let status = handle
            .await
            .map_err(|e| anyhow!("Status worker panicked: {}", e))?;
        results.push(status);
    }

    Ok(results)
}

fn compute_repository_status(root: &Path, entry: &RepositoryEntry) -> RepositoryStatus {
    let full_path = resolve_repository_path(root, &entry.path);
    let now = Utc::now();

    let mut status = RepositoryStatus {
        repo_id: entry.id.clone(),
        name: entry.name.clone(),
        enabled: entry.enabled,
        path: entry.path.to_string_lossy().to_string(),
        tags: entry.tags.clone(),
        remote_url: if entry.remote_url.is_empty() {
            None
        } else {
            Some(entry.remote_url.clone())
        },
        current_branch: None,
        upstream_branch: None,
        ahead: 0,
        behind: 0,
        sync_state: SyncState::Unknown,
        working_state: WorkingTreeState::Clean,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        conflicts: 0,
        last_commit_id: None,
        last_commit_message: None,
        last_commit_time: None,
        last_commit_unix_time: None,
        status_timestamp: now.to_rfc3339(),
        status_unix_time: now.timestamp(),
        is_cached: false,
        error: None,
    };

    if !full_path.exists() {
        status.working_state = WorkingTreeState::Missing;
        append_error(&mut status, "Repository path not found");
        return status;
    }

    match GitRepository::open(&full_path) {
        Ok(repo) => {
            if status.remote_url.is_none() {
                if let Ok(remote) = repo.find_remote("origin") {
                    status.remote_url = remote.url().map(|s| s.to_string());
                }
            }

            let head_detached = repo.head_detached().unwrap_or(false);
            match repo.head() {
                Ok(head_ref) => {
                    if head_ref.is_branch() {
                        if let Some(branch) = head_ref.shorthand() {
                            status.current_branch = Some(branch.to_string());
                            if let Ok(branch_obj) = repo.find_branch(branch, BranchType::Local) {
                                if let Ok(upstream) = branch_obj.upstream() {
                                    status.upstream_branch =
                                        upstream.name().ok().flatten().map(|s| s.to_string());
                                    if let (Some(local_oid), Some(upstream_oid)) =
                                        (branch_obj.get().target(), upstream.get().target())
                                    {
                                        if let Ok((ahead, behind)) =
                                            repo.graph_ahead_behind(local_oid, upstream_oid)
                                        {
                                            status.ahead = ahead as u32;
                                            status.behind = behind as u32;
                                        }
                                    }
                                }
                            }
                        }
                        status.sync_state = SyncState::Clean;
                    } else if head_detached {
                        status.sync_state = SyncState::Detached;
                    }

                    if let Ok(commit) = head_ref.peel_to_commit() {
                        status.last_commit_id = Some(commit.id().to_string());
                        status.last_commit_message = commit
                            .message()
                            .map(|m| m.trim().to_string())
                            .or_else(|| commit.summary().map(|s| s.trim().to_string()));

                        if let Some((timestamp, unix)) = commit_time_rfc3339(&commit) {
                            status.last_commit_time = Some(timestamp);
                            status.last_commit_unix_time = Some(unix);
                        }
                    }
                }
                Err(err) => {
                    append_error(&mut status, format!("Failed to read HEAD: {err}"));
                    status.sync_state = SyncState::Unknown;
                }
            }

            let mut opts = StatusOptions::new();
            opts.include_untracked(true);
            opts.recurse_untracked_dirs(true);
            opts.renames_head_to_index(true);
            opts.renames_index_to_workdir(true);

            match repo.statuses(Some(&mut opts)) {
                Ok(statuses) => {
                    for entry in statuses.iter() {
                        let st = entry.status();
                        if st.intersects(
                            GitStatus::INDEX_NEW
                                | GitStatus::INDEX_MODIFIED
                                | GitStatus::INDEX_DELETED
                                | GitStatus::INDEX_RENAMED
                                | GitStatus::INDEX_TYPECHANGE,
                        ) {
                            status.staged += 1;
                        }
                        if st.intersects(GitStatus::WT_NEW) {
                            status.untracked += 1;
                        }
                        if st.intersects(
                            GitStatus::WT_MODIFIED
                                | GitStatus::WT_DELETED
                                | GitStatus::WT_RENAMED
                                | GitStatus::WT_TYPECHANGE,
                        ) {
                            status.unstaged += 1;
                        }
                        if st.intersects(GitStatus::CONFLICTED) {
                            status.conflicts += 1;
                        }
                    }
                }
                Err(err) => {
                    append_error(&mut status, format!("Failed to collect status: {err}"));
                    status.working_state = WorkingTreeState::Error;
                }
            }

            let dirty = status.staged > 0
                || status.unstaged > 0
                || status.untracked > 0
                || status.conflicts > 0;
            status.working_state = if dirty {
                WorkingTreeState::Dirty
            } else {
                WorkingTreeState::Clean
            };

            status.sync_state = derive_sync_state(
                &status.sync_state,
                status.ahead,
                status.behind,
                status.current_branch.is_some(),
            );
        }
        Err(err) => {
            status.working_state = WorkingTreeState::Error;
            append_error(&mut status, format!("Failed to open repository: {err}"));
        }
    }

    status
}

fn commit_time_rfc3339(commit: &git2::Commit<'_>) -> Option<(String, i64)> {
    let time = commit.time();
    let utc = DateTime::<Utc>::from_timestamp(time.seconds(), 0)?;
    let offset_seconds = time.offset_minutes() * 60;
    let offset =
        FixedOffset::east_opt(offset_seconds).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
    let localized = utc.with_timezone(&offset);
    Some((localized.to_rfc3339(), utc.timestamp()))
}

fn derive_sync_state(current: &SyncState, ahead: u32, behind: u32, has_branch: bool) -> SyncState {
    if !has_branch {
        return match current {
            SyncState::Detached => SyncState::Detached,
            SyncState::Unknown => SyncState::Unknown,
            _ => SyncState::Unknown,
        };
    }
    if ahead > 0 && behind > 0 {
        SyncState::Diverged
    } else if ahead > 0 {
        SyncState::Ahead
    } else if behind > 0 {
        SyncState::Behind
    } else if matches!(current, SyncState::Detached) {
        SyncState::Detached
    } else {
        SyncState::Clean
    }
}

fn resolve_repository_path(root: &Path, repo_path: &Path) -> PathBuf {
    if repo_path.is_absolute() {
        repo_path.to_path_buf()
    } else {
        root.join(repo_path)
    }
}

fn append_error(status: &mut RepositoryStatus, msg: impl Into<String>) {
    let msg = msg.into();
    if let Some(existing) = &mut status.error {
        existing.push_str("; ");
        existing.push_str(&msg);
    } else {
        status.error = Some(msg);
    }
}

fn case_insensitive_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

fn compare_optional_strings(a: Option<&String>, b: Option<&String>) -> Ordering {
    match (a, b) {
        (Some(a_val), Some(b_val)) => case_insensitive_cmp(a_val, b_val),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn working_state_rank(state: &WorkingTreeState) -> u8 {
    match state {
        WorkingTreeState::Clean => 0,
        WorkingTreeState::Dirty => 1,
        WorkingTreeState::Missing => 2,
        WorkingTreeState::Error => 3,
    }
}

fn build_summary(statuses: &[RepositoryStatus]) -> WorkspaceStatusSummary {
    let mut summary = WorkspaceStatusSummary::default();

    for status in statuses {
        match status.working_state {
            WorkingTreeState::Clean => summary.working_states.clean += 1,
            WorkingTreeState::Dirty => summary.working_states.dirty += 1,
            WorkingTreeState::Missing => summary.working_states.missing += 1,
            WorkingTreeState::Error => summary.working_states.error += 1,
        }

        match status.sync_state {
            SyncState::Clean => summary.sync_states.clean += 1,
            SyncState::Ahead => summary.sync_states.ahead += 1,
            SyncState::Behind => summary.sync_states.behind += 1,
            SyncState::Diverged => summary.sync_states.diverged += 1,
            SyncState::Detached => summary.sync_states.detached += 1,
            SyncState::Unknown => summary.sync_states.unknown += 1,
        }

        if status.error.is_some() {
            summary.error_repositories.push(status.repo_id.clone());
        }
    }

    summary.error_count = summary.error_repositories.len();
    summary
}

fn sanitize_ttl(ttl: u64) -> u64 {
    if ttl == 0 {
        10
    } else {
        ttl
    }
}

fn sanitize_concurrency(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_filter_matches() {
        let mut status = RepositoryStatus {
            repo_id: "repo".into(),
            name: "My Repo".into(),
            enabled: true,
            path: "repo".into(),
            tags: vec!["frontend".into(), "critical".into()],
            remote_url: Some("https://example.com/repo.git".into()),
            current_branch: Some("main".into()),
            upstream_branch: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
            sync_state: SyncState::Clean,
            working_state: WorkingTreeState::Dirty,
            staged: 1,
            unstaged: 0,
            untracked: 0,
            conflicts: 0,
            last_commit_id: None,
            last_commit_message: None,
            last_commit_time: None,
            last_commit_unix_time: None,
            status_timestamp: Utc::now().to_rfc3339(),
            status_unix_time: Utc::now().timestamp(),
            is_cached: false,
            error: None,
        };

        let mut filter = StatusFilter::default();
        filter.branch = Some("main".into());
        filter.name_contains = Some("repo".into());
        filter.tags = Some(vec!["frontend".into()]);
        filter.has_local_changes = Some(true);
        filter.sync_states = Some(vec![SyncState::Clean, SyncState::Ahead]);
        assert!(filter.matches(&status));

        status.sync_state = SyncState::Behind;
        assert!(!filter.matches(&status));
    }

    #[test]
    fn workspace_status_service_sanitizes_config_defaults() {
        let mut cfg = WorkspaceConfig::default();
        cfg.status_cache_ttl_secs = 0;
        cfg.status_max_concurrency = 0;
        cfg.status_auto_refresh_secs = Some(0);

        let service = WorkspaceStatusService::new(&cfg);
        assert_eq!(service.ttl().as_secs(), 10);
        assert_eq!(service.concurrency(), 1);
        assert_eq!(service.auto_refresh(), None);
    }

    #[test]
    fn workspace_status_service_applies_runtime_updates() {
        let mut initial = WorkspaceConfig::default();
        initial.status_cache_ttl_secs = 30;
        initial.status_max_concurrency = 4;
        initial.status_auto_refresh_secs = Some(25);

        let service = WorkspaceStatusService::new(&initial);

        let mut updated = WorkspaceConfig::default();
        updated.status_cache_ttl_secs = 5;
        updated.status_max_concurrency = 2;
        updated.status_auto_refresh_secs = Some(40);

        service.update_from_config(&updated);
        assert_eq!(service.ttl().as_secs(), 5);
        assert_eq!(service.concurrency(), 2);
        assert_eq!(service.auto_refresh(), Some(40));
    }
}
