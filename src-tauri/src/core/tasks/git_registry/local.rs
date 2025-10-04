use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::git::errors::GitError;
use crate::events::emitter::{emit_all, AppHandle};

use super::super::registry::{TaskRegistry, EV_PROGRESS};
use super::helpers::{handle_cancel, report_failure};
use crate::core::tasks::model::TaskProgressEvent;

impl TaskRegistry {
    pub fn spawn_git_init_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitInit");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitInit");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::init::git_init(
                    &dest_path,
                    &interrupt_flag,
                    move |_p| {
                        if let Some(app_ref) = &app_for_cb {
                            let prog = TaskProgressEvent {
                                task_id: id_for_cb,
                                kind: "GitInit".into(),
                                phase: "Running".into(),
                                percent: 100,
                                objects: None,
                                bytes: None,
                                total_hint: None,
                                retried_times: None,
                            };
                            emit_all(app_ref, EV_PROGRESS, &prog);
                        }
                    },
                )
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                handle_cancel(&this, &app, &id, "GitInit");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitInit",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_add_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        paths: Vec<String>,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitAdd");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitAdd");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let path_vec = paths.clone();
            let ref_slices: Vec<&str> = path_vec.iter().map(|s| s.as_str()).collect();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::add::git_add(
                    &dest_path,
                    &ref_slices,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitAdd");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitAdd",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_commit_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        message: String,
        allow_empty: bool,
        author_name: Option<String>,
        author_email: Option<String>,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitCommit");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitCommit");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                let author_opt = match (author_name.as_deref(), author_email.as_deref()) {
                    (Some(n), Some(e)) => Some(crate::core::git::default_impl::commit::Author {
                        name: Some(n),
                        email: Some(e),
                    }),
                    _ => None,
                };
                crate::core::git::default_impl::commit::git_commit(
                    &dest_path,
                    &message,
                    author_opt,
                    allow_empty,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitCommit");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitCommit",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_branch_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        name: String,
        checkout: bool,
        force: bool,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitBranch");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitBranch");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::branch::git_branch(
                    &dest_path,
                    &name,
                    checkout,
                    force,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitBranch");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitBranch",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_checkout_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        reference: String,
        create: bool,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitCheckout");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitCheckout");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::checkout::git_checkout(
                    &dest_path,
                    &reference,
                    create,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitCheckout");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitCheckout",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_tag_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        name: String,
        message: Option<String>,
        annotated: bool,
        force: bool,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitTag");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitTag");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let msg_opt = message.clone();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::tag::git_tag(
                    &dest_path,
                    &name,
                    msg_opt.as_deref(),
                    annotated,
                    force,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitTag");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitTag",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_remote_set_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        name: String,
        url: String,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitRemoteSet");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitRemoteSet");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::remote::git_remote_set(
                    &dest_path,
                    &name,
                    &url,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitRemoteSet");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitRemoteSet",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_remote_add_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        name: String,
        url: String,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitRemoteAdd");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitRemoteAdd");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::remote::git_remote_add(
                    &dest_path,
                    &name,
                    &url,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitRemoteAdd");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitRemoteAdd",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }

    pub fn spawn_git_remote_remove_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        name: String,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            this.mark_running(&app, &id, "GitRemoteRemove");
            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitRemoteRemove");
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id;
                crate::core::git::default_impl::remote::git_remote_remove(
                    &dest_path,
                    &name,
                    &interrupt_flag,
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
                handle_cancel(&this, &app, &id, "GitRemoteRemove");
                return;
            }
            match res {
                Ok(()) => {
                    this.mark_completed(&app, &id);
                }
                Err(e) => {
                    report_failure(
                        &this,
                        &app,
                        &id,
                        "GitRemoteRemove",
                        &e,
                        None,
                        "failed without error event",
                    );
                }
            }
        })
    }
}
