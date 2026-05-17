# KlyntBot — Technical Debt Inventory

> **Living document.** Categorized, not chronological. Updated as items are found, fixed, or re-evaluated.
> **Last refreshed:** 2026-05-17 (Batch A hygiene — closed 5 more entries: Cargo `0.1.1` accepted as intentional, KCA Track 7 + Focus-session both already self-confirmed documented, two duplicate `desktop --hook` rows merged-and-removed (also documented in `crates/coding-ingest.md`). Cumulative ~38 entries closed today).
> **Scope:** the whole Rust workspace + `/desktop-ui` frontend. Not third-party deps.
> **Total entries:** ~130 across 9 categories.

## How to use this file

This isn't a backlog — there's no priority sort beyond severity buckets. It's a **searchable inventory** so you can:

- **Before a release:** grep for `P0` entries; resolve or accept.
- **Before touching an area:** search for the file path or crate name; check what's known broken or in transition before you make assumptions.
- **During code review:** check whether the change touches anything listed here. If yes, the PR is implicitly resolving (or adding to) tech debt.
- **When something surprises you:** add an entry. The cost of capturing now is much lower than re-discovering later.

### Severity guide (not a roadmap)

- **P0** — Visible to users, or breaks an explicit user-facing claim (e.g., README says X works, X doesn't).
- **P1** — Invisible today, blocks a near-term feature, or contains a real correctness gotcha.
- **P2** — Ergonomic / maintainability cost. No immediate impact.
- **P3** — Cosmetic / dead code / stale comment.

Severity is a triage hint, not a commitment to fix in any particular order.

### Format

Every entry has:
- **Severity** (P0–P3)
- **Location** — file:line where possible, OR a clear reference to where the issue lives
- **Description** — what's actually wrong
- **Notes** — optional context (what would close it, why it matters, related debt)

### Closing rules

- **Don't close an item by deleting the entry alone.** Confirm by grepping the cited file:line and verifying the change shipped. Then remove the row.
- **Don't add aspirational debt.** "Refactor X" without a concrete pointer doesn't belong here — write a spec under `docs/superpowers/specs/` instead.
- **Promote/demote severity freely.** Today's P3 cosmetic can become a P0 if it starts blocking users; the reverse happens too.
- **Avoid duplicate entries.** If two findings are the same root cause, write it once and cross-link the symptoms.

## How to read this

Every entry has:
- A **severity** guess (P0 / P1 / P2 / P3) — not a roadmap commitment, a triage hint.
  - **P0** = visible to users or breaks an explicit user-facing claim.
  - **P1** = invisible today but blocks a near-term feature.
  - **P2** = ergonomic / maintainability cost; no immediate impact.
  - **P3** = cosmetic / dead code / stale comment.
- A **file:line** citation so you can jump to it.
- Optional context — what would be required to close it.

Add entries during normal work — don't batch. Remove entries when closed; **don't** mark them "fixed" in this file (the history is in git).

---

## Contents

1. [Pure TODO / FIXME / `unimplemented!()`](#1-pure-todo--fixme--unimplemented)
2. [Stubs & phased gates](#2-stubs--phased-gates)
3. [Legacy code paths in active use](#3-legacy-code-paths-in-active-use)
4. [Stale references & dead consts](#4-stale-references--dead-consts)
5. [Documentation drift](#5-documentation-drift)
6. [Hardcoded values awaiting config](#6-hardcoded-values-awaiting-config)
7. [Architectural anomalies](#7-architectural-anomalies)
8. [Naming & convention inconsistencies](#8-naming--convention-inconsistencies)
9. [Untracked surfaces](#9-untracked-surfaces)

---

## 1. Pure TODO / FIXME / `unimplemented!()`

| Sev | Location | Item | Notes |
|---|---|---|---|
| P1 | `crates/lsp-client/src/lib.rs:42` | `diagnostics_for` — `TODO(T5): Send textDocument/didOpen, wait for publishDiagnostics, return` | Returns empty. Blocks coding-mode diagnostic display. |
| P1 | `crates/lsp-client/src/lib.rs:59` | `document_symbols` — `TODO(T5): Send textDocument/documentSymbol, parse response` | Returns empty. |
| P1 | `crates/lsp-client/src/server_pool.rs:24` | Server pool stores nothing; `TODO(T5): Store the async-lsp ClientSocket here` | No actual server registration. |
| P1 | `crates/lsp-client/src/server_pool.rs:58,86` | Spawn no-op; shutdown no-op | Full T5 implementation note in code. |
| P2 | `crates/voice-engine/src/phoneme_aligner.rs:48` | `TODO: Integrate qwen3_asr forced alignment API` | Pronunciation scoring runs without real alignment. |
| P2 | `crates/voice-engine/src/phoneme_aligner.rs:64` | `TODO: Use pitch-detection crate (YIN) to extract F0 contour per syllable` | Same — no real F0 data. |
| P2 | `crates/voice-engine/src/tone_analyzer.rs:81` | `TODO: Extract actual F0 contour per syllable using pitch-detection crate` | Returns placeholder values. |
| P2 | `crates/voice-engine/src/error_classifier.rs:44` | `TODO: actual vs expected comparison` | Uses `p.phoneme.clone()` placeholder. |
| P2 | `crates/voice-engine/src/service.rs:753` | `TODO: tee the audio stream during capture to actually write the WAV file` | WAV recording not implemented. |
| P2 | `crates/cognitive/src/mirror/sources/skill_effectiveness.rs:77,84` | `TODO(T7): Extract tool_name and success from coding-memory queries` | Mirror source wired but produces no data. |
| P2 | `crates/feature-language-learning/src/practice_tool.rs:84` | `TODO: Query pronunciation_logs for the current session` | |
| P2 | `crates/feature-language-learning/src/practice_tool.rs:91` | `TODO: Query phoneme_mastery for low-stability phonemes` | |
| P2 | `crates/feature-language-learning/src/pronunciation_provider.rs:35` | `TODO: Wire the full pipeline when phoneme aligner produces real data` | Depends on phoneme_aligner TODOs above. |
| P2 | `crates/coding-ingest/src/adapters/kimi_cli/mapper.rs:265` | `TODO(distiller): attach token usage to the prior AssistantMsg row` | Token accounting incomplete for kimi-cli adapter. |
| P3 | `crates/app-core/src/handlers/coding_todo.rs:268` | `TODO: cache empty CompiledRules in a static once the type supports it` | Micro-optimization. |
| P3 | `crates/app-core/src/handlers/coding_plan.rs:203` | `TODO: Spawn untitled-rename watcher if title was empty` | UX polish. |
| P3 | `crates/app-core/src/coding/recall_stats_handler.rs:33` | `TODO: wire up recall_invocations repo once coding-memory telemetry is …` | Telemetry surface incomplete. |
| P3 | `crates/app-core/src/init/coding_subscribers.rs:51` | `TODO: wire actual success/failure once ToolCallExecuted carries it` | |
| P3 | `crates/app-core/src/init/temporal_scheduler.rs:19-21` | `DEFAULT_MATERIALIZE_AHEAD = 3` hardcoded; spec §3.2 references config field | See category #6. |
| P3 | `crates/app-core/src/handlers/cron.rs:94,221` | `TODO(4.4c): wire to TemporalScheduler::is_running() once CronService is retired` | Depends on scheduler migration. |
| P3 | `crates/common/src/notify.rs:195` | `TODO(priority-toast): consider adding <audio src="ms-winsoundevent:Notification.Looping.Alarm"/>` | UX polish. |
| P3 | `crates/platform-macos/src/lifecycle.rs:176` | NSWorkspace observers stubbed; `TODO: wire objc2 blocks` | App lifecycle events not observable yet. |
| P3 | `crates/platform-macos/src/computer_use/capture.rs:97` | `scale: 2.0, // TODO: replace with NSScreen.backingScaleFactor in Phase 2` | Hardcoded Retina assumption. |
| P3 | `crates/klynt-process-hardening/src/lib.rs:95` | `TODO: Windows hardening (Job Object, mitigations) is out of scope for Phase 3` | Acknowledged non-goal. |
| P3 | `tests/integration/mcp_alarm_tool.rs:56` | `TODO(alarm-tool): uncomment once AlarmTool is built and registered in MCP exposure` | Test scaffolding pending. |

---

## 2. Stubs & phased gates

| Sev | Location | Item | Notes |
|---|---|---|---|
| **P0** | `crates/plugin-runtime/src/host/mod.rs:477` | `agent_ask_user` host function returns `{"error":"agent callbacks not connected"}` unconditionally | **User-facing surprise:** granting plugin `Agent` permission does nothing. Tracked as "Task #8" in code. |
| **P0** | `crates/notifications/src/channel/mod.rs:64` | `TelegramNotificationChannel`, `DiscordNotificationChannel`, `EmailNotificationChannel` exist as types but are **not wired** into `NotificationDispatcher` | **User-facing surprise:** alarms with those channel bits silently no-op. Comment defers to "4.8 / follow-up." |
| **P0** | `crates/mcp/src/server/approval.rs:6,21` | `BlockingFallbackChannel::desktop_prompt()` always returns Decline for MCP server-side approval | **User-facing surprise:** remote MCP clients cannot get approval for sensitive tools. |
| P1 | `crates/coding-memory/src/reforge_phase.rs:104` | Multiple Reforge phases return `NotImplementedInPhase` at runtime | Phased rollout — by design but limits production surface. |
| P1 | `crates/coding-memory/src/reforge_phase.rs` | `CodingSynthesisPhase` (2.5) and `RuleArtifactGenerationPhase` (3.5) return `NotImplementedInPhase { required_phase: 5 }` in the legacy trait file. **However**, real implementations exist in `reforge/coding_synthesis.rs` and `reforge/rule_artifacts.rs` and are wired into `app-core::CodingPhaseRunnerImpl`. `SessionEndPass` and `CrossSessionDedup` in `reforge/` are fully implemented. | Legacy stub file shadows real implementations; should be deleted once fully migrated. |
| P1 | `crates/plugin-sdk/src/lib.rs` (`db_query` impl) | Reads var `__db_query_not_implemented`, returns `"[]"` — SDK-side is a no-op placeholder | Real call goes through host function; this dead code is misleading. |
| P2 | `crates/klynt-protocol::HookExecutionMode::InProcess` + `crates/klynt-hooks/src/engine/dispatcher.rs` | `InProcess` variant typed but **no dispatch path**. Only `Subprocess` mode is wired. | Decide: implement in-process hooks or remove the variant. |
| P2 | `crates/klynt-hooks/src/engine/dispatcher.rs` | `Hook.fail_open` field defined in schema but **ignored** — fail-open is hardcoded. Hook errors silently dropped. | Security: hooks can't actually enforce blocks. Respect the field or remove it. |
| P3 | `crates/cognitive/src/mirror/sources/skill_effectiveness.rs:77,86` | `accumulate` + `flush` are no-ops with `TODO(T7)` comments | Mirror source wired but inert. |

---

## 3. Legacy code paths in active use

These are not bugs — they're real fallback paths kept alive during a migration. Goal: delete them when the migration completes.

| Sev | Location | Item | Notes |
|---|---|---|---|
| P1 | `crates/scheduling/src/service/mod.rs:3` + `crates/app-core/src/init/temporal_scheduler.rs:3-5,99` | `CronExecutor` and `TemporalScheduler` run side-by-side; runtime info-log "TemporalScheduler started (side-by-side with CronService)" | Phase 3 consolidation incomplete. |
| P1 | `crates/storage/src/repos/session.rs:914,933,966` | `SessionRepo` mirrors `Text` parts into the legacy `messages.content` column on every write; reads fall back to wrapping legacy `content` in a `Text` part | Anthropic-shape compatibility. Long-lived. |
| P2 | `crates/feature-tasks/src/tool/actions/query.rs:203-209` | Falls back to "legacy status-based summary" when `summary_by_group` fails | Logged at `warn!`. |
| P2 | `crates/scheduling/src/temporal/cron_bridge.rs:4` | "Bridge between legacy `cron_jobs` (definition table) and `scheduled_fires` (firing table)" | Bridge exists to keep two storage models reconciled. |
| P2 | `crates/cognitive/src/services/background.rs:310,1100` | "Legacy broadcast-collector startup that lived here is …"; "now as it is still referenced by legacy unit tests; primary pipeline …" | Dead code referenced by tests; can't delete without test refactor. |
| P2 | `crates/app-core/src/init/cognitive.rs:132` | "The legacy ActivityLogSubscriber bus subscription has been removed" | Confirms a delete already happened; comment can stay. |
| P2 | `crates/agent/src/agent_loop/mod.rs:1200` | "The legacy `mode: Option<String>` parameter is now an override hint" | Soft-deprecated parameter still accepted. |
| P3 | `crates/coding-ingest/src/adapters/codex/mod.rs:8` | "The legacy `dispatch` and `payload` modules below are retained as dead …" | Acknowledged dead code. |
| P3 | `crates/agent/src/agent_loop/builder.rs:706` | "Notification dispatcher removed (Phase 3): legacy agent::NotificationDispatcher …" | Phase 3 cleanup remnant. |
| P3 | `crates/feature-productivity/src/feature.rs:39` | Migration description: "Create productivity tracking tables (removed legacy focus_sessions)" | Schema cleanup. |
| P2 | `crates/agent/src/agent_runtime/runtime.rs` (SourceContext) | `intent_summary: Option<String>` is **always `None`** in the current flat runtime — `intent_pipeline` module no longer exists | Vestigial field. Decide: delete or repurpose. |

---

## 4. Stale references & dead consts

| Sev | Location | Item | Notes |
|---|---|---|---|

---

## 5. Documentation drift

| Sev | Location | Item | Notes |
|---|---|---|---|
| P1 | Bundle budget not wired into any merge-gate script | `.size-limit.json` exists (threads route ≤ 350 kB gzipped, total ≤ 2.5 MB) but no script invokes `size-limit` — only `bun run size-limit` manually. | Wire into `run_chat_perf_gates.sh` or similar so regressions fail CI. |
| P1 | TTFT perf gate is a no-op skeleton | `scripts/run_chat_perf_gates.sh` runs `agent/benches/ttft_e2e.rs` with `THRESHOLD_TTFT_P95_MS=25` (default — NOT 15ms as docs claim) but prints `"numeric gate deferred to PR8"` and never `exit 1`s. | Implement the numeric assertion. |
| P1 | `crates/plugin-runtime/src/manifest.rs` (`PluginCronJob`) | Manifest deserializes `cronJobs: Vec<PluginCronJob>` but **no executor reads them**. Plugins can declare cron jobs and they will never fire. | Implement plugin-cron executor or remove the field. |
| P1 | `crates/platform-macos/src/computer_use/ax_walker.rs` (`AccessibilityNode.frame`) | Frame coordinates are in **AppKit (bottom-left)** space, not Quartz (top-left) as the rest of `PlatformInput`/`PlatformCapture` API documents. Y-flip is a Phase 4 TODO. | Significant correctness gotcha for future Computer Use. Fix y-flip or document the inconsistency loudly. |
| P2 | `crates/platform-macos/src/computer_use/input.rs` (`MacInput`) | `Screenshot` + `Zoom` variants return `NotImplemented` (14 of 16 `ComputerUseAction` work). | |
| P2 | `crates/platform-macos/src/computer_use/capture.rs` (`MacCapture`) | `capture_window` + `get_active_window` return `NotImplemented` (3 of 5 `PlatformCapture` methods work). Plus `scale` hardcoded `2.0` instead of `NSScreen.backingScaleFactor`. | |
| P2 | `crates/platform-macos/src/dnd.rs` (`toggle_dnd`) | Calls `shortcuts run "Toggle Do Not Disturb"` — **requires user to manually create that Shortcut**. Brittle setup dependency not surfaced anywhere. | Document loudly or auto-install the Shortcut. |
| P2 | `desktop-ui/src/features/threads/store/chatStreamStore.ts` | Legacy v1 event bridge — ~30 `agent:*` Tauri event listeners. Still active for assistant chat; coding threads migrated to v2 `ThreadEvent`. | Plan + schedule the v2 migration cut for assistant chat. |
| P2 | OAuth callback uses fixed `CALLBACK_PORT` | `crates/desktop/src/oauth/` — no fallback if port is in use. | Add port retry or document. |
| P2 | Embedded MCP HTTP server has no `/health` route | Status only via `get_status` MCP tool or `klyntbot://status` resource — external HTTP monitors can't probe. | Add `/health` endpoint. |

---

## 6. Hardcoded values awaiting config

| Sev | Location | Item | Notes |
|---|---|---|---|
| P2 | `crates/app-core/src/init/temporal_scheduler.rs:19-21` | `DEFAULT_MATERIALIZE_AHEAD = 3` hardcoded; spec §3.2 references `config.notifications.default_materialize_ahead` | Promotion to config deferred. |
| P3 | `crates/platform-macos/src/computer_use/capture.rs:97` | `scale: 2.0` hardcoded | Should use `NSScreen.backingScaleFactor`. |
| P3 | `crates/scheduling/Cargo.toml:13-16` | Both `chrono`/`chrono-tz` and `jiff` dependencies — `cron` crate API boundary requires `chrono::TimeZone` | Intentional dual dependency; ongoing friction. |

---

## 7. Architectural anomalies

These are not bugs but they violate stated invariants.

| Sev | Location | Item | Notes |
|---|---|---|---|
| P1 | `crates/storage/Cargo.toml:7` | `storage` depends on `ai-core.workspace = true` | **Upward dependency.** Layer model in CLAUDE.md places `storage` at L2 and `ai-core` (implicitly) higher. Decide: move trait to `common`? Invert? Formalize? |
| P1 | Tool wiring across crates | Four different paths: (a) `FeaturePackage::tools()`, (b) wire in `crates/agent/src/agent_loop/builder.rs`, (c) wire in `crates/app-core/src/init/mod.rs`, (d) wire per-invocation in `crates/agent/src/subagent.rs` | `TaskTool`, `AlarmTool`, `LearningTool` use path (b); `LauncherTool` uses path (c); `AgentTaskTool` uses path (d); everything else uses (a). Normalize? Document? |
| P2 | `crates/feature-alarms` | Only `feature-*` crate with **no** `FeaturePackage` impl | Schema migrations for `scheduled_fires` come from the `scheduling` crate instead. |
| P2 | `crates/feature-insights` | No `FeaturePackage` impl + no LLM tools at all | Pure backend service. Naming convention implies user-facing feature with tools. |
| P2 | `crates/feature-learning/src/feature.rs:43` | `FeaturePackage::tools()` returns empty; actual `LearningTool` lives in `crates/tools/src/domain/learning_tool.rs` | Misdirection; comment partially explains. |
| P1 | `cognitive/src/services/reforge/service.rs::run_reforge` | **26-parameter signature** (many `Option<&dyn Trait>` hooks) | Code smell. Refactor candidate: `ReforgeContext` builder. |
| P1 | Two `AutotunerBridge` traits with the same name | `cognitive/src/services/reforge/mod.rs::AutotunerBridge` (Phase 6 orchestration) vs `cognitive/src/mirror/types.rs::AutotunerBridge` (MirrorFacade champion promotion) | Confusing for cross-file work; rename one. |
| P1 | Two `retrievability` functions with different formulas | `cognitive/src/services/fsrs5.rs::retrievability` (power-law: `1/(1+t/9S)`) for flashcards vs `cognitive/src/services/decay.rs::retrievability` (exponential: `exp(ln(0.9)*t/s)`) for retrieval scoring | Importing the wrong one is silent and subtly wrong. Rename or co-locate. |
| P1 | `bus::domain_events::ConcurrencyClass` enum | Defined as `{ Safe, Sequential, Exclusive }` but **not used** by `Tool::is_concurrency_safe(args) -> bool` | Decide: wire the enum to Tool (third "Exclusive" tier) or remove. |
| P2 | `LearningTool` location | Lives at `crates/tools/src/domain/learning_tool.rs`, NOT inside `crates/feature-learning/` (which has `FeaturePackage::tools()` returning `vec![]`) | Naming mismatch confuses anyone looking for the implementation. |

---

## 8. Naming & convention inconsistencies

| Sev | Location | Item | Notes |
|---|---|---|---|
| P2 | `ToolOutput::Structured` (`crates/tools-core/src/lib.rs:165-220`) | Enum + `__STRUCTURED__` parsing convention defined and documented, but **zero production tools** emit it | Incomplete upgrade path. Decide: implement, or remove? |
| P2 | `TasksFeature::new()` without `.with_task_tool(...)` registers zero tools silently | `crates/agent/src/agent_loop/builder.rs:1353` is the only correct wiring | Footgun for plugins/tests that construct `TasksFeature::new()` expecting tools. |
| P2 | Two `AlarmFired` `kind` strings | `"cron_job"` (TemporalScheduler → CronExecutor internal dispatch) vs `"cron"` (`app-core/init/cron.rs::publish_cron_alarm` user-facing notifications) | Confusingly similar; same enum variant, different consumers. Rename one (e.g., `cron_user_notification`). |
| P2 | `CronHandler` is sync (`Fn`, not `AsyncFn`) | Dispatched via `tokio::task::spawn_blocking`; async work uses `tokio::task::block_in_place + rt.block_on(...)` | Footgun for new handlers; obvious only after reading existing impls. Consider `AsyncCronHandler`. |
| P2 | Plural / singular tool-name footgun | Registry keys: `tasks`, `notes` (plural) but `memory`, `finance`, `alarm` (singular). MCP exposure inherits. Calling `mcp__klyntbot__task` returns `ToolNotFound` | Standardize at next MCP whitelist refresh. |
| P2 | `MidLoopCompressor` vs `TieredHistoryCompressor` naming | Two compressors with overlapping concerns: `MidLoopCompressor` (in `agent::execution`) compresses stale tool results inside the ReAct loop; `TieredHistoryCompressor` (in `context_engine`) compresses turn history before context assembly | Similar names hide different scopes. Both can fire on the same turn. |
| P2 | `SkillSource` priority numbering | `User=0`, `ReforgePrivate=1`, `ReforgeTeam=2`, `Project=3`, `Mcp=4`, `SkillsMarketplace=5` — **higher number wins on collision**. Reverse of common file-precedence convention. | A user editing `~/.klyntbot/skills/` expecting their version to win for a project-scoped skill will be silently overridden. Document loudly or invert. |
| P2 | `Hook.matcher` / Glob rule tool-name normalization | Strips `_` and `-` and lowercases tool names before matching. So `BashTool`, `bash-tool`, `bash_tool`, and `bash` all match a `bash(*)` glob. | Footgun for case-sensitive systems; non-obvious behavior. |
| P2 | `CodingApprovalPolicy::YoloMode` expiry edge case | When `until` timestamp passes, `classify` returns `None`. The fallback logic in `Default` then applies — but the hardcoded fallback is `matches!(DefaultPolicy::Ask, DefaultPolicy::Allow)` (false). Effectively becomes "ask everything," NOT "go back to Default." | Subtle; document loudly or change semantics. |
| P2 | `BlockingFallbackChannel.capabilities()` mismatch | Claims `supports_classes: {Destructive, Admin}` but always returns Decline regardless of class. | Either tighten capabilities to advertise correctly, or rename to convey "I will decline but you should still ask me." |

---

## 9. Untracked surfaces

Things present in the codebase that aren't enumerated in any doc today.

| Sev | What | Where | Why it matters |
|---|---|---|---|
| **P0** | Computer Use platform layer is real | `crates/platform-input`, `crates/platform-capture`, `crates/platform-macos/src/computer_use/{capture,input,ax_walker}.rs` | Capture, input injection, AX tree walker all implemented and tested. **Not wired** into any agent tool, Tauri command, or MCP tool. See `subsystems/12-plugins-platform.md` for the full inventory. |
| P2 | Skill discovery roots (4 of them) | `klynt-skill-loader::discovery` | User / ReforgePrivate / Project / ReforgeTeam — priority order matters; not documented at the project level. |
| P2 | OS_NATIVE + Tray notification channels | `crates/notifications/src/channel/` | Notification-only surfaces (no chat). Listed separately from chat channels but conflated in some docs. |
| P2 | Voice has 5 engines | `Qwen3AsrEngine` + `CloudAsrEngine` (STT); `Qwen3TtsEngine` + `AvSpeechTtsEngine` + `CloudTtsEngine` (TTS) | `Qwen3TtsEngine` requires `--features qwen3` and isn't on by default. |
| P3 | `klynt-protocol` + `klynt-hooks` provenance | Crate `Cargo.toml` descriptions | "Adapted from codex-rs/protocol/" and "Adapted from codex-rs/hooks/" — origin worth crediting in subsystem docs. |
| P3 | 5 plugin host namespaces | `crates/plugin-runtime/src/host/mod.rs` | `db`, `log`, `http`, `agent`, `tool`. Three permissions: `Network`, `Storage`, `Agent`. Not enumerated outside source. |
| P2 | `strategy_records` table is raw-SQL accessed | `cognitive/src/services/reforge/feedback.rs:173` | No typed repo struct; reads via raw SQL. Inconsistent with every other table. |
| P2 | `coding_approval_history` retention | Grows unboundedly; no retention policy or compaction job. | Add a 90-day retention or similar. |
| P2 | `feature-productivity` 20 tables undocumented | Only `activity_events` and `daily_summaries` are mentioned in CLAUDE.md. 18 other tables (incl. `productivity_quality_scores`, `productivity_narratives`, `productivity_voice_journals`, `productivity_categorization_cache`, `productivity_privacy_rules`, `productivity_rule_evolution_log`, etc.) are completely undocumented. | Schema discoverability gap. |
| P2 | Cross-feature shared tables | `practice_sessions` is defined in `feature-notes` migration 002 but also used by `feature-language-learning`. `activity_events` (in `feature-productivity`) is read by `feature-launcher::AttentionAggregator`. | No enforcement mechanism prevents schema changes from breaking the other crate. Add a runtime ownership registry or move shared tables to a dedicated crate. |

---

## Closing rules

- **Don't close an item by deleting the entry alone.** Confirm by grepping the cited file:line and verifying the code change shipped. Then remove the row.
- **Don't add aspirational debt.** "Refactor X" without a concrete file or behavior pointer doesn't belong here — write a spec under `docs/superpowers/specs/` instead.
- **Promote/demote severity freely.** Today's P3 cosmetic can become a P0 if it starts blocking users; the reverse also happens when scope changes.
- **Avoid duplicate entries.** If two findings are the same root cause, write it once and cross-link the symptoms.
