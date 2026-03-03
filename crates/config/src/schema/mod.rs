//! Configuration schema using serde.
//!
//! Split into submodules by config section:
//! - `core`: Secret, Config (root composition)
//! - `agents`: AgentsConfig, AgentDefaults
//! - `calendar`: CalendarConfig, provider configs
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
mod calendar;
mod channels;
mod confidence;
mod conversation;
mod core;
mod finance;
mod gateway;
mod learning;
mod mcp;
mod orchestrator;
mod packs;
mod plugins;
mod project;
mod providers;
mod todo;
mod tools;

pub use self::agents::*;
pub use self::calendar::*;
pub use self::channels::*;
pub use self::confidence::*;
pub use self::conversation::*;
pub use self::core::*;
pub use self::finance::*;
pub use self::gateway::*;
pub use self::learning::*;
pub use self::mcp::*;
pub use self::orchestrator::*;
pub use self::packs::*;
pub use self::plugins::*;
pub use self::project::*;
pub use self::providers::*;
pub use self::todo::*;
pub use self::tools::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");
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
        assert!(json.contains("bridgeUrl"));
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
        let perms = config.tools.permissions.unwrap();
        assert_eq!(perms.default_level, "standard");
        assert_eq!(perms.channels.get("cli").unwrap(), "admin");
        assert_eq!(perms.channels.get("telegram").unwrap(), "readOnly");
    }

    #[test]
    fn test_tools_no_permissions_defaults_to_none() {
        let config = Config::default();
        assert!(config.tools.permissions.is_none());
    }

    #[test]
    fn test_config_serde_roundtrips() {
        // CalendarConfig serialization
        let cal_config = CalendarConfig {
            providers: vec![
                CalendarProviderConfig::Apple(AppleCalendarConfig {
                    enabled: true,
                    username: "user@apple.com".to_string(),
                    password: Secret::new("pass".to_string()),
                    ..AppleCalendarConfig::default()
                }),
                CalendarProviderConfig::Google(GoogleCalendarConfig {
                    enabled: true,
                    client_id: "id123".to_string(),
                    client_secret: Secret::new("secret".to_string()),
                    access_token: Secret::new("tok".to_string()),
                    refresh_token: Secret::new("ref".to_string()),
                    ..GoogleCalendarConfig::default()
                }),
            ],
            conflict_resolution: "server_wins".to_string(),
            ..CalendarConfig::default()
        };

        let json = serde_json::to_string(&cal_config).unwrap();
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"type\":\"apple\""));
        assert!(json.contains("\"user@apple.com\""));

        // CalendarConfig roundtrip
        let deserialized: CalendarConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cal_config.providers.len(), deserialized.providers.len());
        assert_eq!(
            cal_config.conflict_resolution,
            deserialized.conflict_resolution
        );

        // Config-level calendar serialization
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: true,
                username: "test@example.com".to_string(),
                password: Secret::new("password123".to_string()),
                ..AppleCalendarConfig::default()
            }));
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"calendar\""));
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"test@example.com\""));

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
}
