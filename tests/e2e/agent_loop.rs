//! Integration tests for AgentLoop

use klyntbot::config::Config;
use klyntbot::{AgentEvent, AgentLoop, MessageBus};
use std::sync::Arc;
use tempfile::TempDir;

use super::common::{ErrorProvider, MockProvider};

/// Helper: build an AgentLoop with default test configuration.
async fn build_test_agent(
    bus: Arc<MessageBus>,
    provider: Arc<dyn klyntbot::LlmProvider>,
    config: Config,
) -> AgentLoop {
    AgentLoop::builder(bus, provider, config)
        .build()
        .await
        .unwrap()
}

/// Test basic message processing through agent loop
#[tokio::test]
async fn agent_loop_handles_single_message() {
    let temp_dir = TempDir::new().unwrap();
    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(MockProvider::new("Hello! How can I help you?"));
    let config = super::common::test_config(&temp_dir);

    let agent_loop = build_test_agent(bus, provider.clone(), config).await;

    let response = agent_loop
        .process_direct("Hi there!".to_string(), "test:session1".to_string())
        .await
        .unwrap();

    assert_eq!(response, "Hello! How can I help you?");
    assert_eq!(provider.call_count(), 1);
}

/// Test agent loop with tool execution
#[tokio::test]
async fn agent_loop_executes_tool_call() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let test_file = workspace.join("test.txt");
    std::fs::write(&test_file, "File contents here").unwrap();

    let bus = Arc::new(MessageBus::new(10));

    let provider = Arc::new(MockProvider::with_responses(vec![
        super::common::tool_call_response(
            "read_file",
            serde_json::json!({ "path": test_file.to_str().unwrap() }),
        ),
        super::common::text_response("The file contains: File contents here"),
    ]));

    let mut config = super::common::test_config(&temp_dir);
    config.agents.defaults.workspace = workspace.to_str().unwrap().to_string();

    let agent_loop = build_test_agent(bus, provider.clone(), config).await;

    let response = agent_loop
        .process_direct("Read test.txt".to_string(), "test:session2".to_string())
        .await
        .unwrap();

    assert!(response.contains("File contents here"));
    assert_eq!(provider.call_count(), 2);
}

/// Test agent loop max iterations enforcement
#[tokio::test]
async fn agent_loop_enforces_max_iterations() {
    let temp_dir = TempDir::new().unwrap();
    let bus = Arc::new(MessageBus::new(10));

    let provider = Arc::new(MockProvider::with_responses(vec![
        super::common::tool_call_response("nonexistent_tool", serde_json::json!({})),
    ]));

    let mut config = super::common::test_config(&temp_dir);
    config.agents.defaults.max_tool_iterations = 3;

    let agent_loop = build_test_agent(bus, provider.clone(), config).await;

    let result = agent_loop
        .process_direct("Start loop".to_string(), "test:session3".to_string())
        .await;

    // The agent loop should handle max iterations gracefully —
    // either return a stop message or an error.
    assert!(
        result.is_ok() || result.unwrap_err().to_string().contains("iteration"),
        "max iterations should terminate gracefully"
    );
}

/// Test error handling during tool execution
#[tokio::test]
async fn agent_loop_handles_tool_error() {
    let temp_dir = TempDir::new().unwrap();
    let bus = Arc::new(MessageBus::new(10));

    let provider = Arc::new(MockProvider::with_responses(vec![
        super::common::tool_call_response(
            "read_file",
            serde_json::json!({ "path": "/nonexistent/file.txt" }),
        ),
        super::common::text_response("I couldn't read that file. It doesn't exist."),
    ]));

    let config = super::common::test_config(&temp_dir);

    let agent_loop = build_test_agent(bus, provider.clone(), config).await;

    let response = agent_loop
        .process_direct(
            "Read /nonexistent/file.txt".to_string(),
            "test:session4".to_string(),
        )
        .await
        .unwrap();

    assert!(response.contains("couldn't") || response.contains("error"));
}

/// Test session persistence across agent loop interactions
#[tokio::test]
async fn agent_loop_persists_session() {
    let temp_dir = TempDir::new().unwrap();

    let provider = Arc::new(MockProvider::with_responses(vec![
        super::common::text_response("I remember you said hello!"),
    ]));

    let config = super::common::test_config(&temp_dir);

    {
        let bus = Arc::new(MessageBus::new(10));
        let agent_loop = build_test_agent(bus, provider.clone(), config.clone()).await;
        let _ = agent_loop
            .process_direct("Hello!".to_string(), "test:session5".to_string())
            .await
            .unwrap();
    }

    let bus2 = Arc::new(MessageBus::new(10));
    let agent_loop2 = build_test_agent(bus2, provider.clone(), config).await;

    let response = agent_loop2
        .process_direct("What did I say?".to_string(), "test:session5".to_string())
        .await
        .unwrap();

    assert!(!response.is_empty());
}

/// Test that process_direct_streaming emits Done event with content
#[tokio::test]
async fn streaming_emits_done_event() {
    let temp_dir = TempDir::new().unwrap();
    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(MockProvider::new("streaming response"));
    let config = super::common::test_config(&temp_dir);

    let agent_loop = Arc::new(build_test_agent(bus, provider, config).await);

    let streaming_handle = agent_loop
        .process_direct_streaming("Hello".to_string(), "test:stream1".to_string(), None)
        .await
        .unwrap();

    let mut event_rx = streaming_handle.event_rx;
    let handle = streaming_handle.handle;

    let mut got_content = false;
    let mut got_done = false;
    let mut done_content = String::new();

    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::ContentChunk { .. } => got_content = true,
            AgentEvent::Done { content, .. } => {
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
async fn streaming_emits_error_on_failure() {
    let temp_dir = TempDir::new().unwrap();
    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(ErrorProvider::new("provider crashed"));
    let config = super::common::test_config(&temp_dir);

    let agent_loop = Arc::new(build_test_agent(bus, provider, config).await);

    let streaming_handle = agent_loop
        .process_direct_streaming("Hello".to_string(), "test:stream2".to_string(), None)
        .await
        .unwrap();

    let mut event_rx = streaming_handle.event_rx;
    let handle = streaming_handle.handle;

    let mut got_error = false;
    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::Error { message: msg } => {
                got_error = true;
                assert!(
                    msg.contains("provider crashed"),
                    "error message should contain the provider error: {}",
                    msg
                );
                break;
            }
            AgentEvent::Done { .. } => break,
            _ => {}
        }
    }

    let result = handle.await.unwrap();
    assert!(result.is_err(), "agent loop should return error");
    assert!(got_error, "should emit Error event before returning");
}
