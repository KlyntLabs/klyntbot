use serde::{Deserialize, Serialize};
use specta::Type;

use super::thread::{ThreadId, TurnId};

pub type ApprovalId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRequest {
    CommandExecution {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        command: Vec<String>,
        cwd: String,
        reason: String,
        proposed_execpolicy_amendment: Option<ExecpolicyAmendment>,
        layer_decisions: LayerDecisions,
    },
    FileChange {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        path: String,
        diff_unified: String,
        write_kind: WriteKind,
        layer_decisions: LayerDecisions,
    },
    UserInput {
        approval_id: ApprovalId,
        thread_id: ThreadId,
        turn_id: TurnId,
        prompt: String,
        questions: Vec<UserInputQuestion>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WriteKind {
    Create,
    Modify,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecpolicyAmendment {
    pub command_pattern: Vec<String>,
    pub starlark_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UserInputQuestion {
    pub id: String,
    pub prompt: String,
    pub kind: String,
    pub options: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LayerDecisions {
    pub privacy: LayerOutcome,
    pub layer1: LayerOutcome,
    pub layer2: LayerOutcome,
    pub layer3: LayerOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LayerOutcome {
    Allowed {
        reason: String,
        rule_matched: Option<String>,
    },
    Denied {
        reason: String,
        rule_matched: Option<String>,
    },
    Deferred {
        reason: String,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalDecisionDto {
    Accept,
    Decline,
    AcceptForSession,
    AcceptWithExecpolicyAmendment {
        execpolicy_amendment: ExecpolicyAmendment,
    },
    Cancel,
}
