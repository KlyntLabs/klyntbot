//! Configuration schema using serde.
//!
//! Split into submodules by config section:
//! - `core`: Secret, Config (root composition)
//! - `agents`: AgentsConfig, AgentDefaults
//! - `channels`: All chat channel configuration structs
//! - `confidence`: ConfidenceConfig
//! - `conversation`: ConversationConfig, embedding/search
//! - `finance`: FinanceConfig and sub-structs
//! - `gateway`: GatewayConfig
//! - `learning`: LearningConfig
//! - `project`: ProjectConfig
//! - `providers`: LLM provider configuration structs
//! - `todo`: TodoConfig, enrichment, notifications, focus, search
//! - `tools`: ToolsConfig, permissions, exec, web

mod agents;
mod autotuner;
mod capture;
mod channels;
mod cognitive;
mod confidence;
mod content;
mod conversation;
pub(crate) mod core;
mod execution;
mod finance;
mod gateway;
pub mod history_compression;
pub mod hot;
mod integrations;
mod language;
mod language_learning;
mod launcher;
mod learning;
mod lifecycle;
mod mcp;
pub use mcp::EXPLICIT_TOOL_ALLOWLIST;
mod notes;
mod notifications;
mod packs;
mod productivity;
mod project;
mod providers;
mod scenario;
mod shortcuts;
mod todo;
mod tools;
mod user;
mod voice;
mod work_context;

pub use self::agents::*;
pub use self::autotuner::*;
pub use self::capture::*;
pub use self::channels::*;
pub use self::cognitive::*;
pub use self::confidence::*;
pub use self::content::*;
pub use self::conversation::*;
pub use self::core::*;
pub use self::execution::*;
pub use self::finance::*;
pub use self::gateway::*;
pub use self::history_compression::*;
pub use self::integrations::*;
pub use self::language::*;
pub use self::language_learning::*;
pub use self::launcher::*;
pub use self::learning::*;
pub use self::lifecycle::*;
pub use self::mcp::*;
pub use self::notes::*;
pub use self::notifications::*;
pub use self::packs::*;
pub use self::productivity::*;
pub use self::project::*;
pub use self::providers::*;
pub use self::scenario::*;
pub use self::shortcuts::*;
pub use self::todo::*;
pub use self::tools::*;
pub use self::user::*;
pub use self::voice::*;
pub use self::work_context::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.agents.defaults.model, DEFAULT_MODEL);
        assert_eq!(config.agents.defaults.max_tokens, 8192);
        assert_eq!(config.agents.defaults.temperature, 0.7);
        assert_eq!(config.agents.defaults.max_tool_iterations, 20);
        assert_eq!(config.agents.defaults.workspace, "~/.klyntbot/workspace");
    }

    #[test]
    fn test_config_workspace_path_tilde_expansion() {
        let mut config = Config::default();
        config.agents.defaults.workspace = "~/.klyntbot/workspace".to_string();

        let workspace = config.workspace_path();
        assert!(workspace.is_absolute());
        assert!(workspace.to_string_lossy().contains(".klyntbot"));
        assert!(workspace.to_string_lossy().contains("workspace"));
    }

    #[test]
    fn test_config_workspace_path_absolute() {
        let mut config = Config::default();
        config.agents.defaults.workspace = "/absolute/path/workspace".to_string();

        let workspace = config.workspace_path();
        assert_eq!(workspace, PathBuf::from("/absolute/path/workspace"));
    }

    #[test]
    fn test_agent_defaults_serialization() {
        let defaults = AgentDefaults::default();
        let json = serde_json::to_value(&defaults).unwrap();

        assert_eq!(json["model"], "anthropic/claude-opus-4-5");
        assert_eq!(json["maxTokens"], 8192);
        let temp = json["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7).abs() < 0.01,
            "temperature should be ~0.7, got {}",
            temp
        );
        assert_eq!(json["maxToolIterations"], 20);
    }

    #[test]
    fn test_tools_config_default() {
        let config = ToolsConfig::default();
        assert!(!config.restrict_to_workspace);
        assert_eq!(config.web.brave_api_key.expose(), "");
    }

    #[test]
    fn test_web_tools_config() {
        let config = WebToolsConfig {
            brave_api_key: Secret::new("brave-api-key-123".to_string()),
            max_results: 5,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["braveApiKey"], "brave-api-key-123");
        assert_eq!(json["maxResults"], 5);
    }

    #[test]
    fn test_todo_config_default() {
        let config = TodoConfig::default();
        assert_eq!(config.focus.max_slots, 3);
        assert_eq!(config.focus.deadline_hours, 18);
        assert_eq!(config.notifications.targets, vec!["os_native"]);
        assert!(config.notifications.focus_reminders);
        assert!(config.notifications.daily_digest);
        assert_eq!(config.notifications.daily_digest_time, "09:00");
    }

    #[test]
    fn test_todo_notification_config_default() {
        let config = TodoNotificationConfig::default();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0], "os_native");
        assert!(config.focus_reminders);
        assert!(config.daily_digest);
        assert_eq!(config.daily_digest_time, "09:00");
    }

    #[test]
    fn test_todo_focus_config_default() {
        let config = TodoFocusConfig::default();
        assert_eq!(config.max_slots, 3);
        assert_eq!(config.deadline_hours, 18);
    }

    #[test]
    fn test_config_with_todo_field() {
        let config = Config::default();
        assert_eq!(config.todo.focus.max_slots, 3);
    }

    #[test]
    fn test_todo_config_serialization() {
        let config = TodoConfig::default();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["focus"]["maxSlots"], 3);
        assert_eq!(json["focus"]["deadlineHours"], 18);
        assert_eq!(json["notifications"]["targets"][0], "os_native");
        assert_eq!(json["notifications"]["focusReminders"], true);
        assert_eq!(json["notifications"]["dailyDigest"], true);
        assert_eq!(json["notifications"]["dailyDigestTime"], "09:00");
    }

    #[test]
    fn test_full_config_serialization_camel_case() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains("maxTokens"));
        assert!(json.contains("maxToolIterations"));
        assert!(json.contains("allowFrom"));
        assert!(json.contains("botToken"));
        assert!(json.contains("appToken"));
        assert!(json.contains("imapHost"));
        assert!(json.contains("imapPort"));
        assert!(json.contains("smtpHost"));
        assert!(json.contains("apiKey"));
        assert!(json.contains("restrictToWorkspace"));
        assert!(json.contains("braveApiKey"));
    }

    #[test]
    fn test_config_round_trip() {
        let mut config = Config::default();
        config.agents.defaults.model = "test-model".to_string();
        config.agents.defaults.max_tokens = 4096;
        config.providers.anthropic.api_key = Secret::new("sk-test-123".to_string());
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = Secret::new("bot-token".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.agents.defaults.model, "test-model");
        assert_eq!(loaded.agents.defaults.max_tokens, 4096);
        assert_eq!(loaded.providers.anthropic.api_key.expose(), "sk-test-123");
        assert!(loaded.channels.telegram.enabled);
        assert_eq!(loaded.channels.telegram.token.expose(), "bot-token");
    }

    #[test]
    fn test_config_with_multiple_providers() {
        let mut config = Config::default();
        config.providers.anthropic.api_key = Secret::new("anthropic-key".to_string());
        config.providers.openai.api_key = Secret::new("openai-key".to_string());
        config.providers.deepseek.api_key = Secret::new("deepseek-key".to_string());
        config.providers.openrouter.api_key = Secret::new("sk-or-v1-test".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.providers.anthropic.api_key.expose(), "anthropic-key");
        assert_eq!(loaded.providers.openai.api_key.expose(), "openai-key");
        assert_eq!(loaded.providers.deepseek.api_key.expose(), "deepseek-key");
        assert_eq!(
            loaded.providers.openrouter.api_key.expose(),
            "sk-or-v1-test"
        );
    }

    #[test]
    fn test_config_with_custom_api_bases() {
        let mut config = Config::default();
        config.providers.anthropic.api_base = Some("https://custom-anthropic.com/v1".to_string());
        config.providers.openai.api_base = Some("https://custom-openai.com/v1".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(
            loaded.providers.anthropic.api_base,
            Some("https://custom-anthropic.com/v1".to_string())
        );
        assert_eq!(
            loaded.providers.openai.api_base,
            Some("https://custom-openai.com/v1".to_string())
        );
    }

    #[test]
    fn test_config_with_allow_from_lists() {
        let mut config = Config::default();
        config.channels.telegram.allow_from = vec!["123".to_string(), "456".to_string()];
        config.channels.discord.allow_from = vec!["guild1".to_string()];
        config.channels.slack.allow_from = vec!["channel1".to_string(), "channel2".to_string()];

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.channels.telegram.allow_from.len(), 2);
        assert_eq!(loaded.channels.discord.allow_from.len(), 1);
        assert_eq!(loaded.channels.slack.allow_from.len(), 2);
    }

    #[test]
    fn test_config_without_todo_deserializes() {
        let json = r#"{
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.todo.focus.max_slots, 3);
        assert_eq!(config.todo.notifications.targets, vec!["os_native"]);
    }

    #[test]
    fn test_todo_config_round_trip() {
        let config = TodoConfig {
            focus: TodoFocusConfig {
                max_slots: 5,
                deadline_hours: 24,
            },
            notifications: TodoNotificationConfig {
                targets: vec!["os_native".to_string(), "telegram".to_string()],
                daily_digest_time: "08:00".to_string(),
                ..Default::default()
            },
            enrichment: TodoEnrichmentConfig::default(),
            search: TodoSearchConfig::default(),
            daily_planning: DailyPlanningConfig::default(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: TodoConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.focus.max_slots, 5);
        assert_eq!(loaded.focus.deadline_hours, 24);
        assert_eq!(loaded.notifications.targets.len(), 2);
        assert_eq!(loaded.notifications.daily_digest_time, "08:00");
    }

    #[test]
    fn test_todo_config_camel_case_json() {
        let config = TodoConfig::default();
        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains("maxSlots"));
        assert!(json.contains("deadlineHours"));
        assert!(json.contains("focusReminders"));
        assert!(json.contains("dailyDigest"));
        assert!(json.contains("dailyDigestTime"));
    }

    #[test]
    fn test_partial_notification_config_deserializes() {
        let json = r#"{
            "todo": {
                "notifications": {
                    "focusReminders": false
                }
            },
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.todo.notifications.focus_reminders);
    }

    #[test]
    fn test_tools_permissions_config() {
        let json = r#"{
            "tools": {
                "permissions": {
                    "defaultLevel": "standard",
                    "channels": {
                        "cli": "admin",
                        "telegram": "readOnly"
                    }
                }
            },
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        let policy = config.tools.approval_policy;
        assert_eq!(
            policy.non_ui_channels,
            common::tool_channel::NonUiPolicy::Allow
        );
    }

    #[test]
    fn test_tools_no_permissions_defaults_to_none() {
        let config = Config::default();
        let policy = config.tools.approval_policy;
        assert_eq!(
            policy.non_ui_channels,
            common::tool_channel::NonUiPolicy::default()
        );
    }

    #[test]
    fn test_config_serde_roundtrips() {
        // DailyPlanningConfig roundtrip
        let daily = DailyPlanningConfig {
            enabled: false,
            planning_time: "09:30".to_string(),
        };
        let json = serde_json::to_string(&daily).unwrap();
        assert!(json.contains("\"planningTime\""));
        assert!(json.contains("\"09:30\""));
        let loaded: DailyPlanningConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.enabled);
        assert_eq!(loaded.planning_time, "09:30");
    }

    // ── FIRE config tests ────────────────────────────────────────────

    #[test]
    fn test_fire_config_default() {
        let config = FireConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.safe_withdrawal_rate, 4.0);
        assert_eq!(config.fire_type, "regular");
        assert!(config.current_age.is_none());
        assert!(config.target_retirement_age.is_none());
        assert!(config.annual_expenses.is_none());
        assert!(config.target_number.is_none());
        assert!(config.monthly_savings_rate.is_none());
        assert!(config.current_net_worth.is_none());
    }

    #[test]
    fn test_fire_config_serde_roundtrip() {
        let config = FireConfig {
            enabled: true,
            current_age: Some(30),
            target_retirement_age: Some(45),
            annual_expenses: Some(3_000_000),
            safe_withdrawal_rate: 3.5,
            fire_type: "lean".to_string(),
            target_number: Some(85_714_285),
            monthly_savings_rate: Some(500_000),
            current_net_worth: Some(10_000_000),
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: FireConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.current_age, Some(30));
        assert_eq!(loaded.target_retirement_age, Some(45));
        assert_eq!(loaded.annual_expenses, Some(3_000_000));
        assert_eq!(loaded.safe_withdrawal_rate, 3.5);
        assert_eq!(loaded.fire_type, "lean");
        assert_eq!(loaded.target_number, Some(85_714_285));
        assert_eq!(loaded.monthly_savings_rate, Some(500_000));
        assert_eq!(loaded.current_net_worth, Some(10_000_000));
    }

    #[test]
    fn test_fire_config_camel_case() {
        let config = FireConfig {
            enabled: true,
            current_age: Some(30),
            safe_withdrawal_rate: 4.0,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("currentAge"));
        assert!(json.contains("safeWithdrawalRate"));
        assert!(json.contains("fireType"));
    }

    #[test]
    fn test_fire_config_skip_serializing_none() {
        let config = FireConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        // None fields should be skipped
        assert!(!json.contains("currentAge"));
        assert!(!json.contains("targetRetirementAge"));
        assert!(!json.contains("annualExpenses"));
        assert!(!json.contains("targetNumber"));
    }

    #[test]
    fn test_setup_completed_default_false() {
        let config = Config::default();
        assert!(!config.setup_completed);
    }

    #[test]
    fn test_setup_completed_serde() {
        let json = r#"{"setupCompleted": true}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.setup_completed);
    }

    #[test]
    fn test_default_finance_categories_count() {
        let cats = default_finance_categories();
        assert!(cats.len() >= 25);
        assert!(cats.iter().any(|c| c.name == "Salary"));
        assert!(cats.iter().any(|c| c.name == "Housing"));
        assert!(cats.iter().any(|c| c.name == "Charity"));
        // Check group diversity
        assert!(cats.iter().any(|c| c.group == "income"));
        assert!(cats.iter().any(|c| c.group == "essential"));
        assert!(cats.iter().any(|c| c.group == "lifestyle"));
        assert!(cats.iter().any(|c| c.group == "savings"));
        assert!(cats.iter().any(|c| c.group == "giving"));
    }

    #[test]
    fn test_finance_config_includes_fire() {
        let config = Config::default();
        assert!(!config.finance.fire.enabled);
        assert_eq!(config.finance.fire.safe_withdrawal_rate, 4.0);
    }

    #[test]
    fn test_user_config_default() {
        let config = Config::default();
        assert_eq!(config.user.name, "");
    }

    #[test]
    fn test_user_config_serde_roundtrip() {
        let json = r#"{"user": {"name": "Vu"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.user.name, "Vu");

        let serialized = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(loaded.user.name, "Vu");
    }

    #[test]
    fn test_config_without_user_deserializes() {
        let json = r#"{"agents": {"defaults": {"workspace": "~/.klyntbot/workspace", "model": "anthropic/claude-opus-4-5", "maxTokens": 8192, "temperature": 0.7, "maxToolIterations": 20}}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.user.name, "");
    }

    #[test]
    fn test_fire_config_deserializes_from_empty() {
        // Existing config without FIRE section should deserialize fine
        let json = r#"{"finance": {}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.finance.fire.enabled);
        assert_eq!(config.finance.fire.fire_type, "regular");
    }

    #[test]
    fn test_shortcuts_config_default() {
        let config = Config::default();
        assert_eq!(config.shortcuts.launcher, "alt+space");
        assert_eq!(config.shortcuts.tray, "alt+shift+space");
    }

    #[test]
    fn test_shortcuts_config_serde_roundtrip() {
        let json = r#"{"shortcuts": {"launcher": "ctrl+space", "tray": "ctrl+shift+space"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.shortcuts.launcher, "ctrl+space");
        assert_eq!(config.shortcuts.tray, "ctrl+shift+space");

        let serialized = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(loaded.shortcuts.launcher, "ctrl+space");
    }

    #[test]
    fn test_config_without_shortcuts_deserializes() {
        let json = r#"{}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.shortcuts.launcher, "alt+space");
    }
}
