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

/// Audit trace of each layer's outcome when an `Ask` decision is produced.
/// Passed through to the frontend so the user can see *why* they're being asked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerOutcomeAudit {
    pub privacy_passed: bool,
    pub layer1: String,
    pub layer2: String,
    pub layer3: String,
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
        layer_audit: Option<LayerOutcomeAudit>,
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
    pub fn ask(layer: ApprovalLayer, reason: impl Into<String>) -> Self {
        Self::Ask {
            layer,
            reason: reason.into(),
            layer_audit: None,
        }
    }
    pub fn ask_with_audit(
        layer: ApprovalLayer,
        reason: impl Into<String>,
        audit: LayerOutcomeAudit,
    ) -> Self {
        Self::Ask {
            layer,
            reason: reason.into(),
            layer_audit: Some(audit),
        }
    }
    pub fn layer(&self) -> ApprovalLayer {
        match self {
            Self::Auto { layer, .. } | Self::Ask { layer, .. } => layer.clone(),
            Self::PrivacyDenied { .. } => ApprovalLayer::Privacy,
            _ => ApprovalLayer::DefaultMode,
        }
    }
}
