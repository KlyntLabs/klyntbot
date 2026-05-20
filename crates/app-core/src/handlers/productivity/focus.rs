//! Focus session handlers — start, end, status, pomodoro, breaks.

use desktop_shared::commands::{FocusSessionResponse, IntelligenceSessionResponse};
use desktop_shared::errors::ApiError;

use super::converters::session_to_response;
use crate::errors::{map_prod_err, parse_date_or_err, parse_local_day_range};
use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
    pub async fn productivity_focus_status(
        &self,
    ) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.get_active().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn productivity_sessions(
        &self,
        date: String,
    ) -> Result<Vec<FocusSessionResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let start = parse_date_or_err(&date)?;
        let end = start
            .checked_add(jiff::SignedDuration::from_secs(86400))
            .unwrap_or(start);
        let sessions = repos
            .sessions
            .list_range(&start, &end, None)
            .await
            .map_err(map_prod_err)?;
        Ok(sessions.into_iter().map(session_to_response).collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn productivity_intelligence_sessions(
        &self,
        date: String,
        tz_offset_mins: Option<i32>,
    ) -> Result<Vec<IntelligenceSessionResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let (start, end) = parse_local_day_range(&date, tz_offset_mins)?;
        let start_str = start.to_string();
        let end_str = end.to_string();
        let next_date = end
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date()
            .strftime("%Y-%m-%d")
            .to_string();

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

    #[tracing::instrument(skip(self), err)]
    pub async fn productivity_pomodoro_start(
        &self,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        self.productivity_pomodoro_start_with_action(None, None, work_mins, break_mins)
            .await
    }

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
    pub async fn productivity_break_end(&self) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.end_break_session().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

}
