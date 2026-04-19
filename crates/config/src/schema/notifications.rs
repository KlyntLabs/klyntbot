//! Notification system configuration.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsConfig {
    #[serde(default)]
    pub quiet_hours: QuietHoursConfig,
    #[serde(default = "default_channels")]
    pub default_channels: Vec<String>,
    #[serde(default = "default_misfire_policy")]
    pub default_misfire_policy: String,
    #[serde(default = "default_grace_window_secs")]
    pub default_grace_window_secs: i64,
    #[serde(default)]
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietHoursConfig {
    pub enabled: bool,
    pub start: String,
    pub end: String,
    #[serde(default = "default_true")]
    pub override_for_urgent_tasks: bool,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".into(),
            end: "07:00".into(),
            override_for_urgent_tasks: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_secs: 1,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_channels() -> Vec<String> {
    vec!["os_native".into(), "tray".into()]
}
fn default_misfire_policy() -> String {
    "skip_if_stale".into()
}
fn default_grace_window_secs() -> i64 {
    3600
}
