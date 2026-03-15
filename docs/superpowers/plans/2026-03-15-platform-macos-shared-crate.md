# Platform-macOS Shared Crate Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract duplicated macOS-specific code from `feature-launcher` and `feature-productivity` into a shared `platform-macos` crate, unify browser knowledge, share the app icon cache, and implement working window management for the launcher.

**Architecture:** Create a new L0 crate `crates/platform-macos` that consolidates all native macOS FFI (NSWorkspace, AXUIElement, CoreGraphics, NSPasteboard) behind clean Rust APIs. Both `feature-launcher` (L4) and `feature-productivity` (L4) depend on it. No internal workspace crate deps — only external `objc2`, `core-graphics`, `core-foundation`, `base64`. All functions have `#[cfg(not(target_os = "macos"))]` stubs returning `None`/defaults so CI compiles on Linux.

**Tech Stack:** `objc2` 0.6, `objc2-app-kit` 0.3, `objc2-foundation` 0.3, `core-graphics` 0.24, `core-foundation` 0.10, `base64` 0.22, `tracing`

---

## File Structure

### New files (crates/platform-macos/)

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Crate manifest — macOS-only deps, no internal workspace deps |
| `src/lib.rs` | Re-exports all modules |
| `src/window.rs` | `WindowInfo` struct, `get_frontmost_window()`, `get_window_title_ax()`, `get_window_title_cg()`, `get_screen_frame()`, `set_window_frame()` |
| `src/input.rs` | `seconds_since_last_input()` |
| `src/apps.rs` | `running_applications()` via NSWorkspace, `AppIconCache` (extract+cache icons) |
| `src/browser.rs` | Unified browser registry (11 browsers), `is_browser()`, `get_browser_url()`, `chromium_profile_dir()`, `extract_site_name()` |
| `src/pasteboard.rs` | `pasteboard_change_count()`, `read_pasteboard_string()` |

### Modified files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Add `platform-macos` to members + workspace.dependencies |
| `crates/feature-productivity/Cargo.toml` | Add `platform-macos` dep, remove macOS-specific deps (objc2, core-graphics, core-foundation) |
| `crates/feature-productivity/src/tracker/macos.rs` | Replace implementations with calls to `platform_macos::*` |
| `crates/feature-productivity/src/tracker/categorizer/browser.rs` | Re-export browser constants from `platform_macos::browser` |
| `crates/feature-launcher/Cargo.toml` | Add `platform-macos` dep, remove objc2 deps |
| `crates/feature-launcher/src/clipboard/monitor.rs` | Replace `get_frontmost_app_name()` with `platform_macos::window::get_frontmost_window()` |
| `crates/feature-launcher/src/window_mgmt/accessibility.rs` | Replace stub with `platform_macos::window::*` calls |
| `crates/feature-launcher/src/search/running_apps.rs` | Replace JXA subprocess with `platform_macos::apps::running_applications()` |
| `crates/feature-launcher/src/search/mod.rs` | Replace `chromium_profile_dir()` with `platform_macos::browser::chromium_profile_dir()` |
| `crates/feature-launcher/src/search/bookmarks.rs` | Update import for `chromium_profile_dir` |
| `crates/feature-launcher/src/search/browser_history.rs` | Update import for `chromium_profile_dir` |

---

## Chunk 1: Create platform-macos crate with window + input modules

### Task 1: Scaffold the crate and register in workspace

**Files:**
- Create: `crates/platform-macos/Cargo.toml`
- Create: `crates/platform-macos/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create the crate directory**

```bash
mkdir -p crates/platform-macos/src
```

- [ ] **Step 2: Write Cargo.toml**

Create `crates/platform-macos/Cargo.toml`:

```toml
[package]
name = "platform-macos"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
tracing.workspace = true
base64.workspace = true

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [
    "NSWorkspace",
    "NSRunningApplication",
    "NSPasteboard",
    "NSPasteboardType",
] }
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray"] }
core-graphics = "0.24"
core-foundation = "0.10"
```

- [ ] **Step 3: Write lib.rs**

Create `crates/platform-macos/src/lib.rs`:

```rust
pub mod apps;
pub mod browser;
pub mod input;
pub mod pasteboard;
pub mod window;
```

- [ ] **Step 4: Add to workspace root Cargo.toml**

In `Cargo.toml` at workspace root:
1. Add `"crates/platform-macos"` to the `members` array (after `"crates/feature-launcher"`)
2. Add `platform-macos = { path = "crates/platform-macos" }` to `[workspace.dependencies]` (after the `feature-launcher` entry)

- [ ] **Step 5: Create stub modules so it compiles**

Create placeholder files with just enough to compile:
- `src/window.rs`: empty file
- `src/input.rs`: empty file
- `src/apps.rs`: empty file
- `src/browser.rs`: empty file
- `src/pasteboard.rs`: empty file

- [ ] **Step 6: Verify it compiles**

```bash
cargo check -p platform-macos
```

Expected: success (empty crate, no errors)

- [ ] **Step 7: Commit**

```bash
git add crates/platform-macos/ Cargo.toml Cargo.lock
git commit -m "feat: scaffold platform-macos shared crate"
```

---

### Task 2: Implement window module (migrate from productivity)

**Files:**
- Create: `crates/platform-macos/src/window.rs`

The window module consolidates code currently in `crates/feature-productivity/src/tracker/macos.rs` lines 1–207 plus `crates/feature-launcher/src/window_mgmt/accessibility.rs`. It provides:
- `WindowInfo` struct (from productivity)
- `get_frontmost_window()` (from productivity)
- `get_window_title_ax()` (from productivity)
- `get_window_title_cg()` (from productivity)
- `get_screen_frame()` (new — replace launcher's stub)
- `set_window_frame()` (new — implement AX position/size setting for launcher's WindowManager)

- [ ] **Step 1: Write window.rs with WindowInfo + get_frontmost_window**

Copy the working implementation from `crates/feature-productivity/src/tracker/macos.rs` lines 1–207 into `crates/platform-macos/src/window.rs`. Key changes:
- Make `WindowInfo` and all public functions `pub`
- Make `get_window_title_ax` and `get_window_title_cg` private helpers (not part of public API)
- Return `Option<WindowInfo>` instead of `Result<Option<WindowInfo>>` (remove the `common::Result` dependency — this crate has no internal deps)
- Add `#[cfg(not(target_os = "macos"))]` stubs for all pub functions

```rust
/// Information about the currently focused window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub pid: i32,
}

/// Get the currently focused window's app, bundle ID, title, and PID.
///
/// Uses NSWorkspace for app info, then tries Accessibility API (AX)
/// for window title, falling back to CoreGraphics (CG).
#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Option<WindowInfo> {
    // ... (copy from productivity's macos.rs lines 22-50, change Result to Option)
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window() -> Option<WindowInfo> {
    None
}
```

Include the full `get_window_title_ax()` and `get_window_title_cg()` private functions exactly as they are in productivity.

- [ ] **Step 2: Add get_screen_frame**

Implement the currently-stubbed function using `CGMainDisplayID` + `CGDisplayBounds`:

```rust
/// Get the visible screen frame of the main display: (x, y, width, height).
#[cfg(target_os = "macos")]
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    use core_graphics::display::{CGDisplay, CGMainDisplayID};

    let display_id = unsafe { CGMainDisplayID() };
    let display = CGDisplay::new(display_id);
    let bounds = display.bounds();
    (bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height)
}

#[cfg(not(target_os = "macos"))]
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    (0.0, 0.0, 1920.0, 1080.0)
}
```

- [ ] **Step 3: Add set_window_frame for window management**

Implement the AX write operations the launcher needs:

```rust
/// Move and resize the focused window of the app with the given PID.
/// Uses AXUIElement to set position and size.
#[cfg(target_os = "macos")]
pub fn set_window_frame(pid: i32, x: f64, y: f64, w: f64, h: f64) -> bool {
    // 1. AXUIElementCreateApplication(pid)
    // 2. Get AXFocusedWindow
    // 3. Create AXValue from CGPoint(x,y) and set AXPosition
    // 4. Create AXValue from CGSize(w,h) and set AXSize
    // Returns true on success, false on failure
}

#[cfg(not(target_os = "macos"))]
pub fn set_window_frame(_pid: i32, _x: f64, _y: f64, _w: f64, _h: f64) -> bool {
    false
}
```

The macOS implementation:
```rust
#[cfg(target_os = "macos")]
pub fn set_window_frame(pid: i32, x: f64, y: f64, w: f64, h: f64) -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::ptr;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *mut std::ffi::c_void;
        fn AXUIElementCopyAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut *mut std::ffi::c_void,
        ) -> i32;
        fn AXUIElementSetAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *const std::ffi::c_void,
        ) -> i32;
        fn AXValueCreate(value_type: u32, value: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    const AX_ERROR_SUCCESS: i32 = 0;
    const AX_VALUE_TYPE_CGPOINT: u32 = 1;
    const AX_VALUE_TYPE_CGSIZE: u32 = 2;

    #[repr(C)]
    struct CGPoint { x: f64, y: f64 }
    #[repr(C)]
    struct CGSize { width: f64, height: f64 }

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return false;
        }

        // Get focused window
        let focused_attr = CFString::new("AXFocusedWindow");
        let mut focused_window: *mut std::ffi::c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            focused_attr.as_CFTypeRef() as *const _,
            &mut focused_window,
        );
        core_foundation::base::CFRelease(app_element as _);

        if err != AX_ERROR_SUCCESS || focused_window.is_null() {
            return false;
        }

        // Set position
        let point = CGPoint { x, y };
        let pos_value = AXValueCreate(AX_VALUE_TYPE_CGPOINT, &point as *const _ as *const _);
        if !pos_value.is_null() {
            let pos_attr = CFString::new("AXPosition");
            AXUIElementSetAttributeValue(
                focused_window,
                pos_attr.as_CFTypeRef() as *const _,
                pos_value as *const _,
            );
            core_foundation::base::CFRelease(pos_value as _);
        }

        // Set size
        let size = CGSize { width: w, height: h };
        let size_value = AXValueCreate(AX_VALUE_TYPE_CGSIZE, &size as *const _ as *const _);
        if !size_value.is_null() {
            let size_attr = CFString::new("AXSize");
            AXUIElementSetAttributeValue(
                focused_window,
                size_attr.as_CFTypeRef() as *const _,
                size_value as *const _,
            );
            core_foundation::base::CFRelease(size_value as _);
        }

        core_foundation::base::CFRelease(focused_window as _);
        true
    }
}
```

- [ ] **Step 4: Verify window module compiles**

```bash
cargo check -p platform-macos
```

- [ ] **Step 5: Commit**

```bash
git add crates/platform-macos/src/window.rs
git commit -m "feat(platform-macos): add window module with AX read/write + CG fallback"
```

---

### Task 3: Implement input module

**Files:**
- Create: `crates/platform-macos/src/input.rs`

- [ ] **Step 1: Write input.rs**

Move from `crates/feature-productivity/src/tracker/macos.rs` lines 209–226:

```rust
/// Get seconds since last user input (mouse or keyboard).
#[cfg(target_os = "macos")]
pub fn seconds_since_last_input() -> f64 {
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(source_state_id: u32, event_type: u32) -> f64;
    }
    const COMBINED_SESSION: u32 = 0;
    const ANY_INPUT: u32 = u32::MAX;
    unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION, ANY_INPUT) }
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_last_input() -> f64 {
    0.0
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p platform-macos
```

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/input.rs
git commit -m "feat(platform-macos): add idle detection via CGEventSource"
```

---

### Task 4: Implement pasteboard module

**Files:**
- Create: `crates/platform-macos/src/pasteboard.rs`

- [ ] **Step 1: Write pasteboard.rs**

Move from `crates/feature-launcher/src/clipboard/monitor.rs` lines 47–62:

```rust
/// Get the current pasteboard change count. Increments on each copy.
#[cfg(target_os = "macos")]
pub fn pasteboard_change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    NSPasteboard::generalPasteboard().changeCount() as i64
}

/// Read the current string content from the general pasteboard.
#[cfg(target_os = "macos")]
pub fn read_pasteboard_string() -> Option<String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    let pasteboard = NSPasteboard::generalPasteboard();
    let string = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }?;
    Some(string.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn pasteboard_change_count() -> i64 { 0 }

#[cfg(not(target_os = "macos"))]
pub fn read_pasteboard_string() -> Option<String> { None }
```

- [ ] **Step 2: Verify and commit**

```bash
cargo check -p platform-macos
git add crates/platform-macos/src/pasteboard.rs
git commit -m "feat(platform-macos): add pasteboard read via NSPasteboard"
```

---

## Chunk 2: Browser + Apps modules, then rewire consumers

### Task 5: Implement browser module (unify both crates' browser knowledge)

**Files:**
- Create: `crates/platform-macos/src/browser.rs`

- [ ] **Step 1: Write browser.rs with unified registry**

Combine productivity's 11-browser list with launcher's profile directory knowledge:

```rust
use std::path::PathBuf;

/// Known browser definitions.
pub struct BrowserDef {
    /// Display name (case-preserved).
    pub name: &'static str,
    /// Lowercased name for matching.
    pub name_lower: &'static str,
    /// Bundle ID prefixes for matching.
    pub bundle_prefixes: &'static [&'static str],
    /// Chromium profile directory relative to ~/Library/Application Support/
    /// (None for non-Chromium browsers like Safari, Firefox).
    pub profile_dir: Option<&'static str>,
    /// Suffix appended to window titles by this browser.
    pub title_suffix: &'static str,
}

pub const BROWSERS: &[BrowserDef] = &[
    BrowserDef {
        name: "Google Chrome", name_lower: "google chrome",
        bundle_prefixes: &["com.google.chrome"],
        profile_dir: Some("Google/Chrome/Default"),
        title_suffix: " - Google Chrome",
    },
    BrowserDef {
        name: "Arc", name_lower: "arc",
        bundle_prefixes: &["company.thebrowser.browser"],
        profile_dir: Some("Arc/User Data/Default"),
        title_suffix: " - Arc",
    },
    BrowserDef {
        name: "Brave Browser", name_lower: "brave browser",
        bundle_prefixes: &["com.brave.browser"],
        profile_dir: Some("BraveSoftware/Brave-Browser/Default"),
        title_suffix: " - Brave",
    },
    BrowserDef {
        name: "Microsoft Edge", name_lower: "microsoft edge",
        bundle_prefixes: &["com.microsoft.edgemac"],
        profile_dir: Some("Microsoft Edge/Default"),
        title_suffix: " - Microsoft Edge",
    },
    BrowserDef {
        name: "Safari", name_lower: "safari",
        bundle_prefixes: &["com.apple.safari"],
        profile_dir: None,
        title_suffix: " - Safari",
    },
    BrowserDef {
        name: "Firefox", name_lower: "firefox",
        bundle_prefixes: &["org.mozilla.firefox"],
        profile_dir: None,
        title_suffix: " - Mozilla Firefox",
    },
    BrowserDef {
        name: "Vivaldi", name_lower: "vivaldi",
        bundle_prefixes: &["com.vivaldi.vivaldi"],
        profile_dir: None,
        title_suffix: " - Vivaldi",
    },
    BrowserDef {
        name: "Opera", name_lower: "opera",
        bundle_prefixes: &["com.operasoftware.opera"],
        profile_dir: None,
        title_suffix: " - Opera",
    },
    BrowserDef {
        name: "Chromium", name_lower: "chromium",
        bundle_prefixes: &[],
        profile_dir: None,
        title_suffix: " - Chromium",
    },
    BrowserDef {
        name: "Orion", name_lower: "orion",
        bundle_prefixes: &[],
        profile_dir: None,
        title_suffix: " - Orion",
    },
    BrowserDef {
        name: "Zen Browser", name_lower: "zen browser",
        bundle_prefixes: &[],
        profile_dir: None,
        title_suffix: " - Zen",
    },
];

/// Check if an app is a browser by name or bundle ID.
pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
    let lower = app_name.to_lowercase();
    BROWSERS.iter().any(|b| {
        b.name_lower == lower
            || bundle_id.is_some_and(|bid| {
                let bid_lower = bid.to_lowercase();
                b.bundle_prefixes.iter().any(|p| bid_lower.starts_with(p))
            })
    })
}

/// Get the Chromium profile directory for a browser name (e.g., "chrome" → ~/Library/Application Support/Google/Chrome/Default).
pub fn chromium_profile_dir(browser: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let app_support = PathBuf::from(&home).join("Library/Application Support");
    let lower = browser.to_lowercase();
    BROWSERS.iter()
        .find(|b| b.name_lower == lower || b.name_lower.starts_with(&lower))
        .and_then(|b| b.profile_dir)
        .map(|dir| app_support.join(dir))
}

/// Extract the site name from a browser window title by stripping known browser suffixes.
pub fn extract_site_name(window_title: &str) -> Option<String> {
    let lower = window_title.to_lowercase();
    for browser in BROWSERS {
        if let Some(pos) = lower.rfind(browser.title_suffix) {
            let site = &window_title[..pos];
            if !site.is_empty() {
                return Some(site.to_string());
            }
        }
    }
    // Also handle em-dash variants
    let em_dash_suffixes = [" — Mozilla Firefox", " — Safari"];
    for suffix in em_dash_suffixes {
        if let Some(pos) = lower.rfind(&suffix.to_lowercase()) {
            let site = &window_title[..pos];
            if !site.is_empty() {
                return Some(site.to_string());
            }
        }
    }
    None
}

/// Get the URL of the active browser tab via AppleScript.
/// Works for Chrome-family browsers and Safari.
#[cfg(target_os = "macos")]
pub fn get_browser_url(app_name: &str, bundle_id: Option<&str>) -> Option<String> {
    if !is_browser(app_name, bundle_id) {
        return None;
    }
    let lower = app_name.to_lowercase();
    let script = if lower == "safari" {
        r#"tell application "Safari" to get URL of front document"#.to_string()
    } else {
        format!(r#"tell application "{app_name}" to get URL of active tab of front window"#)
    };
    let output = std::process::Command::new("osascript")
        .arg("-e").arg(&script).output().ok()?;
    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if url.is_empty() || url == "missing value" { None } else { Some(url) }
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_browser_url(_app_name: &str, _bundle_id: Option<&str>) -> Option<String> {
    None
}
```

- [ ] **Step 2: Verify and commit**

```bash
cargo check -p platform-macos
git add crates/platform-macos/src/browser.rs
git commit -m "feat(platform-macos): add unified 11-browser registry with profile dirs + URL extraction"
```

---

### Task 6: Implement apps module (running apps + icon cache)

**Files:**
- Create: `crates/platform-macos/src/apps.rs`

- [ ] **Step 1: Write apps.rs with running_applications()**

Replace the launcher's JXA subprocess approach with native NSWorkspace:

```rust
use std::path::PathBuf;

/// A running application entry.
#[derive(Debug, Clone)]
pub struct RunningApp {
    pub name: String,
    pub bundle_id: Option<String>,
    pub pid: u32,
    pub path: PathBuf,
}

/// Get all visible (non-background) running applications via NSWorkspace.
#[cfg(target_os = "macos")]
pub fn running_applications() -> Vec<RunningApp> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    let mut result = Vec::new();

    for app in apps.iter() {
        // Skip background-only apps
        if app.activationPolicy() as i64 != 0 {
            // NSApplicationActivationPolicyRegular = 0
            continue;
        }
        let name = match app.localizedName() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let pid = app.processIdentifier() as u32;
        let path = app.bundleURL()
            .map(|u| PathBuf::from(u.path().map(|p| p.to_string()).unwrap_or_default()))
            .unwrap_or_default();

        result.push(RunningApp { name, bundle_id, pid, path });
    }
    result
}

#[cfg(not(target_os = "macos"))]
pub fn running_applications() -> Vec<RunningApp> {
    vec![]
}
```

Note: `NSWorkspace.runningApplications()` requires the `NSRunningApplication` feature which is already in our dep. We also need `objc2-app-kit` features `"NSApplication"` and may need `"block2"` — verify at compile time. The `activationPolicy()` and `bundleURL()` methods may need additional features: `"NSRunningApplication"` should cover them.

- [ ] **Step 2: Add AppIconCache**

Move the icon extraction logic from `crates/feature-launcher/src/search/app_index.rs` lines 117–231 into a standalone struct:

```rust
/// Disk-backed app icon cache. Extracts .icns → 32px PNG via `sips`,
/// caches by app mtime in `{cache_dir}/{stem}.png` + `{stem}.mtime`.
pub struct AppIconCache {
    cache_dir: PathBuf,
}

impl AppIconCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        Self { cache_dir }
    }

    /// Get the icon for an app as a base64 data URI (e.g., "data:image/png;base64,...").
    /// Returns (data_uri, was_cache_hit).
    pub fn get_icon(&self, app_path: &std::path::Path) -> (Option<String>, bool) {
        // ... (move resolve_icon logic from app_index.rs)
    }
}
```

Copy `resolve_icon`, `get_mtime`, and `extract_icon` from `app_index.rs` as methods on `AppIconCache`. Keep them `#[cfg(target_os = "macos")]` with non-macOS stubs.

- [ ] **Step 3: Verify and commit**

```bash
cargo check -p platform-macos
git add crates/platform-macos/src/apps.rs
git commit -m "feat(platform-macos): add running_applications() via NSWorkspace + AppIconCache"
```

---

### Task 7: Rewire feature-productivity to use platform-macos

**Files:**
- Modify: `crates/feature-productivity/Cargo.toml`
- Modify: `crates/feature-productivity/src/tracker/macos.rs`
- Modify: `crates/feature-productivity/src/tracker/categorizer/browser.rs`

- [ ] **Step 1: Update Cargo.toml**

In `crates/feature-productivity/Cargo.toml`:
1. Add `platform-macos.workspace = true` to `[dependencies]`
2. Remove the `[target.'cfg(target_os = "macos")'.dependencies]` section entirely (objc2, core-graphics, core-foundation all come through platform-macos now)

- [ ] **Step 2: Rewrite tracker/macos.rs as thin wrapper**

Replace the entire file with delegations to `platform_macos`:

```rust
//! macOS-specific window and idle detection — delegates to platform-macos crate.

pub use platform_macos::window::WindowInfo;

pub fn get_frontmost_window() -> common::Result<Option<WindowInfo>> {
    Ok(platform_macos::window::get_frontmost_window())
}

pub fn seconds_since_last_input() -> f64 {
    platform_macos::input::seconds_since_last_input()
}

pub fn get_browser_url(app_name: &str, bundle_id: Option<&str>) -> Option<String> {
    platform_macos::browser::get_browser_url(app_name, bundle_id)
}
```

The existing callers in `tracker/mod.rs` use `macos::get_frontmost_window()` returning `Result<Option<WindowInfo>>`, so we wrap the new `Option<WindowInfo>` in `Ok()`. No changes needed in `tracker/mod.rs`.

- [ ] **Step 3: Update browser.rs to use shared constants**

In `crates/feature-productivity/src/tracker/categorizer/browser.rs`, replace the hardcoded `BROWSER_APPS` and `BROWSER_BUNDLE_PREFIXES` with references to `platform_macos::browser`:

```rust
/// Check if an app is a browser.
pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
    platform_macos::browser::is_browser(app_name, bundle_id)
}
```

Keep `BROWSER_SUFFIXES` and `extract_site_name` if they have productivity-specific behavior beyond what platform-macos provides, otherwise delegate.

**Important:** The categorizer module may have other functions that reference the old constants. Read the file carefully and update all references.

- [ ] **Step 4: Verify productivity compiles and tests pass**

```bash
cargo check -p feature-productivity
cargo nextest run -p feature-productivity
```

- [ ] **Step 5: Commit**

```bash
git add crates/feature-productivity/
git commit -m "refactor(productivity): delegate macOS FFI to platform-macos crate"
```

---

### Task 8: Rewire feature-launcher to use platform-macos

**Files:**
- Modify: `crates/feature-launcher/Cargo.toml`
- Modify: `crates/feature-launcher/src/clipboard/monitor.rs`
- Modify: `crates/feature-launcher/src/window_mgmt/accessibility.rs`
- Modify: `crates/feature-launcher/src/search/running_apps.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`
- Modify: `crates/feature-launcher/src/search/bookmarks.rs`
- Modify: `crates/feature-launcher/src/search/browser_history.rs`
- Modify: `crates/feature-launcher/src/window_mgmt/actions.rs`

- [ ] **Step 1: Update Cargo.toml**

In `crates/feature-launcher/Cargo.toml`:
1. Add `platform-macos.workspace = true` to `[dependencies]`
2. Remove `objc2-app-kit` and `objc2-foundation` from `[target.'cfg(target_os = "macos")'.dependencies]` (if no other macOS deps remain, remove that section entirely)

- [ ] **Step 2: Rewrite clipboard/monitor.rs**

Replace the three macOS methods with platform-macos calls:

```rust
#[cfg(target_os = "macos")]
fn get_change_count(&self) -> i64 {
    platform_macos::pasteboard::pasteboard_change_count()
}

#[cfg(target_os = "macos")]
fn read_pasteboard(&self) -> Option<String> {
    platform_macos::pasteboard::read_pasteboard_string()
}

#[cfg(target_os = "macos")]
fn get_frontmost_app_name(&self) -> Option<String> {
    platform_macos::window::get_frontmost_window().map(|w| w.app_name)
}
```

Remove the `use objc2_app_kit::*` imports from this file.

- [ ] **Step 3: Rewrite window_mgmt/accessibility.rs**

Replace the entire stubbed file with platform-macos delegations:

```rust
//! Window management using platform-macos AXUIElement wrappers.

/// Get the frontmost window's PID for window management operations.
pub fn get_frontmost_pid() -> Option<i32> {
    platform_macos::window::get_frontmost_window().map(|w| w.pid)
}

/// Get the screen frame: (x, y, width, height).
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    platform_macos::window::get_screen_frame()
}

/// Move and resize the frontmost window.
pub fn set_window_frame(pid: i32, x: f64, y: f64, w: f64, h: f64) -> bool {
    platform_macos::window::set_window_frame(pid, x, y, w, h)
}
```

- [ ] **Step 4: Update window_mgmt/actions.rs**

Replace the `WindowManager::execute` method to use the new functions:

```rust
#[cfg(target_os = "macos")]
pub fn execute(&self, action: &WindowAction) -> common::Result<()> {
    use super::accessibility;

    let pid = accessibility::get_frontmost_pid().ok_or_else(|| {
        common::KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No frontmost window",
        ))
    })?;
    let screen = accessibility::get_screen_frame();

    // Use PID as window ID for cycle tracking (since we no longer have AXWindow.id())
    let window_id = pid as u32;
    let cycle_index = { /* ... existing cycle logic using window_id ... */ };

    let (x, y, w, h) = self.compute_frame(action, &screen, cycle_index);
    if !accessibility::set_window_frame(pid, x, y, w, h) {
        return Err(common::KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to set window frame (Accessibility permission required)",
        )));
    }
    Ok(())
}
```

Update the cycle tracking to use `pid as u32` instead of `window.id()`.

- [ ] **Step 5: Rewrite running_apps.rs**

Replace the JXA subprocess with platform-macos:

```rust
async fn refresh(&self) {
    let apps = tokio::task::spawn_blocking(|| {
        platform_macos::apps::running_applications()
            .into_iter()
            .map(|a| (a.name, a.pid, a.path))
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    tracing::debug!("Refreshed {} running apps", apps.len());
    *self.apps.write() = apps;
}
```

Remove the `#[cfg(target_os = "macos")]` impl block with `get_running_apps()` and the `#[cfg(not)]` stub — the platform-macos crate handles platform gating.

- [ ] **Step 6: Replace chromium_profile_dir in search/mod.rs**

Remove the `chromium_profile_dir()` function from `search/mod.rs` (lines 251–261). Add a re-export:

```rust
pub use platform_macos::browser::chromium_profile_dir;
```

- [ ] **Step 7: Update bookmarks.rs and browser_history.rs imports**

In `bookmarks.rs`, change:
```rust
// Before:
use super::chromium_profile_dir;
// After:
use platform_macos::browser::chromium_profile_dir;
```

Same in `browser_history.rs`.

- [ ] **Step 8: Verify launcher compiles and tests pass**

```bash
cargo check -p feature-launcher
cargo nextest run -p feature-launcher
```

- [ ] **Step 9: Commit**

```bash
git add crates/feature-launcher/
git commit -m "refactor(launcher): delegate macOS FFI to platform-macos, fix window management"
```

---

## Chunk 3: Final verification

### Task 9: Full workspace verification

- [ ] **Step 1: Check the full workspace compiles**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all tests**

```bash
cargo nextest run --workspace
```

- [ ] **Step 3: Run clippy on affected crates**

```bash
cargo clippy -p platform-macos -p feature-launcher -p feature-productivity -- -D warnings
```

- [ ] **Step 4: Verify formatting**

```bash
cargo fmt --all --check
```

- [ ] **Step 5: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix: workspace-wide compilation fixes for platform-macos refactor"
```
