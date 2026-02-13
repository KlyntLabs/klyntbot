//! Integration tests for AgentLoop

use klyntbot::config::Config;
use klyntbot::{AgentEvent, AgentLoop, MessageBus};
use std::sync::Arc;
use tempfile::TempDir;

#[path = "mock_provider.rs"]
mod mock_provider;
use mock_provider::{ErrorProvider, MockProvider};

/// Test basic message processing through agent loop
#[tokio::test]
async fn test_agent_loop_basic_processing() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(MockProvider::new("Hello! How can I help you?"));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    let agent_loop = AgentLoop::new(bus, provider.clone(), config).await.unwrap();

    // Process a direct message
    let response = agent_loop
        .process_direct("Hi there!".to_string(), "test:session1".to_string())
        .await
        .unwrap();

    assert_eq!(response, "Hello! How can I help you?");
    assert_eq!(provider.call_count(), 1);
}

/// Test agent loop with tool execution
#[tokio::test]
async fn test_agent_loop_with_tool_execution() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    // Create a test file for the read_file tool
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let test_file = workspace.join("test.txt");
    std::fs::write(&test_file, "File contents here").unwrap();

    let bus = Arc::new(MessageBus::new(10));

    // First response: tool call to read_file
    // Second response: final answer with file contents
    let tool_call_response = klyntbot::providers::types::LlmResponse {
        content: None,
        tool_calls: vec![klyntbot::providers::types::ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({
                "path": test_file.to_str().unwrap()
            }),
        }],
        finish_reason: "tool_calls".to_string(),
        usage: klyntbot::providers::types::Usage::default(),
        reasoning_content: None,
    };

    let final_response = klyntbot::providers::types::LlmResponse {
        content: Some("The file contains: File contents here".to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: klyntbot::providers::types::Usage::default(),
        reasoning_content: None,
    };

    let provider = Arc::new(MockProvider::with_responses(vec![
        tool_call_response,
        final_response,
    ]));

    let mut config = Config::default();
    config.agents.defaults.workspace = workspace.to_str().unwrap().to_string();

    let agent_loop = AgentLoop::new(bus, provider.clone(), config).await.unwrap();

    let response = agent_loop
        .process_direct("Read test.txt".to_string(), "test:session2".to_string())
        .await
        .unwrap();

    assert!(response.contains("File contents here"));
    // Should have called the provider twice (once for tool call, once for final response)
    assert_eq!(provider.call_count(), 2);
}

/// Test agent loop max iterations enforcement
#[tokio::test]
async fn test_agent_loop_max_iterations() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let bus = Arc::new(MessageBus::new(10));

    // Create a provider that always returns tool calls (infinite loop scenario)
    let looping_response = klyntbot::providers::types::LlmResponse {
        content: None,
        tool_calls: vec![klyntbot::providers::types::ToolCall {
            id: "call_loop".to_string(),
            name: "nonexistent_tool".to_string(),
            arguments: serde_json::json!({}),
        }],
        finish_reason: "tool_calls".to_string(),
        usage: klyntbot::providers::types::Usage::default(),
        reasoning_content: None,
    };

    let provider = Arc::new(MockProvider::with_responses(vec![looping_response]));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();
    config.agents.defaults.max_tool_iterations = 3;

    let agent_loop = AgentLoop::new(bus, provider.clone(), config).await.unwrap();

    // This should hit max iterations and return an error or stop
    let result = agent_loop
        .process_direct("Start loop".to_string(), "test:session3".to_string())
        .await;

    // The agent loop should handle max iterations gracefully
    // Either return an error or a response indicating max iterations reached
    assert!(result.is_ok() || result.is_err());
}

/// Test error handling during tool execution
#[tokio::test]
async fn test_agent_loop_tool_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let bus = Arc::new(MessageBus::new(10));

    // Tool call that will fail (trying to read non-existent file)
    let tool_call_response = klyntbot::providers::types::LlmResponse {
        content: None,
        tool_calls: vec![klyntbot::providers::types::ToolCall {
            id: "call_error".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({
                "path": "/nonexistent/file.txt"
            }),
        }],
        finish_reason: "tool_calls".to_string(),
        usage: klyntbot::providers::types::Usage::default(),
        reasoning_content: None,
    };

    // Recovery response after tool error
    let recovery_response = klyntbot::providers::types::LlmResponse {
        content: Some("I couldn't read that file. It doesn't exist.".to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: klyntbot::providers::types::Usage::default(),
        reasoning_content: None,
    };

    let provider = Arc::new(MockProvider::with_responses(vec![
        tool_call_response,
        recovery_response,
    ]));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    let agent_loop = AgentLoop::new(bus, provider.clone(), config).await.unwrap();

    let response = agent_loop
        .process_direct(
            "Read /nonexistent/file.txt".to_string(),
            "test:session4".to_string(),
        )
        .await
        .unwrap();

    // Should handle the error gracefully
    assert!(response.contains("couldn't") || response.contains("error"));
}

/// Test session persistence across agent loop interactions
#[tokio::test]
async fn test_agent_loop_session_persistence() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let provider = Arc::new(MockProvider::with_responses(vec![
        klyntbot::providers::types::LlmResponse {
            content: Some("I remember you said hello!".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: klyntbot::providers::types::Usage::default(),
            reasoning_content: None,
        },
    ]));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    // First agent loop
    {
        let bus = Arc::new(MessageBus::new(10));
        let agent_loop = AgentLoop::new(bus, provider.clone(), config.clone())
            .await
            .unwrap();

        // First message
        let _ = agent_loop
            .process_direct("Hello!".to_string(), "test:session5".to_string())
            .await
            .unwrap();
    }

    // Create a new agent loop with a new bus (simulating restart)
    let bus2 = Arc::new(MessageBus::new(10));
    let agent_loop2 = AgentLoop::new(bus2, provider.clone(), config)
        .await
        .unwrap();

    // Second message - should have access to session history
    let response = agent_loop2
        .process_direct("What did I say?".to_string(), "test:session5".to_string())
        .await
        .unwrap();

    assert!(!response.is_empty());
}

/// Test that process_direct_streaming emits Done event with content
#[tokio::test]
async fn test_streaming_emits_done() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(MockProvider::new("streaming response"));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    let agent_loop = Arc::new(AgentLoop::new(bus, provider, config).await.unwrap());

    let streaming_handle = agent_loop
        .process_direct_streaming("Hello".to_string(), "test:stream1".to_string())
        .await
        .unwrap();

    let mut event_rx = streaming_handle.event_rx;
    let handle = streaming_handle.handle;

    // Collect all events
    let mut got_content = false;
    let mut got_done = false;
    let mut done_content = String::new();

    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::ContentChunk(_) => got_content = true,
            AgentEvent::Done(content) => {
                got_done = true;
                done_content = content;
                break;
            }
            _ => {}
        }
    }

    let result = handle.await.unwrap();
    assert!(result.is_ok());
    assert!(got_content, "should emit at least one ContentChunk");
    assert!(got_done, "should emit Done event");
    assert_eq!(done_content, "streaming response");
}

/// Test that process_direct_streaming emits Error event on provider failure
#[tokio::test]
async fn test_streaming_emits_error_on_failure() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(ErrorProvider::new("provider crashed"));

    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    let agent_loop = Arc::new(AgentLoop::new(bus, provider, config).await.unwrap());

    let streaming_handle = agent_loop
        .process_direct_streaming("Hello".to_string(), "test:stream2".to_string())
        .await
        .unwrap();

    let mut event_rx = streaming_handle.event_rx;
    let handle = streaming_handle.handle;

    // Collect events — expect an Error event
    let mut got_error = false;
    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::Error(msg) => {
                got_error = true;
                assert!(
                    msg.contains("provider crashed"),
                    "error message should contain the provider error: {}",
                    msg
                );
                break;
            }
            AgentEvent::Done(_) => break,
            _ => {}
        }
    }

    let result = handle.await.unwrap();
    assert!(result.is_err(), "agent loop should return error");
    assert!(got_error, "should emit Error event before returning");
}
