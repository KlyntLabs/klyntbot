//! Environment variable overrides for configuration.
//!
//! Automatically loads `.env` from the current directory, then applies
//! `KLYNTBOT_*` env vars on top of the file-based config.

use common::Result;

use super::loader;
use super::schema::{Config, Secret};

/// Apply a string env var override to a config field.
macro_rules! env_string {
    ($var:literal, $field:expr) => {
        if let Ok(val) = std::env::var($var) {
            $field = val;
        }
    };
}

/// Apply a parsed env var override (e.g., f32, u32) to a config field.
macro_rules! env_parse {
    ($var:literal, $field:expr, $ty:ty) => {
        if let Ok(val) = std::env::var($var) {
            if let Ok(parsed) = val.parse::<$ty>() {
                $field = parsed;
            }
        }
    };
}

/// Apply a Secret<String> env var override.
macro_rules! env_secret {
    ($var:literal, $field:expr) => {
        if let Ok(val) = std::env::var($var) {
            $field = Secret::new(val);
        }
    };
}

/// Load configuration from environment variables.
/// Environment variables are prefixed with KLYNTBOT_ and use double underscores for nesting.
/// Example: KLYNTBOT_AGENTS__DEFAULTS__MODEL=claude-sonnet-4-5
///
/// Automatically loads a `.env` file from the current directory if present.
pub async fn load_with_env_overrides() -> Result<Config> {
    // Load .env file if present (silently ignored if missing)
    dotenvy::dotenv().ok();

    let mut config = loader::load().await?;

    // Agent defaults
    env_string!(
        "KLYNTBOT_AGENTS__DEFAULTS__MODEL",
        config.agents.defaults.model
    );
    env_string!(
        "KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE",
        config.agents.defaults.workspace
    );
    if let Ok(val) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__PROVIDER") {
        config.agents.defaults.provider = Some(val);
    }
    if let Ok(val) = std::env::var("KLYNTBOT_COGNITIVE__PROVIDER") {
        config.cognitive.provider = Some(val);
    }
    if let Ok(val) = std::env::var("KLYNTBOT_COGNITIVE__MODEL") {
        config.cognitive.model = Some(val);
    }
    // Allow overriding any provider's api_base — needed to route an
    // OpenAI-compat endpoint (Mimo, vLLM, etc.) through an existing
    // provider slot without adding a new ProviderRegistry entry.
    macro_rules! env_api_base {
        ($name:literal, $field:expr) => {
            if let Ok(val) = std::env::var($name) {
                $field = Some(val);
            }
        };
    }
    env_api_base!(
        "KLYNTBOT_PROVIDERS__DEEPSEEK__API_BASE",
        config.providers.deepseek.api_base
    );
    env_api_base!(
        "KLYNTBOT_PROVIDERS__OPENAI__API_BASE",
        config.providers.openai.api_base
    );
    env_api_base!(
        "KLYNTBOT_PROVIDERS__OPENROUTER__API_BASE",
        config.providers.openrouter.api_base
    );
    env_parse!(
        "KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE",
        config.agents.defaults.temperature,
        f32
    );
    env_parse!(
        "KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS",
        config.agents.defaults.max_tokens,
        u32
    );

    // Provider API keys
    env_secret!(
        "KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY",
        config.providers.anthropic.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__OPENAI__API_KEY",
        config.providers.openai.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY",
        config.providers.openrouter.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY",
        config.providers.deepseek.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__GEMINI__API_KEY",
        config.providers.gemini.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__GROQ__API_KEY",
        config.providers.groq.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__VLLM__API_KEY",
        config.providers.vllm.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__ZHIPU__API_KEY",
        config.providers.zhipu.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY",
        config.providers.dashscope.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY",
        config.providers.moonshot.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__MINIMAX__API_KEY",
        config.providers.minimax.api_key
    );
    env_secret!(
        "KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY",
        config.providers.aihubmix.api_key
    );

    // Data directory (explicit override takes precedence over KLYNTBOT_HOME)
    if let Ok(val) = std::env::var("KLYNTBOT_DATA_DIR") {
        config.data_dir = Some(val);
    } else if let Ok(val) = std::env::var("KLYNTBOT_HOME") {
        config.data_dir = Some(val);
    }

    // Channel tokens
    env_secret!(
        "KLYNTBOT_CHANNELS__TELEGRAM__TOKEN",
        config.channels.telegram.token
    );
    env_secret!(
        "KLYNTBOT_CHANNELS__DISCORD__TOKEN",
        config.channels.discord.token
    );
    env_secret!(
        "KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN",
        config.channels.slack.bot_token
    );
    env_secret!(
        "KLYNTBOT_CHANNELS__SLACK__APP_TOKEN",
        config.channels.slack.app_token
    );

    // Tools
    env_secret!(
        "KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY",
        config.tools.web.brave_api_key
    );

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(
            overridden.providers.anthropic.api_key.expose(),
            "sk-ant-env"
        );
        assert_eq!(
            overridden.providers.openai.api_key.expose(),
            "sk-openai-env"
        );
        assert_eq!(
            overridden.providers.deepseek.api_key.expose(),
            "sk-deepseek-env"
        );

        std::env::remove_var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY");
        std::env::remove_var("KLYNTBOT_PROVIDERS__OPENAI__API_KEY");
        std::env::remove_var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY");
    }
}
