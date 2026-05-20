use serde::{Deserialize, Serialize};

/// Parameters for setting the active desktop view.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveViewParams {
    /// Dashboard identifier (e.g., "tasks", "projects", "notes", "dashboard").
    pub dashboard: String,
    /// Specific entity focused within the dashboard (e.g., "FIRE projection", project ID).
    pub focused_entity: Option<String>,
    /// Human-readable description of what the user is looking at.
    /// Used by the LLM rewriter for context.
    pub description: Option<String>,
}

/// Response when getting the current active view.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveViewResponse {
    pub dashboard: Option<String>,
    pub focused_entity: Option<String>,
    pub description: Option<String>,
}
