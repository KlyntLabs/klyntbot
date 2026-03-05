//! Focus session manager — start/end sessions, record distractions, compute quality.

use chrono::Utc;
use uuid::Uuid;

use crate::config::FocusConfig;
use crate::repos::ProductivityRepos;
use crate::types::{DistractionEvent, FocusSession, SessionType};

pub struct FocusManager {
    repos: ProductivityRepos,
    config: FocusConfig,
}

impl FocusManager {
    pub fn new(repos: ProductivityRepos, config: FocusConfig) -> Self {
        Self { repos, config }
    }

    /// Start a new focus session. Fails if one is already active.
    pub async fn start_session(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        target_mins: Option<i64>,
    ) -> common::Result<FocusSession> {
        // Check for existing active session
        if let Some(active) = self.repos.sessions.get_active().await? {
            return Err(common::ToolError::ExecutionFailed(format!(
                "Focus session already active (id: {}, started: {})",
                active.id,
                active.started_at.format("%H:%M")
            ))
            .into());
        }

        let target = target_mins.unwrap_or(self.config.default_duration_mins as i64);
        let session = FocusSession {
            id: Uuid::new_v4().to_string(),
            action_id,
            project_id,
            session_type: SessionType::Focus,
            target_mins: Some(target),
            started_at: Utc::now(),
            ended_at: None,
            actual_mins: None,
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: false,
            notes: None,
        };

        self.repos.sessions.create(&session).await?;
        Ok(session)
    }

    /// End the active focus session, computing quality score.
    pub async fn end_session(&self, notes: Option<String>) -> common::Result<Option<FocusSession>> {
        let Some(mut session) = self.repos.sessions.get_active().await? else {
            return Ok(None);
        };

        let now = Utc::now();
        let actual_mins = (now - session.started_at).num_minutes();
        let target = session
            .target_mins
            .unwrap_or(self.config.default_duration_mins as i64);

        session.ended_at = Some(now);
        session.actual_mins = Some(actual_mins);
        session.completed = actual_mins >= target;
        session.notes = notes;

        // Compute quality score
        let on_task_ratio = compute_on_task_ratio(&session);
        session.quality_score = Some(compute_quality(
            &session,
            on_task_ratio,
            self.config.default_duration_mins,
        ));

        self.repos.sessions.update(&session).await?;
        Ok(Some(session))
    }

    /// Start a Pomodoro session (work period).
    pub async fn start_pomodoro(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> common::Result<FocusSession> {
        if let Some(active) = self.repos.sessions.get_active().await? {
            return Err(common::ToolError::ExecutionFailed(format!(
                "Session already active (id: {}, type: {})",
                active.id, active.session_type
            ))
            .into());
        }

        let work = work_mins.unwrap_or(25);
        let session = FocusSession {
            id: Uuid::new_v4().to_string(),
            action_id,
            project_id,
            session_type: SessionType::Pomodoro,
            target_mins: Some(work),
            started_at: Utc::now(),
            ended_at: None,
            actual_mins: None,
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: false,
            notes: break_mins.map(|b| format!("break_mins:{b}")),
        };

        self.repos.sessions.create(&session).await?;
        Ok(session)
    }

    /// Get the currently active focus session, if any.
    pub async fn get_active(&self) -> common::Result<Option<FocusSession>> {
        self.repos.sessions.get_active().await
    }

    /// Record a distraction during an active focus session.
    pub async fn record_distraction(&self, app_name: &str) -> common::Result<()> {
        let Some(session) = self.repos.sessions.get_active().await? else {
            return Ok(());
        };
        self.record_distraction_for(session, app_name).await
    }

    /// Record a distraction against a known session (avoids redundant DB lookup).
    pub async fn record_distraction_for(
        &self,
        mut session: FocusSession,
        app_name: &str,
    ) -> common::Result<()> {
        session.interruptions += 1;
        session.distraction_events.push(DistractionEvent {
            timestamp: Utc::now(),
            app_name: app_name.to_string(),
            duration_secs: None,
        });

        self.repos.sessions.update(&session).await?;
        Ok(())
    }
}

/// Estimate on-task ratio from distraction events.
/// Simple heuristic: each distraction costs ~2 minutes of focus time.
fn compute_on_task_ratio(session: &FocusSession) -> f64 {
    let elapsed_mins = session
        .actual_mins
        .unwrap_or_else(|| (Utc::now() - session.started_at).num_minutes())
        .max(1) as f64;

    let distraction_mins = session
        .distraction_events
        .iter()
        .map(|d| d.duration_secs.unwrap_or(120) as f64 / 60.0)
        .sum::<f64>();

    ((elapsed_mins - distraction_mins) / elapsed_mins).clamp(0.0, 1.0)
}

/// Quality score formula:
/// quality = (on_task_ratio * 0.6) + (focus_continuity * 0.3) + (completion_bonus * 0.1)
///
/// `default_target_mins` is used when `session.target_mins` is `None`.
pub fn compute_quality(
    session: &FocusSession,
    on_task_ratio: f64,
    default_target_mins: u64,
) -> f64 {
    let target = session.target_mins.unwrap_or(default_target_mins as i64) as f64;
    let actual = session.actual_mins.unwrap_or(0) as f64;

    // focus_continuity = 1.0 - (interruptions / expected_interruptions)
    let expected_interruptions = (target / 15.0).max(1.0);
    let focus_continuity =
        (1.0 - (session.interruptions as f64 / expected_interruptions)).clamp(0.0, 1.0);

    // completion_bonus
    let completion_bonus = if session.completed {
        1.0
    } else if actual >= 0.8 * target {
        0.5
    } else {
        0.0
    };

    let quality = (on_task_ratio * 0.6) + (focus_continuity * 0.3) + (completion_bonus * 0.1);
    (quality * 100.0).round() / 100.0 // Round to 2 decimal places
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::ProductivityRepos;
    use crate::ProductivityFeature;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    #[tokio::test]
    async fn test_start_focus_session() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let mgr = FocusManager::new(repos, FocusConfig::default());

        let session = mgr.start_session(None, None, Some(25)).await.unwrap();
        assert_eq!(session.target_mins, Some(25));
        assert!(session.ended_at.is_none());
        assert_eq!(session.interruptions, 0);
    }

    #[tokio::test]
    async fn test_cannot_start_two_sessions() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let mgr = FocusManager::new(repos, FocusConfig::default());

        mgr.start_session(None, None, None).await.unwrap();
        let result = mgr.start_session(None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_end_focus_session_calculates_quality() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let mgr = FocusManager::new(repos, FocusConfig::default());

        mgr.start_session(None, None, Some(1)).await.unwrap(); // 1 min target
        let ended = mgr.end_session(Some("good session".into())).await.unwrap();
        let session = ended.unwrap();

        assert!(session.ended_at.is_some());
        assert!(session.quality_score.is_some());
        assert!(session.notes.as_deref() == Some("good session"));
    }

    #[tokio::test]
    async fn test_record_distraction() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let mgr = FocusManager::new(repos, FocusConfig::default());

        mgr.start_session(None, None, None).await.unwrap();
        mgr.record_distraction("Safari").await.unwrap();
        mgr.record_distraction("Slack").await.unwrap();

        let active = mgr.get_active().await.unwrap().unwrap();
        assert_eq!(active.interruptions, 2);
        assert_eq!(active.distraction_events.len(), 2);
        assert_eq!(active.distraction_events[0].app_name, "Safari");
    }

    #[tokio::test]
    async fn test_quality_score_formula() {
        // Perfect session: no distractions, completed
        let session = FocusSession {
            id: "test".into(),
            action_id: None,
            project_id: None,
            session_type: SessionType::Focus,
            target_mins: Some(45),
            started_at: Utc::now(),
            ended_at: None,
            actual_mins: Some(45),
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: true,
            notes: None,
        };
        let quality = compute_quality(&session, 1.0, 45);
        // 1.0 * 0.6 + 1.0 * 0.3 + 1.0 * 0.1 = 1.0
        assert!((quality - 1.0).abs() < 0.01);

        // Session with 3 interruptions, not completed, 50% on-task
        let session2 = FocusSession {
            interruptions: 3,
            completed: false,
            actual_mins: Some(20),
            ..session.clone()
        };
        let quality2 = compute_quality(&session2, 0.5, 45);
        // 0.5 * 0.6 + (1 - 3/3) * 0.3 + 0.0 * 0.1 = 0.3
        assert!((quality2 - 0.3).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_end_nonexistent_session_returns_none() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let mgr = FocusManager::new(repos, FocusConfig::default());

        let result = mgr.end_session(None).await.unwrap();
        assert!(result.is_none());
    }
}
