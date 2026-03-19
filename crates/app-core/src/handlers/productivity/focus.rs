//! Focus session handlers — start, end, status, pomodoro, breaks, auto-focus.

use chrono::Utc;
use desktop_shared::commands::{FocusSessionResponse, IntelligenceSessionResponse};
use desktop_shared::errors::ApiError;
use desktop_shared::events::AutoFocusPayload;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::types::FocusSession;

use super::converters::session_to_response;
use crate::errors::{map_prod_err, parse_date_or_err, parse_local_day_range, parse_rfc3339_or_err};
use crate::state::AppCore;

impl AppCore {
    pub async fn productivity_focus_start(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        target_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_session(action_id, project_id, target_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_focus_end(
        &self,
        notes: Option<String>,
    ) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.end_session(notes).await.map_err(map_prod_err)?;

        // Clear interceptor session state (whitelist + temp passes)
        if let Ok(interceptor) = self.distraction_interceptor() {
            let mut guard = interceptor.lock().await;
            guard.reset_session();
        }

        Ok(session.map(session_to_response))
    }

    pub async fn productivity_focus_status(
        &self,
    ) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.get_active().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

    pub async fn productivity_sessions(
        &self,
        date: String,
    ) -> Result<Vec<FocusSessionResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let start = parse_date_or_err(&date)?;
        let end = start + chrono::Duration::days(1);
        let sessions = repos
            .sessions
            .list_range(&start, &end, None)
            .await
            .map_err(map_prod_err)?;
        Ok(sessions.into_iter().map(session_to_response).collect())
    }

    pub async fn productivity_intelligence_sessions(
        &self,
        date: String,
        tz_offset_mins: Option<i32>,
    ) -> Result<Vec<IntelligenceSessionResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let (start, end) = parse_local_day_range(&date, tz_offset_mins)?;
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();
        let next_date = end.date_naive().format("%Y-%m-%d").to_string();

        // Fetch sessions and quality scores in parallel
        let (sessions_res, scores_res) = tokio::join!(
            repos.intelligence_sessions.list_range(&start_str, &end_str),
            repos.quality_scores.list_range(&date, &next_date),
        );
        let sessions = sessions_res.map_err(map_prod_err)?;
        let scores = scores_res.map_err(map_prod_err)?;
        let score_map: std::collections::HashMap<String, f64> = scores
            .into_iter()
            .filter_map(|s| s.session_id.map(|sid| (sid, s.overall_score)))
            .collect();

        Ok(sessions
            .into_iter()
            .map(|s| {
                let quality = s.quality_score.or_else(|| score_map.get(&s.id).copied());
                let title = s.tags.clone().unwrap_or_else(|| s.fallback_title());
                let description = s.notes.clone().or_else(|| s.fallback_description(quality));
                IntelligenceSessionResponse {
                    id: s.id,
                    session_type: s.session_type,
                    started_at: s.started_at,
                    ended_at: s.ended_at,
                    duration_secs: s.duration_secs,
                    dominant_category: s.dominant_category,
                    category_purity: s.category_purity,
                    quality_score: quality,
                    title: Some(title),
                    description,
                    app_breakdown: s.app_breakdown,
                    context_switches: s.context_switches,
                    distraction_count: s.distraction_count,
                    source: s.source,
                }
            })
            .collect())
    }

    pub async fn productivity_pomodoro_start(
        &self,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        self.productivity_pomodoro_start_with_action(None, None, work_mins, break_mins)
            .await
    }

    pub async fn productivity_pomodoro_start_with_action(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_pomodoro(action_id, project_id, work_mins, break_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_break_start(
        &self,
        break_mins: i64,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_break_session(break_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_break_end(&self) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.end_break_session().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

    /// Create or get an active auto-detected focus session (called when auto-focus starts).
    pub async fn productivity_auto_focus_start(&self) -> Result<FocusSessionResponse, ApiError> {
        let repos = self.productivity_repos()?;

        // Check if there's already an active auto-detected session
        if let Ok(Some(active)) = repos.sessions.get_active().await {
            if active.source == feature_productivity::types::SessionSource::AutoDetected
                && active.ended_at.is_none()
            {
                return Ok(session_to_response(active));
            }
        }

        // Create a new active session (no end time yet)
        let focus_session = FocusSession {
            id: uuid::Uuid::new_v4().to_string(),
            action_id: None,
            project_id: None,
            session_type: feature_productivity::types::SessionType::Focus,
            target_mins: None,
            started_at: Utc::now(),
            ended_at: None, // Still active
            actual_mins: None,
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: false,
            notes: Some("Auto-detected focus session".to_string()),
            source: feature_productivity::types::SessionSource::AutoDetected,
        };
        repos
            .sessions
            .create(&focus_session)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(focus_session))
    }

    /// End the active auto-detected session with final stats (called when auto-focus ends).
    pub async fn productivity_auto_focus_end(
        &self,
        event: AutoFocusEvent,
    ) -> Result<FocusSessionResponse, ApiError> {
        let repos = self.productivity_repos()?;

        // Find the active auto-detected session
        let active = repos
            .sessions
            .get_active()
            .await
            .map_err(map_prod_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "No active focus session"))?;

        if active.source != feature_productivity::types::SessionSource::AutoDetected {
            return Err(ApiError::new(
                "INVALID_STATE",
                "Active session is not auto-detected",
            ));
        }

        // Extract stats from the Ended event
        let (started_at, ended_at, productive_ratio, total_secs) = match event {
            AutoFocusEvent::Ended {
                started_at,
                ended_at,
                productive_ratio,
                total_secs,
                ..
            } => (started_at, ended_at, productive_ratio, total_secs),
            _ => {
                return Err(ApiError::new(
                    "INVALID_EVENT",
                    "Expected Ended event for auto-focus end",
                ))
            }
        };

        let actual_mins = total_secs / 60;

        // Update the session with final stats
        let mut updated = active.clone();
        updated.started_at = started_at;
        updated.ended_at = Some(ended_at);
        updated.actual_mins = Some(actual_mins);
        updated.quality_score = Some(productive_ratio);
        updated.completed = true;

        repos
            .sessions
            .update(&updated)
            .await
            .map_err(map_prod_err)?;

        Ok(session_to_response(updated))
    }

    /// Confirm and persist a completed auto-detected focus session.
    /// Called from the UI toast after the FSM has already ended the session.
    pub async fn productivity_auto_focus_confirm(
        &self,
        payload: AutoFocusPayload,
    ) -> Result<FocusSessionResponse, ApiError> {
        let repos = self.productivity_repos()?;

        let started_at = parse_rfc3339_or_err("started_at", &payload.started_at)?;
        let ended_at = parse_rfc3339_or_err("ended_at", &payload.ended_at)?;

        let session = FocusSession {
            id: uuid::Uuid::new_v4().to_string(),
            action_id: None,
            project_id: None,
            session_type: feature_productivity::types::SessionType::Focus,
            target_mins: None,
            started_at,
            ended_at: Some(ended_at),
            actual_mins: Some(payload.duration_mins),
            interruptions: 0,
            distraction_events: vec![],
            quality_score: Some(payload.productive_ratio),
            completed: true,
            notes: Some(format!(
                "Auto-detected: {} ({}min)",
                payload.dominant_app, payload.duration_mins
            )),
            source: feature_productivity::types::SessionSource::AutoDetected,
        };

        repos
            .sessions
            .create(&session)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }
}
