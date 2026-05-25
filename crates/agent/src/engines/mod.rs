//! The execution seam.
//!
//! A single agentic turn is driven by [`execution::execute_loop`], a free
//! function taking `&ExecutionCore`. Three call sites invoke it: the main agent
//! ([`crate::agent_runtime::AgentRuntime::process_message`]) and the two subagent
//! entry points (`subagent::run_subagent_loop`, `run_subagent_task`).
//!
//! [`ExecutionEngine`] names that contract as a seam so it can be swapped without
//! editing the callers in place. The production adapter is [`CoreEngine`], which
//! owns an `Arc<ExecutionCore>` and delegates to `execute_loop` — behaviour is
//! identical to calling the function directly. A `MockEngine` test adapter
//! (added in Phase B) is what makes the seam load-bearing: it lets tests script
//! the result and the event stream that `execute_loop` would otherwise be the
//! only producer of.
//!
//! The loop body itself ([`execute_loop`]) and [`ExecutionCore`] stay deep and
//! unchanged — the seam sits *above* them. See ADR-0003 and
//! `docs/architecture/deep-dive-03-execution-engine-seam.md`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::events::AgentEvent;
use crate::execution::{execute_loop, ExecuteLoopResult, ExecutionCore, ExecutionParams, SafetyCap};
use tools::RoutingContext;

/// The contract for driving one agentic turn to completion.
///
/// Mirrors the shape of [`execute_loop`]: given the conversation so far, the tool
/// definitions, execution params, a mutable safety cap, the routing context, and
/// an optional event sink, run the LLM→tool loop until it terminates and return
/// the [`ExecuteLoopResult`].
///
/// **Contract:** an implementation must drop the `event_tx` sender before (or as)
/// `run` returns — consumers (e.g. the subagent event-forwarder in
/// `subagent::drive_subagent_with_progress`) await the channel closing to know the
/// turn is finished. An implementation that retains a `Sender` clone in a detached
/// task past `run`'s return would hang those consumers.
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn run(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        cap: &mut SafetyCap,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    ) -> common::Result<ExecuteLoopResult>;
}

/// Production adapter: delegates to [`execute_loop`] over an owned
/// `Arc<ExecutionCore>`. Constructing one and calling [`CoreEngine::run`] is
/// behaviourally identical to calling `execute_loop(&core, …)` directly.
pub struct CoreEngine {
    core: Arc<ExecutionCore>,
}

impl CoreEngine {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl ExecutionEngine for CoreEngine {
    async fn run(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        cap: &mut SafetyCap,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    ) -> common::Result<ExecuteLoopResult> {
        execute_loop(&self.core, messages, tools, params, cap, ctx, event_tx).await
    }
}

/// Test adapter: the second implementor that makes [`ExecutionEngine`] a real
/// seam. It records each call's `(messages.len(), tools.len())`, emits a scripted
/// sequence of [`AgentEvent`]s on `event_tx`, then returns a canned
/// [`ExecuteLoopResult`] — letting tests exercise the behaviour wrapped around the
/// loop (e.g. the subagent event-forwarder) without a live LLM.
#[cfg(test)]
pub(crate) struct MockEngine {
    scripted_events: Vec<AgentEvent>,
    result: ExecuteLoopResult,
    calls: std::sync::Arc<std::sync::Mutex<Vec<(usize, usize)>>>,
}

#[cfg(test)]
impl MockEngine {
    pub(crate) fn new(result: ExecuteLoopResult) -> Self {
        Self {
            scripted_events: Vec::new(),
            result,
            calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn with_events(mut self, events: Vec<AgentEvent>) -> Self {
        self.scripted_events = events;
        self
    }
}

#[cfg(test)]
#[async_trait]
impl ExecutionEngine for MockEngine {
    async fn run(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        _params: &ExecutionParams,
        _cap: &mut SafetyCap,
        _ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    ) -> common::Result<ExecuteLoopResult> {
        self.calls.lock().unwrap().push((messages.len(), tools.len()));
        if let Some(tx) = event_tx {
            for ev in &self.scripted_events {
                let _ = tx.send(ev.clone()).await;
            }
        }
        Ok(self.result.clone())
    }
}
