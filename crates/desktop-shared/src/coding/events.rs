use serde::{Deserialize, Serialize};
use specta::Type;

use super::thread::{MessageDto, MessageId, SubscriptionId, ThreadId, TurnId};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadEvent {
    TurnStarted {
        thread_id: ThreadId,
        turn_id: TurnId,
        model: String,
        started_at: i64,
    },
    ItemStarted {
        thread_id: ThreadId,
        turn_id: TurnId,
        item: MessageDto,
    },
    ItemDelta {
        thread_id: ThreadId,
        turn_id: TurnId,
        item_id: MessageId,
        part_idx: u32,
        delta: PartDelta,
    },
    ItemCompleted {
        thread_id: ThreadId,
        turn_id: TurnId,
        item: MessageDto,
    },
    ToolCallStarted {
        thread_id: ThreadId,
        turn_id: TurnId,
        item_id: MessageId,
        call_id: String,
        tool: String,
    },
    ToolCallCompleted {
        thread_id: ThreadId,
        turn_id: TurnId,
        call_id: String,
        success: bool,
        duration_ms: u64,
    },
    FileChanged {
        thread_id: ThreadId,
        turn_id: TurnId,
        path: String,
        change: FileChangeKindDto,
    },
    CommandExecuted {
        thread_id: ThreadId,
        turn_id: TurnId,
        command: Vec<String>,
        exit_code: Option<i32>,
    },
    ContextCompressed {
        thread_id: ThreadId,
        turn_id: TurnId,
        before_tokens: u64,
        after_tokens: u64,
    },
    TurnCompleted {
        thread_id: ThreadId,
        turn_id: TurnId,
        finish_reason: serde_json::Value,
        completed_at: i64,
        duration_ms: u64,
    },
    Heartbeat {
        subscription_id: SubscriptionId,
        server_time: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartDelta {
    Text { append: String },
    Reasoning { append: String, redacted: bool },
    ToolCallArgs { json_patch: serde_json::Value },
    CommandStdout { append: String },
    CommandStderr { append: String },
    FileChangeProgress { bytes_written: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKindDto {
    Created,
    Modified,
    Deleted,
}
