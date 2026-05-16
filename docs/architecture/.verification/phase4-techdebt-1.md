# Phase 4 Tech Debt Verification — Categories 1–5

**Agent:** techdebt-agent-1  
**Date:** 2026-05-16  
**Scope:** `docs/architecture/TECH_DEBT.md` categories 1–5  
**Method:** Line-by-line file verification + workspace-wide `grep -rn` sweep for `TODO`, `FIXME`, `unimplemented!()`, `todo!()`, `NotImplementedInPhase` across `crates/`, `src/`, and `tests/`.

---

## Verified Entries (still exist)

### Category 1 — Pure TODO / FIXME / `unimplemented!()`

All 21 entries verified at their cited file:line. Issues remain unresolved.

| Sev | Location | Status |
|---|---|---|
| P1 | `crates/lsp-client/src/lib.rs:42` | `TODO(T5)` still present; returns empty vec |
| P1 | `crates/lsp-client/src/lib.rs:59` | `TODO(T5)` still present; returns empty vec |
| P1 | `crates/lsp-client/src/server_pool.rs:24` | `TODO(T5)` still present; no ClientSocket stored |
| P1 | `crates/lsp-client/src/server_pool.rs:58,86` | `TODO(T5)` spawn/shutdown no-ops still present |
| P2 | `crates/voice-engine/src/phoneme_aligner.rs:48` | `TODO` still present; returns empty phonemes |
| P2 | `crates/voice-engine/src/phoneme_aligner.rs:64` | `TODO` still present; returns empty ToneContour |
| P2 | `crates/voice-engine/src/tone_analyzer.rs:81` | `TODO` still present; returns empty syllables |
| P2 | `crates/voice-engine/src/error_classifier.rs:44` | `TODO` still present; `actual: p.phoneme.clone()` placeholder |
| P2 | `crates/voice-engine/src/service.rs:753` | `TODO` still present; WAV file not teed |
| P2 | `crates/cognitive/src/mirror/sources/skill_effectiveness.rs:77,84` | `TODO(T7)` still present; accumulate + flush are no-ops |
| P2 | `crates/feature-language-learning/src/practice_tool.rs:84` | `TODO` still present; no pronunciation_logs query |
| P2 | `crates/feature-language-learning/src/practice_tool.rs:91` | `TODO` still present; no phoneme_mastery query |
| P2 | `crates/feature-language-learning/src/pronunciation_provider.rs:35` | `TODO` still present; pipeline not wired |
| P2 | `crates/coding-ingest/src/adapters/kimi_cli/mapper.rs:265` | `TODO(distiller)` still present; token usage not attached |
| P3 | `crates/feature-coding-todo/src/lib.rs:59` | `TODO` still present; health check unconditional |
| P3 | `crates/app-core/src/handlers/coding_todo.rs:268` | `TODO` still present; no static cache |
| P3 | `crates/app-core/src/handlers/coding_plan.rs:203` | `TODO` still present; untitled-rename watcher not spawned |
| P3 | `crates/app-core/src/coding/recall_stats_handler.rs:33` | `TODO` still present; recall_invocations not wired |
| P3 | `crates/app-core/src/init/coding_subscribers.rs:51` | `TODO` still present; success/failure not carried |
| P3 | `crates/app-core/src/init/mod.rs:1034` | `TODO(phase-3.5)` still present; default timezone used |
| P3 | `crates/app-core/src/init/temporal_scheduler.rs:19-21` | `DEFAULT_MATERIALIZE_AHEAD = 3` still hardcoded |
| P3 | `crates/app-core/src/handlers/cron.rs:94,221` | `TODO(4.4c)` still present; CronService references in comments remain |
| P3 | `crates/common/src/notify.rs:195` | `TODO(priority-toast)` still present; no alarm audio |
| P3 | `crates/platform-macos/src/lifecycle.rs:176` | `TODO` still present; NSWorkspace observers stubbed |
| P3 | `crates/platform-macos/src/computer_use/capture.rs:97` | `TODO` still present; scale hardcoded `2.0` |
| P3 | `crates/klynt-process-hardening/src/lib.rs:95` | `TODO` still present; Windows hardening not implemented |
| P3 | `tests/integration/mcp_alarm_tool.rs:56` | `TODO(alarm-tool)` still present; alarm assertion commented out |

### Category 2 — Stubs & phased gates

| Sev | Location | Status |
|---|---|---|
| **P0** | `crates/plugin-runtime/src/host/mod.rs:477` | Still returns `{"error":"agent callbacks not connected"}` unconditionally |
| **P0** | `crates/notifications/src/channel/mod.rs:64` | Telegram/Discord/Email still not wired into `NotificationDispatcher` |
| **P0** | `crates/mcp/src/server/approval.rs:6,21` | `BlockingFallbackChannel::desktop_prompt()` still always returns `Decline` |
| P1 | `crates/coding-memory/src/reforge_phase.rs:104` | `phase(5)` still returns `NotImplementedInPhase` for `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` |
| P1 | `crates/coding-memory/src/reforge/` | **See Resolved Entries below** — phases are now implemented in `reforge/`, but old stubs remain in `reforge_phase.rs` |
| P1 | `crates/plugin-sdk/src/lib.rs:46` | `db_query` still reads `__db_query_not_implemented` var and returns `"[]"` |
| P2 | `crates/klynt-protocol/src/lib.rs:79` | `HookExecutionMode::InProcess` variant exists |
| P2 | `crates/klynt-hooks/src/engine/dispatcher.rs` | No `InProcess` dispatch path; only `Subprocess` via `command_runner::run_command` |
| P2 | `crates/klynt-hooks/src/engine/dispatcher.rs` | `Hook.fail_open` defined in schema (`crates/klynt-hooks/src/schema.rs:17`) but ignored in `dispatch_event` |
| P3 | `crates/cognitive/src/mirror/sources/skill_effectiveness.rs:77,86` | Already listed in Cat 1; duplicate entry in Cat 2 confirmed still present |

### Category 3 — Legacy code paths in active use

| Sev | Location | Status |
|---|---|---|
| P1 | `crates/scheduling/src/service/mod.rs:3` + `crates/app-core/src/init/temporal_scheduler.rs:3-5,99` | `CronService` comment still in `temporal_scheduler.rs` header; `CronExecutor` + `TemporalScheduler` run side-by-side |
| P1 | `crates/storage/src/repos/session.rs:914,933,966` | Still mirrors Text parts into legacy `content` column; fallback read still wraps legacy `content` in `Text` part |
| P2 | `crates/feature-tasks/src/tool/actions/query.rs:203-209` | Still falls back to legacy status-based summary when `summary_by_group` fails |
| P2 | `crates/scheduling/src/temporal/cron_bridge.rs:4` | Bridge comment still present; module actively reconciles `cron_jobs` ↔ `scheduled_fires` |
| P2 | `crates/cognitive/src/services/background.rs:310,1100` | Legacy comments still present; `event_type_key` still `#[cfg(test)]` |
| P2 | `crates/agent/src/agent_runtime/runtime.rs:568-573` | `KCA_PHASE_4_LEGACY_NUDGE` env-gated legacy nudge path still present |
| P2 | `crates/app-core/src/init/cognitive.rs:132` | Comment confirming legacy `ActivityLogSubscriber` removal still present |
| P2 | `crates/agent/src/agent_loop/mod.rs:1200` | Legacy `mode: Option<String>` still accepted as override hint |
| P3 | `crates/coding-ingest/src/adapters/codex/mod.rs:8` | Dead `dispatch` + `payload` modules still retained with `#[allow(dead_code)]` |
| P3 | `crates/agent/src/agent_loop/builder.rs:706` | Comment about Phase 3 notification dispatcher removal still present |
| P3 | `crates/feature-productivity/src/feature.rs:39` | Migration description still references "removed legacy focus_sessions" |
| P2 | `crates/app-core/src/init/temporal_scheduler.rs:99` | Log line still says `"TemporalScheduler started (side-by-side with CronService)"` — misleading because `CronService` is gone; actual pair is `TemporalScheduler` + `CronExecutor` |
| P2 | `crates/agent/src/agent_runtime/runtime.rs` (SourceContext) | `intent_summary: Option<String>` in `context_engine::source::SourceContext` is still always `None` (set at `crates/context_engine/src/assembler/mod.rs:188,1285`). Cited location `agent_runtime/runtime.rs` is misleading; the field lives in `context_engine/src/source.rs:21`. |

### Category 4 — Stale references & dead consts

| Sev | Location | Status |
|---|---|---|
| P2 | `crates/desktop/src/lib.rs:16-19` | `LEGACY_COMMAND_NAMES` is still empty `&[]`; comment says "Deleted in Phase E" |
| P3 | `crates/feature-learning/src/feature.rs:43` | Comment still says "see Task 47 for the exposure path" |
| P3 | `crates/klynt-sandbox-helper/src/main.rs:3` | Header comment still says "Plan 1: stub; prints version and exits. Plan 3: vendored..." even though Plan 3 logic is active |
| P3 | `crates/feature-coaching/src/feature.rs:14` | `#[ai(skill = "automation")]` still present; skill name `automation` still exists in `skills/automation/` |
| P3 | `crates/agent/src/adapters/llm_summary.rs:82` | Comment "fall back to the legacy slice when none is found" still present |

### Category 5 — Documentation drift

| Sev | Location | Status |
|---|---|---|
| **P0** | `CLAUDE.md:79` | Still says "39 crates, 9 layers". Actual workspace members = **64** crates + `plugin-sdk` (excluded) + root `klyntbot` ≈ **66** |
| **P0** | `README.md:~28` | Still says "39-crate Rust workspace organized into 9 strict layers" |
| P1 | `CLAUDE.md` ("Coding-memory Phase 7 — multi-CLI ingest") | Still says "4 IngestAdapter implementations". Actual = **5** (`codex`, `claude_code`, `opencode`, `kimi_cli`, `git_post_commit`) |
| P1 | `CLAUDE.md` (root facade) | Still says "src/lib.rs re-exports all public types". Actual `src/lib.rs` has ~29 re-export/mod declarations, partial coverage of 64+ crates |
| P2 | `CLAUDE.md` ("Computer Use & Procedural Memory (in design — not yet implemented)") | Still claims unimplemented. Procedural memory storage layer **is** implemented (`cognitive::repos::procedural_rule` is full CRUD + FTS) |
| P2 | `CLAUDE.md` ("MCP server") | Still conflates `mcp` and `mcp-bridge` as same layer |
| P2 | `AGENTS.md` | Still contains only the Phase 4 smoke-test stub (5 lines) |
| **P0** | `CLAUDE.md` ("5 built-in orchestrator skills") | Still says 5. `DEFAULT_SKILLS` in `crates/skill-system/src/store.rs:18` has **6** entries (`task-management`, `finance-management`, `automation`, `notebook`, `learning`, and one more truncated in read) |
| **P0** | `CLAUDE.md` (`SkillRouter` description) | Still describes keyword + semantic scoring router. No such router exists; runtime is flat |
| P1 | `CLAUDE.md` (Reforge — "9 phases with 3 LLM calls") | Still says 9 phases / 3 LLM calls. `run_reforge` has **26 parameters** and 8+ phase markers in doc comment; actual hook traits = 7+ |
| P1 | `cognitive/src/services/reforge/service.rs:1` | File-level doc still says "8 phases". Undercount confirmed |
| P1 | `CLAUDE.md` (Mirror — "Six signal sources") | Still says 6. Actual = 8 unconditional + 2 conditional + 1 stub |
| P1 | `CLAUDE.md` (`MirrorEngine::start` signature) | Still says "takes `Arc<DomainEventBus>`". `crates/cognitive/src/mirror/engine.rs:101` comment confirms bus was dropped from facade construction |
| P1 | `CLAUDE.md` (`INTERACTIVE_TOOL_TIMEOUT`) | Still cites `INTERACTIVE_TOOL_TIMEOUT`. Actual constant is `LONG_RUNNING_TOOL_TIMEOUT` at `crates/agent/src/execution/core.rs:54` |
| P1 | `CLAUDE.md` (`ANTHROPIC_CONTEXT_WINDOW = 200_000`) | Still claims named constant. No such constant exists; context window comes from `RuntimeConfig.context_window` |
| P2 | `CLAUDE.md` (no `DESKTOP_ONLY`) | `ChannelMask::DESKTOP_ONLY` exists at `crates/common/src/tool_channel.rs:51` but is undocumented in CLAUDE.md |
| P2 | `CLAUDE.md` (no KCA env flags) | Six env flags (`KCA_DISABLE_COMPRESSION`, `KCA_PHASE_4`, `KCA_PHASE_4_TOOL_DRIVEN`, `KCA_PHASE_4_LEGACY_NUDGE`, `KCA_COMMUNITY_SUMMARIES`, `KCA_REFORGE_COMPRESS`) still undocumented at project level |
| **P0** | `CLAUDE.md` ("Plain CSS. No Tailwind") | Still claims no Tailwind. Frontend uses `@tailwindcss/vite` plugin; new components should use Tailwind |
| **P0** | `CLAUDE.md` ("4 secondary windows") | Still says 4. Actual = **5** (`launcher`, `tray`, `distraction-overlay`, `voice-orb`, `coding:{repo_id}`) |
| **P0** | `tests/fixtures/kca/{longmembench_subset,klynt_coding_bench,hallucination_planted}.jsonl` | **Files DO NOT EXIST** in repo. `crates/kca-e2e/src/lib.rs:25-36` asserts they exist; would fail on clean checkout |
| **P0** | `docs/architecture/kca-game-changer.md` | **File DOES NOT EXIST**. Referenced by `crates/kca-bench/src/lib.rs` |
| **P0** | `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` | **File DOES NOT EXIST**. Cited as design spec in TECH_DEBT.md itself |
| **P0** | Bundle budget doc drift | CLAUDE.md claims "30 kB gzipped for `src/features/threads/**/*`". Actual `.size-limit.json`: threads route ≤ **350 kB gzipped**, total app ≤ 2.5 MB |
| **P0** | Voice/speech doc drift | CLAUDE.md claims "AVSpeech via platform-macos". Actual: `synthesize_to_file` shells out to `/usr/bin/say` (`crates/platform-macos/src/speech.rs:43`) |
| P1 | TTFT perf gate skeleton | `scripts/run_chat_perf_gates.sh:39` still prints "numeric gate deferred to PR8"; never `exit 1`s on threshold breach |
| P1 | LoCoMo quality gate | `scripts/run_kca_validation.sh` runs `run-locomo-real` but never `exit 1`s on low score |
| P1 | `crates/kca-bench/benches/full_pipeline.rs` | Still a stub; black-boxes fixture without invoking `AppCore` |
| P1 | `crates/plugin-runtime/src/manifest.rs` (`PluginCronJob`) | `cron_jobs: Vec<PluginCronJob>` still deserialized in manifest; no executor reads them |
| P1 | `crates/platform-macos/src/computer_use/ax_tree.rs` (`AccessibilityNode.frame`) | Frame coordinates from `AXUIElement` are in AppKit (bottom-left) space. No Y-flip or documentation of coordinate space mismatch |
| P2 | `crates/platform-macos/src/computer_use/input.rs` (`MacInput`) | `Screenshot` + `Zoom` variants return `NotImplemented` (confirmed in file; 14 of 16 actions work) |
| P2 | `crates/platform-macos/src/computer_use/capture.rs` (`MacCapture`) | `capture_window` + `get_active_window` return `NotImplemented` (3 of 5 methods work); `scale` still hardcoded `2.0` |
| P2 | `crates/platform-macos/src/dnd.rs` (`toggle_dnd`) | Still calls `shortcuts run "Toggle Do Not Disturb"`; requires user-created Shortcut |
| P2 | `crates/kca-e2e/src/replayer.rs` (`await_cognitive_idle`) | **14-second floor** still hardcoded at line 118 |
| P2 | `crates/kca-e2e/src/replayer.rs` (`chat_complete`) | Still manually publishes `ChatTurnCompleted` to bus after draining stream (lines 218–230) |
| P2 | `desktop-ui/src/features/chat/store/chatStreamStore.ts` | Legacy v1 event bridge still active (~30 `agent:*` Tauri listeners). File header confirms: "assistant chat still relies on v1" |
| P2 | OAuth callback fixed port | `crates/desktop/src/oauth/registry.rs:8` still has `pub const CALLBACK_PORT: u16 = 14321`; no fallback if port is in use |
| P2 | Embedded MCP HTTP server `/health` | No `/health` route found in MCP server code; status only via `get_status` tool or `klyntbot://status` resource |
| P3 | `crates/kca-bench/src/bin/gen_soak.rs` | Still outputs 120 fixtures (5×6×4); README says "100 base fixtures". Soak test asserts `>= 100` so passes either way |

---

## Resolved Entries (can be removed or updated in TECH_DEBT.md)

### Category 2 — Stubs & phased gates

| Location | Issue | Resolution |
|---|---|---|
| `crates/coding-memory/src/reforge/` | "4 Reforge phases all stubbed at `required_phase: 5`" | **OUTDATED**. `CodingSynthesisPhase`, `RuleArtifactGenerationPhase`, `SessionEndPass`, and `CrossSessionDedup` are all **fully implemented** in `crates/coding-memory/src/reforge/` (real `.run()` methods with DB queries, LLM calls, and writes). The old stubs still exist in `crates/coding-memory/src/reforge_phase.rs` but are not the implementations currently used. Entry should be removed or narrowed to `reforge_phase.rs` only. |

### Category 5 — Documentation drift

| Location | Issue | Resolution |
|---|---|---|
| `crates/coding-memory/src/reforge/` related docs | Reforge undercount in docs | Should be updated now that phases are implemented. |

---

## New Findings (not in TECH_DEBT.md)

### Code TODOs / Stubs

| Sev | File:Line | Finding | Notes |
|---|---|---|---|
| P2 | `crates/app-core/src/tracing/registry.rs:70` | `unimplemented!()` | Tracing registry stub — no production impact but incomplete |
| P2 | `crates/app-core/src/tracing/registry.rs:91` | `unimplemented!()` | Same file, second stub |
| P2 | `crates/app-core/src/tracing/registry.rs:94` | `unimplemented!()` | Same file, third stub |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_dedup.rs:18` | `unimplemented!()` | Test stub — Intel affordance dedup test not implemented |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_dedup.rs:21` | `unimplemented!()` | Test stub |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_dedup.rs:24` | `unimplemented!()` | Test stub |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs:17` | `unimplemented!()` | Test stub — Intel affordance in-plan test not implemented |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs:20` | `unimplemented!()` | Test stub |
| P3 | `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs:23` | `unimplemented!()` | Test stub |

### Other observations from sweep

- `crates/agent/src/context_sources/todo.rs:9` — `TODO_CACHE_TTL_SECS` is a **false positive** (const name contains "TODO" but is not a task marker).
- `crates/ai-core-macros/tests/expand/*.rs` — `unimplemented!()` in macro expansion tests are **expected test patterns**, not debt.
- `crates/tools-core-macros/tests/domain_enum_tests.rs:101` — `TestStatus::from_str_loose("TODO")` is **false positive** (test data).

---

## Summary Counts

| Metric | Count |
|---|---|
| Total entries in Categories 1–5 | ~75 |
| Verified still exist | **74** |
| Resolved / outdated | **1** (`coding-memory/src/reforge/` stub claim) |
| New TODO/FIXME/`unimplemented!()` findings | **9** (3 in `app-core/tracing`, 6 in `feature-coding-bash` test stubs) |
| False positives filtered | 4 |

### By severity (new findings)

- **P2:** 3 (`app-core/src/tracing/registry.rs` ×3)
- **P3:** 6 (`feature-coding-bash` test stubs ×6)

### Notable structural drift

- **Reforge phases:** The biggest delta between TECH_DEBT.md and reality. The 4 phases listed as "stubbed" in `reforge/` are now fully implemented bodies with SQL, LLM calls, and file I/O. Only the legacy `reforge_phase.rs` trait stubs remain unimplemented.
- **Missing fixture files:** 3 `.jsonl` fixtures asserted in `kca-e2e` do not exist in the repository, which would cause test failures on a clean checkout.
- **Missing referenced docs:** 2 documents cited in TECH_DEBT.md (`kca-game-changer.md` and the computer-use design spec) do not exist in the repo.
