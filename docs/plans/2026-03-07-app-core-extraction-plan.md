# App-Core Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract a framework-agnostic `app-core` crate from the Tauri-coupled `desktop` crate, moving 22 cognitive/coaching handlers as methods on `AppCore` so both `desktop` and `dev-api` are thin wrappers calling shared logic.

**Architecture:** Create `crates/app-core` with the `AppCore` struct, accessors, and handler methods. Decouple from Tauri by removing `tauri::AppHandle` from init — callers wire event forwarding themselves using returned channels. The `desktop` and `dev-api` crates become thin wrappers that extract state and delegate to `AppCore` methods.

**Tech Stack:** Rust, Tauri 2, Axum, tokio, feature-coaching, cognitive crate

**Design doc:** `docs/plans/2026-03-07-app-core-extraction-design.md`

---

### Task 1: Create app-core crate skeleton

**Files:**
- Create: `crates/app-core/Cargo.toml`
- Create: `crates/app-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Create `crates/app-core/Cargo.toml`**

```toml
[package]
name = "app-core"
version.workspace = true
edition.workspace = true

[dependencies]
cognitive = { workspace = true }
common = { workspace = true }
config = { workspace = true }
desktop-shared = { workspace = true }
feature-coaching = { workspace = true }
feature-notes = { workspace = true }
feature-productivity = { workspace = true }
storage = { workspace = true }
agent = { workspace = true }
bus = { workspace = true }
providers = { workspace = true }
scheduling = { workspace = true }
context_engine = { workspace = true }
channels = { workspace = true }
tools = { workspace = true }
tools-core = { workspace = true }

chrono = { workspace = true }
dashmap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
```

**Step 2: Create `crates/app-core/src/lib.rs`**

```rust
mod coaching;
mod cognitive;
mod state;

pub use state::AppCore;
```

**Step 3: Add to workspace**

In root `Cargo.toml`, add `"crates/app-core"` to the `members` list (after `crates/feature-coaching`), and add to `[workspace.dependencies]`:

```toml
app-core = { path = "crates/app-core" }
```

**Step 4: Create stub `state.rs`**

Create `crates/app-core/src/state.rs` with an empty struct:

```rust
pub struct AppCore;
```

**Step 5: Create stub modules**

Create `crates/app-core/src/cognitive.rs`:
```rust
use crate::AppCore;
```

Create `crates/app-core/src/coaching.rs`:
```rust
use crate::AppCore;
```

**Step 6: Verify it compiles**

Run: `cargo build -p app-core`
Expected: Compiles with no errors.

**Step 7: Commit**

```
feat(app-core): create app-core crate skeleton
```

---

### Task 2: Move AppCore struct and accessors into app-core

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/lib.rs`

**Goal:** Move the `AppCore` struct definition and its accessor methods (productivity_repos, signal_accumulator, etc.) from `desktop/src/app_core.rs` into `app-core/src/state.rs`. Do NOT move init or event forwarding yet — just the struct and accessors.

**Step 1: Write `crates/app-core/src/state.rs`**

Copy the `AppCore` struct definition from `desktop/src/app_core.rs:28-77` with these changes:
- Remove `tauri::Emitter` import (not needed for struct definition)
- Keep all fields exactly as they are
- Copy all accessor methods: `productivity_repos()`, `focus_manager()`, `aggregator()`, `distraction_interceptor()`, `signal_accumulator()`, `pattern_detector()`, `intervention_router()`, `feedback_tracker()`, `user_situation()`, `domain_event_bus()` from lines 574-638
- Copy `shutdown()` method from lines 640-653
- Add a `pub fn new(...)` constructor that takes all fields (no init logic, just assignment)

The struct should use these imports:

```rust
use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use cognitive::situation::UserSituation;
use common::FormResponse;
use desktop_shared::errors::ApiError;
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_notes::repo::NoteRepo;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::CronService;
use storage::Repos;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
```

**Important:** The `new()` constructor should be a plain field-by-field constructor. No init logic. Example:

```rust
impl AppCore {
    pub fn new(
        repos: Repos,
        agent: Arc<AgentLoop>,
        bus: Arc<MessageBus>,
        // ... all fields ...
    ) -> Self {
        Self { repos, agent, bus, /* ... */ }
    }
}
```

**Step 2: Update `crates/app-core/src/lib.rs`**

```rust
mod coaching;
mod cognitive;
mod state;

pub use state::AppCore;

/// Convert a cognitive/sqlx error into an `ApiError`.
pub(crate) fn map_cognitive_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("STORAGE_ERROR", e.to_string())
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: Compiles. The struct is standalone, no consumers yet.

**Step 4: Commit**

```
feat(app-core): move AppCore struct and accessors from desktop
```

---

### Task 3: Move cognitive handler methods into app-core

**Files:**
- Modify: `crates/app-core/src/cognitive.rs`

**Goal:** Move the 12 cognitive handler implementations from `desktop/src/commands/cognitive.rs` into methods on `AppCore`. These are: `cognitive_user_model`, `cognitive_facts_list`, `cognitive_episodic_list`, `cognitive_rules_list`, `cognitive_memory_stats`, `cognitive_system_status`, `cognitive_inject_event`, `cognitive_fact_create`, `cognitive_fact_update`, `cognitive_fact_delete`, `cognitive_rule_create`, `cognitive_rule_deactivate`, `cognitive_run_compaction`.

**Step 1: Write `crates/app-core/src/cognitive.rs`**

Convert each `#[tauri::command] pub async fn foo(state: State<'_, Arc<AppCore>>, ...) -> Result<T, ApiError>` into `pub async fn foo(&self, ...) -> Result<T, ApiError>`.

The conversion pattern is:
- Remove `#[tauri::command]`
- Replace `state: State<'_, Arc<AppCore>>` with `&self`
- Replace `state.repos` with `self.repos`
- Replace `state.field()?.lock().await` with `self.field()?.lock().await`
- Replace `super::map_cognitive_err` with `crate::map_cognitive_err`

Copy the helper functions `fact_to_response`, `rule_to_response`, `fact_preview`, and `active_fact_count` as module-level functions.

Key imports:
```rust
use chrono::Timelike;
use cognitive::decay::retrievability;
use cognitive::repos::{load_user_model, SemanticFactRepo, RULE_DOMAINS, USER_MODEL_DOMAINS};
use cognitive::types::{ProceduralRule, SemanticFact};
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::AppCore;
```

**Step 2: Verify it compiles**

Run: `cargo build -p app-core`
Expected: Compiles. Methods exist on AppCore but no one calls them yet.

**Step 3: Commit**

```
feat(app-core): move cognitive handlers as AppCore methods
```

---

### Task 4: Move coaching handler methods into app-core

**Files:**
- Modify: `crates/app-core/src/coaching.rs`

**Goal:** Move the 8 coaching handler implementations: `coaching_situation`, `coaching_signals`, `coaching_patterns`, `coaching_feedback_stats`, `coaching_router_status`, `coaching_reset_dismissals`, `coaching_clear_signals`, `coaching_set_situation` (if it exists).

**Step 1: Write `crates/app-core/src/coaching.rs`**

Same conversion pattern as Task 3. Key handlers:

```rust
impl AppCore {
    pub async fn coaching_situation(&self) -> Result<UserSituationResponse, ApiError> {
        let sit = self.user_situation()?.lock().await;
        Ok(UserSituationResponse {
            energy_level: sit.energy_level,
            focus_state: sit.focus_state,
            deadline_pressure: sit.deadline_pressure,
            distraction_risk: sit.distraction_risk,
            coaching_receptivity: sit.coaching_receptivity,
            task_avoidance_detected: sit.task_avoidance_detected,
            hours_active_today: sit.hours_active_today,
            mins_since_break: sit.mins_since_break,
            hour_of_day: chrono::Local::now().hour(),
            recent_context_switches: sit.recent_context_switches,
        })
    }

    pub async fn coaching_signals(&self) -> Result<SignalWindowResponse, ApiError> {
        // full implementation from desktop/commands/cognitive.rs:257-298
    }

    // ... etc
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p app-core`
Expected: Compiles.

**Step 3: Commit**

```
feat(app-core): move coaching handlers as AppCore methods
```

---

### Task 5: Rewire desktop crate to use app-core

**Files:**
- Modify: `crates/desktop/Cargo.toml` — add `app-core = { workspace = true }`
- Modify: `crates/desktop/src/app_core.rs` — replace struct definition with re-export, keep init + event forwarding
- Modify: `crates/desktop/src/commands/cognitive.rs` — replace handler bodies with one-line delegations
- Modify: `crates/desktop/src/commands/mod.rs` — may need to update imports

**Step 1: Add dependency**

In `crates/desktop/Cargo.toml` add:
```toml
app-core = { workspace = true }
```

**Step 2: Rewrite `desktop/src/app_core.rs`**

Replace the `AppCore` struct definition with `pub use app_core::AppCore;` — but since `desktop/src/app_core.rs` also contains `init()` and event forwarding (Tauri-specific), restructure as:

- Remove the struct definition (it's now in `app-core`)
- Keep `init()` but change it to:
  1. Run the shared init logic (config, storage, provider, agent, coaching — factored as helper calls)
  2. Create `app_core::AppCore::new(...)` with all the fields
  3. Wire Tauri-specific event forwarding (domain events, pipeline events, interventions)
  4. Return the `AppCore`
- Keep `spawn_background`, `register_cron_callbacks`, `ensure_cron_jobs` — these are Tauri-specific orchestration
- Remove all accessor methods (they're now on `app_core::AppCore`)

The key insight: `desktop/src/app_core.rs` becomes a **Tauri-specific init wrapper** around the framework-agnostic `AppCore` struct.

**Step 3: Rewrite `desktop/src/commands/cognitive.rs`**

Each command becomes a one-line delegation:

```rust
use std::sync::Arc;
use app_core::AppCore;
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;
use tauri::State;

#[tauri::command]
pub async fn cognitive_user_model(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserModelSummaryResponse, ApiError> {
    state.cognitive_user_model().await
}

#[tauri::command]
pub async fn cognitive_facts_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    state.cognitive_facts_list(domain).await
}

// ... 19 more, all one-line delegations
```

**Step 4: Verify desktop compiles**

Run: `cargo build -p desktop`
Expected: Compiles with same pre-existing warnings.

**Step 5: Verify tests pass**

Run: `cargo nextest run -p cognitive -p agent -p feature-coaching`
Expected: All 415 tests pass.

**Step 6: Commit**

```
refactor(desktop): delegate cognitive/coaching commands to app-core
```

---

### Task 6: Rewire dev-api to use app-core

**Files:**
- Modify: `crates/dev-api/Cargo.toml` — add `app-core = { workspace = true }`
- Modify: `crates/dev-api/src/main.rs` — drop `DevState`, use `AppCore`

**Step 1: Add dependency**

In `crates/dev-api/Cargo.toml` add:
```toml
app-core = { workspace = true }
```

**Step 2: Replace `DevState` with `AppCore`**

In `dev-api/src/main.rs`:

1. Remove the `DevState` struct definition (lines 43-67)
2. Remove `DevState` accessor methods (lines 75-99)
3. Change `type AppState = Arc<DevState>;` to `type AppState = Arc<app_core::AppCore>;`
4. In `main()`, replace the manual state construction with `app_core::AppCore::new(...)` using the same fields
5. Keep SSE-specific state (sse_channels, cognitive_sse_senders) as separate fields — either wrap `AppCore` in a `DevApiState` that holds both, or store SSE senders alongside AppCore in a tuple

Suggested pattern — a thin wrapper:

```rust
struct DevApiState {
    core: app_core::AppCore,
    sse_channels: DashMap<String, Vec<UnboundedSender<SseEvent>>>,
    cognitive_sse_senders: Mutex<Vec<UnboundedSender<SseEvent>>>,
}
```

**Step 3: Replace cognitive/coaching match arms with delegations**

For each cognitive/coaching handler in the big match block, replace the implementation with:

```rust
"cognitive_system_status" => ok(core.core.cognitive_system_status().await?),
"coaching_signals" => ok(core.core.coaching_signals().await?),
// etc.
```

Handlers that need request body parsing (inject_event, fact_create, etc.) still parse the body in dev-api, then call the method:

```rust
"cognitive_inject_event" => {
    let event_type: String = get_str(&body, "event_type")?;
    ok(core.core.cognitive_inject_event(event_type, body).await?)
}
```

**Step 4: Remove duplicate coaching init**

The coaching component initialization in dev-api `main()` (lines 178-213) should use the same fields passed to `AppCore::new()`. Since AppCore stores all coaching components, dev-api just needs to:
1. Create the components (signal_accumulator, pattern_detector, etc.)
2. Start CoachingService
3. Pass all components to `AppCore::new()`

**Step 5: Verify dev-api compiles**

Run: `cargo build -p dev-api`
Expected: Compiles.

**Step 6: Verify workspace compiles**

Run: `cargo build --workspace`
Expected: Compiles.

**Step 7: Commit**

```
refactor(dev-api): replace DevState with app-core AppCore
```

---

### Task 7: Cleanup and verify

**Files:**
- Modify: `crates/desktop/src/commands/mod.rs` — `map_cognitive_err` can be removed if no other module uses it (it's now in app-core)
- Verify: no dead code warnings from our changes

**Step 1: Remove dead code**

Check if `map_cognitive_err` in `desktop/src/commands/mod.rs` is still used by any file other than cognitive.rs. If not, remove it.

**Step 2: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: No new warnings from our changes.

**Step 4: Run fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 5: Manual smoke test**

1. Run `cargo tauri dev` — verify all 5 debug dashboard tabs show real data
2. Run `cargo run -p dev-api` + `cd desktop-ui && bun run dev` — verify browser shows same data
3. Inject a `UserStatedFact` event in both — verify Pipeline tab updates in both

**Step 6: Commit**

```
chore: cleanup dead code after app-core extraction
```

---

## Summary

| Task | Description | Estimated steps |
|------|-------------|-----------------|
| 1 | Create app-core crate skeleton | 7 |
| 2 | Move AppCore struct + accessors | 4 |
| 3 | Move cognitive handlers (13 methods) | 3 |
| 4 | Move coaching handlers (8 methods) | 3 |
| 5 | Rewire desktop to delegate | 6 |
| 6 | Rewire dev-api to use AppCore | 7 |
| 7 | Cleanup and verify | 6 |

**Total: 7 tasks, ~36 steps**

After completion:
- `app-core` owns `AppCore` struct + 21 handler methods
- `desktop` has zero handler logic — just `#[tauri::command]` one-liners
- `dev-api` has zero handler logic — just match-arm one-liners
- Adding/changing a handler is a single edit in `app-core`
- Removing `dev-api` later just means deleting the crate
