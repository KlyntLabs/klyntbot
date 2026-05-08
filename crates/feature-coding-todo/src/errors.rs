//! LLM-facing error variants. Each Display impl is the literal message
//! sent back to the model so it can self-correct.

use crate::types::{ConcurrencyClass, TodoStatus};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodingTodoError {
    #[error("item `{item_id}` has status=blocked but no blocked_reason. Provide a reason or change status.")]
    BlockedItemMissingReason { item_id: String },

    #[error("agent `{agent_id}` has multiple in_progress items: {item_ids:?}. Only one item can be in_progress at a time per agent.")]
    MultipleInProgressInAgent { agent_id: String, item_ids: Vec<String> },

    #[error("item `{item_id}` has concurrency={class:?} but conflicts with in-progress item(s) elsewhere: {conflicts_with:?}. Wait or relax the class.")]
    ConcurrencyViolation {
        item_id: String,
        class: ConcurrencyClass,
        conflicts_with: Vec<(String, String)>, // (agent_id, item_id)
    },

    #[error("cycle in blocked_by graph: {chain:?}. Remove circular dependency.")]
    CycleInBlockedBy { chain: Vec<String> },

    #[error("item `{item_id}` declares blocked_by={missing_dep} but no item with that id exists in this list.")]
    BlockedByUnknownItem { item_id: String, missing_dep: String },

    #[error("plan mode active: item `{item_id}` has status={status:?} but only `pending` is allowed in plan mode.")]
    PlanModeNonPendingStatus { item_id: String, status: TodoStatus },

    #[error("item `{item_id}` declares delegated_to={agent_id} but no agent with that id is registered.")]
    DelegatedToUnknownAgent { item_id: String, agent_id: String },

    #[error("agent `{caller}` cannot write to row owned by `{target}`. Each agent maintains its own todo list.")]
    CrossAgentMutationAttempt { caller: String, target: String },

    #[error("blocked items {item_ids:?} have no paired user-facing message in the same turn. After two consecutive violations, calls are rejected.")]
    BlockedItemMissingUserMessage { item_ids: Vec<String> },

    #[error("storage error: {0}")]
    Storage(#[from] common::KlyntbotError),

    #[error("invalid item shape: {0}")]
    InvalidItemShape(#[from] serde_json::Error),

    #[error("corrupted database data: {reason}")]
    CorruptedDbData { reason: String },

    #[error("internal error: item `{title}` is missing an id after assignment")]
    MissingId { title: String },
}

impl From<CodingTodoError> for common::KlyntbotError {
    fn from(e: CodingTodoError) -> Self {
        common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_missing_reason_message_names_item() {
        let e = CodingTodoError::BlockedItemMissingReason { item_id: "task_4".into() };
        assert!(e.to_string().contains("task_4"));
        assert!(e.to_string().contains("blocked_reason"));
    }

    #[test]
    fn multiple_in_progress_lists_offending_items() {
        let e = CodingTodoError::MultipleInProgressInAgent {
            agent_id: "root".into(),
            item_ids: vec!["task_1".into(), "task_2".into()],
        };
        let s = e.to_string();
        assert!(s.contains("root"));
        assert!(s.contains("task_1"));
        assert!(s.contains("task_2"));
    }

    #[test]
    fn cycle_lists_chain() {
        let e = CodingTodoError::CycleInBlockedBy {
            chain: vec!["a".into(), "b".into(), "a".into()],
        };
        assert!(e.to_string().contains("a"));
        assert!(e.to_string().contains("b"));
    }
}
