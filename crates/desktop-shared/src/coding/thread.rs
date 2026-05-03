use serde::{Deserialize, Serialize};
use specta::Type;

pub type ThreadId = String;
pub type TurnId = String;
pub type SubscriptionId = String;
pub type MessageId = String;
pub type WorkspaceId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: ThreadId,
    pub workspace_id: WorkspaceId,
    pub cwd: String,
    pub model: Option<String>,
    pub approval_policy: ApprovalPolicy,
    pub sandbox: SandboxKind,
    pub instruction_sources: Vec<InstructionSource>,
    pub created_at: i64,
    pub updated_at: i64,
    pub title: Option<String>,
    pub starred: bool,
    pub archived_at: Option<i64>,
    pub ephemeral: bool,
    pub forked_from_id: Option<ThreadId>,
    pub summary_message_id: Option<MessageId>,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub items: Vec<MessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub title: Option<String>,
    pub workspace_id: WorkspaceId,
    pub message_count: i64,
    pub total_cost_usd: f64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    AskAlways,
    AskOnRisky,
    AskOnFailure,
    YoloMode,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self::AskOnRisky
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SandboxKind {
    MacosSeatbelt,
    LinuxBwrapLandlock,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InstructionSource {
    pub path: String,
    pub bytes: u64,
    pub is_global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    pub id: MessageId,
    pub session_id: ThreadId,
    pub role: String,
    pub parts: Vec<serde_json::Value>,
    pub model: Option<String>,
    pub turn_id: Option<TurnId>,
    pub created_at: i64,
    pub finish_reason: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    pub id: TurnId,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}
