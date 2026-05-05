use serde::{Deserialize, Serialize};

use desktop_shared::coding::LayerOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLayer {
    Privacy,
    Layer1Declarative,
    Layer2Starlark,
    Layer3Mirror,
    DefaultMode,
}

impl std::fmt::Display for ApprovalLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Privacy => write!(f, "privacy"),
            Self::Layer1Declarative => write!(f, "layer1_declarative"),
            Self::Layer2Starlark => write!(f, "layer2_starlark"),
            Self::Layer3Mirror => write!(f, "layer3_mirror"),
            Self::DefaultMode => write!(f, "default_mode"),
        }
    }
}

/// Audit trace of each layer's outcome when an `Ask` decision is produced.
/// Passed through to the frontend so the user can see *why* they're being asked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerOutcomeAudit {
    pub privacy_passed: bool,
    pub layer1: LayerOutcome,
    pub layer2: LayerOutcome,
    pub layer3: LayerOutcome,
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
    pub fn auto_allow(
        layer: ApprovalLayer,
        reason: impl Into<String>,
        rule_matched: Option<String>,
    ) -> Self {
        Self::Auto {
            allowed: true,
            layer,
            reason: reason.into(),
            rule_matched,
        }
    }
    pub fn auto_deny(
        layer: ApprovalLayer,
        reason: impl Into<String>,
        rule_matched: Option<String>,
    ) -> Self {
        Self::Auto {
            allowed: false,
            layer,
            reason: reason.into(),
            rule_matched,
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
    pub fn rule_matched(&self) -> Option<String> {
        if let Self::Auto { rule_matched, .. } = self {
            rule_matched.clone()
        } else {
            None
        }
    }
    pub fn reason(&self) -> String {
        match self {
            Self::Auto { reason, .. } | Self::Ask { reason, .. } => reason.clone(),
            Self::PrivacyDenied { reason, .. } => reason.clone(),
            Self::Cancelled => "cancelled".into(),
            Self::TimedOut => "timeout".into(),
        }
    }
    pub fn decided_by(&self) -> &'static str {
        match self {
            Self::Auto { allowed: true, .. } => "auto_allow",
            Self::Auto { allowed: false, .. } => "auto_deny",
            Self::Ask { .. } => "user",
            Self::PrivacyDenied { .. } => "auto_deny",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timeout",
        }
    }
}
