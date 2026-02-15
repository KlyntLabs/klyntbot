//! Sprint 3 integration tests — Bidirectional Calendar Sync
//!
//! Tests for calendar reconciliation system where calendar changes
//! automatically update todos. Covers:
//!
//! ## Core Reconciliation
//! - Event time changed → todo.due_date updates
//! - Event marked complete → todo.status = Done
//! - Event cancelled → calendar_event_uid cleared
//!
//! ## Config & Features
//! - bidirectional_sync flag enables/disables feature
//! - Respects sync_interval_secs
//! - Manual trigger bypasses interval
//!
//! ## Multi-Provider
//! - Apple Calendar, Google Calendar, Generic CalDAV
//! - Provider failure handling
//! - Conflict resolution
//!
//! ## Edge Cases
//! - Network timeouts, malformed data, race conditions
//! - Orphaned todos, duplicate UIDs
//!
//! ## Performance
//! - Sync 100 todos < 2s
//! - Sync 1000 events < 5s

#[allow(dead_code)] // Used by ignored tests that will be implemented later
mod mock_calendar_handler;

use agent::calendar_reconcile::reconcile_calendar_events;
use calendar::{CalendarEvent, EventSource};
use chrono::{TimeZone, Utc};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::{
    todo_store::TodoStore,
    todo_types::{Todo, TodoStatus},
};

// ─── Test helpers ──────────────────────────────────────────────

fn create_test_todo(title: &str) -> Todo {
    Todo {
        id: Todo::generate_id(),
        title: title.to_string(),
        description: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        parent_id: None,
        project_id: None,
        attachments: Vec::new(),
        time_entries: Vec::new(),
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
    }
}

async fn create_store() -> (TodoStore, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = TodoStore::new(file_path);
    (store, temp_dir)
}

fn create_test_event(
    uid: &str,
    start: chrono::DateTime<Utc>,
    status: Option<String>,
) -> CalendarEvent {
    CalendarEvent {
        uid: uid.to_string(),
        summary: "Test Event".to_string(),
        description: None,
        start,
        end: start + chrono::Duration::hours(1),
        source: EventSource::CalDAV,
        etag: None,
        status,
    }
}

// ═══════════════════════════════════════════════════════════════
// CORE RECONCILIATION LOGIC
// ═══════════════════════════════════════════════════════════════

// ─── AC1: Event time changed → todo.due_date updates ───────────

#[tokio::test]
async fn test_reconcile_event_time_changed_updates_todo() {
    // Setup
    let (mut store, _dir) = create_store().await;

    // Create todo with calendar event
    let original_due = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let mut todo = create_test_todo("Meeting");
    todo.due_date = Some(original_due);
    todo.calendar_event_uid = Some("event-123".to_string());
    let todo = store.add(todo).await.unwrap();

    // Calendar returns event with new time
    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let events = vec![create_test_event("event-123", new_time, None)];

    // Action: Run reconciliation
    let store_arc = Arc::new(RwLock::new(store));
    let report = reconcile_calendar_events(store_arc.clone(), events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 1);
    assert_eq!(report.due_dates_updated, 1);
    assert_eq!(report.todos_completed, 0);
    assert_eq!(report.links_cleared, 0);

    let updated_todo = store_arc
        .write()
        .await
        .get(&todo.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_todo.due_date, Some(new_time));
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_time_changed_by_30_minutes() {
    // Small time shift (30 min) should still trigger update
    // Implementation: Similar to above but with 30-min delta
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_time_changed_multiple_todos() {
    // Setup 3 todos, change 2 event times, leave 1 unchanged
    // Assert: 2 todos updated, 1 untouched
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_title_changed_updates_todo() {
    // Event summary changed → todo.title updates
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_description_changed_updates_todo() {
    // Event description changed → todo.description updates
}

// ─── AC2: Event marked complete → todo.status = Done ───────────

#[tokio::test]
async fn test_reconcile_event_completed_marks_todo_done() {
    let (mut store, _dir) = create_store().await;

    let mut todo = create_test_todo("Task");
    todo.status = TodoStatus::Todo;
    todo.calendar_event_uid = Some("event-456".to_string());
    todo.due_date = Some(Utc::now());
    let todo = store.add(todo).await.unwrap();

    // Calendar returns completed event
    let events = vec![create_test_event(
        "event-456",
        Utc::now(),
        Some("COMPLETED".to_string()),
    )];

    // Action
    let store_arc = Arc::new(RwLock::new(store));
    let report = reconcile_calendar_events(store_arc.clone(), events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 1);
    assert_eq!(report.todos_completed, 1);
    assert_eq!(report.due_dates_updated, 0);
    assert_eq!(report.links_cleared, 0);

    let updated = store_arc
        .write()
        .await
        .get(&todo.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, TodoStatus::Done);
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_completed_already_done_idempotent() {
    // Event completed, but todo already Done → no duplicate completion
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_completed_with_blockers() {
    // Event completed but todo has blockers
    // Decision: Force complete (calendar is source of truth)
}

// ─── AC3: Event cancelled/deleted → calendar_event_uid cleared ─

#[tokio::test]
async fn test_reconcile_event_cancelled_clears_uid() {
    let (mut store, _dir) = create_store().await;

    let mut todo = create_test_todo("Cancelled meeting");
    todo.calendar_event_uid = Some("event-789".to_string());
    let todo = store.add(todo).await.unwrap();

    // Calendar returns cancelled event
    let events = vec![create_test_event(
        "event-789",
        Utc::now(),
        Some("CANCELLED".to_string()),
    )];

    // Action
    let store_arc = Arc::new(RwLock::new(store));
    let report = reconcile_calendar_events(store_arc.clone(), events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 1);
    assert_eq!(report.links_cleared, 1);
    assert_eq!(report.todos_completed, 0);
    assert_eq!(report.due_dates_updated, 0);

    let updated = store_arc
        .write()
        .await
        .get(&todo.id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.calendar_event_uid.is_none());
}

#[tokio::test]
async fn test_reconcile_event_deleted_from_provider_clears_uid() {
    // Event no longer exists in provider response → clear UID
    let (mut store, _dir) = create_store().await;

    let mut todo = create_test_todo("Deleted event");
    todo.calendar_event_uid = Some("event-999".to_string());
    let todo = store.add(todo).await.unwrap();

    // Calendar returns empty list (event deleted)
    let events: Vec<CalendarEvent> = vec![];

    // Action
    let store_arc = Arc::new(RwLock::new(store));
    let report = reconcile_calendar_events(store_arc.clone(), events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 1);
    assert_eq!(report.links_cleared, 1);

    let updated = store_arc
        .write()
        .await
        .get(&todo.id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.calendar_event_uid.is_none());
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_deleted_does_not_delete_todo() {
    // Event deleted → UID cleared, but todo remains
}

// ─── Multiple changes ──────────────────────────────────────────

#[tokio::test]
async fn test_reconcile_multiple_changes_in_single_sync() {
    // 5 todos: 1 time changed, 1 completed, 1 cancelled, 1 unchanged, 1 deleted
    let (mut store, _dir) = create_store().await;

    let original_time = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let unchanged_time = Utc.with_ymd_and_hms(2026, 2, 20, 10, 0, 0).unwrap();

    // Todo 1: Time changed
    let mut todo1 = create_test_todo("Todo 1");
    todo1.calendar_event_uid = Some("event-1".to_string());
    todo1.due_date = Some(original_time);
    store.add(todo1).await.unwrap();

    // Todo 2: Completed
    let mut todo2 = create_test_todo("Todo 2");
    todo2.calendar_event_uid = Some("event-2".to_string());
    todo2.status = TodoStatus::Todo;
    todo2.due_date = Some(unchanged_time);
    store.add(todo2).await.unwrap();

    // Todo 3: Cancelled
    let mut todo3 = create_test_todo("Todo 3");
    todo3.calendar_event_uid = Some("event-3".to_string());
    store.add(todo3).await.unwrap();

    // Todo 4: Unchanged
    let mut todo4 = create_test_todo("Todo 4");
    todo4.calendar_event_uid = Some("event-4".to_string());
    todo4.due_date = Some(unchanged_time);
    store.add(todo4).await.unwrap();

    // Todo 5: Event deleted
    let mut todo5 = create_test_todo("Todo 5");
    todo5.calendar_event_uid = Some("event-5".to_string());
    store.add(todo5).await.unwrap();

    // Calendar returns events
    let events = vec![
        create_test_event("event-1", new_time, None), // Time changed
        create_test_event("event-2", unchanged_time, Some("COMPLETED".to_string())), // Completed
        create_test_event("event-3", unchanged_time, Some("CANCELLED".to_string())), // Cancelled
        create_test_event("event-4", unchanged_time, None), // Unchanged (same time as todo4)
                                                      // event-5 not in list (deleted)
    ];

    // Action
    let store_arc = Arc::new(RwLock::new(store));
    let report = reconcile_calendar_events(store_arc.clone(), events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 5);
    assert_eq!(report.due_dates_updated, 1); // todo1
    assert_eq!(report.todos_completed, 1); // todo2
    assert_eq!(report.links_cleared, 2); // todo3 + todo5
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_time_and_completion_together() {
    // Event both moved AND completed → apply both updates
}

// ═══════════════════════════════════════════════════════════════
// CONFIG & FEATURE FLAGS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_reconcile_disabled_when_bidirectional_sync_false() {
    // Setup adapter with config.bidirectional_sync = false
    // Run reconciliation
    // Assert: no changes applied, report indicates disabled
}

#[tokio::test]
#[ignore]
async fn test_reconcile_requires_auto_sync_due_dates_enabled() {
    // If auto_sync_due_dates is false, reconciliation should skip
}

#[tokio::test]
#[ignore]
async fn test_reconcile_respects_sync_interval_secs() {
    // Sync triggered at T+0, T+2min → second sync should skip (interval = 5 min)
}

#[tokio::test]
#[ignore]
async fn test_reconcile_manual_trigger_bypasses_interval() {
    // Manual `klyntbot calendar reconcile` always runs
}

#[tokio::test]
#[ignore]
async fn test_reconcile_default_config_enables_feature() {
    // Default CalendarConfig should have bidirectional_sync = true
}

#[tokio::test]
#[ignore]
async fn test_reconcile_config_reload_changes_behavior() {
    // Change config, reload → new sync respects updated flag
}

#[tokio::test]
#[ignore]
async fn test_reconcile_per_provider_enable_disable() {
    // Provider 1 enabled, Provider 2 disabled → only sync from enabled
}

#[tokio::test]
#[ignore]
async fn test_reconcile_conflict_resolution_server_wins() {
    // conflict_resolution: "server_wins" → calendar overrides local
}

// ═══════════════════════════════════════════════════════════════
// CLI INTEGRATION
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_command_exists() {
    // Run: klyntbot calendar reconcile
    // Assert: command executes without error
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_shows_report() {
    // Command output includes: "Updated: 2, Completed: 1, Unlinked: 0"
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_json_output() {
    // --format json flag returns structured report
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_dry_run() {
    // --dry-run flag shows what would change without applying
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_force_flag() {
    // --force flag bypasses interval check
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_provider_filter() {
    // --provider apple → only reconcile from Apple Calendar
}

#[tokio::test]
#[ignore]
async fn test_cli_calendar_reconcile_verbose_output() {
    // --verbose shows per-todo change details
}

// ═══════════════════════════════════════════════════════════════
// NOTIFICATION SYSTEM
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_notification_sent_when_changes_detected() {
    // Reconciliation finds 2 updates → notification sent to enabled channels
}

#[tokio::test]
#[ignore]
async fn test_notification_not_sent_when_no_changes() {
    // Reconciliation finds 0 changes → no notification spam
}

#[tokio::test]
#[ignore]
async fn test_notification_format_includes_summary() {
    // Notification: "📅 Calendar synced: 2 updated, 1 completed, 0 unlinked"
}

#[tokio::test]
#[ignore]
async fn test_notification_includes_task_titles() {
    // Verbose notification lists changed task names
}

#[tokio::test]
#[ignore]
async fn test_notification_respects_channel_config() {
    // Only send to channels with calendar_notifications: true
}

#[tokio::test]
#[ignore]
async fn test_notification_failure_does_not_break_sync() {
    // Notification fails → sync still completes
}

// ═══════════════════════════════════════════════════════════════
// MULTI-PROVIDER SCENARIOS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_reconcile_apple_calendar_only() {
    // Single provider: Apple Calendar
}

#[tokio::test]
#[ignore]
async fn test_reconcile_google_calendar_only() {
    // Single provider: Google Calendar
}

#[tokio::test]
#[ignore]
async fn test_reconcile_generic_caldav_provider() {
    // Generic CalDAV (Nextcloud, FastMail, etc.)
}

#[tokio::test]
#[ignore]
async fn test_reconcile_multiple_providers_same_event() {
    // Event exists in both Apple and Google → deduplicate by UID
}

#[tokio::test]
#[ignore]
async fn test_reconcile_multiple_providers_conflict() {
    // Apple: event at 2pm, Google: event at 3pm → resolve via config
}

#[tokio::test]
#[ignore]
async fn test_reconcile_one_provider_fails_others_succeed() {
    // Apple fails, Google succeeds → partial sync report
}

#[tokio::test]
#[ignore]
async fn test_reconcile_all_providers_fail_reports_error() {
    // All providers fail → reconciliation returns error
}

#[tokio::test]
#[ignore]
async fn test_reconcile_provider_sync_tokens_independent() {
    // Each provider maintains its own sync token
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_deleted_from_one_provider_only() {
    // Event exists in Apple but deleted from Google → keep UID
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_deleted_from_all_providers() {
    // Event deleted everywhere → clear UID
}

// ═══════════════════════════════════════════════════════════════
// EDGE CASES & ERROR HANDLING
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_reconcile_malformed_event_data_skips_gracefully() {
    // Event missing required fields → log warning, continue sync
}

#[tokio::test]
#[ignore]
async fn test_reconcile_network_timeout_retries() {
    // Network timeout → retry logic (or fail gracefully)
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_uid_mismatch() {
    // todo.calendar_event_uid != any event.uid → log orphan
}

#[tokio::test]
#[ignore]
async fn test_reconcile_orphaned_todos_detected() {
    // Todos with UIDs that no longer exist in any provider
}

#[tokio::test]
#[ignore]
async fn test_reconcile_race_condition_concurrent_sync_and_update() {
    // Sync running while user updates todo → detect conflict
}

#[tokio::test]
#[ignore]
async fn test_reconcile_invalid_event_status_value() {
    // Event.status = "UNKNOWN" → handle gracefully
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_with_no_start_time() {
    // All-day event or missing start → skip or use midnight
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_start_equals_end() {
    // Zero-duration event → valid or skip?
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_in_past_not_auto_completed() {
    // Past event that's not marked complete → don't auto-complete
}

#[tokio::test]
#[ignore]
async fn test_reconcile_todo_deleted_locally_but_event_exists() {
    // Todo deleted → should it be recreated from event?
}

#[tokio::test]
#[ignore]
async fn test_reconcile_duplicate_event_uids() {
    // Two todos with same calendar_event_uid → detect and fix
}

#[tokio::test]
#[ignore]
async fn test_reconcile_extremely_large_event_batch() {
    // 5000 events returned from provider → pagination handling
}

#[tokio::test]
#[ignore]
async fn test_reconcile_sync_state_corruption_recovery() {
    // Corrupted sync_token → full resync
}

#[tokio::test]
#[ignore]
async fn test_reconcile_calendar_provider_returns_empty_list() {
    // Provider returns 0 events → don't clear all UIDs
}

#[tokio::test]
#[ignore]
async fn test_reconcile_event_timezone_conversion() {
    // Event in PST, user in UTC → correct conversion
}

// ═══════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_perf_sync_100_todos_with_events_under_2_seconds() {
    // 100 todos, 100 calendar events → sync completes < 2s
    // use std::time::Instant;
    // let start = Instant::now();
    // // ... run reconciliation ...
    // assert!(start.elapsed().as_secs() < 2);
}

#[tokio::test]
#[ignore]
async fn test_perf_sync_1000_events_from_calendar_under_5_seconds() {
    // Pull 1000 events from mock calendar → parse + match < 5s
}

#[tokio::test]
#[ignore]
async fn test_perf_large_batch_updates_50_events_changed() {
    // 50 events with time changes → all applied correctly
}

#[tokio::test]
#[ignore]
async fn test_perf_memory_usage_stays_below_50mb() {
    // Monitor memory during 1000-event sync
    // (Requires external profiling tool or manual verification)
}

#[tokio::test]
#[ignore]
async fn test_perf_concurrent_syncs_from_multiple_users() {
    // Simulate 10 concurrent reconciliation calls (shared store)
}

#[tokio::test]
#[ignore]
async fn test_perf_sync_with_slow_network_uses_timeout() {
    // Network delay 10s → timeout at 5s
}

#[tokio::test]
#[ignore]
async fn test_perf_incremental_sync_faster_than_full_sync() {
    // With sync_token → fetch only new events, faster
}

#[tokio::test]
#[ignore]
async fn test_perf_reconcile_skips_unchanged_todos() {
    // 1000 todos, 990 unchanged → only process 10
}

// ═══════════════════════════════════════════════════════════════
// PERSISTENCE & STATE
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore]
async fn test_reconcile_state_persists_across_restart() {
    // Sync → restart adapter → state reloaded correctly
}

#[tokio::test]
#[ignore]
async fn test_conflict_log_appends_not_overwrites() {
    // Multiple conflicts → all logged in calendar_conflicts.jsonl
}

#[tokio::test]
#[ignore]
async fn test_sync_token_saved_per_provider() {
    // Apple sync_token != Google sync_token
}

#[tokio::test]
#[ignore]
async fn test_sync_token_updated_after_successful_sync() {
    // Before: token1, After: token2
}

#[tokio::test]
#[ignore]
async fn test_sync_token_not_updated_after_failed_sync() {
    // Sync fails → keep old token for retry
}

#[tokio::test]
#[ignore]
async fn test_reconcile_report_stored_in_history() {
    // Each reconciliation report stored in .klyntbot/reconcile_history.jsonl
}

#[tokio::test]
#[ignore]
async fn test_orphaned_uids_cleared_eventually() {
    // After 3 syncs without event → clear UID
}

#[tokio::test]
#[ignore]
async fn test_last_reconcile_timestamp_tracked() {
    // adapter.last_reconcile_at persisted
}
