//! Productivity domain → response converters.

use desktop_shared::commands::{
    ActivityTimelineResponse, CategoryRulesResponse, FocusSessionResponse, InsightCardResponse,
    ProductivityProjectResponse, ProductivitySummaryResponse, WeeklyAssessmentResponse,
};
use desktop_shared::commands::{AppUsageResponse, CategoryUsageResponse, ProjectUsageResponse};
use feature_productivity::types::{DailySummary, FocusSession, InsightCard};

pub(super) fn rules_to_response(
    r: feature_productivity::types::CategoryRules,
) -> CategoryRulesResponse {
    CategoryRulesResponse {
        app_names: r.app_names,
        bundle_ids: r.bundle_ids,
        url_patterns: r.url_patterns,
    }
}

pub(super) fn rules_from_response(
    r: CategoryRulesResponse,
) -> feature_productivity::types::CategoryRules {
    feature_productivity::types::CategoryRules {
        app_names: r.app_names,
        bundle_ids: r.bundle_ids,
        url_patterns: r.url_patterns,
    }
}

pub fn summary_to_response(s: DailySummary) -> ProductivitySummaryResponse {
    ProductivitySummaryResponse {
        date: s.date,
        total_active_secs: s.total_active_secs,
        total_focus_secs: s.total_focus_secs,
        total_break_secs: s.total_break_secs,
        total_idle_secs: s.total_idle_secs,
        productive_secs: s.productive_secs,
        neutral_secs: s.neutral_secs,
        distracting_secs: s.distracting_secs,
        focus_sessions_count: s.focus_sessions_count,
        avg_session_quality: s.avg_session_quality,
        interruptions_count: s.interruptions_count,
        context_switches: s.context_switches,
        top_apps: s
            .top_apps
            .into_iter()
            .map(|a| AppUsageResponse {
                app_name: a.app_name,
                duration_secs: a.duration_secs,
                category: a.category,
            })
            .collect(),
        top_categories: s
            .top_categories
            .into_iter()
            .map(|c| CategoryUsageResponse {
                category_id: c.category_id,
                category: c.category,
                category_type: c.category_type,
                duration_secs: c.duration_secs,
            })
            .collect(),
        top_projects: s
            .top_projects
            .into_iter()
            .map(|p| ProjectUsageResponse {
                project_id: p.project_id,
                display_name: p.display_name,
                duration_secs: p.duration_secs,
                color: p.color,
            })
            .collect(),
        ai_summary: s.ai_summary,
        productivity_score: s.productivity_score,
        score_trend: None,
        focus_time_trend: None,
        active_time_trend: None,
        deep_work_blocks: s.deep_work_blocks,
        deep_work_secs: s.deep_work_secs,
        avg_recovery_secs: s.avg_recovery_secs,
    }
}

pub fn session_to_response(s: FocusSession) -> FocusSessionResponse {
    FocusSessionResponse {
        id: s.id,
        action_id: s.action_id,
        project_id: s.project_id,
        session_type: s.session_type.to_string(),
        target_mins: s.target_mins,
        started_at: common::time::bridge::chrono_to_jiff(s.started_at),
        ended_at: s.ended_at.map(common::time::bridge::chrono_to_jiff),
        actual_mins: s.actual_mins,
        interruptions: s.interruptions,
        quality_score: s.quality_score,
        completed: s.completed,
        notes: s.notes,
    }
}

pub fn project_to_response(
    p: feature_productivity::types::ProductivityProject,
) -> ProductivityProjectResponse {
    ProductivityProjectResponse {
        id: p.id,
        display_name: p.display_name,
        path: p.path,
        url_patterns: p.url_patterns,
        color: p.color,
        is_auto_detected: p.is_auto_detected,
    }
}

pub(super) fn assessment_to_response(
    a: feature_productivity::types::WeeklyAssessment,
) -> WeeklyAssessmentResponse {
    WeeklyAssessmentResponse {
        id: a.id,
        week_start: a.week_start,
        week_end: a.week_end,
        avg_score: a.avg_score,
        total_focus_mins: a.total_focus_mins,
        total_productive_secs: a.total_productive_secs,
        total_distracting_secs: a.total_distracting_secs,
        top_apps: a.top_apps,
        summary: a.summary,
    }
}

pub fn insight_to_response(c: InsightCard) -> InsightCardResponse {
    InsightCardResponse {
        id: c.id,
        insight_type: c.insight_type.to_string(),
        title: c.title,
        body: c.body,
        sentiment: c.sentiment.to_string(),
        metric_value: c.metric_value,
        baseline_value: c.baseline_value,
        date: c.date,
        dismissed: c.dismissed,
        generated_at: common::time::bridge::chrono_to_jiff(c.generated_at),
    }
}

pub fn event_to_timeline(
    e: feature_productivity::types::ActivityEvent,
) -> ActivityTimelineResponse {
    ActivityTimelineResponse {
        app_name: e.app_name,
        window_title: e.window_title,
        site_name: e.site_name,
        category_id: e.category_id,
        started_at: common::time::bridge::chrono_to_jiff(e.started_at),
        duration_secs: e.duration_secs,
        is_idle: e.is_idle,
        project_id: e.project_id,
        focus_session_id: e.focus_session_id,
    }
}
