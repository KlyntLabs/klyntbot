pub use tools_core::approval_class::{ApprovalClass, ApprovalScope};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalLifetime {
    #[default]
    Once,
    Session,
    Forever,
}

impl ApprovalLifetime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Session => "session",
            Self::Forever => "forever",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ApprovalDecision {
    Once,
    Session,
    Forever,
    Decline { reason: String },
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_serializes_to_lowercase_kebab() {
        assert_eq!(
            serde_json::to_string(&ApprovalClass::Safe).unwrap(),
            "\"safe\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalClass::Sensitive).unwrap(),
            "\"sensitive\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalClass::Destructive).unwrap(),
            "\"destructive\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalClass::Admin).unwrap(),
            "\"admin\""
        );
    }

    #[test]
    fn lifetime_once_is_default() {
        assert_eq!(ApprovalLifetime::default(), ApprovalLifetime::Once);
    }

    #[test]
    fn scope_tool_action_has_no_resource() {
        let s = ApprovalScope::ToolAction;
        assert!(matches!(s, ApprovalScope::ToolAction));
    }

    #[test]
    fn scope_resource_carries_key() {
        let s = ApprovalScope::ToolActionResource("path/to/file".into());
        if let ApprovalScope::ToolActionResource(k) = s {
            assert_eq!(k, "path/to/file");
        } else {
            panic!("wrong variant");
        }
    }
}
