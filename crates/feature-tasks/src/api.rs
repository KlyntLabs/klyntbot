//! UI-facing task API functions.
//!
//! These are pure storage-backed operations that return raw rows.
//! `app-core` wraps them into `desktop_shared` response types and emits
//! `EntityUpdate` / domain events.

use std::collections::HashSet;

use storage::{
    KeyResultRow, ObjectiveRow, TaskAttachmentRow, TaskFilter, TaskPatch, TaskRepo, TaskRow,
    TaskTimeEntryRow,
};

// ── CRUD ──────────────────────────────────────────────────────────────────

/// List tasks matching the given filter.
pub async fn list_tasks(
    repo: &TaskRepo,
    filter: &TaskFilter,
) -> Result<Vec<TaskRow>, storage::StorageError> {
    repo.list(filter).await
}

/// Get a single task by ID.
pub async fn get_task(repo: &TaskRepo, id: &str) -> Result<Option<TaskRow>, storage::StorageError> {
    repo.get(id).await
}

/// Create a new task.
pub async fn create_task(repo: &TaskRepo, row: &TaskRow) -> Result<TaskRow, storage::StorageError> {
    repo.add(row).await
}

/// Update a task.
pub async fn update_task(
    repo: &TaskRepo,
    patch: &TaskPatch,
) -> Result<TaskRow, storage::StorageError> {
    repo.update(patch).await
}

/// Delete a task.
pub async fn delete_task(repo: &TaskRepo, id: &str) -> Result<bool, storage::StorageError> {
    repo.delete(id).await
}

/// Toggle a task's completion status.
pub async fn toggle_complete(repo: &TaskRepo, id: &str) -> Result<TaskRow, storage::StorageError> {
    let row = repo.get_or_err(id).await?;
    let new_status = if row.completed {
        "todo".to_string()
    } else {
        "done".to_string()
    };
    let patch = TaskPatch {
        id: id.to_string(),
        status: Some(new_status),
        completed: Some(!row.completed),
        ..Default::default()
    };
    repo.update(&patch).await
}

/// List child tasks of a parent.
pub async fn list_children(
    repo: &TaskRepo,
    parent_id: &str,
) -> Result<Vec<TaskRow>, storage::StorageError> {
    repo.get_children(parent_id).await
}

// ── Dependencies ──────────────────────────────────────────────────────────

/// Add a dependency (blocker) relationship between two tasks.
pub async fn add_dependency(
    repo: &TaskRepo,
    task_id: &str,
    blocker_id: &str,
) -> Result<(), storage::StorageError> {
    repo.add_dependency(task_id, blocker_id, "blocks").await
}

/// List blockers for a task.
pub async fn list_dependencies(
    repo: &TaskRepo,
    task_id: &str,
) -> Result<Vec<TaskRow>, storage::StorageError> {
    repo.get_blockers(task_id).await
}

// ── Attachments ───────────────────────────────────────────────────────────

/// Add an attachment to a task.
pub async fn add_attachment(
    repo: &TaskRepo,
    task_id: &str,
    attachment_type: &str,
    value: &str,
    title: Option<&str>,
) -> Result<TaskAttachmentRow, storage::StorageError> {
    repo.add_attachment(task_id, attachment_type, value, title, &[], "user")
        .await
}

/// List attachments for a task.
pub async fn list_attachments(
    repo: &TaskRepo,
    task_id: &str,
) -> Result<Vec<TaskAttachmentRow>, storage::StorageError> {
    repo.list_attachments(task_id).await
}

// ── Time entries ──────────────────────────────────────────────────────────

/// Add a time entry to a task.
pub async fn add_time_entry(
    repo: &TaskRepo,
    task_id: &str,
    started_at: jiff::Timestamp,
    duration_secs: Option<i64>,
    note: Option<&str>,
) -> Result<TaskTimeEntryRow, storage::StorageError> {
    repo.add_time_entry(task_id, "user", started_at, duration_secs, note, None)
        .await
}

/// List time entries for a task.
pub async fn list_time_entries(
    repo: &TaskRepo,
    task_id: &str,
) -> Result<Vec<TaskTimeEntryRow>, storage::StorageError> {
    repo.list_time_entries(task_id).await
}

// ── Queries ───────────────────────────────────────────────────────────────

/// Return tasks for the "today" view: doing + due today + overdue,
/// deduplicated and sorted (overdue first, then by priority, then due date).
pub async fn today_tasks(repo: &TaskRepo) -> Result<Vec<TaskRow>, storage::StorageError> {
    let now = jiff::Timestamp::now();
    let today_date = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    let start_of_today: jiff::Timestamp = today_date
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map(|z| z.timestamp())
        .unwrap_or(now);
    let start_of_tomorrow = start_of_today
        .checked_add(jiff::SignedDuration::from_secs(86400))
        .unwrap_or(start_of_today);

    let doing_filter = TaskFilter {
        status: Some("doing".to_string()),
        ..Default::default()
    };
    let due_today_filter = TaskFilter {
        due_after: Some(start_of_today),
        due_before: Some(start_of_tomorrow),
        ..Default::default()
    };

    let (doing, due_today, overdue) = tokio::try_join!(
        repo.list(&doing_filter),
        repo.list(&due_today_filter),
        repo.overdue(),
    )?;

    let mut seen = HashSet::new();
    let mut all_rows: Vec<TaskRow> = Vec::new();
    for row in overdue.into_iter().chain(doing).chain(due_today) {
        if !row.completed && row.status != "archived" && seen.insert(row.id.clone()) {
            all_rows.push(row);
        }
    }

    all_rows.sort_by(|a, b| {
        let a_overdue = a.due_date.is_some_and(|d| *d < now) as u8;
        let b_overdue = b.due_date.is_some_and(|d| *d < now) as u8;
        b_overdue
            .cmp(&a_overdue)
            .then(a.priority.unwrap_or(99).cmp(&b.priority.unwrap_or(99)))
            .then(a.due_date.cmp(&b.due_date))
    });

    Ok(all_rows)
}

/// Get the next upcoming task (earliest `due_date > now`, not completed).
pub async fn next_upcoming_task(repo: &TaskRepo) -> Result<Option<TaskRow>, storage::StorageError> {
    let filter = TaskFilter {
        due_after: Some(jiff::Timestamp::now()),
        limit: Some(1),
        ..Default::default()
    };
    let tasks = repo.list(&filter).await?;
    Ok(tasks.into_iter().find(|t| !t.completed))
}

/// List objectives (with their key results) for a project.
pub async fn objective_list(
    objectives_repo: &storage::ObjectiveRepo,
    key_results_repo: &storage::KeyResultRepo,
    project_id: Option<&str>,
) -> Result<Vec<(ObjectiveRow, Vec<KeyResultRow>)>, storage::StorageError> {
    let objectives = objectives_repo.list(project_id, None).await?;
    let kr_futures = objectives
        .iter()
        .map(|o| key_results_repo.list(Some(&o.id)));
    let all_krs = futures_util::future::try_join_all(kr_futures).await?;
    Ok(objectives.into_iter().zip(all_krs).collect())
}
