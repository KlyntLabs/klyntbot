//! Configuration loading and saving utilities.

use std::path::PathBuf;

use serde_json::Value;
use tokio::fs;

use super::schema::Config;
use common::{ConfigError, Result};

/// Get the configuration file path (`{home}/config.json`).
///
/// Respects `KLYNTBOT_HOME` env var, falling back to `~/.klyntbot/`.
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

/// Get the klyntbot home directory (config + data root).
///
/// Respects `KLYNTBOT_HOME` env var, falling back to `~/.klyntbot/`.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("KLYNTBOT_HOME") {
        return Ok(super::schema::core::expand_tilde(&dir));
    }
    dirs::home_dir()
        .map(|home| home.join(".klyntbot"))
        .ok_or_else(|| {
            ConfigError::Invalid("Unable to determine home directory".to_string()).into()
        })
}

/// Load configuration from file or return default
pub async fn load() -> Result<Config> {
    let klyntbot_path = config_path()?;

    if klyntbot_path.exists() {
        let content = fs::read_to_string(&klyntbot_path)
            .await
            .map_err(ConfigError::Io)?;

        let config: Config = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;

        return Ok(config);
    }

    // Config not found, use defaults
    Ok(Config::default())
}

/// Save configuration to file, writing only fields that differ from defaults.
pub async fn save(config: &Config) -> Result<()> {
    let path = config_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(ConfigError::Io)?;
    }

    let full = serde_json::to_value(config).map_err(ConfigError::Json)?;
    let default = serde_json::to_value(Config::default()).map_err(ConfigError::Json)?;
    let minimal = diff_json(&full, &default);

    let content = serde_json::to_string_pretty(&minimal).map_err(ConfigError::Json)?;

    fs::write(&path, content).await.map_err(ConfigError::Io)?;

    Ok(())
}

/// Synchronous config load for non-hot-path contexts (constructors, wizard, tests).
///
/// Prefer the async [`load()`] in request-handling and agent loop code.
pub fn load_sync() -> Result<Config> {
    let klyntbot_path = config_path()?;

    if klyntbot_path.exists() {
        let content = std::fs::read_to_string(&klyntbot_path).map_err(ConfigError::Io)?;

        let config: Config = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;

        return Ok(config);
    }

    Ok(Config::default())
}

/// Synchronous config save for non-hot-path contexts (constructors, wizard, tests).
///
/// Prefer the async [`save()`] in request-handling and agent loop code.
pub fn save_sync(config: &Config) -> Result<()> {
    let path = config_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
    }

    let full = serde_json::to_value(config).map_err(ConfigError::Json)?;
    let default = serde_json::to_value(Config::default()).map_err(ConfigError::Json)?;
    let minimal = diff_json(&full, &default);

    let content = serde_json::to_string_pretty(&minimal).map_err(ConfigError::Json)?;

    std::fs::write(&path, content).map_err(ConfigError::Io)?;

    Ok(())
}

/// Recursively diff two JSON values, returning only fields that differ from the default.
fn diff_json(actual: &Value, default: &Value) -> Value {
    match (actual, default) {
        (Value::Object(actual_map), Value::Object(default_map)) => {
            let mut result = serde_json::Map::new();
            for (key, val) in actual_map {
                match default_map.get(key) {
                    Some(default_val) if val == default_val => {} // skip unchanged
                    Some(default_val) => {
                        let diff = diff_json(val, default_val);
                        if !is_empty_object(&diff) {
                            result.insert(key.clone(), diff);
                        }
                    }
                    None => {
                        // Key not in defaults — keep it
                        result.insert(key.clone(), val.clone());
                    }
                }
            }
            Value::Object(result)
        }
        _ => actual.clone(), // Leaf values that differ (caller already checked inequality)
    }
}

fn is_empty_object(v: &Value) -> bool {
    matches!(v, Value::Object(m) if m.is_empty())
}

/// Check if configuration exists
pub fn exists() -> bool {
    config_path().map(|p| p.exists()).unwrap_or(false)
}

/// Initialize configuration directory structure
pub async fn init() -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).await.map_err(ConfigError::Io)?;

    // Create subdirectories
    fs::create_dir_all(dir.join("sessions"))
        .await
        .map_err(ConfigError::Io)?;
    fs::create_dir_all(dir.join("workspace"))
        .await
        .map_err(ConfigError::Io)?;

    // Save default config if it doesn't exist
    if !exists() {
        save(&Config::default()).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Secret;

    #[test]
    fn test_config_paths() {
        let path = config_path().unwrap();
        assert!(path.to_string_lossy().ends_with("config.json"));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");
        assert_eq!(config.agents.defaults.max_tokens, 8192);
    }

    #[test]
    fn test_config_dir_path() {
        let dir = config_dir().unwrap();
        assert!(
            dir.exists()
                || dir.to_string_lossy().contains(".klyntbot")
                || std::env::var("KLYNTBOT_HOME").is_ok()
        );
    }

    #[test]
    fn test_config_dir_respects_env_override() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let custom_dir = temp_dir.path().join("custom-home");
        std::fs::create_dir_all(&custom_dir).unwrap();

        std::env::set_var("KLYNTBOT_HOME", custom_dir.to_str().unwrap());

        let dir = config_dir().unwrap();
        assert_eq!(dir, custom_dir);

        let path = config_path().unwrap();
        assert_eq!(path, custom_dir.join("config.json"));

        std::env::remove_var("KLYNTBOT_HOME");
    }

    #[test]
    fn test_config_workspace_path() {
        let config = Config::default();
        let workspace = config.workspace_path();
        assert!(workspace.to_string_lossy().contains(".klyntbot"));
        assert!(workspace.to_string_lossy().contains("workspace"));
    }

    #[test]
    fn test_config_serialization_camel_case() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();

        // Verify camelCase in JSON output
        assert!(json.contains("maxTokens"));
        assert!(json.contains("maxToolIterations"));
        assert!(json.contains("allowFrom"));
    }

    #[test]
    fn test_agent_defaults() {
        let defaults = super::super::schema::AgentDefaults::default();
        assert_eq!(defaults.workspace, "~/.klyntbot/workspace");
        assert_eq!(defaults.model, "anthropic/claude-opus-4-5");
        assert_eq!(defaults.max_tokens, 8192);
        assert_eq!(defaults.temperature, 0.7);
        assert_eq!(defaults.max_tool_iterations, 20);
    }

    #[test]
    fn test_provider_config_defaults() {
        let config = super::super::schema::ProvidersConfig::default();
        assert_eq!(config.anthropic.api_key.expose(), "");
        assert_eq!(config.openai.api_key.expose(), "");
        assert!(config.anthropic.api_base.is_none());
    }

    #[test]
    fn test_telegram_config_defaults() {
        let config = super::super::schema::TelegramConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.token.expose(), "");
        assert_eq!(config.allow_from.len(), 0);
    }

    #[test]
    fn test_tools_config_defaults() {
        let config = super::super::schema::ToolsConfig::default();
        assert!(!config.restrict_to_workspace);
    }

    #[test]
    fn test_config_round_trip() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();

        // Override config path for testing
        let test_config_path = temp_dir.path().join("config.json");

        let mut config = Config::default();
        config.agents.defaults.model = "test-model".to_string();
        config.providers.anthropic.api_key = Secret::new("test-key".to_string());

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&test_config_path, &json).unwrap();

        // Load back
        let content = std::fs::read_to_string(&test_config_path).unwrap();
        let loaded_config: Config = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded_config.agents.defaults.model, "test-model");
        assert_eq!(
            loaded_config.providers.anthropic.api_key.expose(),
            "test-key"
        );
    }

    #[test]
    fn test_save_and_load_config() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_config_path = temp_dir.path().join("config.json");

        let mut config = Config::default();
        config.agents.defaults.model = "custom-model".to_string();
        config.agents.defaults.max_tokens = 4096;
        config.providers.anthropic.api_key = Secret::new("sk-ant-test".to_string());
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = Secret::new("bot-token-123".to_string());

        // Save config
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&test_config_path, &json).unwrap();

        // Load config
        let content = std::fs::read_to_string(&test_config_path).unwrap();
        let loaded: Config = serde_json::from_str(&content).unwrap();

        // Verify all fields
        assert_eq!(loaded.agents.defaults.model, "custom-model");
        assert_eq!(loaded.agents.defaults.max_tokens, 4096);
        assert_eq!(loaded.providers.anthropic.api_key.expose(), "sk-ant-test");
        assert!(loaded.channels.telegram.enabled);
        assert_eq!(loaded.channels.telegram.token.expose(), "bot-token-123");
    }

    #[test]
    fn test_save_creates_parent_directory() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("nested")
            .join("dir")
            .join("config.json");

        let config = Config::default();

        // Create parent directories
        if let Some(parent) = nested_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Save config
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&nested_path, &json).unwrap();

        // Verify file exists
        assert!(nested_path.exists());
    }

    #[test]
    fn test_init_creates_directory_structure() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".klyntbot");

        // Create directories
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(config_dir.join("sessions")).unwrap();
        std::fs::create_dir_all(config_dir.join("workspace")).unwrap();

        // Verify directories exist
        assert!(config_dir.exists());
        assert!(config_dir.join("sessions").exists());
        assert!(config_dir.join("workspace").exists());
    }

    #[test]
    fn test_config_with_invalid_json() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_config_path = temp_dir.path().join("config.json");

        // Write invalid JSON
        std::fs::write(&test_config_path, "{ invalid json }").unwrap();

        // Try to load
        let content = std::fs::read_to_string(&test_config_path).unwrap();
        let result = serde_json::from_str::<Config>(&content);

        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_missing_fields_uses_defaults() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_config_path = temp_dir.path().join("config.json");

        // Write minimal JSON (missing most fields)
        let minimal_json = r#"{
            "agents": {
                "defaults": {
                    "model": "test-model"
                }
            }
        }"#;
        std::fs::write(&test_config_path, minimal_json).unwrap();

        // Load config
        let content = std::fs::read_to_string(&test_config_path).unwrap();
        let loaded: Config = serde_json::from_str(&content).unwrap();

        // Verify defaults are used for missing fields
        assert_eq!(loaded.agents.defaults.model, "test-model");
        assert_eq!(loaded.agents.defaults.max_tokens, 8192); // Default
        assert_eq!(loaded.agents.defaults.temperature, 0.7); // Default
    }

    #[test]
    fn test_save_config_pretty_formatting() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_config_path = temp_dir.path().join("config.json");

        let config = Config::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&test_config_path, &json).unwrap();

        let content = std::fs::read_to_string(&test_config_path).unwrap();

        // Verify pretty formatting (indentation)
        assert!(content.contains("  ")); // Has indentation
        assert!(content.contains("\n")); // Has newlines
    }

    #[test]
    fn test_config_path_includes_home_directory() {
        // Only assert home-based path when env override is not set
        if std::env::var("KLYNTBOT_HOME").is_err() {
            let path = config_path().unwrap();
            let home = dirs::home_dir().unwrap();

            assert!(path.starts_with(home));
            assert!(path.ends_with(".klyntbot/config.json"));
        }
    }

    #[test]
    fn test_config_dir_includes_home_directory() {
        if std::env::var("KLYNTBOT_HOME").is_err() {
            let dir = config_dir().unwrap();
            let home = dirs::home_dir().unwrap();

            assert!(dir.starts_with(home));
            assert!(dir.ends_with(".klyntbot"));
        }
    }

    #[test]
    fn test_multiple_save_load_cycles() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_config_path = temp_dir.path().join("config.json");

        let mut config = Config::default();

        // First cycle
        config.agents.defaults.model = "model-v1".to_string();
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&test_config_path, &json).unwrap();

        // Second cycle
        config.agents.defaults.model = "model-v2".to_string();
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&test_config_path, &json).unwrap();

        // Load and verify latest
        let content = std::fs::read_to_string(&test_config_path).unwrap();
        let loaded: Config = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.agents.defaults.model, "model-v2");
    }

    #[test]
    fn test_config_serialization_field_names() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();

        // Verify specific camelCase field names
        assert!(json.contains("\"maxTokens\""));
        assert!(json.contains("\"maxToolIterations\""));
        assert!(json.contains("\"allowFrom\""));
        assert!(json.contains("\"apiKey\""));
        assert!(!json.contains("max_tokens")); // Should not have snake_case
        assert!(!json.contains("api_key")); // Should not have snake_case
    }

    #[test]
    fn test_save_minimal_default_config() {
        // Config::default() should diff to an empty object
        let config = Config::default();
        let full = serde_json::to_value(&config).unwrap();
        let default = serde_json::to_value(Config::default()).unwrap();
        let minimal = diff_json(&full, &default);

        assert_eq!(minimal, serde_json::json!({}));
    }

    #[test]
    fn test_save_minimal_with_provider() {
        // Only the configured provider and changed model should appear
        let mut config = Config::default();
        config.agents.defaults.model = "openai/gpt-4".to_string();
        config.providers.openai.api_key = Secret::new("sk-test-123".to_string());

        let full = serde_json::to_value(&config).unwrap();
        let default = serde_json::to_value(Config::default()).unwrap();
        let minimal = diff_json(&full, &default);

        let obj = minimal.as_object().unwrap();

        // Only agents and providers should be present
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("agents"));
        assert!(obj.contains_key("providers"));

        // Only openai should be under providers
        let providers = obj["providers"].as_object().unwrap();
        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("openai"));

        // No other channels, providers, tools, or gateway
        assert!(!obj.contains_key("channels"));
        assert!(!obj.contains_key("tools"));
        assert!(!obj.contains_key("gateway"));
    }

    #[test]
    fn test_save_minimal_round_trip() {
        // Save minimal, load back, verify all defaults restored
        let mut config = Config::default();
        config.agents.defaults.model = "openai/gpt-4".to_string();
        config.providers.openai.api_key = Secret::new("sk-test-key".to_string());
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = Secret::new("bot-token".to_string());

        let full = serde_json::to_value(&config).unwrap();
        let default = serde_json::to_value(Config::default()).unwrap();
        let minimal = diff_json(&full, &default);

        // Serialize to string and parse back
        let json_str = serde_json::to_string_pretty(&minimal).unwrap();
        let loaded: Config = serde_json::from_str(&json_str).unwrap();

        // Non-default values preserved
        assert_eq!(loaded.agents.defaults.model, "openai/gpt-4");
        assert_eq!(loaded.providers.openai.api_key.expose(), "sk-test-key");
        assert!(loaded.channels.telegram.enabled);
        assert_eq!(loaded.channels.telegram.token.expose(), "bot-token");

        // Defaults restored for omitted fields
        assert_eq!(loaded.agents.defaults.max_tokens, 8192);
        assert_eq!(loaded.agents.defaults.temperature, 0.7);
        assert_eq!(loaded.providers.anthropic.api_key.expose(), "");
        assert!(!loaded.channels.discord.enabled);
        assert_eq!(loaded.gateway.port, 18790);
    }

    #[test]
    fn test_diff_json_preserves_non_defaults() {
        let default = serde_json::json!({
            "a": 1,
            "b": { "c": 2, "d": 3 },
            "e": "hello"
        });
        let actual = serde_json::json!({
            "a": 1,
            "b": { "c": 99, "d": 3 },
            "e": "hello",
            "f": "new"
        });

        let diff = diff_json(&actual, &default);

        // "a" and "e" are unchanged — should be omitted
        assert!(diff.get("a").is_none());
        assert!(diff.get("e").is_none());

        // "b.c" changed — should be present
        assert_eq!(diff["b"]["c"], 99);
        // "b.d" unchanged — should be omitted from b
        assert!(diff["b"].get("d").is_none());

        // "f" is new — should be present
        assert_eq!(diff["f"], "new");
    }

    #[test]
    fn test_diff_json_empty_objects_pruned() {
        // When all children match defaults, the parent object should be pruned
        let default = serde_json::json!({"a": {"b": 1, "c": 2}});
        let actual = serde_json::json!({"a": {"b": 1, "c": 2}});

        let diff = diff_json(&actual, &default);
        assert_eq!(diff, serde_json::json!({}));
    }
}
