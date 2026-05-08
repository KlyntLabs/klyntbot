//! App-core handlers for coding todo operations.

use common::Result;

/// Get the current agent's todo list for a thread.
pub async fn coding_todo_get(
    _thread_id: &str,
    _agent_id: &str,
) -> Result<Vec<feature_coding_todo::types::TodoItem>> {
    // TODO: wire to TodoRepo (Task 45)
    Ok(vec![])
}

/// Ratify a proposed plan.
pub async fn coding_plan_ratify(
    _thread_id: &str,
    _plan_session_id: &str,
) -> Result<bool> {
    // TODO: clear proposed_in_plan_session tag (Task 46)
    Ok(true)
}

/// User edited items in a proposed plan.
pub async fn coding_plan_user_edit(
    _thread_id: &str,
    _plan_session_id: &str,
    _items_json: &str,
) -> Result<bool> {
    // TODO: validate and overwrite row (Task 47)
    Ok(true)
}

/// User removed items from a proposed plan.
pub async fn coding_plan_user_remove(
    _thread_id: &str,
    _plan_session_id: &str,
    _item_ids: &[String],
) -> Result<bool> {
    // TODO: filter items and rewrite row (Task 47)
    Ok(true)
}
