use serde::{Deserialize, Serialize};

/// Summary of a single agent file (AGENT.md or a skill).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileSummary {
    /// e.g. "AGENT.md" or "skills/todo.md"
    pub filename: String,
    /// Display name parsed from frontmatter (e.g. "todo", "daily-planner")
    pub display_name: String,
    /// Description from frontmatter
    pub description: String,
    /// Whether this file comes from the compiled-in builtin agents
    pub is_builtin: bool,
    /// Whether a workspace override exists for a builtin file
    pub has_override: bool,
}

/// A full agent profile listing with its files.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfileSummary {
    pub name: String,
    pub description: String,
    pub is_builtin: bool,
    pub has_override: bool,
    pub files: Vec<AgentFileSummary>,
}

/// Content of a single agent file.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileContent {
    pub agent_name: String,
    pub filename: String,
    pub content: String,
    pub is_builtin: bool,
}
