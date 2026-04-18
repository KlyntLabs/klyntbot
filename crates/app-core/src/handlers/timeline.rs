use desktop_shared::commands::{
    SourceBreakdown, TimelineEntry, TimelineEntryType, TimelineQuery, TimelineResponse,
    TimelineSource, TimelineSummary, TopAppSummary,
};
use desktop_shared::errors::ApiError;
use std::collections::HashMap;

use crate::errors::parse_local_day_range;
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
        let tz = params.tz_offset_mins;

        /// Check if a source should be fetched (no filter = fetch all).
        fn want(sources: Option<&[TimelineSource]>, s: TimelineSource) -> bool {
            sources.is_none_or(|list| list.contains(&s))
        }

        let mut entries = Vec::new();

        // 1. Activity events, Focus sessions, Calendar events (productivity repos)
        let need_prod_repos = want(sources, TimelineSource::Productivity)
            || want(sources, TimelineSource::Focus)
            || want(sources, TimelineSource::Calendar);
        if need_prod_repos {
            if let Ok(repos) = self.productivity_repos() {
                // Use timezone-aware range: start_date 00:00 local → end_date+1 00:00 local (in UTC)
                let (start_utc, _) = parse_local_day_range(start, tz)?;
                let end_utc = parse_local_day_range(end, tz)?.1;

                if want(sources, TimelineSource::Productivity) {
                    if let Ok(app_events) =
                        repos.events.list_range(&start_utc, &end_utc, None).await
                    {
                        entries.extend(app_events.into_iter().map(normalize_app_event));
                    }
                }

                // CRITICAL: Do NOT add focus sessions as separate timeline entries.
                // Instead, they enrich activity events via the inFocusMode flag in metadata.
                // This prevents timeline duplication and keeps the focus context with its activities.
                // if want(sources, TimelineSource::Focus) {
                //     if let Ok(sessions) =
                //         repos.sessions.list_range(&start_utc, &end_utc, None).await
                //     {
                //         entries.extend(sessions.into_iter().map(normalize_focus_session));
                //     }
                // }

                if want(sources, TimelineSource::Calendar) {
                    let start_rfc = start_utc.to_string();
                    let end_rfc = end_utc.to_string();
                    if let Ok(cal_events) =
                        repos.calendar_events.list_range(&start_rfc, &end_rfc).await
                    {
                        entries.extend(cal_events.into_iter().map(normalize_calendar_event));
                    }
                }
            }
        }

        // 2–6. Independent queries — run concurrently for lower latency
        let (time_entries_res, tasks_res, txs_res, notes_res) = tokio::join!(
            async {
                if !want(sources, TimelineSource::Task) {
                    return None;
                }
                self.repos
                    .tasks
                    .time_entries_in_range(start, end)
                    .await
                    .ok()
            },
            async {
                if !want(sources, TimelineSource::Todo) {
                    return None;
                }
                self.repos.tasks.tasks_for_timeline(start, end).await.ok()
            },
            async {
                if !want(sources, TimelineSource::Finance) {
                    return None;
                }
                let filter = storage::rows::finance::FinanceTransactionFilter {
                    date_from: jiff::civil::Date::strptime("%Y-%m-%d", start)
                        .ok()
                        .map(|d| d.into()),
                    date_to: jiff::civil::Date::strptime("%Y-%m-%d", end)
                        .ok()
                        .map(|d| d.into()),
                    limit: Some(100),
                    ..Default::default()
                };
                self.repos.finance.transactions.list(&filter).await.ok()
            },
            async {
                if !want(sources, TimelineSource::Note) {
                    return None;
                }
                self.note_repo.notes_in_date_range(start, end).await.ok()
            },
        );
        if let Some(time_entries) = time_entries_res {
            entries.extend(time_entries.into_iter().map(normalize_time_entry));
        }
        if let Some(tasks) = tasks_res {
            entries.extend(
                tasks
                    .into_iter()
                    .flat_map(|t| normalize_task(t, start, end)),
            );
        }
        if let Some(txs) = txs_res {
            entries.extend(txs.into_iter().map(normalize_transaction));
        }
        if let Some(notes) = notes_res {
            entries.extend(notes.into_iter().map(|n| normalize_note_activity(n, start)));
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
                    // Remove entries now handled by direct pipelines to avoid duplicates
                    domain_entries.retain(|e| {
                        !matches!(
                            e.entry_type,
                            TimelineEntryType::TaskCreated
                                | TimelineEntryType::TaskCompleted
                                | TimelineEntryType::NoteCreated
                                | TimelineEntryType::NoteUpdated
                                | TimelineEntryType::TransactionRecorded
                        )
                    });
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

// ── Normalization functions ─────────────────────────────────────────

fn normalize_app_event(e: feature_productivity::ActivityEvent) -> TimelineEntry {
    let mut metadata = serde_json::json!({});
    if let Some(fid) = e.focus_session_id.as_ref() {
        metadata["focusSessionId"] = serde_json::json!(fid);
        metadata["inFocusMode"] = serde_json::json!(true);
    }
    TimelineEntry {
        id: e.id.map(|i| i.to_string()).unwrap_or_default(),
        source: TimelineSource::Productivity,
        entry_type: TimelineEntryType::AppUsage,
        title: e.app_name.clone(),
        description: e.window_title,
        started_at: e.started_at.to_string(),
        ended_at: e.ended_at.map(|t| t.to_string()),
        duration_secs: e.duration_secs,
        entity_id: None,
        entity_route: Some("/productivity".into()),
        color: "var(--timeline-app-neutral)".into(),
        metadata: if metadata.is_object() && !metadata.as_object().unwrap().is_empty() {
            Some(metadata)
        } else {
            None
        },
    }
}

fn normalize_time_entry(te: storage::TimeEntryWithTask) -> TimelineEntry {
    TimelineEntry {
        id: te.id.to_string(),
        source: TimelineSource::Task,
        entry_type: TimelineEntryType::TaskTimeEntry,
        title: te.task_title,
        description: te.note,
        started_at: te.started_at.to_string(),
        ended_at: te
            .ended_at
            .map(|t| t.to_string()),
        duration_secs: te.duration_secs,
        entity_id: Some(te.task_id.clone()),
        entity_route: Some(format!("/task/{}", te.task_id)),
        color: "var(--timeline-task)".into(),
        metadata: None,
    }
}

fn normalize_domain_event(e: cognitive::DomainEventRow) -> Option<TimelineEntry> {
    let payload: serde_json::Value = serde_json::from_str(&e.payload).ok()?;

    // DomainEvent uses serde's externally-tagged format: {"TaskStatusChanged": {..}}
    // Extract the inner object for field access.
    let inner = payload.get(e.event_type.as_str()).unwrap_or(&payload);

    /// Extract a string field from a JSON payload.
    fn field<'a>(payload: &'a serde_json::Value, key: &str) -> Option<&'a str> {
        payload.get(key).and_then(|v| v.as_str())
    }

    let (entry_type, source, title, entity_id, entity_route, color) = match e.event_type.as_str() {
        "TaskCreated" => {
            let task_id = field(inner, "task_id");
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
            let task_id = field(inner, "task_id");
            (
                TimelineEntryType::TaskCompleted,
                TimelineSource::Task,
                "Task completed".into(),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "TaskStatusChanged" => {
            let task_id = field(inner, "task_id");
            let from = field(inner, "from").unwrap_or("?");
            let to = field(inner, "to").unwrap_or("?");
            (
                TimelineEntryType::TaskStatusChanged,
                TimelineSource::Task,
                format!("Status: {from} → {to}"),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "TaskPriorityChanged" => {
            let task_id = field(inner, "task_id");
            let from = field(inner, "from").unwrap_or("none");
            let to = field(inner, "to").unwrap_or("none");
            (
                TimelineEntryType::TaskPriorityChanged,
                TimelineSource::Task,
                format!("Priority: {from} → {to}"),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "TaskFieldUpdated" => {
            let task_id = field(inner, "task_id");
            let field_name = field(inner, "field").unwrap_or("field");
            let from = field(inner, "from").unwrap_or("");
            let to = field(inner, "to").unwrap_or("");
            (
                TimelineEntryType::TaskFieldUpdated,
                TimelineSource::Task,
                format!("{field_name}: \"{from}\" → \"{to}\""),
                task_id.map(String::from),
                task_id.map(|id| format!("/task/{id}")),
                "var(--timeline-task)",
            )
        }
        "NoteCreated" => {
            let title_str = field(inner, "title").unwrap_or("Untitled");
            (
                TimelineEntryType::NoteCreated,
                TimelineSource::Note,
                format!("Note: {title_str}"),
                field(inner, "note_id").map(String::from),
                Some("/notes".into()),
                "var(--timeline-note)",
            )
        }
        "NoteUpdated" => {
            let title_str = field(inner, "title").unwrap_or("Untitled");
            (
                TimelineEntryType::NoteUpdated,
                TimelineSource::Note,
                format!("Edited: {title_str}"),
                field(inner, "note_id").map(String::from),
                Some("/notes".into()),
                "var(--timeline-note)",
            )
        }
        "TransactionRecorded" => (
            TimelineEntryType::TransactionRecorded,
            TimelineSource::Finance,
            format!(
                "Transaction: {}",
                field(inner, "category").unwrap_or("Uncategorized")
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

/// Normalize a task row into 1+ timeline entries.
fn normalize_task(t: storage::TaskRow, start: &str, end: &str) -> Vec<TimelineEntry> {
    let mut out = Vec::new();
    let start_bound = format!("{start}T00:00:00Z");
    let end_bound = format!("{end}T23:59:59Z");
    let task_route = format!("/task/{}", t.id);

    // Scheduled task — full time block
    if let (Some(ref sched_start), Some(ref sched_end)) = (&t.scheduled_start, &t.scheduled_end) {
        let start_str = sched_start.to_string();
        let end_str = sched_end.to_string();
        let duration = (sched_end.as_millisecond() - sched_start.as_millisecond()) / 1000;
        out.push(TimelineEntry {
            id: format!("{}-scheduled", t.id),
            source: TimelineSource::Todo,
            entry_type: TimelineEntryType::TaskDue,
            title: t.title.clone(),
            description: t.description.clone(),
            started_at: start_str,
            ended_at: Some(end_str),
            duration_secs: Some(duration),
            entity_id: Some(t.id.clone()),
            entity_route: Some(task_route.clone()),
            color: "var(--timeline-todo)".into(),
            metadata: Some(serde_json::json!({
                "scheduled": true,
                "taskId": t.id,
                "status": t.status,
                "priority": t.priority,
            })),
        });
    } else if let Some(ref due) = t.due_date {
        let due_str = due.to_string();
        if due_str >= start_bound && due_str <= end_bound {
            out.push(TimelineEntry {
                id: format!("{}-due", t.id),
                source: TimelineSource::Todo,
                entry_type: TimelineEntryType::TaskDue,
                title: t.title.clone(),
                description: t.description.clone(),
                started_at: due_str,
                ended_at: None,
                duration_secs: t.estimated_minutes.map(|m| m as i64 * 60),
                entity_id: Some(t.id.clone()),
                entity_route: Some(task_route.clone()),
                color: "var(--timeline-todo)".into(),
                metadata: Some(serde_json::json!({
                    "scheduled": false,
                    "taskId": t.id,
                    "status": t.status,
                    "priority": t.priority,
                })),
            });
        }
    }

    // Task created in range
    let created_str = t.created_at.to_string();
    if created_str >= start_bound && created_str <= end_bound {
        out.push(TimelineEntry {
            id: format!("{}-created", t.id),
            source: TimelineSource::Todo,
            entry_type: TimelineEntryType::TaskCreated,
            title: format!("Created: {}", t.title),
            description: None,
            started_at: created_str,
            ended_at: None,
            duration_secs: None,
            entity_id: Some(t.id.clone()),
            entity_route: Some(task_route.clone()),
            color: "var(--timeline-todo)".into(),
            metadata: None,
        });
    }

    // Task completed in range
    if let Some(ref completed) = t.completed_at {
        let comp_str = completed.to_string();
        if comp_str >= start_bound && comp_str <= end_bound {
            out.push(TimelineEntry {
                id: format!("{}-completed", t.id),
                source: TimelineSource::Todo,
                entry_type: TimelineEntryType::TaskCompleted,
                title: format!("Completed: {}", t.title),
                description: None,
                started_at: comp_str,
                ended_at: None,
                duration_secs: None,
                entity_id: Some(t.id.clone()),
                entity_route: Some(task_route),
                color: "var(--timeline-todo)".into(),
                metadata: None,
            });
        }
    }

    out
}

/// Zero-decimal currencies where stored amount = major unit (no cents).
const ZERO_DECIMAL_CURRENCIES: &[&str] = &[
    "VND", "JPY", "KRW", "CLP", "HUF", "ISK", "UGX", "RWF", "PYG", "BIF",
];

fn format_timeline_amount(amount: i64, currency: &str, is_expense: bool) -> String {
    let sign = if is_expense { "-" } else { "+" };
    let is_zero_decimal = ZERO_DECIMAL_CURRENCIES
        .iter()
        .any(|z| z.eq_ignore_ascii_case(currency));

    if is_zero_decimal {
        // VND, JPY, etc.: amount is already in major units, format with thousand separators
        let abs = amount.unsigned_abs();
        let formatted = format_with_separators(abs);
        let symbol = match currency.to_uppercase().as_str() {
            "VND" => "đ",
            "JPY" | "KRW" => currency,
            _ => currency,
        };
        if currency.eq_ignore_ascii_case("VND") {
            format!("{sign}{formatted}{symbol}")
        } else {
            format!("{sign}{symbol}{formatted}")
        }
    } else {
        // USD, EUR, etc.: amount is in cents
        let major = amount.unsigned_abs() as f64 / 100.0;
        let symbol = match currency.to_uppercase().as_str() {
            "USD" | "USDT" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "THB" => "฿",
            _ => currency,
        };
        format!("{sign}{symbol}{major:.2}")
    }
}

fn format_with_separators(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push('.');
        }
        result.push(c);
    }
    result
}

fn normalize_transaction(tx: storage::rows::finance::FinanceTransactionRow) -> TimelineEntry {
    let is_expense = tx.tx_type == "expense";
    let entry_type = if is_expense {
        TimelineEntryType::ExpenseRecorded
    } else {
        TimelineEntryType::IncomeRecorded
    };
    let amount_display = format_timeline_amount(tx.amount, &tx.currency, is_expense);
    let title = match &tx.category {
        Some(cat) => format!("{} {}", amount_display, cat),
        None => amount_display,
    };

    TimelineEntry {
        id: tx.id.clone(),
        source: TimelineSource::Finance,
        entry_type,
        title,
        description: tx.notes.clone(),
        started_at: tx.created_at.to_string(),
        ended_at: None,
        duration_secs: None,
        entity_id: Some(tx.id),
        entity_route: Some("/finance".into()),
        color: if is_expense {
            "var(--timeline-finance-expense)".into()
        } else {
            "var(--timeline-finance-income)".into()
        },
        metadata: Some(serde_json::json!({
            "amount": tx.amount,
            "txType": tx.tx_type,
            "category": tx.category,
            "counterparty": tx.counterparty,
        })),
    }
}

fn normalize_note_activity(note: feature_notes::models::NoteRow, start: &str) -> TimelineEntry {
    let start_bound = format!("{start}T00:00:00Z");
    let is_created = note.created_at >= start_bound && note.created_at == note.updated_at;
    let (entry_type, prefix) = if is_created {
        (TimelineEntryType::NoteCreated, "Created")
    } else {
        (TimelineEntryType::NoteUpdated, "Edited")
    };

    TimelineEntry {
        id: format!(
            "{}-{}",
            note.id,
            if is_created { "created" } else { "updated" }
        ),
        source: TimelineSource::Note,
        entry_type,
        title: format!("{}: {}", prefix, note.title),
        description: None,
        started_at: if is_created {
            note.created_at.clone()
        } else {
            note.updated_at.clone()
        },
        ended_at: None,
        duration_secs: None,
        entity_id: Some(note.id),
        entity_route: Some("/notes".into()),
        color: "var(--timeline-note)".into(),
        metadata: None,
    }
}

fn normalize_calendar_event(e: feature_productivity::CalendarEvent) -> TimelineEntry {
    let duration_secs = {
        let start = e.started_at.parse::<jiff::Timestamp>().ok();
        let end = e.ended_at.parse::<jiff::Timestamp>().ok();
        match (start, end) {
            (Some(s), Some(e)) => Some((e.as_millisecond() - s.as_millisecond()) / 1000),
            _ => None,
        }
    };
    TimelineEntry {
        id: e.id,
        source: TimelineSource::Calendar,
        entry_type: TimelineEntryType::CalendarEvent,
        title: e.title,
        description: e.description,
        started_at: e.started_at,
        ended_at: Some(e.ended_at),
        duration_secs,
        entity_id: e.session_id,
        entity_route: None,
        color: "var(--timeline-calendar)".into(),
        metadata: None,
    }
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
            TimelineEntryType::TaskDue => {}
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
