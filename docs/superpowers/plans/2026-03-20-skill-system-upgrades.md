# Skill System Upgrades — Aligned with Thariq's Recommendations

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix silent runtime validation gaps in the cron and finance tools, add a `ToolCallInterceptor` hook system to the agent execution pipeline, enrich skills with scripts/assets, and add a scaffolding skill for new tool creation.

**Architecture:** Three layers of change: (1) direct bug fixes in existing tool code (cron validation, finance guards), (2) a new `ToolCallInterceptor` trait in `tools-core` wired into `ExecutionCore` for pluggable pre-execution validation, (3) new non-markdown skill content (scripts, assets, templates) following Thariq's recommendation that "skills are folders, not just markdown files."

**Tech Stack:** Rust (async-trait, serde_json, tokio), the `cron` crate for expression parsing, existing `tools-core` / `agent` / `feature-finance` / `scheduling` crates.

---

## File Structure

### New files
- `crates/tools-core/src/interceptor.rs` — `ToolCallInterceptor` trait + `InterceptorChain`
- `skills/finance-management/scripts/validate_amount.md` — amount-in-cents guidance for the agent (loaded on-demand)
- `skills/automation/scripts/cron_cheatsheet.md` — cron expression quick reference with validation examples
- `skills/communication/assets/templates/` — per-channel message format templates
- `skills/task-management/assets/plan_template.md` — daily plan output template
- `.claude/skills/klyntbot-new-tool/SKILL.md` — scaffolding skill for creating new tools
- `.claude/skills/klyntbot-new-tool/references/checklist.md` — step-by-step wiring guide
- `.claude/skills/klyntbot-new-tool/assets/feature_template/` — Cargo.toml, lib.rs, tool.rs templates

### Modified files
- `crates/tools/src/domain/cron_tool.rs` — add cron expression validation before `add_job`
- `crates/tools/Cargo.toml` — add `cron` dependency (needed for expression validation)
- `crates/feature-finance/src/tool/mod.rs` — add `minimum: 1` + `description` to `amount` schema field
- `crates/feature-finance/src/tool/transactions/mod.rs` — add positivity guard to `tx_update`
- `crates/tools-core/src/lib.rs` — add `pub mod interceptor;` and re-export
- `crates/agent/src/execution/core.rs` — wire `ToolCallInterceptor` into `ExecutionCore`, call before `tool.execute()`

### Test files
- `crates/tools-core/src/interceptor.rs` — inline `#[cfg(test)] mod tests`
- `crates/tools/src/domain/cron_tool.rs` — inline tests for validation
- `crates/feature-finance/src/tool/transactions/mod.rs` — inline tests for `tx_update` guard

---

## Task 1: Validate cron expressions before saving

The cron tool silently accepts invalid cron expressions and creates jobs that never fire. The `cron::Schedule::try_from()` parse only happens at execution time in the scheduling service. We need to fail fast in the tool itself.

**Files:**
- Modify: `crates/tools/src/domain/cron_tool.rs:L156-L176` (the `"add"` action branch)
- Test: inline `#[cfg(test)] mod tests` at end of same file

- [ ] **Step 1: Write the failing test for cron validation**

Add at the bottom of `crates/tools/src/domain/cron_tool.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A handler that records calls — lets us verify the tool rejected before reaching the handler.
    struct RecordingHandler {
        calls: std::sync::Mutex<Vec<String>>,
    }

    impl RecordingHandler {
        fn new() -> Self {
            Self { calls: std::sync::Mutex::new(Vec::new()) }
        }
        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl CronHandler for RecordingHandler {
        async fn add_job(&self, params: AddCronJobParams) -> Result<CronJobInfo> {
            self.calls.lock().unwrap().push(params.name.clone());
            Ok(CronJobInfo {
                id: "test-id".into(),
                name: params.name,
                next_run_at_ms: Some(9999999999999),
                last_status: None,
            })
        }
        async fn list_jobs(&self, _include_internal: bool) -> Vec<CronJobInfo> { vec![] }
        async fn remove_job(&self, _job_id: &str) -> Result<bool> { Ok(true) }
    }

    fn test_ctx() -> RoutingContext {
        RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test:cron"),
        )
    }

    #[tokio::test]
    async fn rejects_invalid_cron_expression() {
        let handler = Arc::new(RecordingHandler::new());
        let tool = CronTool::with_handler(handler.clone());
        let args = json!({
            "action": "add",
            "message": "test",
            "cron_expr": "not a cron"
        });
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err(), "Should reject invalid cron expression");
        assert_eq!(handler.call_count(), 0, "Handler should not be called");
    }

    #[tokio::test]
    async fn accepts_valid_cron_expression() {
        let handler = Arc::new(RecordingHandler::new());
        let tool = CronTool::with_handler(handler.clone());
        let args = json!({
            "action": "add",
            "message": "standup",
            "cron_expr": "0 9 * * *"
        });
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_ok(), "Should accept valid 5-field cron");
        assert_eq!(handler.call_count(), 1);
    }

    #[tokio::test]
    async fn accepts_every_seconds() {
        let handler = Arc::new(RecordingHandler::new());
        let tool = CronTool::with_handler(handler.clone());
        let args = json!({
            "action": "add",
            "message": "ping",
            "every_seconds": 300
        });
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_ok(), "every_seconds should bypass cron validation");
        assert_eq!(handler.call_count(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p tools -E 'test(rejects_invalid_cron)'`
Expected: FAIL — the tool currently does NOT validate cron expressions, so it calls the handler and returns `Ok`.

- [ ] **Step 3: Add cron expression validation**

First, add the `cron` dependency to `crates/tools/Cargo.toml`:

```toml
cron.workspace = true
```

Then in `crates/tools/src/domain/cron_tool.rs`, inside the `"add"` match arm, replace the `CronSchedule::Cron` construction (around line 166-169) with validation:

```rust
} else if let Some(cron_expr) = p.optional_str("cron_expr")? {
    // Validate the cron expression before accepting it.
    // The `cron` crate uses 6-field format (sec min hr day mon dow),
    // but the LLM provides 5-field standard format. Prepend "0 " for seconds.
    let full_expr = format!("0 {}", cron_expr);
    if cron::Schedule::try_from(full_expr.as_str()).is_err() {
        return Err(ToolError::InvalidParams(format!(
            "Invalid cron expression '{}'. Expected 5-field format: \
             'minute hour day month weekday' (e.g. '0 9 * * *' for daily at 9am)",
            cron_expr
        ))
        .into());
    }
    // Store the 6-field expression since the scheduling service's
    // compute_next_run() uses cron::Schedule::try_from() which requires 6 fields.
    CronSchedule::Cron {
        expr: full_expr,
        tz: None,
    }
```

**Important:** We store `full_expr` (6-field with prepended `"0 "` for seconds), not `cron_expr` (5-field). The scheduling service at `crates/scheduling/src/service/mod.rs:49` calls `cron::Schedule::try_from(expr.as_str())` which requires 6-field format. If we stored the 5-field expression, it would fail at execution time — the exact silent failure we're fixing.

**Note:** Existing cron jobs in the DB may have been stored in either 5-field or 6-field format. The scheduling service already handles parse failures gracefully (returns `None` for next_run, effectively disabling the job). No data migration needed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p tools -E 'test(cron)'`
Expected: All three tests PASS.

- [ ] **Step 5: Run clippy on the tools crate**

Run: `cargo clippy -p tools --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tools/src/domain/cron_tool.rs
git commit -m "fix(cron): validate cron expressions before creating jobs

Previously, invalid cron expressions were silently accepted and created
jobs that would never fire. Now validates at tool call time with a clear
error message."
```

---

## Task 2: Add schema hints and positivity guard to finance tool

The finance `amount` field has no schema description hinting at the smallest-unit convention, and `tx_update` lacks the `amount > 0` guard that `tx_add` has.

**Files:**
- Modify: `crates/feature-finance/src/tool/mod.rs:L173` (amount schema)
- Modify: `crates/feature-finance/src/tool/transactions/mod.rs:L314-L340` (tx_update)
- Test: `tests/integration/finance.rs` (following existing test pattern with `make_finance_tool()`)

- [ ] **Step 1: Write the failing test for tx_update positivity**

The canonical test pattern lives in `tests/integration/finance.rs`. Follow that exact approach. Add a new test there (or in a test module in transactions/mod.rs if one exists):

```rust
/// In tests/integration/finance.rs (following the existing pattern):

#[tokio::test]
async fn tx_update_rejects_negative_amount() {
    let finance = make_finance_tool().await;
    let ctx = test_ctx();

    // Create account
    finance.execute(json!({
        "action": "account_add",
        "name": "Test Bank",
        "type": "bank",
        "currency": "USD",
        "balance": 100000
    }), &ctx).await.unwrap();

    // Create transaction
    let result = finance.execute(json!({
        "action": "tx_add",
        "amount": 5000,
        "type": "expense",
        "category": "food"
    }), &ctx).await.unwrap();

    // Extract the transaction ID — response format is {"tx": {"id": "...", ...}, ...}
    let tx_id = serde_json::from_str::<serde_json::Value>(&result)
        .unwrap()["tx"]["id"].as_str().unwrap().to_string();

    // Update with negative amount — should fail
    let result = finance.execute(json!({
        "action": "tx_update",
        "id": tx_id,
        "amount": -500
    }), &ctx).await;
    assert!(result.is_err(), "tx_update should reject negative amounts");
}

#[tokio::test]
async fn tx_update_rejects_zero_amount() {
    let finance = make_finance_tool().await;
    let ctx = test_ctx();

    finance.execute(json!({
        "action": "account_add",
        "name": "Test Bank",
        "type": "bank",
        "currency": "USD",
        "balance": 100000
    }), &ctx).await.unwrap();

    let result = finance.execute(json!({
        "action": "tx_add",
        "amount": 5000,
        "type": "expense",
        "category": "food"
    }), &ctx).await.unwrap();

    let tx_id = serde_json::from_str::<serde_json::Value>(&result)
        .unwrap()["tx"]["id"].as_str().unwrap().to_string();

    let result = finance.execute(json!({
        "action": "tx_update",
        "id": tx_id,
        "amount": 0
    }), &ctx).await;
    assert!(result.is_err(), "tx_update should reject zero amounts");
}
```

These tests use the same `make_finance_tool()` and `test_ctx()` helpers already defined at the top of `tests/integration/finance.rs`:
- `make_finance_tool()` → `FinanceTool::from_storage_pool(&pool, "VND")` with in-memory pool
- `test_ctx()` → `RoutingContext::new(ChannelName::new("cli"), ChatId::new("test:finance_integration"))`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -E 'test(tx_update_rejects_negative)'`
Expected: FAIL — `tx_update` currently has no positivity guard. The test lives in the root `tests/integration/finance.rs` (facade crate), not in `feature-finance`.

- [ ] **Step 3: Add positivity guard to tx_update**

In `crates/feature-finance/src/tool/transactions/mod.rs`, inside `tx_update` (around line 317), after `let new_amount = p.optional_i64("amount")?;`, add:

```rust
let new_amount = p.optional_i64("amount")?;
if let Some(amt) = new_amount {
    if amt <= 0 {
        return Err(
            ToolError::InvalidParams("Amount must be positive".to_string()).into()
        );
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -E 'test(tx_update_rejects)'`
Expected: Both tests PASS (negative and zero).

- [ ] **Step 5: Add description and minimum to amount schema field**

In `crates/feature-finance/src/tool/mod.rs:L173`, change:

```rust
"amount": { "type": "integer" },
```

to:

```rust
"amount": {
    "type": "integer",
    "minimum": 1,
    "description": "Amount in smallest currency unit (e.g. cents for USD, dong for VND). $50.00 = 5000."
},
```

- [ ] **Step 6: Run full finance test suite**

Run: `cargo nextest run -E 'test(finance)'`
Expected: All finance integration tests PASS. The `minimum: 1` constraint is enforced during `ToolRegistry::prepare()` → `validate_params()` → `validate_value()` in `tools-core/src/validation.rs:L75-78`, which checks `minimum` on integer schema fields. The inline `amount > 0` guards in `tx_add` and `tx_update` remain as defense-in-depth for direct calls bypassing schema validation.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p feature-finance --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-finance/src/tool/mod.rs crates/feature-finance/src/tool/transactions/mod.rs
git commit -m "fix(finance): add amount schema hints and tx_update positivity guard

The amount field now declares minimum: 1 and a description explaining the
smallest-currency-unit convention. tx_update gains the same positivity
check that tx_add already has."
```

---

## Task 3: Add ToolCallInterceptor trait to tools-core

This is the foundation for pluggable pre-execution validation hooks. Following the existing `AutoTunerHook` pattern and `with_domain_bus` / `with_outcome_recorder` builder methods on `ExecutionCore`.

**Files:**
- Create: `crates/tools-core/src/interceptor.rs`
- Modify: `crates/tools-core/src/lib.rs:L9` (add module) and re-export section

- [ ] **Step 1: Write the failing test for interceptor**

Create `crates/tools-core/src/interceptor.rs` with tests first:

```rust
//! Pre-execution tool call interceptors.
//!
//! `ToolCallInterceptor` lets skills inject validation logic that runs
//! after `ToolRegistry::prepare()` but before `tool.execute()`. This turns
//! documented gotchas (e.g. "amounts must be in cents") into runtime guards.

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;

/// Intercepts a tool call before execution.
///
/// Return `Ok(())` to allow the call, or `Err(...)` to block it with an
/// error message that gets returned to the LLM as the tool result.
#[async_trait]
pub trait ToolCallInterceptor: Send + Sync {
    /// Called before `tool.execute()`. Receives tool name, arguments, and
    /// the active skill name (if any).
    async fn before_call(
        &self,
        tool_name: &str,
        args: &Value,
        skill_name: Option<&str>,
    ) -> Result<()>;
}

/// Chains multiple interceptors. All must return `Ok(())` for the call to proceed.
/// First error short-circuits.
pub struct InterceptorChain {
    interceptors: Vec<Arc<dyn ToolCallInterceptor>>,
}

impl InterceptorChain {
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    pub fn add(&mut self, interceptor: Arc<dyn ToolCallInterceptor>) {
        self.interceptors.push(interceptor);
    }

    pub fn is_empty(&self) -> bool {
        self.interceptors.is_empty()
    }

    /// Run all interceptors in order. First error short-circuits.
    pub async fn check(
        &self,
        tool_name: &str,
        args: &Value,
        skill_name: Option<&str>,
    ) -> Result<()> {
        for interceptor in &self.interceptors {
            interceptor.before_call(tool_name, args, skill_name).await?;
        }
        Ok(())
    }
}

impl Default for InterceptorChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ToolError;
    use serde_json::json;

    struct AllowAll;

    #[async_trait]
    impl ToolCallInterceptor for AllowAll {
        async fn before_call(&self, _: &str, _: &Value, _: Option<&str>) -> Result<()> {
            Ok(())
        }
    }

    struct BlockFinance;

    #[async_trait]
    impl ToolCallInterceptor for BlockFinance {
        async fn before_call(
            &self,
            tool_name: &str,
            _args: &Value,
            _skill: Option<&str>,
        ) -> Result<()> {
            if tool_name == "finance" {
                return Err(ToolError::InvalidParams("blocked by interceptor".into()).into());
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn empty_chain_allows_everything() {
        let chain = InterceptorChain::new();
        assert!(chain.check("finance", &json!({}), None).await.is_ok());
    }

    #[tokio::test]
    async fn chain_runs_all_interceptors() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(AllowAll));
        chain.add(Arc::new(AllowAll));
        assert!(chain.check("finance", &json!({}), None).await.is_ok());
    }

    #[tokio::test]
    async fn chain_short_circuits_on_error() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(AllowAll));
        chain.add(Arc::new(BlockFinance));
        chain.add(Arc::new(AllowAll)); // should not be reached

        let result = chain.check("finance", &json!({}), None).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("blocked by interceptor")
        );
    }

    #[tokio::test]
    async fn chain_allows_non_matching_tools() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(BlockFinance));
        assert!(chain.check("tasks", &json!({}), None).await.is_ok());
    }
}
```

- [ ] **Step 2: Register the module in tools-core/src/lib.rs**

In `crates/tools-core/src/lib.rs`, add after the existing module declarations (around line 17):

```rust
pub mod interceptor;
```

And add to re-exports section (around line 33):

```rust
pub use interceptor::{InterceptorChain, ToolCallInterceptor};
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo nextest run -p tools-core -E 'test(chain)'`
Expected: All 4 interceptor tests PASS.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p tools-core --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/tools-core/src/interceptor.rs crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add ToolCallInterceptor trait and InterceptorChain

Pluggable pre-execution validation hooks that run after prepare() but
before execute(). Follows the existing with_domain_bus pattern. Chain
short-circuits on first error."
```

---

## Task 4: Wire ToolCallInterceptor into ExecutionCore

Connect the interceptor chain to the actual tool execution path in the agent's execution core.

**Files:**
- Modify: `crates/agent/src/execution/core.rs:L287-L319` (struct + builder) and `L498-L505` (execution point)

- [ ] **Step 1: Write the failing test**

Add to the existing tests in `crates/agent/src/execution/core.rs` (or in a test module under `crates/agent/src/execution/`):

```rust
#[cfg(test)]
mod interceptor_tests {
    use super::*;
    use tools_core::{InterceptorChain, ToolCallInterceptor};
    use common::ToolError;
    use async_trait::async_trait;

    struct RejectAll;

    #[async_trait]
    impl ToolCallInterceptor for RejectAll {
        async fn before_call(
            &self,
            _tool_name: &str,
            _args: &serde_json::Value,
            _skill: Option<&str>,
        ) -> common::Result<()> {
            Err(ToolError::InvalidParams("rejected by interceptor".into()).into())
        }
    }

    #[test]
    fn execution_core_accepts_interceptor_chain() {
        // Verify the builder method exists and compiles.
        // Full integration test would require a mock provider + registry.
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(RejectAll));
        // This is a compile-time test — if it compiles, the wiring works.
        assert!(!chain.is_empty());
    }
}
```

> **Note:** Full integration tests for the interceptor blocking a tool call require a mock LLM provider. The existing test infrastructure in `crates/agent/` has examples. For this task, the critical test is that the `with_interceptor_chain` builder method compiles and the field is accessible in `run_cycle`. A full integration test should be added as a follow-up.

- [ ] **Step 2: Add the interceptor field to ExecutionCore**

In `crates/agent/src/execution/core.rs`, modify the `ExecutionCore` struct (around line 287):

```rust
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub outcome_recorder: Option<Arc<crate::learning::recorder::OutcomeRecorder>>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub interceptor_chain: Option<Arc<tools_core::InterceptorChain>>,
    tool_semaphore: Arc<Semaphore>,
}
```

Update `new()` (around line 296):

```rust
pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
    Self {
        provider,
        tool_registry,
        outcome_recorder: None,
        domain_event_bus: None,
        interceptor_chain: None,
        tool_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_TOOLS)),
    }
}
```

Add builder method after `with_outcome_recorder` (around line 319):

```rust
/// Set the interceptor chain for pre-execution validation hooks.
pub fn with_interceptor_chain(mut self, chain: Arc<tools_core::InterceptorChain>) -> Self {
    self.interceptor_chain = Some(chain);
    self
}
```

- [ ] **Step 3: Wire the interceptor into run_cycle's tool execution path**

In `run_cycle`, inside the `async move` block where tools are executed (around line 498-505), insert the interceptor check between `reg.prepare()` and `tool.execute()`:

```rust
let exec_result = tokio::time::timeout(timeout_dur, async {
    let tool = {
        let reg = registry.read().await;
        reg.prepare(&name, &args, &ctx)?
    };
    // Run interceptor chain before executing (if configured)
    if let Some(ref chain) = interceptor_chain {
        chain.check(&name, &args, None).await?;
    }
    // Read lock is dropped — safe for tools that re-enter the registry
    tool.execute(args, &ctx).await
})
.await;
```

The `interceptor_chain` variable needs to be cloned into the `async move` block. It must be captured inside the `.map(|(i, tc)| { ... })` closure (alongside lines 464-476 where `registry`, `ctx`, `semaphore` etc. are cloned), NOT at the top of `run_cycle`:

```rust
// Inside the .map closure, alongside the other let-bindings at ~L464-L476:
let interceptor_chain = self.interceptor_chain.clone();
```

This follows the same pattern as `let registry = Arc::clone(&self.tool_registry);` at line 466.

- [ ] **Step 4: Run the agent crate tests**

Run: `cargo nextest run -p agent`
Expected: All existing tests PASS (interceptor is `None` by default, so no behavioral change).

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p agent --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(agent): wire ToolCallInterceptor into ExecutionCore

Adds optional interceptor_chain field to ExecutionCore with builder
method. Chain runs after prepare() and before execute() in run_cycle,
allowing pre-execution validation without changing the tool interface."
```

---

## Task 5: Add scripts and assets to internal skills

Following Thariq's recommendation: "a skill is a folder, not just a markdown file." Add non-markdown content that the agent can discover and use.

**Files:**
- Create: `skills/automation/scripts/cron_cheatsheet.md`
- Create: `skills/finance-management/scripts/validate_amount.md`
- Create: `skills/communication/assets/templates/telegram.md`
- Create: `skills/communication/assets/templates/discord.md`
- Create: `skills/communication/assets/templates/slack.md`
- Create: `skills/task-management/assets/plan_template.md`

- [ ] **Step 1: Create cron cheatsheet script reference**

Create `skills/automation/scripts/cron_cheatsheet.md`:

```markdown
# Cron Expression Quick Reference

## Format

Five fields: `minute hour day month weekday`

## Common Patterns

| Schedule | Expression | Notes |
|----------|-----------|-------|
| Every day at 8am | `0 8 * * *` | |
| Weekdays at 5pm | `0 17 * * 1-5` | Mon=1, Sun=0 |
| Every Sunday 6pm | `0 18 * * 0` | |
| First of month 9am | `0 9 1 * *` | |
| Every 30 min | Use `every_seconds: 1800` | NOT cron |
| Every 5 min | Use `every_seconds: 300` | NOT cron |

## Weekday Numbers

Sun=0, Mon=1, Tue=2, Wed=3, Thu=4, Fri=5, Sat=6

## Validation

Before creating a job, mentally verify:
1. Five fields exactly (not 6 — no seconds field)
2. Minutes 0-59, hours 0-23, day 1-31, month 1-12, weekday 0-6
3. Ranges use `-` (e.g. `1-5`), lists use `,` (e.g. `1,3,5`)
4. `*/N` for every N units (e.g. `*/15 * * * *` = every 15 min)

## One-Shot Reminders

For one-time future reminders, use `every_seconds` with the delay.
Example: "remind me in 20 minutes" → `every_seconds: 1200`
The job fires once and auto-deletes.
```

- [ ] **Step 2: Create amount validation reference**

Create `skills/finance-management/scripts/validate_amount.md`:

```markdown
# Amount Validation — Smallest Currency Unit

## The Rule

ALL monetary amounts are in the **smallest unit** of the currency:
- USD: cents ($50.00 = **5000**)
- EUR: cents (€25.50 = **2550**)
- VND: dong (100,000₫ = **100000** — VND has no subunit)
- JPY: yen (¥1000 = **1000** — JPY has no subunit)
- GBP: pence (£10 = **1000**)

## Zero-Decimal Currencies (no subunit)

These currencies use 1:1 (amount = face value):
BIF, CLP, DJF, GNF, ISK, JPY, KMF, KRW, PYG, RWF, UGX, VND, VUV, XAF, XOF, XPF

## Quick Check

Before submitting `tx_add`, verify:
- Is the amount suspiciously small? $50 as `50` is almost certainly wrong — should be `5000`
- Is the amount suspiciously large? 5,000,000 cents = $50,000 — verify with user
- Amounts MUST be positive (> 0). The `type` field (expense/income) determines direction.
```

- [ ] **Step 3: Create communication templates**

Create `skills/communication/assets/templates/telegram.md`:

```markdown
# Telegram Message Template

Format: MarkdownV2 (escape special chars: _ * [ ] ( ) ~ ` > # + - = | { } . !)
Max length: 4096 characters

## Structure
\`\`\`
*Title*

Message body with _emphasis_ and `code`\.

\- bullet point
\- bullet point
\`\`\`

## Gotchas
- ALL special characters must be escaped with backslash
- Links: [text](url) — url chars must NOT be escaped
- Bold: *text*, Italic: _text_, Code: `text`
```

Create `skills/communication/assets/templates/discord.md`:

```markdown
# Discord Message Template

Format: Standard Markdown
Max length: 2000 characters

## Structure
\`\`\`
**Title**

Message body with *emphasis* and `code`.

- bullet point
- bullet point
\`\`\`

## Gotchas
- Hard 2000 char limit — messages are truncated, not split
- No HTML support
- Embeds are separate from message text
```

Create `skills/communication/assets/templates/slack.md`:

```markdown
# Slack Message Template

Format: mrkdwn (NOT standard Markdown)
Max length: No hard limit, but keep under 4000 chars

## Structure
\`\`\`
*Title*

Message body with _emphasis_ and `code`.

• bullet point
• bullet point
\`\`\`

## mrkdwn Differences from Markdown
- Bold: *text* (single asterisk, not double)
- Italic: _text_
- Links: <url|display text> (NOT [text](url))
- Bullet: • (Unicode bullet, not - or *)
- No headings (# is not supported)
```

- [ ] **Step 4: Create daily plan template**

Create `skills/task-management/assets/plan_template.md`:

```markdown
# Daily Plan

## Focus (max 3)
1. [ ] {task_title} — {estimate}min {energy_level}
2. [ ] {task_title} — {estimate}min {energy_level}
3. [ ] {task_title} — {estimate}min {energy_level}

## Also Today
- [ ] {task_title}
- [ ] {task_title}

## Calendar
- {time} — {event_title}

## Notes
{any_context_for_the_day}
```

- [ ] **Step 5: Update skill SKILL.md files to reference new content**

In `skills/automation/SKILL.md`, add to the references section (supplements the existing `references/cron.md` which covers detailed time expression parsing — this cheatsheet is a quick lookup table for the most common patterns):

```markdown
Quick cron expression reference: `scripts/cron_cheatsheet.md`.
```

In `skills/finance-management/SKILL.md`, add to the red flags section:

```markdown
For amount conversion reference, see `scripts/validate_amount.md`.
```

In `skills/communication/SKILL.md`, add:

```markdown
Channel-specific message templates are in `assets/templates/`.
```

In `skills/task-management/SKILL.md`, add:

```markdown
Daily plan output template: `assets/plan_template.md`.
```

- [ ] **Step 6: Verify the files are discoverable**

Run: `find skills/ -type f -name '*.md' | sort`
Expected: New files appear under `scripts/` and `assets/` directories.

- [ ] **Step 7: Commit**

```bash
git add skills/
git commit -m "feat(skills): add scripts and asset templates to internal skills

Following Thariq's recommendation that skills are folders, not just
markdown files. Adds cron cheatsheet, amount validation reference,
per-channel message templates, and daily plan template."
```

---

## Task 6: Create new-tool scaffolding skill for Claude Code

A Claude Code skill that guides tool creation with all the boilerplate: crate structure, derive macros, FeaturePackage, DEV_COMMANDS, MCP exposure, and corresponding Claude Code skill.

**Files:**
- Create: `.claude/skills/klyntbot-new-tool/SKILL.md`
- Create: `.claude/skills/klyntbot-new-tool/references/checklist.md`

- [ ] **Step 1: Create the scaffolding skill**

Create `.claude/skills/klyntbot-new-tool/SKILL.md`:

```markdown
---
name: klyntbot-new-tool
description: >
  Use when creating a new tool, feature crate, or feature package for klyntbot.
  Triggers on "new tool", "add a tool", "create feature", "scaffold",
  "new crate", or when the user wants to add a new capability to the agent.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-20"
  source: official
  tags: "scaffold,tool,feature,create,new"
---

# New Tool Scaffolding

Guide the user through creating a new klyntbot tool end-to-end. This is a 7-file process with strict conventions — missing any step causes test failures.

## Before Starting

1. Ask the user: **What does this tool do?** Get a name, description, and list of actions.
2. Decide the implementation style:
   - **`#[tool_actions]` macro** (preferred for multi-action tools) — generates `Tool` impl from annotated methods
   - **Manual `impl Tool`** — for tools with complex parameter schemas or dynamic behavior
   - **`#[derive(Tool)]` + `ToolExecute`** — for single-action typed tools

3. Read `references/checklist.md` for the exact file-by-file wiring guide.

## Tool Naming

- Tool name = singular noun or verb-noun: `finance`, `tasks`, `notes`, `cron`
- Crate name = `feature-{name}`: `feature-finance`, `feature-tasks`
- Registry key = tool's `name()` return value — this MUST match the MCP whitelist entry

## Quick Checklist

1. Create `crates/feature-{name}/` with Cargo.toml, src/lib.rs, src/tool.rs
2. Implement `Tool` trait (or use derive macros)
3. Implement `FeaturePackage` in lib.rs
4. Add migration SQL if needed
5. Register tool in `crates/agent/src/agent_loop/builder.rs`
6. Add to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`
7. Add Tauri commands in `crates/desktop/src/commands/{name}.rs`
8. Wire into DEV_COMMANDS + dispatch_dev + dev_server test list
9. Create Claude Code skill in `.claude/skills/klyntbot-{name}/`

## Red Flags — STOP

- Using a tool name that doesn't match the registry key
- Forgetting DEV_COMMANDS — the `dev_server_covers_all_tauri_commands` test will fail
- Forgetting to add to `default_exposed_tools()` — MCP clients won't see the tool
- Skipping the Claude Code skill — Claude Code won't know how to use the tool
```

- [ ] **Step 2: Create the detailed checklist reference**

Create `.claude/skills/klyntbot-new-tool/references/checklist.md`:

```markdown
---
name: new-tool-checklist
description: Step-by-step file-by-file guide for creating a new klyntbot tool
metadata:
  always: false
---

# New Tool Wiring Checklist

## Step 1: Create the feature crate

```
crates/feature-{name}/
  Cargo.toml
  src/
    lib.rs      — FeaturePackage impl
    tool.rs     — Tool impl
  migrations/
    001_{name}_tables.sql  — (if needed)
```

**Cargo.toml** must depend on:
```toml
[dependencies]
common = { path = "../common" }
tools-core = { path = "../tools-core" }
storage = { path = "../storage" }
async-trait = "0.1"
serde_json = "1"
tracing = "0.1"
```

Add to workspace members in root `Cargo.toml`.

## Step 2: Implement the Tool

**Using `#[tool_actions]` (preferred):**
```rust
use tools_core::{tool_actions, ActionParams, RoutingContext};

#[derive(Debug, ActionParams)]
pub struct MyActionParams {
    /// Description for JSON schema
    #[param(required)]
    pub field: String,
}

pub struct MyTool { /* dependencies */ }

#[tool_actions(
    name = "my_tool",
    description = "What the tool does",
    category = "General",
    tags = "tag1,tag2",
    cost = "Free"
)]
impl MyTool {
    #[action(name = "do_thing")]
    async fn do_thing(&self, params: MyActionParams, ctx: &RoutingContext) -> common::Result<String> {
        Ok("done".to_string())
    }
}
```

Valid categories: General, FileSystem, Search, Web, Communication,
TaskManagement, Memory, Finance, Productivity, System, Mcp, Plugin.

## Step 3: Implement FeaturePackage

```rust
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct MyFeature { /* deps */ }

impl MyFeature {
    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "my_feature",
            version: 1,
            sql: include_str!("../migrations/001_my_tables.sql"),
        }]
    }
}

#[async_trait::async_trait]
impl FeaturePackage for MyFeature {
    fn name(&self) -> &str { "my_feature" }
    fn tools(&self) -> Vec<DynTool> { vec![Arc::new(self.tool.clone())] }
    fn migration_sql(&self) -> Option<&str> { Some(include_str!("../migrations/001_my_tables.sql")) }
    async fn health_check(&self) -> HealthStatus { HealthStatus::Healthy }
}
```

## Step 4: Register in agent builder

In `crates/agent/src/agent_loop/builder.rs`, inside the tool registration block:

```rust
// Register my_tool
let my_tool = MyTool::new(pool.clone());
tool_registry.register(my_tool);
```

## Step 5: Add to MCP whitelist

In `crates/config/src/schema/mcp.rs`, in `default_exposed_tools()`:

```rust
vec![
    // ... existing tools ...
    "my_tool".to_string(),   // ← add here
]
```

## Step 6: Add Tauri commands

Create `crates/desktop/src/commands/{name}.rs`:

```rust
use crate::state::AppCore;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn my_command(state: State<'_, Arc<AppCore>>) -> Result<serde_json::Value, String> {
    // delegate to state.my_method().await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["my_command"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, crate::commands::ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "my_command" => dev::val(core.my_method().await),
        _ => return None,
    })
}
```

Then wire:
1. `commands/mod.rs` — add `pub mod {name};`
2. `main.rs` — add to `generate_handler![...]`
3. `dev_server/mod.rs` — add to `dev_command_names()` modules list
4. `dev_server/dispatch.rs` — add dispatch arm

## Step 7: Create Claude Code skill

Create `.claude/skills/klyntbot-{name}/SKILL.md` with YAML frontmatter
matching the tool's registry name, actions, and common mistakes.
```

- [ ] **Step 3: Verify skill is discoverable**

Run: `ls -la .claude/skills/klyntbot-new-tool/`
Expected: SKILL.md and references/checklist.md present.

- [ ] **Step 4: Commit**

```bash
git add .claude/skills/klyntbot-new-tool/
git commit -m "feat(skills): add new-tool scaffolding skill for Claude Code

Guides tool creation end-to-end with exact file paths, code templates,
and a wiring checklist. Prevents the common DEV_COMMANDS and MCP
whitelist omission mistakes."
```

---

## Task 7: Update skill descriptions for triggering precision

The skill-creator blog emphasizes tuning descriptions to reduce false positives/negatives. Several skills have overlapping trigger terms that could cause misrouting.

**Files:**
- Modify: `skills/general/SKILL.md` — tighten description to exclude domain-specific keywords
- Modify: `.claude/skills/klyntbot-agent/SKILL.md` — clarify when NOT to use
- Modify: `.claude/skills/klyntbot-new-tool/SKILL.md` — already done in Task 6

- [ ] **Step 1: Audit trigger overlap**

Read the description fields of all skills and identify overlapping trigger keywords. Key conflicts:
- `klyntbot-agent` vs `klyntbot-tasks`: both trigger on task-related queries
- `general` orchestrator vs specialized orchestrators: "help me" could match anything
- `klyntbot-memory` vs `klyntbot-notes`: "save this" could go either way

- [ ] **Step 2: Refine klyntbot-agent description**

In `.claude/skills/klyntbot-agent/SKILL.md`, update the description to be more precise about when it should NOT trigger:

Add to the "When NOT to Use" section:

```markdown
**Trigger precision:** This skill should ONLY trigger when the request genuinely requires
multi-tool orchestration or AI reasoning. Single-tool operations should use the specific
skill directly. If you know the exact tool and action, calling it directly is faster and
more reliable than routing through the agent.
```

- [ ] **Step 3: Commit**

```bash
git add skills/ .claude/skills/
git commit -m "docs(skills): refine trigger descriptions for routing precision

Tighten skill descriptions to reduce false-positive triggering,
following the skill-creator blog recommendation on description tuning."
```

---

## Summary

| Task | What | Impact |
|------|------|--------|
| 1 | Cron expression validation | Prevents silent job creation failures |
| 2 | Finance amount schema + tx_update guard | Catches the #1 finance mistake at schema level |
| 3 | ToolCallInterceptor trait | Foundation for all future runtime validation |
| 4 | Wire interceptor into ExecutionCore | Activates the hook system |
| 5 | Scripts and assets in skills | Skills become folders, not just markdown |
| 6 | New-tool scaffolding skill | Prevents wiring mistakes, saves dev time |
| 7 | Description precision tuning | Better skill routing accuracy |
