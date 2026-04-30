use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Op {
    NoOp,
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },
    UserMessage {
        text: String,
    },
    Cancel,
}
