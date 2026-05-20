# Unify KlyntBot to a Single Assistant Experience

> **Status:** Plan — pre-execution
> **Author:** drafted via multi-agent analysis 2026-05-20
> **Strategy:** Big-bang single PR (workspace is pre-release, no users to migrate)
> **End state:** No "mode" concept anywhere — KlyntBot is one product, a second-brain assistant. The coding-agent surface (sessions, ingest, memory, UI, tools tagged `coding_only`) is removed. Shared infrastructure (agent runtime, tools framework, memory, approval gate, MCP, feature-* crates for tasks/finance/notes/etc.) and the `klynt-core` actuator toolset (bash/edit/write/web_fetch/etc.) are kept and repositioned as the assistant's general-purpose tools.

---

## 0. North Star

KlyntBot becomes a single-product, single-experience **second-brain assistant**. The user has one entry point ("new chat"), one session type, one system-prompt file (`KLYNTBOT.md`), one settings tree without a "Coding" tab, and one tool catalog without channel partitioning. The assistant retains shell, file-edit, and web-fetch capability via the renamed `klynt-core` crate so it can act on the user's knowledge, not just describe it.

### Architecture before → after

| Concept | Before | After |
|---|---|---|
| Sessions | `SessionMode::Assistant` / `SessionMode::Coding` (immutable column, CHECK constraint) | Single session type; `SessionMode` enum deleted |
| System prompt | `~/.klyntbot/KLYNTBOT.md` (assistant) + `~/.klyntbot/KLYNTBOT-coding.md` (coding) | `~/.klyntbot/KLYNTBOT.md` only |
| Tool gating | `allowed_channels = "all" | "non_coding" | "coding_only"` per tool, `ApprovalGate` filters at dispatch | No channel gating; all tools available to the one assistant |
| Tool catalog | `klynt-core` (12 coding tools) + assistant feature crates (tasks/finance/notes/learning/language-learning/productivity/alarms) | Same feature crates **plus** all `klynt-core` tools — repositioned as general-purpose actuators |
| Coding-specific pipelines | `coding-ingest` (5 CLI adapters) + `coding-memory` (Distiller, Reforge, ghost-commit snapshots) | Deleted entirely |
| UI | `useAppMode()` toggle, `features/coding/` (90+ files), `features/plugins/coding-memory/`, coding settings tab, two new-chat code paths | Single chat experience; mode store/toggle/folders deleted |
| Tauri commands | ~80 `coding_*` commands + ~30 assistant commands | ~30 commands renamed from `coding_thread_*` → `thread_*` for thread lifecycle (the only entanglement) |
| Database | 14 coding-only tables + coding columns on shared tables + `SessionMode` CHECK | Tables dropped via one consolidating migration; CHECK relaxed; coding-only `_feature_migrations` rows left in place (harmless on existing dev DBs) |

---

## 1. Surface Inventory

### 1.1 Backend crates to **DELETE wholesale**

| Crate | LOC | Why |
|---|---|---|
| `crates/coding-ingest/` | ~5,500 | Adapters for claude_code, codex, kimi_cli, opencode, git_post_commit; daemon + hook binary + event store. No non-coding consumer. |
| `crates/coding-memory/` | ~12,000 | Distiller phases A-C, recall service, reforge writer, mirror signals, skill evolver, symbol extraction. Self-contained subsystem. |
| `crates/coding-agents-md/` | ~166 | Synthesises per-workspace `AGENTS.md`. Coding-mode only. |
| `crates/feature-coding-bash/` | ~4,200 | `coding_task_list/output/resize/stdin/stop` PTY-job tools. All `coding_only`. (Distinct from `klynt-core::bash` which we **keep**.) |
| `crates/feature-coding-todo/` | ~2,500 | `CodingTodo` tool + plan-mode injector + `CodingApprovalPolicy` wiring. |

### 1.2 Backend crates to **KEEP and reposition** (drop coding framing)

| Crate | Action |
|---|---|
| `crates/klynt-core/` | **Rename → `crates/actuator-tools/`** (or `crates/agent-tools/`). Strip `allowed_channels = "coding_only"` from `bash.rs`, `edit.rs`, `write.rs`, `apply_patch.rs`, `notebook_edit.rs`, `plan_mode.rs`. Remove `history_repo: Option<Arc<CodingApprovalHistoryRepo>>` fields. Lift `ASK_USER_TOOL_NAME` constant out — keep as-is, just stop being "coding tools". |
| `crates/klynt-execpolicy/` | Keep. No rename needed — execpolicy is mode-agnostic Starlark prefix-rule engine. |
| `crates/klynt-pty/` | Keep (dependency of `bash`'s interactive mode). |
| `crates/klynt-sandbox/` + `klynt-sandbox-helper/` | Keep (macOS `sandbox-exec` wrapper used by `bash` / `apply_patch`). |
| `crates/klynt-git-utils/` | Keep the diff/log/branches helpers (used by assistant git operations). **DELETE `ghost_commits.rs`** — that's coding-snapshot rewind only. |
| `crates/lsp-client/` | **DELETE.** Consumed only by `klynt-core::edit` for hover/diagnostics; the assistant doesn't need real-time LSP. (Re-evaluate if you later add a "code reading" feature to the second brain.) |

### 1.3 Backend coding modules inside **shared crates** (surgical removal)

These live in crates we keep but contain coding-only code paths:

**`crates/app-core/`** — remove these subdirectories and files:
- `src/coding/` (entire dir, ~4,100 LOC — 25 modules)
- `src/coding_memory/` (entire dir, ~1,970 LOC)
- `src/handlers/coding_jobs.rs`, `coding_plan.rs`, `coding_todo.rs`, `reforge.rs`
- `src/init/coding_recall.rs`, `coding_retention.rs`, `coding_skills.rs`, `coding_subscribers.rs`
- `src/runtime/coding.rs` (`CodingThreadRuntime`)
- `src/tracing/providers/claude_code/`, `kimi/`, `klynt/` (entire dirs)

**`crates/agent/`** — remove:
- `src/context_sources/coding_recall.rs`
- `src/context_sources/todo.rs` (CodingTodo blocked-items injector)
- `src/handlers/coding_synthesis.rs`, `rule_artifacts.rs`
- `src/adapters/reforge_handlers.rs`
- Coding branches in `src/agent_loop/mod.rs` (lines 1212, 1258, 1284), `src/agent_loop/builder.rs` (`coding_recall_service`, `coding_policies`, `with_classify_hooks`)
- `src/subagent.rs` / `subagent_runtime.rs`: `CodingApprovalPolicy` DashMap fields
- `tests/coding_recall_injection.rs`, `tests/coding_channel_filter.rs`

**`crates/approval/`** — remove `src/coding_policy.rs` (entire file, ~240 LOC). Remove `pub use coding_policy::CodingApprovalPolicy` from `lib.rs`. Remove `"coding"` arm at `src/request.rs:25`.

**`crates/skill-system/`** — `src/soul.rs`: delete `DEFAULT_CODING_SOUL` constant (~85 lines), `coding` / `coding_path` / `last_coding_mtime` fields on `SoulStore`, the load/reload paths that touch `KLYNTBOT-coding.md`, and the `SessionMode::Coding` arm of `provide()`. Delete the `include_str!("../../../skills/coding-orchestrator/...")` line in `src/defaults.rs:147` and the corresponding `skills/coding-orchestrator/` directory.

**`crates/cognitive/src/mirror/`** — remove:
- `sources/approval_history.rs`: the `legacy_repo: Arc<CodingApprovalHistoryRepo>` field and its query path (keep `pattern_repo` — `approval_pattern_history` table is shared).
- `types.rs`: `MirrorAlert::Coding`, `MirrorAlertType::Coding`, `coding_alert_kind` / `coding_alert_severity` row fields.
- `narratives.rs`: arms that set the `coding_alert_*` fields.
- `sources/cost_ceiling.rs`: `CodingMirrorAlert` subscription.

**`crates/storage/`** — remove:
- `src/repos/coding_approval_history.rs`, `coding_background_jobs.rs`, `coding_reviews.rs`, `coding_todo.rs`
- Their `pub mod` lines + the three `Repos` struct fields they back

**`crates/bus/src/domain_events.rs`** — remove variants `DomainEvent::CodingMirrorAlert`, `CodingMemoryUpdated`, and corresponding `EventDomain::CodingMemory`. Remove `ContextUpdate::CodingTodoChanged` / `CodingPlanRatified` / `CodingJobsChanged`.

**`crates/desktop-shared/`** — remove `src/coding/` directory wholesale, plus `src/commands/coding_memory.rs`. Remove `EntityKind::CodingFact` and `CodingEpisode` from `src/types.rs`.

**`crates/common/`** — remove `SessionMode::Coding`, `CODING_CHANNEL` constant, `Channel::Coding`, `ChannelMask::CODING`, `CODING_ONLY` mask. After step 7 of the rollout, remove `SessionMode` enum entirely.

**`crates/config/`** — delete entire files `src/schema/coding.rs` (~CodingConfig tree) and `src/schema/coding_memory.rs`. Remove `coding: CodingConfig` and `coding_memory: CodingMemoryConfig` fields from `core.rs`. Remove from `src/schema/mcp.rs::default_exposed_tools()` any coding-only tool names.

**`crates/desktop/src/commands/`** — delete all 17 files prefixed `coding_*` (full list in §2.4). Update `crates/desktop/src/commands/approval.rs` + `approvals.rs` to drop imports of `desktop_shared::coding::ApprovalDecisionDto` and `app_core::coding::approval_handler`.

**`crates/desktop/src/dev_server/dispatch.rs`** — remove all 14 `dispatch_dev` calls for coding commands (line range 48–240).

**`crates/desktop/src/specta_builder.rs`** — remove 80+ entries from `klynt_collect_commands![…]`.

**`crates/mcp/`** — remove `"coding"` channel test stubs in `src/allowlist.rs:65–87` and `SessionMode::Coding` test arm in `src/server/approval.rs:61`.

### 1.4 Frontend (`desktop-ui/`) deletions

**Whole folders to delete:**
- `src/features/coding/` (90+ files: components, hooks, state, slash commands, integration tests)
- `src/features/plugins/coding-memory/` (19 files)
- `src/features/settings/components/sections/coding/` (8 files)
- `src/features/app/components/AppModeSwitch.tsx` + `.test.tsx`
- `src/features/app/hooks/useAppMode.ts` + `.test.ts`
- `src/features/app/hooks/useResetAppViewOnModeChange.ts` + `.test.ts`

**API & bindings:**
- `src/api/endpoints/coding.ts`, `codingMemory.ts` — delete; remove the re-export from `src/api/endpoints/index.ts`
- After backend rename of `coding_thread_*` → `thread_*` (see §3.2), rewrite `src/api/endpoints/thread.ts` accordingly
- `src/bindings.ts` — regenerated by `cargo tauri dev` after backend changes; will lose all `coding*` wrappers automatically

**CSS files:**
- `src/styles/coding.css`, `coding-sidebar.css`, `coding-jobs.css`, `coding-todo.css`, `coding-activity.css`, `app-mode-switch.css`
- Remove the six `@import` lines from `src/styles/index.css`

**Settings:**
- `src/features/settings/components/sections/SettingsCodingSection.tsx`, `CodingRecallStats.tsx` + `.test.tsx`
- Remove `"coding"` from `SETTINGS_EXTRA_SECTION_IDS`, `SettingsSection.Coding`, `SETTINGS_SECTION_LABELS`, `SettingsNav.tsx` panel entry, `useSettingsViewOrchestration.ts:258`

**Components with coding branches to surgically prune:**
- `MainApp.tsx` — remove `useAppMode`, `CodingThreadView`, `initThreadEventBuffer`, `codingThreadStatus` merge, `useResetAppViewOnModeChange`, the `mode === "code"` arm in `onNewChat`, the `codingMessagesNode` and `codeLandingNode` blocks
- `SidebarChatLayout.tsx` — remove `AppModeSwitch`, `CodingSidebarThreadsSection`, mode filtering on `NavItem`, the `mode === "code"` branch in chat list, `codingWorkspaceIdByThread` prop
- `useMainAppLayoutSurfaces.ts` — remove `useAppMode`, `useCodingSessions`, the `mode === "code"` branches (lines 1035-1048, 562-583)
- `DesktopLayout.tsx` — remove `useAppMode`, `homeOrCodeNode` ternary; collapse to `homeNode`
- `buildPrimaryNodes.tsx` + `types.ts` — remove `CodeLanding`, `codeLandingNode`, `codingWorkspaceIdByThread`, `ActivityPanel` (verify not used elsewhere)
- `Composer.tsx` — remove `CodingModePill`, `useCodingMode`, `useSlashCommands` (slash interception), `CostPill` — **but extract `CostPill` first** into a shared location since cost display is useful for the assistant
- `useComposerAutocompleteState.ts` — remove `FLAT_CATALOG` import and coding-slash autocomplete items
- `useChatStore.ts` — remove `codingStateByThread`, `codingRunningIds`, `codingRecentlyCompleted` slice and the four action methods
- `PluginsView.tsx` — remove `CodingMemoryPlugin` import + section
- `Messages.tsx` / `MessageRows.tsx` — remove `CostCeilingBanner`, `DeadEndWarning`, `RecallTrayCard`, `useApprovalQueue`, `useFileEditEvents`, `ApprovalCard`, `DiffPreview`, `ApprovalDecision`

**Query keys & event bridges:**
- `src/lib/query/queryKeys.ts` — remove `codingMemory` subtree (lines 95-101)
- `src/lib/query/entityKindMap.ts`, `tauriEventBridge.ts` — remove `coding_memory_*` → `codingFact` mappings

### 1.5 Frontend — preserve (with care)

- **`ThreadEventBuffer.ts` & `agent:thread_event` listener** in `features/coding/state/` — this owns the *only* global subscription that feeds streaming chat events into the Zustand store. **Move it to `src/services/threadEvents.ts` (or similar) before deleting `features/coding/`.** `BrainEventBridge.tsx` also listens on this event; verify it still works after the relocation.
- **`CostPill`** — extract to `src/features/composer/components/CostPill.tsx` (or shared) before deleting `features/coding/`.
- **Slash-command registry** — only retain if you want slash commands in the unified assistant. If yes, move `features/coding/slash/registry.ts` to `src/features/composer/slash/`. If no, delete with the rest of `features/coding/`.

### 1.6 Database schema deletions

Created as a single new consolidating migration `crates/storage/migrations/002_drop_coding_mode.sql`:

```sql
-- Coding-only tables
DROP TABLE IF EXISTS coding_snapshots;
DROP TABLE IF EXISTS coding_approval_history;
DROP TABLE IF EXISTS approval_pattern_history;  -- if confirmed unused outside coding
DROP TABLE IF EXISTS coding_background_jobs;
DROP TABLE IF EXISTS coding_todos;
DROP TABLE IF EXISTS coding_reviews;
DROP TABLE IF EXISTS klynt_sessions;
DROP TABLE IF EXISTS ingest_event_log;
DROP TABLE IF EXISTS ingest_distillation_retry;
DROP TABLE IF EXISTS recall_invocations;
DROP TABLE IF EXISTS recall_weights;
DROP TABLE IF EXISTS cross_cli_transfer_log;
DROP TABLE IF EXISTS pending_invalidations;
DROP TABLE IF EXISTS workspaces;

-- Coding columns on shared tables (skill_versions, semantic_facts, episodic_memories, mirror_snippets)
-- SQLite supports DROP COLUMN since 3.35 (March 2021) — single-statement form works.
ALTER TABLE skill_versions DROP COLUMN scope;
ALTER TABLE skill_versions DROP COLUMN scope_repo_id;
ALTER TABLE skill_versions DROP COLUMN source_pattern_id;
ALTER TABLE skill_versions DROP COLUMN status;
ALTER TABLE mirror_snippets DROP COLUMN coding_alert_kind;
ALTER TABLE mirror_snippets DROP COLUMN coding_alert_severity;
-- For semantic_facts/episodic_memories scope_repo_id — keep iff cognitive uses it elsewhere; otherwise drop.

-- Indexes on dropped tables die with the tables. Drop sessions indexes that mention mode:
DROP INDEX IF EXISTS idx_sessions_mode;
DROP INDEX IF EXISTS idx_sessions_mode_updated_at;
DROP INDEX IF EXISTS idx_sessions_workspace_archived;

-- sessions table CHECK constraint relaxation: SQLite needs the new-table/copy/drop/rename dance.
-- Pattern already used by coding-memory migration 008. After this step the table only has 'assistant'.
-- (When SessionMode enum is fully deleted in step 7, also drop the mode column entirely.)
```

**Coding columns to drop from `sessions`** (added in the "Coding-in-chat columns" block in `storage/migrations/001_initial.sql`):
`cwd`, `repo_id`, `repo_branch`, `tool_profile`, `approval_mode`, `total_cost_usd`, `total_tokens`, `parent_session_id`, `workspace_id`, `forked_from_id`, `summary_message_id`, `ephemeral`. Verify each against `fork_session()` in `crates/storage/src/repos/session.rs` before dropping — `approval_mode` and `parent_session_id` may have non-coding readers.

**LanceDB on-disk:**
- Delete `~/.klyntbot/lance/todo_embeddings.lance/` (only fed via `feature-coding-todo`).
- Other 11 LanceDB collections (`cognitive_fact_embeddings`, `activity_embeddings`, `conv_embeddings`, `entity_embeddings`, `flashcard_embeddings`, `insight_embeddings`, `note_embeddings`, `task_embeddings`, `tree_node_embeddings`, `work_context_embeddings`, `community_embeddings`) are non-coding and stay.

**Files in `~/.klyntbot/` to delete on dev installs:**
- `KLYNTBOT-coding.md`
- `ingest.sock`, `ingest-buffer.jsonl`

**`_feature_migrations` rows** for `coding_memory`, `feature_coding_bash`, `feature_coding_todo` — leave in place on existing dev DBs (harmless dangling history). Fresh installs never insert them.

---

## 2. Shared infrastructure that must remain untouched

Audited against the deletion plan; the following subsystems do not have a single coding-only line and survive entirely:

- **`crates/common/`** (after step 7's enum cleanup)
- **`crates/tools-core/` + `tools-core-macros/`** — `Tool` trait, `ToolRegistry`, `ApprovalGate`, `RoutingContext`, `#[derive(Tool)]` macros. The `allowed_channels` machinery is **vestigial after this PR** and is removed as a coordinated change in step 7 (see §3).
- **`crates/approval/`** (minus `coding_policy.rs`) — `ApprovalGate::check` is called for every tool invocation in the unified assistant; grant persistence, classify hooks, decision channels all stay.
- **`crates/storage/src/pool.rs`** (`StoragePool`) — pure WAL-mode SQLite pool.
- **`crates/storage/src/vector_store/`** — LanceDB wrapper.
- **`crates/context_engine/`** — `ContextEngine`, `ContextSource`, `SourceContext`.
- **`crates/session/`** — session manager (mode-agnostic).
- **`crates/scheduling/`** — `CronExecutor`, `CronJob`.
- **`crates/bus/`** (minus deleted variants) — `DomainEventBus`, `MessageBus`.
- **`crates/notifications/`, `crates/providers/`, `crates/ai-core/`, `crates/ai-core-macros/`** — zero coding references.
- **`crates/channels/`** — email/IMAP/SMTP/Telegram/Discord adapters.
- **`crates/cognitive/`** (minus coding hooks) — user-model, episodic memory, Mirror facade, coaching.
- **`crates/klynt-process-hardening/`, `crates/klynt-hooks/`** — process security + hook engine.
- **`crates/mcp/`** — MCP server is mode-agnostic; only test fixtures reference coding.
- **`crates/klyntbot-server/`** — standalone MCP/HTTP server.
- **All `feature-*` assistant crates**: `feature-alarms`, `feature-finance`, `feature-notes`, `feature-productivity`, `feature-learning`, `feature-language-learning`, `feature-tasks`, `feature-coaching`, `feature-focus`, `feature-launcher`, `feature-insights`. None import coding crates; their `allowed_channels = "non_coding"` declarations become moot after step 7.
- **Dev HTTP server (`crates/desktop/src/dev_server/`)** — infrastructure stays; only coding dispatch lines are removed.
- **Specta + bindings infrastructure (`crates/desktop/src/specta_builder.rs` skeleton + `desktop-macros`)** — stays; only command-list entries change.

---

## 3. Dependency-ordered execution sequence

> **Invariant**: every commit compiles. The sequence is top-down (consumers before producers) so we never have dangling references.

### Step 0 — Branch setup
```bash
git checkout -b unify-to-assistant
cargo nextest run --workspace --no-fail-fast       # capture a green baseline
cd desktop-ui && bun run test && bun run typecheck # baseline frontend
```

### Step 1 — Extract reusable pieces from `features/coding/` before deletion
- Move `ThreadEventBuffer.ts` + `codingEventReducer.ts` → `desktop-ui/src/services/threadEvents.ts` (rename `coding*` symbols)
- Move `CostPill.tsx` → `desktop-ui/src/features/composer/components/CostPill.tsx`
- *(Decision point — slash commands)* Move `features/coding/slash/` → `features/composer/slash/` only if you want slash commands in the assistant.
- Verify `BrainEventBridge.tsx` still receives `agent:thread_event` after the move.
- Commit: `refactor(ui): extract thread-event buffer and CostPill from coding namespace`

### Step 2 — Frontend: delete coding UI
- Delete `features/coding/`, `features/plugins/coding-memory/`, `features/settings/components/sections/coding/`, `features/settings/components/sections/SettingsCodingSection.tsx`, `CodingRecallStats.tsx`, `AppModeSwitch.tsx`, `useAppMode.ts`, `useResetAppViewOnModeChange.ts`
- Delete CSS files + remove `@import` lines
- Prune branches from `MainApp.tsx`, `SidebarChatLayout.tsx`, `useMainAppLayoutSurfaces.ts`, `DesktopLayout.tsx`, `buildPrimaryNodes.tsx`, `Composer.tsx`, `useComposerAutocompleteState.ts`, `useChatStore.ts`, `PluginsView.tsx`, `Messages.tsx`, `MessageRows.tsx`
- Remove settings nav entry, section IDs, labels, orchestration props
- Remove `coding_memory` from query keys / event bridges
- Adjust tests: drop coding-only test files, surgically prune coding assertions from shared tests
- **`bun run typecheck` will fail** at `src/api/endpoints/thread.ts` because it still calls `coding_thread_*` invoke names. Don't fix yet — fix in step 4.
- Commit: `feat(ui): remove coding-mode UI and unify single-experience surface`

### Step 3 — Backend: introduce unified thread commands
- Add new `thread_*` commands (`thread_start`, `thread_resume`, `thread_read`, `thread_list`, `thread_fork`, `thread_archive`, `thread_set_name`, `thread_subscribe`, `thread_unsubscribe`, `message_send`, `turn_interrupt`, `turn_steer`) in `crates/desktop/src/commands/thread.rs` (or rename the existing `chat.rs` set). These delegate to the same `AppCore` handlers that `coding_thread_*` currently use — for now, both command sets exist side-by-side.
- Register the new commands in `specta_builder.rs::klynt_collect_commands!`.
- Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`.
- Update `desktop-ui/src/api/endpoints/thread.ts` to invoke the new names.
- Verify `bun run typecheck` passes and the app boots.
- Commit: `feat(ipc): add unified thread commands as the canonical surface`

### Step 4 — Backend: remove `coding_*` Tauri commands
- Delete all 17 `commands/coding_*.rs` files
- Remove ~80 entries from `klynt_collect_commands![]` in `specta_builder.rs`
- Remove 14 `dispatch_dev` calls from `dev_server/dispatch.rs`
- Update `commands/approval.rs` + `approvals.rs` to drop `desktop_shared::coding` and `app_core::coding::approval_handler` imports — replace with the assistant-only approval flow
- Regenerate `desktop-ui/src/bindings.ts` (via `cargo tauri dev`)
- Verify frontend still compiles
- Commit: `refactor(ipc): remove coding-mode Tauri commands`

### Step 5 — Backend: remove coding handlers + coding-only crates
- Delete `crates/app-core/src/coding/`, `coding_memory/`, `handlers/coding_*.rs`, `handlers/reforge.rs`, `init/coding_*.rs`, `runtime/coding.rs`, `tracing/providers/{claude_code,kimi,klynt}/`
- Delete `crates/agent/src/context_sources/{coding_recall,todo}.rs`, `handlers/{coding_synthesis,rule_artifacts}.rs`, `adapters/reforge_handlers.rs`
- Prune `agent_loop/mod.rs` (SessionMode::Coding branches) and `agent_loop/builder.rs` (coding_recall_service, coding_policies, with_classify_hooks)
- Delete `crates/approval/src/coding_policy.rs`
- Delete `crates/storage/src/repos/{coding_approval_history,coding_background_jobs,coding_reviews,coding_todo}.rs` and the corresponding `Repos` struct fields
- Delete `crates/desktop-shared/src/coding/` and `commands/coding_memory.rs`
- Prune `crates/cognitive/src/mirror/sources/approval_history.rs` (legacy_repo field) and remove `MirrorAlert::Coding` from `types.rs` / `narratives.rs` / `cost_ceiling.rs`
- Prune `crates/bus/src/domain_events.rs` (CodingMirrorAlert, CodingMemoryUpdated variants) and `app_core/desktop_emit` arms (`EntityKind::CodingFact`/`CodingEpisode`)
- Delete `crates/config/src/schema/coding.rs`, `coding_memory.rs` and their parent fields in `core.rs` / `mod.rs`
- Delete crates wholesale: `coding-ingest`, `coding-memory`, `coding-agents-md`, `feature-coding-bash`, `feature-coding-todo`. Remove their entries from the root `Cargo.toml` workspace members list and from the `klyntbot` facade re-exports.
- Remove their entries from `Cargo.toml` `[dependencies]` of every consuming crate
- Commit: `refactor(core): remove coding-mode handlers, crates, and config schema`

### Step 6 — Backend: reposition `klynt-core`
- *(Optional rename — defer if you want to keep diff smaller)* `git mv crates/klynt-core crates/actuator-tools` and update all `klynt_core::` paths workspace-wide. **Recommended to do as a separate follow-up PR** because the rename touches every consuming file and creates noisy review diffs.
- Strip `allowed_channels = "coding_only"` from `bash.rs`, `edit.rs`, `write.rs`, `apply_patch.rs`, `notebook_edit.rs`, `plan_mode.rs`, `read.rs`, `glob.rs`, `grep.rs`, `list_dir.rs`, `web_fetch.rs`
- Remove `history_repo: Option<Arc<CodingApprovalHistoryRepo>>` from tool fields (compilation will break here because `CodingApprovalHistoryRepo` was deleted in step 5; this is the cleanup of those references)
- Delete `crates/klynt-core/src/snapshots/` (mod.rs + repo.rs — `coding_snapshots` repo)
- Delete `crates/klynt-git-utils/src/ghost_commits.rs`
- Delete `crates/lsp-client/` (verify no remaining consumers first)
- Wire the now-`all`-channels tools into `app-core/src/init/mod.rs` so the unified assistant sees them. Confirm via `cargo run -- mcp tools --list`.
- Commit: `refactor(tools): make klynt-core tools available to unified assistant`

### Step 7 — Backend: collapse `SessionMode` and `ChannelMask`
This is the cross-cutting cleanup. Done last because every prior step still references `SessionMode::Assistant` in match arms.
- Remove the `Coding` variant from `SessionMode` (workspace `cargo check` will flag remaining match arms — fix each by deleting the coding case).
- Decide: keep `SessionMode::Assistant` as the sole variant (simpler for serde) **or** delete `SessionMode` entirely. Per the chosen decision (drop "mode" terminology entirely), the cleanest path is:
  - Delete the enum.
  - Drop `mode` column from `sessions` (in the same migration as the other drops — see §1.6).
  - Remove `session_mode` field from `SourceContext` (`crates/context_engine/src/source.rs`) and `RoutingContext` (`crates/tools-core/src/routing.rs`). Audit every `ContextSource` impl for branches on it.
- Remove `allowed_channels` from the `Tool` trait (`crates/tools-core/src/lib.rs:111`)
- Remove `#[tool(allowed_channels = ...)]` attributes from every `#[derive(Tool)]` site (every feature crate)
- Remove `Channel`, `ChannelMask`, `CODING_CHANNEL`, `CODING_ONLY` from `crates/common/`
- Remove `ChannelMask`-related parameters from `tools-core-macros`
- Verify `cargo nextest run --workspace` is green
- Commit: `refactor(core): remove SessionMode and Channel gating (now vestigial)`

### Step 8 — Schema: consolidating drop migration
- Add `crates/storage/migrations/002_drop_coding_mode.sql` (full SQL in §1.6)
- Existing dev installs: migration runs once, drops the tables, leaves `_feature_migrations` rows in place. Fresh installs: migration runs against tables that never existed (all `IF EXISTS`) — no-op.
- Run `cargo tauri dev` against a stale dev DB to verify no startup errors
- Commit: `chore(schema): drop coding-mode tables and relax sessions CHECK`

### Step 9 — Soul + skills cleanup
- Delete `DEFAULT_CODING_SOUL` from `crates/skill-system/src/soul.rs` and all related fields/load paths
- Delete `skills/coding-orchestrator/` (referenced by `DEFAULTS` in `crates/skill-system/src/defaults.rs:147`)
- Remove `KLYNTBOT-coding.md` from any dev `~/.klyntbot/` instances (one-time `rm`)
- Update `crates/feature-coding-todo/src/tool.rs:23` — already deleted, just confirming the dangling reference is gone
- Commit: `refactor(skills): delete coding soul and coding-orchestrator skill`

### Step 10 — Documentation pass
- Delete: `docs/architecture/subsystems/09-coding-mode.md`, `docs/architecture/crates/coding-ingest.md`, `docs/architecture/crates/coding-memory.md`
- Archive (move to `docs/superpowers/archive/`): all 8 coding-design specs in `docs/superpowers/specs/`, `docs/superpowers/notes/2026-05-02-coding-perf-pass.md`, `docs/superpowers/plans/2026-05-12-coding-bash-interactive-compute.md`
- Surgical edits:
  - `CLAUDE.md`: remove coding lines (specific lines listed in §6)
  - `docs/architecture/00-overview.md`: delete "End-to-end: coding-mode chat turn" section + its sequence diagram, remove subsystem-09 row from inventory, remove CM Mermaid node, remove per-mode soul references, remove coding tables from DB section, remove `allowed_channels = "coding_only"` example
  - `docs/architecture/README.md`: remove subsystem 09 row, the two crate-doc rows, "5 coding-ingest adapters" gotcha
  - `docs/architecture/TECH_DEBT.md`: triage each coding-related entry (close most, keep any that became debt about the cleanup itself)
  - `docs/architecture/subsystems/04-agent-runtime.md`: remove `KLYNTBOT-coding.md` / `DEFAULT_CODING_SOUL` mentions, "fabrication detection skipped in coding mode" note, `coding-orchestrator` skill reference
  - `docs/architecture/subsystems/07-tools-framework.md`: remove `coding_only` / `non_coding` examples and the entire `CodingApprovalPolicy` section
  - `docs/architecture/subsystems/08-assistant-features.md` line 354: remove feature-coding-bash cross-reference
  - `CONTRIBUTING.md` line 201: update `docs/coding-memory/` reference
  - `tests/integration/main.rs`: remove `mod coding_*` declarations
- Commit: `docs: remove coding-mode references and archive coding-design plans`

### Step 11 — Final verification
- `cargo build --workspace`
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all --check`
- `cargo machete` + `cargo +nightly udeps --workspace` (catch any orphan deps left in Cargo.tomls)
- `cd desktop-ui && bun run lint && bun run typecheck && bun run test && bun run build`
- `cargo tauri dev` — manual smoke test of new-chat, message send, approval flow, settings panel, MCP server (`klyntbot mcp tools --list` should show no `coding_*` tools)
- `./scripts/run_chat_perf_gates.sh` (TTFT / stream throughput / coalescer)
- Confirm `~/.klyntbot-dev/` dev install starts cleanly against a fresh DB AND against a stale DB with coding tables present (migration drops them on the second)
- Commit: `chore: verify post-cleanup build and runtime`

---

## 4. Database / schema considerations in detail

### 4.1 Why one consolidating migration is safer than deleting migration files

The repo is pre-release ("no user data to migrate"), but developers have dev installs at `~/.klyntbot-dev/` that already contain the coding tables. If we **delete** the `coding-memory` migration files instead of dropping the tables, two failure modes emerge:

1. **Checksum drift in `_sqlx_migrations`** — sqlx detects that a previously-applied migration file is missing and aborts startup with `migration checksum mismatch`. (Mitigation: the `_feature_migrations` table doesn't use checksums, but `_sqlx_migrations` does for any non-feature migrations.)
2. **Dangling tables forever** — the coding tables stay in the SQLite file consuming a few KB of space and showing up in `.schema` output, making the DB confusing for anyone inspecting it.

The single drop migration (`002_drop_coding_mode.sql` in `crates/storage/migrations/`) sidesteps both: it's idempotent (`DROP TABLE IF EXISTS`), applies cleanly to fresh installs (no-op) and stale installs (cleanup), and registers itself in `_sqlx_migrations` so future hash-check runs are stable.

### 4.2 `sessions.mode` CHECK constraint relaxation

SQLite does **not** support `ALTER TABLE … DROP CONSTRAINT`. The CHECK on `sessions.mode` must be relaxed via the standard create-new/copy/drop/rename pattern (see existing precedent in `crates/coding-memory/migrations/008_fix_coding_reviews_fk.sql`). The migration:

```sql
CREATE TABLE sessions_new ( ... );  -- without the CHECK, or with CHECK (mode = 'assistant') if keeping enum
INSERT INTO sessions_new SELECT ... FROM sessions WHERE mode = 'assistant';  -- drop coding rows
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;
-- Recreate indexes
```

**Side effect**: rows where `mode = 'coding'` are dropped (intentional — no coding sessions in unified product). On a fresh install this is empty; on a dev install it deletes the user's coding session history. Document this in the migration comment.

### 4.3 LanceDB cleanup is a no-op for non-developers

LanceDB collection directories live under `~/.klyntbot/lance/`. Since the only coding-specific collection is `todo_embeddings.lance` (and it's only created if `feature-coding-todo` ever ran), most users won't have it. A startup cleanup routine is overkill; mention in the migration's commit message that a developer can `rm -rf ~/.klyntbot-dev/lance/todo_embeddings.lance` if they want a tidy directory.

### 4.4 Verify columns before dropping

Three columns on shared tables look coding-specific but might have non-coding readers:
- `sessions.approval_mode` — used by `storage/src/repos/session.rs::fork_session()`. Audit whether forking is reachable via assistant flows.
- `sessions.parent_session_id` — same audit.
- `semantic_facts.scope_repo_id` / `episodic_memories.scope_repo_id` — column is defined in `cognitive/001_cognitive_tables.sql` (shared) and re-added by coding-memory; check if the cognitive crate writes anything to it outside coding-memory call paths.

For each: run `rg 'approval_mode|parent_session_id|scope_repo_id' --type rust` against the post-step-5 tree and either drop the column (no readers) or keep it (readers exist).

---

## 5. Testing strategy

### 5.1 What's being deleted vs. what stays

| Test category | Action |
|---|---|
| Coding-only integration tests (12 files in `tests/integration/coding_in_chat/`, `coding_memory_phase*`, etc.) | Delete wholesale (§1.3 of agent 5's report) |
| Crate-local coding tests (`crates/agent/tests/coding_*.rs`, `crates/common/tests/coding_*.rs`, etc.) | Delete wholesale |
| Shared tests with coding branches (`useChatStore.test.ts` coding slice, `DesktopLayout.test.tsx` codeLandingNode case, `tauri.test.ts` coding_thread_* assertions) | Surgically prune |
| Assistant tests (`feature-tasks`, `feature-finance`, etc.) | **Untouched** |
| Chat perf gates (`scripts/run_chat_perf_gates.sh`) | Untouched — these test the chat runtime, not the mode |
| Property soak (`run_chat_proptest_soak.sh`) | Untouched |

### 5.2 New tests to add

- **Migration test** (`crates/storage/tests/migration_002.rs`): build a stale DB with coding tables populated, run the migration, assert tables are gone and `sessions` has only 'assistant' rows.
- **Specta drift test**: the existing `bindings_are_current` test catches if a developer adds a Tauri command without regenerating bindings — this stays valuable.
- **No-coding-references test** (optional, paranoia-grade): a workspace-level test that greps for `coding`, `CodingMode`, etc., and fails if any production source file mentions them. Useful for preventing regressions during the rename of `klynt-core`. Implement as a `cargo test` that walks `crates/*/src/**/*.rs` excluding `Cargo.toml` history. Skip if you trust review.

### 5.3 Manual smoke test checklist (post-merge)

- `cargo tauri dev` boots cleanly with `KLYNTBOT_HOME=~/.klyntbot-dev`
- "New chat" creates a session
- Send message, receive streaming response
- Tool calls execute (try `tasks_list`, `notes_add`, and a tool from `klynt-core` like `read`)
- Approval gate prompts for sensitive tools (e.g., `bash`)
- Settings panel renders without "Coding" tab
- MCP tools list shows the expected set: `klyntbot mcp tools --list`
- Global hotkeys still work (Alt+Cmd+Period launcher)
- Tray window, distraction overlay, voice orb all open

---

## 6. Risks and regression mitigations

| Risk | Mitigation |
|---|---|
| **Frontend breaks because `coding_thread_start` is the only thread-start command** | Step 3 introduces unified `thread_*` commands *before* step 4 deletes the `coding_*` ones — keep both alive for one commit. |
| **Compilation breaks mid-deletion in dependency-cycle crates** | The 11-step sequence is dependency-ordered: consumers (UI, commands) before producers (handlers, crates). Run `cargo check --workspace` after every commit. |
| **`klynt-core` tools lose approval gating** | Their `ApprovalGate::check` calls are still in place — only the `allowed_channels` declaration is removed. `bash`, `edit`, `write` are classified `Destructive` and prompt the user via the standard approval flow. Verified: `CodingApprovalPolicy` was the *runtime* classifier specifically for the coding sandbox; without it, those tools fall back to their static `approval_class` declaration which is correct for the assistant. |
| **`SessionMode` removal breaks `ContextSource` impls that branch on it** | Step 7 runs `cargo check` and fixes each branch. Predominantly the soul source and the coding-recall source — both already deleted by step 5/9. |
| **MCP exposed-tools list drifts** | `default_exposed_tools()` in `crates/config/src/schema/mcp.rs` references tool names by string. Audit it in step 5; remove any names that no longer exist in the registry. |
| **Specta bindings regeneration creates noisy diff** | Run `cargo tauri dev` once at end of each backend-touching step; commit `bindings.ts` in the same commit so review captures the IPC surface change. |
| **Pre-existing `desktop` crate clippy exceptions hide new warnings** | CLAUDE.md notes `desktop` has pre-existing exceptions — don't relax them further. Run clippy with `-D warnings` to fail on new violations. |
| **Dev installs with stale data fail to start** | Migration `002_drop_coding_mode.sql` is idempotent (`IF EXISTS`) and runs at startup; tested in step 11. |
| **Reforge or coding-ingest had silent background tasks** | They live behind `feature-flag`-gated init in `app-core/src/init/coding_subscribers.rs` — deleting the init file is sufficient; no daemon needs separate shutdown. |
| **Telegram/Discord/email channels lose approval routing for coding tools** | `crates/channels/src/adapters/telegram_approval.rs:352` reference to `SessionMode::Coding` is test-only. Production approval flows are mode-agnostic. |

### Rollback strategy

Since this is a single-PR big-bang:
- The PR branch can be discarded (`git checkout main && git branch -D unify-to-assistant`) at any point before merge with zero impact.
- Post-merge rollback: `git revert` the merge commit. The dropped schema columns and tables come back as empty (the migration is one-way; reverting the code restores the table definitions but data is gone, which is acceptable pre-release).
- LanceDB collections deleted on disk are gone, but they get recreated on next ingest event — no rollback needed there.

---

## 7. Documentation updates (the punch list)

**Files to delete:**
- `docs/architecture/subsystems/09-coding-mode.md` (536 lines)
- `docs/architecture/crates/coding-ingest.md` (636 lines)
- `docs/architecture/crates/coding-memory.md` (741 lines)

**Files to archive** (move to `docs/superpowers/archive/`):
- `docs/superpowers/specs/2026-05-07-coding-approval-handler-and-diff-preview-design.md`
- `docs/superpowers/specs/2026-05-07-coding-sidebar-titles-and-running-state-design.md`
- `docs/superpowers/specs/2026-05-07-coding-todowrite-design.md`
- `docs/superpowers/specs/2026-05-08-coding-background-bash-design.md`
- `docs/superpowers/specs/2026-05-08-coding-plan-mode-design.md`
- `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`
- `docs/superpowers/specs/2026-05-10-coding-bash-interactive-compute-design.md`
- `docs/superpowers/notes/2026-05-02-coding-perf-pass.md`
- `docs/superpowers/plans/2026-05-12-coding-bash-interactive-compute.md` (5,243 lines)

**Surgical edits to `CLAUDE.md`:**
- Line 80: "three end-to-end sequence diagrams (assistant turn, coding turn, nightly reforge)" → "two end-to-end sequence diagrams (assistant turn, nightly reforge)"
- Line 82: rewrite the dual-mode paragraph as "Single session type. Tools are unified — no channel partitioning."
- Line 99: remove `CodingApprovalPolicy` mention from the approval-gate bullet
- Line 153: leave `KLYNTBOT.md` reference (still valid); update `DEFAULT_SOUL` sentence to drop the "or coding fallback" framing
- Line 170: delete the coding-ingest bullet
- Line 172: delete the snapshot-rewind bullet
- Lines 174–176: delete `SessionMode` immutability + per-mode soul + assistant tool gating bullets
- Reference to `2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md` (file doesn't exist anyway) — delete

**Surgical edits to `docs/architecture/00-overview.md`:**
- Delete the entire "End-to-end: coding-mode chat turn" section + sequence diagram
- Remove subsystem-09 row from the 14-subsystem inventory table; update count to 13 (or "subsystems" loosely)
- Remove `classDef coding` Mermaid CSS and the CM node from the main architecture diagram
- Remove `~/.klyntbot/KLYNTBOT-coding.md` from glossary entry for "Soul"
- Remove `coding_snapshots`, `coding_thread_messages`, `coding_tool_calls` from the schema section

**Surgical edits to `docs/architecture/README.md`:**
- Remove subsystem-09 index row; remove the two crate-doc rows; remove "5 coding-ingest adapters" gotcha

**`docs/architecture/TECH_DEBT.md`:**
- Per-entry triage. Most coding-mode debt entries can be closed wholesale. Keep any that are about cleanup quality (e.g., "approval-pattern-history kept after coding removal — verify shared usage").

**`docs/architecture/subsystems/04-agent-runtime.md`:**
- Delete `KLYNTBOT-coding.md` / `DEFAULT_CODING_SOUL` / "fabrication detection skipped in coding mode" mentions
- Remove the `coding-orchestrator` skill from the listed 6 default skills

**`docs/architecture/subsystems/07-tools-framework.md`:**
- Remove `allowed_channels` from the `#[derive(Tool)]` attribute table (after step 7 it doesn't exist)
- Delete the entire `CodingApprovalPolicy` section

**`docs/architecture/subsystems/08-assistant-features.md` line 354:**
- Remove feature-coding-bash cross-reference

**`CONTRIBUTING.md` line 201:**
- Replace `docs/coding-memory/` with the current `docs/architecture/` location

**`tests/integration/main.rs`:**
- Remove `mod coding_in_chat`, `mod coding_memory_phase*` declarations

---

## 8. Follow-up tech debt (deliberately out-of-scope for this PR)

These create noise during review if mixed in, but are worth tracking:

1. **Rename `klynt-core` → `actuator-tools`** — touches every consuming crate; do as a focused rename PR with no logic changes.
2. **Reposition `plan_mode`** — currently a coding workflow primitive. Either delete (if the assistant doesn't need a "deliberation gate") or reposition as a general high-stakes-action confirmation pattern. Has its own UI (`coding_plan_*` commands → already deleted in step 4; the Rust tool stays via `klynt-core`).
3. **Audit `feature-productivity` "coding" activity-category strings** — they're string labels (e.g., user-defined activity called "coding"), not crate references. Decide whether to rename for clarity now that the term is gone.
4. **Consider deleting `feature-coaching`, `feature-insights`** if they were only meaningful in the dual-mode product. (Not flagged as coding-only by the audit, but worth user review.)
5. **MCP server tool exposure trimming** — `default_exposed_tools()` in `crates/config/src/schema/mcp.rs` may still expose a wider tool set than the assistant needs. Audit after the cleanup settles.
6. **`crates/desktop` clippy exceptions** — listed as pre-existing in CLAUDE.md. Now might be a good time to address them since the desktop crate just lost ~80 commands.
7. **External evaluation suite wiring** — CLAUDE.md notes `kca-bench` was removed and replacement (LoCoMo + Letta) is pending. Unrelated but blocks confident pre-release validation.
8. **Update `KLYNTBOT.md` content** — the assistant's system prompt should be rewritten to reflect the second-brain positioning and the expanded tool catalog (now includes shell/edit/web_fetch). Not in the cleanup PR, but the first follow-up.

---

## 9. Onboarding implications

A new contributor joining post-cleanup will land in a workspace where:

- `docs/architecture/00-overview.md` describes a single product, no mode partitioning, ~50 crates (down from 64).
- The `Tool` trait has no `allowed_channels` — adding a tool is unconditionally exposed to the one assistant.
- `SessionMode` does not exist; the codebase has no notion of "modes."
- `KLYNTBOT.md` is the only system-prompt file.
- The `klynt-core` (or `actuator-tools` after follow-up rename) crate is "the assistant's actuator tools" — bash, edit, web_fetch, etc. Not "coding tools that survived."

Add a short paragraph to `CLAUDE.md` (the "Architecture" section) reflecting this; the existing dual-mode framing has to be replaced rather than just trimmed.

---

## 10. Execution checklist (copy this into a task tracker)

- [ ] Step 0 — branch + baseline tests green
- [ ] Step 1 — extract `ThreadEventBuffer`, `CostPill`, (optional) slash commands
- [ ] Step 2 — frontend coding UI deletion + prune branches
- [ ] Step 3 — backend unified `thread_*` commands added
- [ ] Step 4 — backend `coding_*` commands deleted; bindings regenerated
- [ ] Step 5 — coding handlers + crates deleted; `Repos` fields removed
- [ ] Step 6 — `klynt-core` tools repositioned (channel gate stripped, history_repo removed, snapshots + ghost_commits + lsp-client deleted)
- [ ] Step 7 — `SessionMode` + `ChannelMask` + `allowed_channels` removed workspace-wide
- [ ] Step 8 — `002_drop_coding_mode.sql` consolidating migration added
- [ ] Step 9 — soul + skills cleanup
- [ ] Step 10 — documentation pass (delete, archive, surgically edit)
- [ ] Step 11 — final verification (cargo, clippy, fmt, machete, nextest, frontend, smoke test, dev DB migration test)
- [ ] PR description includes the before/after table, the deleted-crate list, the migration note, and the rollback procedure

---

## Appendix A — Crate count delta

**Deleted (5 crates):**
`coding-ingest`, `coding-memory`, `coding-agents-md`, `feature-coding-bash`, `feature-coding-todo`

**Deleted (1 crate via Step 6):**
`lsp-client`

**Renamed (1 crate, follow-up PR):**
`klynt-core` → `actuator-tools`

**Pre-PR count:** 64 workspace crates
**Post-PR count:** 58 workspace crates (~9% reduction)
**Post-rename count:** still 58 (same crate, new name)

## Appendix B — Files deleted (rough count)

- Backend Rust: ~250 files (5 whole crates + ~50 modules inside shared crates + ~17 desktop command files + tests + fixtures)
- Frontend: ~140 files (`features/coding/` + `features/plugins/coding-memory/` + settings coding section + CSS + mode store)
- Documentation: ~12 files (3 deleted, 9 archived)
- Total estimated deletions: **~400 files, ~35-45k LOC removed**

## Appendix C — One-line summaries for commit messages

- `refactor(ui): extract thread-event buffer and CostPill from coding namespace`
- `feat(ui): remove coding-mode UI and unify single-experience surface`
- `feat(ipc): add unified thread commands as the canonical surface`
- `refactor(ipc): remove coding-mode Tauri commands`
- `refactor(core): remove coding-mode handlers, crates, and config schema`
- `refactor(tools): make klynt-core tools available to unified assistant`
- `refactor(core): remove SessionMode and Channel gating (now vestigial)`
- `chore(schema): drop coding-mode tables and relax sessions CHECK`
- `refactor(skills): delete coding soul and coding-orchestrator skill`
- `docs: remove coding-mode references and archive coding-design plans`
- `chore: verify post-cleanup build and runtime`
