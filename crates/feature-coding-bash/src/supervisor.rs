//! In-memory live-job registry + SQLite persistence.
//! Spec §5.1.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
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
use tokio_util::sync::CancellationToken;
use tools_core::{
    FailureKind, GateResult, JobError, JobId, JobSpec, JobStatus,
    JobSupervisorHandle, JobView, RingRead,
};
use tracing;

use crate::gate::GateClassifier;
use crate::ring::RingFile;
use crate::spawner::spawn_background_command;

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

    fn storage_err<T>(r: Result<T, storage::error::StorageError>) -> Result<T, JobError> {
        r.map_err(|e| JobError::Storage(e.to_string()))
    }

    fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }

    fn log_path(&self, id: &JobId) -> PathBuf {
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
            if let Err(e) = self.stop(&id, "thread deleted").await {
                tracing::warn!(job_id=%id.0, "reap_session stop failed: {}", e);
            }
        }
        Ok(n)
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
                    tracing::debug!("failed to remove tmp file {}: {}", entry.path().display(), e);
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
                    tracing::debug!("failed to remove orphan file {}: {}", entry.path().display(), e);
                }
            }
        }
        Ok(())
    }

    async fn handle_exit(
        &self,
        id: &JobId,
        exit: std::io::Result<std::process::ExitStatus>,
    ) {
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

        let exit_code = exit.as_ref().ok().and_then(|s| s.code()).unwrap_or(-1);
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
            GateResult::Passed => (JobStatus::Completed.as_str(), None, None, None),
            GateResult::Failed {
                kind,
                detail,
                extracted,
            } => {
                let k = kind.as_db_str();
                (
                    JobStatus::Failed.as_str(),
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
                status_str,
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
            let body = crate::render::completion_notification(id, &live.spec, &result, &final_str);
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
            self.repo.count_active_for_chain(&spec.session_id, chain).await,
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
            cwd: cwd_str.clone(),
            timeout_ms: spec.timeout_ms as i64,
            silent_completion: spec.silent_completion,
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

        let handle = match spawn_background_command(&self.sandbox, &spec.command, &spec.cwd) {
            Ok(h) => h,
            Err(e) => {
                let _ = self.repo.delete(&id.0).await;
                return Err(JobError::Spawn(e.to_string()));
            }
        };
        let pgid = handle.pgid;

        let stdout_ring = ring.clone();
        let stdout_cancel = cancel.clone();
        let mut stdout = handle.stdout;
        tokio::spawn(async move { drain_reader(&mut stdout, stdout_ring, stdout_cancel).await });
        if let Some(mut stderr) = handle.stderr {
            let stderr_ring = ring.clone();
            let stderr_cancel = cancel.clone();
            tokio::spawn(async move { drain_reader(&mut stderr, stderr_ring, stderr_cancel).await });
        }

        let live = Arc::new(LiveJob {
            id: id.clone(),
            spec: spec.clone(),
            pgid,
            ring: ring.clone(),
            cancel: cancel.clone(),
            state: AtomicU8::new(STATE_RUNNING),
            started_at,
        });
        self.jobs.insert(id.clone(), live.clone());

        let supervisor = self.clone();
        let id_for_wait = id.clone();
        let child_handle = handle.child;
        tokio::spawn(async move {
            let exit = match child_handle {
                klynt_pty::ChildHandle::Process { mut child } => child.wait().await,
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
            if block && rd.bytes.is_empty() && live.state.load(Ordering::Acquire) == STATE_RUNNING
            {
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
}

fn drain_reader<R: tokio::io::AsyncRead + Unpin + Send>(
    reader: &mut R,
    ring: Arc<RingFile>,
    cancel: CancellationToken,
) -> impl std::future::Future<Output = std::io::Result<()>> + Send + use<'_, R> {
    async move {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 8192];
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Ok(()),
                n = reader.read(&mut buf) => match n? {
                    0 => return Ok(()),
                    n => {
                        ring.append(&buf[..n]).await?;
                    }
                }
            }
        }
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
        let kind = failure_kind.clone().unwrap_or_else(|| FailureKind::Other("unknown".into()));
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
