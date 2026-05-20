//! Rich metadata for tool discovery and categorization.

use serde::{Deserialize, Serialize};

/// Category for organizing tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    #[default]
    General,
    FileSystem,
    Search,
    Web,
    Communication,
    TaskManagement,
    Memory,
    Finance,
    Productivity,
    System,
    Mcp,
    Plugin,
}

impl std::fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::General => write!(f, "General"),
            Self::FileSystem => write!(f, "File System"),
            Self::Search => write!(f, "Search"),
            Self::Web => write!(f, "Web"),
            Self::Communication => write!(f, "Communication"),
            Self::TaskManagement => write!(f, "Task Management"),
            Self::Memory => write!(f, "Memory"),
            Self::Finance => write!(f, "Finance"),
            Self::Productivity => write!(f, "Productivity"),
            Self::System => write!(f, "System"),
            Self::Mcp => write!(f, "MCP"),
            Self::Plugin => write!(f, "Plugin"),
        }
    }
}

/// Where the tool originated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolSource {
    #[default]
    Native,
    Feature(String),
    Mcp(String),
    External,
}

/// Estimated cost per tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum CostHint {
    #[default]
    Free,
    Low,
    Medium,
    High,
    Variable,
}

/// Rich metadata for tool discovery and categorization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMetadata {
    pub category: ToolCategory,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub source: ToolSource,
    pub cost_hint: CostHint,
}
