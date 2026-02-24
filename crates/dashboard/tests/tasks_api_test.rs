//! Integration tests for /api/tasks endpoints.
//!
//! Covers AC-6.x: Full CRUD for tasks, subtasks, attachments, time entries, focus.
//! All response bodies must use camelCase JSON keys (AC-1.1).

mod common;

// ── List / Filter ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_tasks_empty_returns_empty_array() {
    // AC-6.1: GET /api/tasks on a fresh DB returns []
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_returns_created_tasks() {
    // AC-6.1: After creating tasks, GET /api/tasks lists them
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_filter_by_status() {
    // AC-6.1, UX §1.2: GET /api/tasks?status=todo returns only todo-status tasks
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_filter_by_project_id() {
    // AC-6.1, UX §1.2: GET /api/tasks?projectId=xxx filters by project
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_filter_by_priority_min() {
    // AC-6.1, UX §1.2: GET /api/tasks?priorityMin=1 returns tasks with priority >= 1
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_filter_by_tags() {
    // AC-6.1, UX §1.2: GET /api/tasks?tags=backend,security returns tagged tasks
    // TODO: implement
}

#[tokio::test]
async fn get_tasks_limit_param_caps_results() {
    // AC-6.1, UX §1.2: GET /api/tasks?limit=5 returns at most 5 tasks
    // TODO: implement
}

// ── Create ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn post_task_with_title_only_returns_201() {
    // AC-6.2, UX §6.2: Only `title` is required — all other fields optional
    // POST /api/tasks { "title": "Buy milk" } → 201
    // TODO: implement
}

#[tokio::test]
async fn post_task_with_all_optional_fields() {
    // AC-6.2: POST with title + description + priority + dueDate + tags + projectId
    // TODO: implement
}

#[tokio::test]
async fn post_task_missing_title_returns_422() {
    // AC-6.2, UX §6.2: title is required — missing title → 422 Unprocessable Entity
    // TODO: implement
}

#[tokio::test]
async fn post_task_priority_out_of_range_returns_422() {
    // AC-6.2, UX §6.2: Priority must be 1-5 (BA spec)
    // POST { "title": "x", "priority": 0 } → 422
    // TODO: implement
}

#[tokio::test]
async fn post_task_response_fields_are_camel_case() {
    // AC-1.1: POST /api/tasks response body has camelCase keys
    // Verify: dueDate, createdAt, updatedAt, totalTrackedSecs, etc.
    // TODO: implement
}

// ── Read ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_task_by_id_returns_todo_row() {
    // AC-6.3: GET /api/tasks/{id} returns full TodoRow
    // TODO: implement
}

#[tokio::test]
async fn get_task_by_id_not_found_returns_404() {
    // AC-6.3: GET /api/tasks/nonexistent → 404
    // TODO: implement
}

// ── Update ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn patch_task_updates_status() {
    // AC-6.4, UX §1.2: PATCH /api/tasks/{id} { "status": "done" } → 200
    // TODO: implement
}

#[tokio::test]
async fn patch_task_updates_priority() {
    // AC-6.4: PATCH priority field
    // TODO: implement
}

#[tokio::test]
async fn patch_task_partial_update_preserves_other_fields() {
    // AC-6.4: PATCH only sends changed fields; others remain unchanged
    // TODO: implement
}

#[tokio::test]
async fn patch_task_not_found_returns_404() {
    // AC-6.4: PATCH on nonexistent task → 404
    // TODO: implement
}

// ── Delete ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_task_returns_204() {
    // AC-6.5: DELETE /api/tasks/{id} → 204 No Content
    // TODO: implement
}

#[tokio::test]
async fn delete_task_not_found_returns_404() {
    // AC-6.5: DELETE nonexistent task → 404
    // TODO: implement
}

#[tokio::test]
async fn deleted_task_no_longer_appears_in_list() {
    // AC-6.5: After DELETE, GET /api/tasks does not include the deleted task
    // TODO: implement
}

// ── Summary ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_task_summary_counts_by_status() {
    // AC-6.6: GET /api/tasks/summary returns TodoSummary with todo/doing/done counts
    // TODO: implement
}

// ── Subtasks ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_subtasks_for_parent_task() {
    // AC-6.7: GET /api/tasks/{id}/subtasks returns child tasks
    // TODO: implement
}

#[tokio::test]
async fn get_subtasks_empty_when_no_children() {
    // AC-6.7: Returns [] when task has no subtasks
    // TODO: implement
}

// ── Attachments ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_attachments_returns_todo_attachment_rows() {
    // AC-6.8: GET /api/tasks/{id}/attachments → Vec<TodoAttachmentRow>
    // TODO: implement
}

// ── Time Entries ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_time_entries_returns_rows() {
    // AC-6.9: GET /api/tasks/{id}/time-entries → Vec<TodoTimeEntryRow>
    // TODO: implement
}

#[tokio::test]
async fn post_time_entry_creates_entry() {
    // AC-6.9: POST /api/tasks/{id}/time-entries adds a time entry
    // TODO: implement
}

// ── Focus ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn post_focus_sets_focused_at() {
    // AC-6.10: POST /api/tasks/{id}/focus → 200, task.focusedAt is set
    // TODO: implement
}

#[tokio::test]
async fn delete_focus_clears_focused_at() {
    // AC-6.10: DELETE /api/tasks/{id}/focus → 200, task.focusedAt is cleared
    // TODO: implement
}
