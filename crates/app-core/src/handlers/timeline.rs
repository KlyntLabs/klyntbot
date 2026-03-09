use desktop_shared::commands::{
    SourceBreakdown, TimelineEntry, TimelineEntryType, TimelineQuery, TimelineResponse,
    TimelineSource, TimelineSummary, TopAppSummary,
};
use desktop_shared::errors::ApiError;
use std::collections::HashMap;

use crate::AppCore;

impl AppCore {
    pub async fn timeline_query(
        &self,
        params: TimelineQuery,
    ) -> Result<TimelineResponse, ApiError> {
        let start = &params.start_date;
        let end = &params.end_date;
        let include_point = params.include_point_events.unwrap_or(true);
        let sources = params.sources.as_deref();

        /// Check if a source should be fetched (no filter = fetch all).
        fn want(sources: Option<&[TimelineSource]>, s: TimelineSource) -> bool {
            sources.is_none_or(|list| list.contains(&s))
        }

        let mut entries = Vec::new();

        // 1. Activity events + Focus sessions (productivity repos)
        if want(sources, TimelineSource::Productivity) || want(sources, TimelineSource::Focus) {
            if let Ok(repos) = self.productivity_repos() {
                let start_dt = parse_start_of_day(start);
                let end_dt = parse_end_of_day(end);

                if let (Some(s), Some(e)) = (start_dt, end_dt) {
                    if want(sources, TimelineSource::Productivity) {
                        if let Ok(app_events) = repos.events.list_range(&s, &e, None).await {
                            entries.extend(app_events.into_iter().map(normalize_app_event));
                        }
                    }

                    if want(sources, TimelineSource::Focus) {
                        if let Ok(sessions) = repos.sessions.list_range(&s, &e, None).await {
                            entries.extend(sessions.into_iter().map(normalize_focus_session));
                        }
                    }
                }
            }
        }

        // 2. Task time entries (duration blocks)
        if want(sources, TimelineSource::Task) {
            if let Ok(time_entries) = self.repos.actions.time_entries_in_range(start, end).await {
                entries.extend(time_entries.into_iter().map(normalize_time_entry));
            }
        }

        // 3. Domain event log (point-in-time events — may produce Task/Note/Finance/System)
        if include_point {
            if let Some(ref repo) = self.event_log_repo {
                if let Ok(events) = repo.query_domain_events_range(start, end).await {
                    let mut domain_entries: Vec<_> = events
                        .into_iter()
                        .filter_map(normalize_domain_event)
                        .collect();
                    // Apply source filter to domain events since they span multiple sources
                    if let Some(src_list) = sources {
                        domain_entries.retain(|e| src_list.contains(&e.source));
                    }
                    entries.extend(domain_entries);
                }
            }
        }

        entries.sort_by(|a, b| a.started_at.cmp(&b.started_at));

        let summary = compute_summary(&entries);
        Ok(TimelineResponse { entries, summary })
    }
}

// ── Date helpers ─────────────────────────────────────────────────────

fn parse_start_of_day(date_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
}

fn parse_end_of_day(date_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(23, 59, 59))
        .map(|dt| dt.and_utc())
}

// ── Normalization functions ─────────────────────────────────────────

fn normalize_app_event(e: feature_productivity::ActivityEvent) -> TimelineEntry {
    // category_type (productive/distracting) isn't available on ActivityEvent,
    // so we use neutral for all app events. The UI can refine color via metadata.
    TimelineEntry {
        id: e.id.map(|i| i.to_string()).unwrap_or_default(),
        source: TimelineSource::Productivity,
        entry_type: TimelineEntryType::AppUsage,
        title: e.app_name.clone(),
        description: e.window_title,
        started_at: e.started_at.to_rfc3339(),
        ended_at: e.ended_at.map(|t| t.to_rfc3339()),
        duration_secs: e.duration_secs,
        entity_id: None,
        entity_route: Some("/productivity".into()),
        color: "var(--timeline-app-neutral)".into(),
        metadata: None,
    }
}

fn normalize_focus_session(s: feature_productivity::FocusSession) -> TimelineEntry {
    TimelineEntry {
        id: s.id,
        source: TimelineSource::Focus,
        entry_type: TimelineEntryType::FocusSession,
        title: "Focus Session".into(),
        description: Some(s.session_type.to_string()),
        started_at: s.started_at.to_rfc3339(),
        ended_at: s.ended_at.map(|t| t.to_rfc3339()),
        duration_secs: s.actual_mins.map(|m| m * 60),
        entity_id: None,
        entity_route: Some("/productivity".into()),
        color: "var(--timeline-focus)".into(),
        metadata: None,
    }
}

fn normalize_time_entry(te: storage::TimeEntryWithTask) -> TimelineEntry {
    TimelineEntry {
        id: te.id.to_string(),
        source: TimelineSource::Task,
        entry_type: TimelineEntryType::TaskTimeEntry,
        title: te.action_title,
        description: te.note,
        started_at: te.started_at.to_rfc3339(),
        ended_at: te.ended_at.map(|t| t.to_rfc3339()),
        duration_secs: te.duration_secs,
        entity_id: Some(te.action_id.clone()),
        entity_route: Some(format!("/task/{}", te.action_id)),
        color: "var(--timeline-task)".into(),
        metadata: None,
    }
}

fn normalize_domain_event(e: cognitive::DomainEventRow) -> Option<TimelineEntry> {
    let payload: serde_json::Value = serde_json::from_str(&e.payload).ok()?;

    /// Extract a string field from a JSON payload.
    fn field<'a>(payload: &'a serde_json::Value, key: &str) -> Option<&'a str> {
        payload.get(key).and_then(|v| v.as_str())
    }

    let (entry_type, source, title, entity_id, entity_route, color) = match e.event_type.as_str() {
        "TaskCreated" => {
            let task_id = field(&payload, "task_id");
            (
                TimelineEntryType::TaskCreated,
                TimelineSource::Task,
                format!("Task created: {}", task_id.unwrap_or("?")),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "TaskCompleted" => {
            let task_id = field(&payload, "task_id");
            (
                TimelineEntryType::TaskCompleted,
                TimelineSource::Task,
                "Task completed".into(),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "NoteCreated" => {
            let title_str = field(&payload, "title").unwrap_or("Untitled");
            (
                TimelineEntryType::NoteCreated,
                TimelineSource::Note,
                format!("Note: {title_str}"),
                field(&payload, "note_id").map(String::from),
                Some("/notes".into()),
                "var(--timeline-note)",
            )
        }
        "NoteUpdated" => {
            let title_str = field(&payload, "title").unwrap_or("Untitled");
            (
                TimelineEntryType::NoteUpdated,
                TimelineSource::Note,
                format!("Edited: {title_str}"),
                field(&payload, "note_id").map(String::from),
                Some("/notes".into()),
                "var(--timeline-note)",
            )
        }
        "TransactionRecorded" => (
            TimelineEntryType::TransactionRecorded,
            TimelineSource::Finance,
            format!(
                "Transaction: {}",
                field(&payload, "category").unwrap_or("Uncategorized")
            ),
            None,
            Some("/finance/transactions".into()),
            "var(--timeline-finance)",
        ),
        // Skip events we don't want on the timeline
        "ChatTurnCompleted"
        | "UserStatedFact"
        | "UserCorrectedAI"
        | "CoachingFeedback"
        | "ProductivityScoreComputed" => return None,
        // Other events as System
        _ => (
            TimelineEntryType::SystemEvent,
            TimelineSource::System,
            camel_to_title(&e.event_type),
            None,
            None,
            "var(--timeline-system)",
        ),
    };

    Some(TimelineEntry {
        id: e.id,
        source,
        entry_type,
        title,
        description: None,
        started_at: e.timestamp,
        ended_at: None,
        duration_secs: None,
        entity_id,
        entity_route,
        color: color.into(),
        metadata: Some(payload),
    })
}

/// Convert "CamelCaseString" to "Camel Case String".
fn camel_to_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

// ── Summary computation ─────────────────────────────────────────────

fn compute_summary(entries: &[TimelineEntry]) -> TimelineSummary {
    let mut total_tracked_secs: i64 = 0;
    let mut focus_secs: i64 = 0;
    let mut tasks_completed: i64 = 0;
    let mut tasks_created: i64 = 0;
    let mut notes_touched: i64 = 0;
    let mut transactions_count: i64 = 0;
    let mut app_durations: HashMap<String, i64> = HashMap::new();
    let mut source_durations: HashMap<TimelineSource, (i64, i64)> = HashMap::new();

    for entry in entries {
        let dur = entry.duration_secs.unwrap_or(0);
        if dur > 0 {
            total_tracked_secs += dur;
        }

        let (s_dur, s_count) = source_durations.entry(entry.source).or_insert((0, 0));
        *s_dur += dur;
        *s_count += 1;

        match entry.entry_type {
            TimelineEntryType::FocusSession => focus_secs += dur,
            TimelineEntryType::TaskCompleted => tasks_completed += 1,
            TimelineEntryType::TaskCreated => tasks_created += 1,
            TimelineEntryType::NoteCreated | TimelineEntryType::NoteUpdated => notes_touched += 1,
            TimelineEntryType::TransactionRecorded
            | TimelineEntryType::ExpenseRecorded
            | TimelineEntryType::IncomeRecorded => transactions_count += 1,
            TimelineEntryType::AppUsage => {
                *app_durations.entry(entry.title.clone()).or_insert(0) += dur;
            }
            _ => {}
        }
    }

    // Top 5 apps by duration
    let mut app_list: Vec<_> = app_durations.into_iter().collect();
    app_list.sort_by(|a, b| b.1.cmp(&a.1));
    let total_app_secs: i64 = app_list.iter().map(|(_, d)| d).sum();
    let top_apps: Vec<TopAppSummary> = app_list
        .into_iter()
        .take(5)
        .map(|(name, dur)| TopAppSummary {
            app_name: name,
            duration_secs: dur,
            percentage: if total_app_secs > 0 {
                dur as f64 / total_app_secs as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect();

    let source_breakdown: Vec<SourceBreakdown> = source_durations
        .into_iter()
        .map(|(source, (dur, count))| SourceBreakdown {
            source,
            duration_secs: dur,
            count,
        })
        .collect();

    TimelineSummary {
        total_tracked_secs,
        focus_secs,
        tasks_completed,
        tasks_created,
        notes_touched,
        transactions_count,
        top_apps,
        source_breakdown,
    }
}
