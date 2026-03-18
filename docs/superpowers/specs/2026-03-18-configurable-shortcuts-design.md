# Configurable Global Shortcuts

**Date:** 2026-03-18
**Status:** Approved

## Problem

The three global shortcuts (Launcher, Tray, Quick Capture) are hardcoded in `crates/desktop/src/main.rs` as string literals passed to `tauri_plugin_global_shortcut::Builder::with_shortcuts()`. Users cannot customize them. The shortcuts are also registered before the config loads (plugin init happens before the `setup` hook), making config-driven registration impossible without restructuring.

## Solution

Add a `ShortcutsConfig` section to `config.json`, move shortcut registration into the `setup` hook (after config loads), and provide a dedicated Tauri command for live hot-swapping shortcuts at runtime. A key recorder UI in Settings lets users press a key combo to set each shortcut, with instant feedback and rollback on failure.

## Design

### 1. Config Schema

New file: `crates/config/src/schema/shortcuts.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutsConfig {
    pub launcher: String,       // default: "alt+space"
    pub tray: String,           // default: "alt+shift+space"
    pub quick_capture: String,  // default: "super+shift+c"
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            launcher: "alt+space".to_string(),
            tray: "alt+shift+space".to_string(),
            quick_capture: "super+shift+c".to_string(),
        }
    }
}
```

Added to root `Config` struct as `pub shortcuts: ShortcutsConfig` with `#[serde(default)]`. Existing configs without this section get defaults automatically.

Resulting `config.json`:
```json
{
  "shortcuts": {
    "launcher": "alt+space",
    "tray": "alt+shift+space",
    "quickCapture": "super+shift+c"
  }
}
```

Values are Tauri shortcut strings — human-readable, directly editable by power users. Validation happens at registration time, not deserialization.

### 2. Shared Registration Function

New file: `crates/desktop/src/shortcuts.rs`

```rust
pub fn register_shortcuts(app: &AppHandle, config: &ShortcutsConfig) -> Result<(), String>
```

Steps:
1. Pre-validate all shortcut strings via `s.parse::<Shortcut>()` (`FromStr` impl from `global_hotkey::hotkey::HotKey`). If any fail, return error before touching OS state.
2. `app.global_shortcut().unregister_all()` — clean slate (clears OS-level CGEventTap registrations; safe even if no shortcuts are currently registered).
3. Register each shortcut with its own closure via `app.global_shortcut().on_shortcut(shortcut, handler)`. Each closure captures its target `window_label: &'static str` and contains the toggle logic (show/hide/focus). Three separate `on_shortcut` calls — not a single shared handler with `matches()` dispatch like the current code.
4. On failure at step 3, return error string describing which shortcut failed and why.

**Handler architecture change:** The current `main.rs` uses a single `with_handler` closure that dispatches via `shortcut.matches(Modifiers::*, Code::*)`. This doesn't compose with runtime `on_shortcut` registration. Instead, each shortcut gets its own closure that captures the target window label. A helper function `make_toggle_handler(window_label)` returns the closure:

```rust
fn make_toggle_handler(app: &AppHandle, window_label: &'static str) -> impl Fn() {
    let app = app.clone();
    move || {
        if let Some(window) = app.get_webview_window(window_label) {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                // tray uses focus_timer::open_tray_window, others use center+show+focus
                if window_label == WINDOW_TRAY {
                    focus_timer::open_tray_window(&app);
                } else {
                    let _ = window.center();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    }
}
```

**Thread affinity note:** The `global-hotkey` crate's `GlobalHotKeyManager` uses `run_main_thread!` on macOS. Tauri's `on_shortcut` handles this internally, but the unregister/register calls during `shortcuts_update` execute on the main thread. This is acceptable for three shortcuts but should not be called in a tight loop.

Called from both startup and the live-update command. No diffing — full re-registration for three shortcuts is negligible overhead.

### 3. Startup Changes

`crates/desktop/src/main.rs`:

**Before:** Shortcuts registered in plugin builder (before config loads).

**After:**
- Plugin builder: `tauri_plugin_global_shortcut::Builder::new().build()` — no shortcuts registered
- `setup` hook: After `AppCore` init, read `config.shortcuts` and call `register_shortcuts(app_handle, &config.shortcuts)`. On failure (e.g., user manually edited config.json with invalid value), log warning and fall back to `ShortcutsConfig::default()`. The app must never crash at startup due to a bad shortcut config.

### 4. Live Update Commands

New file: `crates/desktop/src/commands/shortcuts.rs`

Two commands:

**`shortcuts_get`** — Returns current `ShortcutsConfig` from config.

**`shortcuts_update`** — Atomic validate → register → persist with rollback:
1. Pre-validate all shortcut strings parse via `s.parse::<Shortcut>()` — fail fast before touching OS state
2. Snapshot current `ShortcutsConfig` for rollback
3. Call `register_shortcuts()` with new values
4. On OS registration failure → attempt rollback by calling `register_shortcuts()` with old config. If rollback also fails (unlikely but possible), log error and leave shortcuts in degraded state — user can restart to restore defaults
5. On success → persist to `config.json`, return saved config

Lives in `desktop` crate (not `app-core`) because it needs `AppHandle` for `GlobalShortcutExt`. Same pattern as tray icon and focus timer.

### 5. Dev Server Dispatch

The shortcuts module exports `DEV_COMMANDS` and a `dispatch_dev` function per the existing convention.

- **`shortcuts_get`** — works normally in browser-dev mode (reads from config).
- **`shortcuts_update`** — in browser-dev mode, still validates shortcut strings via `s.parse::<Shortcut>()` and persists to config, but skips OS registration (global shortcuts are meaningless in a browser tab). This ensures the config file always contains valid shortcut strings even when testing in the browser.

If `dev_server/mod.rs` has a `TAURI_ONLY` exemption list, `shortcuts_update` does NOT need to be in it — it has a functional dev-mode stub. Both commands appear in `DEV_COMMANDS`.

### 6. Frontend — Key Recorder UI

**ShortcutRecorder component:** `desktop-ui/src/shared/ui/ShortcutRecorder.tsx`

Reusable widget with three states:
- **Display** — shows current shortcut with macOS symbols (⌘⇧C). Clickable.
- **Recording** — pulsing border, "Press shortcut..." text. Captures next keydown with modifiers.
- **Error** — red border + error message from backend.

Key translation:
- **Modifiers** from boolean flags: `event.metaKey` → `"super"` (⌘), `event.altKey` → `"alt"` (⌥), `event.shiftKey` → `"shift"` (⇧), `event.ctrlKey` → `"ctrl"` (⌃)
- **Key** from `event.code` (physical key code, locale-independent). Important mapping rules:
  - Filter out pure modifier keypresses (`MetaLeft`, `ShiftLeft`, `ControlLeft`, `AltLeft` and `*Right` variants) — these are captured via the boolean flags above, not as the key itself
  - Strip `"Key"` prefix: `"KeyC"` → `"c"`, `"KeyA"` → `"a"`
  - Strip `"Digit"` prefix: `"Digit0"` → `"0"`
  - Pass through special keys: `"Space"` → `"space"`, `"ArrowUp"` → `"ArrowUp"`, `"Enter"` → `"Enter"` (the `global_hotkey` parser is case-insensitive)
  - Construct final string: `modifiers.join("+") + "+" + key` (e.g., `"alt+space"`, `"super+shift+c"`)

Uses `event.code` (not `event.key`) because Tauri/`global_hotkey` shortcut strings map to physical key codes, not locale-dependent character values.

Frontend validations:
- At least one modifier required (reject bare keys)
- Duplicate detection across the three fields (highlight both with warning)

**Placement:** New "Keyboard Shortcuts" card in `GeneralSettings.tsx`, after "System" card, before "Agent defaults".

Interactions:
- `[⟲]` button per row resets to default
- Save calls `ipc("shortcuts_update", { launcher, tray, quickCapture })`
- On error: inline error message, field reverts

## Files

| Action | File | Description |
|--------|------|-------------|
| Create | `crates/config/src/schema/shortcuts.rs` | `ShortcutsConfig` struct |
| Create | `crates/desktop/src/shortcuts.rs` | `register_shortcuts()` shared function |
| Create | `crates/desktop/src/commands/shortcuts.rs` | `shortcuts_get`, `shortcuts_update` commands |
| Create | `desktop-ui/src/shared/ui/ShortcutRecorder.tsx` | Key recorder widget |
| Modify | `crates/config/src/schema/core.rs` | Add `shortcuts: ShortcutsConfig` to `Config` |
| Modify | `crates/config/src/schema/mod.rs` | Add `mod shortcuts` + re-export |
| Modify | `crates/desktop/src/main.rs` | Remove hardcoded shortcuts, register in setup |
| Modify | `crates/desktop/src/commands/mod.rs` | Add shortcuts module |
| Modify | `crates/desktop/src/dev_server/mod.rs` | Add shortcuts dispatch |
| Modify | `desktop-ui/src/features/settings/pages/GeneralSettings.tsx` | Add shortcuts card |

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid shortcut string (e.g., `"banana"`) | `VALIDATION` error, nothing changes |
| OS rejects shortcut (conflict with another app) | `SHORTCUT_REGISTRATION_FAILED` error, rollback to previous |
| Config save fails (disk error) | Shortcuts active in memory, config not saved; next restart uses previous config |
| Missing shortcuts section in config.json | Defaults applied via `#[serde(default)]` |
| Invalid shortcut in config.json at startup | Log warning, fall back to `ShortcutsConfig::default()` — app never crashes on bad config |
| Rollback also fails during live update | Log error, return `SHORTCUT_REGISTRATION_FAILED` — user can restart to restore defaults |

## Non-Goals

- Keyboard shortcuts for in-app actions (only global OS-level shortcuts)
- Per-platform shortcut defaults (same defaults on all platforms for now)
- Shortcut profiles or presets
