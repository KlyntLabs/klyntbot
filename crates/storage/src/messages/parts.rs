use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A typed fragment of a Message's content. Replaces the prior `content: String` field.
///
/// Variants are tagged via serde with `kind` for forward-compatibility — adding a new
/// variant doesn't break stored JSON for old variants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        output: ToolOutput,
        is_error: bool,
    },
    Reasoning {
        text: String,
        redacted: bool,
    },
    FileChange {
        path: PathBuf,
        before: Option<String>,
        after: String,
        diff_unified: String,
        applied: bool,
    },
    CommandExecution {
        command: Vec<String>,
        cwd: PathBuf,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    Finish {
        reason: FinishReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ToolOutput {
    pub text: String,
    pub mime: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

/// Why a turn ended. New typed enum (Phase 4 — was a `String` previously).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FinishReason {
    Completed,
    ToolCallsExhausted,
    Cancelled,
    PermissionDenied {
        reason: String,
    },
    SandboxViolation {
        reason: String,
    },
    CostCeilingReached {
        spend_usd: f64,
        ceiling_usd: f64,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
}
