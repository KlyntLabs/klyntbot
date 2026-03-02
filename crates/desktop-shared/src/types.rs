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
}

impl EntityKind {
    /// Parse from a string (case-insensitive). Defaults to Task for unknown kinds.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "task" | "action" => Self::Task,
            "project" => Self::Project,
            "objective" => Self::Objective,
            "area" => Self::Area,
            "key_result" | "keyresult" => Self::KeyResult,
            _ => Self::Task,
        }
    }
}
