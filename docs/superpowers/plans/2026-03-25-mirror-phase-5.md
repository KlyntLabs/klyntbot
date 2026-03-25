# The Mirror Phase 5 — "The Mirror Connects Everything"

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the final Mirror phase — cross-feature ripple (episodic memory, auto-notes), MCP tool exposure, SSE live updates, trial preview cleanup, shutdown wiring, and the `EarlyTrialEvaluator` stub upgrade — completing the full spec.

**Architecture:** The facade gains an `EpisodicMemoryRepo` to write episodic entries on key actions (narrative generated, meta-rule approved, trial killed). Trial kill also emits a `DomainEvent::MirrorTrialKilled` so `app-core` can create an auto-note (cognitive L3-L4 cannot depend on feature-notes L4 directly). A new `MirrorTool` (multi-action `#[derive(Tool)]`) exposes read-only Mirror data via MCP. SSE entity updates are wired by returning `Vec<EntityUpdate>` from Tauri commands that mutate Mirror state, including a new `EntityKind::BrainVersion` for timeline live updates. The cleanup cron gains trial preview cleanup. The MirrorEngine shutdown token is stored in `AppCore` for graceful shutdown.

**Known spec gaps deferred:** Daily midnight routing aggregation (`window_hours=24`) and `EarlyTrialEvaluator` real implementation are acknowledged but not included — the hourly flush is sufficient for current usage, and the evaluator requires MetricSource integration that needs separate design work.

**Tech Stack:** Rust (cognitive/app-core/desktop/tools crates), SQLite, React + Tailwind v4 (desktop-ui)

**Spec:** `docs/superpowers/specs/2026-03-25-mirror-self-reflection-layer-design.md` — Cross-feature ripple (lines 538-543), MCP exposure (line 536), SSE updates (lines 574-579), Retention (line 314)

**Depends on:** Phase 1-4 complete (all 4 subscribers, MirrorEngine, MirrorFacade, ExperimentWatchlist UI)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/tools/src/domain/mirror.rs` | `MirrorTool` — multi-action Tool exposing Mirror data via MCP |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/mirror/facade.rs` | Add episodic memory writing on key actions; add `create_meta_rule_from_text`; accept `EpisodicMemoryRepo` |
| `crates/cognitive/src/mirror/types.rs` | Add `proposed_meta_rule` to `MirrorResponse` |
| `crates/cognitive/src/mirror/engine.rs` | Return shutdown token for external storage; accept `EpisodicMemoryRepo` |
| `crates/bus/src/domain_events.rs` | Add `MirrorTrialKilled` and `MirrorBrainVersionCreated` variants |
| `crates/desktop-shared/src/types.rs` | Add `BrainVersion` variant to `EntityKind` |
| `crates/app-core/src/init/mod.rs` | Store mirror handles + shutdown token; pass episodic repo to engine; wire trial preview cleanup + auto-note on trial kill |
| `crates/app-core/src/state.rs` | Store `_mirror_handles` and `_mirror_shutdown` in `AppCore` |
| `crates/desktop/src/commands/mirror.rs` | Return `EntityUpdate` for approve/dismiss/kill/continue actions |
| `crates/desktop/src/commands/mod.rs` | Import `EntityUpdate` for mirror commands |
| `crates/tools/src/domain/mod.rs` | Export mirror module |
| `crates/tools/src/lib.rs` or `crates/agent/src/agent_builder.rs` | Register MirrorTool (wired with `Arc<MirrorFacade>` from AppCore) |
| `crates/config/src/schema/mcp.rs` | Add `"mirror"` to `default_exposed_tools()` |

---

## Task 1: Store MirrorEngine Shutdown Token + Handles

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`

The MirrorEngine's shutdown token and join handles are currently dropped at the call site (`_handles`, `_mirror_shutdown`). This means Mirror subscribers can't be gracefully shut down. Fix by storing them in `AppCore`.

- [ ] **Step 1: Add fields to AppCore**

In `crates/app-core/src/state.rs`, add after `mirror_facade`:

```rust
/// Mirror subscriber join handles (kept alive for subscriber lifecycle).
pub _mirror_handles: Option<Vec<tokio::task::JoinHandle<()>>>,
/// Mirror shutdown token (cancelled on app shutdown).
pub _mirror_shutdown: Option<tokio_util::sync::CancellationToken>,
```

- [ ] **Step 2: Update init/mod.rs to store them**

In `crates/app-core/src/init/mod.rs`, the mirror init block (line ~252-289) creates the facade inside a `let mirror_facade = { ... }` block. The handles and shutdown token must be hoisted OUT of this inner block so they survive until `AppCore` construction (line ~302).

Change the structure from:
```rust
let mirror_facade = {
    // ...
    let (facade, _handles, _mirror_shutdown) = MirrorEngine::start(...);
    // ...
    Some(Arc::new(facade))
};
```

To:
```rust
let (mirror_facade, mirror_handles, mirror_shutdown) = {
    // ...
    let (facade, handles, shutdown) = MirrorEngine::start(...);
    // ...
    (Some(Arc::new(facade)), Some(handles), Some(shutdown))
};
```

Then in the `AppCore` struct construction, assign:
```rust
_mirror_handles: mirror_handles,
_mirror_shutdown: mirror_shutdown,
```

- [ ] **Step 3: Build and verify**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git commit -m "fix(mirror): store MirrorEngine handles and shutdown token in AppCore"
```

---

## Task 2: Add Trial Preview Cleanup to Cron Job

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

The `JOB_MIRROR_CLEANUP` cron calls `cleanup_old_snapshots(90)` and `cleanup_old_snippets(90)` but does NOT call `cleanup_old_trial_previews()`. The spec says trial previews should be cleaned after 90 days.

- [ ] **Step 1: Add trial preview cleanup**

In the `JOB_MIRROR_CLEANUP` handler (around line 467), add after `cleanup_old_snippets`:

```rust
let preview_count = mirror_repo.cleanup_old_trial_previews(90).await.unwrap_or(0);
```

Update the return format string to include `preview_count`.

- [ ] **Step 2: Build and verify**

Run: `cargo build -p app-core`

- [ ] **Step 3: Commit**

```bash
git commit -m "fix(mirror): add trial preview cleanup to JOB_MIRROR_CLEANUP cron"
```

---

## Task 3: Wire Episodic Memory into MirrorFacade

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`
- Modify: `crates/cognitive/src/mirror/engine.rs`
- Modify: `crates/app-core/src/init/mod.rs`

The spec requires three cross-feature episodic memory writes:
1. Weekly narrative → episodic memory with domain `"mirror"`, importance `0.9`, tag `"mirror-reflection"`
2. Meta-rule approval → episodic memory with domain `"mirror"`, importance `0.8`
3. Trial kill → episodic memory with domain `"mirror"`, importance `0.7`

- [ ] **Step 1: Add episodic repo to MirrorFacade**

IMPORTANT: `facade.rs` is inside the `cognitive` crate, so use `crate::` paths, NOT `cognitive::`.

In `facade.rs`, add field and builder:

```rust
// In struct MirrorFacade:
episodic_repo: Option<crate::repos::EpisodicMemoryRepo>,

// Builder:
pub fn with_episodic_repo(mut self, repo: crate::repos::EpisodicMemoryRepo) -> Self {
    self.episodic_repo = Some(repo);
    self
}
```

Initialize to `None` in `new()`. Add a helper to reduce boilerplate:

```rust
fn write_episodic(&self, content: String, summary: Option<String>, importance: f64) {
    if let Some(ref episodic) = self.episodic_repo {
        let repo = episodic.clone();
        tokio::spawn(async move {
            let mem = crate::types::EpisodicMemory {
                id: Uuid::new_v4().to_string(),
                domain: "mirror".to_string(),
                content,
                summary,
                importance,
                occurred_at: Utc::now().to_rfc3339(),
                recorded_at: Utc::now().to_rfc3339(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "global".to_string(),
                scope_id: None,
            };
            if let Err(e) = repo.insert(&mem).await {
                tracing::warn!("mirror: failed to write episodic memory: {e}");
            }
        });
    }
}
```

Check if `EpisodicMemory` has `scope_type`/`scope_id` fields — if not, omit them. Read `crates/cognitive/src/types.rs` to verify the exact struct fields.

- [ ] **Step 2: Write episodic memory on weekly narrative generation**

In `generate_weekly_narrative()`, after `self.repo.insert_trend_narrative(&narrative).await?`, add:

```rust
self.write_episodic(
    format!("Weekly reflection: {}", narrative.full_narrative),
    Some(narrative.routing_summary.clone()),
    0.9,
);
```

- [ ] **Step 3: Write episodic memory on meta-rule approval**

In `approve_meta_rule()`, after the status update:

```rust
self.write_episodic(
    format!("Approved meta-rule: {}", rule_id),
    None,
    0.8,
);
```

- [ ] **Step 4: Write episodic memory on trial kill + emit domain event for auto-note**

In `kill_trial()`, after cancelling the timer:

```rust
self.write_episodic(
    format!("Killed experiment trial {}", trial_id),
    None,
    0.7,
);
```

The spec also requires auto-creating a note on trial kill (line 539). The facade lives in cognitive (L3-L4) and cannot depend on `feature-notes` (L4) directly. Instead, emit a `DomainEvent::MirrorTrialKilled` and handle note creation in `app-core`. This domain event will be added in Task 6a (below).

- [ ] **Step 5: Wire episodic repo through MirrorEngine**

In `engine.rs`, update `MirrorEngine::start` to accept an optional `EpisodicMemoryRepo` and pass it to the facade:

```rust
pub fn start(
    repo: MirrorRepo,
    bus: &bus::DomainEventBus,
    narrative_handler: Option<Arc<dyn NarrativeHandler>>,
    autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
    episodic_repo: Option<crate::repos::EpisodicMemoryRepo>,
) -> (MirrorFacade, Vec<JoinHandle<()>>, CancellationToken) {
```

In the facade construction, add:

```rust
if let Some(repo) = episodic_repo {
    facade = facade.with_episodic_repo(repo);
}
```

- [ ] **Step 6: Update call site in init/mod.rs**

In `crates/app-core/src/init/mod.rs`, pass the episodic memory repo to `MirrorEngine::start`. The `EpisodicMemoryRepo` is already available from the cognitive repos (`repos.episodic_memory`). Create one from the pool:

```rust
let episodic_repo = Some(::cognitive::repos::EpisodicMemoryRepo::new(storage_pool.inner().clone()));
```

Pass it as the 5th argument to `MirrorEngine::start`.

- [ ] **Step 7: Build and run tests**

Run: `cargo build --workspace`
Run: `cargo nextest run -p cognitive -E 'test(mirror)'`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(mirror): write episodic memories on weekly narrative, meta-rule approval, and trial kill"
```

---

## Task 4: Add `proposed_meta_rule` to MirrorResponse

**Files:**
- Modify: `crates/cognitive/src/mirror/types.rs`

- [ ] **Step 1: Add field**

Update `MirrorResponse` to match the spec:

```rust
pub struct MirrorResponse {
    pub answer: String,
    pub data_sources_used: Vec<String>,
    pub proposed_meta_rule: Option<MetaRule>,
}
```

- [ ] **Step 2: Fix compilation**

Update `generate_mirror_response` in `facade.rs` to include `proposed_meta_rule: None` in the return.

NOTE: The field is structurally present but always `None` for now. Wiring actual LLM-proposed meta-rule extraction from conversational responses requires changes to `NarrativeHandler::generate_mirror_response` return type and is deferred — the LLM would need to return both an answer and an optional rule proposal, which is a prompt engineering task beyond structural wiring.

- [ ] **Step 3: Build**

Run: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add proposed_meta_rule field to MirrorResponse"
```

---

## Task 5: Add `create_meta_rule_from_text` Facade Method

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Implement**

Add to MirrorFacade:

```rust
pub async fn create_meta_rule_from_text(&self, text: String) -> Result<MetaRule> {
    let rule = MetaRule {
        id: Uuid::new_v4(),
        trigger_condition: text,
        action: MetaRuleAction::SurfaceInsight { message: String::new() },
        source: MetaRuleSource::UserCreated,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    self.repo.insert_meta_rule(&rule).await?;
    Ok(rule)
}
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p cognitive`

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(mirror): add create_meta_rule_from_text facade method"
```

---

## Task 6: Wire SSE Entity Updates + Auto-Note Domain Events

This task has two sub-parts: SSE entity updates for live UI refresh, and domain events for cross-feature ripple.

### Task 6a: Add Missing Domain Events + EntityKind

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/desktop-shared/src/types.rs`

- [ ] **Step 1: Add `MirrorTrialKilled` domain event**

In `crates/bus/src/domain_events.rs`, add in the Mirror section:

```rust
/// Emitted when user kills an experiment trial via the Mirror UI.
MirrorTrialKilled {
    trial_id: String,
},
```

Fix all exhaustive match arms (same pattern as `TrialActivated` in Phase 4 — check `salience.rs`, `background.rs`, `app_core.rs`, `streaming.rs`). Use `Discard` for salience, `"mirror"` for domain.

- [ ] **Step 2: Add `BrainVersion` to EntityKind**

In `crates/desktop-shared/src/types.rs`, add `BrainVersion` variant to the `EntityKind` enum, and add the parse helper `"brainversion" | "brain_version" => Some(Self::BrainVersion)`.

- [ ] **Step 3: Build**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add MirrorTrialKilled domain event and BrainVersion EntityKind"
```

### Task 6b: Emit SSE Entity Updates from Tauri Commands

**Files:**
- Modify: `crates/desktop/src/commands/mirror.rs`

- [ ] **Step 1: Read the existing emit_updates pattern**

Read `crates/desktop/src/commands/mod.rs` to find `emit_updates`. Read one command that uses it (e.g., `commands/notes.rs`) to understand the pattern.

- [ ] **Step 2: Add entity updates to mirror mutation commands**

After each mutation in the Tauri commands, emit the appropriate entity update:
- `approve_meta_rule` / `dismiss_meta_rule` → `EntityKind::MirrorSnippet` (rule changes affect snippet display)
- `kill_trial` / `continue_trial` → `EntityKind::MirrorSnippet`
- `revert_brain_version` → `EntityKind::BrainVersion`
- `submit_mirror_feedback` → `EntityKind::MirrorSnippet`

Match the exact pattern used by other command files. Only the Tauri commands need entity updates (dev server has its own SSE mechanism).

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): emit SSE entity updates on mirror mutations"
```

### Task 6c: Auto-Note on Trial Kill (app-core handler)

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/cognitive/src/mirror/facade.rs`

The spec (line 539) says "User kills trial → auto-creates note". Since `MirrorFacade` (cognitive L3-L4) cannot depend on `feature-notes` (L4), `kill_trial` emits `DomainEvent::MirrorTrialKilled` and an `app-core` event handler creates the note.

- [ ] **Step 1: Emit domain event from kill_trial**

In `facade.rs`, add an optional `Arc<bus::DomainEventBus>` field to `MirrorFacade` (with builder `with_domain_event_bus`). In `kill_trial`, after the timer cancel, publish:

```rust
if let Some(ref bus) = self.domain_event_bus {
    bus.publish(bus::DomainEvent::MirrorTrialKilled {
        trial_id: trial_id.to_string(),
    });
}
```

Wire the bus through `MirrorEngine::start` (it already has access to the bus).

- [ ] **Step 2: Handle in app-core init**

In `init/mod.rs`, add a subscriber for `MirrorTrialKilled` that creates a note via the notes repo. Follow the existing domain event handler patterns in init. The note should have title "Killed experiment trial {trial_id}" and body explaining the decision was manual.

IMPORTANT: This is a best-effort fire-and-forget. Don't let note creation failure block the kill action.

- [ ] **Step 3: Build**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): auto-create note on trial kill via domain event"
```

---

## Task 7: MirrorTool for MCP Exposure

**Files:**
- Create: `crates/tools/src/domain/mirror.rs`
- Modify: `crates/tools/src/domain/mod.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/config/src/schema/mcp.rs`

Expose read-only Mirror data via MCP so external AI clients (Claude Code, Cursor) can query the Mirror state.

- [ ] **Step 1: Study the existing Tool pattern**

Read a simple existing tool (e.g., `crates/tools/src/domain/docs.rs` or another single-action tool) to understand the `#[derive(Tool)]` + `#[derive(ToolParams)]` pattern, and how tools are registered in `lib.rs`.

- [ ] **Step 2: Create mirror.rs with MirrorTool**

Create a multi-action tool with actions: `get_state`, `get_narratives`, `get_routing_history`, `get_brain_versions`, `get_meta_rules`. All read-only.

The tool needs access to `MirrorFacade`. Check how other tools get their dependencies (via constructor injection from `FeaturePackage::tools()` or from the agent builder).

Use `#[tool_actions]` + `#[derive(ActionParams)]` for multi-action tools. Read `crates/tools-core-macros/src/` for the macro syntax.

- [ ] **Step 3: Export from domain/mod.rs**

Add `pub mod mirror;` and re-export the tool.

- [ ] **Step 4: Register in agent builder**

The `MirrorTool` needs an `Arc<MirrorFacade>` which lives in `AppCore`. It cannot be registered via `FeaturePackage::tools()` (which doesn't have AppCore access). Instead, wire it in the agent builder where tools are assembled with AppCore state.

Search for where `MemoryTool` or similar AppCore-dependent tools are registered — likely in `crates/agent/src/agent_builder.rs` or `crates/app-core/src/init/mod.rs`. Follow that pattern: construct `MirrorTool::new(mirror_facade.clone())` and add it to the tool registry.

- [ ] **Step 5: Add "mirror" to default_exposed_tools()**

In `crates/config/src/schema/mcp.rs`, add `"mirror"` to the array in `default_exposed_tools()`.

- [ ] **Step 6: Build and verify**

Run: `cargo build --workspace`
Run: `cargo nextest run -p klyntbot-server` (verifies tool appears in `list_tools` and passes whitelist)

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(mirror): add MirrorTool for MCP exposure with read-only actions"
```

---

## Task 8: Enhance Adaptive Tone System Prompt

**Files:**
- Modify: `crates/agent/src/adapters/mirror_handlers.rs`

The `NarrativeHandler` LLM implementation includes `past_narrative_feedback` in the context but the system prompt doesn't explicitly instruct the LLM to adapt tone based on it.

- [ ] **Step 1: Read current system prompt**

Read `crates/agent/src/adapters/mirror_handlers.rs` to find the system prompt used for narrative generation.

- [ ] **Step 2: Enhance the prompt**

Add explicit tone adaptation instructions. When `past_narrative_feedback` contains `NotHelpful` entries, instruct the LLM to be more concise and less flowery. When it contains `Helpful` entries, maintain the current style.

- [ ] **Step 3: Build**

Run: `cargo build -p agent`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): enhance narrative system prompt with adaptive tone based on past feedback"
```

---

## Task 9: Integration Tests

**Files:**
- Modify: `tests/integration/mirror.rs`

- [ ] **Step 1: Add episodic memory ripple test for meta-rule approval**

Test that `approve_meta_rule` writes an episodic memory. Create a facade with `with_episodic_repo`, insert a meta-rule, approve it, then query the episodic repo to verify a memory was written with importance 0.8 and domain "mirror".

- [ ] **Step 2: Add episodic memory ripple test for trial kill**

Test that `kill_trial` writes an episodic memory with importance 0.7.

- [ ] **Step 3: Add create_meta_rule_from_text test**

Test that `create_meta_rule_from_text("When user says X, do Y")` creates a pending meta-rule with `source: UserCreated` and `effectiveness_score: 0.5`.

- [ ] **Step 4: Add trial preview cleanup test**

Directly test `cleanup_old_trial_previews(1)` by inserting a preview with `preview_at` 2 days ago, calling cleanup, and verifying it's deleted.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -E 'test(mirror)'`

- [ ] **Step 6: Commit**

```bash
git commit -m "test(mirror): add Phase 5 cross-feature ripple integration tests"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all mirror tests:** `cargo nextest run -p cognitive -E 'test(mirror)'`
- [ ] **Run integration tests:** `cargo nextest run -E 'test(mirror)'`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Run frontend lint:** `cd desktop-ui && bun run lint`
- [ ] **Run frontend build:** `cd desktop-ui && bun run build`
- [ ] **Verify MCP tool appears:** `cargo build -p klyntbot-mcp && ./target/debug/klyntbot-mcp tools --list | grep mirror`
