use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLayer {
    Privacy,
    Layer1Declarative,
    Layer2Starlark,
    Layer3Mirror,
    DefaultMode,
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Auto {
        allowed: bool,
        layer: ApprovalLayer,
        reason: String,
        rule_matched: Option<String>,
    },
    Ask {
        layer: ApprovalLayer,
        reason: String,
    },
    PrivacyDenied {
        reason: String,
        pattern: String,
    },
    Cancelled,
    TimedOut,
}

impl ApprovalDecision {
    pub fn allowed(&self) -> bool {
        matches!(self, Self::Auto { allowed: true, .. })
    }
    pub fn requires_user_input(&self) -> bool {
        matches!(self, Self::Ask { .. })
    }
}
