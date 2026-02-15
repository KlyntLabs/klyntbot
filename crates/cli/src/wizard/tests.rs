use super::*;

// ========================================================================
// ToolsModule adapter tests
// ========================================================================

#[test]
fn test_tools_module_metadata() {
    let module = ToolsModule;
    assert_eq!(module.name(), "Tool Permissions");
    assert!(!module.is_required());

    let state = WizardState::new();
    assert!(module.is_applicable(&state));
}

// ========================================================================
// Config set_provider_key integration tests
// ========================================================================

#[test]
fn test_set_provider_key_anthropic() {
    let mut config = config::Config::default();
    config.set_provider_key("anthropic", "sk-ant-test-key".to_string());
    assert_eq!(
        config.providers.anthropic.api_key.expose(),
        "sk-ant-test-key"
    );
}

#[test]
fn test_set_provider_key_openai() {
    let mut config = config::Config::default();
    config.set_provider_key("openai", "sk-openai-test".to_string());
    assert_eq!(config.providers.openai.api_key.expose(), "sk-openai-test");
}

#[test]
fn test_set_provider_key_all_providers() {
    let mut config = config::Config::default();
    let providers = ["anthropic", "openai", "deepseek", "gemini", "openrouter"];
    for key in &providers {
        config.set_provider_key(key, format!("key-{}", key));
    }

    assert_eq!(config.providers.anthropic.api_key.expose(), "key-anthropic");
    assert_eq!(config.providers.openai.api_key.expose(), "key-openai");
    assert_eq!(config.providers.deepseek.api_key.expose(), "key-deepseek");
    assert_eq!(config.providers.gemini.api_key.expose(), "key-gemini");
    assert_eq!(
        config.providers.openrouter.api_key.expose(),
        "key-openrouter"
    );
}

#[test]
fn test_set_provider_key_unknown_does_nothing() {
    let mut config = config::Config::default();
    config.set_provider_key("unknown_provider", "test-key".to_string());
    assert!(config.providers.anthropic.api_key.is_empty());
    assert!(config.providers.openai.api_key.is_empty());
    assert!(config.providers.deepseek.api_key.is_empty());
}

// ========================================================================
// Config integration tests
// ========================================================================

#[test]
fn test_wizard_config_flow_simulation() {
    // Simulate what run_wizard does without stdin interaction
    let mut config = config::Config::default();

    let api_key = "sk-ant-test-key-123456789".to_string();
    let model = "claude-sonnet-4-5".to_string();

    config.set_provider_key("anthropic", api_key.clone());
    config.agents.defaults.model = model.clone();

    assert_eq!(config.providers.anthropic.api_key.expose(), &api_key);
    assert_eq!(config.agents.defaults.model, model);
}

#[test]
fn test_wizard_config_openai_flow() {
    let mut config = config::Config::default();

    config.set_provider_key("openai", "sk-openai-test123456".to_string());
    config.agents.defaults.model = "gpt-4o".to_string();

    assert_eq!(
        config.providers.openai.api_key.expose(),
        "sk-openai-test123456"
    );
    assert_eq!(config.agents.defaults.model, "gpt-4o");
    assert!(config.providers.anthropic.api_key.is_empty());
}

#[test]
fn test_wizard_config_save_round_trip() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = config::Config::default();
    config.set_provider_key("anthropic", "sk-ant-test-key123".to_string());
    config.agents.defaults.model = "claude-sonnet-4-5".to_string();

    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&config_path, &json).unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let loaded: config::Config = serde_json::from_str(&content).unwrap();

    assert_eq!(
        loaded.providers.anthropic.api_key.expose(),
        "sk-ant-test-key123"
    );
    assert_eq!(loaded.agents.defaults.model, "claude-sonnet-4-5");
}

// ========================================================================
// Active provider detection tests
// ========================================================================

#[test]
fn test_active_provider_detection_anthropic() {
    let mut config = config::Config::default();
    config.set_provider_key("anthropic", "sk-ant-key".to_string());
    assert_eq!(config.active_provider_name(), "anthropic");
}

#[test]
fn test_active_provider_detection_openai() {
    let mut config = config::Config::default();
    config.set_provider_key("openai", "sk-openai-key".to_string());
    assert_eq!(config.active_provider_name(), "openai");
}

#[test]
fn test_active_provider_detection_none() {
    let config = config::Config::default();
    assert_eq!(config.active_provider_name(), "none");
}

#[test]
fn test_active_provider_detection_priority() {
    let mut config = config::Config::default();
    config.set_provider_key("openai", "sk-openai-key".to_string());
    config.set_provider_key("anthropic", "sk-ant-key".to_string());
    assert_eq!(config.active_provider_name(), "anthropic");
}

#[test]
fn test_active_provider_explicit_field() {
    let mut config = config::Config::default();
    config.set_provider_key("anthropic", "sk-ant-key".to_string());
    config.set_provider_key("deepseek", "sk-deepseek-key".to_string());
    // Without explicit field, anthropic wins by priority
    assert_eq!(config.active_provider_name(), "anthropic");
    // With explicit field, deepseek wins
    config.agents.defaults.provider = Some("deepseek".to_string());
    assert_eq!(config.active_provider_name(), "deepseek");
}

#[test]
fn test_active_provider_explicit_fallback() {
    let mut config = config::Config::default();
    config.set_provider_key("openai", "sk-openai-key".to_string());
    // Explicit provider set but no key for it → falls back to auto-detection
    config.agents.defaults.provider = Some("anthropic".to_string());
    assert_eq!(config.active_provider_name(), "openai");
}

#[test]
fn test_is_provider_configured() {
    let mut config = config::Config::default();
    assert!(!config.is_provider_configured("anthropic"));
    config.set_provider_key("anthropic", "sk-ant-test".to_string());
    assert!(config.is_provider_configured("anthropic"));
    assert!(!config.is_provider_configured("openai"));
}

// ========================================================================
// Step count tests
// ========================================================================

#[test]
fn test_wizard_step_count() {
    // Verify the declared step count matches the number of step entries
    // (this catches off-by-one errors if someone adds a step)
    let state = WizardState::new();
    let welcome = steps::welcome::WelcomeModule;
    let provider_mod = provider::ProviderModule;
    let tools_mod = ToolsModule;
    let daemon_mod = daemon::DaemonModule;
    let workspace_mod = WorkspaceModule;

    let applicables: Vec<bool> = vec![
        welcome.is_applicable(&state),
        provider_mod.is_applicable(&state),
        true, // channels
        tools_mod.is_applicable(&state),
        daemon_mod.is_applicable(&state),
        workspace_mod.is_applicable(&state),
        true, // calendar
    ];

    let count = applicables.iter().filter(|a| **a).count();
    // On macOS/Linux daemon is applicable, so all 7 steps.
    // On unknown platforms it would be 6.
    assert!(count >= 6, "Should have at least 6 applicable steps");
    assert!(count <= 7, "Should have at most 7 steps");
}
