# Architecture Deepening Backlog

> Generated 2026-05-22 from `improve-codebase-architecture` skill review.
> Language: see [LANGUAGE.md](../../../skills/LANGUAGE.md) (module, interface, implementation, depth, seam, adapter, leverage, locality).

---

## 1. Extract a FeatureHost from the dual init monoliths ⭐ TOP

**Strength:** Strong  
**Category:** in-process  
**Files:** `crates/app-core/src/init/mod.rs` (1,750 lines), `crates/agent/src/agent_loop/builder.rs` (2,000 lines), `crates/app-core/src/init/ai_pipeline.rs`, `crates/app-core/src/init/storage.rs`

**Problem:** Two god-functions manually assemble the entire object graph. `init/mod.rs` knows about every feature crate's internal constructors (`feature_notes::repo::NoteRepo`, `feature_productivity::repos::ProductivityRepos`, `cognitive::mirror::MirrorEngine::start`, etc.). `agent_loop/builder.rs` duplicates knowledge about cognitive handlers, tree builders, context sources, and tool registration. Adding a new feature requires touching **four** initialization sites. Neither function has tests.

**Solution:** Define a `FeaturePlugin` trait (or extend the existing `FeaturePackage` + `AiFeature` derive) so each crate declares its own dependencies, migrations, tools, context sources, signal consumers, and event translators. A `FeatureHost` resolves the dependency graph and builds the object tree. The host is the only module that knows about all plugins; plugins know only their own seams.

**Wins:**
- Locality: init logic lives in the crate that owns it
- Leverage: one host, N plugins
- Tests: each plugin tests its own wiring; the host tests graph resolution
- Delete ~3,750 lines of assembly

---

## 2. Collapse the agent message pipeline into a deep orchestrator

**Strength:** Strong  
**Category:** ports & adapters  
**Files:** `crates/agent/src/agent_loop/mod.rs`, `agent_runtime/runtime.rs`, `execution/core.rs`, `execution/execute_loop.rs`

**Problem:** Understanding one message requires bouncing across 6 modules. Each is shallow — its interface nearly matches its implementation. `AgentLoop` (25 fields), `AgentRuntime` (25 fields), and `ExecutionCore` all mutate the same `Vec<Message>` history. No single seam captures the full message lifecycle.

**Solution:** Extract an `AgentOrchestrator` trait. Behind it, collapse loop / runtime / execution into one deep module. Expose one seam. Tests hit the orchestrator trait with a mock adapter.

**Wins:**
- Locality: one place to fix message-flow bugs
- Tests hit one interface, not six
- AgentLoop shrinks from 25 fields to ~5

---

## 3. Genericize the repo surface

**Strength:** Worth exploring  
**Category:** local-substitutable  
**Files:** `crates/cognitive/src/repos/*` (29 files), `crates/feature-productivity/src/repos/*` (20 files), `crates/feature-notes/src/repo/*` (7 files)

**Problem:** 50+ repo files follow the identical pattern: `struct XRepo { pool: SqlitePool }` with `new/get/list`. Interface is as wide as the SQL inside. No generic seam.

**Solution:** Introduce `SqliteRepo<T>` with a trait bound for `FromRow` + `Entity`. Each domain supplies only the SQL fragments and row type.

**Wins:**
- Leverage: one interface, N entity types
- Locality: repo bug = fix once
- Delete ~40 shallow files
- Tests: one suite for the generic repo

---

## 4. Generate or flatten the Tauri command layer

**Strength:** Worth exploring  
**Category:** ports & adapters  
**Files:** `crates/desktop/src/commands/*.rs` (~55 files, ~2,500 lines)

**Problem:** Every command file is a 1-line adapter from Tauri IPC to `AppCore`. `dispatch_dev()` duplicates the same parameter extraction 55 times. Shallow, hand-written, untested.

**Solution:** Derive commands from an `AppCoreCommands` trait via proc-macro, or collapse into a single dispatch router that reflects over AppCore.

**Wins:**
- Delete ~2,500 lines of boilerplate
- One seam: transport ↔ application
- New command = add method, no new file

---

## 5. Replace RoutingContext with focused contexts

**Strength:** Worth exploring  
**Category:** in-process  
**Files:** `crates/tools-core/src/routing.rs`

**Problem:** `RoutingContext` accumulates every cross-cutting concern (20+ fields). Any tool needing one dependency accepts all 20+. Tests must construct the entire surface.

**Solution:** Split into `InteractionContext`, `HookContext`, `JobContext`, `SessionContext`. Each tool declares only the contexts it needs.

**Wins:**
- Interface shrinks per tool
- Tests construct only needed context
- Leaks stop: tool A cannot see tool B's deps

---

## 6. Merge platform micro-crates into a single seam

**Strength:** Speculative  
**Category:** ports & adapters  
**Files:** `crates/platform-input` (174 lines), `crates/platform-capture` (146 lines), `crates/platform-macos`

**Problem:** Two trait-only crates with exactly one adapter. They force readers to bounce between three crates for computer-use functionality. One adapter = hypothetical seam.

**Solution:** Collapse into `platform-macos` as a `traits` module, or a single `platform` crate. Re-split only when a second platform adapter justifies the seam.

**Wins:**
- Delete 2 crates, ~320 lines
- One place to understand platform abstractions
- Seam becomes real when Linux/Windows arrive
