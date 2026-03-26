use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub kind: LauncherItemKind,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    File,
    Folder,
    Image,
    Document,
    Code,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LauncherItemKind {
    Application {
        path: PathBuf,
        running: bool,
    },
    Task {
        task_id: String,
        status: String,
    },
    Note {
        note_id: String,
        preview: String,
    },
    ClipboardEntry {
        entry_id: i64,
        content_type: ClipboardContentType,
    },
    SystemCommand {
        action: SystemAction,
    },
    Script {
        path: PathBuf,
        name: String,
    },
    Calculator {
        expression: String,
        result: f64,
    },
    Calendar {
        event_id: String,
        starts_at: DateTime<Utc>,
    },
    AiChat {
        query: String,
    },
    File {
        path: PathBuf,
        kind: FileKind,
    },
    ContentMatch {
        path: PathBuf,
        line: u32,
        preview: String,
    },
    Contact {
        name: String,
        email: Option<String>,
        phone: Option<String>,
    },
    SystemPref {
        pane_id: String,
    },
    RunningApp {
        pid: u32,
        path: PathBuf,
    },
    Bookmark {
        url: String,
        browser: String,
    },
    BrowserHistory {
        url: String,
        visited_at: String,
    },
    BrewPackage {
        name: String,
        is_cask: bool,
    },
    SshHost {
        host: String,
        user: Option<String>,
    },
    GitRepo {
        path: PathBuf,
    },
    UrlNavigation {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardContentType {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemAction {
    LockScreen,
    Sleep,
    Restart,
    Shutdown,
    EmptyTrash,
    ToggleDarkMode,
    ToggleDoNotDisturb,
    EjectAll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowAction {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    LeftThird,
    CenterThird,
    RightThird,
    Maximize,
    Center,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub focus: Option<FocusDashboard>,
    pub calendar: Vec<CalendarDashboard>,
    pub tasks: Vec<TaskDashboard>,
    pub productivity: ProductivityDashboard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusDashboard {
    pub task_name: Option<String>,
    pub elapsed_secs: i64,
    pub target_secs: Option<i64>,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDashboard {
    pub event_id: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub minutes_until: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDashboard {
    pub id: String,
    pub title: String,
    pub status: String,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityDashboard {
    pub total_minutes: i64,
    pub top_category: String,
    pub top_category_pct: f64,
    pub score: i64,
}

/// Result from a search provider before ranking
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub item: LauncherItem,
    pub base_score: f64,
}
