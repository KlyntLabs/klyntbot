//! Integration tests for klyntbot
//!
//! These tests verify that components work together correctly.

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::schema::Secret;
use klyntbot::config::Config;
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

/// Test session persistence across multiple messages (in-memory only).
/// Full SQL persistence is tested via the SQL-backed manager integration tests.
#[test]
fn test_session_persistence_flow() {
    use klyntbot::session::Session;

    let mut session = Session::new("telegram:chat123");

    // First conversation turn
    session.add_message("user", "What is Rust?");
    session.add_message("assistant", "Rust is a systems programming language.");

    // Verify history was preserved
    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.messages[0].content, "What is Rust?");
    assert_eq!(
        session.messages[1].content,
        "Rust is a systems programming language."
    );

    // Add another turn
    session.add_message("user", "Tell me more");
    assert_eq!(session.messages.len(), 3);
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

/// Test multiple sessions created independently
#[test]
fn test_multiple_sessions_parallel() {
    use klyntbot::session::Session;

    let mut session1 = Session::new("telegram:chat1");
    session1.add_message("user", "Session 1 message");

    let mut session2 = Session::new("discord:guild1");
    session2.add_message("user", "Session 2 message");

    let mut session3 = Session::new("slack:channel1");
    session3.add_message("user", "Session 3 message");

    // Verify each session has correct data
    assert_eq!(session1.key, "telegram:chat1");
    assert_eq!(session1.messages.len(), 1);
    assert_eq!(session2.key, "discord:guild1");
    assert_eq!(session2.messages.len(), 1);
    assert_eq!(session3.key, "slack:channel1");
    assert_eq!(session3.messages.len(), 1);
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

/// Test session history truncation
#[test]
fn test_session_history_limit() {
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

/// Test session clear operation
#[test]
fn test_session_cleanup() {
    use klyntbot::session::Session;

    let mut session = Session::new("test:chat");
    session.add_message("user", "msg1");
    session.add_message("assistant", "msg2");
    assert_eq!(session.messages.len(), 2);

    session.clear();
    assert_eq!(session.messages.len(), 0);
}

// LRU eviction test removed — relied on `with_capacity()` and disk persistence.
// Cache eviction + SQL reload is tested implicitly by get_or_create() in the SQL-backed manager.

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
    assert_eq!(config.gateway.host, "127.0.0.1");
    assert_eq!(config.gateway.port, 18790);
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
        ..Default::default()
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

// Plan integration tests removed — plan functionality is tested via PlanHandlerImpl + PlanRepo in the agent crate.
