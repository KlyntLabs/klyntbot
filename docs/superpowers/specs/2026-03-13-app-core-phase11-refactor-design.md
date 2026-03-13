# app-core Phase 11 Refactor — Design Spec

**Date:** 2026-03-13
**Scope:** `crates/app-core/` — structural reorganization only, zero logic changes
**Constraint:** Preserve all function signatures, trait impls, struct definitions, derive macros, and runtime behavior exactly.

---

## Problem Statement

`app-core` is the L7 orchestration crate that wires the entire agent stack and exposes handler methods to `desktop`. It has grown to 10,940 lines across 33 files with several structural problems:

1. **`init.rs` (1,377 lines)** — one monolithic function initializing 8+ subsystems sequentially
2. **8 handler files exceed 400 lines** — each mixes multiple sub-concerns
3. **Infrastructure scattered at root** — `file_watcher.rs` and `shell_hook.rs` sit alongside state/init/errors
4. **`handlers/` has 24 flat files** — no domain grouping; AI agents can't navigate by feature

---

## Design: Option A — Domain-Grouped Handlers + Phased Init Split

### Guiding Principle

`app-core` is a pure orchestration layer (no own domain model). Organization principle is **by feature domain**, not hexagonal layers.

---

## Proposed File Tree

```
crates/app-core/src/
├── lib.rs                              # re-exports: AppCore, EntityUpdate, HandlerResult, EventChannels, AppEventEmitter
├── state.rs                            # AppCore struct + feature accessor methods + shutdown (UNCHANGED)
├── errors.rs                           # error-mapping utilities (UNCHANGED)
├── events.rs                           # AppEventEmitter trait + NoopEmitter (UNCHANGED)
│
├── init/                               # SPLIT from init.rs (1377 → 8 files)
│   ├── mod.rs                          # EventChannels, AppCore::init(), AppCore::init_with_sender()
│   ├── storage.rs                      # storage pool, migrations, NoteRepo helper
│   ├── agent.rs                        # agent loop + persona manager helper
│   ├── channels.rs                     # channel manager helper
│   ├── cron.rs                         # cron service + cron job registration helper
│   ├── productivity.rs                 # productivity repos + engines helper
│   ├── coaching.rs                     # coaching pipeline helper
│   └── cognitive.rs                    # cognitive event log + domain bus helper
│
├── infrastructure/                     # MOVED from src/
│   ├── mod.rs
│   ├── file_watcher.rs                 # moved from src/file_watcher.rs
│   └── shell_hook.rs                   # moved from src/shell_hook.rs
│
└── handlers/
    ├── mod.rs                          # all pub mod declarations + key re-exports
    │
    ├── chat/                           # SPLIT from chat.rs (1114 → 4 files)
    │   ├── mod.rs                      # re-exports ChatStreamInfo
    │   ├── streaming.rs                # ChatStreamInfo struct, chat_send
    │   ├── sessions.rs                 # session CRUD: list, get, delete, update, rename, archive
    │   └── threads.rs                  # thread queries + context/persona management + interaction reply
    │
    ├── productivity/                   # SPLIT from productivity.rs (983 → 6 files)
    │   ├── mod.rs                      # re-exports pub converter fns used by desktop
    │   ├── converters.rs               # summary_to_response, session_to_response, insight_to_response, etc.
    │   ├── summaries.rs                # productivity_today, productivity_weekly, productivity_summary_range
    │   ├── focus.rs                    # focus, pomodoro, break sessions + auto_focus
    │   ├── tracking.rs                 # categories, tracked_apps, time_entries, goals, projects, recategorize
    │   └── calendar.rs                 # calendar events + weekly assessment
    │
    ├── finance/                        # SPLIT from finance.rs (791 → 6 files)
    │   ├── mod.rs
    │   ├── accounts.rs                 # account CRUD
    │   ├── transactions.rs             # transaction CRUD + filtered queries
    │   ├── budgets.rs                  # budget CRUD
    │   ├── investments.rs              # portfolio + investment CRUD
    │   └── reports.rs                  # net_worth, exchange_rates, spending/income reports, trends
    │
    ├── cognitive/                      # SPLIT from cognitive.rs (662 → 4 files)
    │   ├── mod.rs                      # re-exports: fact_to_response, rule_to_response, fact_preview, build_reflection_handlers (pub(crate))
    │   ├── memory.rs                   # read-only: facts, episodic, rules, stats, system_status
    │   ├── mutations.rs                # fact CRUD + rule CRUD
    │   └── operations.rs               # compaction, reflection, event_log, pipeline_log, inject_event
    │
    ├── notes/                          # SPLIT from notes.rs (573 → 4 files)
    │   ├── mod.rs
    │   ├── converters.rs               # note_row_to_response, notes_with_tags_batch, extract_links_and_mentions
    │   ├── notes.rs                    # note CRUD + attachments
    │   └── notebooks.rs                # notebook CRUD
    │
    ├── settings/                       # SPLIT from settings.rs (445 → 3 files)
    │   ├── mod.rs
    │   ├── mcp.rs                      # MCP server CRUD + helpers (server_to_response, build_transport, etc.)
    │   └── config.rs                   # app_info, config_get_section, config_update_section + deep_merge + tests
    │
    ├── tasks/                          # SPLIT from tasks.rs (597 → 4 files)
    │   ├── mod.rs                      # re-exports: rows_to_tasks, row_to_task, kr_to_response, objective_to_response, priority_label
    │   ├── converters.rs               # row_to_task_response, action_to_today_task, rows_to_tasks, resolve_status_label, kr_to_response, objective_to_response, priority_label
    │   ├── crud.rs                     # task CRUD + toggle + subtasks
    │   └── queries.rs                  # today_tasks, project_list_for_tasks, objective_list_for_tasks
    │
    ├── areas.rs                        # UNCHANGED (141)
    ├── capture.rs                      # UPDATED: 7 `crate::shell_hook::` → `crate::infrastructure::shell_hook::` (only path changes)
    ├── coaching.rs                     # UNCHANGED (254)
    ├── columns.rs                      # UNCHANGED (204)
    ├── cron.rs                         # UNCHANGED (169)
    ├── distraction.rs                  # UNCHANGED (74)
    ├── entity_links.rs                 # UNCHANGED (187)
    ├── groups.rs                       # UNCHANGED (117)
    ├── key_results.rs                  # UNCHANGED (187)
    ├── objectives.rs                   # UNCHANGED (128)
    ├── project_conversations.rs        # UNCHANGED (37)
    ├── project_memories.rs             # UNCHANGED (38)
    ├── project_sources.rs              # UNCHANGED (86)
    ├── projects.rs                     # UNCHANGED (255)
    ├── status.rs                       # UNCHANGED (35)
    ├── timeline.rs                     # UNCHANGED (581) — tightly cohesive, no split needed
    ├── work_context.rs                 # UNCHANGED (561) — single topic
    └── workflows.rs                    # UNCHANGED (207)
```

---

## Splitting Strategy

### `init.rs` (1,377 lines) → `init/` (8 files)

The `init_with_sender()` function is one long sequential init. Split by **extracting initialization phases into `pub(super) async fn` helpers** in their respective phase modules, then having `init/mod.rs::init_with_sender()` orchestrate them in the original order.

Phase boundaries:
| Module | Responsibility |
|--------|---------------|
| `storage.rs` | StoragePool connect, all feature migrations, NoteRepo, Repos, VectorStore |
| `agent.rs` | PersonaManager + AgentLoop construction |
| `channels.rs` | ChannelManager setup |
| `cron.rs` | CronService + cron job registration (reflection, productivity, etc.) |
| `productivity.rs` | ProductivityRepos, ProductivityEngine, FocusManager, NudgeService, DistractionInterceptor |
| `coaching.rs` | SignalAccumulator, PatternDetector, InterventionRouter, FeedbackTracker, UserSituation, CoachingService |
| `cognitive.rs` | EventLogRepo, pipeline_broadcast, domain bus wiring, ActivityIngestionService |

The `EventChannels` struct and the two public `init()`/`init_with_sender()` methods remain in `init/mod.rs`.

### Handler Subdirectories

Each subdirectory's `mod.rs` declares submodules and re-exports any `pub(crate)` functions or types that other handlers reference. No visibility changes — just `pub mod` + `pub use` redirections.

### Infrastructure Move

`file_watcher.rs` and `shell_hook.rs` are moved verbatim to `infrastructure/`. Their content is unchanged; only the module path changes. All callers of `crate::file_watcher::*` and `crate::shell_hook::*` update their paths to `crate::infrastructure::file_watcher::*` etc.

---

## Cross-Handler References to Preserve

| Caller | Called | Action |
|--------|--------|--------|
| `handlers/tasks/queries.rs` | `super::projects::build_project_response` | Becomes `super::super::projects::build_project_response` (tasks.rs was sibling; now in subdirectory) |
| `handlers/notes/notes.rs` | `extract_links_and_mentions` | Becomes `super::converters::extract_links_and_mentions` |
| `handlers/cognitive/operations.rs` | `build_reflection_handlers` | Stays in same `cognitive/` subtree; move to `cognitive/mod.rs` as `pub(crate)` |
| `handlers/project_memories.rs` (UNCHANGED) | `super::cognitive::fact_to_response` | `fact_to_response` must be `pub(crate)` and re-exported from `cognitive/mod.rs` |
| `handlers/objectives.rs` (UNCHANGED) | `super::tasks::{kr_to_response, objective_to_response}` | Must be re-exported from `tasks/mod.rs` as `pub(crate)` |
| `handlers/key_results.rs` (UNCHANGED) | `super::tasks::kr_to_response` | Must be re-exported from `tasks/mod.rs` as `pub(crate)` |
| `handlers/entity_links.rs` (UNCHANGED) | `super::tasks::priority_label` | Must be re-exported from `tasks/mod.rs` as `pub(crate)` |
| `handlers/entity_links.rs` (UNCHANGED) | `super::project_sources::source_row_to_response` | No path change — both files stay flat in `handlers/`; no action required |
| `handlers/status.rs` (UNCHANGED) | `super::tasks::row_to_task` | Must be re-exported from `tasks/mod.rs` as `pub(crate)` (already in re-export list) |
| `handlers/capture.rs` | `crate::shell_hook::*` (7 call sites) | Update to `crate::infrastructure::shell_hook::*` |
| `init.rs` (in `init/mod.rs` after split) | `crate::file_watcher::*` | Update to `crate::infrastructure::file_watcher::*` |

**Visibility rule:** All helper functions shared across handler subdirectory boundaries (`fact_to_response`, `rule_to_response`, `fact_preview`, `build_reflection_handlers`, `kr_to_response`, `objective_to_response`, `priority_label`, `rows_to_tasks`, `row_to_task`) must remain `pub(crate)` — never downgraded to `pub(super)`.

---

## `lib.rs` After Refactor

```rust
pub mod errors;
pub mod events;
pub mod handlers;
pub mod infrastructure;
pub mod init;
pub mod state;

// AppEventEmitter is additive — already accessible as `app_core::events::AppEventEmitter`
// Adding a top-level re-export for ergonomics (not a breaking change)
pub use events::AppEventEmitter;
pub use init::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
```

Note: `AppEventEmitter` is not currently re-exported at the crate root. Adding it here is intentional and additive (no breaking change to existing consumers).

---

## Preservation Guarantees

- **Zero logic changes** — no function body, algorithm, or error path modified
- **All public APIs unchanged** — `AppCore` methods, `HandlerResult`, `EntityUpdate`, `EventChannels` identical
- **All derive macros, trait impls, test modules preserved** — `settings/config.rs` carries the `#[cfg(test)]` block from `settings.rs`
- **`pub(crate)` visibility** — converter functions that are called cross-module boundaries stay `pub(crate)`; never downgrade to `pub(super)` for any function listed in the cross-reference table
- **Cargo.toml** — no dependency changes needed; all deps already present

---

## Verification Steps

After each phase:
```bash
cargo check -p app-core           # fast type + borrow check
cargo clippy -p app-core --all-targets  # zero new warnings
cargo nextest run -p app-core     # all tests pass
cargo check --workspace           # no downstream breakage
```
