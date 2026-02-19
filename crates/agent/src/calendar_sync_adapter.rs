// CalendarSyncAdapter - Multi-provider two-way sync between CalDAV and TodoStore

use async_trait::async_trait;
use calendar::{
    detect_conflict, load_provider_sync_state, resolve_conflict, save_provider_sync_state,
    AppleCalendarProvider, CalendarEvent, CalendarProvider, ConflictResolutionStrategy,
    EventSource, GenericCalDavProvider, GoogleCalendarProvider,
};
use chrono::{Duration, Local, Utc};
use common::Result;
use config::{CalendarConfig, CalendarProviderConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tools::{
    calendar_tool::CalendarHandler,
    todo_types::{Todo, TodoStatus},
};

/// CalendarSyncAdapter - implements two-way calendar synchronization
/// with multiple CalDAV providers.
pub struct CalendarSyncAdapter {
    providers: Vec<(String, Box<dyn CalendarProvider>)>,
    _provider_configs: Vec<CalendarProviderConfig>,
    todo_repo: storage::TodoRepo,
    conflicts_path: PathBuf,
    auto_sync_due_dates: bool,
    dispatcher: Option<Arc<crate::NotificationDispatcher>>,
    bidirectional_sync: bool,
    conflict_strategy: ConflictResolutionStrategy,
}

impl CalendarSyncAdapter {
    /// Create a new CalendarSyncAdapter from the calendar config.
    pub async fn new(
        todo_repo: storage::TodoRepo,
        config: &CalendarConfig,
        timezone: String,
        dispatcher: Option<Arc<crate::NotificationDispatcher>>,
        bidirectional_sync: bool,
    ) -> Result<Self> {
        let mut providers: Vec<(String, Box<dyn CalendarProvider>)> = Vec::new();
        let mut provider_configs = Vec::new();
        let mut any_auto_sync = false;

        for provider_config in &config.providers {
            if !provider_config.is_enabled() {
                continue;
            }

            let provider_id = provider_config.provider_id();
            any_auto_sync = any_auto_sync || provider_config.auto_sync_due_dates();

            let provider: Box<dyn CalendarProvider> = match provider_config {
                CalendarProviderConfig::Apple(apple) => Box::new(AppleCalendarProvider::new(
                    apple.caldav_url.clone(),
                    apple.username.clone(),
                    apple.password.expose().to_string(),
                    apple.calendar_name.clone(),
                    timezone.clone(),
                )),
                CalendarProviderConfig::Google(google) => Box::new(GoogleCalendarProvider::new(
                    google.client_id.clone(),
                    google.client_secret.expose().to_string(),
                    google.access_token.expose().to_string(),
                    google.refresh_token.expose().to_string(),
                    google.calendar_id.clone(),
                    timezone.clone(),
                )),
                CalendarProviderConfig::GenericCalDav(generic) => {
                    Box::new(GenericCalDavProvider::new(
                        generic.name.clone(),
                        generic.caldav_url.clone(),
                        generic.username.clone(),
                        generic.password.expose().to_string(),
                        generic.calendar_name.clone(),
                        timezone.clone(),
                    ))
                }
            };

            providers.push((provider_id, provider));
            provider_configs.push(provider_config.clone());
        }

        let conflicts_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("calendar_conflicts.jsonl");

        let conflict_strategy = config
            .conflict_resolution
            .parse::<ConflictResolutionStrategy>()
            .unwrap_or_default();

        Ok(Self {
            providers,
            _provider_configs: provider_configs,
            todo_repo,
            conflicts_path,
            auto_sync_due_dates: any_auto_sync,
            dispatcher,
            bidirectional_sync,
            conflict_strategy,
        })
    }

    /// Two-way sync: Pull from all CalDAV providers, resolve conflicts, push local changes.
    async fn sync_calendar_internal(&self) -> Result<Value> {
        let mut total_pulled = 0;
        let mut total_pushed = 0;
        let mut total_conflicts = 0;
        let mut provider_results = Vec::new();
        let mut errors = Vec::new();

        for (provider_id, provider) in &self.providers {
            match self
                .sync_single_provider(provider_id, provider.as_ref())
                .await
            {
                Ok(result) => {
                    let pulled = result["events_pulled"].as_u64().unwrap_or(0);
                    let pushed = result["events_pushed"].as_u64().unwrap_or(0);
                    let conflicts = result["conflicts_detected"].as_u64().unwrap_or(0);
                    total_pulled += pulled;
                    total_pushed += pushed;
                    total_conflicts += conflicts;
                    provider_results.push(json!({
                        "provider": provider.name(),
                        "provider_id": provider_id,
                        "status": "success",
                        "events_pulled": pulled,
                        "events_pushed": pushed,
                        "conflicts_detected": conflicts,
                    }));
                }
                Err(e) => {
                    tracing::error!("Calendar sync failed for {}: {:?}", provider.name(), e);
                    errors.push(format!("{}: {}", provider.name(), e));
                    provider_results.push(json!({
                        "provider": provider.name(),
                        "provider_id": provider_id,
                        "status": "error",
                        "error": e.to_string(),
                    }));
                }
            }
        }

        // Clear calendar_event_uid for todos that had events deleted from all providers
        if self.auto_sync_due_dates {
            let rows = self
                .todo_repo
                .list(&storage::TodoFilter::default())
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            for row in rows {
                if row.due_date.is_none() && row.calendar_event_uid.is_some() {
                    let patch = storage::TodoPatch {
                        id: row.id.clone(),
                        calendar_event_uid: Some(None),
                        ..Default::default()
                    };
                    if let Err(e) = self.todo_repo.update(&patch).await {
                        tracing::warn!(
                            "Failed to clear calendar_event_uid for todo {}: {}",
                            row.id,
                            e
                        );
                    }
                }
            }
        }

        // Run reconciliation if bidirectional sync is enabled
        let mut reconcile_report = None;
        if self.bidirectional_sync {
            match self.get_events_for_reconciliation().await {
                Ok(events) => {
                    match crate::reconcile_calendar_events(&self.todo_repo, events).await {
                        Ok(report) => {
                            let changes_count = report.due_dates_updated
                                + report.todos_completed
                                + report.links_cleared;

                            if changes_count > 0 {
                                tracing::info!(
                                    "Reconciliation: {} due dates updated, {} todos completed, {} links cleared",
                                    report.due_dates_updated,
                                    report.todos_completed,
                                    report.links_cleared
                                );

                                // Send notification if dispatcher is available
                                if let Some(ref dispatcher) = self.dispatcher {
                                    let summary = format!(
                                        "Updated: {} | Completed: {} | Unlinked: {}",
                                        report.due_dates_updated,
                                        report.todos_completed,
                                        report.links_cleared
                                    );
                                    dispatcher
                                        .notify("📅 Calendar Sync: Changes Detected", &summary)
                                        .await
                                        .ok();
                                }
                            }

                            if !report.errors.is_empty() {
                                tracing::warn!(
                                    "Reconciliation encountered {} errors: {:?}",
                                    report.errors.len(),
                                    report.errors
                                );
                            }

                            reconcile_report = Some(report);
                        }
                        Err(e) => {
                            tracing::error!("Reconciliation failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch events for reconciliation: {}", e);
                }
            }
        }

        let status = if errors.is_empty() {
            "success"
        } else if errors.len() < self.providers.len() {
            "partial_success"
        } else {
            "failed"
        };

        let mut result = json!({
            "status": status,
            "events_pulled": total_pulled,
            "events_pushed": total_pushed,
            "conflicts_detected": total_conflicts,
            "providers": provider_results,
            "last_sync": Local::now().to_rfc3339(),
        });

        // Add reconciliation report if available
        if let Some(report) = reconcile_report {
            result["reconciliation"] = json!({
                "checked": report.checked,
                "due_dates_updated": report.due_dates_updated,
                "todos_completed": report.todos_completed,
                "links_cleared": report.links_cleared,
                "errors": report.errors,
            });
        }

        Ok(result)
    }

    /// Sync a single provider.
    async fn sync_single_provider(
        &self,
        provider_id: &str,
        provider: &dyn CalendarProvider,
    ) -> Result<Value> {
        // Load per-provider sync state
        let mut sync_state = load_provider_sync_state(provider_id).await?;

        // Fetch remote events
        let (remote_events, new_sync_token) = provider
            .get_events(sync_state.sync_token.as_deref())
            .await?;

        let mut conflicts_detected = 0;
        let mut events_pulled = 0;
        let mut events_pushed = 0;

        // Process remote changes
        for remote_event in &remote_events {
            if let Some(local_todo) = self.find_todo_by_calendar_uid(&remote_event.uid).await {
                if let Some(local_event) = self.todo_to_event(&local_todo) {
                    if detect_conflict(remote_event, &local_event) {
                        conflicts_detected += 1;
                        self.log_conflict(remote_event, &local_event).await?;
                        let resolved_event =
                            resolve_conflict(remote_event, &local_event, self.conflict_strategy);
                        self.update_todo_from_event(&local_todo.id, &resolved_event)
                            .await?;
                    } else {
                        self.update_todo_from_event(&local_todo.id, remote_event)
                            .await?;
                    }
                } else {
                    self.update_todo_from_event(&local_todo.id, remote_event)
                        .await?;
                }
                events_pulled += 1;
            } else {
                self.create_todo_from_event(remote_event).await?;
                events_pulled += 1;
            }
        }

        // Push local changes to this provider
        if self.auto_sync_due_dates {
            let rows = self
                .todo_repo
                .list(&storage::TodoFilter::default())
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

            for todo in todos {
                if todo.due_date.is_some() {
                    // Always PUT — CalDAV creates if new, updates if exists
                    if let Some(event) = self.todo_to_event(&todo) {
                        match provider.put_event(&event).await {
                            Ok(_etag) => {
                                if todo.calendar_event_uid.is_none() {
                                    let patch = storage::TodoPatch {
                                        id: todo.id.clone(),
                                        calendar_event_uid: Some(Some(event.uid.clone())),
                                        ..Default::default()
                                    };
                                    if let Err(e) = self.todo_repo.update(&patch).await {
                                        tracing::warn!(
                                            "Failed to set calendar_event_uid for todo {}: {}",
                                            todo.id,
                                            e
                                        );
                                    }
                                }
                                events_pushed += 1;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to sync event {} to {}: {}",
                                    event.uid,
                                    provider_id,
                                    e
                                );
                            }
                        }
                    }
                } else if todo.calendar_event_uid.is_some() {
                    // Delete from this provider (don't clear UID yet — deferred)
                    if let Some(uid) = &todo.calendar_event_uid {
                        match provider.delete_event(uid).await {
                            Ok(()) => events_pushed += 1,
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to delete event {} from {}: {}",
                                    uid,
                                    provider_id,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        // Save sync state
        sync_state.sync_token = new_sync_token;
        sync_state.last_sync = Some(Utc::now());
        save_provider_sync_state(provider_id, &sync_state).await?;

        Ok(json!({
            "events_pulled": events_pulled,
            "events_pushed": events_pushed,
            "conflicts_detected": conflicts_detected,
        }))
    }

    /// Find a todo by its calendar event UID via SQL.
    async fn find_todo_by_calendar_uid(&self, uid: &str) -> Option<Todo> {
        let filter = storage::TodoFilter::default();
        if let Ok(rows) = self.todo_repo.list(&filter).await {
            rows.into_iter()
                .map(Todo::from)
                .find(|t| t.calendar_event_uid.as_ref().is_some_and(|u| u == uid))
        } else {
            None
        }
    }

    /// Update a todo from a calendar event via SQL.
    async fn update_todo_from_event(&self, todo_id: &str, event: &CalendarEvent) -> Result<()> {
        let patch = storage::TodoPatch {
            id: todo_id.to_string(),
            title: Some(event.summary.clone()),
            description: Some(event.description.clone()),
            due_date: Some(Some(event.start)),
            ..Default::default()
        };
        self.todo_repo
            .update(&patch)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Create a new todo from a calendar event via SQL.
    async fn create_todo_from_event(&self, event: &CalendarEvent) -> Result<()> {
        let now = Utc::now();
        let todo = Todo {
            id: Todo::generate_id(),
            title: event.summary.clone(),
            description: event.description.clone(),
            priority: None,
            due_date: Some(event.start),
            tags: vec![],
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: None,
            project_id: None,
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: Some(event.uid.clone()),
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        };

        let row: storage::TodoRow = (&todo).into();
        self.todo_repo
            .add(&row)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Convert a todo to a calendar event.
    fn todo_to_event(&self, todo: &Todo) -> Option<CalendarEvent> {
        let due_date = todo.due_date?;
        let uid = todo
            .calendar_event_uid
            .clone()
            .unwrap_or_else(|| format!("klyntbot-{}", todo.id));
        let duration_mins = todo.estimated_minutes.unwrap_or(60);
        let end = due_date + Duration::minutes(duration_mins as i64);

        Some(CalendarEvent {
            uid,
            summary: todo.title.clone(),
            description: todo.description.clone(),
            start: due_date,
            end,
            source: EventSource::TodoItem,
            etag: None,
            status: Some(match todo.status {
                TodoStatus::Todo => "TENTATIVE".to_string(),
                TodoStatus::Doing => "CONFIRMED".to_string(),
                TodoStatus::Done => "CONFIRMED".to_string(),
                TodoStatus::Archived => "CANCELLED".to_string(),
            }),
        })
    }

    /// Log a conflict to the conflicts file.
    async fn log_conflict(
        &self,
        server_event: &CalendarEvent,
        local_event: &CalendarEvent,
    ) -> Result<()> {
        if let Some(parent) = self.conflicts_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let conflict_entry = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "uid": server_event.uid,
            "server_version": {
                "summary": server_event.summary,
                "description": server_event.description,
                "start": server_event.start.to_rfc3339(),
                "end": server_event.end.to_rfc3339(),
                "etag": server_event.etag
            },
            "local_version": {
                "summary": local_event.summary,
                "description": local_event.description,
                "start": local_event.start.to_rfc3339(),
                "end": local_event.end.to_rfc3339(),
                "etag": local_event.etag
            },
            "resolution": "server_wins"
        });

        let mut line = serde_json::to_string(&conflict_entry)?;
        line.push('\n');
        use tokio::io::AsyncWriteExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.conflicts_path)
            .await?;
        file.write_all(line.as_bytes()).await?;

        Ok(())
    }
}

#[async_trait]
impl CalendarHandler for CalendarSyncAdapter {
    /// Synchronize calendar with all enabled providers.
    async fn sync_calendar(&self) -> Result<Value> {
        self.sync_calendar_internal().await
    }

    /// List upcoming calendar events from all enabled providers.
    async fn list_events(&self, limit: usize) -> Result<Value> {
        let mut all_events = Vec::new();

        for (_provider_id, provider) in &self.providers {
            match provider.get_events(None).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => {
                    tracing::warn!("Failed to list events from {}: {}", provider.name(), e);
                }
            }
        }

        // Deduplicate by UID, filter to upcoming, sort by start time
        let now = Utc::now();
        let mut seen_uids = std::collections::HashSet::new();
        let mut upcoming: Vec<_> = all_events
            .into_iter()
            .filter(|e| e.start >= now && seen_uids.insert(e.uid.clone()))
            .collect();

        upcoming.sort_by_key(|e| e.start);
        upcoming.truncate(limit);

        let event_list: Vec<Value> = upcoming
            .iter()
            .map(|e| {
                json!({
                    "uid": e.uid,
                    "summary": e.summary,
                    "description": e.description,
                    "start": e.start.to_rfc3339(),
                    "end": e.end.to_rfc3339(),
                    "source": format!("{:?}", e.source)
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "count": event_list.len(),
            "events": event_list
        }))
    }

    /// Create a new calendar event (uses first enabled provider).
    async fn create_event(
        &self,
        summary: String,
        description: Option<String>,
        start: String,
        end: String,
    ) -> Result<Value> {
        use chrono::DateTime;

        let start_dt = DateTime::parse_from_rfc3339(&start)
            .map_err(|e| {
                common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(format!(
                    "Invalid start time: {}",
                    e
                )))
            })?
            .with_timezone(&Utc);

        let end_dt = DateTime::parse_from_rfc3339(&end)
            .map_err(|e| {
                common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(format!(
                    "Invalid end time: {}",
                    e
                )))
            })?
            .with_timezone(&Utc);

        let uid = format!("klyntbot-event-{}", uuid::Uuid::new_v4());

        let event = CalendarEvent {
            uid: uid.clone(),
            summary: summary.clone(),
            description: description.clone(),
            start: start_dt,
            end: end_dt,
            source: EventSource::TodoItem,
            etag: None,
            status: None, // New events default to no status
        };

        // Push to all providers
        if self.providers.is_empty() {
            return Err(common::KlyntbotError::Calendar(
                common::CalendarError::ConnectionFailed(
                    "No calendar providers configured".to_string(),
                ),
            ));
        }

        let mut push_results = Vec::new();
        for (provider_id, provider) in &self.providers {
            match provider.put_event(&event).await {
                Ok(etag) => {
                    push_results.push(json!({
                        "provider": provider.name(),
                        "provider_id": provider_id,
                        "status": "created",
                        "etag": etag,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to create event on {}: {}", provider.name(), e);
                    push_results.push(json!({
                        "provider": provider.name(),
                        "provider_id": provider_id,
                        "status": "error",
                        "error": e.to_string(),
                    }));
                }
            }
        }

        Ok(json!({
            "status": "created",
            "uid": uid,
            "summary": summary,
            "description": description,
            "start": start,
            "end": end,
            "providers": push_results,
        }))
    }

    /// Get sync status for all providers.
    async fn get_status(&self) -> Result<Value> {
        let rows = self
            .todo_repo
            .list(&storage::TodoFilter::default())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let synced_count = rows
            .iter()
            .filter(|r| r.calendar_event_uid.is_some())
            .count();
        let total_count = rows.len();

        let mut provider_statuses = Vec::new();

        for (provider_id, provider) in &self.providers {
            let sync_state = load_provider_sync_state(provider_id).await.ok();

            provider_statuses.push(json!({
                "provider": provider.name(),
                "provider_id": provider_id,
                "last_sync": sync_state.as_ref().and_then(|s| s.last_sync.map(|t| t.with_timezone(&Local).to_rfc3339())),
                "has_sync_token": sync_state.as_ref().is_some_and(|s| s.sync_token.is_some()),
            }));
        }

        Ok(json!({
            "status": "ok",
            "providers": provider_statuses,
            "provider_count": self.providers.len(),
            "todos_with_calendar_events": synced_count,
            "total_todos": total_count,
            "auto_sync_enabled": self.auto_sync_due_dates
        }))
    }

    /// Get a single calendar event by UID from any enabled provider.
    /// Returns the first match found across all providers, or None if not found.
    async fn get_event(&self, uid: &str) -> Result<Option<CalendarEvent>> {
        for (_provider_id, provider) in &self.providers {
            match provider.get_events(None).await {
                Ok((events, _sync_token)) => {
                    if let Some(event) = events.into_iter().find(|e| e.uid == uid) {
                        return Ok(Some(event));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to get events from {} while searching for UID {}: {}",
                        provider.name(),
                        uid,
                        e
                    );
                }
            }
        }

        Ok(None)
    }

    /// Get all calendar events from all enabled providers for reconciliation.
    /// Returns a combined, deduplicated list of events (by UID).
    async fn get_events_for_reconciliation(&self) -> Result<Vec<CalendarEvent>> {
        let mut all_events = Vec::new();

        for (_provider_id, provider) in &self.providers {
            match provider.get_events(None).await {
                Ok((events, _sync_token)) => all_events.extend(events),
                Err(e) => {
                    tracing::warn!(
                        "Failed to get events from {} for reconciliation: {}",
                        provider.name(),
                        e
                    );
                }
            }
        }

        // Deduplicate by UID (keep first occurrence)
        let mut seen_uids = std::collections::HashSet::new();
        let deduplicated: Vec<CalendarEvent> = all_events
            .into_iter()
            .filter(|event| seen_uids.insert(event.uid.clone()))
            .collect();

        Ok(deduplicated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{AppleCalendarConfig, Secret};

    /// Try to connect to a test database. Returns None if no DB is available,
    /// allowing tests to gracefully skip.
    async fn test_todo_repo() -> Option<storage::TodoRepo> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
        match storage::StoragePool::connect(&url).await {
            Ok(pool) => Some(storage::Repos::from_pool(&pool).todos),
            Err(_) => None,
        }
    }

    fn test_calendar_config() -> CalendarConfig {
        CalendarConfig {
            providers: vec![CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: true,
                username: "user@example.com".to_string(),
                password: Secret::new("password123".to_string()),
                caldav_url: "https://caldav.example.com/calendar".to_string(),
                calendar_name: "Test Calendar".to_string(),
                sync_interval_secs: 300,
                auto_sync_due_dates: true,
            })],
            conflict_resolution: "server_wins".to_string(),
            ..CalendarConfig::default()
        }
    }

    #[tokio::test]
    async fn test_calendar_sync_adapter_creation() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };
        let config = test_calendar_config();

        let adapter =
            CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false).await;
        assert!(adapter.is_ok());

        let adapter = adapter.unwrap();
        assert_eq!(adapter.providers.len(), 1);
        assert_eq!(adapter.providers[0].0, "apple");
    }

    #[tokio::test]
    async fn test_todo_to_event_conversion() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };
        let config = test_calendar_config();

        let adapter = CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false)
            .await
            .unwrap();

        let now = Utc::now();
        let due_date = now + Duration::hours(2);

        let todo = Todo {
            id: "test-123".to_string(),
            title: "Test Task".to_string(),
            description: Some("Task description".to_string()),
            priority: Some(1),
            due_date: Some(due_date),
            tags: vec![],
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: None,
            project_id: None,
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: Some(90),
            calendar_event_uid: Some("event-uid-123".to_string()),
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        };

        let event = adapter.todo_to_event(&todo);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.uid, "event-uid-123");
        assert_eq!(event.summary, "Test Task");
        assert_eq!(event.description, Some("Task description".to_string()));
        assert_eq!(event.start, due_date);
        assert_eq!(event.end, due_date + Duration::minutes(90));
    }

    #[tokio::test]
    async fn test_todo_status_mapping() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };
        let config = test_calendar_config();

        let adapter = CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false)
            .await
            .unwrap();

        let now = Utc::now();
        let due_date = now + Duration::hours(2);

        // Test Todo status -> TENTATIVE
        let mut todo = Todo {
            id: "test-todo".to_string(),
            title: "Test Task".to_string(),
            description: None,
            priority: Some(1),
            due_date: Some(due_date),
            tags: vec![],
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: None,
            project_id: None,
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: Some(60),
            calendar_event_uid: Some("event-uid".to_string()),
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        };

        let event = adapter.todo_to_event(&todo).unwrap();
        assert_eq!(event.status, Some("TENTATIVE".to_string()));

        // Test Doing status -> CONFIRMED
        todo.status = TodoStatus::Doing;
        let event = adapter.todo_to_event(&todo).unwrap();
        assert_eq!(event.status, Some("CONFIRMED".to_string()));

        // Test Done status -> CONFIRMED
        todo.status = TodoStatus::Done;
        let event = adapter.todo_to_event(&todo).unwrap();
        assert_eq!(event.status, Some("CONFIRMED".to_string()));

        // Test Archived status -> CANCELLED
        todo.status = TodoStatus::Archived;
        let event = adapter.todo_to_event(&todo).unwrap();
        assert_eq!(event.status, Some("CANCELLED".to_string()));
    }

    #[tokio::test]
    async fn test_todo_to_event_no_due_date() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };
        let config = test_calendar_config();

        let adapter = CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false)
            .await
            .unwrap();

        let now = Utc::now();

        let todo = Todo {
            id: "test-456".to_string(),
            title: "No Due Date Task".to_string(),
            description: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: None,
            project_id: None,
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        };

        let event = adapter.todo_to_event(&todo);
        assert!(event.is_none());
    }

    #[tokio::test]
    async fn test_multi_provider_adapter() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };

        let config = CalendarConfig {
            providers: vec![
                CalendarProviderConfig::Apple(AppleCalendarConfig {
                    enabled: true,
                    username: "user@apple.com".to_string(),
                    password: Secret::new("pass".to_string()),
                    caldav_url: "https://caldav.icloud.com".to_string(),
                    ..AppleCalendarConfig::default()
                }),
                CalendarProviderConfig::Google(config::GoogleCalendarConfig {
                    enabled: true,
                    client_id: "id".to_string(),
                    client_secret: Secret::new("secret".to_string()),
                    access_token: Secret::new("tok".to_string()),
                    refresh_token: Secret::new("ref".to_string()),
                    ..config::GoogleCalendarConfig::default()
                }),
            ],
            ..CalendarConfig::default()
        };

        let adapter = CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false)
            .await
            .unwrap();
        assert_eq!(adapter.providers.len(), 2);
        assert_eq!(adapter.providers[0].0, "apple");
        assert_eq!(adapter.providers[1].0, "google");
    }

    #[tokio::test]
    async fn test_disabled_provider_skipped() {
        let Some(todo_repo) = test_todo_repo().await else {
            return;
        };

        let config = CalendarConfig {
            providers: vec![CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: false, // disabled
                username: "user@apple.com".to_string(),
                password: Secret::new("pass".to_string()),
                ..AppleCalendarConfig::default()
            })],
            ..CalendarConfig::default()
        };

        let adapter = CalendarSyncAdapter::new(todo_repo, &config, "UTC".to_string(), None, false)
            .await
            .unwrap();
        assert_eq!(adapter.providers.len(), 0);
    }
}
