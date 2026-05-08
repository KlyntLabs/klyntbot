//! View types returned by the four app-core handlers and consumed by the frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::types::TodoItem;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CodingTodoView {
    /// Map of agent_id → that agent's items.
    pub agents: HashMap<String, Vec<TodoItem>>,
    /// Plan-mode state if the thread is currently in plan mode; `None` otherwise.
    pub plan_mode_state: Option<PlanModeView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanModeView {
    pub plan_session_id: String,
    pub plan_file_slug: String,
    #[specta(type = String)]
    pub plan_file_path: PathBuf,
    pub proposed_item_count: usize,
}
