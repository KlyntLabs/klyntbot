use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArgSpec {
    pub name: String,
    pub placeholder: String,
    pub kind: ArgKind,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", content = "values", rename_all = "camelCase")]
pub enum ArgKind {
    Text,
    Number,
    Choice(Vec<String>),
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LauncherItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub kind: LauncherItemKind,
    pub score: f64,
    #[serde(default)]
    pub no_view: bool,
    #[serde(default)]
    pub arguments: Vec<ArgSpec>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    File,
    Folder,
    Image,
    Document,
    Code,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
        #[specta(type = String)]
        starts_at: Timestamp,
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
    WindowAction {
        action: crate::types::WindowAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardContentType {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
    Preset(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub calendar: Vec<CalendarDashboard>,
    pub tasks: Vec<TaskDashboard>,
    pub productivity: ProductivityDashboard,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDashboard {
    pub event_id: String,
    pub title: String,
    #[specta(type = String)]
    pub starts_at: Timestamp,
    #[specta(type = String)]
    pub ends_at: Timestamp,
    pub minutes_until: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskDashboard {
    pub id: String,
    pub title: String,
    pub status: String,
    pub project_name: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityDashboard {
    pub total_minutes: i64,
    pub top_category: String,
    pub top_category_pct: f64,
    pub score: i64,
}

/// Result from a search provider before ranking
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchResult {
    pub item: LauncherItem,
    pub base_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LauncherExecuteResult {
    pub status: ExecStatus,
    pub message: Option<String>,
    pub badge: BadgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum ExecStatus {
    Ok,
    Err(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BadgeKind {
    Success,
    Warn,
    Error,
    Info,
}

impl Default for LauncherExecuteResult {
    fn default() -> Self {
        Self {
            status: ExecStatus::Ok,
            message: None,
            badge: BadgeKind::Info,
        }
    }
}

impl From<()> for LauncherExecuteResult {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl LauncherExecuteResult {
    pub fn ok_msg(msg: impl Into<String>) -> Self {
        Self {
            status: ExecStatus::Ok,
            message: Some(msg.into()),
            badge: BadgeKind::Success,
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        let m = msg.into();
        Self {
            status: ExecStatus::Err(m.clone()),
            message: Some(m),
            badge: BadgeKind::Error,
        }
    }
}
