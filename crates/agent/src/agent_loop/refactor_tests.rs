//! Tests for the two-phase AgentLoop construction refactor.
//!
//! Verifies that:
//! - `take_inbound_rx()` extracts the receiver and leaves None in place
//! - `run_with_rx()` accepts `&self` (not `&mut self`)
//! - The original `run(&mut self)` still compiles and works (backward compat)
//! - `Arc<AgentLoop>` can call `process_direct_streaming()` without a Mutex

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use bus::MessageBus;
    use providers::{ChatParams, LlmResponse, Message, Usage};
    use tempfile::TempDir;

    use crate::AgentEvent;
    use crate::agent_loop::AgentLoop;

    /// Minimal mock LLM provider for unit tests.
    struct MockProvider {
        response: String,
    }

    impl MockProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl providers::LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.response.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }

        fn default_model(&self) -> &str {
            "mock"
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Helper: build a minimal AgentLoop for tests.
    /// Returns (agent, bus, tempdir). The builder has already consumed bus.inbound_rx.
    async fn build_test_agent(response: &str) -> (AgentLoop, Arc<MessageBus>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("HOME", temp_dir.path());

        let bus = Arc::new(MessageBus::new(10));
        let provider: providers::DynProvider = Arc::new(MockProvider::new(response));

        let mut config = config::Config::default();
        config.agents.defaults.workspace = temp_dir
            .path()
            .join("workspace")
            .to_str()
            .unwrap()
            .to_string();

        let agent = AgentLoop::builder()
            .with_bus(Arc::clone(&bus))
            .with_provider(provider)
            .with_config(config)
            .build()
            .await
            .unwrap();

        (agent, bus, temp_dir)
    }

    // ── take_inbound_rx ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn take_inbound_rx_returns_receiver() {
        // AC-2.1: take_inbound_rx() returns the mpsc::Receiver<InboundMessage>
        // that was created during AgentLoop construction.
        let (mut agent, _bus, _tmp) = build_test_agent("hello").await;
        let rx = agent.take_inbound_rx();
        assert!(rx.is_some(), "Expected a receiver on first call");
    }

    #[tokio::test]
    async fn take_inbound_rx_leaves_none_on_second_call() {
        // AC-2.2: After take_inbound_rx() is called once, inbound_rx is None.
        // A second call should return None (not panic).
        let (mut agent, _bus, _tmp) = build_test_agent("hello").await;
        let first = agent.take_inbound_rx();
        assert!(first.is_some(), "First call should return Some");
        let second = agent.take_inbound_rx();
        assert!(second.is_none(), "Second call should return None");
    }

    // ── run_with_rx ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn run_with_rx_processes_inbound_message() {
        // AC-2.3: run_with_rx(&self, rx) processes an InboundMessage sent through rx.
        // Sends one message, verifies the response is dispatched through the bus.
        let (mut agent, bus, _tmp) = build_test_agent("test response").await;

        // Take the outbound receiver before anything starts consuming it
        let mut outbound_rx = bus.take_outbound_rx().unwrap();

        // Extract inbound receiver from agent
        let inbound_rx = agent.take_inbound_rx().unwrap();

        // Wrap in Arc (no Mutex needed — this is the core of the refactor)
        let agent = Arc::new(agent);

        let agent_handle = {
            let a = Arc::clone(&agent);
            tokio::spawn(async move {
                let _ = a.run_with_rx(inbound_rx).await;
            })
        };

        // Send one message through the bus
        let msg = bus::InboundMessage::new("test", "user1", "test:session", "hello");
        bus.publish_inbound(msg).await.unwrap();

        // Wait for an outbound response
        let timeout = tokio::time::Duration::from_secs(5);
        let response = tokio::time::timeout(timeout, outbound_rx.recv())
            .await
            .expect("timed out waiting for outbound message")
            .expect("outbound channel closed unexpectedly");

        assert!(!response.content.is_empty(), "Response should not be empty");

        agent_handle.abort();
    }

    #[tokio::test]
    async fn run_with_rx_is_callable_on_shared_reference() {
        // AC-2.3: Verifies at compile time that run_with_rx takes &self.
        // Build an Arc<AgentLoop>, take the rx, then call run_with_rx on the Arc.
        // If this compiles, the &self signature is correct.
        let (mut agent, _bus, _tmp) = build_test_agent("hello").await;
        let rx = agent.take_inbound_rx().unwrap();
        let agent = Arc::new(agent);

        // run_with_rx is &self — call it on Arc<AgentLoop> without Mutex
        let handle = {
            let a = Arc::clone(&agent);
            tokio::spawn(async move {
                let _ = a.run_with_rx(rx).await;
            })
        };
        handle.abort();
    }

    // ── Backward compatibility: run(&mut self) ────────────────────────────────

    #[tokio::test]
    async fn run_backward_compat_delegates_to_run_with_rx() {
        // AC-2.4: The existing run(&mut self) method still compiles and works.
        // It should internally call take_inbound_rx() + run_with_rx().
        let (mut agent, bus, _tmp) = build_test_agent("backward compat response").await;
        let mut outbound_rx = bus.take_outbound_rx().unwrap();

        let handle = tokio::spawn(async move {
            // run() delegates to take_inbound_rx() + run_with_rx()
            let _ = agent.run().await;
        });

        let msg = bus::InboundMessage::new("test", "user1", "test:session", "hello");
        bus.publish_inbound(msg).await.unwrap();

        let timeout = tokio::time::Duration::from_secs(5);
        let response = tokio::time::timeout(timeout, outbound_rx.recv())
            .await
            .expect("timed out waiting for outbound message")
            .expect("outbound channel closed");

        assert!(!response.content.is_empty());
        handle.abort();
    }

    // ── process_direct_streaming on Arc<AgentLoop> ────────────────────────────

    #[tokio::test]
    async fn arc_agent_loop_process_direct_streaming() {
        // AC-2.5: process_direct_streaming() can be called on Arc<AgentLoop> directly,
        // without any Mutex wrapping.
        let (agent, _bus, _tmp) = build_test_agent("streaming hello").await;
        let agent = Arc::new(agent);

        // Call on Arc<AgentLoop> — must compile without Mutex
        let streaming_handle = agent
            .process_direct_streaming("hello".to_string(), "test:session".to_string())
            .await
            .expect("process_direct_streaming should succeed");

        let mut event_rx = streaming_handle.event_rx;
        let handle = streaming_handle.handle;

        let mut got_event = false;
        let timeout = tokio::time::Duration::from_secs(5);
        if let Ok(Some(_)) = tokio::time::timeout(timeout, event_rx.recv()).await {
            got_event = true;
        }

        assert!(got_event, "Should receive at least one AgentEvent");
        let _ = handle.await;
    }

    #[tokio::test]
    async fn process_direct_streaming_sends_done_event() {
        // AC-2.5: The StreamingHandle eventually delivers a Done event after processing.
        let (agent, _bus, _tmp) = build_test_agent("done response").await;
        let agent = Arc::new(agent);

        let streaming_handle = agent
            .process_direct_streaming("hello".to_string(), "test:session2".to_string())
            .await
            .expect("process_direct_streaming should succeed");

        let mut event_rx = streaming_handle.event_rx;
        let handle = streaming_handle.handle;

        let mut got_done = false;
        let timeout = tokio::time::Duration::from_secs(5);
        while let Ok(Some(event)) = tokio::time::timeout(timeout, event_rx.recv()).await {
            if matches!(event, AgentEvent::Done { .. }) {
                got_done = true;
                break;
            }
        }

        assert!(got_done, "Should receive a Done event");
        let _ = handle.await;
    }

    // ── Accessor methods ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn skill_manager_accessor_returns_skill_manager() {
        // AC-2.6: skill_manager() returns a reference to the SkillManager,
        // enabling /api/skills to list available skills without additional state.
        let (agent, _bus, _tmp) = build_test_agent("hello").await;
        let sm = agent.skill_manager();
        // SkillManager is always present; accessor must return a valid reference
        let _ = sm;
    }

    #[tokio::test]
    async fn tool_names_returns_registered_tools() {
        // AC-2.6: tool_names() returns a Vec<String> of registered tool names.
        // Should include tools registered unconditionally (no pool required).
        let (agent, _bus, _tmp) = build_test_agent("hello").await;
        let names = agent.tool_names().await;
        assert!(!names.is_empty(), "Should have at least some registered tools");
        assert!(
            names.iter().any(|n| n.contains("spawn") || n == "spawn"),
            "Should include spawn tool, got: {:?}",
            names
        );
    }
}
