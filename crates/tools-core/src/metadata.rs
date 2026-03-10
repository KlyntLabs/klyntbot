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
        write!(f, "{:?}", self)
    }
}

/// Where the tool originated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolSource {
    #[default]
    Native,
    Feature(String),
    Mcp(String),
    Plugin(String),
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

/// An example usage of the tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    pub description: String,
    pub params: serde_json::Value,
}

/// Rich metadata for tool discovery and categorization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMetadata {
    pub category: ToolCategory,
    pub tags: Vec<String>,
    pub author: String,
    pub version: String,
    pub source: ToolSource,
    pub examples: Vec<ToolExample>,
    pub related_tools: Vec<String>,
    pub cost_hint: CostHint,
}
