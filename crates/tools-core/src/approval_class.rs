use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalClass {
    Safe,
    Sensitive,
    Destructive,
    Admin,
}

impl ApprovalClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Sensitive => "sensitive",
            Self::Destructive => "destructive",
            Self::Admin => "admin",
        }
    }

    pub fn requires_prompt_on_remote(&self) -> bool {
        matches!(self, Self::Destructive | Self::Admin)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum ApprovalScope {
    ToolAction,
    ToolActionResource(String),
}

impl ApprovalScope {
    pub fn resource_key(&self) -> Option<&str> {
        match self {
            Self::ToolAction => None,
            Self::ToolActionResource(k) => Some(k.as_str()),
        }
    }
}
