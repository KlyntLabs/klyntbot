# Phase 2 Verification — Agent 7 (Plugins, Desktop, Validation)

> **Docs inspected:**
> - `docs/architecture/subsystems/12-plugins-platform.md`
> - `docs/architecture/subsystems/13-desktop-frontend.md`
> - `docs/architecture/subsystems/14-validation.md`
>
> **Crates inspected:** 14  
> `plugin-runtime`, `plugin-sdk`, `platform-input`, `platform-capture`, `platform-macos`, `desktop`, `desktop-shared`, `desktop-macros`, `desktop-ui` (stub + root), `app-core`, `klyntbot-server`, `klyntbot` (root facade), `kca-bench`, `kca-e2e`

---

## Summary

| Metric | Count |
|---|---|
| Crates inspected | 14 |
| `✅ Accurate` claims | 47 |
| `⚠️ Drift` items | 7 |
| `❌ Wrong` claims | 2 |
| `🔍 Missing` (in code, not docs) | 5 |
| `📋 Tech Debt` (TODO / FIXME / unimplemented!) | 19 |

**Overall assessment:** Docs 12 and 13 are largely accurate with a handful of naming/numbering drifts. Doc 14 is honest about stubs and missing fixtures and matches reality. The most significant factual error is the host-function count in doc 12 (claims 14, actual 12). The most impactful missing item is the `kca-e2e` load-time assertion that fails on clean checkouts because three fixture files are absent.

---

## Per-Crate Findings

### `plugin-runtime`

**✅ Accurate**
- `PluginManager::scan_manifests` and `load_all` exist with claimed signatures.
- `PluginPackage` implements `FeaturePackage` and wraps `WasmPlugin` tools.
- `PluginManifest`, `PluginCronJob`, `PluginMigrationDef`, `PluginPermission` enums/structs match the documented schema exactly.
- Host-function permission checks (`ctx.permissions.contains(&PluginPermission::X)`) are present at the top of every host function.
- `agent_ask_user` returns the exact hard-coded JSON `{"error":"agent callbacks not connected"}`.
- `agent_emit_event` validates kind length (≤ 64), ASCII alphanumeric/underscore, non-empty, and payload ≤ 4 KiB.
- Table-sandbox heuristics (`is_select_only`, `check_table_sandbox`) behave exactly as documented (keyword-splitting, reject multi-statement, reject mutation keywords, prefix check `plugin_{id}_*`).
- `event_schema.rs` contains `PluginEmittedEvent` and `PluginEventValidationError` with the exact validation rules described.
- No hot-reload or unload logic exists; plugins are restart-only.

**⚠️ Drift**
- **File map:** Doc lists `src/sandbox.rs` as the location of `is_select_only` / `check_table_sandbox`. The file does not exist; those functions live in `src/host/mod.rs`.
- **Host-function count:** Doc says "14 functions" in the diagram and "All 14 host functions" in text. The actual count is **12** (`db` ×2, `log` ×4, `http` ×1, `agent` ×3, `tool` ×2). No additional functions are defined in the source.

**🔍 Missing**
- `src/event_schema.rs` is not listed in the doc’s file map (it is present and load-bearing).

**📋 Tech Debt**
- None found in this crate.

---

### `plugin-sdk`

**✅ Accurate**
- `Cargo.toml` has `crate-type = ["cdylib", "rlib"]`.
- `lib.rs` re-exports `extism_pdk::*` at crate root.
- `prelude` module re-exports `serde`, `serde_json`, `config_get`, `http_get`, `log_info`, `log_warn`, `log_error`.
- Dead `db_query` placeholder returns `"[]"` unconditionally, exactly as documented.

**📋 Tech Debt**
- None found.

---

### `platform-input`

**✅ Accurate**
- `PlatformInput` trait has `perform_action`, `get_cursor_position`, `release_all` with correct signatures.
- `ComputerUseAction` enum has exactly 16 variants, tagged `#[serde(tag = "kind", rename_all = "snake_case")]`.
- `Point`, `Rect`, `KeyMods`, `ScrollDir`, `MouseButton` shapes match.
- Coordinates are documented as Quartz top-left origin; code comments confirm this.

**📋 Tech Debt**
- None found.

---

### `platform-capture`

**✅ Accurate**
- `PlatformCapture` trait has all 5 methods (`capture_screen`, `capture_window`, `list_displays`, `get_active_window`, `get_ax_tree`).
- `AxScope` enum has `FullDesktop`, `ActiveApp`, `Window(WindowId)`.
- `AccessibilityNode` has `role`, `label`, `value`, `frame`, `children`, `attrs`.

**📋 Tech Debt**
- None found.

---

### `platform-macos`

**✅ Accurate**
- `MacCapture` implements `capture_screen` via ScreenCaptureKit + `spawn_blocking`.
- `MacCapture::capture_window` and `get_active_window` return `NotImplemented`.
- `MacCapture::get_ax_tree` only supports `AxScope::ActiveApp`; `FullDesktop` and `Window(_)` return errors.
- `MacInput` implements 14 of 16 `ComputerUseAction` variants; `Screenshot` and `Zoom` return `NotImplemented`.
- `MacInput::release_all` posts `LeftMouseUp`, `RightMouseUp`, `OtherMouseUp` at current cursor position.
- `LeftClickDrag` uses 16 interpolated steps with 8 ms sleep (≈128 ms total), exactly as documented.
- `walk_focused_app` bounds depth at 6 and returns `AccessibilityNode`.
- `AccessibilityNode.frame` is in AppKit (bottom-left) coords; code comment explicitly notes Y-flip is deferred to Phase 4.
- `browser.rs` contains 11 `BrowserDef` entries including **Zen Browser**.
- `speech.rs` uses `say` CLI (not `AVSpeechSynthesizer`); `list_voices()` parses `say --voice=?`.
- `dnd.rs` reads `defaults read com.apple.controlcenter` and calls `shortcuts run "Toggle Do Not Disturb"`.
- `lifecycle.rs` has pure `LifecycleStateMachine` + `LifecycleMonitor` polling `CGEventSourceSecondsSinceLastEventType`.
- `apps.rs` has `AppIconCache` using PlistBuddy + `sips` (avoids NSWorkspace).
- `window.rs` has `get_frontmost_window`, `get_frontmost_app_name`, `get_screen_frame`, `set_window_frame`.

**⚠️ Drift**
- **File name:** Doc lists `computer_use/ax_walker.rs`. Actual file is `computer_use/ax_tree.rs` (function `walk_focused_app` lives inside it).

**📋 Tech Debt**
- `crates/platform-macos/src/computer_use/capture.rs:97` — `// TODO: replace with NSScreen.backingScaleFactor in Phase 2`
- `crates/platform-macos/src/lifecycle.rs:176` — `/// - NSWorkspace observers are stubbed for now (TODO: wire objc2 blocks)`

---

### `desktop`

**✅ Accurate**
- `main.rs` contains the full 17-step startup sequence; every step is present in the claimed order.
- `--hook` short-circuit lives before `configure_mimalloc()` and `Cli::parse()`.
- `pre_main_hardening` precedes mimalloc (load-bearing order confirmed).
- `configure_mimalloc` sets `PURGE_DELAY=0`, `ARENA_PURGE_MULT=1`, `ABANDONED_PAGE_PURGE=1`, disables large OS pages + eager commit.
- Tokio runtime is capped at 4 workers + 2 MB stacks and leaked (`Box::leak`).
- Tauri plugins registered: global-shortcut, notification, updater, dialog, process.
- `setup` closure calls `specta.mount_events(app)`, `app_core::init(handle)`, spawns `claude_code_integration::run_first_launch_check`, optionally starts `dev_server`, optionally starts embedded MCP HTTP Axum server.
- Managed state: `core`, `approval_channel`, `Arc::new(FocusTimer::new())`.
- `shortcuts::register_shortcuts` parses config and registers launcher + tray shortcuts.
- Voice hotkey handler is context-aware (focus session → quick capture; launcher visible → emit event; otherwise toggle voice-orb).
- macOS menu: `Cmd+Q` bound to hide dashboard (`CloseRequested` → hide + `ActivationPolicy::Accessory`). `Cmd+W` intentionally unbound.
- Tray icon left-click toggles voice pause/resume when `VOICE_ACTIVE` is true, otherwise toggles tray window.
- 10-second `mi_collect(true)` timer spawned until shutdown token fires.
- `tray_countdown::spawn` subscribes to `DomainEventBus` with adaptive tick rates (1 s / 2 s / 60 s / 1 h).
- OAuth is lazy: `mcp_oauth_start` command starts local Axum server on fixed `CALLBACK_PORT = 14321`.
- `.invoke_handler(crate::specta_builder::klynt_invoke_handler())` replaces `tauri::generate_handler!`.
- `lazy_window.rs` defines 5 secondary windows: `launcher`, `tray`, `distraction-overlay`, `voice-orb`, and `coding:{repo_id}`.
- `hud_effects()` uses `Effect::HudWindow` + `EffectState::Active` + `radius(16.0)`.
- `coding:{repo_id}` window is full decorations, 1200×800, min 700×500.

**⚠️ Drift**
- **Step 9 wording:** Doc says "Secondary windows (lazy) — Registered but NOT created at startup." In code the windows are *not* pre-registered in the Tauri builder; they are created on first demand via `get_or_create_window`. The behavior is the same (not created at startup), but "registered" implies a builder registration that does not exist.

**📋 Tech Debt**
- None found in `crates/desktop/src/`.

---

### `desktop-shared`

**✅ Accurate**
- `ThreadEvent` v2 is a 26-variant tagged union (`#[serde(tag = "event", rename_all = "snake_case")]`).
- `Terminal` variant wraps `TerminalKind { Done, Error, Cancelled }`.
- Every variant carries `generation: u32` + `session_key: String`.
- `CommandResult<T>` and `ApiError { code, message }` exist with `specta::Type` derive.
- `ApiError` has exhaustive `From<KlyntbotError>`.
- `permissions.rs` references macOS permission APIs (Screen Recording, Accessibility) — used for permission checking, not Computer Use invocation.

**📋 Tech Debt**
- None found.

---

### `desktop-macros`

**✅ Accurate**
- Four macros exist: `#[klynt_command]`, `#[klynt_raw_command]`, `klynt_collect_commands!`, `klynt_collect_events!`.
- Source files match doc description: `klynt_command.rs`, `klynt_raw_command.rs`, `collect_commands.rs`, `collect_events.rs`.
- `KLYNT_COMMANDS` distributed slice and `KLYNT_SPECTA_COMMAND_NAMES` / `SPECTA_COMMAND_NAMES` alias are present in `specta_builder.rs`.

**📋 Tech Debt**
- None found.

---

### `desktop-ui`

**✅ Accurate**
- `crates/desktop-ui/` is a stub containing only `src/bindings.ts` (auto-generated tauri-specta output).
- Real frontend lives at repo root `/desktop-ui/`.
- Stack is React 19 + Vite + Tailwind (`@tailwindcss/vite` plugin is present in `vite.config.ts`).
- `vite.config.ts` defines path aliases `@app`, `@settings`, `@threads`, `@services`, `@utils`, plus `@/` catch-all.
- Manual chunks include `vendor-react`, `vendor-markdown`, `vendor-tauri`, `vendor-ui`, `vendor-xterm`, `vendor-mermaid`, `vendor-katex`.
- `useChatStore.ts` exists and imports `CoalescerRegistry`.
- `VirtualizedMessageList.tsx` exists.
- `useThreadWatchdog.ts` exists.
- Approval surfaces exist at the documented paths (`ApprovalCard.tsx`, `useThreadApprovalEvents.ts`, `ApprovalToasts.tsx`).
- `.size-limit.json` defines 350 kB gzipped for threads route and 2.5 MB total.

**⚠️ Drift**
- **Feature directory count:** Doc claims 33 feature directories. Actual count is **32** (`about`, `app`, `apps`, `chat`, `coding`, `collaboration`, `composer`, `dashboard`, `debug`, `design-system`, `dictation`, `distraction`, `files`, `git`, `home`, `launcher`, `layout`, `messages`, `mobile`, `models`, `notifications`, `plan`, `plugins`, `prompts`, `settings`, `shared`, `skills`, `terminal`, `threads`, `tray`, `update`, `workspaces`).

**📋 Tech Debt**
- `desktop-ui/src/features/settings/components/SettingsView.test.tsx:1680` — `// TODO(phase-2.3): re-enable once underDevelopment-stage feature rendering`
- `desktop-ui/src/features/settings/components/SettingsView.test.tsx:1735` — `// TODO(phase-2.3): re-enable alongside the steer-mode test above.`
- `desktop-ui/src/features/coding/state/jobsStore.ts:3` — `// TODO: switch to @/bindings once tauri-specta bindings are regenerated`
- `desktop-ui/src/features/git/components/GitDiffPanel.test.tsx:137` — `// TODO(phase-2.2): re-enable once platform-path resolution under jsdom is`
- `desktop-ui/src/features/git/components/GitDiffPanel.test.tsx:200` — `// TODO(phase-2.2): re-enable alongside the file-manager test above.`
- `desktop-ui/src/services/__mocks__/tauri-shims.ts:1` — `// TODO(klynt-integration): aggregate mock surface for all Tauri sub-modules`
- `desktop-ui/src/services/__mocks__/tauri-shims.ts:10` — `// TODO(klynt-integration): expose real app version once Tauri is wired.`
- `desktop-ui/src/services/__mocks__/tauri-shims.ts:47` — `// full API surface. TODO(klynt-integration): swap for the real webview handle.`
- `desktop-ui/src/services/__mocks__/tauri-shims.ts:273` — `// TODO(klynt-integration): wire to real updater. Returning null = "no update".`

---

### `app-core`

**✅ Accurate**
- `AppCore` struct is transport-agnostic (no `tauri::*` or `axum::*` types in the struct itself).
- `AppCore::init_with_sender` exists and is called by `desktop` setup closure.
- `runtime/mod.rs` defines `ThreadRuntime` trait with `start_turn`, `cancel_turn`, `is_active`, `active_turns`.
- `StreamGuard` implements value-identity removal on `Drop` using `guard_id` from `STREAM_GUARD_COUNTER`.
- Two concrete impls exist: `AssistantThreadRuntime` (`runtime/assistant.rs`) and `CodingThreadRuntime` (`runtime/coding.rs`).
- `handlers/` directory contains ~40 domain modules, matching the doc’s list (agents, annotations, areas, atoms, autotuner, capture, chat, coaching, coding_jobs, coding_plan, coding_todo, cognitive, columns, cron, distraction, entities, entity_links, fabric, finance, git, groups, integrations, key_results, knowledge_health, launcher, morning_briefing, notes, objectives, productivity, project_conversations, project_memories, project_sources, projects, reforge, retention_history, review_stats, settings, status, subagent, tasks, timeline, view, voice, voice_conversation, voice_conversation_commands, voice_echo, work_context, workflows, workspace).
- `ActiveStreams` (`DashMap`) and `pending_interactions` are present.

**⚠️ Drift**
- **Init phase naming:** Doc lists 14 phases with names like "Config load", "init::storage", "init::channels", etc. The actual `init_with_sender` starts with "Phase 1: Storage" (config load happens inside `storage::init_storage`), then cron, temporal scheduler, agent, channel manager, DND, productivity + launcher concurrently, coaching, cognitive, mirror, AI pipeline, BrainVoice, etc. The doc’s simplified 14-phase list is directionally correct but does not map 1-to-1 to source comments.
- **AppCore field types:** Doc shows `channel_manager: ChannelManager` and `cron_bridge: Option<CronBridge>`. Actual types are `channel_manager: Arc<Mutex<ChannelManager>>` and `cron_bridge: Arc<CronBridge>` (not `Option`). These are minor signature drifts in the doc’s selected-fields excerpt.

**📋 Tech Debt**
- `crates/app-core/src/tracing/registry.rs:70` — `unimplemented!()`
- `crates/app-core/src/tracing/registry.rs:91` — `unimplemented!()`
- `crates/app-core/src/tracing/registry.rs:94` — `unimplemented!()`
- `crates/app-core/src/init/mod.rs:1034` — `// TODO(phase-3.5): wire real user timezone when config has it.`
- `crates/app-core/src/init/coding_subscribers.rs:51` — `// TODO: wire actual success/failure once ToolCallExecuted carries it.`
- `crates/app-core/src/coding/recall_stats_handler.rs:33` — `// TODO: wire up recall_invocations repo once coding-memory telemetry is`
- `crates/app-core/src/handlers/cron.rs:94` — `// TODO(4.4c): wire to TemporalScheduler::is_running() once CronService is retired.`
- `crates/app-core/src/handlers/cron.rs:221` — `// TODO(4.4c): update semantics will simplify once CronService is removed —`
- `crates/app-core/src/handlers/coding_plan.rs:203` — `// 8. TODO: Spawn untitled-rename watcher if title was empty.`
- `crates/app-core/src/handlers/coding_todo.rs:268` — `// TODO: cache empty CompiledRules in a static once the type supports it.`

---

### `klyntbot-server`

**✅ Accurate**
- `KlyntbotServerHandler::new(app, whitelist)` exists.
- Implements `rmcp::handler::server::ServerHandler`.
- `list_tools` returns `[get_status]` + optional `[agent]` + `bridge.list_tools()`.
- `call_tool` dispatches to `handle_get_status`, `agent_bridge.execute`, or `bridge.execute`.
- Resources `klyntbot://status`, `klyntbot://memory/recent`, `klyntbot://tasks/today`, `klyntbot://config/skills` are built in `build_resources()`.
- `serve_stdio` uses `rmcp::transport::io::stdio()` and calls `app.shutdown()` before returning.
- Embedded HTTP mode uses `StreamableHttpService` mounted at `/mcp` with optional bearer-token middleware.
- `emit_entity_update_for_tool` dispatches via `AiFeatureRegistry` primary or `NON_FEATURE_TOOL_ENTITY_KINDS` fallback.

**📋 Tech Debt**
- None found.

---

### `klyntbot` (root facade)

**✅ Accurate**
- `src/lib.rs` does `pub use` for every workspace crate plus convenience type-level re-exports (`AgentEvent`, `AgentLoop`, `MessageBus`, `Config`, `DynTool`, etc.).
- `pub const VERSION: &str = env!("CARGO_PKG_VERSION");` exists.
- `src/main.rs` is a minimal CLI-removed stub pointing to `cargo tauri dev` / `build`.

**📋 Tech Debt**
- None found.

---

### `kca-bench`

**✅ Accurate**
- Three criterion benches declared in `Cargo.toml` with `harness = false`: `full_pipeline`, `ppr_only`, `extraction_path`.
- Three standalone binaries declared: `run-locomo-real`, `analyze-trace`, `gen-soak`.
- `full_pipeline` bench is a stub that black-boxes a `ConversationFixture` value without invoking `AppCore`.
- `ppr_only` bench measures `personalized_pagerank` on 50-node and 2000-node chain graphs.
- `extraction_path` bench upserts 100 `SemanticFactRepo` rows against in-memory SQLite.
- `run-locomo-real` loads `tests/fixtures/kca/locomo10_real.json`, grades via OpenAI gpt-4.1 SimpleQA-style A/B/C.
- `analyze-trace` supports single-file and two-file diff modes.
- `gen-soak` outputs 120 fixtures (5 personas × 6 topics × 4 actions), not 100 as the README claims.
- `lib.rs` doc comment notes synthetic fixtures were removed 2026-05-01 and references `docs/architecture/kca-game-changer.md`.

**📋 Tech Debt**
- None found in source.

---

### `kca-e2e`

**✅ Accurate**
- `ReplayContext::new()` boots a real `AppCore` with `IntelligenceMode::Deep`, micro-reforge, predictive cache, hierarchical enabled.
- `ReplayContext::replay()` calls `chat_complete` per turn and `await_cognitive_idle()` after each turn.
- `await_cognitive_idle()` polls every 750 ms, requires 4 stable readings, has a 14-second mandatory floor, and a 60-second timeout.
- `FixtureLoader::load_jsonl` and `fixtures_root` exist.
- `ConversationFixture`, `TurnFixture`, `QueryFixture` structs match documented shapes.

**❌ Wrong**
- **Load-time fixture assertions are broken on clean checkout.** `src/lib.rs` unit test `loads_seed_fixtures_without_error` asserts `longmembench_subset.jsonl`, `klynt_coding_bench.jsonl`, and `hallucination_planted.jsonl` exist and are non-empty. **None of these files are present in the repository.** This causes `cargo test -p kca-e2e` to fail immediately on a fresh clone.

**🔍 Missing**
- `tests/fixtures/kca/` directory does not exist in the repo root (or inside the crate). The only fixtures present are `locomo10_real.json`, `regression_panel.jsonl`, and `soak_10k.jsonl` (located at repo root `tests/fixtures/kca/`). The doc correctly notes the three missing files, but the code assertion was never updated.

**📋 Tech Debt**
- None found in source (the broken test is a structural debt item, not a TODO comment).

---

## Cross-Reference Check

| Source Doc | Target | Status |
|---|---|---|
| `12-plugins-platform.md` | `01-foundations.md` | ✅ Exists |
| `12-plugins-platform.md` | `02-storage.md` | ✅ Exists |
| `12-plugins-platform.md` | `07-tools-framework.md` | ✅ Exists |
| `12-plugins-platform.md` | `08-assistant-features.md` | ✅ Exists |
| `12-plugins-platform.md` | `10-sandboxing-security.md` | ✅ Exists |
| `12-plugins-platform.md` | `13-desktop-frontend.md` | ✅ Exists |
| `13-desktop-frontend.md` | `01-foundations.md` | ✅ Exists |
| `13-desktop-frontend.md` | `02-storage.md` | ✅ Exists |
| `13-desktop-frontend.md` | `04-agent-runtime.md` | ✅ Exists |
| `13-desktop-frontend.md` | `09-coding-mode.md` | ✅ Exists |
| `13-desktop-frontend.md` | `10-sandboxing-security.md` | ✅ Exists |
| `13-desktop-frontend.md` | `11-channels-mcp.md` | ✅ Exists |
| `13-desktop-frontend.md` | `crates/app-core.md` | ⚠️ Planned — does not exist yet |
| `13-desktop-frontend.md` | `crates/desktop.md` | ⚠️ Planned — does not exist yet |
| `14-validation.md` | `00-overview.md` | ✅ Exists (`docs/architecture/00-overview.md`) |
| `14-validation.md` | `02-storage.md` | ✅ Exists |
| `14-validation.md` | `05-cognitive-memory.md` | ✅ Exists |
| `14-validation.md` | `13-desktop-frontend.md` | ✅ Exists |
| All three docs | `TECH_DEBT.md` | ✅ Exists |

**Notes:**
- The two "planned" deep-dive crate docs (`crates/app-core.md`, `crates/desktop.md`) are explicitly called out as *planned* in doc 13, so their absence is documented and not an error.
- Doc 12 references `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` as "vapor" and correctly states it does not exist. This is an honest doc claim, not a broken link.
