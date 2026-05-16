# Phase 4 Tech Debt Verification — Categories 6–9

> **Agent:** techdebt-agent-2  
> **Scope:** `docs/architecture/TECH_DEBT.md` categories 6 (Hardcoded values), 7 (Architectural anomalies), 8 (Naming & conventions), 9 (Untracked surfaces)  
> **Date:** 2026-05-16

---

## Verified Entries

These entries were inspected against current source (`HEAD`) and **remain unresolved**.

### Category 6 — Hardcoded values awaiting config

| # | Location | State | Notes |
|---|----------|-------|-------|
| 6.1 | `crates/app-core/src/init/temporal_scheduler.rs:21` | **Verified** | `const DEFAULT_MATERIALIZE_AHEAD: u32 = 3;` still hardcoded. Comment references spec §3.2 config field. |
| 6.2 | `crates/platform-macos/src/computer_use/capture.rs:97` | **Verified** | `scale: 2.0` still hardcoded with `TODO: replace with NSScreen.backingScaleFactor in Phase 2`. |
| 6.3 | `crates/scheduling/Cargo.toml:13-16` | **Verified** | Both `chrono`/`chrono-tz` and `jiff` present. Comment in file explicitly states `chrono` is kept for `cron` crate API boundary. |

### Category 7 — Architectural anomalies

| # | Location | State | Notes |
|---|----------|-------|-------|
| 7.1 | `crates/storage/Cargo.toml:7` | **Verified** | `ai-core.workspace = true` dependency still present. Upward dependency against stated L2-layer model. |
| 7.2 | Four tool-wiring paths | **Verified** | All four paths still coexist: (a) `FeaturePackage::tools()` — `tasks_feature.tools()` (`builder.rs:1417`), `package.tools()` (`builder.rs:1666`); (b) `agent_loop/builder.rs` — `LearningTool` (`builder.rs:1735`), `AlarmTool` (`builder.rs:690`), `TaskTool` (`builder.rs:1418`); (c) `app-core/init/mod.rs` — `LauncherTool` (`mod.rs:1139`), coding bash tools (`mod.rs:2017-2020`); (d) `subagent.rs` per-invocation — `AgentTaskTool` (`subagent.rs:800`). |
| 7.3 | `crates/feature-alarms` | **Verified** | No `FeaturePackage` impl found in `crates/feature-alarms/src/`. |
| 7.4 | `crates/feature-insights` | **Verified** | No `FeaturePackage` impl and no LLM tools in `crates/feature-insights/src/`. |
| 7.5 | `crates/feature-learning/src/feature.rs:43` | **Verified** | `FeaturePackage::tools()` returns `Vec::new()`. `LearningTool` lives at `crates/tools/src/domain/learning_tool.rs`. |
| 7.6 | `crates/feature-coding-bash/Cargo.toml` | **Verified** | Uses `path = "../<crate>"` for 9 internal deps (`approval`, `bus`, `common`, `config`, `klynt-pty`, `klynt-sandbox`, `storage`, `tools-core`, `tools-core-macros`). Root cause: `feature-coding-bash` is **missing from `[workspace.dependencies]`** (see New Findings). |
| 7.7 | `crates/desktop-ui` vs `/desktop-ui` | **Changed** | `crates/desktop-ui/Cargo.toml` no longer exists and the crate is **not in root `Cargo.toml` workspace members**. The directory `crates/desktop-ui/src/bindings.ts` remains as an artifact. The original anomaly (Specta stub crate colliding with frontend dir) is **resolved by removal**, but the orphaned directory is a cleanup gap. |
| 7.8 | `cognitive/src/services/reforge/service.rs::run_reforge` | **Verified** | 26-parameter signature confirmed at line 30. Parameters include 7 `Option<&dyn Trait>` hook args plus 19 other args. `#[allow(clippy::too_many_arguments)]` is present. |
| 7.9 | Two `AutotunerBridge` traits | **Verified** | `cognitive/src/services/reforge/mod.rs:37` (Phase 6 orchestration) and `cognitive/src/mirror/types.rs:35` (MirrorFacade champion promotion). Both active. |
| 7.10 | Two `retrievability` functions | **Verified** | `cognitive/src/services/fsrs5.rs:35` uses power-law `1/(1+t/9S)`; `cognitive/src/services/decay.rs:8` uses exponential `exp(ln(0.9)*t/s)`. Different formulas, same name. |
| 7.11 | `bus::domain_events::ConcurrencyClass` | **Verified** | Enum `{Safe, Sequential, Exclusive}` exists and is used heavily in `feature-coding-todo`, but `Tool::is_concurrency_safe(args) -> bool` (`tools-core/src/lib.rs:105`) returns a boolean — the "Exclusive" tier is unused by the `Tool` trait. |
| 7.12 | `LearningTool` location | **Verified** | Still at `crates/tools/src/domain/learning_tool.rs`, not inside `crates/feature-learning/`. |
| 7.13 | `feature-coding-bash/Cargo.toml` path deps | **Duplicate of 7.6** | Same root cause. |

### Category 8 — Naming & convention inconsistencies

| # | Location | State | Notes |
|---|----------|-------|-------|
| 8.1 | `ToolOutput::Structured` | **Verified** | Defined in `tools-core/src/lib.rs:165-173` with `__STRUCTURED__` parsing convention. 25 production `impl Tool for` found; **zero** emit `ToolOutput::Structured`. |
| 8.2 | `TasksFeature::new()` silent zero-tool registration | **Verified** | `TasksFeature::new()` without `.with_task_tool(...)` found at 5 call sites: `feature-tasks/src/lib.rs:151,157,163`, `app-core/src/init/storage.rs:109`, `feature-tasks/tests/feature_package_test.rs:5`. Only `agent/src/agent_loop/builder.rs:1416` uses the correct `.with_task_tool()` chaining. |
| 8.3 | `kca-bench` synthetic fixtures removal | **Resolved but undocumented** | `crates/kca-bench/src/lib.rs:3-5` confirms synthetic fixtures removed 2026-05-01. **Still not documented in CLAUDE.md** or project-level docs. |
| 8.4 | Cargo `version = "0.1.1"` | **Verified** | `workspace.package.version = "0.1.1"` in root `Cargo.toml:72`. CHANGELOG.md says first public release is `0.1.0`. |
| 8.5 | Two `AlarmFired` `kind` strings | **Verified** | `"cron_job"` at `scheduling/src/temporal/cron_bridge.rs:87` (TemporalScheduler → CronExecutor internal dispatch) vs `"cron"` at `app-core/src/init/cron.rs:27` (user-facing notifications). |
| 8.6 | `CronHandler` is sync | **Verified** | `scheduling/src/temporal/cron_executor.rs:55` defines `pub type CronHandler = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>`. Still `Fn`, not `AsyncFn`. Handlers that need async use `block_in_place + rt.block_on(...)`. |
| 8.7 | Plural / singular tool-name footgun | **Verified** | Registry keys: `tasks`, `notes`, `finance` (plural) vs `memory`, `alarm`, `learning` (singular). MCP bridge test at `klyntbot-server/src/bridge/registry.rs:220` uses `"tasks"`. No standardization. |
| 8.8 | `MidLoopCompressor` vs `TieredHistoryCompressor` | **Verified** | `agent/src/execution/mid_loop_compressor.rs:26` (in-loop stale-tool compression) and `context_engine/src/history_compressor/tiered.rs:24` (turn-history compression before context assembly). Both can fire on the same turn. |
| 8.9 | `SkillSource` priority numbering | **Verified** | `klynt-skill-loader/src/index.rs:20-25`: `User=0` … `SkillsMarketplace=5`. Higher number wins on collision. Reverse of common precedence convention. |
| 8.10 | `Hook.matcher` / Glob normalization | **Verified** | `approval/src/coding_policy.rs:177-182` strips `_`, `-`, and lowercases. `BashTool`, `bash-tool`, `bash_tool`, `bash` all match `bash(*)` glob. |
| 8.11 | `CodingApprovalPolicy::YoloMode` expiry | **Verified** | `approval/src/coding_policy.rs:82-87`: when `until` passes, falls through to `matches!(DefaultPolicy::Ask, DefaultPolicy::Allow)` which is `false`. Effectively becomes "ask everything." |
| 8.12 | `BlockingFallbackChannel.capabilities()` mismatch | **Verified** | `approval/src/channel.rs:43-47` advertises `supports_classes: {Destructive, Admin}`. `request()` at `channel.rs:38-42` unconditionally returns `Decline`. |
| 8.13 | kimi-cli + opencode hook USAGE | **Verified** | `coding-ingest/src/hook_cli.rs:20-23` lists both as supported sources. Lines 81 and 85 short-circuit with `"poll-only (Phase 7)"`. |

### Category 9 — Untracked surfaces

| # | What | State | Notes |
|---|------|-------|-------|
| 9.1 | Computer Use platform layer | **Verified** | `platform-input`, `platform-capture`, `platform-macos/src/computer_use/` all implemented. Still not wired into agent tool / Tauri command / MCP tool. |
| 9.2 | `kca-bench` binaries | **Verified** | `src/bin/{run-locomo-real,analyze-trace,gen-soak}.rs` exist. Not in CLAUDE.md. |
| 9.3 | `kca-e2e` fixture data | **Verified** | `tests/fixtures/kca/{longmembench_subset,klynt_coding_bench,hallucination_planted}.jsonl` still **do not exist** in repo. `cargo test -p kca-e2e` would fail on clean checkout. |
| 9.4 | `desktop --hook` short-circuit | **Verified** | `crates/desktop/src/main.rs:101-108`. `argv[1] == "--hook"` fires before Tauri/mimalloc init. Sub-10ms startup. Undocumented. |
| 9.5 | Skill discovery roots | **Verified** | 4 roots in `klynt-skill-loader::discovery`: User / ReforgePrivate / Project / ReforgeTeam. Priority order not documented at project level. |
| 9.6 | OS_NATIVE + Tray channels | **Verified** | `crates/notifications/src/channel/` has `OsNativeChannel` and `TrayChannel`. Separately listed from chat channels. |
| 9.7 | Voice 5 engines | **Verified** | `Qwen3AsrEngine` + `CloudAsrEngine` (STT); `Qwen3TtsEngine` + `AvSpeechTtsEngine` + `CloudTtsEngine` (TTS). `Qwen3TtsEngine` requires `--features qwen3`. |
| 9.8 | `klynt-protocol` + `klynt-hooks` provenance | **Verified** | Crate `Cargo.toml` descriptions say "Adapted from codex-rs/protocol/" and "Adapted from codex-rs/hooks/". Not in subsystem docs. |
| 9.9 | 5 plugin host namespaces | **Verified** | `crates/plugin-runtime/src/host/mod.rs`: `db`, `log`, `http`, `agent`, `tool`. Three permissions: `Network`, `Storage`, `Agent`. Not enumerated outside source. |
| 9.10 | KCA env-only feature flags | **Verified** | 6 flags confirmed by grep: `KCA_DISABLE_COMPRESSION`, `KCA_PHASE_4`, `KCA_PHASE_4_TOOL_DRIVEN`, `KCA_PHASE_4_LEGACY_NUDGE`, `KCA_COMMUNITY_SUMMARIES`, `KCA_REFORGE_COMPRESS`. Only discoverable by grep. |
| 9.11 | KCA Track 7 — predictive cache warming | **Verified** | `agent::adapters::cognitive_handlers::LlmQueryPredictorHandler` + `cognitive::services::predictive_cache`. Wired in `agent/src/agent_loop/builder.rs:746-795`. Not documented at project level. |
| 9.12 | Focus-session message deferral | **Verified** | `agent/src/agent_loop/mod.rs:321-401`. `FocusSessionStarted` → buffer inbound messages + single auto-reply. Drains on `FocusSessionEnded`. |
| 9.13 | `strategy_records` raw SQL | **Verified** | `cognitive/src/services/reforge/feedback.rs:173` reads via raw SQL. Inconsistent with typed repos elsewhere. **Note:** `storage/src/repos/strategy.rs` now exists as a typed repo, but the Reforge feedback path still bypasses it. |
| 9.14 | `coding_approval_history` retention | **Verified** | Table grows unboundedly. `storage/src/repos/coding_approval_history.rs` has `delete_for_tool_and_repo` / `delete_for_tool` methods, but no scheduled retention job or policy found. |
| 9.15 | `feature-productivity` 20 tables undocumented | **Verified** | Migration `001_productivity_tables.sql` defines 21 tables. Only `activity_events` and `daily_summaries` mentioned in CLAUDE.md. |
| 9.16 | Cross-feature shared tables | **Verified** | `practice_sessions` defined in `feature-notes/migrations/002_create_learning.sql` (note: owned by notes crate) but used by `feature-language-learning`. `activity_events` (`feature-productivity`) read by `feature-launcher::AttentionAggregator`. No ownership registry. |
| 9.17 | `mcp-bridge` `MAX_FRAME_BYTES` | **Verified** | `crates/mcp-bridge/src/protocol.rs:5`: `1 << 20` (1 MB). Undocumented in user-facing material. |
| 9.18 | `klynt-sandbox-helper` exit code 2 | **Verified** | `crates/klynt-sandbox-helper/src/main.rs:60-63`: non-Linux prints error + `process::exit(2)`. |

---

## Resolved Entries

| # | Original Entry | Resolution | Evidence |
|---|---------------|------------|----------|
| R.1 | `crates/desktop-ui/` Specta stub crate colliding with `/desktop-ui/` frontend | **Crate removed** from workspace. `crates/desktop-ui/Cargo.toml` no longer exists; crate is absent from root `Cargo.toml` workspace members. Directory `crates/desktop-ui/src/bindings.ts` remains as a leftover artifact. | `find crates/desktop-ui -name Cargo.toml` → empty. Root `Cargo.toml` members list inspected. |
| R.2 | `app-core/src/init/mod.rs:1034` — user timezone uses default | **Code resolved, comment stale** | Line 1034 still has `TODO(phase-3.5): wire real user timezone when config has it`, but line 1035 immediately uses `config.timezone.as_str()`. The TODO is a stale comment. |

---

## New Findings

### 1. Circular dependency: `coding-ingest` ↔ `coding-memory` **[P1 — Architectural anomaly]**

- `crates/coding-ingest/Cargo.toml:30` → `coding-memory = { path = "../coding-memory" }`
- `crates/coding-memory/Cargo.toml:16` → `coding-ingest = { path = "../coding-ingest" }`

Both crates depend on each other via path deps. This violates acyclic crate graph invariants and complicates build ordering, incremental compilation, and reasoning about layer boundaries.

### 2. `feature-coding-bash` missing from `[workspace.dependencies]` **[P2 — Architectural anomaly / Config drift]**

`feature-coding-bash` is a workspace member (in root `Cargo.toml` members list) but **absent from `[workspace.dependencies]`**. This forces every consumer (`app-core`, `agent`, `desktop`) to use `path = "../feature-coding-bash"` instead of `feature-coding-bash.workspace = true`. The same pattern forces `feature-coding-bash` itself to use path deps for all its internal dependencies (already noted in 7.6).

### 3. `mcp-bridge` upward dependency on `app-core` and `desktop-shared` **[P1 — Architectural anomaly]**

`crates/mcp-bridge/Cargo.toml:16-17`:
```toml
app-core = { path = "../app-core" }
desktop-shared = { path = "../desktop-shared" }
```

A Unix-socket IPC transport/protocol crate (`mcp-bridge`) depends on the application orchestrator (`app-core`) and desktop types (`desktop-shared`). This is an upward dependency — transport layers should not depend on application layers. The `emitter` module comment says "Used only by MCP side," suggesting the dependency could be inverted or the emitter moved.

### 4. Multiple crates use `path = "../<crate>"` for dependencies that ARE in `[workspace.dependencies]` **[P2 — Convention inconsistency]**

| Crate | Path-only deps (all present in `[workspace.dependencies]`) |
|-------|-----------------------------------------------------------|
| `approval` | `common`, `config`, `tools-core`, `storage`, `bus`, `activity-log` |
| `context_engine` | `common`, `config`, `providers` |
| `feature-coding-todo` | `approval`, `bus`, `common`, `config`, `storage`, `tools-core`, `tools-core-macros` |
| `feature-focus` | `common`, `storage`, `scheduling`, `tools-core` |
| `feature-learning` | `ai-core`, `ai-core-macros` |
| `feature-notes` | `ai-core`, `ai-core-macros`, `bus` |
| `klynt-skill-loader` | `common`, `config`, `skill-system` |

These should use `<crate>.workspace = true` for consistency with the rest of the workspace.

### 5. `approval` crate version out of sync with workspace **[P3 — Convention inconsistency]**

`crates/approval/Cargo.toml:3` hardcodes `version = "0.1.0"` while the workspace declares `version = "0.1.1"`. Most other crates use `version.workspace = true`.

### 6. `mcp-bridge` edition out of sync **[P3 — Convention inconsistency]**

`crates/mcp-bridge/Cargo.toml:4` uses `edition = "2024"` while the workspace declares `edition = "2021"`. All other crates use `edition.workspace = true`.

### 7. Orphaned `crates/desktop-ui/src/bindings.ts` **[P3 — Cleanup gap]**

The `desktop-ui` stub crate was removed from the workspace, but `crates/desktop-ui/src/bindings.ts` remains. This file is already generated into `/desktop-ui/src/bindings.ts` by the Specta export in `crates/desktop/src/main.rs:269`. The duplicate in `crates/desktop-ui/` is dead weight.

---

## Summary Counts

| Category | Total Entries | Verified (still present) | Resolved / Changed | New Findings |
|----------|---------------|--------------------------|--------------------|--------------|
| 6. Hardcoded values | 4 | 3 | 1 (partially resolved — stale TODO) | — |
| 7. Architectural anomalies | 13 | 12 | 1 (desktop-ui stub removed) | 4 (circular dep, missing workspace dep, upward mcp-bridge, path-vs-workspace pattern) |
| 8. Naming & conventions | 13 | 12 | 1 (kca-bench fixtures removed but undocumented) | 3 (approval version drift, mcp-bridge edition drift, path-vs-workspace) |
| 9. Untracked surfaces | 18 | 18 | 0 | — |
| **Totals** | **48** | **45** | **3** | **7** |

### Severity of new findings

| Finding | Severity |
|---------|----------|
| Circular `coding-ingest` ↔ `coding-memory` | **P1** |
| `mcp-bridge` depends on `app-core` / `desktop-shared` | **P1** |
| `feature-coding-bash` missing from `[workspace.dependencies]` | **P2** |
| 7 crates using path-only deps for workspace-declared crates | **P2** |
| `approval` version not using workspace | **P3** |
| `mcp-bridge` edition not using workspace | **P3** |
| Orphaned `crates/desktop-ui/src/bindings.ts` | **P3** |
