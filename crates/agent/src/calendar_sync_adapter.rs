// CalendarSyncAdapter - Two-way sync implementation between CalDAV and TodoStore

use async_trait::async_trait;
use calendar::{
    load_sync_state, save_sync_state, CalDavClient, CalendarEvent, EventSource, SyncEngine,
};
use chrono::{Duration, Utc};
use common::Result;
use config::CalendarConfig;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tools::{
    calendar_tool::CalendarHandler,
    todo_store::TodoStore,
    todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus},
};

/// CalendarSyncAdapter - implements two-way calendar synchronization
pub struct CalendarSyncAdapter {
    caldav_client: Arc<RwLock<CalDavClient>>,
    todo_store: Arc<RwLock<TodoStore>>,
    config: CalendarConfig,
    conflicts_path: PathBuf,
    discovered_url: Arc<RwLock<Option<String>>>,
}

impl CalendarSyncAdapter {
    /// Create a new CalendarSyncAdapter
    pub fn new(
        todo_store: Arc<RwLock<TodoStore>>,
        config: CalendarConfig,
    ) -> Result<Self> {
        let caldav_client = CalDavClient::new(
            config.caldav_url.clone(),
            config.username.clone(),
            config.password.expose().to_string(),
        );

        let conflicts_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("calendar_conflicts.jsonl");

        Ok(Self {
            caldav_client: Arc::new(RwLock::new(caldav_client)),
            todo_store,
            config,
            conflicts_path,
            discovered_url: Arc::new(RwLock::new(None)),
        })
    }

    /// Ensure the calendar URL has been discovered (if using a base URL like caldav.icloud.com)
    async fn ensure_calendar_url_discovered(&self) -> Result<()> {
        // Check if we've already discovered the URL
        {
            let discovered = self.discovered_url.read().await;
            if discovered.is_some() {
                return Ok(());
            }
        }

        // Check if the current URL looks like a base URL that needs discovery
        let current_url = &self.config.caldav_url;
        let needs_discovery = current_url == "https://caldav.icloud.com/"
            || current_url == "https://caldav.icloud.com"
            || current_url.ends_with("/.well-known/caldav")
            || (!current_url.ends_with(".ics") && !current_url.contains("/calendars/"));

        if !needs_discovery {
            tracing::debug!("Calendar URL appears to be specific, skipping discovery");
            return Ok(());
        }

        tracing::info!("Discovering calendar URL for base URL: {}", current_url);

        // Perform discovery
        let discovered_calendar_url = match CalDavClient::discover_calendar_url(
            current_url,
            &self.config.username,
            self.config.password.expose(),
            &self.config.calendar_name,
        )
        .await
        {
            Ok(url) => url,
            Err(e) => {
                tracing::error!("CalDAV discovery failed: {:?}", e);
                return Err(e);
            }
        };

        // Convert relative path to full URL
        let full_url = if discovered_calendar_url.starts_with("http") {
            discovered_calendar_url.clone()
        } else {
            // Extract base from current URL and append discovered path
            let base = if current_url.contains("://") {
                current_url.split('/').take(3).collect::<Vec<_>>().join("/")
            } else {
                format!("https://{}", current_url.trim_start_matches("https://").split('/').next().unwrap_or("caldav.icloud.com"))
            };
            format!("{}{}", base, discovered_calendar_url)
        };

        tracing::info!("Discovered full calendar URL: {}", full_url);

        // Update the client's calendar_url
        {
            let mut client = self.caldav_client.write().await;
            client.set_calendar_url(full_url.clone());
        }

        // Store the discovered URL
        {
            let mut discovered = self.discovered_url.write().await;
            *discovered = Some(full_url);
        }

        Ok(())
    }

    /// Two-way sync: Pull from CalDAV server, resolve conflicts, push local changes
    async fn sync_calendar_internal(&self) -> Result<Value> {
        // Auto-discover calendar URL on first sync if using base URL
        self.ensure_calendar_url_discovered().await?;

        // Step 1: Load sync state
        let mut sync_state = load_sync_state().await?;

        // Step 2: Fetch remote events (incremental if we have a sync token)
        let (remote_events, new_sync_token) = {
            let client = self.caldav_client.read().await;
            client.get_events(sync_state.sync_token.as_deref()).await?
        };

        let mut store = self.todo_store.write().await;

        // Step 3: Process remote changes
        let mut conflicts_detected = 0;
        let mut events_pulled = 0;
        let mut events_pushed = 0;

        for remote_event in &remote_events {
            // Find corresponding local todo by calendar UID
            if let Some(local_todo) = self
                .find_todo_by_calendar_uid_async(&mut store, &remote_event.uid)
                .await
            {
                // Convert local todo to event for comparison
                if let Some(local_event) = self.todo_to_event(&local_todo) {
                    // Detect conflict
                    if SyncEngine::detect_conflict(remote_event, &local_event) {
                        conflicts_detected += 1;
                        self.log_conflict(remote_event, &local_event).await?;

                        // Resolve using server-wins strategy
                        let resolved_event = SyncEngine::resolve_conflict(remote_event, &local_event);
                        self.update_todo_from_event(&mut store, &local_todo.id, &resolved_event)
                            .await?;
                    } else {
                        // No conflict, but update from server if needed
                        self.update_todo_from_event(&mut store, &local_todo.id, remote_event)
                            .await?;
                    }
                } else {
                    // Local todo exists but can't be converted to event - update it
                    self.update_todo_from_event(&mut store, &local_todo.id, remote_event)
                        .await?;
                }
                events_pulled += 1;
            } else {
                // New event from server - create local todo
                self.create_todo_from_event(&mut store, remote_event).await?;
                events_pulled += 1;
            }
        }

        // Step 4: Push local changes to server
        if self.config.auto_sync_due_dates {
            let filter = TodoFilter::default();
            let todos = store.list(&filter).await?;
            for todo in todos {
                // Only sync todos with due dates that aren't already synced
                if todo.due_date.is_some()
                    && (todo.calendar_event_uid.is_none()
                        || !remote_events
                            .iter()
                            .any(|e| Some(&e.uid) == todo.calendar_event_uid.as_ref()))
                {
                    if let Some(event) = self.todo_to_event(&todo) {
                        let put_result = {
                            let client = self.caldav_client.read().await;
                            client.put_event(&event).await
                        };
                        match put_result {
                            Ok(_etag) => {
                                // Update todo with calendar UID
                                let patch = TodoPatch {
                                    calendar_event_uid: Some(Some(event.uid.clone())),
                                    ..Default::default()
                                };
                                store.update(&todo.id, patch).await?;
                                events_pushed += 1;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to push event {}: {}", event.uid, e);
                            }
                        }
                    }
                }
            }
        }

        // Step 5: Update sync state
        sync_state.sync_token = new_sync_token;
        sync_state.last_sync = Some(Utc::now());
        save_sync_state(&sync_state).await?;

        Ok(json!({
            "status": "success",
            "events_pulled": events_pulled,
            "events_pushed": events_pushed,
            "conflicts_detected": conflicts_detected,
            "last_sync": sync_state.last_sync,
            "sync_token": sync_state.sync_token
        }))
    }

    /// Find a todo by its calendar event UID (async helper)
    async fn find_todo_by_calendar_uid_async(
        &self,
        store: &mut TodoStore,
        uid: &str,
    ) -> Option<Todo> {
        let filter = TodoFilter::default();
        if let Ok(todos) = store.list(&filter).await {
            todos
                .into_iter()
                .find(|t| t.calendar_event_uid.as_ref().is_some_and(|u| u == uid))
        } else {
            None
        }
    }

    /// Update a todo from a calendar event
    async fn update_todo_from_event(
        &self,
        store: &mut TodoStore,
        todo_id: &str,
        event: &CalendarEvent,
    ) -> Result<()> {
        let patch = TodoPatch {
            title: Some(event.summary.clone()),
            description: Some(event.description.clone()),
            due_date: Some(Some(event.start)),
            calendar_event_uid: Some(Some(event.uid.clone())),
            ..Default::default()
        };

        store.update(todo_id, patch).await?;
        Ok(())
    }

    /// Create a new todo from a calendar event
    async fn create_todo_from_event(
        &self,
        store: &mut TodoStore,
        event: &CalendarEvent,
    ) -> Result<()> {
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
        };

        store.add(todo).await?;
        Ok(())
    }

    /// Convert a todo to a calendar event
    fn todo_to_event(&self, todo: &Todo) -> Option<CalendarEvent> {
        let due_date = todo.due_date?;

        // Generate UID if not present
        let uid = todo
            .calendar_event_uid
            .clone()
            .unwrap_or_else(|| format!("klyntbot-{}", todo.id));

        // Calculate event duration from estimated_minutes or default to 1 hour
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
        })
    }

    /// Log a conflict to the conflicts file
    async fn log_conflict(
        &self,
        server_event: &CalendarEvent,
        local_event: &CalendarEvent,
    ) -> Result<()> {
        // Ensure parent dir exists
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

        fs::write(&self.conflicts_path, line.as_bytes()).await?;

        Ok(())
    }
}

#[async_trait]
impl CalendarHandler for CalendarSyncAdapter {
    /// Synchronize calendar with CalDAV server
    async fn sync_calendar(&self) -> Result<Value> {
        self.sync_calendar_internal().await
    }

    /// List upcoming calendar events
    async fn list_events(&self, limit: usize) -> Result<Value> {
        // Fetch all events from CalDAV server
        let (events, _sync_token) = {
            let client = self.caldav_client.read().await;
            client.get_events(None).await?
        };

        // Filter to upcoming events only
        let now = Utc::now();
        let mut upcoming: Vec<_> = events
            .into_iter()
            .filter(|e| e.start >= now)
            .collect();

        // Sort by start time
        upcoming.sort_by_key(|e| e.start);

        // Limit results
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

    /// Create a new calendar event
    async fn create_event(
        &self,
        summary: String,
        description: Option<String>,
        start: String,
        end: String,
    ) -> Result<Value> {
        use chrono::DateTime;

        // Parse ISO 8601 timestamps
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

        // Generate unique UID
        let uid = format!("klyntbot-event-{}", uuid::Uuid::new_v4());

        let event = CalendarEvent {
            uid: uid.clone(),
            summary: summary.clone(),
            description: description.clone(),
            start: start_dt,
            end: end_dt,
            source: EventSource::TodoItem,
            etag: None,
        };

        // Push to CalDAV server
        let etag = {
            let client = self.caldav_client.read().await;
            client.put_event(&event).await?
        };

        Ok(json!({
            "status": "created",
            "uid": uid,
            "summary": summary,
            "description": description,
            "start": start,
            "end": end,
            "etag": etag
        }))
    }

    /// Get sync status
    async fn get_status(&self) -> Result<Value> {
        let sync_state = load_sync_state().await?;

        let mut store = self.todo_store.write().await;
        let filter = TodoFilter::default();
        let todos = store.list(&filter).await?;
        let synced_count = todos
            .iter()
            .filter(|t| t.calendar_event_uid.is_some())
            .count();

        Ok(json!({
            "status": "ok",
            "last_sync": sync_state.last_sync.map(|t| t.to_rfc3339()),
            "has_sync_token": sync_state.sync_token.is_some(),
            "todos_with_calendar_events": synced_count,
            "total_todos": todos.len(),
            "calendar_url": self.config.caldav_url,
            "auto_sync_enabled": self.config.auto_sync_due_dates
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::todo_store::TodoStore;

    #[tokio::test]
    async fn test_calendar_sync_adapter_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store_path = temp_dir.path().join("todos.jsonl");
        let todo_store = Arc::new(RwLock::new(TodoStore::new(store_path)));

        let config = CalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: config::Secret::new("password123".to_string()),
            caldav_url: "https://caldav.example.com/calendar".to_string(),
            calendar_name: "Test Calendar".to_string(),
            sync_interval_secs: 300,
            conflict_resolution: "server_wins".to_string(),
            auto_sync_due_dates: true,
        };

        let adapter = CalendarSyncAdapter::new(todo_store, config);
        assert!(adapter.is_ok());
    }

    #[tokio::test]
    async fn test_todo_to_event_conversion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store_path = temp_dir.path().join("todos.jsonl");
        let todo_store = Arc::new(RwLock::new(TodoStore::new(store_path)));

        let config = CalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: config::Secret::new("password123".to_string()),
            caldav_url: "https://caldav.example.com/calendar".to_string(),
            calendar_name: "Test Calendar".to_string(),
            sync_interval_secs: 300,
            conflict_resolution: "server_wins".to_string(),
            auto_sync_due_dates: true,
        };

        let adapter = CalendarSyncAdapter::new(todo_store, config).unwrap();

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
    async fn test_todo_to_event_no_due_date() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store_path = temp_dir.path().join("todos.jsonl");
        let todo_store = Arc::new(RwLock::new(TodoStore::new(store_path)));

        let config = CalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: config::Secret::new("password123".to_string()),
            caldav_url: "https://caldav.example.com/calendar".to_string(),
            calendar_name: "Test Calendar".to_string(),
            sync_interval_secs: 300,
            conflict_resolution: "server_wins".to_string(),
            auto_sync_due_dates: true,
        };

        let adapter = CalendarSyncAdapter::new(todo_store, config).unwrap();

        let now = Utc::now();

        let todo = Todo {
            id: "test-456".to_string(),
            title: "No Due Date Task".to_string(),
            description: None,
            priority: None,
            due_date: None, // No due date
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
        };

        let event = adapter.todo_to_event(&todo);
        assert!(event.is_none()); // Should return None without due date
    }
}
