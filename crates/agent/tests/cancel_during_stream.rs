//! Verifies that `call_provider_streaming` (consumed by `ExecutionCore`)
//! observes a `CancellationToken` between SSE chunks and returns promptly
//! with `CycleOutcome::Cancelled { partial_content }`, preserving any
//! partial content streamed before the cancel.
//!
//! Expected state BEFORE Task 3: the test times out because `run_cycle` has
//! no cancel awareness — `call_provider_streaming` blocks forever on the
//! pending stream and ignores the token.  The outer `tokio::time::timeout`
//! fires and the test FAILS (expected red bar).
//!
//! Expected state AFTER Task 3: `call_provider_streaming` selects on the
//! cancel token between chunks, returns a response with
//! `finish_reason == "cancelled"` and the partial content, `run_cycle`
//! wraps that as `CycleOutcome::Cancelled { partial_content: "partial-" }`,
//! and all assertions pass.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use providers::{
    CacheBreakpoint, ChatParams, DynProvider, LlmProvider, LlmResponse, LlmStream,
    LlmStreamChunk, Message, ProviderCapabilities,
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use agent::{CycleOutcome, ExecutionCore, ExecutionParams};

// ── Hung-stream provider ──────────────────────────────────────────────────────

/// A provider that streams one content chunk then blocks forever on the next
/// chunk until the stream is dropped.  Mirrors a slow / stuck Anthropic SSE
/// connection.
struct HungStreamProvider;

#[async_trait]
impl LlmProvider for HungStreamProvider {
    fn name(&self) -> &str {
        "hung-stream-provider"
    }

    fn default_model(&self) -> &str {
        "hung-stream-model"
    }

    async fn chat(
        &self,
        _msgs: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
        _bp: &[CacheBreakpoint],
    ) -> common::Result<LlmResponse> {
        unreachable!("test only exercises chat_stream path")
    }

    async fn chat_stream(
        &self,
        _msgs: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
        _bp: &[CacheBreakpoint],
    ) -> common::Result<LlmStream> {
        let chunk = LlmStreamChunk {
            content: Some("partial-".to_string()),
            reasoning_content: None,
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            usage: None,
        };

        let first = futures_util::stream::iter(vec![
            Ok::<_, common::KlyntbotError>(chunk),
        ]);
        // After the first chunk the stream pends forever — simulating a stalled
        // SSE connection that never emits a `[DONE]` frame.
        let pending = futures_util::stream::pending::<common::Result<LlmStreamChunk>>();
        Ok(Box::pin(first.chain(pending)))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Advertise streaming support so `run_cycle` uses `call_provider_streaming`.
        ProviderCapabilities {
            streaming: true,
            ..ProviderCapabilities::default()
        }
    }
}

// ── Test ─────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "current_thread")]
async fn cancel_during_stream_returns_partial_within_200ms() {
    let provider: DynProvider = Arc::new(HungStreamProvider);
    let registry = {
        use std::sync::Arc;
        use tokio::sync::RwLock;
        use tools::registry::ToolRegistry;
        Arc::new(RwLock::new(ToolRegistry::new()))
    };

    let core = ExecutionCore::new(provider, registry);

    let token = CancellationToken::new();
    let token_for_task = token.clone();

    let handle = tokio::spawn(async move {
        use std::collections::HashSet;
        use tools::RoutingContext;

        let params = ExecutionParams::new("hung-stream-model", 200_000)
            .with_cancel_token(token_for_task);

        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        let routing = RoutingContext::new("test".into(), "test".into());

        core.run_cycle(
            &mut vec![Message::user("hi")],
            &[],
            &params,
            &routing,
            Some(&tx),
            None::<&mut HashSet<String>>,
            &[],
        )
        .await
    });

    // Let the stream emit the first chunk and then block.
    tokio::time::sleep(Duration::from_millis(20)).await;
    token.cancel();

    let result = tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("run_cycle should return within 200ms of cancel — Task 3 not yet implemented?")
        .expect("spawned task should not panic")
        .expect("run_cycle should return Ok");

    // `run_cycle` returns `(CycleOutcome, Usage)`.
    let (outcome, _usage) = result;

    match outcome {
        CycleOutcome::Cancelled { partial_content, partial_reasoning: _ } => {
            assert_eq!(
                partial_content, "partial-",
                "partial content streamed before cancel must be preserved"
            );
        }
        other => panic!("Expected CycleOutcome::Cancelled, got {:?}", other),
    }
}
