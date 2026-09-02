use serde::{Deserialize, Serialize};

use super::tasks::TaskResponse;

// ── Agent Status ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusResponse {
    pub status: String,
    pub active_task_count: i64,
    pub focus_task: Option<TaskResponse>,
}

// ── MCP Settings ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigResponse {
    pub enabled: bool,
    pub servers: Vec<McpServerResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpServerResponse {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_provider: Option<String>,
    pub oauth_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAddServerParams {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpToggleParams {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpRemoveParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartParams {
    pub provider: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpUpdateServerParams {
    pub name: String,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Persistent embedded MCP server status (distinct from client server list).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedMcpStatusResponse {
    /// `ready` | `disabled` | `invalid`
    pub state: String,
    pub requested: Vec<String>,
    /// Effective builtins + registry tools advertised when Ready.
    pub effective: Vec<String>,
    pub rejected: Vec<EmbeddedMcpRejection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedMcpRejection {
    pub name: String,
    /// `unknown` | `forbidden`
    pub reason: String,
}

// ── App Info ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoResponse {
    pub version: String,
    pub data_dir: String,
    pub setup_completed: bool,
}
