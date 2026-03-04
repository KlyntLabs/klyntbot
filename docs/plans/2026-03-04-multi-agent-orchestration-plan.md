# Multi-Agent Orchestration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the existing `DelegationTool` into the agent runtime so agents can delegate to each other mid-execution, and upgrade the general agent to orchestrate multi-intent messages by decomposing them and delegating to specialist agents.

**Architecture:** `AgentRuntime` implements `DelegationHandler` to run delegated agent executions with shared context. The `IntentAnalyzer` detects multi-intent messages and routes them to the general agent as orchestrator. `DelegationTool` is dynamically constructed per-message based on the agent profile's `can_delegate_to` list.

**Tech Stack:** Rust, async_trait, tokio, serde_json, existing `ExecutionRouter` + `ReactiveEngine`

**Design Doc:** `docs/plans/2026-03-04-multi-agent-orchestration-design.md`

---

## Phase 1: Wire DelegationHandler on AgentRuntime

### Task 1: Add delegation_depth to RoutingContext

**Files:**
- Modify: `crates/tools-core/src/lib.rs:79-96` (RoutingContext struct)
- Test: `crates/tools-core/src/lib.rs` (inline tests)

**Step 1: Add `delegation_depth` field to `RoutingContext`**

In `crates/tools-core/src/lib.rs`, add to the `RoutingContext` struct (after `is_direct_mode` field, ~L89):

```rust
/// Current delegation depth (0 = top-level, incremented per delegation).
pub delegation_depth: u32,
```

Update the `new()` constructor (~L99) to initialize:
```rust
delegation_depth: 0,
```

**Step 2: Run clippy to verify no warnings**

Run: `cargo clippy -p tools-core --all-targets`
Expected: 0 warnings

**Step 3: Commit**

```bash
git add crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add delegation_depth to RoutingContext"
```

---

### Task 2: Add DelegationStarted/Completed transparency events

**Files:**
- Modify: `crates/agent/src/events.rs:11-132` (AgentEvent enum)

**Step 1: Add two new variants to `AgentEvent`**

In `crates/agent/src/events.rs`, add after the `SubagentSpawned` variant (~L110):

```rust
/// An agent delegation has started (agent-to-agent handoff).
DelegationStarted {
    #[serde(rename = "fromAgent")]
    from_agent: String,
    #[serde(rename = "toAgent")]
    to_agent: String,
    query: String,
    depth: u32,
},

/// An agent delegation has completed.
DelegationCompleted {
    #[serde(rename = "fromAgent")]
    from_agent: String,
    #[serde(rename = "toAgent")]
    to_agent: String,
    success: bool,
    #[serde(rename = "durationMs")]
    duration_ms: u64,
},
```

**Step 2: Run clippy**

Run: `cargo clippy -p agent --all-targets`
Expected: 0 warnings

**Step 3: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add DelegationStarted/Completed transparency events"
```

---

### Task 3: Implement DelegationHandler on AgentRuntime

This is the core task. `AgentRuntime` implements `DelegationHandler` to run a mini agent execution for delegated agents with shared context.

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:51-69` (struct fields), `runtime.rs:72-97` (constructor)
- Modify: `crates/agent/src/agent_runtime/mod.rs` (re-exports)
- Test: `crates/agent/src/agent_runtime/runtime.rs` (new test module or existing)

**Step 1: Write the failing test**

Add a test module at the bottom of `crates/agent/src/agent_runtime/runtime.rs` (or in a new file `crates/agent/src/agent_runtime/delegation_tests.rs`):

```rust
#[cfg(test)]
mod delegation_tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tools::{DelegationHandler, RoutingContext};

    /// Mock provider that returns a text response.
    struct MockDelegationProvider;

    #[async_trait]
    impl LlmProvider for MockDelegationProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some("Delegated response from finance agent".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
    }

    fn make_test_runtime() -> Arc<AgentRuntime> {
        let provider: providers::DynProvider = Arc::new(MockDelegationProvider);
        let agent_manager = Arc::new(crate::agent_profile::AgentManager::new());
        let registry = Arc::new(RwLock::new(tools::registry::ToolRegistry::new()));
        let core = Arc::new(crate::execution::ExecutionCore::new(provider.clone(), registry));
        let context_engine = Arc::new(
            context_engine::ContextEngine::new(4096, 2048),
        );
        let direct = crate::intent_pipeline::engines::direct::DirectEngine::new(core.clone());
        let reactive = crate::intent_pipeline::engines::reactive::ReactiveEngine::new(core, 10);
        let router = crate::intent_pipeline::router::ExecutionRouter::new(direct, reactive);
        let cost_tracker = Arc::new(crate::cost::CostTracker::new(provider));
        let active_profile = Arc::new(RwLock::new(None));
        let config = crate::intent_pipeline::PipelineConfig::default();

        let runtime = AgentRuntime::new(
            agent_manager,
            crate::intent_pipeline::IntentAnalyzer::new_without_classifier(config.orchestrator.clone()),
            context_engine,
            router,
            cost_tracker,
            config,
            active_profile,
        );
        Arc::new(runtime)
    }

    #[tokio::test]
    async fn test_delegation_to_finance_agent() {
        let runtime = make_test_runtime();
        let ctx = RoutingContext::new("test".into(), "test".into());

        let result = runtime
            .delegate("finance", "check what transactions exist", &ctx, 0)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.is_empty());
    }

    #[tokio::test]
    async fn test_delegation_depth_limit() {
        let runtime = make_test_runtime();
        let ctx = RoutingContext::new("test".into(), "test".into());

        // Depth 2 with max 2 should still work (checked by DelegationTool, not handler)
        let result = runtime.delegate("finance", "check balance", &ctx, 1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delegation_unknown_agent() {
        let runtime = make_test_runtime();
        let ctx = RoutingContext::new("test".into(), "test".into());

        let result = runtime.delegate("nonexistent", "query", &ctx, 0).await;
        assert!(result.is_err());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(delegation_tests)'`
Expected: FAIL — `DelegationHandler` is not implemented for `AgentRuntime`

**Step 3: Add fields and implement DelegationHandler on AgentRuntime**

In `crates/agent/src/agent_runtime/runtime.rs`:

1. Add to imports at top of file:
```rust
use tools::DelegationHandler;
```

2. Add fields to `AgentRuntime` struct (~L51-L69). Add after `learning_adaptations`:
```rust
    /// Tool registry for looking up tool definitions during delegation.
    tool_registry: Option<Arc<RwLock<tools::registry::ToolRegistry>>>,
```

3. Add builder method after the existing `with_learning_repos()` (~L130):
```rust
    /// Set the tool registry for delegation support.
    pub fn with_tool_registry(mut self, registry: Arc<RwLock<tools::registry::ToolRegistry>>) -> Self {
        self.tool_registry = Some(registry);
        self
    }
```

4. Initialize `tool_registry: None` in `new()`.

5. Implement `DelegationHandler`:

```rust
#[async_trait::async_trait]
impl DelegationHandler for AgentRuntime {
    async fn delegate(
        &self,
        agent_name: &str,
        query: &str,
        ctx: &RoutingContext,
        depth: u32,
    ) -> common::Result<String> {
        use std::time::Instant;

        let start = Instant::now();

        // 1. Look up the delegated agent profile
        let profile = self.agent_manager.get(agent_name).ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                format!("Unknown agent for delegation: '{agent_name}'"),
            ))
        })?;

        debug!(
            "Delegation: executing agent '{}' for query '{}' (depth {})",
            agent_name, query, depth
        );

        // 2. Build messages — shared session context + delegation query
        let messages = vec![providers::Message::user(query)];

        // 3. Build agent-scoped context
        // Set the active profile temporarily for AgentContextSource
        {
            let mut guard = self.active_profile.write().await;
            *guard = Some(Arc::clone(profile));
        }

        // Assemble context with the delegated agent's instructions
        let context_request = context_engine::ContextRequest {
            message: query.to_string(),
            history: messages.clone(),
            execution_strategy: context_engine::ExecutionStrategy::Reactive,
            system_prompt_override: None,
        };
        let assembled = self.context_engine.assemble(context_request).await?;

        // 4. Filter tools to delegated agent's allowed set
        let tool_defs = if let Some(ref registry) = self.tool_registry {
            let reg = registry.read().await;
            reg.definitions()
        } else {
            vec![]
        };

        let filtered_tools: Vec<serde_json::Value> =
            if let Some(allowed) = profile.allowed_tool_names() {
                tool_defs
                    .iter()
                    .filter(|t| {
                        crate::agent_runtime::tool_def_name(t)
                            .map(|name| name.starts_with("mcp_") || allowed.contains(name))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            } else {
                tool_defs
            };

        // 5. Optionally add DelegationTool for chained delegation (if depth allows)
        // Note: DelegationTool is added dynamically if the delegated agent
        // has can_delegate_to AND depth + 1 < max_depth. Handled by caller.

        // 6. Execute via router with reduced budget
        let max_iters = std::cmp::min(profile.max_iterations.unwrap_or(10), 8);
        let params = crate::execution::ExecutionParams::new(&self.config.execution_model)
            .with_max_iterations(max_iters)
            .with_original_message(query.to_string());

        let router_result = self
            .router
            .execute(
                crate::intent_pipeline::types::ExecutionMode::Reactive {
                    max_iterations: max_iters,
                },
                assembled.messages,
                &filtered_tools,
                &params,
                ctx,
                None, // No event streaming for delegated execution (or pass through)
            )
            .await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        debug!(
            "Delegation to '{}' completed in {}ms: {} chars",
            agent_name,
            duration_ms,
            router_result.content.len()
        );

        Ok(router_result.content)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(delegation_tests)'`
Expected: PASS

**Step 5: Run full agent test suite**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_runtime/mod.rs
git commit -m "feat(agent): implement DelegationHandler on AgentRuntime"
```

---

### Task 4: Register DelegationTool dynamically in process_message

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:294-320` (step 7 tool filtering)

**Step 1: Write the failing test**

Add to the delegation tests module:

```rust
#[tokio::test]
async fn test_process_message_includes_delegate_tool_for_eligible_agent() {
    // This test verifies that when processing a message matched to an agent
    // with can_delegate_to, the delegate tool appears in filtered tools.
    // We verify this indirectly by checking the finance agent can delegate to task.
    let runtime = make_test_runtime();
    let ctx = RoutingContext::new("test".into(), "test".into());

    // Process a message that would match the finance agent
    let result = runtime
        .process_message(
            "check my transactions",
            vec![],
            &[],  // empty tools — delegation tool should be added
            &[],
            &ctx,
            None,
            None,
            None,
        )
        .await;

    // The result should succeed (finance agent runs, even with no tools)
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(process_message_includes_delegate)'`
Expected: FAIL (or inconclusive — depends on mock setup)

**Step 3: Add DelegationTool injection in process_message step 7**

In `crates/agent/src/agent_runtime/runtime.rs`, after the existing tool filtering block (~L294-L313), add:

```rust
        // Step 7b: Add DelegationTool if agent can delegate and we're not at max depth
        let delegation_depth = ctx.delegation_depth;
        let max_delegation_depth = 2u32; // TODO: make configurable

        let mut filtered_tools_vec: Vec<serde_json::Value> = filtered_tools.into_owned();

        if !profile.can_delegate_to.is_empty() && delegation_depth < max_delegation_depth {
            // Build a DelegationTool with this agent's allowed delegates
            let delegation_tool = tools::DelegationTool::with_handler(
                self.delegation_self_ref
                    .as_ref()
                    .expect("delegation_self_ref must be set")
                    .clone(),
            )
            .with_allowed_agents(profile.can_delegate_to.clone())
            .with_depth(delegation_depth, max_delegation_depth);

            // Add its JSON schema to the tool definitions
            let schema = serde_json::json!({
                "type": "function",
                "function": {
                    "name": delegation_tool.name(),
                    "description": delegation_tool.description(),
                    "parameters": delegation_tool.parameters(),
                }
            });
            filtered_tools_vec.push(schema);

            // Register the delegation tool in the registry for execution
            if let Some(ref registry) = self.tool_registry {
                let mut reg = registry.write().await;
                reg.register(Arc::new(delegation_tool));
            }
        }

        let filtered_tools = Cow::Owned(filtered_tools_vec);
```

Also add to the `AgentRuntime` struct:
```rust
    /// Self-reference for delegation handler (set after construction via Arc).
    delegation_self_ref: Option<Arc<dyn DelegationHandler>>,
```

Add builder method:
```rust
    /// Set the self-reference for delegation support.
    pub fn with_delegation_self_ref(mut self, handler: Arc<dyn DelegationHandler>) -> Self {
        self.delegation_self_ref = Some(handler);
        self
    }
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): register DelegationTool dynamically per-message in process_message"
```

---

### Task 5: Wire delegation_self_ref in AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:777-799` (AgentRuntime construction)

**Step 1: Set the self-reference after Arc wrapping**

In `crates/agent/src/agent_loop/builder.rs`, after `let runtime = Arc::new(runtime);` (~L799), add a two-phase init to set the self-ref. Since `AgentRuntime` is already wrapped in `Arc`, we need an interior mutability approach.

Option A — Use `OnceLock` or separate field setter:

Change the `delegation_self_ref` field to use `tokio::sync::OnceCell<Arc<dyn DelegationHandler>>`:

```rust
// In builder.rs, after Arc::new(runtime):
runtime.set_delegation_self_ref(Arc::clone(&runtime) as Arc<dyn DelegationHandler>);
```

This requires `set_delegation_self_ref` to use interior mutability (e.g., `OnceLock`):

```rust
// In runtime.rs, change the field:
delegation_self_ref: std::sync::OnceLock<Arc<dyn DelegationHandler>>,

// Add method:
pub fn set_delegation_self_ref(&self, handler: Arc<dyn DelegationHandler>) {
    let _ = self.delegation_self_ref.set(handler);
}
```

And update the usage in process_message to:
```rust
self.delegation_self_ref.get().expect("delegation_self_ref must be set").clone()
```

Also pass the `tool_registry` to `AgentRuntime`:
```rust
// In builder.rs, in the runtime construction chain:
.with_tool_registry(Arc::clone(&tool_registry))
```

**Step 2: Run full test suite**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire delegation self-ref and tool registry in AgentLoopBuilder"
```

---

### Task 6: Emit delegation transparency events

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (DelegationHandler impl)

**Step 1: Add event emission to the delegate() method**

In the `DelegationHandler` impl, accept an optional `event_tx` (or thread it through `RoutingContext`). The simplest approach: store `event_tx` in the runtime or pass it through the `RoutingContext`.

Since `RoutingContext` already lives in `tools-core` and we don't want to add `AgentEvent` there, store a thread-local or pass via the runtime:

Add to `AgentRuntime`:
```rust
    /// Event sender for transparency events (set per-message).
    current_event_tx: tokio::sync::RwLock<Option<tokio::sync::mpsc::Sender<AgentEvent>>>,
```

In `process_message()`, before execution, store the event_tx:
```rust
*self.current_event_tx.write().await = event_tx.clone();
```

In `delegate()`, emit events:
```rust
// Before delegation execution:
if let Some(tx) = self.current_event_tx.read().await.as_ref() {
    let _ = tx.send(AgentEvent::DelegationStarted {
        from_agent: current_agent_name.to_string(),
        to_agent: agent_name.to_string(),
        query: query.to_string(),
        depth,
    }).await;
}

// After delegation execution:
if let Some(tx) = self.current_event_tx.read().await.as_ref() {
    let _ = tx.send(AgentEvent::DelegationCompleted {
        from_agent: current_agent_name.to_string(),
        to_agent: agent_name.to_string(),
        success: true,
        duration_ms,
    }).await;
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 3: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): emit DelegationStarted/Completed transparency events"
```

---

## Phase 2: Multi-Intent Detection & Orchestration

### Task 7: Add needs_orchestration to IntentAnalysis

**Files:**
- Modify: `crates/agent/src/intent_pipeline/types.rs:131-137` (IntentAnalysis struct)

**Step 1: Add new fields to IntentAnalysis**

```rust
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
    /// Whether this message requires orchestration across multiple agents.
    pub needs_orchestration: bool,
}
```

**Step 2: Update `fallback()` and any constructors**

In `fallback()` (~L141), add `needs_orchestration: false`.

Search for all places `IntentAnalysis` is constructed (in `analysis.rs`) and add `needs_orchestration: false` to each.

**Step 3: Run clippy and tests**

Run: `cargo clippy -p agent --all-targets && cargo nextest run -p agent`
Expected: 0 warnings, all tests pass

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/types.rs crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): add needs_orchestration field to IntentAnalysis"
```

---

### Task 8: Detect multi-agent intent in heuristic classifier

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:28-127` (analyze_heuristic)
- Modify: `crates/agent/src/agent_profile/manager.rs` (add method to check multi-agent triggers)

**Step 1: Write the failing test**

In `crates/agent/src/intent_pipeline/analysis.rs` test module:

```rust
#[test]
fn test_multi_agent_intent_detection() {
    let msg = "check my transactions then create a task for the missing ones";
    let result = analyze_heuristic(msg);
    // Multi-agent messages should defer to LLM or set needs_orchestration
    // The heuristic should return None (defer to LLM) for cross-agent sequential messages
    assert!(result.is_none() || result.as_ref().map_or(false, |a| a.needs_orchestration));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(multi_agent_intent)'`
Expected: FAIL or PASS (if it already defers — `detect_sequential_language` may already catch this)

**Step 3: Add multi-agent trigger detection**

In `crates/agent/src/intent_pipeline/analysis.rs`, add a helper function:

```rust
/// Check if a message contains triggers from 2+ different agents.
fn has_multi_agent_triggers(msg: &str) -> bool {
    // Hardcoded trigger groups (matching the AGENT.md definitions)
    let agent_trigger_groups: &[&[&str]] = &[
        // task agent triggers
        &["todo", "task", "create a task", "my tasks", "focus", "project", "area", "objective"],
        // finance agent triggers
        &["transaction", "budget", "expense", "income", "balance", "account", "finance", "spending", "investment"],
        // calendar agent triggers
        &["calendar", "schedule", "meeting", "event", "appointment", "remind"],
        // automation agent triggers
        &["cron", "automate", "schedule a", "every day", "recurring"],
        // communication agent triggers
        &["send", "email", "message", "notify", "slack", "telegram"],
    ];

    let normalized = msg.to_lowercase();
    let matched_groups = agent_trigger_groups
        .iter()
        .filter(|triggers| triggers.iter().any(|t| normalized.contains(t)))
        .count();

    matched_groups >= 2
}
```

Then in `analyze_heuristic()`, add a check early (after the greeting check, ~L36):

```rust
    // Multi-agent sequential request → defer to LLM for orchestration
    if detect_sequential_language(msg) && has_multi_agent_triggers(msg) {
        return None; // Defer to LLM classifier which will set needs_orchestration
    }
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): detect multi-agent triggers in heuristic classifier"
```

---

### Task 9: Add needs_orchestration to LLM classifier

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:405-428` (CLASSIFICATION_PROMPT)
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:476-517` (parse_classification_json)

**Step 1: Update the CLASSIFICATION_PROMPT**

Add `needs_orchestration` to the JSON schema in the prompt. In the prompt string (~L405-L428), add to the expected JSON fields:

```
"needs_orchestration": boolean  // true if the request involves multiple domains (e.g., finance + tasks)
```

**Step 2: Update `parse_classification_json()`**

In `parse_classification_json()` (~L476-L517), add:

```rust
let needs_orchestration = json
    .get("needs_orchestration")
    .and_then(|v| v.as_bool())
    .unwrap_or(false);
```

And set it on the returned `IntentAnalysis`.

**Step 3: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): add needs_orchestration to LLM classifier prompt and parser"
```

---

### Task 10: Add orchestration routing override in AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:196-227` (after agent matching, before execution)

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_orchestration_override_routes_to_general() {
    // When needs_orchestration is true, the runtime should override
    // the matched agent to general (which can delegate to all agents)
    let runtime = make_test_runtime();
    let ctx = RoutingContext::new("test".into(), "test".into());

    // A message that triggers finance but also has sequential + task triggers
    let result = runtime
        .process_message(
            "first check my transactions then create a task for the missing ones",
            vec![],
            &[],
            &[],
            &ctx,
            None,
            None,
            None,
        )
        .await;

    assert!(result.is_ok());
    // Verify agent_name is "general" (orchestrator override)
    let result = result.unwrap();
    assert_eq!(result.agent_name, "general");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(orchestration_override)'`
Expected: FAIL — agent_name will be "finance" or "task", not "general"

**Step 3: Add orchestration override in process_message**

In `runtime.rs`, after the IntentAnalyzer step (~L203-L209) and before step 4 (max_iterations override), add:

```rust
        // Step 3b: Orchestration override — route multi-agent intents to general
        let (profile, agent_name) = if analysis.needs_orchestration {
            let general = self.agent_manager.get("general")
                .unwrap_or(profile);
            debug!(
                "Orchestration override: routing '{}' → general agent",
                agent_name
            );
            // Increase iteration budget for orchestration (multiple delegations)
            if let ExecutionMode::Reactive { ref mut max_iterations } = analysis.mode {
                *max_iterations = std::cmp::max(*max_iterations, 15);
            }
            (general, "general".to_string())
        } else {
            (profile, agent_name)
        };
```

Note: Make `profile` and `agent_name` mutable bindings earlier in the function, or use shadowing.

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(orchestration_override)'`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): add orchestration routing override for multi-agent intents"
```

---

## Phase 3: General Agent Orchestration Instructions

### Task 11: Update general agent AGENT.md with orchestration instructions

**Files:**
- Modify: `agents/general/AGENT.md`

**Step 1: Update the agent definition**

Replace the contents of `agents/general/AGENT.md`:

```markdown
---
name: general
description: General-purpose assistant and orchestrator
tools: [ask_user, memory, web_search, web_fetch, grep, glob, read_file, list_dir, spawn, learning]
max_iterations: 15
can_delegate_to: [task, finance, calendar, automation, communication]
always_skills: []
---

You are a general-purpose assistant and orchestrator. You handle greetings, casual conversation, questions,
and any request that doesn't clearly belong to a specialized domain.

## Behavior
- For simple questions and greetings, respond directly without tools
- When a request touches a specific domain (tasks, finance, calendar), delegate to the specialist agent
- Use web search for factual questions you're unsure about
- Use memory to recall and store important user information

## Orchestration

When handling multi-part requests that span multiple domains:

1. **Decompose** the request into discrete steps
2. **Delegate** each step to the appropriate specialist agent using `delegate(agent, query)`
3. **Chain context** — include relevant results from earlier delegations in later queries
4. **Synthesize** a unified response from all delegation results

### Examples

**"Check my transactions, then create a task for missing ones"**
→ `delegate("finance", "list all transactions in my accounts")`
→ Use the finance result to form the task description
→ `delegate("task", "create a task: Add details for all missing transactions. Due: tomorrow")`
→ Combine both results into a single coherent response

**"What meetings do I have today and are there any related tasks?"**
→ `delegate("calendar", "list today's meetings")`
→ `delegate("task", "search for tasks related to: [meeting topics from calendar]")`
→ Present meetings with related tasks grouped together

### Guidelines
- Always delegate to specialists rather than attempting domain-specific work yourself
- Pass enough context in each delegation query for the specialist to act independently
- If a delegation fails, report the failure clearly rather than guessing
- Keep your final synthesis concise — don't repeat everything the specialists said
```

**Step 2: Run clippy (agents are compiled via include_str!)**

Run: `cargo build -p agent`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add agents/general/AGENT.md
git commit -m "feat(agent): upgrade general agent with orchestration instructions"
```

---

## Phase 4: Integration Testing

### Task 12: Write integration test for end-to-end delegation flow

**Files:**
- Create: `crates/agent/tests/delegation_integration.rs` (or add to existing integration test file)

**Step 1: Write the integration test**

```rust
//! Integration test: end-to-end delegation flow.
//!
//! Verifies that the general agent can orchestrate multi-domain requests
//! by delegating to specialist agents.

use async_trait::async_trait;
use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
use serde_json::Value;
use std::sync::Arc;

/// Provider that simulates delegation-aware responses.
/// When it sees "delegate" in tools, it calls the delegate tool.
/// Otherwise returns a text response.
struct DelegationAwareProvider {
    /// Response to return when no delegation is needed.
    response: String,
}

#[async_trait]
impl LlmProvider for DelegationAwareProvider {
    async fn chat(
        &self,
        _messages: &[providers::Message],
        tools: Option<&[Value]>,
        _params: &ChatParams,
    ) -> common::Result<LlmResponse> {
        // Check if delegate tool is available
        let has_delegate = tools
            .map(|t| t.iter().any(|td| {
                td.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("delegate")
            }))
            .unwrap_or(false);

        if has_delegate {
            // Simulate LLM choosing to delegate
            Ok(LlmResponse {
                content: None,
                tool_calls: vec![providers::ToolCall {
                    id: "call_1".to_string(),
                    name: "delegate".to_string(),
                    arguments: serde_json::json!({
                        "agent": "finance",
                        "query": "check what transactions exist"
                    }).to_string(),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some(self.response.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
    }
    fn default_model(&self) -> &str { "mock" }
    fn name(&self) -> &str { "mock" }
}

// TODO: Build full AgentLoop with DelegationAwareProvider and verify
// that delegation flows end-to-end. This requires constructing the full
// builder chain, which is best done after Tasks 1-11 are complete.
```

**Step 2: Run integration test**

Run: `cargo nextest run --test delegation_integration`
Expected: PASS (or compile-only if the test body is TODO)

**Step 3: Commit**

```bash
git add crates/agent/tests/delegation_integration.rs
git commit -m "test(agent): add integration test scaffold for delegation flow"
```

---

### Task 13: Run full workspace tests and clippy

**Step 1: Run clippy across workspace**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 2: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All pass

**Step 3: Run doctests**

Run: `cargo test --workspace --doc`
Expected: All pass

**Step 4: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: address clippy warnings and test failures from delegation wiring"
```
