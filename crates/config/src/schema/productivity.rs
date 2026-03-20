//! Productivity tracking configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Top-level productivity config section.
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
    #[serde(default = "default_raw_retention")]
    pub raw_retention_days: u64,
    #[serde(default = "default_bucket_retention")]
    pub bucket_retention_days: u64,
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
    #[serde(default = "default_soft_block_cooldown")]
    pub soft_block_cooldown_secs: u64,
    #[serde(default = "default_temp_pass_mins")]
    pub soft_block_temp_pass_mins: u64,
    #[serde(default = "default_true")]
    pub soft_block_llm_enabled: bool,
    #[serde(default = "default_llm_timeout")]
    pub soft_block_llm_timeout_ms: u64,
    #[serde(default = "default_learned_rule_threshold")]
    pub learned_rule_threshold: u64,
    #[serde(default = "default_true")]
    pub auto_detect_enabled: bool,
    #[serde(default = "default_auto_detect_min_mins")]
    pub auto_detect_min_mins: u64,
    #[serde(default = "default_auto_detect_productive_threshold")]
    pub auto_detect_productive_threshold: f64,
    #[serde(default = "default_auto_detect_max_switches")]
    pub auto_detect_max_switches: u64,
    #[serde(default = "default_cooldown_grace_secs")]
    pub cooldown_grace_secs: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConfig {
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub exclude_window_titles: bool,
    #[serde(default)]
    pub excluded_url_patterns: Vec<String>,
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
fn default_raw_retention() -> u64 {
    7
}
fn default_bucket_retention() -> u64 {
    365
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
fn default_soft_block_cooldown() -> u64 {
    60
}
fn default_temp_pass_mins() -> u64 {
    5
}
fn default_llm_timeout() -> u64 {
    3000
}
fn default_learned_rule_threshold() -> u64 {
    3
}
fn default_auto_detect_min_mins() -> u64 {
    15
}
fn default_auto_detect_productive_threshold() -> f64 {
    0.8
}
fn default_auto_detect_max_switches() -> u64 {
    2
}
fn default_cooldown_grace_secs() -> u64 {
    30
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
            raw_retention_days: default_raw_retention(),
            bucket_retention_days: default_bucket_retention(),
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
            soft_block_cooldown_secs: default_soft_block_cooldown(),
            soft_block_temp_pass_mins: default_temp_pass_mins(),
            soft_block_llm_enabled: true,
            soft_block_llm_timeout_ms: default_llm_timeout(),
            learned_rule_threshold: default_learned_rule_threshold(),
            auto_detect_enabled: true,
            auto_detect_min_mins: default_auto_detect_min_mins(),
            auto_detect_productive_threshold: default_auto_detect_productive_threshold(),
            auto_detect_max_switches: default_auto_detect_max_switches(),
            cooldown_grace_secs: default_cooldown_grace_secs(),
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
