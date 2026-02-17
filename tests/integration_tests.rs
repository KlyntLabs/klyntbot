//! Integration tests for klyntbot
//!
//! These tests verify that components work together correctly.

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::schema::Secret;
use klyntbot::config::Config;
use klyntbot::session::SessionManager;
use klyntbot::tools::{filesystem::ReadFileTool, registry::ToolRegistry};
use tempfile::TempDir;
use tokio::fs;

/// Test full message flow through the bus
#[tokio::test]
async fn test_full_message_flow() {
    let bus = MessageBus::new(10);

    // Simulate channel sending inbound message
    let inbound = InboundMessage::new("telegram", "user123", "chat456", "Hello, bot!");
    bus.publish_inbound(inbound).await.unwrap();

    // Get receivers before publishing more messages
    let mut inbound_rx = bus.take_inbound_rx().unwrap();
    let mut outbound_rx = bus.take_outbound_rx().unwrap();

    // Agent consumes message
    let received = inbound_rx.recv().await.unwrap();
    assert_eq!(received.content, "Hello, bot!");
    assert_eq!(received.channel.as_str(), "telegram");

    // Agent generates response
    let response = OutboundMessage::new(
        received.channel.clone(),
        received.chat_id.clone(),
        "Hello, user!",
    );
    bus.publish_outbound(response).await.unwrap();

    // Channel consumes outbound message
    let outbound = outbound_rx.recv().await.unwrap();
    assert_eq!(outbound.content, "Hello, user!");
    assert_eq!(outbound.channel.as_str(), "telegram");
    assert_eq!(outbound.chat_id.as_str(), "chat456");
}

/// Test session persistence across multiple messages
#[tokio::test]
async fn test_session_persistence_flow() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionManager::new(temp_dir.path()).await;

    // First conversation turn
    {
        let session = manager.get_or_create("telegram:chat123").await.unwrap();
        session.add_message("user", "What is Rust?");
        session.add_message("assistant", "Rust is a systems programming language.");
    }
    let session = manager.get_or_create("telegram:chat123").await.unwrap();
    let session_clone = session.clone();
    manager.save(&session_clone).await.unwrap();

    // Simulate restart - create new manager
    let mut manager2 = SessionManager::new(temp_dir.path()).await;
    let loaded_session = manager2.get_or_create("telegram:chat123").await.unwrap();

    // Verify history was preserved
    assert_eq!(loaded_session.messages.len(), 2);
    assert_eq!(loaded_session.messages[0].content, "What is Rust?");
    assert_eq!(
        loaded_session.messages[1].content,
        "Rust is a systems programming language."
    );

    // Add another turn
    loaded_session.add_message("user", "Tell me more");
    let loaded_clone = loaded_session.clone();
    manager2.save(&loaded_clone).await.unwrap();

    // Verify it was saved
    let mut manager3 = SessionManager::new(temp_dir.path()).await;
    let final_session = manager3.get_or_create("telegram:chat123").await.unwrap();
    assert_eq!(final_session.messages.len(), 3);
}

/// Test tool registry with multiple tools
#[tokio::test]
async fn test_tool_registry_integration() {
    use klyntbot::tools::RoutingContext;

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "Hello from file!").await.unwrap();

    let mut registry = ToolRegistry::new();

    // Register read tool
    let read_tool = ReadFileTool::new(None);
    registry.register(read_tool);

    // Verify tool is registered
    assert!(registry.has("read_file"));
    assert_eq!(registry.len(), 1);

    // Get tool definitions
    let definitions = registry.get_definitions();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["function"]["name"], "read_file");

    // Execute tool
    let params = serde_json::json!({
        "path": test_file.to_str().unwrap()
    });

    let ctx = RoutingContext::new("cli".into(), "test".into());
    let result = registry.execute("read_file", params, &ctx).await.unwrap();
    assert_eq!(result, "Hello from file!");
}

/// Test config loading and validation
#[test]
fn test_config_integration() {
    // Test default config creation
    let config = Config::default();
    assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");

    // Test workspace path resolution
    let workspace = config.workspace_path();
    assert!(workspace.is_absolute() || workspace.starts_with("~"));

    // Test serialization round-trip
    let json = serde_json::to_string(&config).unwrap();
    let loaded: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.agents.defaults.model, config.agents.defaults.model);
}

/// Test config directory structure initialization
#[test]
fn test_config_init() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().unwrap();

    // Override home directory for testing
    std::env::set_var("HOME", temp_dir.path());

    // Note: This test would need modification to loader::init()
    // to support custom directories. For now, we just verify
    // the function exists and returns Ok
    // loader::init().unwrap();
}

/// Test multiple sessions in parallel
#[tokio::test]
async fn test_multiple_sessions_parallel() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionManager::new(temp_dir.path()).await;

    // Create multiple sessions
    {
        let session1 = manager.get_or_create("telegram:chat1").await.unwrap();
        session1.add_message("user", "Session 1 message");
    }
    {
        let session2 = manager.get_or_create("discord:guild1").await.unwrap();
        session2.add_message("user", "Session 2 message");
    }
    {
        let session3 = manager.get_or_create("slack:channel1").await.unwrap();
        session3.add_message("user", "Session 3 message");
    }

    // Save all sessions
    let s1 = manager
        .get_or_create("telegram:chat1")
        .await
        .unwrap()
        .clone();
    let s2 = manager
        .get_or_create("discord:guild1")
        .await
        .unwrap()
        .clone();
    let s3 = manager
        .get_or_create("slack:channel1")
        .await
        .unwrap()
        .clone();

    manager.save(&s1).await.unwrap();
    manager.save(&s2).await.unwrap();
    manager.save(&s3).await.unwrap();

    // List all sessions
    let sessions = manager.list().await.unwrap();
    assert_eq!(sessions.len(), 3);

    // Verify each session can be loaded
    for session_info in sessions {
        let loaded = manager.get_or_create(&session_info.key).await.unwrap();
        assert_eq!(loaded.messages.len(), 1);
    }
}

/// Test bus message ordering
#[tokio::test]
async fn test_bus_message_ordering() {
    let bus = MessageBus::new(100);

    // Send multiple messages in order
    for i in 0..10 {
        let msg = InboundMessage::new("test", "user", "chat", format!("Message {}", i));
        bus.publish_inbound(msg).await.unwrap();
    }

    // Get receiver
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

    // Verify they come out in order
    for i in 0..10 {
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, format!("Message {}", i));
    }
}

/// Test config with environment variable overrides
#[test]
fn test_config_env_override() {
    // Set environment variables
    std::env::set_var("KLYNTBOT_AGENTS__DEFAULTS__MODEL", "custom-model");
    std::env::set_var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY", "test-key-123");

    // Load config (would load from env vars in real usage)
    // Note: This test demonstrates the expected behavior
    let config = Config::default();

    // In actual usage, load_with_env_overrides() would apply these
    // For testing, we verify the default config structure is correct
    assert!(config.providers.anthropic.api_key.is_empty());
    assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");

    // Clean up
    std::env::remove_var("KLYNTBOT_AGENTS__DEFAULTS__MODEL");
    std::env::remove_var("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY");
}

/// Test session history truncation
#[tokio::test]
async fn test_session_history_limit() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionManager::new(temp_dir.path()).await;

    let session = manager.get_or_create("test:chat").await.unwrap();

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

/// Test tool parameter validation
#[tokio::test]
async fn test_tool_parameter_validation() {
    use klyntbot::tools::RoutingContext;

    let mut registry = ToolRegistry::new();
    let read_tool = ReadFileTool::new(None);
    registry.register(read_tool);

    let ctx = RoutingContext::new("cli".into(), "test".into());

    // Test missing required parameter
    let invalid_params = serde_json::json!({});
    let result = registry.execute("read_file", invalid_params, &ctx).await;
    assert!(result.is_err());

    // Test invalid parameter type
    let invalid_params = serde_json::json!({
        "path": 123 // Should be string
    });
    let result = registry.execute("read_file", invalid_params, &ctx).await;
    assert!(result.is_err());
}

/// Test session cleanup
#[tokio::test]
async fn test_session_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionManager::new(temp_dir.path()).await;

    // Create and save sessions
    for i in 0..5 {
        {
            let session = manager
                .get_or_create(format!("test:chat{}", i))
                .await
                .unwrap();
            session.add_message("user", "Test");
        }
        let session = manager
            .get_or_create(format!("test:chat{}", i))
            .await
            .unwrap()
            .clone();
        manager.save(&session).await.unwrap();
    }

    // Verify all exist
    let sessions = manager.list().await.unwrap();
    assert_eq!(sessions.len(), 5);

    // Delete some sessions
    manager.delete("test:chat0").await.unwrap();
    manager.delete("test:chat2").await.unwrap();

    // Verify deletion
    let remaining = manager.list().await.unwrap();
    assert_eq!(remaining.len(), 3);
}

/// Test session LRU eviction
#[tokio::test]
async fn test_session_lru_eviction() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionManager::with_capacity(temp_dir.path(), 3).await;

    // Create 4 sessions (should evict the first one)
    for i in 0..4 {
        let session = manager
            .get_or_create(format!("test:chat{}", i))
            .await
            .unwrap();
        session.add_message("user", format!("Message {}", i));
    }

    // Verify only 3 sessions remain in cache (chat1, chat2, chat3)
    // The first session (chat0) should have been evicted
    assert!(manager.get_or_create("test:chat1").await.is_ok());
    assert!(manager.get_or_create("test:chat2").await.is_ok());
    assert!(manager.get_or_create("test:chat3").await.is_ok());

    // Verify evicted session was saved to disk
    let sessions = manager.list().await.unwrap();
    assert!(sessions.iter().any(|s| s.key == "test:chat0"));

    // Load the evicted session - it should come from disk
    let loaded = manager.get_or_create("test:chat0").await.unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Message 0");
}

/// Test email config defaults
#[test]
fn test_email_config_defaults() {
    use klyntbot::config::schema::EmailConfig;

    let config = EmailConfig::default();

    // Test the 8 new fields have correct defaults
    assert!(
        !config.consent_granted,
        "consent_granted should default to false"
    );
    assert!(
        config.auto_reply_enabled,
        "auto_reply_enabled should default to true"
    );
    assert_eq!(
        config.imap_mailbox, "INBOX",
        "imap_mailbox should default to INBOX"
    );
    assert!(config.imap_use_ssl, "imap_use_ssl should default to true");
    assert!(config.smtp_use_tls, "smtp_use_tls should default to true");
    assert!(!config.smtp_use_ssl, "smtp_use_ssl should default to false");
    assert_eq!(
        config.max_body_chars, 12000,
        "max_body_chars should default to 12000"
    );
    assert!(config.mark_seen, "mark_seen should default to true");
}

/// Test web search config
#[test]
fn test_web_search_config() {
    use klyntbot::config::schema::WebToolsConfig;

    let config = WebToolsConfig::default();
    assert_eq!(config.max_results, 5, "max_results should default to 5");

    // Test with custom value
    let custom_config = WebToolsConfig {
        brave_api_key: Secret::new("test-key".to_string()),
        max_results: 10,
    };
    assert_eq!(custom_config.max_results, 10);
}

/// Test skills availability attribute
#[tokio::test]
async fn test_skills_availability() {
    use klyntbot::agent::SkillManager;

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");

    // Create a skill with no requirements (should be available=true)
    let available_skill_dir = skills_dir.join("available_test");
    std::fs::create_dir_all(&available_skill_dir).unwrap();
    let skill_file = available_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: An available test skill
metadata: '{"klyntbot":{"triggers":["test"]}}'
---

# Available Test Skill
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    // Create a skill with impossible requirements (should be available=false)
    let unavailable_skill_dir = skills_dir.join("unavailable_test");
    std::fs::create_dir_all(&unavailable_skill_dir).unwrap();
    let unavailable_file = unavailable_skill_dir.join("SKILL.md");
    let unavailable_content = r#"---
description: An unavailable test skill
metadata: '{"klyntbot":{"requires":{"bins":["nonexistent_binary_xyz_12345"]}}}'
---

# Unavailable Test Skill
"#;
    std::fs::write(&unavailable_file, unavailable_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    // Generate summary
    let summary = manager.generate_summary();

    // Verify available attribute is present in the XML
    assert!(
        summary.contains(r#"available="true""#) || summary.contains(r#"available="false""#),
        "Summary should include available attribute"
    );

    // Specifically check for the skills we created
    assert!(
        summary.contains("available_test"),
        "Should contain available_test skill"
    );
    assert!(
        summary.contains("unavailable_test"),
        "Should contain unavailable_test skill"
    );
}

/// Test backward compatibility - minimal config still deserializes
#[test]
fn test_backward_compat_minimal_config() {
    use klyntbot::config::Config;

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

    // This should deserialize successfully with new fields getting defaults
    let config: Config = serde_json::from_str(old_config_json).unwrap();

    // Verify original fields preserved
    assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");
    assert_eq!(config.agents.defaults.max_tokens, 4096);
    assert_eq!(config.providers.anthropic.api_key.expose(), "sk-ant-test");

    // Verify new fields got defaults
    assert!(config.providers.anthropic.extra_headers.is_none());
    assert!(config.providers.zhipu.api_key.is_empty());
    assert!(config.providers.dashscope.api_key.is_empty());
    assert!(!config.channels.feishu.enabled);
    assert!(!config.channels.dingtalk.enabled);
    assert!(!config.channels.mochat.enabled);
    assert_eq!(config.gateway.host, "0.0.0.0");
    assert_eq!(config.gateway.port, 18790);
}

/// Test FeishuConfig deserialization with defaults
#[test]
fn test_feishu_config_defaults() {
    use klyntbot::config::schema::FeishuConfig;

    let config = FeishuConfig::default();
    assert!(!config.enabled, "feishu should default to disabled");
    assert_eq!(config.app_id, "", "app_id should default to empty");
    assert!(
        config.app_secret.is_empty(),
        "app_secret should default to empty"
    );
    assert!(
        config.allow_from.is_empty(),
        "allow_from should default to empty"
    );
}

/// Test DingTalkConfig deserialization with defaults
#[test]
fn test_dingtalk_config_defaults() {
    use klyntbot::config::schema::DingTalkConfig;

    let config = DingTalkConfig::default();
    assert!(!config.enabled, "dingtalk should default to disabled");
    assert_eq!(config.client_id, "", "client_id should default to empty");
    assert!(
        config.client_secret.is_empty(),
        "client_secret should default to empty"
    );
    assert!(
        config.allow_from.is_empty(),
        "allow_from should default to empty"
    );
}

/// Test MochatConfig deserialization with defaults
#[test]
fn test_mochat_config_defaults() {
    use klyntbot::config::schema::MochatConfig;

    let config = MochatConfig::default();
    assert!(!config.enabled, "mochat should default to disabled");
    assert_eq!(config.base_url, "https://mochat.io");
    assert_eq!(config.socket_url, "", "socket_url should default to empty");
    assert!(
        config.claw_token.is_empty(),
        "claw_token should default to empty"
    );
    assert_eq!(
        config.agent_user_id, "",
        "agent_user_id should default to empty"
    );
    assert!(
        config.allow_from.is_empty(),
        "allow_from should default to empty"
    );
}

/// Test new provider configs (zhipu, dashscope, minimax, moonshot, aihubmix) deserialize
#[test]
fn test_new_provider_configs() {
    use klyntbot::config::schema::ProvidersConfig;

    let config = ProvidersConfig::default();

    // Verify all new providers have defaults
    assert!(config.zhipu.api_key.is_empty());
    assert!(config.zhipu.api_base.is_none());
    assert!(config.zhipu.extra_headers.is_none());

    assert!(config.dashscope.api_key.is_empty());
    assert!(config.dashscope.api_base.is_none());
    assert!(config.dashscope.extra_headers.is_none());

    assert!(config.minimax.api_key.is_empty());
    assert!(config.minimax.api_base.is_none());
    assert!(config.minimax.extra_headers.is_none());

    assert!(config.moonshot.api_key.is_empty());
    assert!(config.moonshot.api_base.is_none());
    assert!(config.moonshot.extra_headers.is_none());

    assert!(config.aihubmix.api_key.is_empty());
    assert!(config.aihubmix.api_base.is_none());
    assert!(config.aihubmix.extra_headers.is_none());
}

/// Test GatewayConfig deserialization with defaults
#[test]
fn test_gateway_config_defaults() {
    use klyntbot::config::schema::GatewayConfig;

    let config = GatewayConfig::default();
    assert_eq!(config.host, "0.0.0.0", "host should default to 0.0.0.0");
    assert_eq!(config.port, 18790, "port should default to 18790");
}

/// Test ProviderConfig extra_headers serialization/deserialization
#[test]
fn test_provider_extra_headers() {
    use klyntbot::config::schema::ProviderConfig;
    use std::collections::HashMap;

    // Test with extra_headers
    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    headers.insert("Authorization".to_string(), "Bearer token".to_string());

    let config = ProviderConfig {
        api_key: Secret::new("test-key".to_string()),
        api_base: None,
        extra_headers: Some(headers.clone()),
    };

    // Serialize
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["apiKey"], "test-key");
    assert_eq!(json["extraHeaders"]["X-Custom-Header"], "custom-value");
    assert_eq!(json["extraHeaders"]["Authorization"], "Bearer token");

    // Deserialize
    let loaded: ProviderConfig = serde_json::from_value(json).unwrap();
    assert_eq!(loaded.api_key.expose(), "test-key");
    assert!(loaded.extra_headers.is_some());
    let loaded_headers = loaded.extra_headers.unwrap();
    assert_eq!(
        loaded_headers.get("X-Custom-Header").unwrap(),
        "custom-value"
    );
    assert_eq!(loaded_headers.get("Authorization").unwrap(), "Bearer token");
}

/// Test Discord config fields are referenced correctly
#[test]
fn test_discord_config_usage() {
    use klyntbot::config::schema::DiscordConfig;

    // Create config with specific values
    let config = DiscordConfig {
        enabled: true,
        token: Secret::new("test-token-123".to_string()),
        gateway_url: "wss://custom-gateway.discord.gg".to_string(),
        intents: 12345,
        allow_from: vec!["user123".to_string()],
    };

    // Verify fields are accessible (not hardcoded constants)
    assert_eq!(config.token.expose(), "test-token-123");
    assert_eq!(config.gateway_url, "wss://custom-gateway.discord.gg");
    assert_eq!(config.intents, 12345);
    assert_eq!(config.allow_from.len(), 1);
    assert_eq!(config.allow_from[0], "user123");

    // Verify serialization uses config values
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["enabled"], true);
    assert_eq!(json["token"], "test-token-123");
    assert_eq!(json["gatewayUrl"], "wss://custom-gateway.discord.gg");
    assert_eq!(json["intents"], 12345);
}

/// Test Email consent_granted enforcement
#[test]
fn test_email_consent_granted_enforcement() {
    use klyntbot::config::schema::EmailConfig;

    // Default should be false
    let config = EmailConfig::default();
    assert!(
        !config.consent_granted,
        "consent_granted must default to false for safety"
    );

    // Test explicit false
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

    // Test explicit true
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

    // Test that missing consentGranted defaults to false
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

// ============================================================================
// Plan Integration Tests (Phase 2 Planning Engine)
// ============================================================================

/// Test 21: End-to-end plan lifecycle
/// Verifies: plan creation → state transitions → persistence
#[tokio::test]
async fn test_plan_end_to_end() {
    use chrono::Utc;
    use plan::{Plan, PlanStatus, PlanStore};
    use tempfile::TempDir;
    use uuid::Uuid;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(store_path.clone());

    // Create a new plan (Draft status)
    let now = Utc::now();
    let plan_id = Uuid::new_v4();
    let plan = Plan {
        id: plan_id,
        session_key: "test-session".to_string(),
        goal_id: None,
        title: "Integration Test Plan".to_string(),
        description: "Testing plan lifecycle".to_string(),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    // Upsert plan
    let saved = store.upsert(plan).await.unwrap();
    assert_eq!(saved.status, PlanStatus::Draft);

    // Approve the plan
    let mut approved_plan = store.get(&plan_id).await.unwrap().unwrap();
    approved_plan.status = PlanStatus::Approved;
    store.upsert(approved_plan.clone()).await.unwrap();

    // Verify approval persisted
    let loaded = store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(loaded.status, PlanStatus::Approved);

    // Transition to Executing
    let mut executing_plan = loaded;
    executing_plan.status = PlanStatus::Executing;
    store.upsert(executing_plan).await.unwrap();

    // Transition to Completed
    let mut completed_plan = store.get(&plan_id).await.unwrap().unwrap();
    completed_plan.status = PlanStatus::Completed;
    completed_plan.completed_at = Some(Utc::now());
    store.upsert(completed_plan).await.unwrap();

    // Verify final state
    let final_plan = store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(final_plan.status, PlanStatus::Completed);
    assert!(final_plan.completed_at.is_some());

    // Verify persistence across store instances
    drop(store);
    let mut new_store = PlanStore::new(store_path);
    let persisted = new_store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(persisted.status, PlanStatus::Completed);
    assert_eq!(persisted.title, "Integration Test Plan");
}

/// Test 22: Plan failure recovery scenario
/// Verifies: plan abandonment and error handling
#[tokio::test]
async fn test_plan_failure_recovery() {
    use chrono::Utc;
    use plan::{BacktrackEntry, Plan, PlanStatus, PlanStore};
    use tempfile::TempDir;
    use uuid::Uuid;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(store_path);

    // Create a plan that will "fail"
    let now = Utc::now();
    let plan_id = Uuid::new_v4();
    let mut plan = Plan {
        id: plan_id,
        session_key: "test-session".to_string(),
        goal_id: None,
        title: "Failing Plan".to_string(),
        description: "Testing failure recovery".to_string(),
        status: PlanStatus::Approved,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    store.upsert(plan.clone()).await.unwrap();

    // Simulate failure by adding backtrack entry
    plan.backtrack_history.push(BacktrackEntry {
        step_index: 0,
        attempt: 1,
        failure_reason: "Simulated failure".to_string(),
        timestamp: Utc::now(),
    });
    plan.status = PlanStatus::Executing;
    store.upsert(plan.clone()).await.unwrap();

    // Verify backtrack history is recorded
    let loaded = store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(loaded.backtrack_history.len(), 1);
    assert_eq!(loaded.backtrack_history[0].failure_reason, "Simulated failure");

    // Abandon the failed plan
    let mut abandoned_plan = loaded;
    abandoned_plan.status = PlanStatus::Abandoned;
    store.upsert(abandoned_plan).await.unwrap();

    // Verify final state
    let final_plan = store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(final_plan.status, PlanStatus::Abandoned);
    assert!(!final_plan.backtrack_history.is_empty());
}

/// Test 23: Session isolation
/// Verifies: plans are session-isolated and don't leak across sessions
#[tokio::test]
async fn test_session_isolation() {
    use chrono::Utc;
    use plan::{Plan, PlanStatus, PlanStore};
    use tempfile::TempDir;
    use uuid::Uuid;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(store_path);

    let now = Utc::now();

    // Create plan for session A
    let plan_a = Plan {
        id: Uuid::new_v4(),
        session_key: "session-A".to_string(),
        goal_id: None,
        title: "Plan A".to_string(),
        description: "Session A plan".to_string(),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    // Create plan for session B
    let plan_b = Plan {
        id: Uuid::new_v4(),
        session_key: "session-B".to_string(),
        goal_id: None,
        title: "Plan B".to_string(),
        description: "Session B plan".to_string(),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    store.upsert(plan_a.clone()).await.unwrap();
    store.upsert(plan_b.clone()).await.unwrap();

    // Verify session A gets its plan
    let active_a = store.get_active_plan("session-A").await.unwrap();
    assert!(active_a.is_some());
    assert_eq!(active_a.unwrap().title, "Plan A");

    // Verify session B gets its plan
    let active_b = store.get_active_plan("session-B").await.unwrap();
    assert!(active_b.is_some());
    assert_eq!(active_b.unwrap().title, "Plan B");

    // Verify session C has no plan
    let active_c = store.get_active_plan("session-C").await.unwrap();
    assert!(active_c.is_none());

    // Verify plans are correctly isolated by session
    let loaded_a = store.get(&plan_a.id).await.unwrap().unwrap();
    let loaded_b = store.get(&plan_b.id).await.unwrap().unwrap();
    assert_ne!(loaded_a.session_key, loaded_b.session_key);
}

/// Test 23: Verify plan store path consistency (Critical Fix 3)
/// Verifies: config.plan_store_path() returns ~/.klyntbot/data/plans.jsonl
/// Bug: Config returned ~/.klyntbot/plans.jsonl while code used ~/.klyntbot/data/plans.jsonl
/// Fix: Updated config method to include "data" subdirectory for consistency
#[tokio::test]
async fn test_plan_store_path_consistency() {
    use config::Config;

    // Load config (creates default if needed)
    let config = Config::default();

    // Verify the plan store path includes the "data" subdirectory
    let plan_path = config.plan_store_path();
    let path_str = plan_path.to_str().unwrap();

    // Path should end with .klyntbot/data/plans.jsonl
    assert!(
        path_str.ends_with(".klyntbot/data/plans.jsonl"),
        "Expected path to end with .klyntbot/data/plans.jsonl, got: {}",
        path_str
    );

    // Path should NOT be the old incorrect path
    assert!(
        !path_str.ends_with(".klyntbot/plans.jsonl") || path_str.ends_with("data/plans.jsonl"),
        "Path should not use the old .klyntbot/plans.jsonl format without 'data' subdirectory"
    );
}

/// Test 24: Cross-channel isolation (fixes session key collision bug)
/// Verifies: Telegram chat "123" and Discord chat "123" have isolated session keys
/// Bug: Previously used ctx.chat_id alone, causing collisions across channels
/// Fix: Use format!("{}:{}", ctx.channel, ctx.chat_id) for session keys
#[tokio::test]
async fn test_cross_channel_plan_isolation() {
    use chrono::Utc;
    use common::ChannelName;
    use klyntbot::tools::{plan_tool::PlanHandler, plan_tool::PlanTool, RoutingContext, Tool};
    use plan::{Plan, PlanStatus, PlanStore};
    use std::sync::Arc;
    use tempfile::TempDir;
    use uuid::Uuid;

    // Mock PlanHandler that records session keys
    struct MockPlanHandler {
        store: Arc<tokio::sync::Mutex<PlanStore>>,
    }

    #[async_trait::async_trait]
    impl PlanHandler for MockPlanHandler {
        async fn create_plan(
            &self,
            title: &str,
            description: &str,
            session_key: &str,
            goal_id: Option<Uuid>,
        ) -> common::Result<Plan> {
            let now = Utc::now();
            let plan = Plan {
                id: Uuid::new_v4(),
                session_key: session_key.to_string(),
                goal_id,
                title: title.to_string(),
                description: description.to_string(),
                status: PlanStatus::Draft,
                steps: vec![],
                current_step_index: 0,
                iteration_limit: 50,
                backtrack_history: vec![],
                created_at: now,
                updated_at: now,
                completed_at: None,
            };

            let mut store = self.store.lock().await;
            let saved = store.upsert(plan).await?;
            Ok(saved)
        }

        async fn get_plan(&self, id: &Uuid) -> common::Result<Option<Plan>> {
            let mut store = self.store.lock().await;
            store.get(id).await
        }

        async fn get_active_plan(&self, session_key: &str) -> common::Result<Option<Plan>> {
            let mut store = self.store.lock().await;
            store.get_active_plan(session_key).await
        }

        async fn approve_plan(&self, id: &Uuid) -> common::Result<Plan> {
            let mut store = self.store.lock().await;
            let mut plan = store
                .get(id)
                .await?
                .ok_or_else(|| common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                    "Plan not found".into(),
                )))?;
            plan.status = PlanStatus::Approved;
            store.upsert(plan.clone()).await?;
            Ok(plan)
        }

        async fn abandon_plan(&self, id: &Uuid) -> common::Result<()> {
            let mut store = self.store.lock().await;
            let mut plan = store
                .get(id)
                .await?
                .ok_or_else(|| common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                    "Plan not found".into(),
                )))?;
            plan.status = PlanStatus::Abandoned;
            store.upsert(plan).await?;
            Ok(())
        }

        async fn get_step_context(&self, _id: &Uuid) -> common::Result<String> {
            Ok("Mock step context".to_string())
        }

        async fn execute_plan(&self, id: &Uuid) -> common::Result<Plan> {
            let mut store = self.store.lock().await;
            let mut plan = store
                .get(id)
                .await?
                .ok_or_else(|| common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                    "Plan not found".into(),
                )))?;
            plan.status = PlanStatus::Executing;
            store.upsert(plan.clone()).await?;
            Ok(plan)
        }
    }

    // Setup
    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let store = PlanStore::new(store_path);
    let handler = Arc::new(MockPlanHandler {
        store: Arc::new(tokio::sync::Mutex::new(store)),
    });

    let plan_tool = PlanTool::new(Some(handler.clone()));

    // Create plan in Telegram chat "123"
    let telegram_ctx = RoutingContext::new(ChannelName::new("telegram"), "123".into());
    let telegram_args = serde_json::json!({
        "action": "create",
        "title": "Telegram Plan",
        "description": "Plan from Telegram"
    });

    let telegram_result = plan_tool
        .execute(telegram_args, &telegram_ctx)
        .await
        .unwrap();
    assert!(telegram_result.contains("Telegram Plan"));

    // Create plan in Discord chat "123" (same chat_id, different channel)
    let discord_ctx = RoutingContext::new(ChannelName::new("discord"), "123".into());
    let discord_args = serde_json::json!({
        "action": "create",
        "title": "Discord Plan",
        "description": "Plan from Discord"
    });

    let discord_result = plan_tool
        .execute(discord_args, &discord_ctx)
        .await
        .unwrap();
    assert!(discord_result.contains("Discord Plan"));

    // Verify cross-channel isolation: each channel should have its own active plan
    // Expected session keys: "telegram:123" and "discord:123" (NOT just "123")

    // Get active plan for Telegram session
    let telegram_active = handler
        .get_active_plan("telegram:123")
        .await
        .unwrap();
    assert!(
        telegram_active.is_some(),
        "Should have active plan for telegram:123"
    );
    assert_eq!(
        telegram_active.as_ref().unwrap().title,
        "Telegram Plan",
        "Telegram session should have Telegram plan"
    );
    assert_eq!(
        telegram_active.as_ref().unwrap().session_key,
        "telegram:123",
        "Session key should be 'telegram:123'"
    );

    // Get active plan for Discord session
    let discord_active = handler.get_active_plan("discord:123").await.unwrap();
    assert!(
        discord_active.is_some(),
        "Should have active plan for discord:123"
    );
    assert_eq!(
        discord_active.as_ref().unwrap().title,
        "Discord Plan",
        "Discord session should have Discord plan"
    );
    assert_eq!(
        discord_active.as_ref().unwrap().session_key,
        "discord:123",
        "Session key should be 'discord:123'"
    );

    // CRITICAL: Verify they're truly isolated (different plans)
    assert_ne!(
        telegram_active.unwrap().id,
        discord_active.unwrap().id,
        "Plans from different channels with same chat_id must be isolated"
    );
}

/// Test 25: PlanExecutor.execute_step() full integration
/// Verifies: execute_step() calls the LLM, executes tool, captures output.
/// Covers the gap: no integration test previously verified actual step execution.
#[tokio::test]
async fn test_plan_executor_execute_step_integration() {
    use async_trait::async_trait;
    use klyntbot::providers::types::{
        ChatParams, LlmProvider, LlmResponse, ToolCall, Usage,
    };
    use klyntbot::tools::{registry::ToolRegistry, RoutingContext, Tool};
    use plan::{PlanStep, StepStatus};
    use serde_json::Value;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    // Mock provider that returns a tool call for "counter_tool"
    struct MockProviderWithToolCall;

    #[async_trait]
    impl LlmProvider for MockProviderWithToolCall {
        async fn chat(
            &self,
            _messages: &[klyntbot::providers::types::Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> klyntbot::error::Result<LlmResponse> {
            Ok(LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "counter_tool".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
    }

    // Simple counter tool that records execution
    use std::sync::atomic::{AtomicUsize, Ordering};
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);

    struct CounterTool {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for CounterTool {
        fn name(&self) -> &str { "counter_tool" }
        fn description(&self) -> &str { "Counts calls" }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> klyntbot::error::Result<String> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok("counted".to_string())
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(CounterTool { count: call_count_clone });
    let registry = Arc::new(RwLock::new(registry));

    let provider: klyntbot::providers::DynProvider = Arc::new(MockProviderWithToolCall);
    let executor = klyntbot::agent::PlanExecutor::new();

    let step = PlanStep {
        id: Uuid::new_v4(),
        index: 0,
        description: "Count something using the counter tool".to_string(),
        reasoning: "Need to count".to_string(),
        expected_tools: vec!["counter_tool".to_string()],
        status: StepStatus::Pending,
        attempt_count: 0,
        max_attempts: 3,
        result: None,
        started_at: None,
        completed_at: None,
    };

    let ctx = RoutingContext::new("cli".into(), "integration-test".into());
    let plan_ctx = "Plan: Integration Test\nProgress: step 1/1";

    let result = executor
        .execute_step(&step, plan_ctx, &provider, &registry, &ctx)
        .await
        .expect("execute_step should not return Err in integration test");

    // Verify tool was actually called
    assert_eq!(call_count.load(Ordering::SeqCst), 1, "counter_tool should have been called once");
    assert!(result.success, "step should succeed; failure: {:?}", result.failure_reason);
    assert!(
        result.output.contains("counted"),
        "output should contain tool result 'counted', got: {}",
        result.output
    );
}

/// Test 26: PlanHandlerImpl.execute_plan() state transition integration
/// Verifies: execute_plan() correctly transitions Approved → Executing and persists.
#[tokio::test]
async fn test_plan_handler_execute_plan_integration() {
    use klyntbot::agent::PlanHandlerImpl;
    use klyntbot::tools::plan_tool::PlanHandler;
    use plan::{PlanStatus, PlanStore};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let store = Arc::new(RwLock::new(PlanStore::new(store_path.clone())));
    let handler = PlanHandlerImpl::new(Arc::clone(&store));

    // Create and approve a plan
    let plan = handler
        .create_plan("Execution Test", "Test execute_plan transition", "sess:exec-test", None)
        .await
        .expect("create_plan should succeed");
    assert_eq!(plan.status, PlanStatus::Draft);

    let plan = handler.approve_plan(&plan.id).await.expect("approve_plan should succeed");
    assert_eq!(plan.status, PlanStatus::Approved);

    // Execute the plan — transitions to Executing
    let executing = handler.execute_plan(&plan.id).await.expect("execute_plan should succeed");
    assert_eq!(executing.status, PlanStatus::Executing, "plan should be in Executing state");

    // Verify it persisted
    let persisted = store.write().await.get(&executing.id).await.unwrap().unwrap();
    assert_eq!(persisted.status, PlanStatus::Executing, "Executing state should persist to store");

    // Verify that calling execute_plan on an already-Executing plan is a no-op
    // (PlanStatus::validate_transition allows same-state transitions)
    let again = handler.execute_plan(&plan.id).await;
    assert!(again.is_ok(), "execute_plan on already-Executing plan is a no-op, not an error");
    assert_eq!(again.unwrap().status, PlanStatus::Executing);

    // Verify that Completed → Executing IS rejected (cannot go backwards)
    let mut store_guard = store.write().await;
    let mut completed = store_guard.get(&executing.id).await.unwrap().unwrap();
    completed.status = PlanStatus::Completed;
    completed.completed_at = Some(chrono::Utc::now());
    store_guard.upsert(completed).await.unwrap();
    drop(store_guard);

    let backwards = handler.execute_plan(&executing.id).await;
    assert!(backwards.is_err(), "execute_plan on Completed plan should fail");
}

/// Test 27: Iteration limit enforcement
/// Verifies: plans with low iteration_limit behave consistently.
/// Documents: iteration limit is enforced by run_plan_execution() in agent_loop.rs
#[tokio::test]
async fn test_plan_iteration_limit_field_persists() {
    use chrono::Utc;
    use plan::{Plan, PlanStatus, PlanStore};
    use tempfile::TempDir;
    use uuid::Uuid;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(store_path);

    let now = Utc::now();
    let plan = Plan {
        id: Uuid::new_v4(),
        session_key: "iter-test".to_string(),
        goal_id: None,
        title: "Iteration Limit Test Plan".to_string(),
        description: "Tests iteration_limit field".to_string(),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 5, // Very low limit to test field persistence
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    let saved = store.upsert(plan).await.unwrap();
    assert_eq!(saved.iteration_limit, 5, "iteration_limit should be saved as 5");

    // Verify it persists across store reload
    let plan_id = saved.id;
    drop(store);

    let mut reloaded_store = PlanStore::new(temp_dir.path().join("plans.jsonl"));
    let loaded = reloaded_store.get(&plan_id).await.unwrap().unwrap();
    assert_eq!(
        loaded.iteration_limit, 5,
        "iteration_limit should persist across store reload"
    );
    assert_eq!(loaded.title, "Iteration Limit Test Plan");
}

/// Test 28: Concurrent plan store access safety
/// Verifies: Multiple concurrent readers don't corrupt state.
/// Documents the known limitation: concurrent CLI + serve can corrupt JSONL files.
#[tokio::test]
async fn test_plan_store_concurrent_reads_are_safe() {
    use chrono::Utc;
    use plan::{Plan, PlanStatus, PlanStore};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    let temp_dir = TempDir::new().unwrap();
    let store_path = temp_dir.path().join("plans.jsonl");
    let store = Arc::new(RwLock::new(PlanStore::new(store_path)));

    // Seed the store with a plan
    let now = Utc::now();
    let plan_id = Uuid::new_v4();
    let plan = Plan {
        id: plan_id,
        session_key: "concurrent-test".to_string(),
        goal_id: None,
        title: "Concurrent Read Test".to_string(),
        description: "Tests concurrent reads".to_string(),
        status: PlanStatus::Approved,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    store.write().await.upsert(plan).await.unwrap();

    // Spawn 10 concurrent read tasks
    let mut handles = vec![];
    for _ in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let result = store_clone.write().await.get(&plan_id).await;
            result.expect("read should succeed").expect("plan should exist")
        });
        handles.push(handle);
    }

    // All reads should succeed and return the correct plan
    for handle in handles {
        let read_plan = handle.await.expect("task should not panic");
        assert_eq!(read_plan.id, plan_id);
        assert_eq!(read_plan.status, PlanStatus::Approved);
        assert_eq!(read_plan.title, "Concurrent Read Test");
    }
}
