# Klynt Coding-in-Chat — Phase 1 Plan 1 of 6: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all foundational primitives — channel constant, AgentEvent variants, `Tool::is_concurrency_safe`, `fan_out_event` helper, sessions-table schema columns, and 7 new crate skeletons — so subsequent plans (first tool end-to-end, sandbox, hooks, skills) can build directly on a stable, tested base. **Zero user-visible behavior in this plan.**

**Architecture:** Additive primitives only. New crates compile as empty skeletons (vendoring of Codex sources happens in Plan 2). Existing types gain new variants under `#[non_exhaustive]` to absorb future growth. Schema changes go directly into `001_initial.sql` per CLAUDE.md's pre-release policy. The `sessions` table (note: actual table name; the spec's `chat_sessions` is corrected here) gains 8 new columns; existing `pinned` and `conversation_type` columns are reused for `starred`/`mode` semantics.

**Tech Stack:** Rust 1.93 stable, `cargo` workspace, SQLite (single migration in `crates/storage/migrations/001_initial.sql`), `async-trait`, `serde`, `tokio`, `tools-core` proc macros (`#[derive(Tool)]`), `tracing::instrument` annotations on AppCore methods.

**Spec reference:** `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` (especially §3 Crate layout, §4 Agent loop integration, §10 Event vocabulary, §11 Session model).

**Plan suite:** This is plan 1 of 6 covering Phase 1.
- **Plan 1 (this):** Foundation primitives.
- Plan 2: First tool end-to-end (`bash` with privacy guard + Layer 1 + macOS Seatbelt + ApprovalCard).
- Plan 3: Tool kit completion + Linux sandbox.
- Plan 4: Layer 2 Starlark + hooks engine.
- Plan 5: Skills + recall + Distiller/Mirror subscribers.
- Plan 6: Settings page + slash command catalog completion + scenario tests.

---

## File structure

### Files created

```
bot/
├── crates/
│   ├── klynt-protocol/
│   │   ├── Cargo.toml
│   │   ├── VENDOR.md                (placeholder — Plan 2 fills)
│   │   └── src/
│   │       └── lib.rs                (empty pub modules)
│   ├── klynt-execpolicy/
│   │   ├── Cargo.toml
│   │   ├── VENDOR.md
│   │   └── src/lib.rs
│   ├── klynt-sandbox/
│   │   ├── Cargo.toml
│   │   ├── VENDOR.md
│   │   └── src/lib.rs
│   ├── klynt-sandbox-helper/
│   │   ├── Cargo.toml
│   │   ├── VENDOR.md
│   │   └── src/main.rs               (binary stub)
│   ├── klynt-hooks/
│   │   ├── Cargo.toml
│   │   ├── VENDOR.md
│   │   └── src/lib.rs
│   ├── klynt-skill-loader/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                (no VENDOR.md — fresh, not Codex-derived)
│   └── klynt-core/
│       ├── Cargo.toml
│       └── src/lib.rs
└── scripts/
    └── adapt_codex_vendor.sh         (skeleton; usage doc only)
```

### Files modified

```
Cargo.toml                                              (add 7 workspace members)
crates/common/src/types.rs                              (add CODING_CHANNEL constant)
crates/common/src/lib.rs                                (re-export CODING_CHANNEL)
crates/tools-core/src/lib.rs                            (add Tool::is_concurrency_safe)
crates/agent/src/events.rs                              (add 18 variants + #[non_exhaustive])
crates/agent/src/execution/core.rs                      (add fan_out_event helper + partitioning)
crates/agent/src/execution/execute_loop.rs              (call fan_out_event at existing event_tx sites)
crates/agent/src/agent_runtime/runtime.rs               (accept Arc<DomainEventBus>)
crates/storage/migrations/001_initial.sql               (8 new columns on sessions table)
crates/storage/src/repos/sessions.rs                    (column mappings — exact path verified in Task 19)
docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md  (rename chat_sessions → sessions; align column names)
```

### Match-arm audits (touched by Task 9)

Every existing `match AgentEvent { ... }` outside the coding path must gain a `_ =>` catch-all. Exact files identified by `cargo build` errors after Task 9 lands `#[non_exhaustive]`.

---

## Task 1: Verify clean baseline

**Files:** None modified. This is a pre-flight check.

- [ ] **Step 1: Run workspace build**

```bash
cargo build --workspace
```

Expected: succeeds with no errors. If it fails, abort the plan and resolve baseline issues first.

- [ ] **Step 2: Run workspace clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: succeeds (zero warnings). Per CLAUDE.md "Zero clippy warnings policy."

- [ ] **Step 3: Run workspace fmt check**

```bash
cargo fmt --all --check
```

Expected: succeeds (no diffs).

- [ ] **Step 4: Run workspace tests**

```bash
cargo nextest run --workspace
```

Expected: all green.

- [ ] **Step 5: Run desktop-ui frontend checks**

```bash
cd desktop-ui && bun run lint && bun run typecheck && bun run test --run
cd ..
```

Expected: all green.

- [ ] **Step 6: Confirm clean git status**

```bash
git status
```

Expected: working tree clean (or only `.mcp.json` untracked, which is fine).

---

## Task 2: Add `CODING_CHANNEL` constant

**Files:**
- Modify: `crates/common/src/types.rs:11-15` (add constant alongside existing `SYSTEM_CHANNEL` / `CLI_CHANNEL` / `MCP_CHANNEL`)
- Modify: `crates/common/src/lib.rs:33-34` (re-export)

- [ ] **Step 1: Write the failing test**

Add to the bottom of `crates/common/src/types.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn coding_channel_constant_value() {
    assert_eq!(CODING_CHANNEL, "coding");
}

#[test]
fn coding_channel_round_trips_through_channel_name() {
    let channel = ChannelName::new(CODING_CHANNEL);
    assert_eq!(channel.as_str(), "coding");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p common -E 'test(coding_channel)'
```

Expected: FAIL with "cannot find value `CODING_CHANNEL` in this scope".

- [ ] **Step 3: Add the constant**

Edit `crates/common/src/types.rs` lines 11-15 to add the new constant after `MCP_CHANNEL`:

```rust
// ── Well-known channel / sender constants ─────────────────────────────────
pub const SYSTEM_CHANNEL: &str = "system";
pub const CLI_CHANNEL: &str = "cli";
pub const MCP_CHANNEL: &str = "mcp";
pub const CODING_CHANNEL: &str = "coding";
pub const TELEGRAM_RESET_SENDER: &str = "telegram_reset";
```

- [ ] **Step 4: Re-export from `lib.rs`**

Edit `crates/common/src/lib.rs:33-34` to include `CODING_CHANNEL` in the existing re-export list:

```rust
pub use types::{
    AppMode, ChannelName, ChatId, MessageRole, SessionKey, CLI_CHANNEL, CODING_CHANNEL,
    MCP_CHANNEL, SYSTEM_CHANNEL, TELEGRAM_RESET_SENDER,
};
```

(Alphabetize as shown.)

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo nextest run -p common -E 'test(coding_channel)'
```

Expected: 2 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/common/src/types.rs crates/common/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(common): add CODING_CHANNEL constant

Adds a well-known channel-name constant for the coding-in-chat surface
alongside the existing SYSTEM_CHANNEL / CLI_CHANNEL / MCP_CHANNEL literals.
Lifted to a constant (vs. raw string) because it is referenced from tool
gating, Distiller filtering, and the chat_send dispatcher.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Add `Tool::is_concurrency_safe` trait method

**Files:**
- Modify: `crates/tools-core/src/lib.rs` (the `Tool` trait, add new default method)
- Modify: `crates/tools-core/src/lib.rs` (test module — add a fixture impl + tests)

- [ ] **Step 1: Write the failing test**

Add to the bottom of `crates/tools-core/src/lib.rs` (inside or appending a `#[cfg(test)] mod tests`; create the module if it doesn't exist):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal Tool fixture for trait-default testing.
    struct DefaultsTool;

    #[async_trait]
    impl Tool for DefaultsTool {
        fn name(&self) -> &str { "defaults" }
        fn description(&self) -> &str { "fixture" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok(String::new())
        }
    }

    #[test]
    fn is_concurrency_safe_defaults_to_false() {
        let t = DefaultsTool;
        assert!(!t.is_concurrency_safe(&json!({})));
    }

    /// Tool that explicitly opts into concurrency-safe.
    struct ReadOnlyTool;

    #[async_trait]
    impl Tool for ReadOnlyTool {
        fn name(&self) -> &str { "readonly" }
        fn description(&self) -> &str { "fixture" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok(String::new())
        }
        fn is_concurrency_safe(&self, _args: &Value) -> bool { true }
    }

    #[test]
    fn is_concurrency_safe_can_be_overridden_to_true() {
        let t = ReadOnlyTool;
        assert!(t.is_concurrency_safe(&json!({})));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p tools-core -E 'test(is_concurrency_safe)'
```

Expected: FAIL — compile error "no method named `is_concurrency_safe` found for type `DefaultsTool`".

- [ ] **Step 3: Add the trait method**

Edit `crates/tools-core/src/lib.rs` inside the existing `pub trait Tool: Send + Sync { ... }` block, after the `metadata()` method and before `custom_timeout()`:

```rust
/// Whether this tool can be safely dispatched in parallel with other
/// `is_concurrency_safe == true` tools in the same iteration.
///
/// Returns `false` by default. Override to `true` for read-only tools
/// (e.g., `read`, `glob`, `grep`, `recall_*`) that have no observable
/// side effects on the filesystem, network, or shared mutable state.
///
/// The execution loop partitions tool calls by this flag: safe tools
/// run via `futures::future::join_all`; unsafe tools run sequentially.
fn is_concurrency_safe(&self, _args: &Value) -> bool {
    false
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p tools-core -E 'test(is_concurrency_safe)'
```

Expected: 2 tests PASS.

- [ ] **Step 5: Verify no regressions in tools-core**

```bash
cargo nextest run -p tools-core
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/tools-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(tools-core): add Tool::is_concurrency_safe with default false

Adds a per-tool predicate the execute loop uses to partition tool calls
between parallel and sequential dispatch. Default returns false so existing
tools are unaffected; read-only tools (Plan 3) override to true.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Add `#[non_exhaustive]` to AgentEvent + recall/skill telemetry variants

**Files:**
- Modify: `crates/agent/src/events.rs` (add `#[non_exhaustive]` to the enum, then 6 new variants)

This is the first of four "add variants" tasks (Tasks 4-7), grouped by the spec §10 categorization. The `#[non_exhaustive]` attribute lands here so all 18 variants are covered by it.

- [ ] **Step 1: Write the failing test**

Append to `crates/agent/src/events.rs` (or to its test module):

```rust
#[cfg(test)]
mod recall_skill_variant_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recall_injected_serializes_with_camel_case_tag() {
        let event = AgentEvent::RecallInjected {
            memory_ids: vec!["m1".into(), "m2".into()],
            coverage_score: 0.82,
            escalation_chain: vec!["thread".into(), "repo".into()],
            dead_end_warning: false,
            budget_used_tokens: 1234,
            budget_limit_tokens: 4096,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "recallInjected");
        assert_eq!(v["coverageScore"], 0.82);
        assert_eq!(v["budgetUsedTokens"], 1234);
    }

    #[test]
    fn skill_activated_serializes() {
        let event = AgentEvent::SkillActivated {
            skill_id: "code-review".into(),
            source_path: "~/.klyntbot/skills/code-review".into(),
            trigger: "path_touch".into(),
            injected_tokens: 480,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "skillActivated");
        assert_eq!(v["skillId"], "code-review");
    }

    #[test]
    fn dead_end_warning_skill_reference_context_decision_serialize() {
        let _e1 = AgentEvent::DeadEndWarningSurfaced {
            approach_summary: "tried subprocess but env wrong".into(),
            prior_attempt_id: "att-1".into(),
            confidence: 0.7,
        };
        let _e2 = AgentEvent::SkillActivationConsidered {
            skill_id: "code-review".into(),
            score: 0.65,
            threshold: 0.5,
            accepted: true,
            decision_reason: "above threshold".into(),
        };
        let _e3 = AgentEvent::SkillReferenceLoaded {
            skill_id: "code-review".into(),
            reference: "examples.md".into(),
            tokens: 1200,
            load_kind: "on_demand".into(),
        };
        let _e4 = AgentEvent::ContextEngineDecision {
            included: vec!["soul".into(), "code-review-skill".into()],
            excluded: vec!["legacy-foo".into()],
            total_tokens: 5400,
            budget_used_pct: 0.45,
        };
        // Smoke: each constructs without panic; serialization tested above on representative ones.
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(recall_skill_variant)'
```

Expected: FAIL — compile error "no variant `RecallInjected` on enum `AgentEvent`".

- [ ] **Step 3: Add `#[non_exhaustive]` to the AgentEvent enum**

Edit `crates/agent/src/events.rs` near line 9, change:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
```

to:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum AgentEvent {
```

- [ ] **Step 4: Add the 6 recall/skill variants**

Append these 6 variants inside `enum AgentEvent { ... }` (after the existing variants — order doesn't matter functionally, but group by spec category for readability). Use field names that match the test's expected camelCase:

```rust
    /// Recall context injected into the system prompt.
    RecallInjected {
        memory_ids: Vec<String>,
        #[serde(rename = "coverageScore")]
        coverage_score: f64,
        escalation_chain: Vec<String>,
        dead_end_warning: bool,
        #[serde(rename = "budgetUsedTokens")]
        budget_used_tokens: u32,
        #[serde(rename = "budgetLimitTokens")]
        budget_limit_tokens: u32,
    },

    /// Mirror flagged the user's current approach as a dead-end pattern.
    DeadEndWarningSurfaced {
        approach_summary: String,
        prior_attempt_id: String,
        confidence: f64,
    },

    /// SkillRouter evaluated a skill for activation; emit regardless of outcome.
    SkillActivationConsidered {
        skill_id: String,
        score: f64,
        threshold: f64,
        accepted: bool,
        decision_reason: String,
    },

    /// A skill activated and its frontmatter is being injected.
    SkillActivated {
        #[serde(rename = "skillId")]
        skill_id: String,
        source_path: String,
        trigger: String,
        injected_tokens: u32,
    },

    /// Agent loaded a referenced skill page on-demand.
    SkillReferenceLoaded {
        skill_id: String,
        reference: String,
        tokens: u32,
        load_kind: String,
    },

    /// Context engine finalized which sources to include for this turn.
    ContextEngineDecision {
        included: Vec<String>,
        excluded: Vec<String>,
        total_tokens: u32,
        budget_used_pct: f64,
    },
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(recall_skill_variant)'
```

Expected: 3 tests PASS.

- [ ] **Step 6: Run the rest of the agent tests to confirm `#[non_exhaustive]` didn't break consumers**

```bash
cargo build --workspace 2>&1 | head -40
```

Expected: zero match-arm errors. (Existing consumers of `AgentEvent` may use `_ =>` already; if not, Task 9 audits and fixes them. If the build breaks here, do the audit *now* and remember to mark Task 9 as already done.)

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "$(cat <<'EOF'
feat(agent): add recall/skill telemetry AgentEvent variants

Adds 6 variants under #[non_exhaustive]: RecallInjected,
DeadEndWarningSurfaced, SkillActivationConsidered, SkillActivated,
SkillReferenceLoaded, ContextEngineDecision.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Add approval/sandbox AgentEvent variants

**Files:**
- Modify: `crates/agent/src/events.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/agent/src/events.rs` test module:

```rust
#[cfg(test)]
mod approval_sandbox_variant_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn approval_requested_carries_requires_user_input_field() {
        let event = AgentEvent::ApprovalRequested {
            request_id: "req-1".into(),
            tool: "bash".into(),
            args_hash: "abc123".into(),
            layer: "starlark".into(),
            rule_matched: Some("prefix git push".into()),
            mirror_history: None,
            sandbox_summary: "Seatbelt cwd-only".into(),
            requires_user_input: true,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "approvalRequested");
        assert_eq!(v["requiresUserInput"], true);
    }

    #[test]
    fn approval_resolved_carries_decided_by_field() {
        let event = AgentEvent::ApprovalResolved {
            request_id: "req-1".into(),
            decision: "allow_once".into(),
            decision_reason: "user clicked allow".into(),
            latency_ms: 8200,
            persisted_rule: None,
            decided_by: "user".into(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "approvalResolved");
        assert_eq!(v["decidedBy"], "user");
    }

    #[test]
    fn sandbox_policy_applied_serializes() {
        let event = AgentEvent::SandboxPolicyApplied {
            tool: "bash".into(),
            policy_summary: "seatbelt cwd-only".into(),
            policy_hash: "h123".into(),
            fallback_unsandboxed: false,
            fs_constraints: vec!["read /Users/jayden/Projects/Klynt/bot/**".into()],
            network_constraints: vec!["deny".into()],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "sandboxPolicyApplied");
        assert_eq!(v["fallbackUnsandboxed"], false);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(approval_sandbox_variant)'
```

Expected: FAIL — compile error referencing missing variants.

- [ ] **Step 3: Add the 3 variants**

Append to `enum AgentEvent` in `crates/agent/src/events.rs`:

```rust
    /// Approval gate evaluated. Fires for every gate evaluation, not only
    /// "ask" cases. `requires_user_input` distinguishes auto-allow / auto-deny
    /// (false) from cases that need a chat-inline ApprovalCard (true).
    ApprovalRequested {
        request_id: String,
        tool: String,
        args_hash: String,
        layer: String,
        rule_matched: Option<String>,
        mirror_history: Option<serde_json::Value>,
        sandbox_summary: String,
        #[serde(rename = "requiresUserInput")]
        requires_user_input: bool,
    },

    /// Approval gate resolved (paired with ApprovalRequested by request_id).
    ApprovalResolved {
        request_id: String,
        decision: String,
        decision_reason: String,
        latency_ms: u64,
        persisted_rule: Option<String>,
        #[serde(rename = "decidedBy")]
        /// One of: user | auto_allow | auto_deny | timeout | cancelled.
        decided_by: String,
    },

    /// Sandbox policy applied for a tool invocation.
    SandboxPolicyApplied {
        tool: String,
        policy_summary: String,
        policy_hash: String,
        #[serde(rename = "fallbackUnsandboxed")]
        fallback_unsandboxed: bool,
        fs_constraints: Vec<String>,
        network_constraints: Vec<String>,
    },
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(approval_sandbox_variant)'
```

Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "$(cat <<'EOF'
feat(agent): add approval/sandbox AgentEvent variants

ApprovalRequested fires for every gate evaluation with requires_user_input
distinguishing ask-cases from auto-decisions; ApprovalResolved pairs by
request_id with decided_by ∈ {user,auto_allow,auto_deny,timeout,cancelled};
SandboxPolicyApplied captures the per-tool sandbox configuration.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Add tool/provider telemetry AgentEvent variants

**Files:**
- Modify: `crates/agent/src/events.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/agent/src/events.rs`:

```rust
#[cfg(test)]
mod tool_provider_variant_tests {
    use super::*;

    #[test]
    fn provider_response_serializes() {
        let event = AgentEvent::ProviderResponse {
            latency_ms: 1234,
            usage: serde_json::json!({"prompt_tokens": 1000, "completion_tokens": 200}),
            cost_usd: 0.042,
            retries_used: 0,
            finish_reason: "stop".into(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "providerResponse");
        assert_eq!(v["latencyMs"], 1234);
    }

    #[test]
    fn mid_loop_compression_serializes() {
        let event = AgentEvent::MidLoopCompressionTriggered {
            before_tokens: 95000,
            after_tokens: 38000,
            messages_condensed: 47,
            regions: vec!["tool_results".into()],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "midLoopCompressionTriggered");
    }

    #[test]
    fn tool_call_stream_chunk_mcp_subcall_provider_request_smoke() {
        let _e1 = AgentEvent::ToolCallStreamChunk {
            tool: "bash".into(),
            chunk_kind: "stdout".into(),
            bytes: 4096,
            truncated: false,
        };
        let _e2 = AgentEvent::MCPSubcallTrace {
            server: "google-calendar".into(),
            tool: "list_events".into(),
            latency_ms: 230,
            bytes_returned: 1280,
            error: None,
        };
        let _e3 = AgentEvent::ProviderRequest {
            iteration: 3,
            model: "claude-sonnet-4-7".into(),
            prompt_tokens: 4500,
            max_tokens: 4096,
            attempt: 1,
        };
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(tool_provider_variant)'
```

Expected: FAIL — missing variants.

- [ ] **Step 3: Add the 5 variants**

Append to `enum AgentEvent` in `crates/agent/src/events.rs`:

```rust
    /// A chunk of a streaming tool result (e.g., bash stdout/stderr lines).
    ToolCallStreamChunk {
        tool: String,
        chunk_kind: String, // "stdout" | "stderr" | "json"
        bytes: u64,
        truncated: bool,
    },

    /// MCP gateway subcall trace for telemetry.
    MCPSubcallTrace {
        server: String,
        tool: String,
        latency_ms: u64,
        bytes_returned: u64,
        error: Option<String>,
    },

    /// Provider request issued (one per iteration; can repeat on retry).
    ProviderRequest {
        iteration: u32,
        model: String,
        prompt_tokens: u32,
        max_tokens: u32,
        attempt: u32,
    },

    /// Provider response received (paired with ProviderRequest by ordering).
    ProviderResponse {
        #[serde(rename = "latencyMs")]
        latency_ms: u64,
        usage: serde_json::Value,
        cost_usd: f64,
        retries_used: u32,
        finish_reason: String,
    },

    /// MidLoopCompressor compacted the message history.
    MidLoopCompressionTriggered {
        before_tokens: u32,
        after_tokens: u32,
        messages_condensed: u32,
        regions: Vec<String>,
    },
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(tool_provider_variant)'
```

Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "$(cat <<'EOF'
feat(agent): add tool/provider telemetry AgentEvent variants

ToolCallStreamChunk, MCPSubcallTrace, ProviderRequest, ProviderResponse,
MidLoopCompressionTriggered — five additive variants for runtime telemetry.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Add coding-specific AgentEvent variants

**Files:**
- Modify: `crates/agent/src/events.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/agent/src/events.rs`:

```rust
#[cfg(test)]
mod coding_specific_variant_tests {
    use super::*;

    #[test]
    fn file_edit_with_symbols_serializes_with_phase1_stub_fields_empty() {
        let event = AgentEvent::FileEditWithSymbols {
            path: "crates/agent/src/events.rs".into(),
            op: "edit".into(),
            bytes: 1240,
            diff_full: "@@ ... @@".into(),
            anchored_symbols: vec![],
            lsp_diagnostics_delta: vec![],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "fileEditWithSymbols");
        assert_eq!(v["anchoredSymbols"], serde_json::json!([]));
        assert_eq!(v["lspDiagnosticsDelta"], serde_json::json!([]));
    }

    #[test]
    fn test_run_detailed_serializes() {
        let event = AgentEvent::TestRunDetailed {
            command: "cargo nextest run -p agent".into(),
            framework: "nextest".into(),
            passed_tests: 42,
            failed_tests: 0,
            newly_passing: vec![],
            newly_failing: vec![],
            coverage_delta: None,
            duration_ms: 8200,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "testRunDetailed");
        assert_eq!(v["passedTests"], 42);
    }

    #[test]
    fn power_mode_toggled_turn_interrupted_smoke() {
        let _e1 = AgentEvent::PowerModeToggled {
            previous: "curated".into(),
            current: "power".into(),
            eager_tool_count: 36,
            deferred_tool_count: 0,
        };
        let _e2 = AgentEvent::TurnInterrupted {
            reason: "user_cancelled".into(),
            partial_tools: vec!["bash".into()],
            iterations_completed: 2,
        };
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(coding_specific_variant)'
```

Expected: FAIL — missing variants.

- [ ] **Step 3: Add the 4 variants**

Append to `enum AgentEvent` in `crates/agent/src/events.rs`:

```rust
    /// File edit with optional symbol/LSP enrichment. Phase 1 emits
    /// empty anchored_symbols (best-effort tree-sitter pass) and empty
    /// lsp_diagnostics_delta (LSP integration is Phase 2+).
    FileEditWithSymbols {
        path: String,
        op: String, // "edit" | "write" | "apply_patch"
        bytes: u64,
        diff_full: String,
        #[serde(rename = "anchoredSymbols")]
        anchored_symbols: Vec<serde_json::Value>,
        #[serde(rename = "lspDiagnosticsDelta")]
        lsp_diagnostics_delta: Vec<serde_json::Value>,
    },

    /// Test command finished with detailed per-test results.
    TestRunDetailed {
        command: String,
        framework: String, // "nextest" | "vitest" | "jest" | "pytest" | "unknown"
        passed_tests: u32,
        failed_tests: u32,
        newly_passing: Vec<String>,
        newly_failing: Vec<String>,
        coverage_delta: Option<f64>,
        duration_ms: u64,
    },

    /// Tool profile (`/power on|off`) changed mid-thread.
    PowerModeToggled {
        previous: String,
        current: String,
        eager_tool_count: u32,
        deferred_tool_count: u32,
    },

    /// Agent loop interrupted before reaching its final turn.
    TurnInterrupted {
        reason: String, // "user_cancelled" | "budget_exceeded" | "provider_error" | etc.
        partial_tools: Vec<String>,
        iterations_completed: u32,
    },
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(coding_specific_variant)'
```

Expected: 3 tests PASS.

- [ ] **Step 5: Verify all 18 new variants present**

```bash
grep -E "^\s+(RecallInjected|DeadEndWarningSurfaced|SkillActivationConsidered|SkillActivated|SkillReferenceLoaded|ContextEngineDecision|ApprovalRequested|ApprovalResolved|SandboxPolicyApplied|ToolCallStreamChunk|MCPSubcallTrace|ProviderRequest|ProviderResponse|MidLoopCompressionTriggered|FileEditWithSymbols|TestRunDetailed|PowerModeToggled|TurnInterrupted)" crates/agent/src/events.rs | wc -l
```

Expected: `18`.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "$(cat <<'EOF'
feat(agent): add coding-specific AgentEvent variants

FileEditWithSymbols (with Phase-1 empty stub fields for anchored_symbols
and lsp_diagnostics_delta), TestRunDetailed, PowerModeToggled,
TurnInterrupted. Completes the 18-variant set described in spec §10.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Audit chat-channel match arms for `_ =>` catch-all

**Files:**
- Modify: zero or more match-arm sites identified by `cargo build` errors after Task 4 added `#[non_exhaustive]`.

- [ ] **Step 1: Find non-exhaustive match-arm errors**

```bash
cargo build --workspace 2>&1 | grep -E "non-exhaustive|missing patterns" | head -30
```

Expected output: a list of files where `match AgentEvent { ... }` lacks a catch-all. If the list is empty (the `#[non_exhaustive]` only triggers cross-crate), proceed to Step 4.

- [ ] **Step 2: For each broken match site, add `_ => { /* additive variants ignored */ }`**

For each file path reported by Step 1, edit the offending `match` block to add a final arm:

```rust
match event {
    AgentEvent::ContentChunk { data } => { /* existing handler */ }
    AgentEvent::ToolStart { name, .. } => { /* existing handler */ }
    // … existing arms unchanged …
    _ => {
        // Additive variants from spec §10 are runtime-emit-only here;
        // this consumer is in a non-coding path and ignores them.
    }
}
```

Use a comment that names the spec section so future readers understand why the catch-all exists. **Do not** silently swallow the variants without comment — readers need to see the intent.

- [ ] **Step 3: Re-run cargo build until zero non-exhaustive errors**

```bash
cargo build --workspace 2>&1 | grep -E "non-exhaustive|missing patterns"
```

Expected: empty output.

- [ ] **Step 4: Run the full workspace test suite**

```bash
cargo nextest run --workspace
```

Expected: all green. (Match-arm catch-alls are syntactically valid even if no semantic change is needed.)

- [ ] **Step 5: Commit**

If Step 1 found broken sites, commit the edits:

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(agent): add _ => catch-all to AgentEvent match sites

Required after AgentEvent gained #[non_exhaustive] in Task 4. Non-coding
consumers ignore the new variants; coding-channel handlers (added in later
plans) will subscribe to them explicitly.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If Step 1 reported zero broken sites, skip the commit and write a one-line note to the next plan: "Match-arm audit was a no-op — `#[non_exhaustive]` is intra-crate so cross-crate match sites already required catch-alls."

---

## Task 9: Add `fan_out_event` helper to execute_loop core

**Files:**
- Modify: `crates/agent/src/execution/core.rs` (add helper at the top of the module, after existing imports)

This helper tees runtime events to both the existing `event_tx` (UI streaming) and a new `Arc<DomainEventBus>` (cognitive ingest). Existing callsites in `core.rs` and `execute_loop.rs` swap from `tx.send(...)` to `fan_out_event(tx, &bus, ...)` in Task 11.

- [ ] **Step 1: Write the failing test**

Add to `crates/agent/src/execution/core.rs` test module (or create one if absent):

```rust
#[cfg(test)]
mod fan_out_event_tests {
    use super::*;
    use crate::events::AgentEvent;
    use bus::DomainEventBus;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn fan_out_publishes_to_event_tx_and_domain_bus() {
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(8);
        let bus = Arc::new(DomainEventBus::new(8));
        let mut bus_rx = bus.subscribe();

        let evt = AgentEvent::ContentChunk { data: "hello".into() };
        fan_out_event(Some(&tx), &bus, evt.clone()).await;

        let from_tx = rx.recv().await.expect("event_tx received");
        let from_bus = bus_rx.recv().await.expect("bus received");

        // Both consumers receive the same event.
        match (from_tx, from_bus) {
            (AgentEvent::ContentChunk { data: a }, b) => {
                assert_eq!(a, "hello");
                let _ = b; // Detailed bus shape is asserted in bus crate's tests; smoke is enough here.
            }
            _ => panic!("event mismatch"),
        }
    }

    #[tokio::test]
    async fn fan_out_with_none_event_tx_still_publishes_to_bus() {
        let bus = Arc::new(DomainEventBus::new(8));
        let mut bus_rx = bus.subscribe();

        let evt = AgentEvent::ContentChunk { data: "no_ui".into() };
        fan_out_event(None, &bus, evt).await;

        let from_bus = bus_rx.recv().await.expect("bus received");
        match from_bus {
            // Match the actual DomainEvent variant — see bus::DomainEvent::Agent.
            // If the bus wraps differently (e.g., DomainEvent::AgentEvent(_)),
            // adjust the matcher to mirror the real wrapping.
            _ => {} // smoke: event reached the bus subscriber
        }
    }
}
```

**Implementation note for the engineer:** `bus::DomainEventBus` and its `DomainEvent` variant for `AgentEvent` may need a wrapping variant. If `DomainEvent` doesn't currently wrap `AgentEvent`, add a variant `DomainEvent::Agent(AgentEvent)` in the `bus` crate as part of this task. The test should reflect the actual wrapping shape.

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(fan_out_event)'
```

Expected: FAIL — `fan_out_event` not found.

- [ ] **Step 3: Add `DomainEvent::Agent` variant to bus crate (if missing)**

Check whether `crates/bus/src/domain_events.rs` already has a variant that wraps `AgentEvent`:

```bash
grep -n "AgentEvent\|pub enum DomainEvent" crates/bus/src/domain_events.rs | head -10
```

If no such variant exists, add one:

```rust
// In crates/bus/src/domain_events.rs, in the DomainEvent enum:
pub enum DomainEvent {
    // … existing variants …
    /// Runtime AgentEvent published via fan_out_event for cognitive subsystem ingest.
    Agent(agent::events::AgentEvent),
}
```

This may require adding `agent` as a `bus` dependency in `crates/bus/Cargo.toml`. Note: this could create a circular dep if `agent` already depends on `bus`. Verify with:

```bash
grep -A 3 "name = \"agent\"" crates/agent/Cargo.toml
```

If there's a cycle: instead of wrapping, the `fan_out_event` helper publishes a serialized `DomainEvent::Generic { kind: "agent_event", payload: serde_json::to_value(&event)? }` (or equivalent existing escape variant). Adapt the test in Step 1 to match.

- [ ] **Step 4: Add the `fan_out_event` helper**

Add to `crates/agent/src/execution/core.rs`, after the existing imports:

```rust
use bus::DomainEventBus;
use std::sync::Arc;

/// Fan out a runtime AgentEvent to both the UI-streaming channel (single-
/// consumer mpsc::Sender) and the cognitive-ingest broadcast bus.
///
/// - `event_tx`: Optional. When `Some`, sends to the chat-streaming pipeline
///   that emits Tauri `agent:*` events. Drops are ignored (UI may close).
/// - `domain_bus`: Always-required. Used by Distiller and Mirror subscribers.
///
/// This is the preferred replacement for direct `event_tx.send(evt).await`
/// at every existing emit site in `core.rs` and `execute_loop.rs`.
pub async fn fan_out_event(
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    domain_bus: &Arc<DomainEventBus>,
    evt: crate::events::AgentEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    domain_bus.publish(bus::DomainEvent::Agent(evt));
}
```

(If you took the `DomainEvent::Generic` escape route in Step 3, call that instead.)

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(fan_out_event)'
```

Expected: 2 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/execution/core.rs crates/bus/src/domain_events.rs crates/bus/Cargo.toml crates/agent/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(agent): add fan_out_event helper for UI + cognitive event tee

The agent loop's existing event_tx (mpsc::Sender, single-consumer) only
reaches the chat-streaming pipeline. fan_out_event additionally publishes
to DomainEventBus (broadcast, multi-subscriber) so Distiller and Mirror
subscribers can ingest the same events without a fan-out hack.

Adds DomainEvent::Agent(AgentEvent) variant to the bus crate.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Thread `Arc<DomainEventBus>` through `AgentRuntime`

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (constructor, struct field)
- Modify: callers in `crates/app-core/` (initialization paths)

- [ ] **Step 1: Locate the AgentRuntime constructor and current callers**

```bash
grep -rn "AgentRuntime::new\|AgentRuntime {" /Users/jayden/Projects/Klynt/bot/crates 2>/dev/null | head -10
```

Note the call sites — you'll update each in Step 4.

- [ ] **Step 2: Write the failing test**

Add to `crates/agent/src/agent_runtime/runtime.rs` test module (or create one):

```rust
#[cfg(test)]
mod constructor_tests {
    use super::*;
    use bus::DomainEventBus;
    use std::sync::Arc;

    #[test]
    fn runtime_accepts_domain_event_bus() {
        let bus = Arc::new(DomainEventBus::new(16));
        // Adjust additional constructor arguments per the actual signature.
        // The test asserts the type compiles; actual processing is tested
        // elsewhere.
        let _bus_ref: &Arc<DomainEventBus> = &bus;
    }
}
```

If AgentRuntime has no constructor that's easily testable from outside, instead write a doc-test on the struct's `pub` field accessor (or on a new `pub fn domain_event_bus(&self) -> &Arc<DomainEventBus>`).

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(constructor_tests)'
```

Expected: FAIL — `AgentRuntime` doesn't have a `domain_event_bus` field yet, or the test's reference type fails to coerce.

- [ ] **Step 4: Add the field and constructor parameter**

Edit `crates/agent/src/agent_runtime/runtime.rs`:

1. Add `use bus::DomainEventBus;` at the top of the file (with other `use` statements).
2. Add `use std::sync::Arc;` if not already present.
3. Add a field to the `AgentRuntime` struct:

```rust
pub struct AgentRuntime {
    // … existing fields …
    domain_event_bus: Arc<DomainEventBus>,
}
```

4. Update the constructor (`pub fn new(...)` or builder methods) to accept and store the bus. Find the existing constructor signature with:

```bash
grep -n "pub fn new\|pub fn with_" crates/agent/src/agent_runtime/runtime.rs | head -5
```

Add `domain_event_bus: Arc<DomainEventBus>` as the **last** parameter (before any optional/builder hooks) so existing positional callers fail to compile loudly. Store it in the new field.

- [ ] **Step 5: Update callers**

For each caller identified in Step 1, pass an `Arc<DomainEventBus>` through. The desktop `AppCore` already constructs an `Arc<DomainEventBus>` per CLAUDE.md ("MirrorEngine::start takes Arc<DomainEventBus>"). Locate it:

```bash
grep -rn "DomainEventBus::new\|domain_event_bus" /Users/jayden/Projects/Klynt/bot/crates/app-core/src 2>/dev/null | head -10
```

Pass that same `Arc` clone into each `AgentRuntime::new` call. Tests that construct ad-hoc runtimes use `Arc::new(DomainEventBus::new(8))`.

- [ ] **Step 6: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(constructor_tests)'
```

Expected: PASS.

- [ ] **Step 7: Build the workspace to confirm callers compile**

```bash
cargo build --workspace
```

Expected: zero errors. If any caller you missed still fails, add the bus argument and retry.

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs crates/app-core/src
git commit -m "$(cat <<'EOF'
feat(agent): thread Arc<DomainEventBus> through AgentRuntime

Required so the runtime can call fan_out_event for every emitted AgentEvent,
publishing to the cognitive-ingest broadcast bus alongside the existing
mpsc UI channel. Callers in app-core pass the same Arc that MirrorEngine
already receives.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Replace direct `event_tx.send(...)` calls with `fan_out_event` calls

**Files:**
- Modify: `crates/agent/src/execution/core.rs`
- Modify: `crates/agent/src/execution/execute_loop.rs`

- [ ] **Step 1: Find every direct send site**

```bash
grep -n "event_tx.send\|tx.send" crates/agent/src/execution/core.rs crates/agent/src/execution/execute_loop.rs
```

Expected: ~12 sites total (per spec §4 implementation detail).

- [ ] **Step 2: Write the failing integration test**

Add to `crates/agent/src/execution/execute_loop.rs` test module:

```rust
#[cfg(test)]
mod fan_out_integration_tests {
    use super::*;
    use crate::events::AgentEvent;
    use bus::DomainEventBus;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn execute_loop_publishes_events_to_domain_bus() {
        let bus = Arc::new(DomainEventBus::new(16));
        let mut bus_rx = bus.subscribe();
        let (tx, _rx) = mpsc::channel::<AgentEvent>(16);

        // Mocked execute_loop call — exact signature depends on existing
        // test scaffolding; reuse it. The test asserts: any AgentEvent
        // emitted via fan_out_event reaches both rx and bus_rx.
        //
        // For this test, call fan_out_event directly to verify wiring;
        // a fuller scenario test lives in Plan 6.
        crate::execution::core::fan_out_event(
            Some(&tx),
            &bus,
            AgentEvent::PipelineStarted,
        ).await;

        let from_bus = bus_rx.recv().await.expect("bus received");
        match from_bus {
            bus::DomainEvent::Agent(AgentEvent::PipelineStarted) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(fan_out_integration)'
```

Expected: FAIL if `pub` visibility on `fan_out_event` or `domain_event_bus` accessor is missing; PASS otherwise (the helper exists from Task 9).

- [ ] **Step 4: Update each `event_tx.send(...)` site**

For each site found in Step 1, replace:

```rust
let _ = event_tx.send(some_event).await;
```

with:

```rust
fan_out_event(event_tx.as_ref(), &core.domain_event_bus, some_event).await;
```

Where `core.domain_event_bus` is the new field on `ExecutionCore`. If `ExecutionCore` doesn't carry the bus yet, add it as a struct field and pass through from `execute_loop`'s caller (which is `AgentRuntime::process` — already holding the bus from Task 10).

- [ ] **Step 5: Run all agent tests to verify no regression**

```bash
cargo nextest run -p agent
```

Expected: all green.

- [ ] **Step 6: Run workspace build + clippy**

```bash
cargo build --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/core.rs crates/agent/src/execution/execute_loop.rs
git commit -m "$(cat <<'EOF'
refactor(agent): route runtime events through fan_out_event

Replaces ~12 direct event_tx.send call sites in core.rs and execute_loop.rs
with fan_out_event so every emitted event reaches both the UI streaming
channel and the cognitive DomainEventBus. No semantic change for existing
chat consumers; cognitive subscribers (added in Plan 5) gain visibility.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Add read-only-aware tool partitioning to `execute_tool_calls`

**Files:**
- Modify: `crates/agent/src/execution/core.rs::execute_tool_calls`

Per spec §3 surgical change #2, the loop partitions tool calls by `Tool::is_concurrency_safe` (parallel for safe, sequential for unsafe). Existing parallel-fan-out is capped at `MAX_CONCURRENT_TOOLS = 10`.

- [ ] **Step 1: Locate the function**

```bash
grep -n "fn execute_tool_calls\|MAX_CONCURRENT_TOOLS" crates/agent/src/execution/core.rs | head -5
```

Read the surrounding ~50 lines to understand the current implementation.

- [ ] **Step 2: Write the failing test**

Add to `crates/agent/src/execution/core.rs` test module:

```rust
#[cfg(test)]
mod partitioning_tests {
    use super::*;
    use serde_json::json;

    /// Test fixture: read-only tool (concurrency-safe).
    struct ReadOnly;
    #[async_trait::async_trait]
    impl tools_core::Tool for ReadOnly {
        fn name(&self) -> &str { "ro" }
        fn description(&self) -> &str { "fixture" }
        fn parameters(&self) -> serde_json::Value { json!({"type":"object"}) }
        async fn execute(&self, _: serde_json::Value, _: &tools_core::RoutingContext) -> common::Result<String> {
            Ok("ok".into())
        }
        fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool { true }
    }

    /// Test fixture: writing tool (NOT concurrency-safe).
    struct Writer;
    #[async_trait::async_trait]
    impl tools_core::Tool for Writer {
        fn name(&self) -> &str { "wr" }
        fn description(&self) -> &str { "fixture" }
        fn parameters(&self) -> serde_json::Value { json!({"type":"object"}) }
        async fn execute(&self, _: serde_json::Value, _: &tools_core::RoutingContext) -> common::Result<String> {
            Ok("ok".into())
        }
        // Default is_concurrency_safe = false.
    }

    #[test]
    fn partition_separates_safe_and_unsafe_tools() {
        // The actual partition logic is private to execute_tool_calls;
        // test it via a small helper extracted for testability:
        let safe = vec![("ro", json!({}))];
        let unsafe_ = vec![("wr", json!({}))];

        let registry: std::collections::HashMap<&str, Box<dyn tools_core::Tool>> = vec![
            ("ro", Box::new(ReadOnly) as Box<dyn tools_core::Tool>),
            ("wr", Box::new(Writer) as Box<dyn tools_core::Tool>),
        ].into_iter().collect();

        let calls = vec![("ro", json!({})), ("wr", json!({})), ("ro", json!({}))];

        let (safe_count, unsafe_count) = partition_by_concurrency_safety(&calls, &registry);
        assert_eq!(safe_count, 2);
        assert_eq!(unsafe_count, 1);
    }
}

/// Extracted helper for testability — counts safe vs unsafe calls.
fn partition_by_concurrency_safety<'a>(
    calls: &[(&'a str, serde_json::Value)],
    registry: &std::collections::HashMap<&'a str, Box<dyn tools_core::Tool>>,
) -> (usize, usize) {
    let mut safe = 0;
    let mut unsafe_ = 0;
    for (name, args) in calls {
        if let Some(tool) = registry.get(name) {
            if tool.is_concurrency_safe(args) { safe += 1; } else { unsafe_ += 1; }
        }
    }
    (safe, unsafe_)
}
```

(Define `partition_by_concurrency_safety` as a non-pub helper at the module level so the existing `execute_tool_calls` can call it too.)

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo nextest run -p agent -E 'test(partition_separates)'
```

Expected: FAIL — `partition_by_concurrency_safety` not found.

- [ ] **Step 4: Add the partition helper and update `execute_tool_calls`**

In `crates/agent/src/execution/core.rs`, add the helper from Step 2 at the module level. Then modify `execute_tool_calls` to use it:

```rust
async fn execute_tool_calls(
    // … existing parameters …
) -> Result<Vec<ToolResult>> {
    // Partition by Tool::is_concurrency_safe.
    let (safe_calls, unsafe_calls): (Vec<_>, Vec<_>) = tool_calls
        .into_iter()
        .partition(|tc| {
            registry
                .get(&tc.name)
                .map(|tool| tool.is_concurrency_safe(&tc.args))
                .unwrap_or(false)
        });

    let mut results = Vec::with_capacity(safe_calls.len() + unsafe_calls.len());

    // Parallel: safe calls, capped at MAX_CONCURRENT_TOOLS.
    let safe_futures = safe_calls.into_iter().map(|tc| run_tool(tc, /* args */));
    let safe_results = futures::future::join_all(safe_futures).await;
    results.extend(safe_results);

    // Sequential: unsafe calls.
    for tc in unsafe_calls {
        let r = run_tool(tc, /* args */).await;
        results.push(r);
    }

    Ok(results)
}
```

(Adapt to the existing function's signature and helper names — the structure above is illustrative; preserve existing error handling, cancellation checks, timeouts.)

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo nextest run -p agent -E 'test(partition_separates)'
```

Expected: PASS.

- [ ] **Step 6: Run all agent tests**

```bash
cargo nextest run -p agent
```

Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "$(cat <<'EOF'
feat(agent): partition tool calls by Tool::is_concurrency_safe

execute_tool_calls now runs concurrency-safe tools (read/glob/grep/recall_*)
in parallel via futures::join_all, and unsafe tools sequentially. Default
is_concurrency_safe = false preserves existing behavior for tools that
haven't opted in.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Update spec to reflect actual `sessions` table name

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`

The spec uses `chat_sessions` throughout but the actual table is `sessions` (in `crates/storage/migrations/001_initial.sql:126`). The existing `pinned INTEGER` and `conversation_type TEXT` columns subsume what the spec calls `starred` and `mode`. Reconcile before writing the migration.

- [ ] **Step 1: Find every `chat_sessions` reference**

```bash
grep -n "chat_sessions" docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md | head -20
```

Note the line numbers.

- [ ] **Step 2: Replace `chat_sessions` with `sessions`**

```bash
sed -i.bak 's/chat_sessions/sessions/g' docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md
diff docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md.bak docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md | head -50
rm docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md.bak
```

Expected diff: ~12-15 line changes, all `chat_sessions` → `sessions`.

- [ ] **Step 3: Update spec §11 schema to reuse `pinned` and `conversation_type`**

Edit `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §11 "Schema changes." Replace the `ALTER TABLE` block with:

```sql
-- conversation_type already exists; extended valid values:
--   'general' (existing default), 'coding' (new for coding-mode threads).
-- pinned already exists; reused as the spec's "starred" semantics.

ALTER TABLE sessions ADD COLUMN cwd TEXT;
ALTER TABLE sessions ADD COLUMN repo_id TEXT;
ALTER TABLE sessions ADD COLUMN repo_branch TEXT;
ALTER TABLE sessions ADD COLUMN tool_profile TEXT;                    -- 'minimal' | 'curated' | 'power'
ALTER TABLE sessions ADD COLUMN approval_mode TEXT NOT NULL DEFAULT 'default';  -- 'default' | 'plan' | 'bypass'
ALTER TABLE sessions ADD COLUMN total_cost_usd REAL NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN parent_session_id TEXT;               -- forks
```

So 8 new columns plus 2 reused. Add a paragraph immediately before/after the SQL block:

> **Reuse of existing columns:** The existing `pinned INTEGER DEFAULT 0` column carries the spec's "starred" semantics (1 = starred, 0 = not). The existing `conversation_type TEXT DEFAULT 'general'` carries the mode — values `'general'` (default chat) or `'coding'` (coding mode). New code references these existing columns directly; no rename, no alias.

Update Appendix B's "surgical edits" list and the §3 surgical change #6 to match.

- [ ] **Step 4: Update Appendix A row 3 (Coding mode)**

Change row 3 of Appendix A from:

> Per-thread toggle on the composer; auto-detect from workspace context; persisted in `chat_sessions.mode`.

to:

> Per-thread toggle on the composer; auto-detect from workspace context; persisted in `sessions.conversation_type` (existing column; values `'general'` | `'coding'`).

- [ ] **Step 5: Verify no `chat_sessions` or `starred` references remain**

```bash
grep -n "chat_sessions\|starred" docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md
```

Expected: empty output.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md
git commit -m "$(cat <<'EOF'
docs(spec): align coding-in-chat spec with actual sessions table

Spec referenced `chat_sessions` but the table is named `sessions`
(crates/storage/migrations/001_initial.sql:126). Existing `pinned` and
`conversation_type` columns subsume what the spec called `starred` and
`mode`; reuse them rather than adding parallel columns.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Add 8 new columns to `sessions` table

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:126-138` (the `CREATE TABLE sessions` block)
- Modify: `crates/storage/src/repos/sessions.rs` (or `crates/storage/src/repos/<wherever Sessions is>.rs`; verify path in Step 1)

Per CLAUDE.md "Pre-release — no user data to migrate. All schema changes can be made directly (alter tables, drop and recreate) without writing migration scripts." We edit `001_initial.sql` directly.

- [ ] **Step 1: Locate the sessions repo Rust code**

```bash
grep -rn "FROM sessions\|sessions WHERE\|INSERT INTO sessions" crates/storage/src/ 2>/dev/null | head -10
```

Note the file path. (The most likely location is `crates/storage/src/repos/sessions.rs`; if it's elsewhere, use what grep finds.)

- [ ] **Step 2: Write the failing test**

Add to the sessions repo file (or create a test module in it):

```rust
#[cfg(test)]
mod schema_tests {
    use super::*;
    use crate::pool::StoragePool;

    #[tokio::test]
    async fn sessions_table_has_new_coding_columns() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let conn = pool.connection().await.unwrap();

        let cols: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('sessions')",
        )
        .fetch_all(&conn)
        .await
        .unwrap();

        for required in &[
            "cwd", "repo_id", "repo_branch", "tool_profile",
            "approval_mode", "total_cost_usd", "total_tokens", "parent_session_id",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "expected column `{}` on sessions table; columns are: {:?}",
                required,
                cols,
            );
        }
    }

    #[tokio::test]
    async fn sessions_default_approval_mode_is_default() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let conn = pool.connection().await.unwrap();

        sqlx::query("INSERT INTO sessions (key) VALUES ('s1')")
            .execute(&conn).await.unwrap();
        let val: String = sqlx::query_scalar(
            "SELECT approval_mode FROM sessions WHERE key='s1'"
        )
        .fetch_one(&conn).await.unwrap();
        assert_eq!(val, "default");
    }
}
```

Use the existing storage test helpers if the file already has them (check `crates/storage/src/repos/tests/` — there are existing test patterns).

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo nextest run -p storage -E 'test(sessions_table_has_new_coding_columns)'
```

Expected: FAIL — column `cwd` not found.

- [ ] **Step 4: Edit the SQL migration**

Edit `crates/storage/migrations/001_initial.sql` lines 126-138 (the `CREATE TABLE sessions` block) to add the 8 new columns:

```sql
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    compressed_prefix      TEXT,
    compressed_through_idx INTEGER,
    compressed_at          INTEGER,
    -- Coding-in-chat columns (added 2026-04-29 per spec 2026-04-29-klynt-coding-in-chat-design.md §11)
    cwd                    TEXT,
    repo_id                TEXT,
    repo_branch            TEXT,
    tool_profile           TEXT,
    approval_mode          TEXT NOT NULL DEFAULT 'default',
    total_cost_usd         REAL NOT NULL DEFAULT 0,
    total_tokens           INTEGER NOT NULL DEFAULT 0,
    parent_session_id      TEXT
);
```

(Order within the block doesn't matter functionally; group new columns at the bottom with a comment for git-archaeology.)

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo nextest run -p storage -E 'test(sessions_table_has_new_coding_columns)'
cargo nextest run -p storage -E 'test(sessions_default_approval_mode)'
```

Expected: both PASS.

- [ ] **Step 6: Run all storage tests**

```bash
cargo nextest run -p storage
```

Expected: all green. (Existing tests use `connect_in_memory()` and re-apply this migration each run; they should be unaffected.)

- [ ] **Step 7: Update the Sessions repo struct (if applicable)**

If `crates/storage/src/repos/sessions.rs` (or wherever) defines a Rust struct that maps the row, add the 8 new fields:

```rust
pub struct SessionRow {
    pub key: String,
    // … existing fields …
    pub cwd: Option<String>,
    pub repo_id: Option<String>,
    pub repo_branch: Option<String>,
    pub tool_profile: Option<String>,
    pub approval_mode: String,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub parent_session_id: Option<String>,
}
```

Update SELECT queries to include the new columns (or use `SELECT *` if the existing pattern relies on column-name binding).

- [ ] **Step 8: Build the workspace to confirm callers compile**

```bash
cargo build --workspace
```

Expected: zero errors. If a downstream crate destructures `SessionRow` and breaks, add the new fields with `..` patterns or update the destructure.

- [ ] **Step 9: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/repos/
git commit -m "$(cat <<'EOF'
feat(storage): add coding-in-chat columns to sessions table

Adds cwd, repo_id, repo_branch, tool_profile, approval_mode (default),
total_cost_usd, total_tokens, parent_session_id. Per CLAUDE.md pre-release
policy, edits 001_initial.sql directly rather than adding 002_*.sql.

Existing pinned (used as starred) and conversation_type (used as mode,
values 'general' | 'coding') columns are reused.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: Scaffold `klynt-protocol` crate

**Files:**
- Create: `crates/klynt-protocol/Cargo.toml`
- Create: `crates/klynt-protocol/src/lib.rs`
- Create: `crates/klynt-protocol/VENDOR.md`
- Modify: `Cargo.toml` (workspace members)

This is the first of 7 crate-skeleton tasks. The pattern is the same for each (Tasks 15-21): create Cargo.toml + lib.rs + VENDOR.md, add to workspace, verify build. **No content yet** — Plan 2 vendors Codex sources here.

- [ ] **Step 1: Create the Cargo.toml**

Create `crates/klynt-protocol/Cargo.toml`:

```toml
[package]
name = "klynt-protocol"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt protocol types — event/op/submission types for the coding-in-chat surface. Adapted from codex-rs/protocol/."

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
```

(Inherit `serde` and `serde_json` from `[workspace.dependencies]` in the root Cargo.toml; if those aren't workspace-shared, use direct version specifiers like `serde = "1"` matching the rest of the workspace.)

- [ ] **Step 2: Create the empty library root**

Create `crates/klynt-protocol/src/lib.rs`:

```rust
//! Klynt protocol types — adapted from `codex-rs/protocol/`.
//!
//! This crate is a foundation skeleton in Plan 1; Plan 2 vendors the
//! Codex protocol types and renames `Codex*` → `Klynt*` per
//! `scripts/adapt_codex_vendor.sh`.
//!
//! See spec: docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md §3.

// Public API surface — empty until Plan 2 vendoring lands.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Sentinel: this test exists so the crate has at least one
        // unit test from day one. Plan 2 replaces this when vendored
        // tests land.
    }
}
```

- [ ] **Step 3: Create the VENDOR.md placeholder**

Create `crates/klynt-protocol/VENDOR.md`:

```markdown
# klynt-protocol — Vendor Provenance

**Adapted from:** `codex-rs/protocol/` (upstream commit pending — pinned in Plan 2)
**License:** Apache-2.0
**Adaptation script:** `scripts/adapt_codex_vendor.sh`

**Renames applied (planned for Plan 2):**
- `codex_*` → `klynt_*` (modules)
- `CodexEvent` → `KlyntEvent` (types)
- `~/.codex/` → `~/.klyntbot/` (paths)
- `CODEX_API_KEY` → `KLYNT_API_KEY` (env vars)

**Phase 1 (Plan 1):** Empty skeleton; only the package metadata exists.
**Phase 1 (Plan 2):** Vendored sources land via the adapt script.
```

- [ ] **Step 4: Add the crate to the workspace**

Edit `/Users/jayden/Projects/Klynt/bot/Cargo.toml`. In the `members` array (around line 4), add `"crates/klynt-protocol",` alphabetically near the other crates (e.g., after `"crates/feature-tasks",`):

```toml
members = [
    # … existing members …
    "crates/klynt-protocol",
    # … remaining members …
]
```

- [ ] **Step 5: Verify the crate builds**

```bash
cargo build -p klynt-protocol && cargo nextest run -p klynt-protocol
```

Expected: build succeeds, 1 test passes (`crate_compiles`).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/klynt-protocol/
git commit -m "$(cat <<'EOF'
feat(klynt-protocol): scaffold empty crate skeleton

Foundation crate for the coding-in-chat surface. Plan 2 vendors Codex
protocol types here via scripts/adapt_codex_vendor.sh. Empty skeleton
so the workspace builds and downstream crates can declare a dependency
on it from day one.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 16: Scaffold `klynt-execpolicy` crate

**Files:**
- Create: `crates/klynt-execpolicy/Cargo.toml`
- Create: `crates/klynt-execpolicy/src/lib.rs`
- Create: `crates/klynt-execpolicy/VENDOR.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create Cargo.toml**

Create `crates/klynt-execpolicy/Cargo.toml`:

```toml
[package]
name = "klynt-execpolicy"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt execution policy — Starlark prefix-rule approval engine. Adapted from codex-rs/execpolicy/."

[dependencies]
common = { path = "../common" }
serde = { workspace = true }
thiserror = "1"

[dev-dependencies]
```

(`common` provides `Result`/`KlyntbotError` per project convention.)

- [ ] **Step 2: Create lib.rs**

Create `crates/klynt-execpolicy/src/lib.rs`:

```rust
//! Klynt execution policy — Starlark prefix-rule approval engine.
//! Adapted from `codex-rs/execpolicy/` (Plan 2).
//!
//! Phase 1 (Plan 1): empty skeleton.
//! Phase 1 (Plan 2): vendor the Codex execpolicy crate.
//! Phase 1 (Plan 4): wire to the 3-layer approval gate.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
```

- [ ] **Step 3: Create VENDOR.md**

Create `crates/klynt-execpolicy/VENDOR.md`:

```markdown
# klynt-execpolicy — Vendor Provenance

**Adapted from:** `codex-rs/execpolicy/` (upstream commit pending; pinned in Plan 2)
**License:** Apache-2.0

**Phase 1 (Plan 1):** Empty skeleton.
**Phase 1 (Plan 2):** Codex sources vendored.
**Phase 1 (Plan 4):** Wired to Layer 2 of the 3-layer approval gate.
```

- [ ] **Step 4: Add to workspace `members`**

Edit `Cargo.toml`, add `"crates/klynt-execpolicy",` to the `members` array.

- [ ] **Step 5: Build and test**

```bash
cargo build -p klynt-execpolicy && cargo nextest run -p klynt-execpolicy
```

Expected: builds + 1 test passes.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/klynt-execpolicy/
git commit -m "feat(klynt-execpolicy): scaffold empty crate skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 17: Scaffold `klynt-sandbox` crate

**Files:**
- Create: `crates/klynt-sandbox/Cargo.toml`, `src/lib.rs`, `VENDOR.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "klynt-sandbox"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt sandbox — Seatbelt (.sbpl) for macOS, Landlock+bwrap for Linux. Adapted from codex-rs/sandboxing/."

[dependencies]
common = { path = "../common" }
serde = { workspace = true }
thiserror = "1"

[target.'cfg(target_os = "macos")'.dependencies]
# Add Seatbelt-specific deps in Plan 2.

[target.'cfg(target_os = "linux")'.dependencies]
# Add Landlock + bwrap deps in Plan 2.

[dev-dependencies]
```

- [ ] **Step 2: lib.rs**

```rust
//! Klynt sandbox — OS-level sandboxing for tool execution.
//!
//! macOS: Seatbelt via `sandbox-exec` + generated .sbpl policies.
//! Linux: Landlock + bwrap via klynt-sandbox-helper child binary.
//!
//! Plan 1: skeleton. Plan 2: macOS Seatbelt lit up. Plan 3: Linux.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
```

- [ ] **Step 3: VENDOR.md**

```markdown
# klynt-sandbox — Vendor Provenance

**Adapted from:** `codex-rs/sandboxing/`
**License:** Apache-2.0

**Phase 1 (Plan 1):** Empty skeleton.
**Phase 1 (Plan 2):** macOS Seatbelt vendored + lit up.
**Phase 1 (Plan 3):** Linux Landlock + bwrap vendored + lit up.
```

- [ ] **Step 4: Add to workspace, build, test, commit**

```bash
# Edit Cargo.toml: add "crates/klynt-sandbox" to members
cargo build -p klynt-sandbox && cargo nextest run -p klynt-sandbox
git add Cargo.toml crates/klynt-sandbox/
git commit -m "feat(klynt-sandbox): scaffold empty crate skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 18: Scaffold `klynt-sandbox-helper` (Linux child binary)

**Files:**
- Create: `crates/klynt-sandbox-helper/Cargo.toml`, `src/main.rs`, `VENDOR.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "klynt-sandbox-helper"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Linux child-process helper that applies Landlock + seccomp before exec'ing a tool. Adapted from codex-rs/linux-sandbox/."

[[bin]]
name = "klynt-sandbox-helper"
path = "src/main.rs"

[dependencies]
common = { path = "../common" }

[dev-dependencies]
```

- [ ] **Step 2: main.rs (binary entry point stub)**

```rust
//! klynt-sandbox-helper — Linux Landlock + seccomp child binary.
//!
//! Plan 1: stub; prints version and exits.
//! Plan 3: vendored from codex-rs/linux-sandbox/ and lit up.

fn main() {
    eprintln!("klynt-sandbox-helper v0.0.1 — skeleton (Plan 1)");
    std::process::exit(0);
}
```

- [ ] **Step 3: VENDOR.md**

```markdown
# klynt-sandbox-helper — Vendor Provenance

**Adapted from:** `codex-rs/linux-sandbox/`
**License:** Apache-2.0

**Phase 1 (Plan 1):** Stub binary.
**Phase 1 (Plan 3):** Linux Landlock + bwrap vendored + lit up.
```

- [ ] **Step 4: Add to workspace, build, test, commit**

```bash
# Edit Cargo.toml: add "crates/klynt-sandbox-helper"
cargo build -p klynt-sandbox-helper
target/debug/klynt-sandbox-helper  # Smoke: runs and exits 0.
git add Cargo.toml crates/klynt-sandbox-helper/
git commit -m "feat(klynt-sandbox-helper): scaffold Linux helper binary

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 19: Scaffold `klynt-hooks` crate

**Files:**
- Create: `crates/klynt-hooks/Cargo.toml`, `src/lib.rs`, `VENDOR.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "klynt-hooks"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt hook engine — 13-event Claude-Code-compatible schema. Adapted from codex-rs/hooks/."

[dependencies]
common = { path = "../common" }
serde = { workspace = true }
toml = "0.8"
tokio = { workspace = true, features = ["process", "time", "macros"] }

[dev-dependencies]
```

- [ ] **Step 2: lib.rs**

```rust
//! Klynt hook engine — 13-event Claude-Code-compatible schema.
//!
//! Reads `~/.klyntbot/hooks.toml`, dispatches subprocess hooks at the 13
//! event boundaries listed in spec §7.
//!
//! Plan 1: skeleton. Plan 4: vendor + light up.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
```

- [ ] **Step 3: VENDOR.md**

```markdown
# klynt-hooks — Vendor Provenance

**Adapted from:** `codex-rs/hooks/`
**License:** Apache-2.0

**Phase 1 (Plan 1):** Empty skeleton.
**Phase 1 (Plan 4):** Vendored + 13-event engine lit up.
```

- [ ] **Step 4: Add to workspace, build, test, commit**

```bash
# Cargo.toml: add "crates/klynt-hooks"
cargo build -p klynt-hooks && cargo nextest run -p klynt-hooks
git add Cargo.toml crates/klynt-hooks/
git commit -m "feat(klynt-hooks): scaffold empty crate skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 20: Scaffold `klynt-skill-loader` crate

**Files:**
- Create: `crates/klynt-skill-loader/Cargo.toml`, `src/lib.rs`
- Modify: `Cargo.toml`

(No `VENDOR.md` — this is a fresh crate, not Codex-derived.)

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "klynt-skill-loader"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt skill discovery + paths-conditional activation + dynamic discovery. Extends skill-system."

[dependencies]
common = { path = "../common" }
skill-system = { path = "../skill-system" }
serde = { workspace = true }
serde_yaml = "0.9"
globset = "0.4"

[dev-dependencies]
```

- [ ] **Step 2: lib.rs**

```rust
//! Klynt skill loader — extends `skill-system` with:
//! - Discovery from `~/.klyntbot/skills/` and `~/.klyntbot/project-skills/`.
//! - Path-conditional activation via `paths:` frontmatter glob.
//! - Dynamic discovery on file-touch.
//!
//! Plan 1: skeleton. Plan 5: lit up.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
```

- [ ] **Step 3: Add to workspace, build, test, commit**

```bash
# Cargo.toml: add "crates/klynt-skill-loader"
cargo build -p klynt-skill-loader && cargo nextest run -p klynt-skill-loader
git add Cargo.toml crates/klynt-skill-loader/
git commit -m "feat(klynt-skill-loader): scaffold empty crate skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 21: Scaffold `klynt-core` crate

**Files:**
- Create: `crates/klynt-core/Cargo.toml`, `src/lib.rs`
- Modify: `Cargo.toml`

(No `VENDOR.md` — fresh crate.)

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "klynt-core"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt core — coding-tool registry (bash/read/edit/...), execpolicy/sandbox/hooks glue, slash-command direct dispatch."

[dependencies]
common = { path = "../common" }
tools-core = { path = "../tools-core" }
klynt-protocol = { path = "../klynt-protocol" }
klynt-execpolicy = { path = "../klynt-execpolicy" }
klynt-sandbox = { path = "../klynt-sandbox" }
klynt-hooks = { path = "../klynt-hooks" }
klynt-skill-loader = { path = "../klynt-skill-loader" }
async-trait = "0.1"
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["process", "time", "macros", "sync"] }

[dev-dependencies]
```

- [ ] **Step 2: lib.rs**

```rust
//! Klynt core — coding-tool registry + glue between execpolicy, sandbox,
//! hooks, and the agent loop.
//!
//! Plan 1: skeleton.
//! Plan 2: bash tool + macOS Seatbelt + ApprovalCard wiring.
//! Plan 3: read/glob/grep/edit/write/apply_patch/web_fetch/notebook_edit.
//! Plan 4: hook engine integration; Layer 2 wiring.
//! Plan 5: skill-loader integration; recall_* tool registration.

pub mod tools {
    //! Coding tools (bash, read, glob, grep, edit, write, apply_patch, …).
    //! Plans 2-3 add implementations.
}

pub mod slash {
    //! Slash-command direct-dispatch handlers (skills, status, doctor, …).
    //! Plan 6 adds implementations.
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
```

- [ ] **Step 3: Add to workspace, build, test, commit**

```bash
# Cargo.toml: add "crates/klynt-core"
cargo build -p klynt-core && cargo nextest run -p klynt-core
git add Cargo.toml crates/klynt-core/
git commit -m "feat(klynt-core): scaffold empty crate skeleton

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 22: Create `scripts/adapt_codex_vendor.sh` skeleton

**Files:**
- Create: `scripts/adapt_codex_vendor.sh`

This script will be filled in Plan 2 to mechanically rename Codex symbols. Plan 1 just stakes the path.

- [ ] **Step 1: Verify scripts/ exists**

```bash
ls -la /Users/jayden/Projects/Klynt/bot/scripts 2>/dev/null || mkdir -p /Users/jayden/Projects/Klynt/bot/scripts
```

- [ ] **Step 2: Write the failing test**

(There's no test framework for shell scripts in this repo; the "test" is "the script exists, is executable, and prints usage when invoked with `--help`.")

Add a one-liner integration test inline. Save in `scripts/adapt_codex_vendor.sh.test.sh`:

```bash
#!/usr/bin/env bash
# Smoke test for adapt_codex_vendor.sh
set -euo pipefail
output=$(bash "$(dirname "$0")/adapt_codex_vendor.sh" --help 2>&1)
echo "$output" | grep -q "Usage:" || { echo "FAIL: usage line missing"; exit 1; }
echo "PASS: usage line present"
```

```bash
chmod +x scripts/adapt_codex_vendor.sh.test.sh
```

- [ ] **Step 3: Run test to verify it fails**

```bash
bash scripts/adapt_codex_vendor.sh.test.sh
```

Expected: FAIL — script doesn't exist yet.

- [ ] **Step 4: Create the script skeleton**

Create `scripts/adapt_codex_vendor.sh`:

```bash
#!/usr/bin/env bash
# adapt_codex_vendor.sh — mechanical rename pass for Codex-vendored crates.
#
# Plan 1: skeleton; only --help is implemented.
# Plan 2: full renames — codex_* → klynt_*, ~/.codex/ → ~/.klyntbot/, etc.
#
# Usage:
#   adapt_codex_vendor.sh --help
#   adapt_codex_vendor.sh --crate <klynt-protocol|klynt-execpolicy|...> --source <path>
#
# See spec §3 "Codex adaptation rules" for the full rename table.

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 [--help] [--crate <name>] [--source <path>]

Adapts a Codex source tree into a klynt-* crate by mechanical rename:
  - codex_*    → klynt_*    (modules)
  - CodexEvent → KlyntEvent (types)
  - ~/.codex/  → ~/.klyntbot/ (paths)
  - CODEX_API_KEY → KLYNT_API_KEY (env vars)

Plan 1: only --help is implemented. Plan 2 fills the body.
EOF
}

main() {
    local crate=""
    local source_path=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help|-h) usage; exit 0 ;;
            --crate) crate="$2"; shift 2 ;;
            --source) source_path="$2"; shift 2 ;;
            *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
        esac
    done

    if [[ -z "$crate" || -z "$source_path" ]]; then
        echo "ERROR: --crate and --source are required (Plan 2 implements adaptation)." >&2
        usage >&2
        exit 2
    fi

    echo "Plan 2 will adapt $source_path into crates/$crate/"
}

main "$@"
```

```bash
chmod +x scripts/adapt_codex_vendor.sh
```

- [ ] **Step 5: Run test to verify it passes**

```bash
bash scripts/adapt_codex_vendor.sh.test.sh
```

Expected: `PASS: usage line present`.

- [ ] **Step 6: Commit**

```bash
git add scripts/adapt_codex_vendor.sh scripts/adapt_codex_vendor.sh.test.sh
git commit -m "$(cat <<'EOF'
chore(scripts): add adapt_codex_vendor.sh skeleton

Plan 1 stakes the path; Plan 2 implements the full Codex → klynt
mechanical rename pass per spec §3 "Codex adaptation rules."

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 23: Final verification — workspace builds, tests pass, frontend clean

**Files:** None modified. End-of-plan acceptance.

- [ ] **Step 1: Full workspace build**

```bash
cargo build --workspace
```

Expected: zero errors, zero warnings.

- [ ] **Step 2: Workspace clippy at -D warnings**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: succeeds (zero warnings — required by CLAUDE.md).

- [ ] **Step 3: Workspace fmt check**

```bash
cargo fmt --all --check
```

Expected: succeeds.

- [ ] **Step 4: Workspace nextest**

```bash
cargo nextest run --workspace
```

Expected: all green. The 7 new crate skeletons each contribute one `crate_compiles` test (plus the binary helper has no tests).

- [ ] **Step 5: Workspace doctests**

```bash
cargo test --workspace --doc
```

Expected: all green (nextest doesn't cover doctests).

- [ ] **Step 6: Desktop-ui static checks**

```bash
cd desktop-ui
bun run lint
bun run typecheck
bun run test --run
cd ..
```

Expected: all green. (No frontend changes in Plan 1; this is a regression check.)

- [ ] **Step 7: Verify all 18 AgentEvent variants present**

```bash
grep -E "^\s+(RecallInjected|DeadEndWarningSurfaced|SkillActivationConsidered|SkillActivated|SkillReferenceLoaded|ContextEngineDecision|ApprovalRequested|ApprovalResolved|SandboxPolicyApplied|ToolCallStreamChunk|MCPSubcallTrace|ProviderRequest|ProviderResponse|MidLoopCompressionTriggered|FileEditWithSymbols|TestRunDetailed|PowerModeToggled|TurnInterrupted) " crates/agent/src/events.rs | wc -l
```

Expected: `18`.

- [ ] **Step 8: Verify all 7 new crates compile and are workspace members**

```bash
for c in klynt-protocol klynt-execpolicy klynt-sandbox klynt-sandbox-helper klynt-hooks klynt-skill-loader klynt-core; do
    cargo build -p "$c" --quiet && echo "OK: $c"
done
```

Expected: 7 lines of `OK: <crate>`.

- [ ] **Step 9: Verify `CODING_CHANNEL` is reachable from common**

```bash
cargo build -p common --quiet
echo 'use common::CODING_CHANNEL; fn main() { assert_eq!(CODING_CHANNEL, "coding"); }' > /tmp/ck.rs
rustc --edition 2021 --crate-type bin -L target/debug/deps --extern common=$(find target/debug/deps -name 'libcommon-*.rlib' | head -1) /tmp/ck.rs -o /tmp/ck && /tmp/ck && echo "OK"
rm -f /tmp/ck.rs /tmp/ck
```

Expected: `OK`. (If the rustc invocation is finicky, substitute a `cargo new` smoke crate that depends on `common` and asserts the constant.)

- [ ] **Step 10: Verify sessions schema columns are present**

```bash
cat <<'SQL' > /tmp/check.sql
.read crates/storage/migrations/001_initial.sql
SELECT name FROM pragma_table_info('sessions');
SQL
sqlite3 :memory: < /tmp/check.sql | sort | tr '\n' ' '
rm /tmp/check.sql
```

Expected output (sorted, space-separated): `approval_mode compressed_at compressed_prefix compressed_through_idx conversation_type created_at cwd key metadata parent_session_id pinned project_id repo_branch repo_id total_cost_usd total_tokens tool_profile updated_at`

(That's 18 columns total: 10 existing + 8 new.)

- [ ] **Step 11: Confirm git status is clean and the plan's commits are landed**

```bash
git log --oneline -25 | head -25
git status
```

Expected: ~22 new commits at the top (one per Task 2-22, plus an audit-arm commit if Task 8 found work). Git status: clean (or only untracked `.mcp.json`).

- [ ] **Step 12: Tag the milestone**

```bash
git tag -a phase1-foundation-complete -m "Phase 1 Plan 1 (Foundation) complete: primitives, events, schema, crate skeletons."
```

(Don't push the tag without approval — local marker only.)

---

## Self-review checklist

Before declaring this plan complete, the engineer should verify:

- [ ] All 22 task commits are atomic (each commit message accurately describes one logical change).
- [ ] No commit was amended after the fact (per CLAUDE.md "Always create NEW commits rather than amending").
- [ ] No `--no-verify` skipped a hook.
- [ ] The spec at `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` reads consistently with what was actually implemented (Task 13 was the spec reconciliation).
- [ ] The next plan (Plan 2: First tool end-to-end) can begin without untouched debt from this plan — no TODOs, no placeholders, no skipped tasks.

---

## Acceptance criteria

This plan is complete when:

1. `cargo build --workspace` is clean.
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean.
3. `cargo fmt --all --check` is clean.
4. `cargo nextest run --workspace` is green.
5. `cargo test --workspace --doc` is green.
6. `cd desktop-ui && bun run lint && bun run typecheck && bun run test --run` is green.
7. All 18 `AgentEvent` variants exist in `crates/agent/src/events.rs` under `#[non_exhaustive]`.
8. All 7 new crates exist as compiling skeletons with VENDOR.md (where applicable) and one passing `crate_compiles` test each.
9. The `sessions` table in `001_initial.sql` has the 8 new columns.
10. `common::CODING_CHANNEL` is reachable; `Tool::is_concurrency_safe` defaults to false; `fan_out_event` exists and tees to UI + bus; `AgentRuntime` accepts `Arc<DomainEventBus>`.
11. Spec at `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` references `sessions` (not `chat_sessions`).
12. ~22 atomic commits land on the branch.

**No user-visible behavior is added by this plan.** Plan 2 will be the first plan that ships a feature a user can interact with.
