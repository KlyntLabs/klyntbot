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
    pub fn auto_allow(layer: ApprovalLayer, reason: String) -> Self {
        Self::Auto {
            allowed: true,
            layer,
            reason,
            rule_matched: None,
        }
    }
    pub fn ask(layer: ApprovalLayer, reason: String) -> Self {
        Self::Ask { layer, reason }
    }
    pub fn layer(&self) -> ApprovalLayer {
        match self {
            Self::Auto { layer, .. } | Self::Ask { layer, .. } => layer.clone(),
            Self::PrivacyDenied { .. } => ApprovalLayer::Privacy,
            _ => ApprovalLayer::DefaultMode,
        }
    }
}
