//! Auto-detection of existing configuration from config file, env vars, and system probes.

use std::fmt;

/// How a value was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectSource {
    /// From existing ~/.klyntbot/config.json
    Config,
    /// From KLYNTBOT_* environment variable
    EnvVar,
    /// From system probe (e.g., pg_isready)
    Detected,
}

impl fmt::Display for DetectSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config => write!(f, "from config"),
            Self::EnvVar => write!(f, "from env"),
            Self::Detected => write!(f, "detected"),
        }
    }
}

/// Pre-detected values for the core setup screen.
#[derive(Debug, Default)]
pub struct DetectedState {
    /// (provider_name, source)
    pub provider: Option<(String, DetectSource)>,
    /// (api_key, source) — raw key value
    pub api_key: Option<(String, DetectSource)>,
    /// (model, source)
    pub model: Option<(String, DetectSource)>,
    /// (database_url, source)
    pub database_url: Option<(String, DetectSource)>,
    /// Whether postgres is running (from pg_isready probe)
    pub pg_running: bool,
    /// (channel_key, source) — e.g., "telegram"
    pub channel: Option<(String, DetectSource)>,
    /// (token, source)
    pub channel_token: Option<(String, DetectSource)>,
    /// (calendar_provider_key, source) — e.g., "apple"
    pub calendar: Option<(String, DetectSource)>,
}

impl DetectedState {
    /// Build detected state from an existing config.
    pub fn from_config(config: &config::Config) -> Self {
        let mut state = Self::default();

        // Detect provider from config
        let active = config.active_provider_name();
        if active != "none" {
            state.provider = Some((active.to_string(), DetectSource::Config));

            // Get the API key for the active provider
            for (name, pc) in [
                ("anthropic", &config.providers.anthropic),
                ("openai", &config.providers.openai),
                ("openrouter", &config.providers.openrouter),
                ("deepseek", &config.providers.deepseek),
                ("gemini", &config.providers.gemini),
                ("groq", &config.providers.groq),
                ("vllm", &config.providers.vllm),
                ("zhipu", &config.providers.zhipu),
                ("dashscope", &config.providers.dashscope),
                ("moonshot", &config.providers.moonshot),
                ("minimax", &config.providers.minimax),
                ("aihubmix", &config.providers.aihubmix),
            ] {
                if name == active && !pc.api_key.is_empty() {
                    state.api_key = Some((pc.api_key.expose().clone(), DetectSource::Config));
                    break;
                }
            }
        }

        // Detect model (only if non-default)
        if config.agents.defaults.model != "anthropic/claude-opus-4-5" {
            state.model = Some((config.agents.defaults.model.clone(), DetectSource::Config));
        }

        // Detect database URL
        if let Some(ref url) = config.database_url {
            state.database_url = Some((url.clone(), DetectSource::Config));
        }

        // Detect first enabled channel
        let channels: &[(&str, bool, &str)] = &[
            (
                "telegram",
                config.channels.telegram.enabled,
                config.channels.telegram.token.expose(),
            ),
            (
                "discord",
                config.channels.discord.enabled,
                config.channels.discord.token.expose(),
            ),
            (
                "slack",
                config.channels.slack.enabled,
                config.channels.slack.bot_token.expose(),
            ),
        ];
        for (name, enabled, token) in channels {
            if *enabled && !token.is_empty() {
                state.channel = Some((name.to_string(), DetectSource::Config));
                state.channel_token = Some((token.to_string(), DetectSource::Config));
                break;
            }
        }

        // Detect first enabled calendar provider
        if config.calendar.is_any_enabled() {
            if let Some(provider) = config.calendar.enabled_providers().first() {
                let key = match provider {
                    config::CalendarProviderConfig::Apple(_) => "apple",
                    config::CalendarProviderConfig::Google(_) => "google",
                    config::CalendarProviderConfig::GenericCalDav(_) => "caldav",
                };
                state.calendar = Some((key.to_string(), DetectSource::Config));
            }
        }

        state
    }

    /// Overlay environment variable detections (higher priority than config).
    pub fn overlay_env_vars(&mut self) {
        // Provider API keys from env
        let env_providers = [
            ("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY", "anthropic"),
            ("KLYNTBOT_PROVIDERS__OPENAI__API_KEY", "openai"),
            ("KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY", "openrouter"),
            ("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY", "deepseek"),
            ("KLYNTBOT_PROVIDERS__GEMINI__API_KEY", "gemini"),
            ("KLYNTBOT_PROVIDERS__GROQ__API_KEY", "groq"),
        ];
        for (var, name) in env_providers {
            if let Ok(key) = std::env::var(var) {
                if !key.is_empty() {
                    self.provider = Some((name.to_string(), DetectSource::EnvVar));
                    self.api_key = Some((key, DetectSource::EnvVar));
                    break; // Use the first found
                }
            }
        }

        // Database URL from env
        if let Ok(url) = std::env::var("KLYNTBOT_DATABASE_URL") {
            if !url.is_empty() {
                self.database_url = Some((url, DetectSource::EnvVar));
            }
        }

        // Channel tokens from env
        let env_channels = [
            ("KLYNTBOT_CHANNELS__TELEGRAM__TOKEN", "telegram"),
            ("KLYNTBOT_CHANNELS__DISCORD__TOKEN", "discord"),
            ("KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN", "slack"),
        ];
        for (var, name) in env_channels {
            if let Ok(token) = std::env::var(var) {
                if !token.is_empty() {
                    self.channel = Some((name.to_string(), DetectSource::EnvVar));
                    self.channel_token = Some((token, DetectSource::EnvVar));
                    break;
                }
            }
        }
    }

    /// Probe system for running PostgreSQL (fills pg_running and default DB URL).
    pub fn probe_postgres(&mut self) {
        if self.database_url.is_some() {
            return; // Already have a URL, skip probe
        }

        // Check if pg_isready succeeds
        let pg_running = std::process::Command::new("pg_isready")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        self.pg_running = pg_running;
        if pg_running {
            self.database_url = Some((
                "postgresql://localhost/klyntbot".to_string(),
                DetectSource::Detected,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_state_default_has_no_sources() {
        let state = DetectedState::default();
        assert!(state.provider.is_none());
        assert!(state.api_key.is_none());
        assert!(state.database_url.is_none());
        assert!(state.channel.is_none());
    }

    #[test]
    fn test_detect_from_config_with_provider() {
        let mut config = config::Config::default();
        config.providers.anthropic.api_key = config::Secret::new("sk-ant-test".to_string());
        config.agents.defaults.provider = Some("anthropic".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.provider,
            Some(("anthropic".to_string(), DetectSource::Config))
        );
        assert_eq!(
            state.api_key.as_ref().map(|(k, _)| k.as_str()),
            Some("sk-ant-test")
        );
    }

    #[test]
    fn test_detect_from_config_with_channel() {
        let mut config = config::Config::default();
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = config::Secret::new("bot123".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.channel,
            Some(("telegram".to_string(), DetectSource::Config))
        );
        assert_eq!(
            state.channel_token.as_ref().map(|(t, _)| t.as_str()),
            Some("bot123")
        );
    }

    #[test]
    fn test_detect_from_config_with_database() {
        let config = config::Config {
            database_url: Some("postgres://custom/db".to_string()),
            ..Default::default()
        };

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.database_url,
            Some(("postgres://custom/db".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_detect_source_display() {
        assert_eq!(format!("{}", DetectSource::Config), "from config");
        assert_eq!(format!("{}", DetectSource::EnvVar), "from env");
        assert_eq!(format!("{}", DetectSource::Detected), "detected");
    }

    #[test]
    fn test_detect_from_config_no_provider() {
        let config = config::Config::default();
        let state = DetectedState::from_config(&config);
        assert!(state.provider.is_none());
        assert!(state.api_key.is_none());
    }

    #[test]
    fn test_detect_from_config_no_channel() {
        let config = config::Config::default();
        let state = DetectedState::from_config(&config);
        assert!(state.channel.is_none());
        assert!(state.channel_token.is_none());
    }

    #[test]
    fn test_detect_from_config_disabled_channel_ignored() {
        let mut config = config::Config::default();
        // Token set but not enabled — should not be detected
        config.channels.telegram.token = config::Secret::new("bot123".to_string());
        config.channels.telegram.enabled = false;

        let state = DetectedState::from_config(&config);
        assert!(state.channel.is_none());
    }

    #[test]
    fn test_detect_from_config_enabled_but_empty_token_ignored() {
        let mut config = config::Config::default();
        config.channels.telegram.enabled = true;
        // Token is empty — should not be detected

        let state = DetectedState::from_config(&config);
        assert!(state.channel.is_none());
    }

    #[test]
    fn test_detect_from_config_auto_detect_provider() {
        let mut config = config::Config::default();
        // No explicit provider set, but openai has a key
        config.providers.openai.api_key = config::Secret::new("sk-openai-test".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.provider,
            Some(("openai".to_string(), DetectSource::Config))
        );
        assert_eq!(
            state.api_key.as_ref().map(|(k, _)| k.as_str()),
            Some("sk-openai-test")
        );
    }

    #[test]
    fn test_detect_from_config_model_default_not_detected() {
        let config = config::Config::default();
        let state = DetectedState::from_config(&config);
        assert!(state.model.is_none());
    }

    #[test]
    fn test_detect_from_config_custom_model() {
        let mut config = config::Config::default();
        config.agents.defaults.model = "gpt-4o".to_string();

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.model,
            Some(("gpt-4o".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_detect_from_config_discord_channel() {
        let mut config = config::Config::default();
        config.channels.discord.enabled = true;
        config.channels.discord.token = config::Secret::new("discord-tok".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.channel,
            Some(("discord".to_string(), DetectSource::Config))
        );
        assert_eq!(
            state.channel_token.as_ref().map(|(t, _)| t.as_str()),
            Some("discord-tok")
        );
    }

    #[test]
    fn test_detect_from_config_slack_channel() {
        let mut config = config::Config::default();
        config.channels.slack.enabled = true;
        config.channels.slack.bot_token = config::Secret::new("xoxb-slack-tok".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.channel,
            Some(("slack".to_string(), DetectSource::Config))
        );
        assert_eq!(
            state.channel_token.as_ref().map(|(t, _)| t.as_str()),
            Some("xoxb-slack-tok")
        );
    }

    #[test]
    fn test_detect_from_config_first_channel_wins() {
        let mut config = config::Config::default();
        // Both telegram and discord are enabled
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = config::Secret::new("tg-tok".to_string());
        config.channels.discord.enabled = true;
        config.channels.discord.token = config::Secret::new("dc-tok".to_string());

        let state = DetectedState::from_config(&config);
        // Telegram comes first in iteration
        assert_eq!(
            state.channel,
            Some(("telegram".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_probe_postgres_skips_when_url_set() {
        let mut state = DetectedState {
            database_url: Some(("postgres://custom/db".to_string(), DetectSource::Config)),
            ..Default::default()
        };

        state.probe_postgres();
        // URL should remain unchanged
        assert_eq!(
            state.database_url,
            Some(("postgres://custom/db".to_string(), DetectSource::Config))
        );
        // pg_running stays at default false since probe was skipped
        assert!(!state.pg_running);
    }

    #[test]
    fn test_default_pg_running_is_false() {
        let state = DetectedState::default();
        assert!(!state.pg_running);
    }

    #[test]
    fn test_detect_from_config_no_calendar() {
        let config = config::Config::default();
        let state = DetectedState::from_config(&config);
        assert!(state.calendar.is_none());
    }

    #[test]
    fn test_detect_from_config_apple_calendar() {
        let mut config = config::Config::default();
        config.calendar.providers.push(
            config::CalendarProviderConfig::Apple(config::AppleCalendarConfig {
                enabled: true,
                username: "user@icloud.com".to_string(),
                password: config::Secret::new("pass".to_string()),
                ..config::AppleCalendarConfig::default()
            }),
        );

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.calendar,
            Some(("apple".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_detect_from_config_google_calendar() {
        let mut config = config::Config::default();
        config.calendar.providers.push(
            config::CalendarProviderConfig::Google(config::GoogleCalendarConfig {
                enabled: true,
                ..config::GoogleCalendarConfig::default()
            }),
        );

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.calendar,
            Some(("google".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_detect_from_config_disabled_calendar_ignored() {
        let mut config = config::Config::default();
        config.calendar.providers.push(
            config::CalendarProviderConfig::Apple(config::AppleCalendarConfig {
                enabled: false,
                ..config::AppleCalendarConfig::default()
            }),
        );

        let state = DetectedState::from_config(&config);
        assert!(state.calendar.is_none());
    }

    #[test]
    fn test_detect_from_config_generic_caldav() {
        let mut config = config::Config::default();
        config.calendar.providers.push(
            config::CalendarProviderConfig::GenericCalDav(config::GenericCalDavConfig {
                enabled: true,
                name: "Nextcloud".to_string(),
                caldav_url: "https://nc.example.com/dav".to_string(),
                username: "user".to_string(),
                password: config::Secret::new("pass".to_string()),
                calendar_name: "Tasks".to_string(),
                sync_interval_secs: 300,
                auto_sync_due_dates: true,
            }),
        );

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.calendar,
            Some(("caldav".to_string(), DetectSource::Config))
        );
    }
}
