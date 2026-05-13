//! In-memory live-job registry + SQLite persistence.
//! Spec §5.1.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bus::context_updates::{
    ContextUpdate, ContextUpdateQueue, ContextUpdateReason, UpdatePriority,
};
use bus::{BashJobEvent, DomainEventBus};
use dashmap::DashMap;
use jiff::Timestamp;
use klynt_pty::kill_process_group;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::{BashJobRepo, BashJobRow};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tools_core::{
    FailureKind, GateResult, JobError, JobId, JobSpec, JobStatus, JobSupervisorHandle, JobView,
    RingRead,
};
use tracing;

use crate::gate::GateClassifier;
use crate::intelligence::command_key;
use crate::ring::RingFile;
use crate::spawner::spawn_background_command;

pub(crate) enum ChildBackend {
    Process,
    Pty {
        master: std::sync::Arc<tokio::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
        writer: std::sync::Arc<std::sync::Mutex<Option<Box<dyn std::io::Write + Send>>>>,
        rows: AtomicU16,
        cols: AtomicU16,
    },
}

impl std::fmt::Debug for ChildBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChildBackend::Process => f.debug_struct("Process").finish(),
            ChildBackend::Pty { rows, cols, .. } => f
                .debug_struct("Pty")
                .field("rows", rows)
                .field("cols", cols)
                .finish(),
        }
    }
}

impl ChildBackend {
    pub(crate) fn is_pty(&self) -> bool {
        matches!(self, ChildBackend::Pty { .. })
    }
}

#[derive(Debug, Default)]
pub(crate) struct AttachState {
    pub user_at: Option<Timestamp>,
    pub token: Option<String>,
    pub ws_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

const CAP_PER_CHAIN: usize = 6;

const STATE_RUNNING: u8 = 0;
const STATE_STOPPING: u8 = 1;
const STATE_COMPLETED: u8 = 2;

#[derive(Debug)]
struct LiveJob {
    id: JobId,
    spec: JobSpec,
    pgid: Option<u32>,
    ring: Arc<RingFile>,
    cancel: CancellationToken,
    state: AtomicU8,
    started_at: Timestamp,
    backend: ChildBackend,
    attach: Arc<RwLock<AttachState>>,
}

#[derive(Clone)]
pub struct JobSupervisor {
    jobs: Arc<DashMap<JobId, Arc<LiveJob>>>,
    repo: BashJobRepo,
    bus: Arc<DomainEventBus>,
    queue: Arc<ContextUpdateQueue>,
    data_dir: PathBuf,
    sandbox: Arc<MacOsSeatbeltRunner>,
}

impl std::fmt::Debug for JobSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobSupervisor")
            .field("jobs", &self.jobs.len())
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl JobSupervisor {
    pub fn new(
        repo: BashJobRepo,
        bus: Arc<DomainEventBus>,
        queue: Arc<ContextUpdateQueue>,
        data_dir: PathBuf,
        sandbox: Arc<MacOsSeatbeltRunner>,
    ) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            repo,
            bus,
            queue,
            data_dir,
            sandbox,
        }
    }

    /// Synchronous list of in-memory active jobs (no DB block_on needed).
    pub fn list_active(&self, session_id: &str, agent_chain: &[String]) -> Vec<JobView> {
        self.jobs
            .iter()
            .filter(|e| {
                e.value().spec.session_id == session_id
                    && agent_chain.contains(&e.value().spec.agent_id)
            })
            .map(|e| {
                let live = e.value();
                JobView {
                    id: live.id.clone(),
                    session_id: live.spec.session_id.clone(),
                    agent_id: live.spec.agent_id.clone(),
                    description: live.spec.description.clone(),
                    command: live.spec.command.clone(),
                    cwd: live.spec.cwd.clone(),
                    status: JobStatus::Running,
                    started_at: live.started_at,
                    finished_at: None,
                    exit_code: None,
                    gate_result: None,
                    failure_extracted: None,
                    total_bytes_emitted: live.ring.total_bytes_emitted(),
                    bisect_generation: live.ring.bisect_generation(),
                    last_polled_at: None,
                    last_seen_offset: 0,
                }
            })
            .collect()
    }

    /// Like `list_active`, but also returns each job's attached_user_at.
    /// Used by the injector to render the cooperative-handoff section.
    pub fn list_active_with_attach(
        &self,
        session_id: &str,
        agent_chain: &[String],
    ) -> Vec<(JobView, Option<Timestamp>)> {
        self.jobs
            .iter()
            .filter(|e| {
                e.value().spec.session_id == session_id
                    && agent_chain.contains(&e.value().spec.agent_id)
            })
            .map(|e| {
                let live = e.value();
                let attached_at = live
                    .attach
                    .try_read()
                    .ok()
                    .and_then(|g| g.user_at);
                let view = JobView {
                    id: live.id.clone(),
                    session_id: live.spec.session_id.clone(),
                    agent_id: live.spec.agent_id.clone(),
                    description: live.spec.description.clone(),
                    command: live.spec.command.clone(),
                    cwd: live.spec.cwd.clone(),
                    status: JobStatus::Running,
                    started_at: live.started_at,
                    finished_at: None,
                    exit_code: None,
                    gate_result: None,
                    failure_extracted: None,
                    total_bytes_emitted: live.ring.total_bytes_emitted(),
                    bisect_generation: live.ring.bisect_generation(),
                    last_polled_at: None,
                    last_seen_offset: 0,
                };
                (view, attached_at)
            })
            .collect()
    }

    fn storage_err<T>(r: Result<T, storage::error::StorageError>) -> Result<T, JobError> {
        r.map_err(|e| JobError::Storage(e.to_string()))
    }

    fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }

    async fn read_ring_tail_b64(&self, id: &JobId, max_bytes: usize) -> std::io::Result<String> {
        let path = self.log_path(id);
        if !path.exists() {
            return Ok(String::new());
        }
        let bytes = tokio::fs::read(&path).await?;
        let start = bytes.len().saturating_sub(max_bytes);
        use base64::engine::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(&bytes[start..]))
    }

    pub fn log_path(&self, id: &JobId) -> PathBuf {
        self.jobs_dir().join(format!("{}.log", id.as_str()))
    }
    fn final_path(&self, id: &JobId) -> PathBuf {
        self.jobs_dir().join(format!("{}.final", id.as_str()))
    }

    pub async fn reap_session(&self, session_id: &str) -> Result<usize, JobError> {
        let to_kill: Vec<_> = self
            .jobs
            .iter()
            .filter(|e| e.value().spec.session_id == session_id)
            .map(|e| e.key().clone())
            .collect();
        let n = to_kill.len();
        for id in to_kill {
            // Defensively detach any live attach so the WebSocket gets a clean
            // close frame before the process group dies. detach() is idempotent.
            if let Err(e) = <Self as JobSupervisorHandle>::detach(self, &id).await {
                tracing::debug!(job_id=%id.0, "detach during reap failed (ok if not attached): {e}");
            }
            if let Err(e) = self.stop(&id, "thread deleted").await {
                tracing::warn!(job_id=%id.0, "reap_session stop failed: {}", e);
            }
        }
        Ok(n)
    }

    pub async fn list_for_thread(
        &self,
        session_id: &str,
        agent_chain: &[String],
        active_only: bool,
    ) -> Vec<JobView> {
        let rows = match self
            .repo
            .list_for_session(session_id, agent_chain, active_only)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(session_id, "list_for_session failed: {e}");
                return vec![];
            }
        };
        rows.into_iter().map(view_from_row).collect()
    }

    /// Borrow the underlying repo (cheap clone — wraps an Arc<Pool>).
    pub fn repo(&self) -> &BashJobRepo {
        &self.repo
    }

    pub async fn reconcile_on_startup(&self) -> Result<usize, JobError> {
        let orphans = self
            .repo
            .list_orphans()
            .await
            .map_err(|e| JobError::Storage(e.to_string()))?;
        let mut count = 0;
        for row in orphans {
            let id = JobId::from_str(&row.id)?;
            let final_path = self.final_path(&id);
            let log_path = PathBuf::from(&row.log_path);

            if final_path.exists() {
                Self::storage_err(
                    self.repo
                        .update_status(
                            &row.id,
                            JobStatus::Completed.as_str(),
                            None,
                            None,
                            None,
                            None,
                            Some(Timestamp::now()),
                            Some(final_path.to_str().unwrap_or("")),
                            row.total_bytes_emitted,
                            row.bisect_count,
                        )
                        .await,
                )?;
            } else {
                let detail = "klynt restarted while job was running";
                let extracted = serde_json::json!({ "log_preserved": log_path.exists() });
                Self::storage_err(
                    self.repo
                        .update_status(
                            &row.id,
                            JobStatus::Lost.as_str(),
                            None,
                            Some(FailureKind::Lost.as_db_str().as_ref()),
                            Some(detail),
                            Some(&extracted.to_string()),
                            Some(Timestamp::now()),
                            None,
                            row.total_bytes_emitted,
                            row.bisect_count,
                        )
                        .await,
                )?;

                if row.tty {
                    if let Err(e) = self.repo.clear_attached(&row.id).await {
                        tracing::warn!(error = ?e, job_id=%row.id, "clear_attached on lost row failed");
                    }
                    if row.attached_user_at.is_some() {
                        self.bus.publish_bash_job(BashJobEvent::AttachEnded {
                            job_id: row.id.clone(),
                            thread_id: row.session_id.clone(),
                            agent_id: row.agent_id.clone(),
                            timestamp: Timestamp::now(),
                            duration_ms: 0,
                        });
                    }
                }

                let body = format!(
                    "<system-reminder>\nBackground job {} was lost (klynt restarted while it was running).\nDescription: {}\nPartial output preserved at: {}\nUse coding_task_output(\"{}\") to inspect.\n</system-reminder>",
                    row.id, row.description, row.log_path, row.id
                );
                self.queue.push(ContextUpdate {
                    reason: ContextUpdateReason::CodingJobsChanged,
                    content: Some(body),
                    metadata: None,
                    priority: UpdatePriority::High,
                    timestamp: Timestamp::now(),
                });

                self.bus.publish_bash_job(BashJobEvent::Lost {
                    job_id: row.id.clone(),
                    thread_id: row.session_id.clone(),
                    agent_id: row.agent_id.clone(),
                });
            }
            count += 1;
        }
        if let Err(e) = self.sweep_orphan_files().await {
            tracing::warn!("sweep_orphan_files failed: {}", e);
        }
        Ok(count)
    }

    async fn sweep_orphan_files(&self) -> std::io::Result<()> {
        let dir = self.jobs_dir();
        if !dir.exists() {
            return Ok(());
        }
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = match name.to_str() {
                Some(s) => s,
                None => continue,
            };
            if name.ends_with(".log.tmp") {
                if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    tracing::debug!(
                        "failed to remove tmp file {}: {}",
                        entry.path().display(),
                        e
                    );
                }
                continue;
            }
            let id_part = name.trim_end_matches(".log").trim_end_matches(".final");
            if !id_part.starts_with("bash-") {
                continue;
            }
            let exists_in_db = self
                .repo
                .get(id_part)
                .await
                .map(|opt| opt.is_some())
                .unwrap_or(false);
            if !exists_in_db {
                if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    tracing::debug!(
                        "failed to remove orphan file {}: {}",
                        entry.path().display(),
                        e
                    );
                }
            }
        }
        Ok(())
    }

    async fn handle_exit(&self, id: &JobId, exit: std::io::Result<Option<i32>>) {
        let live = match self.jobs.get(id).map(|e| e.value().clone()) {
            Some(l) => l,
            None => return,
        };
        // Capture cancellation flag *before* overwriting state.
        let was_cancelled = live.state.load(Ordering::Acquire) == STATE_STOPPING;
        live.state.store(STATE_COMPLETED, Ordering::Release);
        live.cancel.cancel();
        let final_path = self.final_path(id);
        let total = live.ring.total_bytes_emitted();
        let bisect_count = live.ring.bisect_count() as i64;

        let exit_code = exit.as_ref().ok().and_then(|s| *s).unwrap_or(-1);
        let was_timeout = false;
        let elapsed_ms = (Timestamp::now() - live.started_at)
            .total(jiff::Unit::Millisecond)
            .unwrap_or(0.0) as u64;

        if let Err(e) = live.ring.finalize(&final_path).await {
            tracing::warn!(job_id=%id.0, "ring finalize failed: {}", e);
        }

        let final_bytes = match tokio::fs::read(&final_path).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(job_id=%id.0, "failed to read final file: {}", e);
                vec![]
            }
        };
        let final_str = String::from_utf8_lossy(&final_bytes);
        let head = final_str.chars().take(8000).collect::<String>();
        let tail = final_str
            .chars()
            .rev()
            .take(8000)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();

        let result = GateClassifier::classify(
            &head,
            &tail,
            exit_code,
            &live.spec.command,
            was_timeout,
            was_cancelled,
            false,
            elapsed_ms,
        );
        let (status_str, kind_str, detail_str, extracted_str) = match &result {
            GateResult::Passed => (JobStatus::Completed.as_str().to_string(), None, None, None),
            GateResult::Failed {
                kind,
                detail,
                extracted,
            } => {
                let k = kind.as_db_str();
                let status_owned = if matches!(kind, FailureKind::Cancelled | FailureKind::Lost) {
                    k.as_ref().to_string()
                } else {
                    JobStatus::Failed.as_str().to_string()
                };
                (
                    status_owned,
                    Some(k.as_ref().to_string()),
                    Some(detail.clone()),
                    Some(extracted.to_string()),
                )
            }
        };

        if let Err(e) = self
            .repo
            .update_status(
                &id.0,
                &status_str,
                Some(exit_code),
                kind_str.as_deref(),
                detail_str.as_deref(),
                extracted_str.as_deref(),
                Some(Timestamp::now()),
                Some(final_path.to_str().unwrap_or("")),
                total as i64,
                bisect_count,
            )
            .await
        {
            tracing::error!(job_id=%id.0, "update_status failed: {}", e);
        }

        // 2.3b: query prior run + compute diff for the completion notification.
        let curr_command_key = command_key(&live.spec.command);
        let prior = match self
            .repo
            .find_prior_by_command_key(&live.spec.session_id, &curr_command_key, id.as_str())
            .await
        {
            Ok(opt) => opt,
            Err(e) => {
                tracing::warn!(error = ?e, "prior lookup failed; skipping diff");
                None
            }
        };
        let curr_row = match self.repo.get(id.as_str()).await {
            Ok(Some(r)) => Some(r),
            _ => None,
        };
        let diff = match (prior, curr_row) {
            (Some(p), Some(c)) => Some(crate::intelligence::diff_against_prior(&p, &c)),
            _ => None,
        };

        match &result {
            GateResult::Passed => {
                self.bus.publish_bash_job(BashJobEvent::Completed {
                    job_id: id.0.clone(),
                    thread_id: live.spec.session_id.clone(),
                    agent_id: live.spec.agent_id.clone(),
                    exit_code,
                    duration_ms: elapsed_ms,
                });
            }
            GateResult::Failed { kind, detail, .. } => {
                if matches!(kind, FailureKind::Cancelled) {
                    self.bus.publish_bash_job(BashJobEvent::Cancelled {
                        job_id: id.0.clone(),
                        thread_id: live.spec.session_id.clone(),
                        agent_id: live.spec.agent_id.clone(),
                        reason: detail.clone(),
                    });
                } else {
                    self.bus.publish_bash_job(BashJobEvent::Failed {
                        job_id: id.0.clone(),
                        thread_id: live.spec.session_id.clone(),
                        agent_id: live.spec.agent_id.clone(),
                        exit_code: Some(exit_code),
                        failure_kind: kind.as_db_str().as_ref().to_string(),
                        failure_detail: detail.clone(),
                    });
                }
            }
        }

        if !live.spec.silent_completion {
            let body = crate::render::completion_notification(
                id,
                &live.spec,
                &result,
                &final_str,
                diff.as_ref(),
            );
            self.queue.push(ContextUpdate {
                reason: ContextUpdateReason::CodingJobsChanged,
                content: Some(body),
                metadata: None,
                priority: UpdatePriority::High,
                timestamp: Timestamp::now(),
            });
        }

        self.jobs.remove(id);
    }

    async fn update_poll_cursor(&self, id: &JobId, new_offset: u64) -> Result<(), JobError> {
        Self::storage_err(
            self.repo
                .update_poll_cursor(&id.0, Timestamp::now(), new_offset as i64)
                .await,
        )
    }
}

fn parse_failure_kind(s: &str) -> FailureKind {
    if let Some(rest) = s.strip_prefix("Other:") {
        FailureKind::Other(rest.to_string())
    } else {
        match s {
            "CompileError" => FailureKind::CompileError,
            "TestFailure" => FailureKind::TestFailure,
            "LintFailure" => FailureKind::LintFailure,
            "NetworkBindFailure" => FailureKind::NetworkBindFailure,
            "Timeout" => FailureKind::Timeout,
            "Cancelled" => FailureKind::Cancelled,
            "Lost" => FailureKind::Lost,
            _ => FailureKind::Other(s.to_string()),
        }
    }
}

#[async_trait]
impl JobSupervisorHandle for JobSupervisor {
    async fn spawn(&self, spec: JobSpec) -> Result<JobView, JobError> {
        let chain = &spec.agent_chain;
        let active = Self::storage_err(
            self.repo
                .count_active_for_chain(&spec.session_id, chain)
                .await,
        )?;
        if active >= CAP_PER_CHAIN as i64 {
            return Err(JobError::CapReached {
                active: active as usize,
            });
        }

        let id = JobId::new();
        let log_path = self.log_path(&id);
        let started_at = Timestamp::now();
        let cwd_str = spec.cwd.to_string_lossy().to_string();

        let row = BashJobRow {
            id: id.0.clone(),
            session_id: spec.session_id.clone(),
            agent_id: spec.agent_id.clone(),
            description: spec.description.clone(),
            command: spec.command.clone(),
            command_key: command_key(&spec.command),
            cwd: cwd_str.clone(),
            timeout_ms: spec.timeout_ms as i64,
            silent_completion: spec.silent_completion,
            tty: spec.tty,
            tty_rows: spec.tty_rows,
            tty_cols: spec.tty_cols,
            attached_user_at: None,
            attach_token: None,
            status: JobStatus::Starting.as_str().into(),
            exit_code: None,
            failure_kind: None,
            failure_detail: None,
            failure_extracted: None,
            started_at,
            finished_at: None,
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: log_path.to_string_lossy().to_string(),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        };
        Self::storage_err(self.repo.insert(&row).await)?;

        let ring = match RingFile::create(&log_path, 4 * 1024 * 1024).await {
            Ok(r) => r,
            Err(e) => {
                let _ = self.repo.delete(&id.0).await;
                return Err(JobError::Io(e));
            }
        };
        let cancel = CancellationToken::new();

        let handle = if spec.tty {
            let rows = spec.tty_rows.unwrap_or(24);
            let cols = spec.tty_cols.unwrap_or(80);
            match crate::spawner::spawn_pty(&self.sandbox, &spec.command, &spec.cwd, rows, cols) {
                Ok(h) => h,
                Err(e) => {
                    let _ = self.repo.delete(&id.0).await;
                    return Err(JobError::Spawn(e.to_string()));
                }
            }
        } else {
            match spawn_background_command(&self.sandbox, &spec.command, &spec.cwd) {
                Ok(h) => h,
                Err(e) => {
                    let _ = self.repo.delete(&id.0).await;
                    return Err(JobError::Spawn(e.to_string()));
                }
            }
        };
        let pgid = handle.pgid;

        let backend = match &handle.child {
            klynt_pty::ChildHandle::Process { .. } => ChildBackend::Process,
            klynt_pty::ChildHandle::Pty { master, .. } => {
                let writer = {
                    let m = master.clone();
                    let guard = m.lock().await;
                    guard.take_writer().map_err(|e| JobError::Spawn(format!("take_writer: {e}")))?
                };
                ChildBackend::Pty {
                    master: master.clone(),
                    writer: std::sync::Arc::new(std::sync::Mutex::new(Some(writer))),
                    rows: AtomicU16::new(spec.tty_rows.unwrap_or(24)),
                    cols: AtomicU16::new(spec.tty_cols.unwrap_or(80)),
                }
            }
        };

        let live = Arc::new(LiveJob {
            id: id.clone(),
            spec: spec.clone(),
            pgid,
            ring: ring.clone(),
            cancel: cancel.clone(),
            state: AtomicU8::new(STATE_RUNNING),
            started_at,
            backend,
            attach: Arc::new(RwLock::new(AttachState::default())),
        });
        self.jobs.insert(id.clone(), live.clone());

        // Spawn readers AFTER `live` exists so they can fan output to attach.ws_tx.
        let attach_for_readers = live.attach.clone();
        let mut stdout = handle.stdout;
        let stdout_ring = ring.clone();
        let stdout_cancel = cancel.clone();
        let stdout_attach = attach_for_readers.clone();
        tokio::spawn(async move {
            drain_reader_with_attach(&mut stdout, stdout_ring, stdout_cancel, stdout_attach).await
        });
        if let Some(mut stderr) = handle.stderr {
            let stderr_ring = ring.clone();
            let stderr_cancel = cancel.clone();
            let stderr_attach = attach_for_readers.clone();
            tokio::spawn(async move {
                drain_reader_with_attach(&mut stderr, stderr_ring, stderr_cancel, stderr_attach).await
            });
        }

        let supervisor = self.clone();
        let id_for_wait = id.clone();
        let child_handle = handle.child;
        tokio::spawn(async move {
            let exit = match child_handle {
                klynt_pty::ChildHandle::Process { mut child } => {
                    child.wait().await.map(|s| Some(s.code().unwrap_or(-1)))
                }
                klynt_pty::ChildHandle::Pty { child, .. } => {
                    // portable-pty's Child::wait() is blocking; offload.
                    tokio::task::spawn_blocking(move || {
                        let mut guard = child.blocking_lock();
                        guard.wait()
                    })
                    .await
                    .map_err(|e| std::io::Error::other(format!("wait join: {e}")))
                    .and_then(|res| {
                        res.map_err(|e| std::io::Error::other(e.to_string()))
                            .map(|s| Some(s.exit_code() as i32))
                    })
                }
            };
            supervisor.handle_exit(&id_for_wait, exit).await;
        });

        Self::storage_err(
            self.repo
                .update_status(
                    &id.0,
                    JobStatus::Running.as_str(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    0,
                    0,
                )
                .await,
        )?;

        self.bus.publish_bash_job(BashJobEvent::Started {
            job_id: id.0.clone(),
            thread_id: spec.session_id.clone(),
            agent_id: spec.agent_id.clone(),
            command: spec.command.clone(),
            description: spec.description.clone(),
            started_at,
        });
        self.queue.push(ContextUpdate {
            reason: ContextUpdateReason::CodingJobsChanged,
            content: None,
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        });

        Ok(JobView {
            id: id.clone(),
            session_id: spec.session_id,
            agent_id: spec.agent_id,
            description: spec.description,
            command: spec.command,
            cwd: spec.cwd,
            status: JobStatus::Running,
            started_at,
            finished_at: None,
            exit_code: None,
            gate_result: None,
            failure_extracted: None,
            total_bytes_emitted: 0,
            bisect_generation: 0,
            last_polled_at: None,
            last_seen_offset: 0,
        })
    }

    async fn output_delta(
        &self,
        id: &JobId,
        since: u64,
        block: bool,
        timeout_ms: u64,
    ) -> Result<RingRead, JobError> {
        if let Some(live) = self.jobs.get(id) {
            let ring = live.ring.clone();
            let mut rd = ring.read_delta(since).await?;
            if block && rd.bytes.is_empty() && live.state.load(Ordering::Acquire) == STATE_RUNNING {
                ring.wait_for_change(std::time::Duration::from_millis(timeout_ms))
                    .await;
                rd = ring.read_delta(since).await?;
            }
            self.update_poll_cursor(id, rd.new_offset).await?;
            return Ok(rd);
        }
        let row = Self::storage_err(self.repo.get(&id.0).await)?
            .ok_or_else(|| JobError::NotFound(id.0.clone()))?;
        let path = match (&row.final_path, &row.log_path) {
            (Some(f), _) if std::path::Path::new(f).exists() => f.clone(),
            (_, l) => l.clone(),
        };
        let bytes = tokio::fs::read(&path).await?;
        let total = bytes.len() as u64;
        let start = since.min(total) as usize;
        let end = (start + 50_000).min(bytes.len());
        Ok(RingRead {
            bytes: bytes[start..end].to_vec(),
            new_offset: end as u64,
            bisect_generation: 0,
            bisect_occurred_since: false,
            total_bytes_emitted: total,
        })
    }

    async fn stop(&self, id: &JobId, reason: &str) -> Result<JobView, JobError> {
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| JobError::NotFound(id.0.clone()))?;
        live.state.store(STATE_STOPPING, Ordering::Release);

        if let Some(pgid) = live.pgid {
            #[cfg(unix)]
            let _ = kill_process_group(pgid, libc::SIGTERM);
            #[cfg(unix)]
            {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = kill_process_group(pgid, libc::SIGKILL);
            }
        }
        live.cancel.cancel();

        let row = Self::storage_err(self.repo.get(&id.0).await)?
            .ok_or_else(|| JobError::NotFound(id.0.clone()))?;
        let mut view = view_from_row(row);
        view.status = JobStatus::Cancelled;
        tracing::info!(job_id=%id.0, reason, "job stopped by user request");
        Ok(view)
    }

    async fn list(
        &self,
        session_id: &str,
        agent_chain: &[String],
        active_only: bool,
    ) -> Vec<JobView> {
        let rows = self
            .repo
            .list_for_session(session_id, agent_chain, active_only)
            .await
            .unwrap_or_default();
        rows.into_iter().map(view_from_row).collect()
    }

    async fn write_stdin(&self, id: &JobId, data: &[u8]) -> Result<usize, JobError> {
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| JobError::NotFound(id.0.clone()))?;
        let writer = match &live.backend {
            ChildBackend::Process => return Err(JobError::NotPty),
            ChildBackend::Pty { writer, .. } => writer.clone(),
        };
        let bytes = data.to_vec();
        let n = bytes.len();
        let res = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let mut guard = writer.lock().unwrap();
            let w = guard
                .as_mut()
                .ok_or_else(|| std::io::Error::other("PTY writer not available"))?;
            std::io::Write::write_all(w, &bytes)?;
            std::io::Write::flush(w)?;
            Ok(())
        })
        .await
        .map_err(|e| JobError::Spawn(format!("join: {e}")))?;
        res.map_err(JobError::Io)?;
        Ok(n)
    }

    async fn resize(&self, id: &JobId, rows: u16, cols: u16) -> Result<(), JobError> {
        let rows = rows.clamp(4, 200);
        let cols = cols.clamp(20, 400);
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| JobError::NotFound(id.0.clone()))?;
        let (master, r_atom, c_atom) = match &live.backend {
            ChildBackend::Process => return Err(JobError::NotPty),
            ChildBackend::Pty { master, rows, cols, .. } => (master.clone(), rows, cols),
        };
        {
            let guard = master.lock().await;
            guard
                .resize(portable_pty::PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| JobError::Spawn(format!("resize: {e}")))?;
        }
        r_atom.store(rows, std::sync::atomic::Ordering::Relaxed);
        c_atom.store(cols, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn attach(&self, id: &JobId) -> Result<tools_core::AttachHandle, tools_core::AttachError> {
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| tools_core::AttachError::NotFound(id.0.clone()))?;
        if !live.backend.is_pty() {
            return Err(tools_core::AttachError::NotPty);
        }
        let token = crate::attach::generate_attach_token();
        match self
            .repo
            .mark_attached(id.as_str(), Some(&token))
            .await
        {
            Ok(_) => {}
            Err(storage::repos::AttachStorageError::AlreadyAttached) => {
                return Err(tools_core::AttachError::AlreadyAttached);
            }
            Err(e) => return Err(tools_core::AttachError::Storage(e.to_string())),
        }
        {
            let mut state = live.attach.write().await;
            state.user_at = Some(Timestamp::now());
            state.token = Some(token.clone());
        }
        let (rows, cols) = match &live.backend {
            ChildBackend::Pty { rows, cols, .. } => (
                rows.load(std::sync::atomic::Ordering::Relaxed),
                cols.load(std::sync::atomic::Ordering::Relaxed),
            ),
            _ => unreachable!(),
        };
        self.bus.publish_bash_job(BashJobEvent::AttachStarted {
            job_id: id.0.clone(),
            thread_id: live.spec.session_id.clone(),
            agent_id: live.spec.agent_id.clone(),
            timestamp: Timestamp::now(),
        });
        // Tail = last 4 KB of ring file, base64.
        let tail_b64 = self
            .read_ring_tail_b64(id, 4096)
            .await
            .map_err(|e| tools_core::AttachError::Io(e))?;
        Ok(tools_core::AttachHandle {
            ws_url: format!(
                "ws://localhost:3456/api/coding/jobs/{}/attach?token={}",
                id.as_str(),
                token
            ),
            rows,
            cols,
            tail_b64,
        })
    }

    async fn detach(&self, id: &JobId) -> Result<(), tools_core::AttachError> {
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| tools_core::AttachError::NotFound(id.0.clone()))?;
        let started_at = {
            let mut state = live.attach.write().await;
            let ts = state.user_at.take();
            state.token = None;
            state.ws_tx = None;
            ts
        };
        self.repo
            .clear_attached(id.as_str())
            .await
            .map_err(|e| tools_core::AttachError::Storage(e.to_string()))?;
        if let Some(ts) = started_at {
            let duration_ms = (Timestamp::now() - ts)
                .total(jiff::Unit::Millisecond)
                .unwrap_or(0.0) as u64;
            self.bus.publish_bash_job(BashJobEvent::AttachEnded {
                job_id: id.0.clone(),
                thread_id: live.spec.session_id.clone(),
                agent_id: live.spec.agent_id.clone(),
                timestamp: Timestamp::now(),
                duration_ms,
            });
        }
        Ok(())
    }

    async fn set_attach_channel(
        &self,
        id: &JobId,
        tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<(), tools_core::AttachError> {
        let live = self
            .jobs
            .get(id)
            .map(|e| e.value().clone())
            .ok_or_else(|| tools_core::AttachError::NotFound(id.0.clone()))?;
        live.attach.write().await.ws_tx = Some(tx);
        Ok(())
    }
}

fn drain_reader_with_attach<R: tokio::io::AsyncRead + Unpin + Send>(
    reader: &mut R,
    ring: Arc<RingFile>,
    cancel: CancellationToken,
    attach: Arc<RwLock<AttachState>>,
) -> impl std::future::Future<Output = std::io::Result<()>> + Send + use<'_, R> {
    async move {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 8192];
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    eprintln!("[drain] cancelled");
                    return Ok(());
                }
                n = reader.read(&mut buf) => match n {
                    Ok(0) => return Ok(()),
                    Ok(n) => {
                        ring.append(&buf[..n]).await?;
                        // Fork to attach WS if a live attachment exists.
                        let guard = attach.read().await;
                        if let Some(tx) = guard.ws_tx.as_ref() {
                            // UnboundedSender::send only fails on closed receiver;
                            // we drop in that case (the bridge will clean up on next call).
                            let _ = tx.send(buf[..n].to_vec());
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_core::JobSupervisorHandle;

    fn ephemeral_spec(tty: bool) -> JobSpec {
        JobSpec {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            agent_chain: vec!["a1".into()],
            description: "t".into(),
            command: "bash -c 'read x; echo got=$x'".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 5_000,
            silent_completion: true,
            tty,
            tty_rows: if tty { Some(24) } else { None },
            tty_cols: if tty { Some(80) } else { None },
        }
    }

    async fn build_supervisor() -> JobSupervisor {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let migration = crate::migrations::coding_background_jobs_migration();
        sqlx::query(&migration.sql).execute(pool.inner()).await.unwrap();
        let repo = BashJobRepo::new(pool.inner().clone());
        let bus = Arc::new(bus::DomainEventBus::new(256));
        let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
        let data_dir = tempfile::tempdir().unwrap().into_path();
        let sandbox = Arc::new(MacOsSeatbeltRunner::new());
        JobSupervisor::new(repo, bus, queue, data_dir, sandbox)
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn write_stdin_to_pty_job_echoes_input() {
        let sup = build_supervisor().await;
        let view = sup.spawn(ephemeral_spec(true)).await.expect("spawn");
        // Give the child a moment to call `read`.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let n = sup.write_stdin(&view.id, b"hello\n").await.expect("stdin");
        assert!(n >= 6, "expected at least 6 bytes, got {n}");
        // Wait for the child to finish + ring to drain.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let rd = sup
            .output_delta(&view.id, 0, false, 0)
            .await
            .expect("delta");
        let s = String::from_utf8_lossy(&rd.bytes);
        assert!(
            s.contains("got=hello"),
            "expected got=hello in output, got: {s:?}"
        );
    }

    #[tokio::test]
    async fn write_stdin_to_non_pty_job_errors() {
        let sup = build_supervisor().await;
        let view = sup.spawn(ephemeral_spec(false)).await.expect("spawn");
        let err = sup.write_stdin(&view.id, b"x").await;
        assert!(matches!(err, Err(JobError::NotPty)));
    }
}

fn view_from_row(row: BashJobRow) -> JobView {
    let status = JobStatus::parse(&row.status).unwrap_or_else(|| {
        tracing::warn!(status=%row.status, "unknown job status in DB");
        JobStatus::Running
    });
    let extracted = row
        .failure_extracted
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    let failure_kind = row.failure_kind.as_deref().map(parse_failure_kind);
    let gate_result = if status == JobStatus::Completed && row.exit_code == Some(0) {
        Some(GateResult::Passed)
    } else if status.is_terminal() {
        let kind = failure_kind
            .clone()
            .unwrap_or_else(|| FailureKind::Other("unknown".into()));
        let detail = row.failure_detail.clone().unwrap_or_default();
        Some(GateResult::Failed {
            kind,
            detail,
            extracted: extracted.clone().unwrap_or_default(),
        })
    } else {
        None
    };
    JobView {
        id: JobId(row.id),
        session_id: row.session_id,
        agent_id: row.agent_id,
        description: row.description,
        command: row.command,
        cwd: row.cwd.into(),
        status,
        started_at: row.started_at,
        finished_at: row.finished_at,
        exit_code: row.exit_code,
        gate_result,
        failure_extracted: extracted,
        total_bytes_emitted: row.total_bytes_emitted as u64,
        bisect_generation: row.bisect_count as u64,
        last_polled_at: row.last_polled_at,
        last_seen_offset: row.last_seen_offset as u64,
    }
}
