use serde::{Deserialize, Serialize};

fn default_scan_dirs() -> Vec<String> {
    vec!["~/Projects".to_string(), "~/Developer".to_string()]
}
fn default_chrome() -> String {
    "chrome".to_string()
}
fn default_30() -> i64 {
    30
}
fn default_dot() -> String {
    ".".to_string()
}
fn default_1000() -> i64 {
    1000
}
fn default_scripts_dir() -> String {
    "~/.klyntbot/scripts".to_string()
}
fn default_file_scan_dirs() -> Vec<String> {
    vec![
        "~/Projects".to_string(),
        "~/Documents".to_string(),
        "~/Desktop".to_string(),
    ]
}
fn default_120() -> u64 {
    120
}
fn default_max_entries() -> usize {
    1_000_000
}
fn default_true() -> bool {
    true
}
fn default_mdfind_threshold() -> usize {
    20
}
fn default_rebuild_interval_min() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherConfig {
    pub enabled: bool,
    pub sources: LauncherSourcesConfig,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: LauncherSourcesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherSourcesConfig {
    pub apps: SourceToggle,
    pub system_prefs: SourceToggle,
    pub brew: SourceToggle,
    pub ssh_hosts: SourceToggle,
    pub git_repos: GitReposConfig,
    pub scripts: ScriptsConfig,
    pub files: FileSearchConfig,
    pub content_grep: ContentGrepConfig,
    pub contacts: SourceToggle,
    pub running_apps: SourceToggle,
    pub bookmarks: BrowserSourceConfig,
    pub browser_history: BrowserHistoryConfig,
    pub tasks: SourceToggle,
    pub notes: SourceToggle,
    pub clipboard: ClipboardSourceConfig,
    pub window_presets: SourceToggle,
    pub calendar: CalendarSourceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceToggle {
    pub enabled: bool,
}

impl Default for SourceToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarSourceConfig {
    pub enabled: bool,
    #[serde(default = "default_lookback")]
    pub lookback_days: u32,
    #[serde(default = "default_lookahead")]
    pub lookahead_days: u32,
}

fn default_lookback() -> u32 { 1 }
fn default_lookahead() -> u32 { 7 }

impl Default for CalendarSourceConfig {
    fn default() -> Self {
        Self { enabled: true, lookback_days: 1, lookahead_days: 7 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitReposConfig {
    pub enabled: bool,
    #[serde(default = "default_scan_dirs")]
    pub scan_dirs: Vec<String>,
}

impl Default for GitReposConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_dirs: default_scan_dirs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptsConfig {
    pub enabled: bool,
    #[serde(default = "default_scripts_dir")]
    pub dir: String,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: default_scripts_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileSearchConfig {
    pub enabled: bool,
    #[serde(default = "default_file_scan_dirs")]
    pub scan_dirs: Vec<String>,
    #[serde(default = "default_120")]
    pub refresh_interval_secs: u64,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    #[serde(default = "default_true")]
    pub mdfind_fallback: bool,
    #[serde(default = "default_mdfind_threshold")]
    pub mdfind_threshold: usize,
    #[serde(default = "default_rebuild_interval_min")]
    pub rebuild_interval_min: u64,
}

impl Default for FileSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_dirs: default_file_scan_dirs(),
            refresh_interval_secs: 120,
            max_entries: default_max_entries(),
            mdfind_fallback: true,
            mdfind_threshold: default_mdfind_threshold(),
            rebuild_interval_min: default_rebuild_interval_min(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserSourceConfig {
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
}

impl Default for BrowserSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            browser: default_chrome(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserHistoryConfig {
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
    #[serde(default = "default_30")]
    pub max_days: i64,
}

impl Default for BrowserHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            browser: default_chrome(),
            max_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentGrepConfig {
    pub enabled: bool,
    #[serde(default = "default_dot")]
    pub default_scope: String,
}

impl Default for ContentGrepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_scope: default_dot(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipboardSourceConfig {
    pub enabled: bool,
    #[serde(default = "default_1000")]
    pub max_entries: i64,
}

impl Default for ClipboardSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
        }
    }
}
