use serde::{Deserialize, Serialize};

// ── AI Tool Integration ─────────────────────────────────────────────

/// Info about a detected AI coding tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolInfo {
    /// Machine ID (e.g., "claude-code", "cursor").
    pub id: String,
    /// Human-readable name (e.g., "Claude Code", "Cursor").
    pub name: String,
    /// Whether the tool was detected on this machine.
    pub detected: bool,
    /// How it was detected (e.g., "Found ~/.claude.json").
    pub detection_hint: String,
    /// Icon identifier for the UI.
    pub icon: String,
}

/// Params for installing skills to selected AI tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolsInstallParams {
    /// List of tool IDs to install for (e.g., ["claude-code", "cursor"]).
    pub tools: Vec<String>,
}

/// Result of installing skills to one AI tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolInstallResult {
    pub tool_id: String,
    pub tool_name: String,
    pub success: bool,
    pub files_written: Vec<String>,
    pub message: String,
}
