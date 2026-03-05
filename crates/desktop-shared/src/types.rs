use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    P1,
    P2,
    P3,
    P4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Todo,
    Doing,
    Done,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AreaFilter {
    All,
    Work,
    Personal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    Table,
    Board,
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidebarItem {
    Chat,
    Tasks,
    Okr,
    Calendar,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityKind {
    Task,
    Project,
    Objective,
    Area,
    KeyResult,
    FocusSession,
    Productivity,
}

impl EntityKind {
    /// Parse from a string (case-insensitive). Returns None for unknown kinds.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "task" | "action" => Some(Self::Task),
            "project" => Some(Self::Project),
            "objective" => Some(Self::Objective),
            "area" => Some(Self::Area),
            "key_result" | "keyresult" => Some(Self::KeyResult),
            "focus_session" | "focussession" => Some(Self::FocusSession),
            "productivity" => Some(Self::Productivity),
            _ => None,
        }
    }
}
