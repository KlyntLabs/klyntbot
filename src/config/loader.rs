//! Configuration loading and saving utilities.

use std::fs;
use std::path::PathBuf;

use super::schema::{Config, Secret};
use crate::error::{ConfigError, Result};

/// Get the default configuration file path (~/.klyntbot/config.json)
pub fn config_path() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".klyntbot").join("config.json"))
        .ok_or_else(|| ConfigError::Invalid("Unable to determine home directory".to_string()).into())
}

/// Get the klyntbot data directory (~/.klyntbot/)
pub fn config_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".klyntbot"))
        .ok_or_else(|| ConfigError::Invalid("Unable to determine home directory".to_string()).into())
}

/// Load configuration from file or return default
pub fn load() -> Result<Config> {
    let klyntbot_path = config_path()?;

    if klyntbot_path.exists() {
        let content = fs::read_to_string(&klyntbot_path).map_err(ConfigError::Io)?;

        let config: Config = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;

        return Ok(config);
    }

    // Fallback: try nanobot config
    let nanobot_path = dirs::home_dir()
        .ok_or_else(|| ConfigError::Invalid("Unable to determine home directory".to_string()))?
        .join(".nanobot")
        .join("config.json");

    if nanobot_path.exists() {
        eprintln!("Migrating config from ~/.nanobot/config.json to ~/.klyntbot/config.json");
        let content = fs::read_to_string(&nanobot_path).map_err(ConfigError::Io)?;

        // Parse and migrate
        let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            eprintln!("Failed to parse nanobot config, using defaults: {}", e);
            ConfigError::Invalid(format!("Nanobot config parse error: {}", e))
        })?;

        // Migrate: tools.exec.restrictToWorkspace -> tools.restrictToWorkspace
        migrate_config(&mut data);

        let config: Config = serde_json::from_value(data)
            .map_err(|e| ConfigError::Invalid(format!("Failed to migrate config: {}", e)))?;

        // Save migrated config to klyntbot path
        if let Err(e) = save(&config) {
            eprintln!("Failed to save migrated config: {}", e);
        }

        return Ok(config);
    }

    // Neither exists
    Ok(Config::default())
}

fn migrate_config(data: &mut serde_json::Value) {
    if let Some(tools) = data.get_mut("tools").and_then(|v| v.as_object_mut()) {
        if let Some(exec) = tools.get_mut("exec").and_then(|v| v.as_object_mut()) {
            if let Some(rtw) = exec.remove("restrictToWorkspace") {
                if !tools.contains_key("restrictToWorkspace") {
                    tools.insert("restrictToWorkspace".to_string(), rtw);
                }
            }
        }
    }
}

/// Save configuration to file
pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::Io)?;
    }

    let content = serde_json::to_string_pretty(config).map_err(ConfigError::Json)?;

    fs::write(&path, content).map_err(ConfigError::Io)?;

    Ok(())
}

/// Load configuration from environment variables
/// Environment variables are prefixed with KLYNTBOT_ and use double underscores for nesting
/// Example: KLYNTBOT_AGENTS__DEFAULTS__MODEL=claude-sonnet-4-5
pub fn load_with_env_overrides() -> Result<Config> {
    let mut config = load()?;

    // Agent defaults
    if let Ok(model) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__MODEL") {
        config.agents.defaults.model = model;
    }

    if let Ok(workspace) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE") {
        config.agents.defaults.workspace = workspace;
    }

    if let Ok(val) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE") {
        if let Ok(temp) = val.parse::<f32>() {
            config.agents.defaults.temperature = temp;
        }
    }

    if let Ok(val) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS") {
        if let Ok(tokens) = val.parse::<u32>() {
            config.agents.defaults.max_tokens = tokens;
        }
    }

    // Provider API keys
    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY") {
        config.providers.anthropic.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__OPENAI__API_KEY") {
        config.providers.openai.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY") {
        config.providers.openrouter.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY") {
        config.providers.deepseek.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__GEMINI__API_KEY") {
        config.providers.gemini.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__GROQ__API_KEY") {
        config.providers.groq.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__VLLM__API_KEY") {
        config.providers.vllm.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__ZHIPU__API_KEY") {
        config.providers.zhipu.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY") {
        config.providers.dashscope.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY") {
        config.providers.moonshot.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__MINIMAX__API_KEY") {
        config.providers.minimax.api_key = Secret::new(key);
    }

    if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY") {
        config.providers.aihubmix.api_key = Secret::new(key);
    }

    // Channel tokens
    if let Ok(token) = std::env::var("KLYNTBOT_CHANNELS__TELEGRAM__TOKEN") {
        config.channels.telegram.token = Secret::new(token);
    }

    if let Ok(token) = std::env::var("KLYNTBOT_CHANNELS__DISCORD__TOKEN") {
        config.channels.discord.token = Secret::new(token);
    }

    if let Ok(token) = std::env::var("KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN") {
        config.channels.slack.bot_token = Secret::new(token);
    }

    if let Ok(token) = std::env::var("KLYNTBOT_CHANNELS__SLACK__APP_TOKEN") {
        config.channels.slack.app_token = Secret::new(token);
    }

    // Tools
    if let Ok(key) = std::env::var("KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY") {
        config.tools.web.brave_api_key = Secret::new(key);
    }

    Ok(config)
}

/// Check if configuration exists
pub fn exists() -> bool {
    config_path().map(|p| p.exists()).unwrap_or(false)
}

/// Initialize configuration directory structure
pub fn init() -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(ConfigError::Io)?;

    // Create subdirectories
    fs::create_dir_all(dir.join("sessions")).map_err(ConfigError::Io)?;
    fs::create_dir_all(dir.join("workspace")).map_err(ConfigError::Io)?;

    // Save default config if it doesn't exist
    if !exists() {
        save(&Config::default())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths() {
        let path = config_path().unwrap();
        assert!(path.to_string_lossy().contains(".klyntbot"));
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
        assert!(dir.to_string_lossy().contains(".klyntbot"));
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
        assert_eq!(config.exec.timeout, 60);
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
        assert_eq!(loaded_config.providers.anthropic.api_key.expose(), "test-key");
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
    fn test_load_with_env_overrides_model() {
        std::env::set_var("KLYNTBOT_AGENTS__DEFAULTS__MODEL", "env-model");

        let config = Config::default();
        let mut overridden = config.clone();

        if let Ok(model) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__MODEL") {
            overridden.agents.defaults.model = model;
        }

        assert_eq!(overridden.agents.defaults.model, "env-model");

        std::env::remove_var("KLYNTBOT_AGENTS__DEFAULTS__MODEL");
    }

    #[test]
    fn test_load_with_env_overrides_workspace() {
        std::env::set_var("KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE", "/custom/workspace");

        let config = Config::default();
        let mut overridden = config.clone();

        if let Ok(workspace) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE") {
            overridden.agents.defaults.workspace = workspace;
        }

        assert_eq!(overridden.agents.defaults.workspace, "/custom/workspace");

        std::env::remove_var("KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE");
    }

    #[test]
    fn test_load_with_env_overrides_api_keys() {
        std::env::set_var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY", "sk-ant-env");
        std::env::set_var("KLYNTBOT_PROVIDERS__OPENAI__API_KEY", "sk-openai-env");
        std::env::set_var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY", "sk-deepseek-env");

        let config = Config::default();
        let mut overridden = config.clone();

        if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY") {
            overridden.providers.anthropic.api_key = Secret::new(key);
        }
        if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__OPENAI__API_KEY") {
            overridden.providers.openai.api_key = Secret::new(key);
        }
        if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY") {
            overridden.providers.deepseek.api_key = Secret::new(key);
        }

        assert_eq!(overridden.providers.anthropic.api_key.expose(), "sk-ant-env");
        assert_eq!(overridden.providers.openai.api_key.expose(), "sk-openai-env");
        assert_eq!(overridden.providers.deepseek.api_key.expose(), "sk-deepseek-env");

        std::env::remove_var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY");
        std::env::remove_var("KLYNTBOT_PROVIDERS__OPENAI__API_KEY");
        std::env::remove_var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY");
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
        let path = config_path().unwrap();
        let home = dirs::home_dir().unwrap();

        assert!(path.starts_with(home));
        assert!(path.ends_with(".klyntbot/config.json"));
    }

    #[test]
    fn test_config_dir_includes_home_directory() {
        let dir = config_dir().unwrap();
        let home = dirs::home_dir().unwrap();

        assert!(dir.starts_with(home));
        assert!(dir.ends_with(".klyntbot"));
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
}
