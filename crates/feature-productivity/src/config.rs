//! Productivity feature configuration — inlined from config crate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub focus: FocusConfig,
    #[serde(default)]
    pub nudges: NudgeConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,
    #[serde(default = "default_batch_interval")]
    pub batch_write_interval_secs: u64,
    #[serde(default = "default_retention")]
    pub retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusConfig {
    #[serde(default = "default_focus_duration")]
    pub default_duration_mins: u64,
    #[serde(default = "default_break_interval")]
    pub break_interval_mins: u64,
    #[serde(default = "default_break_duration")]
    pub break_duration_mins: u64,
    #[serde(default = "default_max_daily_focus")]
    pub max_daily_focus_hours: u64,
    #[serde(default = "default_true")]
    pub soft_block_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeConfig {
    #[serde(default = "default_true")]
    pub break_reminders: bool,
    #[serde(default = "default_true")]
    pub focus_suggestions: bool,
    #[serde(default = "default_true")]
    pub daily_summary: bool,
    #[serde(default = "default_true")]
    pub burnout_alerts: bool,
    #[serde(default = "default_cooldown")]
    pub cooldown_mins: u64,
    #[serde(default)]
    pub quiet_hours_start: Option<String>,
    #[serde(default)]
    pub quiet_hours_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub exclude_window_titles: bool,
    #[serde(default)]
    pub excluded_url_patterns: Vec<String>,
}

fn default_true() -> bool {
    true
}
fn default_poll_interval() -> u64 {
    5
}
fn default_idle_threshold() -> u64 {
    120
}
fn default_batch_interval() -> u64 {
    30
}
fn default_retention() -> u64 {
    90
}
fn default_focus_duration() -> u64 {
    45
}
fn default_break_interval() -> u64 {
    90
}
fn default_break_duration() -> u64 {
    10
}
fn default_max_daily_focus() -> u64 {
    8
}
fn default_cooldown() -> u64 {
    15
}

impl Default for ProductivityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tracking: TrackingConfig::default(),
            focus: FocusConfig::default(),
            nudges: NudgeConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval(),
            idle_threshold_secs: default_idle_threshold(),
            batch_write_interval_secs: default_batch_interval(),
            retention_days: default_retention(),
        }
    }
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            default_duration_mins: default_focus_duration(),
            break_interval_mins: default_break_interval(),
            break_duration_mins: default_break_duration(),
            max_daily_focus_hours: default_max_daily_focus(),
            soft_block_enabled: true,
        }
    }
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            break_reminders: true,
            focus_suggestions: true,
            daily_summary: true,
            burnout_alerts: true,
            cooldown_mins: default_cooldown(),
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }
}

