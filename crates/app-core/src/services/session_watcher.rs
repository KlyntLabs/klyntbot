use chrono::Utc;
use feature_session_tracker::discovery;
use feature_session_tracker::repos::SessionTrackerRepos;
use feature_session_tracker::types::{SessionStatus, TrackedSession};
use feature_session_tracker::watcher::{SessionWatcher, WatchEvent};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Domain events emitted by the session watcher service.
#[derive(Debug, Clone)]
pub enum SessionWatcherEvent {
    NewSession {
        session: TrackedSession,
    },
    StatusChanged {
        session_id: String,
        status: SessionStatus,
    },
}

const STATUS_TICK_SECS: u64 = 60;

/// Start the session watcher service.
///
/// Returns `None` if `~/.claude` doesn't exist or the watcher fails to start.
/// The caller receives events via the returned `mpsc::Receiver`.
pub fn start(
    repos: SessionTrackerRepos,
    shutdown: CancellationToken,
) -> Option<mpsc::Receiver<SessionWatcherEvent>> {
    let claude_dir = discovery::default_claude_dir()?;
    let projects_dir = claude_dir.join("projects");

    if !projects_dir.exists() {
        warn!(
            "Claude projects dir does not exist: {}",
            projects_dir.display()
        );
        return None;
    }

    let (watch_tx, watch_rx) = mpsc::unbounded_channel::<WatchEvent>();

    let watcher = match SessionWatcher::start(&projects_dir, watch_tx) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to start session watcher: {e}");
            return None;
        }
    };

    let (event_tx, event_rx) = mpsc::channel::<SessionWatcherEvent>(64);

    tokio::spawn(run_service(
        watcher, watch_rx, repos, claude_dir, event_tx, shutdown,
    ));

    Some(event_rx)
}

async fn run_service(
    watcher: SessionWatcher,
    mut watch_rx: mpsc::UnboundedReceiver<WatchEvent>,
    repos: SessionTrackerRepos,
    claude_dir: PathBuf,
    event_tx: mpsc::Sender<SessionWatcherEvent>,
    shutdown: CancellationToken,
) {
    // Initial discovery — populate DB and set watcher offsets.
    let discovered = discovery::discover_sessions(&claude_dir).await;
    let jsonl_paths: Vec<PathBuf> = discovered
        .iter()
        .map(|s| PathBuf::from(&s.jsonl_path))
        .collect();
    watcher.init_offsets(&jsonl_paths);

    for session in &discovered {
        if let Err(e) = repos.upsert_session(session).await {
            warn!(
                "Failed to upsert discovered session {}: {e}",
                session.session_id
            );
        }
    }
    info!(
        "Session watcher: initial discovery found {} sessions",
        discovered.len()
    );

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(STATUS_TICK_SECS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // `watcher` must stay alive — dropping it stops the notify watcher.
    // It was already moved into this function; the select loop keeps it in scope.

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Session watcher service shutting down");
                break;
            }
            event = watch_rx.recv() => {
                let Some(event) = event else { break };
                handle_watch_event(&event, &repos, &event_tx, &claude_dir).await;
            }
            _ = tick.tick() => {
                check_status_transitions(&repos, &event_tx).await;
            }
        }
    }
    // `watcher` dropped here — notify watcher stops.
    drop(watcher);
}

async fn handle_watch_event(
    event: &WatchEvent,
    repos: &SessionTrackerRepos,
    event_tx: &mpsc::Sender<SessionWatcherEvent>,
    claude_dir: &Path,
) {
    match event {
        WatchEvent::NewSession {
            session_id,
            jsonl_path,
        } => {
            info!("New session detected: {session_id}");
            // Discover just this session's metadata from the jsonl_path rather than
            // rescanning the entire projects directory.
            let discovered = discovery::discover_sessions(claude_dir).await;
            if let Some(session) = discovered.into_iter().find(|s| s.session_id == *session_id) {
                if let Err(e) = repos.upsert_session(&session).await {
                    warn!("Failed to upsert new session {session_id}: {e}");
                    return;
                }
                let _ = event_tx
                    .send(SessionWatcherEvent::NewSession { session })
                    .await;
            } else {
                // Fallback: build a minimal session from the path alone.
                let project_path = jsonl_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| n.replace('-', "/"))
                    .unwrap_or_default();
                let project_name = project_path
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let session = TrackedSession {
                    session_id: session_id.clone(),
                    project_path,
                    project_name,
                    jsonl_path: jsonl_path.to_string_lossy().to_string(),
                    status: SessionStatus::Active,
                    first_message_preview: None,
                    message_count: 0,
                    git_branch: None,
                    last_activity: Some(Utc::now()),
                    created_at: Utc::now(),
                };
                if let Err(e) = repos.upsert_session(&session).await {
                    warn!("Failed to upsert new session {session_id}: {e}");
                    return;
                }
                let _ = event_tx
                    .send(SessionWatcherEvent::NewSession { session })
                    .await;
            }
        }
        WatchEvent::FileModified { session_id } => {
            // Always refresh last_activity so the status tick has accurate timestamps.
            let _ = repos.increment_message_count(session_id).await;

            // Reactivate if session was previously Idle or Completed.
            if let Ok(Some(session)) = repos.get_session(session_id).await {
                if session.status != SessionStatus::Active {
                    if let Err(e) = repos
                        .update_session_status(session_id, &SessionStatus::Active)
                        .await
                    {
                        warn!("Failed to reactivate session {session_id}: {e}");
                        return;
                    }
                    let _ = event_tx
                        .send(SessionWatcherEvent::StatusChanged {
                            session_id: session_id.clone(),
                            status: SessionStatus::Active,
                        })
                        .await;
                }
            }
        }
        WatchEvent::NewMessage { session_id, .. } => {
            let _ = repos.increment_message_count(session_id).await;
        }
    }
}

async fn check_status_transitions(
    repos: &SessionTrackerRepos,
    event_tx: &mpsc::Sender<SessionWatcherEvent>,
) {
    let live_sessions = match repos
        .list_sessions_by_status(&[SessionStatus::Active, SessionStatus::Idle])
        .await
    {
        Ok(s) => s,
        Err(e) => {
            warn!("Status tick: failed to query live sessions: {e}");
            return;
        }
    };

    let now = Utc::now();
    for session in live_sessions {
        let idle_secs = session
            .last_activity
            .map(|t| (now - t).num_seconds())
            .unwrap_or(i64::MAX);

        // Use the shared thresholds from SessionStatus.
        let expected = SessionStatus::from_idle_secs(idle_secs);
        if expected != session.status {
            if let Err(e) = repos
                .update_session_status(&session.session_id, &expected)
                .await
            {
                warn!("Status tick: failed to update {}: {e}", session.session_id);
                continue;
            }
            let _ = event_tx
                .send(SessionWatcherEvent::StatusChanged {
                    session_id: session.session_id,
                    status: expected,
                })
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use feature_session_tracker::types::SessionStatus;
    use storage::StoragePool;
    use tokio::sync::mpsc;

    async fn setup_repos() -> SessionTrackerRepos {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &feature_session_tracker::SessionTrackerFeature::migrations_static(),
        )
        .await
        .unwrap();
        SessionTrackerRepos::new(pool.inner().clone())
    }

    fn make_session(
        id: &str,
        status: SessionStatus,
        last_activity: DateTime<Utc>,
    ) -> TrackedSession {
        TrackedSession {
            session_id: id.to_string(),
            project_path: "/test".to_string(),
            project_name: "test".to_string(),
            jsonl_path: format!("/tmp/{id}.jsonl"),
            status,
            first_message_preview: None,
            message_count: 0,
            git_branch: None,
            last_activity: Some(last_activity),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn active_session_becomes_idle_after_threshold() {
        let repos = setup_repos().await;
        let old = Utc::now() - chrono::Duration::seconds(SessionStatus::ACTIVE_THRESHOLD_SECS + 5);
        repos
            .upsert_session(&make_session("s1", SessionStatus::Active, old))
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        let event = rx.try_recv().unwrap();
        match event {
            SessionWatcherEvent::StatusChanged { session_id, status } => {
                assert_eq!(session_id, "s1");
                assert_eq!(status, SessionStatus::Idle);
            }
            _ => panic!("expected StatusChanged"),
        }
    }

    #[tokio::test]
    async fn idle_session_becomes_completed_after_threshold() {
        let repos = setup_repos().await;
        let old = Utc::now() - chrono::Duration::seconds(SessionStatus::IDLE_THRESHOLD_SECS + 5);
        repos
            .upsert_session(&make_session("s1", SessionStatus::Idle, old))
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        let event = rx.try_recv().unwrap();
        match event {
            SessionWatcherEvent::StatusChanged { session_id, status } => {
                assert_eq!(session_id, "s1");
                assert_eq!(status, SessionStatus::Completed);
            }
            _ => panic!("expected StatusChanged"),
        }
    }

    #[tokio::test]
    async fn recent_active_session_stays_active() {
        let repos = setup_repos().await;
        let recent = Utc::now() - chrono::Duration::seconds(5);
        repos
            .upsert_session(&make_session("s1", SessionStatus::Active, recent))
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        assert!(rx.try_recv().is_err(), "no transition expected");
    }
}
