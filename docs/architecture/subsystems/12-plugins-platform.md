# Subsystem 12 — Plugin System & Platform Adapters

> **Status:** 🟠 Scaffolded
> Plugin infrastructure works (WASM sandbox + 5 host namespaces) but `agent_ask_user` is a stub, `cron_jobs` are parsed but not scheduled, and there's no hot-reload.
> Platform Computer Use surface exists (16 `ComputerUseAction` variants, full AX walker) but is **completely unwired** — no agent tool, Tauri command, or MCP tool routes to it.
> **Status last verified:** 2026-05-16
> **Crates:** `plugin-runtime`, `plugin-sdk`, `platform-input`, `platform-capture`, `platform-macos` *(5 crates)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Two distinct extension surfaces grouped because both expose **infrastructure that exists but isn't connected to user-visible behavior**.

**Plugins** — Extism WASM sandbox with 5 host-function namespaces (`db`, `log`, `http`, `agent`, `tool`) and 3 permissions (`Network`, `Storage`, `Agent`). Plugins are restart-only (no hot-reload). The `agent_ask_user` host function returns `{"error":"agent callbacks not connected"}` unconditionally — granting `Agent` permission does nothing today. Plugin manifests declare `cronJobs` but no executor reads them.

**Platform adapters** — Two trait crates (`platform-input`, `platform-capture`) + a macOS implementation (`platform-macos`). Full `ComputerUseAction` enum (16 variants matching Anthropic's `computer_20251124` tool 1:1). `MacInput` implements 14 of 16; `MacCapture` implements 3 of 5. **No agent tool, Tauri command, or MCP tool calls any of this.** The wiring layer is missing.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef plug fill:#b3e5fc,stroke:#0277bd,color:#01579b
    classDef trait fill:#e8eaf6,stroke:#3949ab,color:#1a237e
    classDef impl fill:#fff3e0,stroke:#f57c00,color:#e65100
    classDef stub fill:#f5f5f5,stroke:#999,color:#616161
    classDef gap fill:#ffcdd2,stroke:#c62828,color:#b71c1c

    PM[PluginManager<br/><i>scan_manifests · load_all<br/>NO unload · NO hot-reload</i>]:::plug
    HF[Host namespaces<br/><i>db · log · http · agent · tool</i><br/>14 functions]:::plug
    PERM[PluginPermission<br/><i>Network · Storage · Agent</i>]:::plug
    SDK[plugin-sdk<br/><i>extism-pdk re-export + helpers<br/>dead db_query placeholder</i>]:::plug

    PI[PlatformInput trait<br/><i>perform_action<br/>get_cursor_position<br/>release_all</i>]:::trait
    PC[PlatformCapture trait<br/><i>capture_screen<br/>capture_window<br/>list_displays<br/>get_active_window<br/>get_ax_tree</i>]:::trait
    CUA[ComputerUseAction enum<br/><i>16 variants<br/>Anthropic computer_20251124 1:1</i>]:::trait

    MI[MacInput<br/><i>CGEvent<br/>14 of 16 actions impl<br/>Screenshot + Zoom NotImpl</i>]:::impl
    MC[MacCapture<br/><i>ScreenCaptureKit<br/>3 of 5 methods impl<br/>capture_window + get_active_window NotImpl<br/>scale hardcoded 2.0</i>]:::impl
    AX[walk_focused_app<br/><i>AX walker · depth 6<br/>frames in AppKit coords (NOT Quartz)</i>]:::impl

    OTHER[Other platform-macos modules<br/><i>speech (say CLI) · DnD (shortcuts)<br/>lifecycle (idle polling only)<br/>apps · browser · pasteboard · ax</i>]:::impl

    AGT[agent_ask_user<br/>ALWAYS-DECLINES STUB]:::stub
    CRON[plugin cron_jobs<br/>PARSED BUT NOT SCHEDULED]:::stub
    WIRE[Agent / Tauri / MCP wiring<br/>DOES NOT EXIST]:::gap

    PM --> HF
    HF --> PERM
    HF -.contains.-> AGT
    PM -.parses.-> CRON
    SDK -.calls.-> HF
    MI --> PI
    MC --> PC
    MC --> AX
    PI --> CUA
    WIRE -.MISSING.-> MI
    WIRE -.MISSING.-> MC
```

---

## Mental model

Plugins and platform adapters are both **plug-in points** that the agent will eventually grow into. Today neither is wired to LLM-callable surfaces:

- **Plugins:** The WASM host works. You can ship a plugin and Klyntbot will load it, expose its tools via `FeaturePackage`, give it scoped DB access, and emit its events on the bus. But the `agent_ask_user` callback is dead, plugins can't be hot-reloaded, and any `cronJobs` in the manifest are parsed but ignored.
- **Platform:** `MacInput` can move a mouse, type text, click, drag, scroll. `MacCapture` can take a screenshot of the screen. `walk_focused_app` returns an AX tree of the frontmost app. But nothing inside `agent`, `klynt-core`, or `mcp` ever calls any of this. Computer Use is **scaffolded, not shipped**.

The clearest mental model: this subsystem is **the floor of a future feature, not a current one**. Two scaffolds; both await a wiring layer.

---

## Reference

### `plugin-runtime` — file map

| Path | Purpose |
|---|---|
| `src/lib.rs` | Re-exports + `PluginManager`, `PluginPackage`, `PluginsConfig` |
| `src/manager.rs` | `PluginManager` — `scan_manifests`, `load_all`, plugin lifecycle |
| `src/package.rs` | `PluginPackage` — `FeaturePackage` impl wrapping plugin tools |
| `src/manifest.rs` | `PluginManifest`, `PluginCronJob`, `PluginMigration`, `PluginPermission` |
| `src/host/mod.rs` | Host function namespaces (`db`, `log`, `http`, `agent`, `tool`) |
| `src/host/{db,log,http,agent,tool}.rs` | Per-namespace implementations |
| `src/sandbox.rs` | `is_select_only`, `check_table_sandbox` heuristics |

### Host functions — full enumeration

All 12 host functions live in the Extism namespace `"klyntbot"`. Plugin `wasm` declares `host_fn!(klyntbot, fn_name)` to bind them.

**`db` (2 functions)** — `Storage` permission required:

| Function | Behavior |
|---|---|
| `db_query(sql) -> String` | SELECT-only via `is_select_only`; table sandbox via `check_table_sandbox`. Executes through `sqlx::query_scalar`. |
| `db_execute(sql) -> String` | Full CRUD (any non-SELECT); table sandbox check. Returns `{"rows_affected":N}`. |

**`log` (4 functions)** — no permission required:

| Function | Behavior |
|---|---|
| `log_debug(msg)`, `log_info(msg)`, `log_warn(msg)`, `log_error(msg)` | Each calls matching `tracing` macro tagged with `plugin_id`. |

**`http` (1 function)** — `Network` permission required:

| Function | Behavior |
|---|---|
| `http_request(json) -> String` | Accepts `{url, method, body}`. Uses shared `reqwest::Client`. Returns `{status, body}`. |

**`agent` (3 functions)** — `Agent` permission required:

| Function | Behavior |
|---|---|
| `agent_send_message(json) -> String` | `{channel, chat_id, content}` → `bus_sender.try_send(OutboundMessage)`. |
| `agent_ask_user(question) -> String` | **🔴 STUB — always returns `{"error":"agent callbacks not connected"}`.** Tagged "Task #8" in source. Granting `Agent` permission does not enable interactive prompts. |
| `agent_emit_event(json) -> String` | `PluginEmittedEvent { kind, payload }`. **Validation:** non-empty, ASCII alphanumeric/underscore, ≤ 64 chars, payload ≤ 4 KiB. Publishes `DomainEvent::PluginEvent { plugin_id, kind, payload }`. Silently drops if `domain_event_bus = None`. |

**`tool` (2 functions)** — no permission required:

| Function | Behavior |
|---|---|
| `tool_return(result)` | Logs at `info!`; host reads result from WASM memory after call completes. |
| `tool_error(msg)` | Logs at `error!`. |

### `PluginPermission`

```rust
pub enum PluginPermission { Network, Storage, Agent }
```

Checked at the top of each host function via `ctx.permissions.contains(&PluginPermission::X)`. Returns an error string into the output buffer without panicking. **`Agent` grants nothing functional today** because `agent_ask_user` is stubbed.

### `klyntbot.plugin.json` manifest schema

```jsonc
{
    "id": "my-plugin",                      // required
    "name": "My Plugin",                    // required
    "version": "0.1.0",                     // required
    "description": "What it does",          // required
    "author": "Author Name",                // required
    "minKlyntbotVersion": "0.1.0",          // optional
    "tools": [                              // optional — agent-callable tools
        { "name": "...", "description": "...", "parameters": {...} }
    ],
    "cronJobs": [                           // optional — PARSED BUT NOT SCHEDULED
        { "tool": "...", "schedule": "...", "description": "..." }
    ],
    "migrations": [                         // optional — runs at plugin load
        { "version": 1, "description": "...", "sql": "..." }
    ],
    "permissions": ["network", "storage", "agent"],   // optional
    "configSchema": {                       // optional — typed config
        "key": { "type": "string", "secret": false, "description": "..." }
    }
}
```

### WASM load lifecycle

```
PluginManager::load_all(plugins_dir, pool, config, bus, domain_event_bus):
  if !config.enabled or !plugins_dir.exists() → return empty
  for each subdir in plugins_dir:
    1. Read klyntbot.plugin.json → PluginManifest
    2. Read plugin.wasm
    3. Apply migrations from manifest.migrations
    4. Instantiate Extism plugin with host namespaces wired
    5. Construct PluginPackage (implements FeaturePackage)
    6. Add to manager.packages
```

**No hot-reload, no unload, no swap.** Restart-only.

### Table sandboxing (2 layers)

| Layer | Check |
|---|---|
| `is_select_only(sql)` | SELECT / WITH / EXPLAIN only. Rejects multi-statement injections via internal semicolons. Rejects mutation keywords as standalone tokens. Applies only to `db_query`. |
| `check_table_sandbox(sql, plugin_id)` | Extracts identifiers following `FROM`/`JOIN`/`INTO`/`UPDATE`/`TABLE` keywords. Asserts each starts with `plugin_{id}_` (hyphens in `id` replaced with underscores). Applies to both `db_query` and `db_execute`. |

**Heuristic, not parser.** Cannot handle aliased subqueries, CTEs that shadow names, or quoted identifiers. Conservative for the common case; not a formal sandbox.

### `plugin-sdk` — author-facing

```toml
crate-type = ["cdylib", "rlib"]
```

```rust
// What plugin authors write
use klyntbot_plugin_sdk::prelude::*;

#[plugin_fn]
pub fn my_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input)?;
    Ok(format!("Got: {}", args))
}
```

Re-exports `extism-pdk` at crate root. `prelude` module also re-exports `serde`, `serde_json`, and SDK helpers (`config_get`, `http_get`, `log_*`).

**Dead `db_query` no-op:**
```rust
pub fn db_query(_sql: &str) -> String {
    match extism_pdk::var::get("__db_query_not_implemented") {
        Ok(Some(v)) => String::from_utf8(v).unwrap_or_default(),
        _ => "[]".to_string(),
    }
}
```
The real call goes through the host function in the `"klyntbot"` namespace. The SDK wrapper never invokes that host function and always returns `"[]"`. **Misleading placeholder.** See [`TECH_DEBT.md`](../TECH_DEBT.md) §2.

---

### Platform traits

#### `PlatformInput` (in `platform-input`)

```rust
#[async_trait]
pub trait PlatformInput: Send + Sync {
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()>;
    async fn get_cursor_position(&self) -> Result<Point>;
    async fn release_all(&self) -> Result<()>;
}
```

Coordinates are logical points, **Quartz top-left origin** (note this differs from AX tree below).

#### `PlatformCapture` (in `platform-capture`)

```rust
#[async_trait]
pub trait PlatformCapture: Send + Sync {
    async fn capture_screen(&self, region: Option<Rect>) -> Result<Frame>;
    async fn capture_window(&self, window_id: WindowId) -> Result<Frame>;
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>>;
    async fn get_active_window(&self) -> Result<Option<WindowInfo>>;
    async fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode>;
}

pub enum AxScope { FullDesktop, ActiveApp, Window(WindowId) }
```

#### `ComputerUseAction` — all 16 variants

Tagged `#[serde(tag = "kind", rename_all = "snake_case")]`. Maps 1:1 to Anthropic's `computer_20251124` tool surface.

| Variant | Fields | `MacInput` impl |
|---|---|---|
| `Screenshot` | `region: Option<Rect>` | ❌ `NotImplemented` |
| `LeftClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `DoubleClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `TripleClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `RightClick` | `x, y: i32` | ✅ |
| `MiddleClick` | `x, y: i32` | ✅ |
| `Type` | `text: String` | ✅ unicode strings via `set_string_from_utf16_unchecked` |
| `Key` | `keys: Vec<String>` | ✅ static virtual-key table (a-z + function keys) |
| `MouseMove` | `x, y: i32` | ✅ |
| `Scroll` | `x, y: i32, direction: ScrollDir, amount: i32` | ✅ moves cursor first, then posts `CGScrollWheelEvent` |
| `LeftClickDrag` | `from, to: Point, hold_modifiers: KeyMods` | ✅ **16 interpolated `LeftMouseDragged` events with 8 ms sleep** |
| `LeftMouseDown` | `x, y: i32` | ✅ |
| `LeftMouseUp` | `x, y: i32` | ✅ |
| `HoldKey` | `keys: Vec<String>, duration_ms: u32` | ✅ |
| `Wait` | `duration_ms: u32` | ✅ |
| `Zoom` | `region: Rect` | ❌ `NotImplemented` |

`MacInput::release_all` posts `LeftMouseUp` + `RightMouseUp` + `OtherMouseUp` at current cursor position — used as the emergency-stop primitive.

#### `AccessibilityNode` shape

```rust
pub struct AccessibilityNode {
    pub role: String,                                // e.g. "AXButton", "AXTextField"
    pub label: Option<String>,                       // AXTitle, fallback AXDescription
    pub value: Option<String>,                       // AXValue
    pub frame: Rect,                                 // ⚠️ AppKit coords (bottom-left), NOT Quartz
    pub children: Vec<AccessibilityNode>,
    pub attrs: HashMap<String, String>,              // currently always empty from walker
}
```

**⚠️ `frame` is in AppKit coordinate space (bottom-left origin), not Quartz (top-left).** Y-flip is a Phase 4 TODO. Consumers expecting Quartz coordinates will draw bounding boxes in the wrong place. The whole `PlatformInput`/`PlatformCapture` API documents Quartz; this AX walker accidentally returns the other space.

### `platform-macos` modules

| Module | Purpose | Notes |
|---|---|---|
| `capture.rs` | `MacCapture` — `screencapturekit::SCShareableContent` + `SCScreenshotManager::capture` inside `spawn_blocking`. | Scale hardcoded `2.0` (Retina assumption). `capture_window` + `get_active_window` return `NotImplemented`. |
| `input.rs` | `MacInput` — `CGEventSource` with `HIDSystemState`. | 14 of 16 actions implemented (see table above). |
| `computer_use/ax_tree.rs` | `walk_focused_app(pid, max_depth=6)` — `AXUIElement` + recursive children. | Depth bounded at 6. Frame y-flip not done. |
| `ax.rs` | RAII wrappers for `AXUIElement` and `AXValue` (`declare_TCFType!`/`impl_TCFType!`). | Safe — no manual `CFRelease` anywhere. |
| `speech.rs` | `list_voices()` parses `say --voice=?`; `synthesize_to_file(text, voice, rate, path)` → `say -v <voice> -r <wpm> -o <path>`. | **`say` CLI, NOT `AVSpeechSynthesizer`.** objc2 wiring deferred. |
| `dnd.rs` | `is_dnd_active()` reads `defaults read com.apple.controlcenter "NSStatusItem Visible FocusModes"`; `toggle_dnd()` calls `shortcuts run "Toggle Do Not Disturb"`. | **Requires user to manually create the Shortcut.** Brittle dependency. |
| `lifecycle.rs` | `LifecycleStateMachine` (pure state machine) + `LifecycleMonitor` (tokio loop polling `CGEventSourceSecondsSinceLastEventType`). | **NSWorkspace observers stubbed** (`willSleep`/`didWake` never fire). Only idle transitions work. |
| `apps.rs` | `running_applications()` (NSWorkspace `runningApplications`, filtered `Regular` activation policy); `AppIconCache` (PlistBuddy + `sips` for icns→PNG conversion). | **`AppIconCache` deliberately avoids NSWorkspace** to prevent IconServices mmap leak. |
| `browser.rs` | Static `BROWSERS` registry — 11 entries: Chrome, Arc, Brave, Edge, Safari, Firefox, Vivaldi, Opera, Chromium, Orion, **Zen**. | AppleScript via `osascript`; sanitizes app name to prevent injection. |
| `pasteboard.rs` | `pasteboard_change_count()` + `read_pasteboard_string()` via `objc2-app-kit::NSPasteboard`. | |
| `window.rs` | Window bridge: `get_frontmost_window()`, `get_frontmost_app_name()`, `get_screen_frame()`, `set_window_frame(pid, x, y, w, h)`. | All AX read/write paths through `crate::ax::AXUIElement`. |

---

## Computer Use wiring status

**Confirmed: no wiring exists.** Searching all crates outside `platform-macos`, `platform-capture`, and `platform-input` finds only `crates/desktop-shared/src/permissions.rs` referencing these types — for permission *checking*, not invocation.

**Design spec referenced but vapor:** `docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md` is cited in `TECH_DEBT.md` and earlier docs. **The file does not exist in the repository.** Either never committed or aspirational path.

**What exists:**
- Full `ComputerUseAction` enum (16 variants) matching Anthropic's `computer_20251124` tool 1:1
- `MacInput` with 14 of 16 actions implemented
- `MacCapture` with `capture_screen` + `list_displays` + `get_ax_tree(ActiveApp)` working
- AX tree walker (`walk_focused_app`) bounded at depth 6, tested in `platform-macos/tests/computer_use_smoke.rs`

**What's missing:**
- No `ToolKit` / `FeaturePackage` wrapping `MacInput`/`MacCapture`
- No agent-loop dispatcher handling `ComputerUseAction`
- No Tauri command or MCP tool surface
- No emergency-stop hotkey hook (referenced in `release_all` docstring but not wired)
- `get_active_window` not implemented
- `capture_window` returns `NotImplemented`
- AX frame y-flip (AppKit → Quartz) deferred
- `FullDesktop` and `Window(id)` AX scopes return errors
- Retina scale hardcoded at 2.0

---

## Workflows

### Plugin lifecycle

```
1. Startup: PluginManager::load_all(~/.klyntbot/plugins/, ...)
   - scan_manifests: walk subdirs, find klyntbot.plugin.json + plugin.wasm pairs
   - For each plugin:
     a. Apply migrations from manifest
     b. Instantiate Extism plugin with host namespaces ("klyntbot")
     c. Wrap in PluginPackage (FeaturePackage impl)
2. Plugin tools registered via FeaturePackage::tools() (Path A)
3. LLM invokes a plugin tool
4. PluginPackage routes to Extism plugin.call(fn_name, json)
5. Plugin WASM executes; may call host functions (db_query, log_info, http_request, etc.)
6. Each host function: permission check → table sandbox check (if db_*) → execute → write result to WASM memory
7. Plugin returns via tool_return; host reads memory + surfaces to LLM
```

### Why plugin authors can't use `agent_ask_user` today

```
1. Plugin grants self Agent permission in manifest
2. Plugin calls agent_ask_user("Confirm?")
3. Host function (plugin-runtime/src/host/mod.rs:477):
   - permission_check passes
   - Returns hardcoded: {"error":"agent callbacks not connected"}
4. Plugin receives error JSON; no interaction surface available
5. Plugin must fall back to agent_send_message + buffer the next inbound
```

**Status:** Task #8 in source. No timeline. Granting `Agent` permission today is misleading — only `agent_send_message` + `agent_emit_event` actually work.

### What a Computer Use call *would* look like (if wired)

```
LLM emits Anthropic computer tool call:
   { tool: "computer", action: "left_click", coordinate: [800, 600] }
   ↓
(MISSING) Adapter translates Anthropic tool input → ComputerUseAction::LeftClick { x: 800, y: 600, modifiers: KeyMods::default() }
   ↓
(MISSING) FeaturePackage / ToolKit calls PlatformInput::perform_action
   ↓
MacInput::perform_action(action):
   - Match LeftClick → post_click(x, y, modifiers, click_count=1)
   - CGEvent created with HID source
   - CGEventPost(kCGHIDEventTap, event)
   ↓
Returns Ok(()) — but no audit log, no risk gate, no approval check, no screen capture
```

---

## Internals

### Why `agent_emit_event` validates so strictly

`kind` must be non-empty, ASCII alphanumeric/underscore, ≤ 64 chars. Payload ≤ 4 KiB. These limits exist because the event is published to `DomainEventBus` with no further validation — subscribers trust it. The strict shape prevents plugins from polluting the bus with malformed events that could break consumers.

### `LifecycleMonitor` only detects idle

The state machine handles `on_idle_reading / on_will_sleep / on_did_wake / on_grace_expired`. The monitor wraps it in a polling loop over `CGEventSourceSecondsSinceLastEventType`. **NSWorkspace `willSleep`/`didWake` observers are stubbed** — they never fire. So sleep/wake transitions aren't detected; only idle is. This means cron jobs scheduled for "after wake" don't reliably fire on resume from sleep.

### `MacInput::LeftClickDrag` interpolation

```
for step in 1..=16:
    let t = step as f64 / 16.0;
    let x = from.x + (to.x - from.x) * t;
    let y = from.y + (to.y - from.y) * t;
    post LeftMouseDragged at (x, y)
    sleep 8ms
```

16 events × 8 ms = ~128 ms total drag duration. Trade-off: too few interpolation points and the drag looks teleported (some apps drop it); too many and it's slow. 16 is empirically the sweet spot.

### Why `AppIconCache` avoids NSWorkspace

The source comment explicitly cites `IconServices mmap leak`. NSWorkspace's icon-fetch APIs (`iconForFile:`) leak file descriptors per call. PlistBuddy + `sips` shells out (slower per call) but no leak. The cache (mtime-validated, on disk) makes the slow first call irrelevant for steady-state.

### `ax::declare_TCFType!` for AX safety

AX APIs use Core Foundation reference counting (`CFRetain`/`CFRelease`). Forgetting `CFRelease` leaks; double-`CFRelease` crashes. The `declare_TCFType!`/`impl_TCFType!` macros from `core_foundation` create wrappers that integrate with Rust's RAII — `Drop` calls `CFRelease` automatically. **No manual `CFRelease` anywhere in `platform-macos`.**

---

## Dependencies & extension points

### Upstream deps

- **plugin-runtime:** `extism = "1"`, `wasmtime` (Extism dep)
- **plugin-sdk:** `extism-pdk = "1"`
- **platform-macos:** `objc2` family, `core_foundation`, `core_graphics`, `screencapturekit`, `system_configuration`

### Adding a plugin

1. Author with `klyntbot-plugin-sdk` crate type `["cdylib", "rlib"]`.
2. Implement `#[plugin_fn]` functions; declare permissions in `klyntbot.plugin.json`.
3. Drop `plugin.wasm` + `klyntbot.plugin.json` into `~/.klyntbot/plugins/<id>/`.
4. **Restart Klyntbot** — no hot-reload.
5. **Don't grant `Agent` permission expecting `ask_user`** — it's a stub.

### Adding a `ComputerUseAction` variant

⚠️ Cross-cutting. Update `platform-input::ComputerUseAction` enum, then every `PlatformInput` implementation (only `MacInput` today). Even after that, **nothing dispatches to it** until the wiring layer exists. The variant alone changes nothing.

### Implementing `PlatformInput` / `PlatformCapture` for another OS

1. Create `crates/platform-<os>/` (template after `platform-macos`).
2. Implement both traits.
3. Document any unsupported variants — return `NotImplemented` cleanly.
4. **The wiring layer still doesn't exist** — your impl won't be called.

### Adding a host function

1. Add to the relevant namespace under `crates/plugin-runtime/src/host/`.
2. Check permission at the top: `ctx.permissions.contains(&PluginPermission::X)`.
3. Validate inputs (kind/length/format).
4. Return JSON via Extism — never panic.
5. Document in plugin-sdk and add SDK wrapper if user-facing.

---

## Open questions & debt

- **`agent_ask_user` is dead.** Granting `Agent` permission is half-broken. Either implement or remove the function from the namespace.
- **`cron_jobs` parsed but not scheduled.** Manifest field exists, executor missing.
- **Plugin SDK `db_query` no-op** is misleading dead code.
- **No hot-reload for plugins** — restart-only.
- **Table sandboxing is keyword-splitting**, not SQL parsing. Sufficient for the common case; not a formal sandbox.
- **Computer Use is unwired.** Platform infrastructure works; no agent/Tauri/MCP surface routes to it. Spec referenced is vapor.
- **AX frame y-coordinates are in AppKit space**, not Quartz. Y-flip is a Phase 4 TODO. Significant correctness gotcha for future Computer Use.
- **NSWorkspace sleep/wake observers stubbed.** Lifecycle only detects idle.
- **Speech via `say` CLI**, not `AVSpeechSynthesizer`. CLAUDE.md and earlier docs claim AVSpeech — wrong.
- **DnD toggle requires user-created Shortcut.** Brittle.
- **`MacInput::Screenshot` + `Zoom` return `NotImplemented`**; **`MacCapture::capture_window` + `get_active_window` return `NotImplemented`**.
- **Retina scale hardcoded 2.0** — should use `NSScreen.backingScaleFactor`.
- **Browser list includes Zen** — good coverage of modern Firefox forks, but no auto-detection mechanism if user adds a new browser.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs), #5 (doc drift), #7 (anomalies) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `DomainEventBus` carries `PluginEvent`
- [`02-storage.md`](./02-storage.md) — plugin tables sandboxed via `plugin_{id}_*` prefix
- [`07-tools-framework.md`](./07-tools-framework.md) — plugins register via `FeaturePackage`
- [`08-assistant-features.md`](./08-assistant-features.md) — voice-engine speech wraps `platform-macos::speech`
- [`10-sandboxing-security.md`](./10-sandboxing-security.md) — sandboxing approach
- [`13-desktop-frontend.md`](./13-desktop-frontend.md) — lifecycle observers consumed by desktop
