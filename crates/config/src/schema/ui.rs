//! UI appearance and behaviour configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// UI appearance and behaviour settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    /// Theme preference: "system" | "light" | "dark" | "dim".
    #[serde(default = "default_theme")]
    pub theme: String,

    /// UI scale multiplier (0.75–1.5).
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,

    /// Primary UI font family.
    #[serde(default = "default_ui_font_family")]
    pub ui_font_family: String,

    /// Monospace font family used in code blocks and diffs.
    #[serde(default = "default_code_font_family")]
    pub code_font_family: String,

    /// Code font size in points.
    #[serde(default = "default_code_font_size")]
    pub code_font_size: u32,

    /// Play notification sounds.
    #[serde(default = "default_true")]
    pub notification_sounds_enabled: bool,

    /// Show OS-native notification banners.
    #[serde(default = "default_true")]
    pub system_notifications_enabled: bool,

    /// Show OS-native notification banners for subagent activity.
    #[serde(default = "default_true")]
    pub subagent_system_notifications_enabled: bool,

    /// Automatically generate thread titles from first message.
    #[serde(default)]
    pub thread_title_autogeneration_enabled: bool,

    /// Check for app updates on startup.
    #[serde(default = "default_true")]
    pub automatic_app_update_checks_enabled: bool,

    /// Max chat items to keep in memory (`null` = unlimited).
    #[serde(default = "default_chat_history_scrollback_items")]
    pub chat_history_scrollback_items: Option<u32>,

    /// Show file path in message headers.
    #[serde(default = "default_true")]
    pub show_message_file_path: bool,

    /// Split diff view in chat (side-by-side vs inline).
    #[serde(default)]
    pub split_chat_diff_view: bool,

    /// Show remaining token / credit estimates in usage bar.
    #[serde(default)]
    pub usage_show_remaining: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            ui_scale: default_ui_scale(),
            ui_font_family: default_ui_font_family(),
            code_font_family: default_code_font_family(),
            code_font_size: default_code_font_size(),
            notification_sounds_enabled: true,
            system_notifications_enabled: true,
            subagent_system_notifications_enabled: true,
            thread_title_autogeneration_enabled: false,
            automatic_app_update_checks_enabled: true,
            chat_history_scrollback_items: default_chat_history_scrollback_items(),
            show_message_file_path: true,
            split_chat_diff_view: false,
            usage_show_remaining: false,
        }
    }
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_ui_scale() -> f64 {
    1.0
}

fn default_ui_font_family() -> String {
    "Inter".to_string()
}

fn default_code_font_family() -> String {
    "JetBrains Mono".to_string()
}

fn default_code_font_size() -> u32 {
    11
}

fn default_chat_history_scrollback_items() -> Option<u32> {
    Some(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ui_config_values() {
        let ui = UiConfig::default();
        assert_eq!(ui.theme, "system");
        assert!((ui.ui_scale - 1.0).abs() < f64::EPSILON);
        assert_eq!(ui.ui_font_family, "Inter");
        assert_eq!(ui.code_font_family, "JetBrains Mono");
        assert_eq!(ui.code_font_size, 11);
        assert!(ui.notification_sounds_enabled);
        assert!(ui.system_notifications_enabled);
        assert!(ui.subagent_system_notifications_enabled);
        assert!(!ui.thread_title_autogeneration_enabled);
        assert!(ui.automatic_app_update_checks_enabled);
        assert_eq!(ui.chat_history_scrollback_items, Some(100));
        assert!(ui.show_message_file_path);
        assert!(!ui.split_chat_diff_view);
        assert!(!ui.usage_show_remaining);
    }

    #[test]
    fn ui_config_serde_roundtrip() {
        let ui = UiConfig {
            theme: "dark".to_string(),
            ui_scale: 1.25,
            ui_font_family: "SF Pro".to_string(),
            code_font_family: "Fira Code".to_string(),
            code_font_size: 13,
            notification_sounds_enabled: false,
            system_notifications_enabled: true,
            subagent_system_notifications_enabled: false,
            thread_title_autogeneration_enabled: true,
            automatic_app_update_checks_enabled: false,
            chat_history_scrollback_items: None,
            show_message_file_path: false,
            split_chat_diff_view: true,
            usage_show_remaining: true,
        };
        let json = serde_json::to_string(&ui).unwrap();
        let loaded: UiConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.theme, "dark");
        assert!((loaded.ui_scale - 1.25).abs() < f64::EPSILON);
        assert_eq!(loaded.ui_font_family, "SF Pro");
        assert_eq!(loaded.code_font_family, "Fira Code");
        assert_eq!(loaded.code_font_size, 13);
        assert!(!loaded.notification_sounds_enabled);
        assert!(loaded.system_notifications_enabled);
        assert!(!loaded.subagent_system_notifications_enabled);
        assert!(loaded.thread_title_autogeneration_enabled);
        assert!(!loaded.automatic_app_update_checks_enabled);
        assert_eq!(loaded.chat_history_scrollback_items, None);
        assert!(!loaded.show_message_file_path);
        assert!(loaded.split_chat_diff_view);
        assert!(loaded.usage_show_remaining);
    }

    #[test]
    fn ui_config_deserializes_partial() {
        let json = r#"{"theme": "dim", "uiScale": 0.9}"#;
        let loaded: UiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.theme, "dim");
        assert!((loaded.ui_scale - 0.9).abs() < f64::EPSILON);
        // Remaining fields fall back to defaults
        assert_eq!(loaded.code_font_size, 11);
        assert!(loaded.notification_sounds_enabled);
    }

    #[test]
    fn ui_config_unlimited_scrollback() {
        let json = r#"{"chatHistoryScrollbackItems": null}"#;
        let loaded: UiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.chat_history_scrollback_items, None);
    }
}
