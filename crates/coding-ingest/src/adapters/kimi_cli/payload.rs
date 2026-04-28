//! kimi-cli stdin payload shapes for tier-1 hook events.

#![allow(dead_code)]

use crate::event::FileOp;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct CommonEnvelope {
    pub session_id: String,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub turn_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionStartBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserPromptBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub prompt: String,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMsgBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub truncated: Option<bool>,
    #[serde(default)]
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    #[serde(default)]
    pub cached_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub duration_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct FileEditBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub path: PathBuf,
    pub op: FileOp,
    pub bytes: u64,
    #[serde(default)]
    pub diff_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestRunBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub command: String,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub passed: u32,
    #[serde(default)]
    pub failed: u32,
    #[serde(default)]
    pub duration_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct CompactEventBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub token_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SkillActivatedBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub skill_id: String,
    pub source_path: PathBuf,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecallInjectedBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub memory_ids: Vec<String>,
    #[serde(default)]
    pub coverage_score: f32,
    #[serde(default)]
    pub dead_end_warning: bool,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalDecisionBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool: String,
    pub decision: String,
    #[serde(default)]
    pub layer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderCallBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub model: String,
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub retries: u32,
}
