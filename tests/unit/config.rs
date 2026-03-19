//! Unit tests for configuration schema, backward compatibility,
//! serialization round-trips, and default enforcement.

use klyntbot::config::Config;

// -----------------------------------------------------------------
// Backward compatibility
// -----------------------------------------------------------------

#[test]
fn backward_compat_minimal_config_deserializes() {
    // Simulate a minimal config.json with only basic fields
    let old_config_json = r#"{
        "workspacePath": "~/.klyntbot/workspace",
        "agents": {
            "defaults": {
                "model": "anthropic/claude-opus-4-5",
                "maxTokens": 4096,
                "temperature": 1.0,
                "maxToolIterations": 20
            }
        },
        "providers": {
            "anthropic": {
                "apiKey": "sk-ant-test"
            },
            "openai": {
                "apiKey": ""
            }
        },
        "channels": {
            "telegram": {
                "enabled": false,
                "token": "",
                "allowFrom": []
            }
        },
        "tools": {
            "restrictToWorkspace": false,
            "exec": {
                "timeout": 60,
                "allowedCommands": []
            },
            "web": {
                "braveApiKey": ""
            }
        }
    }"#;

    // Should deserialize successfully with new fields getting defaults
    let config: Config = serde_json::from_str(old_config_json).unwrap();

    // Verify original fields preserved
    assert_eq!(
        config.agents.defaults.model,
        klyntbot::config::schema::DEFAULT_MODEL
    );
    assert_eq!(config.agents.defaults.max_tokens, 4096);
    assert_eq!(config.providers.anthropic.api_key.expose(), "sk-ant-test");

    // Verify new fields got defaults
    assert!(config.providers.anthropic.extra_headers.is_none());
    assert!(config.providers.zhipu.api_key.is_empty());
    assert!(config.providers.dashscope.api_key.is_empty());
    assert_eq!(config.gateway.host, "127.0.0.1");
    assert_eq!(config.gateway.port, 18790);
}

// -----------------------------------------------------------------
// Default round-trip
// -----------------------------------------------------------------

#[test]
fn config_default_round_trips() {
    let config = Config::default();
    assert_eq!(
        config.agents.defaults.model,
        klyntbot::config::schema::DEFAULT_MODEL
    );

    // Workspace path resolution
    let workspace = config.workspace_path();
    assert!(workspace.is_absolute() || workspace.starts_with("~"));

    // Serialization round-trip
    let json = serde_json::to_string(&config).unwrap();
    let loaded: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.agents.defaults.model, config.agents.defaults.model);
}

// -----------------------------------------------------------------
// Email consent enforcement
// -----------------------------------------------------------------

#[test]
fn email_consent_defaults_to_false() {
    use klyntbot::config::schema::EmailConfig;

    // Default should be false
    let config = EmailConfig::default();
    assert!(
        !config.consent_granted,
        "consent_granted must default to false for safety"
    );

    // Explicit false
    let json = r#"{
        "enabled": true,
        "consentGranted": false,
        "imapHost": "imap.test.com",
        "smtpHost": "smtp.test.com",
        "fromAddress": "test@test.com"
    }"#;

    let loaded: EmailConfig = serde_json::from_str(json).unwrap();
    assert!(
        !loaded.consent_granted,
        "consent_granted should be false when explicitly set"
    );
    assert!(loaded.enabled, "enabled should be true");

    // Explicit true
    let json_true = r#"{
        "enabled": true,
        "consentGranted": true,
        "imapHost": "imap.test.com",
        "smtpHost": "smtp.test.com",
        "fromAddress": "test@test.com"
    }"#;

    let loaded_true: EmailConfig = serde_json::from_str(json_true).unwrap();
    assert!(
        loaded_true.consent_granted,
        "consent_granted should be true when explicitly set"
    );

    // Missing consentGranted defaults to false
    let json_missing = r#"{
        "enabled": true,
        "imapHost": "imap.test.com",
        "smtpHost": "smtp.test.com",
        "fromAddress": "test@test.com"
    }"#;

    let loaded_missing: EmailConfig = serde_json::from_str(json_missing).unwrap();
    assert!(
        !loaded_missing.consent_granted,
        "consent_granted must default to false when missing from config"
    );
}

// -----------------------------------------------------------------
// Session history truncation
// -----------------------------------------------------------------

#[test]
fn session_history_returns_last_n() {
    use klyntbot::session::Session;

    let mut session = Session::new("test:chat");

    // Add 100 messages
    for i in 0..100 {
        session.add_message("user", format!("Message {}", i));
    }

    // Get last 10 messages
    let history = session.get_history(10);
    assert_eq!(history.len(), 10);
    assert_eq!(history[0].content, "Message 90");
    assert_eq!(history[9].content, "Message 99");
}
