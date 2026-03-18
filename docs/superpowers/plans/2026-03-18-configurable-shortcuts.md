# Configurable Global Shortcuts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to customize the three global keyboard shortcuts (Launcher, Tray, Quick Capture) via a live Settings UI with instant hot-swap — no app restart required.

**Architecture:** Add `ShortcutsConfig` to the config schema, move shortcut registration from plugin builder to setup hook (config-driven), expose `shortcuts_get`/`shortcuts_update` Tauri commands for live re-registration, and add a key recorder widget in GeneralSettings.

**Tech Stack:** Rust (Tauri 2, `tauri-plugin-global-shortcut` v2, `global_hotkey` crate), TypeScript/React (Vite, Tailwind v4 CSS tokens)

**Spec:** `docs/superpowers/specs/2026-03-18-configurable-shortcuts-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `crates/config/src/schema/shortcuts.rs` | `ShortcutsConfig` struct with defaults |
| Create | `crates/desktop/src/shortcuts.rs` | Shared `register_shortcuts()` function |
| Create | `crates/desktop/src/commands/shortcuts.rs` | `shortcuts_get`, `shortcuts_update` Tauri commands + dev dispatch |
| Create | `desktop-ui/src/shared/ui/ShortcutRecorder.tsx` | Key recorder widget component |
| Modify | `crates/config/src/schema/mod.rs` | Add `mod shortcuts` + re-export |
| Modify | `crates/config/src/schema/core.rs` | Add `shortcuts: ShortcutsConfig` field to `Config` |
| Modify | `crates/desktop/src/main.rs` | Remove hardcoded shortcuts, register in setup, add commands to invoke_handler |
| Modify | `crates/desktop/src/commands/mod.rs` | Add `pub mod shortcuts;` |
| Modify | `crates/desktop/src/dev_server/dispatch.rs` | Add shortcuts dispatch chain entry |
| Modify | `crates/desktop/src/dev_server/mod.rs` | Add `commands::shortcuts::DEV_COMMANDS` to parity test |
| Modify | `desktop-ui/src/features/settings/pages/GeneralSettings.tsx` | Add Keyboard Shortcuts card |

---

### Task 1: Config Schema — `ShortcutsConfig`

**Files:**
- Create: `crates/config/src/schema/shortcuts.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Write the test**

Add to the existing test module at the bottom of `crates/config/src/schema/mod.rs`:

```rust
#[test]
fn test_shortcuts_config_default() {
    let config = Config::default();
    assert_eq!(config.shortcuts.launcher, "alt+space");
    assert_eq!(config.shortcuts.tray, "alt+shift+space");
    assert_eq!(config.shortcuts.quick_capture, "super+shift+c");
}

#[test]
fn test_shortcuts_config_serde_roundtrip() {
    let json = r#"{"shortcuts": {"launcher": "ctrl+space", "tray": "ctrl+shift+space", "quickCapture": "super+shift+v"}}"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.shortcuts.launcher, "ctrl+space");
    assert_eq!(config.shortcuts.tray, "ctrl+shift+space");
    assert_eq!(config.shortcuts.quick_capture, "super+shift+v");

    let serialized = serde_json::to_string(&config).unwrap();
    let loaded: Config = serde_json::from_str(&serialized).unwrap();
    assert_eq!(loaded.shortcuts.launcher, "ctrl+space");
}

#[test]
fn test_shortcuts_config_camel_case() {
    let config = Config::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("quickCapture"));
}

#[test]
fn test_config_without_shortcuts_deserializes() {
    let json = r#"{"agents": {"defaults": {"workspace": "~/.klyntbot/workspace", "model": "anthropic/claude-opus-4-5", "maxTokens": 8192, "temperature": 0.7, "maxToolIterations": 20}}}"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.shortcuts.launcher, "alt+space");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p config -E 'test(shortcuts)'`
Expected: FAIL — `shortcuts` field doesn't exist on `Config`

- [ ] **Step 3: Create the shortcuts schema file**

Create `crates/config/src/schema/shortcuts.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutsConfig {
    pub launcher: String,
    pub tray: String,
    pub quick_capture: String,
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

- [ ] **Step 4: Wire into mod.rs and core.rs**

In `crates/config/src/schema/mod.rs`, add `mod shortcuts;` after the `scenario` line and `pub use self::shortcuts::*;` after the `scenario` re-export.

In `crates/config/src/schema/core.rs`, add the import:
```rust
use super::shortcuts::ShortcutsConfig;
```

And add the field to `Config` struct (after `scenario`):
```rust
    /// Global keyboard shortcuts for Launcher, Tray, and Quick Capture.
    #[serde(default)]
    pub shortcuts: ShortcutsConfig,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(shortcuts)'`
Expected: All 4 tests PASS

- [ ] **Step 6: Run full config tests to check for regressions**

Run: `cargo nextest run -p config`
Expected: All PASS (new `shortcuts` field has `#[serde(default)]` so existing tests with partial JSON still work)

- [ ] **Step 7: Commit**

```bash
git add crates/config/src/schema/shortcuts.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add ShortcutsConfig schema for customizable global shortcuts"
```

---

### Task 2: Shared Registration Function — `register_shortcuts()`

**Files:**
- Create: `crates/desktop/src/shortcuts.rs`
- Modify: `crates/desktop/src/main.rs` (add `mod shortcuts;`)

- [ ] **Step 1: Create `crates/desktop/src/shortcuts.rs`**

```rust
//! Shared global shortcut registration logic.
//!
//! Called at startup (from `setup` hook) and at runtime (from `shortcuts_update` command).
//! Uses `tauri-plugin-global-shortcut` runtime API for register/unregister.

use config::ShortcutsConfig;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::commands::window::{WINDOW_LAUNCHER, WINDOW_QUICK_CAPTURE, WINDOW_TRAY};
use crate::focus_timer;

/// Register all three global shortcuts from config, mapping each to its window toggle.
///
/// Unregisters all existing shortcuts first (clean slate). If any shortcut string
/// is invalid or the OS rejects registration, returns an error describing the failure.
pub fn register_shortcuts(app: &AppHandle, config: &ShortcutsConfig) -> Result<(), String> {
    let manager = app.global_shortcut();

    // Unregister all existing shortcuts (idempotent — safe if none registered).
    manager
        .unregister_all()
        .map_err(|e| format!("failed to unregister existing shortcuts: {e}"))?;

    // Define the three shortcut → window mappings.
    let mappings: [(&str, &'static str); 3] = [
        (&config.launcher, WINDOW_LAUNCHER),
        (&config.tray, WINDOW_TRAY),
        (&config.quick_capture, WINDOW_QUICK_CAPTURE),
    ];

    for (shortcut_str, window_label) in mappings {
        let shortcut = shortcut_str
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| format!("invalid shortcut '{shortcut_str}': {e}"))?;

        let app_clone = app.clone();
        manager
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                toggle_window(&app_clone, window_label);
            })
            .map_err(|e| format!("failed to register shortcut '{shortcut_str}': {e}"))?;
    }

    Ok(())
}

/// Toggle a window's visibility. Tray uses `focus_timer::open_tray_window` for
/// correct positioning; others use center + show + focus.
fn toggle_window(app: &AppHandle, window_label: &str) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(window_label) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else if window_label == WINDOW_TRAY {
            focus_timer::open_tray_window(app);
        } else {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}
```

- [ ] **Step 2: Add `mod shortcuts;` to `crates/desktop/src/main.rs`**

Add after `mod tray_countdown;` (line 10):
```rust
mod shortcuts;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p desktop`
Expected: Compiles (the function is defined but not yet called)

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/shortcuts.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add shared register_shortcuts() function"
```

---

### Task 3: Move Startup Registration to Setup Hook

**Files:**
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Replace the plugin builder**

In `main.rs`, find the `.plugin(tauri_plugin_global_shortcut::Builder::new()` block (the one with `.with_shortcuts(["alt+space", ...])` and the large `.with_handler()` closure). Replace the entire block:

**Remove** from `.plugin(tauri_plugin_global_shortcut::Builder::new()` through the matching `.build(), )`.

**Replace with:**
```rust
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
```

- [ ] **Step 2: Remove unused imports**

Find and remove this line:
```rust
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
```

This is no longer needed in main.rs — these types are used in `shortcuts.rs` now.

- [ ] **Step 3: Add shortcut registration in the setup hook**

In the `setup` closure, find the line `app.manage(core);`. After it, add:

```rust
            // Register global shortcuts from config (or defaults if config invalid).
            {
                let core_ref: &Arc<AppCore> = app.state::<Arc<AppCore>>().inner();
                let shortcuts_config = tauri::async_runtime::block_on(async {
                    core_ref.config.read().await.shortcuts.clone()
                });
                if let Err(e) = shortcuts::register_shortcuts(app.handle(), &shortcuts_config) {
                    tracing::warn!(
                        "Failed to register shortcuts from config, falling back to defaults: {e}"
                    );
                    let defaults = config::ShortcutsConfig::default();
                    if let Err(e2) = shortcuts::register_shortcuts(app.handle(), &defaults) {
                        tracing::error!("Failed to register default shortcuts: {e2}");
                    }
                }
            }
```

- [ ] **Step 4: Verify it compiles and the app starts**

Run: `cargo build -p desktop`
Expected: Compiles without warnings

Run: `cargo tauri dev` (with Vite running in another terminal)
Expected: App starts. Press ⌥Space — launcher toggles. Press ⌥⇧Space — tray toggles. Press ⌘⇧C — quick capture toggles.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): move shortcut registration to setup hook (config-driven)"
```

---

### Task 4: Tauri Commands — `shortcuts_get` and `shortcuts_update`

**Files:**
- Create: `crates/desktop/src/commands/shortcuts.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs` (invoke_handler)
- Modify: `crates/desktop/src/dev_server/dispatch.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs` (parity test)

- [ ] **Step 1: Create `crates/desktop/src/commands/shortcuts.rs`**

```rust
use std::sync::Arc;

use app_core::AppCore;
use config::ShortcutsConfig;
use desktop_shared::errors::ApiError;
use tauri::{AppHandle, State};

use crate::shortcuts::register_shortcuts;

#[tauri::command]
pub async fn shortcuts_get(state: State<'_, Arc<AppCore>>) -> Result<ShortcutsConfig, ApiError> {
    let cfg = state.config.read().await;
    Ok(cfg.shortcuts.clone())
}

#[tauri::command]
pub async fn shortcuts_update(
    app: AppHandle,
    state: State<'_, Arc<AppCore>>,
    launcher: String,
    tray: String,
    quick_capture: String,
) -> Result<ShortcutsConfig, ApiError> {
    let shortcuts = ShortcutsConfig {
        launcher,
        tray,
        quick_capture,
    };

    // 1. Pre-validate all shortcut strings parse.
    validate_shortcut_strings(&shortcuts)?;

    // 2. Snapshot current config for rollback.
    let old_shortcuts = state.config.read().await.shortcuts.clone();

    // 3. Register with the OS.
    if let Err(e) = register_shortcuts(&app, &shortcuts) {
        // Rollback — restore previous shortcuts.
        let _ = register_shortcuts(&app, &old_shortcuts);
        return Err(ApiError::new("SHORTCUT_REGISTRATION_FAILED", e));
    }

    // 4. Persist to config.json (only after OS registration succeeds).
    let mut cfg = state.config.write().await;
    cfg.shortcuts = shortcuts.clone();
    config::save(&cfg)
        .await
        .map_err(|e| ApiError::new("CONFIG_SAVE_FAILED", e.to_string()))?;

    Ok(shortcuts)
}

/// Validate that all shortcut strings parse as valid Tauri shortcuts.
fn validate_shortcut_strings(shortcuts: &ShortcutsConfig) -> Result<(), ApiError> {
    for (name, value) in [
        ("launcher", &shortcuts.launcher),
        ("tray", &shortcuts.tray),
        ("quickCapture", &shortcuts.quick_capture),
    ] {
        value
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| {
                ApiError::new("VALIDATION", format!("invalid shortcut for {name}: '{value}' — {e}"))
            })?;
    }
    Ok(())
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["shortcuts_get", "shortcuts_update"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "shortcuts_get" => {
            let cfg = core.config.read().await;
            dev::val(Ok::<_, ApiError>(cfg.shortcuts.clone()))
        }
        "shortcuts_update" => {
            // Dev mode: validate strings + persist config, but skip OS registration.
            // Frontend sends flat params: { launcher, tray, quickCapture }.
            let shortcuts = ShortcutsConfig {
                launcher: match dev::get_str(body, "launcher") {
                    Ok(s) => s,
                    Err(e) => return Some(Err(e)),
                },
                tray: match dev::get_str(body, "tray") {
                    Ok(s) => s,
                    Err(e) => return Some(Err(e)),
                },
                quick_capture: match dev::get_str(body, "quickCapture") {
                    Ok(s) => s,
                    Err(e) => return Some(Err(e)),
                },
            };
            if let Err(e) = validate_shortcut_strings(&shortcuts) {
                return Some(Err(e));
            }
            let mut cfg = core.config.write().await;
            cfg.shortcuts = shortcuts.clone();
            match config::save(&cfg).await {
                Ok(()) => dev::val(Ok::<_, ApiError>(shortcuts)),
                Err(e) => Some(Err(ApiError::new("CONFIG_SAVE_FAILED", e.to_string()))),
            }
        }
        _ => return None,
    })
}
```

- [ ] **Step 2: Add module to `crates/desktop/src/commands/mod.rs`**

Add after `pub mod settings;` (line 24):
```rust
pub mod shortcuts;
```

- [ ] **Step 3: Register commands in invoke_handler in `main.rs`**

In the `invoke_handler` block (around line 541, after the settings commands), add:
```rust
            commands::shortcuts::shortcuts_get,
            commands::shortcuts::shortcuts_update,
```

- [ ] **Step 4: Add dispatch to `crates/desktop/src/dev_server/dispatch.rs`**

After the launcher dispatch block (around line 131), add:
```rust
    if let Some(r) = commands::shortcuts::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
```

- [ ] **Step 5: Add DEV_COMMANDS to parity test in `crates/desktop/src/dev_server/mod.rs`**

In the `dev_command_names()` function, find the `modules` array. Add after the last entry (`commands::launcher::DEV_COMMANDS,`):
```rust
            commands::shortcuts::DEV_COMMANDS,
```

- [ ] **Step 6: Run the parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server)'`
Expected: Both `dev_server_covers_all_tauri_commands` and `dev_server_has_no_orphan_commands` PASS

- [ ] **Step 7: Run full desktop build**

Run: `cargo build -p desktop`
Expected: Compiles with 0 clippy warnings

- [ ] **Step 8: Commit**

```bash
git add crates/desktop/src/commands/shortcuts.rs crates/desktop/src/commands/mod.rs crates/desktop/src/main.rs crates/desktop/src/dev_server/dispatch.rs crates/desktop/src/dev_server/mod.rs
git commit -m "feat(desktop): add shortcuts_get and shortcuts_update Tauri commands"
```

---

### Task 5: Frontend — ShortcutRecorder Component

**Files:**
- Create: `desktop-ui/src/shared/ui/ShortcutRecorder.tsx`

- [ ] **Step 1: Create the ShortcutRecorder component**

```tsx
import { RotateCcw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

/** Maps browser event.code to Tauri-compatible key string. */
function codeToTauriKey(code: string): string | null {
  // Filter out bare modifier keys — these are captured via boolean flags
  if (/^(Meta|Shift|Control|Alt)(Left|Right)$/.test(code)) return null;
  if (code.startsWith("Key")) return code.slice(3).toLowerCase(); // KeyC → c
  if (code.startsWith("Digit")) return code.slice(5); // Digit0 → 0
  // Map common special keys
  const map: Record<string, string> = {
    Space: "space",
    Enter: "enter",
    Escape: "escape",
    Backspace: "backspace",
    Tab: "tab",
    ArrowUp: "up",
    ArrowDown: "down",
    ArrowLeft: "left",
    ArrowRight: "right",
    Delete: "delete",
    Home: "home",
    End: "end",
    PageUp: "pageup",
    PageDown: "pagedown",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backquote: "`",
  };
  return map[code] ?? code.toLowerCase();
}

/** Build Tauri shortcut string from modifier flags + key. */
function buildShortcutString(e: KeyboardEvent): string | null {
  const key = codeToTauriKey(e.code);
  if (!key) return null; // Pure modifier press

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.metaKey) parts.push("super");
  if (e.shiftKey) parts.push("shift");

  // Require at least one modifier
  if (parts.length === 0) return null;

  parts.push(key);
  return parts.join("+");
}

/** Maps Tauri shortcut string to macOS display symbols. */
function displayShortcut(shortcut: string): string {
  return shortcut
    .split("+")
    .map((part) => {
      switch (part.toLowerCase()) {
        case "super": return "⌘";
        case "alt": return "⌥";
        case "shift": return "⇧";
        case "ctrl": return "⌃";
        case "space": return "Space";
        default: return part.toUpperCase();
      }
    })
    .join("");
}

interface ShortcutRecorderProps {
  value: string;
  defaultValue: string;
  onChange: (value: string) => void;
  error?: string;
}

export function ShortcutRecorder({
  value,
  defaultValue,
  onChange,
  error,
}: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const ref = useRef<HTMLButtonElement>(null);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // Escape cancels recording
      if (e.code === "Escape") {
        setRecording(false);
        return;
      }

      const shortcut = buildShortcutString(e);
      if (shortcut) {
        onChange(shortcut);
        setRecording(false);
      }
    },
    [onChange],
  );

  useEffect(() => {
    if (recording) {
      window.addEventListener("keydown", handleKeyDown, true);
      return () => window.removeEventListener("keydown", handleKeyDown, true);
    }
  }, [recording, handleKeyDown]);

  // Close recording on blur
  useEffect(() => {
    if (recording) {
      const el = ref.current;
      const handleBlur = () => setRecording(false);
      el?.addEventListener("blur", handleBlur);
      return () => el?.removeEventListener("blur", handleBlur);
    }
  }, [recording]);

  const isDefault = value === defaultValue;

  return (
    <div className="flex items-center gap-2">
      <button
        ref={ref}
        type="button"
        onClick={() => setRecording(true)}
        className={`flex-1 px-3 py-1.5 text-[13px] text-left rounded-lg border transition-all ${
          recording
            ? "border-brand bg-accent animate-pulse text-brand"
            : error
              ? "border-red-500/50 bg-accent text-foreground"
              : "border-border bg-accent text-foreground hover:border-brand/30"
        }`}
      >
        {recording ? "Press shortcut..." : displayShortcut(value)}
      </button>
      {!isDefault && (
        <button
          type="button"
          onClick={() => onChange(defaultValue)}
          title="Reset to default"
          className="p-1.5 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <RotateCcw className="w-3.5 h-3.5" />
        </button>
      )}
      {error && <p className="text-[11px] text-red-400 mt-0.5">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run build`
Expected: Builds successfully (component is defined but not imported anywhere yet)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/ui/ShortcutRecorder.tsx
git commit -m "feat(ui): add ShortcutRecorder key-capture component"
```

---

### Task 6: Frontend — Keyboard Shortcuts Card in GeneralSettings

**Files:**
- Modify: `desktop-ui/src/features/settings/pages/GeneralSettings.tsx`

- [ ] **Step 1: Add the Keyboard Shortcuts card**

At the top of `GeneralSettings.tsx`, add imports:
```tsx
import { ShortcutRecorder } from "@shared/ui/ShortcutRecorder";
```

Add a constant outside the component (before the `export function GeneralSettings()` line):

```tsx
const SHORTCUT_DEFAULTS = {
  launcher: "alt+space",
  tray: "alt+shift+space",
  quickCapture: "super+shift+c",
};
```

Add state and query hooks inside the `GeneralSettings` component (after the existing `useQuery` calls, around line 36):

```tsx
  // ── Shortcuts ─────────────────────────────────────

  const { data: shortcutsConfig, refetch: refetchShortcuts } = useQuery<typeof SHORTCUT_DEFAULTS>(
    "shortcuts_get",
    undefined,
    SHORTCUT_DEFAULTS,
  );

  const [shortcutEdits, setShortcutEdits] = useState<Record<string, string>>({});
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [savingShortcuts, setSavingShortcuts] = useState(false);

  const currentShortcuts = {
    launcher: shortcutEdits.launcher ?? shortcutsConfig.launcher,
    tray: shortcutEdits.tray ?? shortcutsConfig.tray,
    quickCapture: shortcutEdits.quickCapture ?? shortcutsConfig.quickCapture,
  };

  const hasShortcutChanges = Object.keys(shortcutEdits).length > 0;

  // Check for duplicates among the three shortcuts
  const values = Object.values(currentShortcuts);
  const duplicateShortcut = values.find((v, i) => values.indexOf(v) !== i) ?? null;

  const handleSaveShortcuts = async () => {
    setSavingShortcuts(true);
    setShortcutError(null);
    try {
      await ipc("shortcuts_update", currentShortcuts);
      refetchShortcuts();
      setShortcutEdits({});
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setShortcutError(msg);
    } finally {
      setSavingShortcuts(false);
    }
  };
```

Insert the Keyboard Shortcuts card JSX between the System card closing `</div>` and the Agent defaults card opening `<div>` (between line 99 and line 101 of the original file):

```tsx
        <div className="bg-card rounded-lg border border-border p-4">
          <h3 className="text-[13px] font-medium text-muted-foreground mb-3">
            Keyboard Shortcuts
          </h3>
          <div className="space-y-3">
            {([
              ["launcher", "Launcher"],
              ["tray", "Tray popup"],
              ["quickCapture", "Quick capture"],
            ] as const).map(([key, label]) => (
              <div key={key} className="flex items-center justify-between gap-4">
                <span className="text-[12px] text-muted-foreground w-28 shrink-0">
                  {label}
                </span>
                <ShortcutRecorder
                  value={currentShortcuts[key]}
                  defaultValue={SHORTCUT_DEFAULTS[key]}
                  onChange={(val) =>
                    setShortcutEdits((prev) => ({ ...prev, [key]: val }))
                  }
                  error={
                    duplicateShortcut && currentShortcuts[key] === duplicateShortcut
                      ? "Duplicate shortcut"
                      : undefined
                  }
                />
              </div>
            ))}

            {shortcutError && (
              <p className="text-[12px] text-red-400">{shortcutError}</p>
            )}

            {hasShortcutChanges && (
              <div className="flex justify-end">
                <button
                  type="button"
                  onClick={handleSaveShortcuts}
                  disabled={savingShortcuts || duplicateShortcut !== null}
                  className="px-4 py-1.5 text-[12px] font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
                >
                  {savingShortcuts ? "Saving..." : "Save changes"}
                </button>
              </div>
            )}
          </div>
        </div>
```

- [ ] **Step 2: Verify the frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Builds successfully

- [ ] **Step 3: Run Biome lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors (auto-fixes any formatting issues)

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/settings/pages/GeneralSettings.tsx
git commit -m "feat(settings): add Keyboard Shortcuts card with live key recorder"
```

---

### Task 7: End-to-End Verification

**Files:** None (testing only)

- [ ] **Step 1: Run all Rust tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Run cargo fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All PASS

- [ ] **Step 5: Manual test — full flow**

1. Run `cargo tauri dev` (with Vite dev server running)
2. Press ⌥Space — launcher should toggle ✓
3. Press ⌥⇧Space — tray should toggle ✓
4. Press ⌘⇧C — quick capture should toggle ✓
5. Navigate to Settings → General
6. See "Keyboard Shortcuts" card with three recorders showing current shortcuts
7. Click the Launcher recorder → press ⌃Space → see "⌃Space" displayed
8. Click "Save changes" → shortcut updates instantly
9. Press ⌃Space → launcher should toggle ✓
10. Press ⌥Space → should NOT toggle launcher (old shortcut unregistered) ✓
11. Click the ⟲ reset button → shortcut reverts to "⌥Space"
12. Save → back to default

- [ ] **Step 6: Manual test — error cases**

1. Set two shortcuts to the same combo → see "Duplicate shortcut" warning, Save disabled
2. Check `~/.klyntbot-dev/config.json` → see `"shortcuts"` section with saved values

- [ ] **Step 7: Commit all remaining changes (if any)**

```bash
git add -A
git commit -m "feat(shortcuts): configurable global shortcuts with live Settings UI"
```
