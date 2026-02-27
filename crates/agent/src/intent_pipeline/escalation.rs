//! Escalation context for mid-execution mode upgrades.
//!
//! When a Reactive engine detects it needs planning, it packages completed
//! work into an `EscalationContext` so the PlannedEngine can continue
//! without repeating steps.

use providers::Message;

/// A single completed step during reactive execution.
#[derive(Debug, Clone)]
pub struct CompletedStep {
    pub description: String,
    pub tool_name: String,
    pub result: String,
}

/// Context carried when escalating from one execution mode to another.
#[derive(Debug, Clone)]
pub struct EscalationContext {
    /// Message history accumulated before escalation.
    pub messages: Vec<Message>,
    /// Work already completed that shouldn't be repeated.
    pub completed_work: Vec<CompletedStep>,
    /// The original user message that triggered execution.
    pub original_message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escalation_context_construction() {
        let ctx = EscalationContext {
            messages: vec![Message::user("test")],
            completed_work: vec![CompletedStep {
                description: "Searched for tasks".to_string(),
                tool_name: "todo".to_string(),
                result: "Found 3 tasks".to_string(),
            }],
            original_message: "organize my tasks by priority".to_string(),
        };
        assert_eq!(ctx.completed_work.len(), 1);
        assert_eq!(ctx.original_message, "organize my tasks by priority");
    }
}
