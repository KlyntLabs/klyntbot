use serde::{Deserialize, Serialize};

/// Lifecycle monitoring — macOS sleep/wake + user presence detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleConfig {
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,
    #[serde(default = "default_presence_threshold")]
    pub presence_threshold_secs: u64,
    #[serde(default = "default_wake_grace_period")]
    pub wake_grace_period_secs: u64,
    #[serde(default = "default_active_poll")]
    pub active_poll_interval_secs: u64,
    #[serde(default = "default_idle_poll")]
    pub idle_poll_interval_secs: u64,
    #[serde(default)]
    pub wake_delivery: WakeDeliveryConfig,
    #[serde(default)]
    pub disable_smart_scheduling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeDeliveryConfig {
    #[serde(default = "default_min_absence_for_panel")]
    pub min_absence_for_panel_secs: u64,
    #[serde(default = "default_quiet_morning")]
    pub quiet_period_morning_secs: u64,
    #[serde(default = "default_quiet_midday")]
    pub quiet_period_midday_secs: u64,
    #[serde(default = "default_quiet_evening")]
    pub quiet_period_evening_secs: u64,
    #[serde(default = "default_quiet_default")]
    pub quiet_period_default_secs: u64,
    #[serde(default = "default_tier_stagger")]
    pub catch_up_tier_stagger_secs: u64,
    #[serde(default = "default_idle_resume_threshold")]
    pub idle_resume_prompt_threshold_secs: u64,
    #[serde(default = "default_nudge_consolidation")]
    pub nudge_consolidation_threshold_secs: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            idle_threshold_secs: default_idle_threshold(),
            presence_threshold_secs: default_presence_threshold(),
            wake_grace_period_secs: default_wake_grace_period(),
            active_poll_interval_secs: default_active_poll(),
            idle_poll_interval_secs: default_idle_poll(),
            wake_delivery: WakeDeliveryConfig::default(),
            disable_smart_scheduling: false,
        }
    }
}

impl Default for WakeDeliveryConfig {
    fn default() -> Self {
        Self {
            min_absence_for_panel_secs: default_min_absence_for_panel(),
            quiet_period_morning_secs: default_quiet_morning(),
            quiet_period_midday_secs: default_quiet_midday(),
            quiet_period_evening_secs: default_quiet_evening(),
            quiet_period_default_secs: default_quiet_default(),
            catch_up_tier_stagger_secs: default_tier_stagger(),
            idle_resume_prompt_threshold_secs: default_idle_resume_threshold(),
            nudge_consolidation_threshold_secs: default_nudge_consolidation(),
        }
    }
}

fn default_idle_threshold() -> u64 {
    300
}
fn default_presence_threshold() -> u64 {
    2
}
fn default_wake_grace_period() -> u64 {
    60
}
fn default_active_poll() -> u64 {
    10
}
fn default_idle_poll() -> u64 {
    30
}
fn default_min_absence_for_panel() -> u64 {
    1800
}
fn default_quiet_morning() -> u64 {
    45
}
fn default_quiet_midday() -> u64 {
    15
}
fn default_quiet_evening() -> u64 {
    60
}
fn default_quiet_default() -> u64 {
    30
}
fn default_tier_stagger() -> u64 {
    120
}
fn default_idle_resume_threshold() -> u64 {
    600
}
fn default_nudge_consolidation() -> u64 {
    1800
}
