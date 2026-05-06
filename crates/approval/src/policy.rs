use crate::class::{ApprovalClass, ApprovalScope};
use serde_json::Value;

pub trait ClassifyHook: Send + Sync {
    fn classify(&self, tool: &str, action: Option<&str>, args: &Value) -> Option<ApprovalClass>;

    fn scope(&self, _tool: &str, _action: Option<&str>, _args: &Value) -> Option<ApprovalScope> {
        None
    }
}
