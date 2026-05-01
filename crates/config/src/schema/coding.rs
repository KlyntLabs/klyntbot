use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_true")]
    pub auto_detect_from_workspace: bool,
    #[serde(default = "default_tool_profile")]
    pub tool_profile: String,
    #[serde(default)]
    pub permissions: CodingPermissions,
    #[serde(default)]
    pub sandbox: CodingSandbox,
    #[serde(default)]
    pub skills: super::coding_memory::CodingSkillsConfig,
    #[serde(default)]
    pub sessions: CodingSessionsConfig,
    #[serde(default)]
    pub cost_ceiling: CostCeilingConfig,
}

impl Default for CodingConfig {
    fn default() -> Self {
        Self {
            default_mode: default_mode(),
            auto_detect_from_workspace: true,
            tool_profile: default_tool_profile(),
            permissions: Default::default(),
            sandbox: Default::default(),
            skills: Default::default(),
            sessions: Default::default(),
            cost_ceiling: Default::default(),
        }
    }
}

fn default_mode() -> String {
    "general".into()
}

fn default_tool_profile() -> String {
    "curated".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingPermissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default = "default_match")]
    pub default_if_no_match: String,
    #[serde(default)]
    pub mirror_learning: bool,
    #[serde(default = "default_mirror_min_approvals")]
    pub mirror_min_approvals: u32,
    #[serde(default = "default_mirror_cooldown_hours")]
    pub mirror_cooldown_hours: u32,
}
fn default_match() -> String {
    "ask".into()
}
fn default_mirror_min_approvals() -> u32 { 5 }
fn default_mirror_cooldown_hours() -> u32 { 24 }
impl Default for CodingPermissions {
    fn default() -> Self {
        Self {
            allow: vec![],
            deny: vec![],
            ask: vec![],
            default_if_no_match: "ask".into(),
            mirror_learning: false,
            mirror_min_approvals: default_mirror_min_approvals(),
            mirror_cooldown_hours: default_mirror_cooldown_hours(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSandbox {
    #[serde(default = "default_true")]
    pub enforce: bool,
}
fn default_true() -> bool {
    true
}
impl Default for CodingSandbox {
    fn default() -> Self {
        Self { enforce: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSessionsConfig {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_total_disk_mb")]
    pub max_total_disk_mb: u32,
    #[serde(default = "default_true")]
    pub preserve_starred: bool,
}

fn default_retention_days() -> u32 {
    90
}
fn default_max_total_disk_mb() -> u32 {
    5120
}

impl Default for CodingSessionsConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            max_total_disk_mb: default_max_total_disk_mb(),
            preserve_starred: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostCeilingConfig {
    #[serde(default)]
    pub per_thread_usd: Option<f64>,
    #[serde(default = "default_cost_alert_pct")]
    pub alert_at_percent: u32,
}

fn default_cost_alert_pct() -> u32 { 80 }

impl Default for CostCeilingConfig {
    fn default() -> Self {
        Self {
            per_thread_usd: None,
            alert_at_percent: default_cost_alert_pct(),
        }
    }
}
