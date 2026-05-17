# Crate: `desktop`

> **Status:** 🟢 Stable
> **Subsystem:** [13 — Desktop App & Frontend](../subsystems/13-desktop-frontend.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The Tauri binary — triple-mode (Tauri app / `mcp serve` / `--hook` short-circuit) — and the macros + tests that gate the IPC surface

---

## TL;DR

The single deployable KlyntBot binary. Three modes selected at startup:
1. **`--hook`** (sub-10ms short-circuit) — doubles as `klyntbot-hook` for Claude Code / git hooks
2. **`mcp serve --stdio`** — runs the MCP stdio server for external AI clients
3. **(default)** — runs the full Tauri desktop app

**Owns the IPC surface** via `desktop-macros` (`#[klynt_command]`, `#[klynt_raw_command]`, `klynt_collect_commands![]`, `klynt_collect_events![]`) and **4 CI guard tests** (`no_raw_tauri_command_outside_macros`, `registration_drift`, `bindings_are_current`, `no_double_registration`).

Five secondary windows (`launcher`, `tray`, `distraction-overlay`, `voice-orb`, `coding:{repo_id}`) all created lazily by `lazy_window::get_or_create_window`. Local Axum HTTP server for OAuth callbacks on a **fixed `CALLBACK_PORT`** (no fallback if port in use). Live tray-countdown with adaptive tick rates (1s/2s/60s/1h). Mimalloc compaction via both 10s timer AND an explicit `set_purge_hook` so lower-layer crates can trigger collection after transient allocations.

---

## Module map

```
crates/desktop/src/
├── main.rs                     ← 17-step startup; triple-mode dispatch; pre_main_hardening
├── lib.rs                      ← LEGACY_COMMAND_NAMES (dead, awaiting deletion)
├── specta_builder.rs           ← build_specta, klynt_invoke_handler, klynt_collect_commands!
├── shortcuts.rs                ← Global shortcut registration + toggle_window
├── lazy_window.rs              ← Secondary window builders + hud_effects + position helpers
├── tray_countdown.rs           ← Adaptive-tick menu-bar countdown
├── focus_timer.rs              ← Coordinated focus session timer
├── focus_session_overlay.rs    ← Distraction overlay logic
├── claude_code_integration.rs  ← run_first_launch_check (idempotent MCP registration)
│
├── commands/                   ← Tauri command shims (every file uses #[klynt_command] or #[klynt_raw_command])
│   ├── chat.rs
│   ├── coding.rs
│   ├── tasks.rs
│   ├── … (40+ files mirroring app-core/handlers/)
│   └── mcp.rs
│
├── oauth/
│   ├── mod.rs                  ← Local Axum HTTP server on CALLBACK_PORT
│   ├── flow.rs                 ← OAuth state machine
│   └── registry.rs             ← OAuthRegistry (token storage)
│
├── dev_server/
│   ├── mod.rs                  ← Optional dev HTTP server (browser-only mode)
│   ├── routes.rs
│   └── sse.rs                  ← Server-sent events (replaces Tauri events for browser dev)
│
├── approval/
│   ├── mod.rs                  ← Tauri-side thin wrapper around DesktopApprovalChannel
│   └── … 
│
└── tests/
    ├── no_raw_tauri_command_outside_macros.rs  ← Forbids bare #[tauri::command]
    ├── registration_drift.rs                   ← BTreeSet diff: linkme vs specta
    ├── bindings_are_current.rs                 ← Byte-compare against bindings.ts
    └── no_double_registration.rs               ← No duplicate names in linkme slice
```

---

## Public API surface

The `desktop` crate is a binary — most of its surface is internal. Public entry points:

```rust
// crates/desktop/src/main.rs
fn main() -> Result<(), Box<dyn Error>>;
```

Most "API" is via the `specta_builder.rs` module:

```rust
// crates/desktop/src/specta_builder.rs
pub fn build_specta() -> tauri_specta::Builder<tauri::Wry>;

/// Invoked by Tauri to dispatch all #[klynt_command] / #[klynt_raw_command] functions.
pub(crate) fn klynt_invoke_handler() -> Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync>;

/// Re-exported aliases for the linkme-collected command names.
pub use self::KLYNT_SPECTA_COMMAND_NAMES as SPECTA_COMMAND_NAMES;
```

And the `lazy_window` window factory:

```rust
// crates/desktop/src/lazy_window.rs
pub fn get_or_create_window(app: &AppHandle, label: &str) -> Result<WebviewWindow>;

pub fn hud_effects() -> WindowEffectsConfig;

pub fn position_on_cursor_monitor(window: &WebviewWindow);
pub fn position_orb_bottom_right(window: &WebviewWindow);

pub fn parse_coding_label(label: &str) -> Option<&str>;  // "coding:{repo_id}" → Some(repo_id)
```

---

## The 17-step startup sequence

```rust
fn main() -> Result<(), Box<dyn Error>> {
    let raw_args: Vec<String> = std::env::args().collect();

    // ─── Step 1: --hook short-circuit ──────────────────────────
    if raw_args.get(1).map(|s| s.as_str()) == Some("--hook") {
        let hook_args = &raw_args[2..];
        std::process::exit(match coding_ingest::hook_cli::run(hook_args) {
            Ok(_) => 0,
            Err(e) => { eprintln!("{e}"); 1 }
        });
    }

    // ─── Step 2: pre_main_hardening ────────────────────────────
    // MUST precede mimalloc init (scrubs MALLOC_* / DYLD_* / LD_* env vars,
    // ptrace deny, RLIMIT_CORE=0).
    klynt_process_hardening::pre_main_hardening();

    // ─── Step 3: configure_mimalloc ────────────────────────────
    // Sets MI_OPTION_PURGE_DELAY=0, ARENA_PURGE_MULT=1, ABANDONED_PAGE_PURGE=1.
    // Disables large OS pages + eager commit. Minimizes RSS growth.
    configure_mimalloc();

    // ─── Step 4: Cli::parse ─────────────────────────────────────
    let cli = Cli::parse();
    match cli.command {
        Some(McpCommand::Serve { stdio: true }) => return run_mcp_stdio(),
        Some(McpCommand::Tools { list: true })  => return run_mcp_tools_list(),
        None => { /* fall through to desktop app */ }
    }

    // ─── Steps 5-17: run_desktop_app() ─────────────────────────
    run_desktop_app()
}

fn run_desktop_app() -> Result<()> {
    // Step 5: Register purge_mimalloc as global memory hook.
    common::memory::set_purge_hook(purge_mimalloc);

    // Build leaked 4-worker tokio runtime (2MB stacks).
    let rt = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()?));

    // Init tracing to stderr.
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    // Specta builder; in debug, export bindings.ts.
    let specta = specta_builder::build_specta();
    #[cfg(debug_assertions)]
    specta.export("desktop-ui/src/bindings.ts")?;

    // Step 6: Tauri builder with plugins.
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())

    // Step 7: setup closure.
        .setup(|app| {
            specta.mount_events(app);

            // app_core::init (blocking) — builds AppCore.
            let (core_inner, global_event_tx, approval_channel) = rt.block_on(
                app_core::init(app.handle().clone())
            )?;

            // First-launch claude-code MCP registration (idempotent, marker-file-gated).
            std::thread::spawn({
                let handle = app.handle().clone();
                move || claude_code_integration::run_first_launch_check(handle)
            });

            // Step 8: Managed state.
            app.manage(core_inner);
            app.manage(approval_channel);
            app.manage(Arc::new(focus_timer::FocusTimer::new()));

            // Optional dev HTTP server (debug only).
            #[cfg(debug_assertions)]
            if std::env::var("KLYNTBOT_DEV_SERVER").is_ok() {
                rt.spawn(dev_server::start(app.handle().clone(), core.clone()));
            }

            // Optional embedded MCP HTTP server.
            if core.config.read().await.mcp.server.enabled {
                rt.spawn(serve_embedded_mcp_http(app.handle().clone(), core.clone()));
            }

            // Step 9: Secondary windows are LAZY — not created here.

            // Step 10: shortcuts::register_shortcuts
            shortcuts::register_shortcuts(app.handle(), &config)?;

            // Step 11: Voice hotkey (separate from the 3-shortcut system)
            shortcuts::register_voice_hotkey(app.handle(), &config)?;

            // Step 12: macOS menu (Cmd+Q hides; Cmd+W unbound)
            #[cfg(target_os = "macos")]
            build_macos_menu(app)?;

            // Step 13: Tray icon
            build_tray(app, core.clone())?;

            // Step 14: mimalloc compaction timer (10s interval)
            rt.spawn(mimalloc_compaction_loop(shutdown_token.clone()));

            // Step 15: tray_countdown
            tray_countdown::spawn(app.handle().clone(), core.clone());

            // Step 16: OAuth (lazy — spins up Axum server on first OAuth start)
            // Registered as Tauri commands in commands/oauth_*.rs
            Ok(())
        })

    // Step 17: invoke handler
        .invoke_handler(crate::specta_builder::klynt_invoke_handler())
        .run(tauri::generate_context!())?;

    Ok(())
}
```

### Why hardening MUST precede mimalloc

mimalloc reads `MALLOC_*` / `MallocStackLogging` env vars at initialization. **The hardening step scrubs these before mimalloc looks.** If anyone reorders these (e.g., "let's init the allocator early for perf"), the hardening becomes a no-op and an attacker who can set env vars in the parent process can manipulate the allocator. Reordering breaks the security model.

---

## Macros (from `desktop-macros`)

### `#[klynt_command]`

```rust
#[klynt_command]
pub async fn task_create(req: CreateTaskRequest) -> Task {
    state.task_create(req).await.unwrap()
}
```

**Constraints (enforced by macro):**
- Must be `pub async fn`
- Must NOT have an explicit `state: State<…>` parameter (injected automatically)
- Must NOT return `Result<…>` (wrapped automatically as `CommandResult<T>`)
- Must have a return type

**What it generates:**
- Injects `state: tauri::State<'_, Arc<AppCore>>` as the first parameter
- Wraps the return type as `CommandResult<Task>` (which is `Result<Task, ApiError>`)
- Adds `#[tauri::command]` + `#[specta::specta]`
- Emits a `__klynt_dispatch_task_create` dispatcher function
- Emits a `#[linkme::distributed_slice(KLYNT_COMMANDS)]` static `CommandRegistration { name, invoke, source: SourceKind::Klynt }`

### `#[klynt_raw_command]`

```rust
#[klynt_raw_command]
pub async fn oauth_callback(state: State<'_, Arc<OAuthRegistry>>, code: String) -> Result<(), MyError> {
    // custom state + Result return — works here
    state.handle_callback(code).await
}
```

**No constraints** beyond requiring the function to be inside one of the macro-scanned paths (`commands/` or `oauth/`). Leaves body unchanged. Use for:
- OAuth flows (custom state types)
- Streaming/long-running commands (non-standard return types)
- Anything with `rename_all`, `serialize_with`, etc.

### `klynt_collect_commands![...]`

```rust
// crates/desktop/src/specta_builder.rs
desktop_macros::klynt_collect_commands![
    crate::commands::chat::chat_send,
    crate::commands::chat::chat_cancel,
    crate::commands::tasks::task_create,
    // … (~465 entries)
];
```

**Generates:**
- `pub const KLYNT_SPECTA_COMMAND_NAMES: &[&str]` — last path segment of each
- `pub(crate) fn __klynt_specta_commands() -> tauri_specta::Commands<Wry>` — calls `tauri_specta::collect_commands![...]` with all paths

### `klynt_collect_events![...]`

```rust
desktop_macros::klynt_collect_events![
    crate::events::thread_event,
    crate::events::approval_event,
    // … 
];
```

Generates the event registration array for specta event export.

### `SPECTA_COMMAND_NAMES`

```rust
pub use self::KLYNT_SPECTA_COMMAND_NAMES as SPECTA_COMMAND_NAMES;
```

Backward-compat alias. Used by the `registration_drift` test.

---

## Four CI guards

| Test | What it checks | Failure shape |
|---|---|---|
| `no_raw_tauri_command_outside_macros` | `rg`-scans `crates/desktop/src/commands/` + `crates/desktop/src/oauth/` for bare `#[tauri::command]` not wrapped in either macro | Lists offending file:line pairs |
| `registration_drift` | Compares `KLYNT_COMMANDS` (linkme slice, runtime truth) with `SPECTA_COMMAND_NAMES` (specta hand-list, FE binding truth) as `BTreeSet<&str>` | Prints the diff (in linkme but not specta / vice versa) |
| `bindings_are_current` | Calls `build_specta().export_str(Typescript::default())` and compares byte-for-byte against `desktop-ui/src/bindings.ts` | Writes the regenerated file on failure (next run is green if you commit it) — also auto-regenerated by `cargo tauri dev` in debug builds |
| `no_double_registration` | Guards against the same command appearing twice in the linkme slice | Prints duplicate names |

All four run as part of `cargo nextest run -p desktop`.

---

## Secondary windows (5)

All created lazily by `lazy_window::get_or_create_window(app, label)`.

| Label | Size | Behaviors |
|---|---|---|
| `launcher` (WINDOW_LAUNCHER) | 660×580 | `hud_effects()`, dismiss-on-blur (also emits `voice-recording-reset`), `always_on_top`, `transparent`, no decorations, centered |
| `tray` (WINDOW_TRAY) | 320×600 | `hud_effects()`, dismiss-on-blur, `always_on_top`, `transparent`, `focused(false)` |
| `distraction-overlay` | 340×300 | `hud_effects()`, `always_on_top`, centered, `focused(true)` |
| `voice-orb` | 200×200 | Transparent, `always_on_top`, no decorations, no blur dismiss; positioned bottom-right of cursor monitor via `position_orb_bottom_right` |
| `coding:{repo_id}` | 1200×800 (min 700×500) | **Full decorations**, visible immediately, normal window. Label parsed by `parse_coding_label`. Per-repo, persists. *(CLAUDE.md doesn't list this one.)* |

### `hud_effects()` helper

```rust
pub fn hud_effects() -> WindowEffectsConfig {
    EffectsBuilder::new()
        .effect(Effect::HudWindow)
        .state(EffectState::Active)
        .radius(16.0)
        .build()
}
```

macOS vibrancy HUD style for floating panels.

### Drag handle pattern

CSS class `.lc-drag-handle` with `-webkit-app-region: drag`. `useWindowDrag.ts` also supports `data-tauri-drag-region` attribute zones via `getCurrentWindow().startDragging()` (calls `startDraggingSafe()`).

### `position_on_cursor_monitor`

```rust
pub fn position_on_cursor_monitor(window: &WebviewWindow) {
    if let Some(cursor) = cursor_position() {
        if let Some(monitor) = find_monitor_containing(cursor) {
            // Center on that monitor at 1/3 height (Spotlight-style)
            let center = compute_position(monitor, window.outer_size(), 1.0 / 3.0);
            window.set_position(center);
            return;
        }
    }
    window.center();  // fallback
}
```

---

## OAuth flow

`crates/desktop/src/oauth/` — local Axum HTTP server on a **fixed `CALLBACK_PORT`**.

```rust
// crates/desktop/src/oauth/mod.rs
pub async fn start_oauth_server(state: Arc<OAuthRegistry>) -> Result<()>;

// crates/desktop/src/oauth/registry.rs
pub struct OAuthRegistry {
    in_flight: DashMap<String /* state */, OAuthFlowState>,
    tokens: DashMap<String /* server */, OAuthTokens>,
}
```

### Workflow

```
1. Frontend invokes mcp_oauth_start("my-server")
2. Tauri command starts the local Axum server (lazy, first call)
3. Generates state + PKCE; opens provider's auth URL in default browser
4. User authorizes; provider redirects to http://localhost:<CALLBACK_PORT>/callback?code=...&state=...
5. Axum callback handler:
   - Validates state via OAuthRegistry.in_flight
   - Exchanges code for tokens via provider's token endpoint
   - Stores via OAuthRegistry.tokens
   - Emits McpOAuthCompletePayload event
   - Returns success HTML page (auto-closes browser tab)
6. Frontend listens for McpOAuthCompletePayload → updates UI
```

### Failure modes

- **`CALLBACK_PORT` in use** → OAuth start fails. **No retry, no fallback.** P1 debt item.
- **State validation fails** → callback returns 400; user sees error page.
- **Token exchange fails** → callback returns 500; error stored in registry; frontend gets error event.

---

## Tray icon + countdown

### Tray icon

```rust
TrayIconBuilder::with_id("klynt-tray")
    .icon(/* embedded PNG */)
    .menu(/* menu */)
    .on_left_click(|app, _event| {
        if VOICE_ACTIVE.load(Ordering::Relaxed) {
            voice_pause_resume(app);
        } else {
            shortcuts::toggle_window(app, WINDOW_TRAY);
        }
    })
```

Left-click is **context-aware**: voice toggle if voice is active, otherwise tray window toggle.

### `tray_countdown::spawn`

```rust
pub fn spawn(handle: AppHandle, core: Arc<AppCore>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval = compute_tick_interval(&core).await;
            tokio::time::sleep(interval).await;
            update_tray_title(&handle, &core).await;
        }
    });
}

fn compute_tick_interval(core: &AppCore) -> Duration {
    if has_visible_countdown(core) { Duration::from_secs(1) }
    else if voice_active() { Duration::from_secs(2) }
    else if focus_active() { Duration::from_secs(60) }
    else { Duration::from_secs(3600) }  // idle
}
```

**Note:** Uses `tauri::async_runtime::spawn`, not `tokio::spawn`. Starts during the Tauri `setup` hook **before the tokio runtime is available** (the runtime is the leaked `rt`, not Tauri's).

---

## Internals

### Triple-mode dispatch

The desktop binary handles three operational modes determined by `argv`:

| Mode | Trigger | Init does |
|---|---|---|
| Hook | `argv[1] == "--hook"` | `hook_cli::run` → exit (no Tauri, no AppCore) |
| MCP stdio | `mcp serve --stdio` subcommand | `app_core::init` (Server mode) → `klyntbot_server::serve_stdio` → exit |
| Desktop | (default) | `app_core::init` (Desktop mode) → full Tauri loop |

### The 4-worker leaked tokio runtime

```rust
let rt = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)
    .thread_stack_size(2 * 1024 * 1024)
    .enable_all()
    .build()?));
```

Capped at 4 workers + 2 MB stacks (default is unbounded workers). For a single-user desktop app, more workers don't help and burn memory. **Leaked because Tauri's lifecycle outlives any structured drop** — drop would race with Tauri shutdown.

### Mimalloc explicit compaction

```rust
common::memory::set_purge_hook(purge_mimalloc);
```

Lower-layer crates (storage, agent) can trigger `mi_collect(true)` after large transient allocations (LanceDB compaction, index rebuilds) without going through a timer. The 10s timer (Step 14 of startup) is the fallback.

### macOS menu Cmd+Q maps to hide

```rust
.on_menu_event(|app, event| {
    if event.id() == "quit" {  // Cmd+Q triggers this
        if let Some(window) = app.get_webview_window("main") {
            window.hide().unwrap();
        }
        app.set_activation_policy(ActivationPolicy::Accessory);  // remove from Dock
    }
})
```

`CloseRequested` event is also intercepted to hide rather than close. Cmd+W is intentionally unbound so it's available for in-app navigation. Matches the menu-bar-app UX pattern (Spotlight, Raycast).

### `LEGACY_COMMAND_NAMES` is dead

```rust
// crates/desktop/src/lib.rs:16-19
pub const LEGACY_COMMAND_NAMES: &[&str] = &[
    // Deleted in Phase E
];
```

Empty array; awaits final removal. See [`TECH_DEBT.md`](../TECH_DEBT.md).

### `voice_active` is an `AtomicBool`

```rust
pub static VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);
```

Set by `VoiceConversationManager` lifecycle hooks. Read by tray-click handler + tray-countdown interval logic. Lock-free.

### `tray_countdown` uses `tauri::async_runtime::spawn`

The leaked tokio runtime isn't available during `setup` (the closure runs before `app.run()` enters the runtime). `tauri::async_runtime::spawn` uses Tauri's own (smaller) runtime — sufficient for the lightweight countdown loop.

### Bindings file is byte-compared

`bindings_are_current` test compares `bindings.ts` byte-for-byte. **Whitespace changes fail the test.** Hand-editing the file is wasted work — `cargo tauri dev` regenerates on every debug build.

### `--hook` path skips `pre_main_hardening`

The hook path exits within ~10ms. Hardening is unnecessary for a process that won't do anything after the hook. Skipping it keeps the hot path fast.

---

## Workflows

See [`subsystems/13-desktop-frontend.md`](../subsystems/13-desktop-frontend.md#workflows) for end-to-end:
- A Tauri command from frontend to backend
- Secondary window creation
- OAuth flow

Briefly:

### Tauri command end-to-end

```
1. Frontend: invoke("chat_send", args) — generated wrapper in bindings.ts
2. Tauri IPC: routes via klynt_invoke_handler dispatch table
3. crates/desktop/src/commands/chat.rs::chat_send (#[klynt_command])
   - state: State<Arc<AppCore>> injected by macro
   - calls state.handlers::chat::handle_send(args)
4. crates/app-core/src/handlers/chat/mod.rs::handle_send
   - returns Result<T, KlyntBotError>
5. Wrapped as CommandResult<T> (ApiError on Err)
6. Serialized via Tauri IPC back to frontend
```

### Adding a new secondary window

```
1. Add WINDOW_<NAME>: &str = "<name>" constant in lazy_window.rs
2. Add build_<name> function constructing WebviewWindowBuilder
3. Apply hud_effects() if floating panel
4. Register dismiss-on-blur if appropriate
5. Add arm in get_or_create_window
6. Add toggle_<name>_window command if you want shortcut-driven toggle
```

---

## Testing approach

### Run the 4 IPC guards

```bash
cargo nextest run -p desktop --test no_raw_tauri_command_outside_macros
cargo nextest run -p desktop --test registration_drift
cargo nextest run -p desktop --test bindings_are_current
cargo nextest run -p desktop --test no_double_registration
```

All four are part of normal `cargo nextest run -p desktop`.

### Regenerate `bindings.ts` after adding a command

```bash
cargo tauri dev   # debug build auto-regenerates bindings.ts
# OR
cargo nextest run -p desktop --test bindings_are_current   # writes the regenerated file on failure
```

Commit the regenerated `desktop-ui/src/bindings.ts`.

### Test a Tauri command in isolation

```rust
#[tokio::test]
async fn test_chat_send_directly() {
    let core = build_test_appcore().await;
    // Bypass Tauri IPC; call the function with a fake State
    let state = tauri::State::from(&Arc::new(core));
    let result = chat_send(state, /* args */).await;
    assert!(matches!(result, CommandResult::Ok(_)));
}
```

Most logic lives in `app-core`; the desktop shim is too thin to need its own tests beyond the 4 guards.

### Test `lazy_window` factories

`hud_effects()` and `position_on_cursor_monitor` can be unit-tested without Tauri. Window builders need a `tauri::App` instance — typically tested via end-to-end smoke tests in `tests/`.

---

## Extension points

### Add a Tauri command

1. Pick a macro: `#[klynt_command]` (happy path) or `#[klynt_raw_command]` (otherwise).
2. Write the function in `crates/desktop/src/commands/<domain>.rs` (or `oauth/` for OAuth commands).
3. Add to `klynt_collect_commands![...]` in `specta_builder.rs`.
4. Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`.
5. Commit the regenerated `bindings.ts`.

The 4 guard tests will pass. Frontend gets typed wrappers via `bindings.ts`.

### Add a Tauri event

1. Define the event in `crates/desktop-shared/` or `crates/desktop/src/events.rs`.
2. Add to `klynt_collect_events![...]`.
3. Emit from backend: `app.emit("event:name", payload)`.
4. Subscribe from frontend: `listen("event:name", handler)`.

### Add a secondary window

See [Workflows](#workflows). Don't forget to add CSS for the route + a `position_*` helper if it needs custom positioning.

### Modify the startup sequence

Edit `main.rs`. **Don't reorder `pre_main_hardening` and `configure_mimalloc`** — the env-var scrub must precede allocator init.

### Add a tray menu item

```rust
let menu = MenuBuilder::new(app)
    .item(&MenuItemBuilder::new("My Action").id("my_action").build(app)?)
    // …
    .build()?;

TrayIconBuilder::with_id("klynt-tray")
    .menu(menu.clone())
    .on_menu_event(|app, event| match event.id() {
        "my_action" => handle_my_action(app),
        // …
    })
```

### Add an OAuth provider

1. Add provider config in `crates/config/src/schema/oauth.rs` (if new pattern).
2. Add a Tauri command `mcp_oauth_start_<provider>` in `oauth/`.
3. The Axum callback server is shared — your callback URL is `http://localhost:<CALLBACK_PORT>/callback`.
4. Encode the provider name in `state` so the dispatcher knows which token-exchange to run.

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| tokio worker threads | `4` | `main.rs::run_desktop_app` |
| tokio thread stack size | `2 MB` | `main.rs::run_desktop_app` |
| mimalloc compaction interval | `10s` | `main.rs::mimalloc_compaction_loop` |
| `WINDOW_LAUNCHER` | `"launcher"` | `lazy_window.rs` |
| `WINDOW_TRAY` | `"tray"` | `lazy_window.rs` |
| Launcher size | `660 × 580` | `lazy_window.rs` |
| Tray size | `320 × 600` | `lazy_window.rs` |
| Distraction overlay size | `340 × 300` | `lazy_window.rs` |
| Voice orb size | `200 × 200` | `lazy_window.rs` |
| Coding window size | `1200 × 800` (min `700 × 500`) | `lazy_window.rs` |
| HUD radius | `16.0` | `lazy_window.rs::hud_effects` |
| Tray countdown intervals | `1s` / `2s` / `60s` / `3600s` | `tray_countdown.rs::compute_tick_interval` |
| Spotlight-style vertical position | `1/3 from top` | `lazy_window.rs::position_on_cursor_monitor` |
| `CALLBACK_PORT` (OAuth) | fixed (no fallback) | `oauth/mod.rs` |

---

## Open questions

- **`CALLBACK_PORT` is fixed.** Port conflict = silent OAuth failure. Add a fallback or document.
- **`LEGACY_COMMAND_NAMES` is empty dead code.** Awaiting final removal.
- **`bindings.ts` byte-compare is brittle to whitespace.** Consider semantic comparison.
- **No `/health` route** on embedded MCP HTTP server.
- **`crates/desktop-ui/` stub naming is confusing** — the real frontend is at `/desktop-ui/` (repo root). Consider renaming the stub to `desktop-bindings`.
- **`tray_countdown` adaptive intervals** are hardcoded. Could be config-driven.
- **The 17-step startup sequence has implicit ordering** (esp. hardening → mimalloc → AppCore → Tauri). A breaking refactor needs a checklist; today only this doc + CLAUDE.md serve as the checklist.
- **Voice hotkey is registered separately** from the 3-shortcut system. Inconsistency; could unify.
- **macOS-only features** (HudWindow effect, Cmd+Q menu, NSWorkspace stubs in platform-macos) aren't guarded uniformly. Cross-platform builds would need audit.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #4 + #5 + #9 for specifics.

---

## Cross-references

- [Subsystem 13 — Desktop App & Frontend](../subsystems/13-desktop-frontend.md) (parent)
- [`crates/app-core.md`](./app-core.md) — `AppCore::init` constructs everything beneath
- [`crates/mcp.md`](./mcp.md) — `mcp serve --stdio` + embedded HTTP
- [Subsystem 10 — Sandboxing & Security](../subsystems/10-sandboxing-security.md) — `pre_main_hardening` ordering constraint
- [`crates/coding-ingest.md`](./coding-ingest.md) — `--hook` short-circuit
- [Subsystem 11 — Channels, MCP](../subsystems/11-channels-mcp.md) — `mcp-bridge` Unix socket consumer
