# System-Wide Input Services Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land a `CGEventTap` primitive in `platform-macos` and a new `feature-input-services` crate that ships snippet expansion + Hyper Key remapping with a settings UI and lazy-permission flow — all in four independently shippable sub-PRs.

**Architecture:** `platform-macos::event_tap` exposes a thin transport (CFRunLoop thread → tokio mpsc). `feature-input-services` houses pure-function `SnippetEngine` and `HyperKeyEngine` plus a `SnippetRepo` with watch-channel hot reload. App-core wires both engines into a single event-tap subscription. Settings UI manages snippets + Hyper Key + permission state.

**Tech Stack:** Rust 1.93, Tokio, `core-foundation` 0.10, `core-graphics` 0.24, `objc2` 0.6, `parking_lot`, `sqlx`, Tauri 2, React 18 + Vite + Bun + Vitest. macOS only.

**Reference spec:** `docs/superpowers/specs/2026-04-19-system-wide-input-services-design.md`

---

## File Map

### PR-1 `event_tap` primitive
- **Create:** `crates/platform-macos/src/event_tap/mod.rs` — public API (`EventTap`, `EventTapConfig`, `RawKeyEvent`, `TapAction`, `EventTapError`, `PermissionStatus`).
- **Create:** `crates/platform-macos/src/event_tap/worker.rs` — CFRunLoop OS thread + C trampoline.
- **Create:** `crates/platform-macos/src/event_tap/permission.rs` — `check_input_monitoring`, `request_input_monitoring`.
- **Create:** `crates/platform-macos/examples/keystroke_logger.rs` — manual smoke-test binary.
- **Modify:** `crates/platform-macos/src/lib.rs` — add `pub mod event_tap;`.
- **Modify:** `crates/platform-macos/Cargo.toml` — add `core-foundation`, `core-graphics`, `objc2`, `objc2-foundation`, `objc2-core-foundation`, `thiserror`.

### PR-2 `feature-input-services` crate (snippets only)
- **Create:** `crates/feature-input-services/Cargo.toml`.
- **Create:** `crates/feature-input-services/src/lib.rs` — `FeaturePackage` impl, public re-exports.
- **Create:** `crates/feature-input-services/src/snippet/mod.rs`.
- **Create:** `crates/feature-input-services/src/snippet/types.rs` — `Snippet`, `SnippetTable`.
- **Create:** `crates/feature-input-services/src/snippet/detector.rs` — `TriggerDetector`.
- **Create:** `crates/feature-input-services/src/snippet/expander.rs` — `Expander`, `EventPoster`/`Clipboard` traits + macOS impls.
- **Create:** `crates/feature-input-services/src/snippet/engine.rs` — `SnippetEngine`.
- **Create:** `crates/feature-input-services/src/repos/mod.rs`.
- **Create:** `crates/feature-input-services/src/repos/snippet_repo.rs`.
- **Create:** `crates/feature-input-services/migrations/001_snippets.sql`.
- **Create:** `crates/feature-input-services/PRIVACY.md`.
- **Modify:** `Cargo.toml` (workspace root) — add `crates/feature-input-services` to `members`.
- **Modify:** `crates/config/src/schema/mod.rs` — add `InputServicesConfig`.
- **Create:** `crates/app-core/src/init/input_services.rs` — `init_input_services`.
- **Modify:** `crates/app-core/src/init/mod.rs` — call `init_input_services`.
- **Modify:** `crates/app-core/src/state.rs` (or wherever `AppCore` is defined) — store handle.
- **Create:** `crates/app-core/src/handlers/input_services.rs`.
- **Create:** `crates/desktop/src/commands/input_services.rs` — Tauri commands + `DEV_COMMANDS`.
- **Modify:** `crates/desktop/src/commands/mod.rs` — register module.
- **Modify:** `crates/desktop/src/lib.rs` — add to `invoke_handler!`.
- **Modify:** `crates/desktop/src/dev_server/mod.rs` — coverage entries.
- **Create:** `desktop-ui/src/features/settings/InputServicesPage.tsx`.
- **Create:** `desktop-ui/src/features/settings/components/SnippetsTab.tsx`.
- **Create:** `desktop-ui/src/features/settings/components/PermissionBanner.tsx`.
- **Modify:** `desktop-ui/src/features/settings/SettingsNav.tsx` (or equivalent) — add nav entry.

### PR-3 Hyper Key engine + UI
- **Create:** `crates/feature-input-services/src/hyper_key/mod.rs`.
- **Create:** `crates/feature-input-services/src/hyper_key/config.rs`.
- **Create:** `crates/feature-input-services/src/hyper_key/engine.rs`.
- **Create:** `crates/feature-input-services/src/hyper_key/caps_lock.rs` — hidutil remap helpers.
- **Modify:** `crates/feature-input-services/src/lib.rs` — re-export.
- **Modify:** `crates/app-core/src/init/input_services.rs` — wire into event tap handler chain.
- **Modify:** `crates/desktop/src/commands/input_services.rs` — `set_hyper_key` command.
- **Create:** `desktop-ui/src/features/settings/components/HyperKeyTab.tsx`.
- **Modify:** `desktop-ui/src/features/settings/InputServicesPage.tsx` — add tab.

### PR-4 Permission polish + failure recovery
- **Modify:** `crates/platform-macos/src/event_tap/worker.rs` — surface `TapDied` events via callback.
- **Modify:** `crates/app-core/src/init/input_services.rs` — `TapDied` handler + desktop notification.
- **Modify:** `crates/desktop/src/commands/input_services.rs` — `input_services_status` returning live `event_tap_alive`.
- **Modify:** `desktop-ui/src/features/settings/components/PermissionBanner.tsx` — polled status, restart prompt after grant.

---

# PR-1 — `event_tap` primitive

### Task 1.1: Add macOS framework deps

**Files:**
- Modify: `crates/platform-macos/Cargo.toml`

- [ ] **Step 1: Add deps**

Append under `[dependencies]`:
```toml
core-foundation = "0.10"
core-graphics = "0.24"
objc2 = "0.6"
objc2-foundation = "0.3"
objc2-core-foundation = "0.3"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
parking_lot = "0.12"
tokio = { version = "1", features = ["sync", "rt"] }
tracing = "0.1"
```

(Some may already exist — check before adding to avoid duplicates. Use `cargo metadata --format-version 1 --no-deps | jq '.packages[] | select(.name=="platform-macos") | .dependencies'` if unsure.)

- [ ] **Step 2: Verify build**

```bash
cargo build -p platform-macos
```
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/Cargo.toml Cargo.lock
git commit -m "build(platform-macos): add core-graphics + objc2 deps for event tap"
```

### Task 1.2: Public types module

**Files:**
- Create: `crates/platform-macos/src/event_tap/mod.rs`
- Modify: `crates/platform-macos/src/lib.rs`

- [ ] **Step 1: Create module skeleton**

Create `crates/platform-macos/src/event_tap/mod.rs`:
```rust
//! Global keystroke event tap built on `CGEventTap`.
//!
//! Spawns a dedicated OS thread running `CFRunLoopRun()`. The handler
//! callback runs on that thread and must return quickly. For long work,
//! the handler should dispatch to a tokio task via mpsc.

mod permission;
mod worker;

pub use permission::{check_input_monitoring, request_input_monitoring, PermissionStatus};

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TapMode { Observe, Modify }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TapEventType { KeyDown, KeyUp, FlagsChanged }

#[derive(Debug, Clone)]
pub struct EventTapConfig {
    pub mode: TapMode,
    pub event_types: Vec<TapEventType>,
}

#[derive(Debug, Clone)]
pub struct RawKeyEvent {
    pub keycode: u16,
    pub is_down: bool,
    pub flags: u64,
    pub characters: Option<String>,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone)]
pub enum TapAction {
    Pass,
    Suppress,
    Replace(Vec<RawKeyEvent>),
}

#[derive(Debug, thiserror::Error)]
pub enum EventTapError {
    #[error("Input Monitoring permission denied")]
    PermissionDenied,
    #[error("CGEventTap creation failed (kernel/HID error)")]
    TapCreation,
    #[error("Worker thread spawn failed: {0}")]
    ThreadSpawn(String),
}

pub struct EventTap {
    inner: Arc<worker::TapWorker>,
}

impl EventTap {
    pub fn start(
        cfg: EventTapConfig,
        handler: impl Fn(RawKeyEvent) -> TapAction + Send + Sync + 'static,
    ) -> Result<Self, EventTapError> {
        let inner = worker::TapWorker::start(cfg, Arc::new(handler))?;
        Ok(Self { inner: Arc::new(inner) })
    }

    pub fn stop(&self) { self.inner.stop(); }

    pub fn is_alive(&self) -> bool { self.inner.is_alive() }
}

impl Drop for EventTap {
    fn drop(&mut self) { self.stop(); }
}
```

In `crates/platform-macos/src/lib.rs`, add:
```rust
pub mod event_tap;
```

- [ ] **Step 2: Stub permission module**

Create `crates/platform-macos/src/event_tap/permission.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus { Granted, Denied, Unknown }

#[cfg(target_os = "macos")]
pub fn check_input_monitoring() -> PermissionStatus {
    extern "C" {
        fn IOHIDCheckAccess(request_type: u32) -> u32;
    }
    const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
    const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;
    const K_IOHID_ACCESS_TYPE_DENIED: u32 = 1;
    let v = unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) };
    match v {
        K_IOHID_ACCESS_TYPE_GRANTED => PermissionStatus::Granted,
        K_IOHID_ACCESS_TYPE_DENIED => PermissionStatus::Denied,
        _ => PermissionStatus::Unknown,
    }
}

#[cfg(target_os = "macos")]
pub fn request_input_monitoring() {
    extern "C" {
        fn IOHIDRequestAccess(request_type: u32) -> bool;
    }
    const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
    let granted = unsafe { IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) };
    if !granted {
        // macOS already showed (or queued) the system dialog; also open System Settings.
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .spawn();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_input_monitoring() -> PermissionStatus { PermissionStatus::Unknown }

#[cfg(not(target_os = "macos"))]
pub fn request_input_monitoring() {}
```

- [ ] **Step 3: Stub worker module (compile-only)**

Create `crates/platform-macos/src/event_tap/worker.rs`:
```rust
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

type Handler = Arc<dyn Fn(RawKeyEvent) -> TapAction + Send + Sync>;

pub(super) struct TapWorker {
    alive: AtomicBool,
    _handler: Handler,
    _cfg: EventTapConfig,
}

impl TapWorker {
    pub fn start(cfg: EventTapConfig, handler: Handler) -> Result<Self, EventTapError> {
        // Real implementation lands in Task 1.3.
        Ok(Self { alive: AtomicBool::new(true), _handler: handler, _cfg: cfg })
    }

    pub fn stop(&self) { self.alive.store(false, Ordering::SeqCst); }

    pub fn is_alive(&self) -> bool { self.alive.load(Ordering::SeqCst) }
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p platform-macos
```
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/platform-macos/src/event_tap/ crates/platform-macos/src/lib.rs
git commit -m "feat(platform-macos): event_tap module skeleton + permission helpers"
```

### Task 1.3: CFRunLoop worker thread + C trampoline

**Files:**
- Modify: `crates/platform-macos/src/event_tap/worker.rs`

- [ ] **Step 1: Implement worker**

Replace `worker.rs` body:
```rust
use super::*;
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPort;
use core_foundation::runloop::{CFRunLoop, CFRunLoopRun, kCFRunLoopCommonModes};
use core_graphics::event::{
    CGEvent, CGEventField, CGEventFlags, CGEventRef, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
};
use core_graphics::sys::CGEventMask;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;
use std::thread;

type Handler = Arc<dyn Fn(RawKeyEvent) -> TapAction + Send + Sync>;

pub(super) struct TapWorker {
    alive: Arc<AtomicBool>,
    runloop_ptr: AtomicPtr<core_foundation::runloop::__CFRunLoop>,
    _join: Option<thread::JoinHandle<()>>,
}

impl TapWorker {
    pub fn start(cfg: EventTapConfig, handler: Handler) -> Result<Self, EventTapError> {
        // Verify permission first
        if super::permission::check_input_monitoring() != PermissionStatus::Granted {
            return Err(EventTapError::PermissionDenied);
        }

        let alive = Arc::new(AtomicBool::new(true));
        let alive_for_thread = alive.clone();
        let runloop_ptr = AtomicPtr::new(std::ptr::null_mut());
        let runloop_share: *mut std::sync::atomic::AtomicPtr<_> = std::ptr::addr_of!(runloop_ptr) as *mut _;

        // Build event mask
        let mask: CGEventMask = cfg.event_types.iter().fold(0, |acc, et| {
            acc | match et {
                TapEventType::KeyDown => 1 << CGEventType::KeyDown as u32,
                TapEventType::KeyUp => 1 << CGEventType::KeyUp as u32,
                TapEventType::FlagsChanged => 1 << CGEventType::FlagsChanged as u32,
            } as u64
        });

        // Box the handler so we can pass a stable pointer through userInfo.
        let user_info: Box<HandlerCtx> = Box::new(HandlerCtx { handler, alive: alive_for_thread });
        let user_info_ptr = Box::into_raw(user_info) as *mut c_void;

        let join = thread::Builder::new()
            .name("klyntbot-event-tap".into())
            .spawn(move || {
                unsafe {
                    let tap = core_graphics::event::CGEventTap::new(
                        CGEventTapLocation::HID,
                        CGEventTapPlacement::HeadInsertEventTap,
                        match cfg.mode {
                            TapMode::Observe => CGEventTapOptions::ListenOnly,
                            TapMode::Modify => CGEventTapOptions::Default,
                        },
                        Vec::new(), // we filter by mask below
                        |proxy, et, evt| {
                            // wrap; never run real handler from here
                            None
                        },
                    );
                    // Note: the high-level core_graphics CGEventTap closure-API doesn't
                    // give us mask control. Use the C API directly:
                    drop(tap);

                    extern "C" {
                        fn CGEventTapCreate(
                            tap: u32, place: u32, options: u32,
                            mask: u64,
                            callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
                            user_info: *mut c_void,
                        ) -> *mut c_void;
                        fn CFMachPortCreateRunLoopSource(
                            allocator: *const c_void, port: *mut c_void, order: isize,
                        ) -> *mut c_void;
                        fn CFRunLoopAddSource(rl: *mut c_void, src: *mut c_void, mode: *const c_void);
                        fn CFRunLoopGetCurrent() -> *mut c_void;
                        fn CGEventTapEnable(port: *mut c_void, enable: bool);
                    }

                    let port = CGEventTapCreate(
                        0,  // kCGSessionEventTap
                        0,  // kCGHeadInsertEventTap
                        match cfg.mode {
                            TapMode::Observe => 1, // kCGEventTapOptionListenOnly
                            TapMode::Modify => 0,  // kCGEventTapOptionDefault
                        },
                        mask,
                        c_callback,
                        user_info_ptr,
                    );
                    if port.is_null() {
                        // Free user_info to avoid leak
                        let _ = Box::from_raw(user_info_ptr as *mut HandlerCtx);
                        tracing::error!("CGEventTapCreate returned null");
                        return;
                    }
                    let src = CFMachPortCreateRunLoopSource(std::ptr::null(), port, 0);
                    let rl = CFRunLoopGetCurrent();
                    // Stash runloop pointer so stop() can wake it
                    (*runloop_share).store(rl as *mut _, Ordering::SeqCst);
                    CFRunLoopAddSource(rl, src, kCFRunLoopCommonModes as *const c_void);
                    CGEventTapEnable(port, true);
                    CFRunLoopRun();
                    // On return: free user_info
                    let _ = Box::from_raw(user_info_ptr as *mut HandlerCtx);
                }
            })
            .map_err(|e| EventTapError::ThreadSpawn(e.to_string()))?;

        Ok(Self {
            alive,
            runloop_ptr,
            _join: Some(join),
        })
    }

    pub fn stop(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let rl = self.runloop_ptr.load(Ordering::SeqCst);
        if !rl.is_null() {
            extern "C" { fn CFRunLoopStop(rl: *mut c_void); }
            unsafe { CFRunLoopStop(rl as *mut c_void); }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

struct HandlerCtx { handler: Handler, alive: Arc<AtomicBool> }

extern "C" fn c_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void {
    if user_info.is_null() || event.is_null() { return event; }
    let ctx: &HandlerCtx = unsafe { &*(user_info as *const HandlerCtx) };
    if !ctx.alive.load(Ordering::SeqCst) {
        // Wind down: pass-through
        return event;
    }
    // Detect tap-disabled events (0xFFFFFFFE = ByTimeout, 0xFFFFFFFF = ByUserInput)
    if event_type == 0xFFFFFFFE || event_type == 0xFFFFFFFF {
        ctx.alive.store(false, Ordering::SeqCst);
        tracing::warn!("CGEventTap disabled (type={event_type})");
        return event;
    }
    let raw = match build_raw_event(event_type, event) {
        Some(r) => r, None => return event,
    };
    let action = (ctx.handler)(raw);
    match action {
        TapAction::Pass => event,
        TapAction::Suppress => std::ptr::null_mut(),
        TapAction::Replace(events) => {
            // Post replacements via CGEventPost; suppress original
            for e in events { post_synthetic(&e); }
            std::ptr::null_mut()
        }
    }
}

fn build_raw_event(event_type: u32, event: *mut c_void) -> Option<RawKeyEvent> {
    extern "C" {
        fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
        fn CGEventGetFlags(event: *mut c_void) -> u64;
        fn CGEventGetTimestamp(event: *mut c_void) -> u64;
    }
    const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
    let flags = unsafe { CGEventGetFlags(event) };
    let ts = unsafe { CGEventGetTimestamp(event) };
    let is_down = match event_type {
        10 => true,   // kCGEventKeyDown
        11 => false,  // kCGEventKeyUp
        12 => true,   // kCGEventFlagsChanged (treat as down for state tracking)
        _ => return None,
    };
    let characters = unicode_for_keycode(keycode, flags);
    Some(RawKeyEvent { keycode, is_down, flags, characters, timestamp_ns: ts })
}

fn unicode_for_keycode(keycode: u16, flags: u64) -> Option<String> {
    // Use UCKeyTranslate via TIS for layout-aware conversion. For v1, do a minimal
    // ASCII-only mapping for keycodes 0-50 to unblock the snippet engine.
    // Full implementation lands in a follow-up; this covers letters/digits/space.
    use std::collections::HashMap;
    static TABLE: once_cell::sync::Lazy<HashMap<u16, char>> = once_cell::sync::Lazy::new(|| {
        let mut m = HashMap::new();
        for (kc, c) in [
            (0x00,'a'),(0x01,'s'),(0x02,'d'),(0x03,'f'),(0x04,'h'),(0x05,'g'),
            (0x06,'z'),(0x07,'x'),(0x08,'c'),(0x09,'v'),(0x0B,'b'),(0x0C,'q'),
            (0x0D,'w'),(0x0E,'e'),(0x0F,'r'),(0x10,'y'),(0x11,'t'),(0x1F,'o'),
            (0x20,'u'),(0x22,'i'),(0x23,'p'),(0x25,'l'),(0x26,'j'),(0x28,'k'),
            (0x2D,'n'),(0x2E,'m'),(0x31,' '),(0x29,';'),(0x2C,'/'),(0x2F,'.'),
            (0x2B,','),(0x18,'='),(0x1B,'-'),
        ] { m.insert(kc, c); }
        m
    });
    let c = TABLE.get(&keycode).copied()?;
    let shift_held = (flags & (1 << 17)) != 0; // kCGEventFlagMaskShift
    Some(if shift_held { c.to_ascii_uppercase().to_string() } else { c.to_string() })
}

fn post_synthetic(ev: &RawKeyEvent) {
    extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const c_void, keycode: u16, down: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
        fn CFRelease(cf: *const c_void);
    }
    unsafe {
        let e = CGEventCreateKeyboardEvent(std::ptr::null(), ev.keycode, ev.is_down);
        if e.is_null() { return; }
        if ev.flags != 0 { CGEventSetFlags(e, ev.flags); }
        CGEventPost(0, e); // kCGSessionEventTap
        CFRelease(e as *const _);
    }
}
```

Add `once_cell = "1"` to `crates/platform-macos/Cargo.toml`.

- [ ] **Step 2: Build**

```bash
cargo build -p platform-macos
```
Expected: builds with warnings tolerated for unused imports — fix any clippy-blocking issues.

- [ ] **Step 3: Lint**

```bash
cargo clippy -p platform-macos --all-targets -- -D warnings
```
Address any warnings (likely: unused `tap` binding, unsafe block warnings — wrap in `#[allow]` only if truly necessary).

- [ ] **Step 4: Commit**

```bash
git add -A crates/platform-macos
git commit -m "feat(platform-macos): CFRunLoop worker thread + C trampoline for event tap"
```

### Task 1.4: Manual smoke-test binary

**Files:**
- Create: `crates/platform-macos/examples/keystroke_logger.rs`

- [ ] **Step 1: Implement example**

```rust
//! Manual smoke test for the event_tap primitive.
//!
//! Requires Input Monitoring permission for the running terminal.
//! Run with: `cargo run --example keystroke_logger -p platform-macos`
//! Press Ctrl+C to exit.

use platform_macos::event_tap::{
    check_input_monitoring, EventTap, EventTapConfig, PermissionStatus,
    RawKeyEvent, TapAction, TapEventType, TapMode,
};
use std::time::Duration;

fn main() {
    if check_input_monitoring() != PermissionStatus::Granted {
        eprintln!("Input Monitoring not granted. Run from a terminal with permission, or grant in System Settings → Privacy & Security → Input Monitoring.");
        std::process::exit(1);
    }
    let _tap = EventTap::start(
        EventTapConfig {
            mode: TapMode::Observe,
            event_types: vec![TapEventType::KeyDown, TapEventType::KeyUp],
        },
        |ev: RawKeyEvent| {
            println!("kc={:#04x} down={} chars={:?} flags={:#x}",
                ev.keycode, ev.is_down, ev.characters, ev.flags);
            TapAction::Pass
        },
    ).expect("event tap start failed");
    println!("Logging keystrokes. Press Ctrl+C to exit.");
    loop { std::thread::sleep(Duration::from_secs(60)); }
}
```

- [ ] **Step 2: Build the example**

```bash
cargo build --example keystroke_logger -p platform-macos
```

- [ ] **Step 3: Manual run (engineer)**

```bash
cargo run --example keystroke_logger -p platform-macos
```
Expected: prints keystroke events for 5 seconds, then Ctrl+C exits cleanly. Verify a few characters print correctly (e.g. type `abc`).

- [ ] **Step 4: Commit**

```bash
git add crates/platform-macos/examples/keystroke_logger.rs
git commit -m "chore(platform-macos): keystroke_logger example for manual event_tap smoke testing"
```

### Task 1.5: PR-1 final gates

- [ ] **Step 1: Lint**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: zero warnings.

- [ ] **Step 2: Format + tests**

```bash
cargo fmt --all --check
cargo nextest run --workspace
```

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(platform-macos): CGEventTap primitive (event_tap module)" --body "Implements PR-1 of docs/superpowers/specs/2026-04-19-system-wide-input-services-design.md. Standalone primitive; no consumers yet. Smoke test via cargo run --example keystroke_logger -p platform-macos."
```

---

# PR-2 — `feature-input-services` crate (snippets only)

### Task 2.1: Crate skeleton

**Files:**
- Create: `crates/feature-input-services/Cargo.toml`
- Create: `crates/feature-input-services/src/lib.rs`
- Create: `crates/feature-input-services/migrations/001_snippets.sql`
- Create: `crates/feature-input-services/PRIVACY.md`
- Modify: workspace root `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "feature-input-services"
version = "0.1.0"
edition = "2021"

[dependencies]
tools-core = { path = "../tools-core" }
storage = { path = "../storage" }
common = { path = "../common" }
config = { path = "../config" }
platform-macos = { path = "../platform-macos" }
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
parking_lot.workspace = true
thiserror.workspace = true
jiff.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Skeleton lib.rs**

```rust
//! System-wide input services: snippet expansion + Hyper Key remapping.

pub mod snippet;
pub mod repos;

pub use repos::snippet_repo::SnippetRepo;
pub use snippet::{Snippet, SnippetEngine, SnippetTable, TriggerDetector};
```

- [ ] **Step 3: Migration**

Create `crates/feature-input-services/migrations/001_snippets.sql`:
```sql
CREATE TABLE IF NOT EXISTS snippets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL UNIQUE,
    body TEXT NOT NULL,
    description TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snippets_trigger ON snippets(trigger);
```

- [ ] **Step 4: PRIVACY.md**

```markdown
# Input Services Privacy Notes

`feature-input-services` taps every keystroke the user types via macOS's
`CGEventTap`. The following invariants are enforced in code:

1. **No keystroke contents are ever logged.** `tracing` events log event
   types, keycodes, and timing only — never characters typed.
2. **No keystroke contents are ever persisted.** The `TriggerDetector` holds
   a 64-character sliding window in memory only; it is dropped on shutdown.
3. **Snippet bodies are never logged at any level.** Trigger names are
   logged at info; bodies are not.
4. **No telemetry.** Keystroke events do not leave the local machine.

Violations of these invariants are review-blocking.
```

- [ ] **Step 5: Workspace registration**

Edit workspace root `Cargo.toml`, add `crates/feature-input-services` to `members = [...]`.

- [ ] **Step 6: Build**

```bash
cargo build -p feature-input-services
```
Expected: success (empty crate compiles).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/feature-input-services/
git commit -m "feat(feature-input-services): crate skeleton + snippets migration"
```

### Task 2.2: `Snippet` + `SnippetTable` types

**Files:**
- Create: `crates/feature-input-services/src/snippet/mod.rs`
- Create: `crates/feature-input-services/src/snippet/types.rs`

- [ ] **Step 1: Mod.rs**

```rust
pub mod types;
pub mod detector;
pub mod expander;
pub mod engine;

pub use types::{Snippet, SnippetTable};
pub use detector::{DetectionResult, TriggerDetector};
pub use engine::SnippetEngine;
```

- [ ] **Step 2: types.rs with tests**

```rust
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: i64,
    pub trigger: String,
    pub body: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Default)]
pub struct SnippetTable {
    pub by_trigger: HashMap<String, Snippet>,
    pub longest_trigger_len: usize,
    pub marker: char,
}

impl SnippetTable {
    pub fn lookup<'a>(&'a self, ending_at: &str) -> Option<&'a Snippet> {
        for n in (1..=self.longest_trigger_len.min(ending_at.chars().count())).rev() {
            // chars-aware suffix
            let suffix: String = ending_at.chars().rev().take(n).collect::<Vec<_>>()
                .into_iter().rev().collect();
            if let Some(s) = self.by_trigger.get(&suffix) {
                if s.enabled { return Some(s); }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snip(id: i64, trigger: &str, body: &str) -> Snippet {
        Snippet {
            id, trigger: trigger.into(), body: body.into(),
            description: None, enabled: true,
            created_at: Timestamp::now(), updated_at: Timestamp::now(),
        }
    }

    fn table(snippets: Vec<Snippet>) -> SnippetTable {
        let longest = snippets.iter().map(|s| s.trigger.chars().count()).max().unwrap_or(0);
        let by_trigger = snippets.into_iter().map(|s| (s.trigger.clone(), s)).collect();
        SnippetTable { by_trigger, longest_trigger_len: longest, marker: ';' }
    }

    #[test]
    fn lookup_finds_trigger_at_end() {
        let t = table(vec![snip(1, ";email", "me@example.com")]);
        let result = t.lookup("hello ;email");
        assert!(result.is_some());
        assert_eq!(result.unwrap().body, "me@example.com");
    }

    #[test]
    fn lookup_returns_longest_match() {
        let t = table(vec![
            snip(1, ";em",    "short"),
            snip(2, ";email", "long"),
        ]);
        let result = t.lookup("xx;email");
        assert_eq!(result.unwrap().body, "long");
    }

    #[test]
    fn lookup_skips_disabled() {
        let mut s = snip(1, ";email", "x"); s.enabled = false;
        let t = table(vec![s]);
        assert!(t.lookup(";email").is_none());
    }

    #[test]
    fn lookup_misses_unknown_trigger() {
        let t = table(vec![snip(1, ";email", "x")]);
        assert!(t.lookup(";nope").is_none());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-input-services snippet::types
```
Expected: 4 pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-input-services/src/snippet/
git commit -m "feat(input-services): Snippet + SnippetTable with longest-match lookup"
```

### Task 2.3: `TriggerDetector`

**Files:**
- Create: `crates/feature-input-services/src/snippet/detector.rs`

- [ ] **Step 1: Implementation + tests**

```rust
use super::types::{Snippet, SnippetTable};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::watch;

const BUFFER_CAP: usize = 64;

pub struct TriggerDetector {
    buffer: VecDeque<char>,
    table: watch::Receiver<Arc<SnippetTable>>,
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub snippet: Snippet,
    pub trigger_len: usize,
    pub boundary_char: char,
}

impl TriggerDetector {
    pub fn new(table: watch::Receiver<Arc<SnippetTable>>) -> Self {
        Self { buffer: VecDeque::with_capacity(BUFFER_CAP), table }
    }

    pub fn on_char(&mut self, c: char) -> Option<DetectionResult> {
        if self.buffer.len() == BUFFER_CAP { self.buffer.pop_front(); }
        self.buffer.push_back(c);
        if !is_word_boundary(c) { return None; }
        let table = self.table.borrow();
        let s: String = self.buffer.iter().take(self.buffer.len() - 1).copied().collect();
        let snippet = table.lookup(&s)?.clone();
        Some(DetectionResult {
            trigger_len: snippet.trigger.chars().count(),
            boundary_char: c,
            snippet,
        })
    }

    pub fn reset(&mut self) { self.buffer.clear(); }
}

fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(
        c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\'' | '\n' | '\t'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::types::Snippet;
    use jiff::Timestamp;
    use std::collections::HashMap;

    fn make_table(triggers: &[(&str, &str)]) -> watch::Receiver<Arc<SnippetTable>> {
        let snippets: Vec<Snippet> = triggers.iter().enumerate().map(|(i, (t, b))| Snippet {
            id: i as i64, trigger: (*t).into(), body: (*b).into(),
            description: None, enabled: true,
            created_at: Timestamp::now(), updated_at: Timestamp::now(),
        }).collect();
        let longest = snippets.iter().map(|s| s.trigger.chars().count()).max().unwrap_or(0);
        let table = SnippetTable {
            by_trigger: snippets.into_iter().map(|s| (s.trigger.clone(), s)).collect(),
            longest_trigger_len: longest, marker: ';',
        };
        let (tx, rx) = watch::channel(Arc::new(table));
        std::mem::forget(tx); // keep channel alive for test
        rx
    }

    #[test]
    fn detects_after_space_boundary() {
        let mut d = TriggerDetector::new(make_table(&[(";email", "x")]));
        for c in ";email".chars() { assert!(d.on_char(c).is_none()); }
        let result = d.on_char(' ');
        assert!(result.is_some());
        assert_eq!(result.unwrap().trigger_len, 6);
    }

    #[test]
    fn no_match_in_middle_of_word() {
        let mut d = TriggerDetector::new(make_table(&[(";em", "x")]));
        for c in "hello;em".chars() { assert!(d.on_char(c).is_none()); }
        // 'a' is not a word boundary — no match
        assert!(d.on_char('a').is_none());
    }

    #[test]
    fn longest_match_wins() {
        let mut d = TriggerDetector::new(make_table(&[(";em", "short"), (";email", "long")]));
        for c in ";email".chars() { assert!(d.on_char(c).is_none()); }
        let r = d.on_char(' ').unwrap();
        assert_eq!(r.snippet.body, "long");
    }

    #[test]
    fn reset_clears_buffer() {
        let mut d = TriggerDetector::new(make_table(&[(";em", "x")]));
        d.on_char(';'); d.on_char('e'); d.on_char('m');
        d.reset();
        // Now typing space alone shouldn't match
        assert!(d.on_char(' ').is_none());
    }
}
```

- [ ] **Step 2: Tests**

```bash
cargo nextest run -p feature-input-services snippet::detector
```
Expected: 4 pass.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-input-services/src/snippet/detector.rs
git commit -m "feat(input-services): TriggerDetector with sliding window + longest-match"
```

### Task 2.4: `Expander` with traits

**Files:**
- Create: `crates/feature-input-services/src/snippet/expander.rs`

- [ ] **Step 1: Implementation + tests**

```rust
use super::detector::DetectionResult;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

#[async_trait]
pub trait EventPoster: Send + Sync {
    async fn post_backspaces(&self, n: usize);
    async fn post_paste(&self);
    async fn post_char(&self, c: char);
}

#[async_trait]
pub trait Clipboard: Send + Sync {
    async fn read(&self) -> Option<String>;
    async fn write(&self, s: &str);
}

pub struct Expander {
    poster: Arc<dyn EventPoster>,
    clipboard: Arc<dyn Clipboard>,
}

impl Expander {
    pub fn new(poster: Arc<dyn EventPoster>, clipboard: Arc<dyn Clipboard>) -> Self {
        Self { poster, clipboard }
    }

    pub async fn expand(&self, det: DetectionResult) {
        let prev = self.clipboard.read().await;
        self.poster.post_backspaces(det.trigger_len + 1).await;
        self.clipboard.write(&det.snippet.body).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.poster.post_paste().await;
        let cb = self.clipboard.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if let Some(prev) = prev { cb.write(&prev).await; }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.poster.post_char(det.boundary_char).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::types::Snippet;
    use jiff::Timestamp;
    use parking_lot::Mutex;

    #[derive(Default)]
    struct MockPoster { events: Mutex<Vec<String>> }
    #[async_trait]
    impl EventPoster for MockPoster {
        async fn post_backspaces(&self, n: usize) {
            self.events.lock().push(format!("BS({n})"));
        }
        async fn post_paste(&self) {
            self.events.lock().push("PASTE".into());
        }
        async fn post_char(&self, c: char) {
            self.events.lock().push(format!("CHAR({c})"));
        }
    }

    #[derive(Default)]
    struct MockClipboard { history: Mutex<Vec<String>>, current: Mutex<Option<String>> }
    #[async_trait]
    impl Clipboard for MockClipboard {
        async fn read(&self) -> Option<String> { self.current.lock().clone() }
        async fn write(&self, s: &str) {
            *self.current.lock() = Some(s.into());
            self.history.lock().push(s.into());
        }
    }

    fn snip(trigger: &str, body: &str) -> Snippet {
        Snippet {
            id: 1, trigger: trigger.into(), body: body.into(),
            description: None, enabled: true,
            created_at: Timestamp::now(), updated_at: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn expand_emits_correct_event_sequence() {
        let poster = Arc::new(MockPoster::default());
        let cb = Arc::new(MockClipboard::default());
        *cb.current.lock() = Some("PRIOR".into());
        let exp = Expander::new(poster.clone(), cb.clone());
        let det = DetectionResult {
            snippet: snip(";email", "me@x.com"),
            trigger_len: 6,
            boundary_char: ' ',
        };
        exp.expand(det).await;
        let events = poster.events.lock().clone();
        assert_eq!(events, vec!["BS(7)".to_string(), "PASTE".into(), "CHAR( )".into()]);
        // Wait for restore
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(cb.history.lock().clone(), vec!["me@x.com".to_string(), "PRIOR".into()]);
    }
}
```

- [ ] **Step 2: Tests**

```bash
cargo nextest run -p feature-input-services snippet::expander
```
Expected: pass.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-input-services/src/snippet/expander.rs
git commit -m "feat(input-services): Expander with EventPoster + Clipboard traits"
```

### Task 2.5: macOS `EventPoster` + `Clipboard` impls

**Files:**
- Modify: `crates/feature-input-services/src/snippet/expander.rs` (append impls)

- [ ] **Step 1: Add CGEventPoster + NSPasteboardClipboard**

```rust
#[cfg(target_os = "macos")]
pub struct CGEventPoster;

#[cfg(target_os = "macos")]
#[async_trait]
impl EventPoster for CGEventPoster {
    async fn post_backspaces(&self, n: usize) {
        for _ in 0..n {
            post_keycode(0x33, true);   // Delete (Backspace)
            post_keycode(0x33, false);
        }
    }
    async fn post_paste(&self) {
        // Cmd+V
        const K_CMD_FLAG: u64 = 1 << 20;
        post_keycode_with_flags(0x09, true, K_CMD_FLAG); // V down
        post_keycode_with_flags(0x09, false, K_CMD_FLAG);
    }
    async fn post_char(&self, c: char) {
        // Translate via CGEventCreateKeyboardEvent + UCKeyTranslate.
        // For v1, only handle ASCII space + ASCII letters via a small table.
        if let Some(kc) = char_to_keycode(c) {
            post_keycode(kc, true);
            post_keycode(kc, false);
        }
    }
}

#[cfg(target_os = "macos")]
fn post_keycode(kc: u16, down: bool) { post_keycode_with_flags(kc, down, 0); }

#[cfg(target_os = "macos")]
fn post_keycode_with_flags(kc: u16, down: bool, flags: u64) {
    extern "C" {
        fn CGEventCreateKeyboardEvent(src: *const std::ffi::c_void, kc: u16, down: bool) -> *mut std::ffi::c_void;
        fn CGEventSetFlags(e: *mut std::ffi::c_void, f: u64);
        fn CGEventPost(tap: u32, e: *mut std::ffi::c_void);
        fn CFRelease(p: *const std::ffi::c_void);
    }
    unsafe {
        let e = CGEventCreateKeyboardEvent(std::ptr::null(), kc, down);
        if e.is_null() { return; }
        if flags != 0 { CGEventSetFlags(e, flags); }
        CGEventPost(0, e);
        CFRelease(e as *const _);
    }
}

#[cfg(target_os = "macos")]
fn char_to_keycode(c: char) -> Option<u16> {
    Some(match c {
        ' ' => 0x31, '\n' => 0x24, '\t' => 0x30,
        '.' => 0x2F, ',' => 0x2B, ';' => 0x29, '/' => 0x2C,
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
pub struct NSPasteboardClipboard;

#[cfg(target_os = "macos")]
#[async_trait]
impl Clipboard for NSPasteboardClipboard {
    async fn read(&self) -> Option<String> {
        platform_macos::pasteboard::read_pasteboard_string()
    }
    async fn write(&self, s: &str) {
        platform_macos::pasteboard::write_pasteboard_string(s);
    }
}
```

(If `platform_macos::pasteboard::write_pasteboard_string` doesn't exist yet, add it as a small wrapper around `NSPasteboard generalPasteboard setString:forType:`.)

- [ ] **Step 2: Build**

```bash
cargo build -p feature-input-services
```
Expected: success on macOS.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-input-services/src/snippet/expander.rs
git commit -m "feat(input-services): macOS CGEventPoster + NSPasteboardClipboard impls"
```

### Task 2.6: `SnippetEngine`

**Files:**
- Create: `crates/feature-input-services/src/snippet/engine.rs`

- [ ] **Step 1: Implement + tests**

```rust
use super::detector::{DetectionResult, TriggerDetector};
use super::expander::Expander;
use super::types::SnippetTable;
use parking_lot::Mutex;
use platform_macos::event_tap::{RawKeyEvent, TapAction};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct SnippetEngine {
    detector: Mutex<TriggerDetector>,
    expand_tx: mpsc::UnboundedSender<DetectionResult>,
}

impl SnippetEngine {
    pub fn start(
        table_rx: watch::Receiver<Arc<SnippetTable>>,
        expander: Arc<Expander>,
    ) -> Arc<Self> {
        let detector = Mutex::new(TriggerDetector::new(table_rx));
        let (tx, mut rx) = mpsc::unbounded_channel::<DetectionResult>();
        tokio::spawn(async move {
            while let Some(det) = rx.recv().await { expander.expand(det).await; }
        });
        Arc::new(Self { detector, expand_tx: tx })
    }

    pub fn on_event(&self, ev: &RawKeyEvent) -> TapAction {
        if !ev.is_down { return TapAction::Pass; }
        let c = match ev.characters.as_deref().and_then(|s| s.chars().next()) {
            Some(c) => c, None => return TapAction::Pass,
        };
        let mut det = self.detector.lock();
        match det.on_char(c) {
            None => TapAction::Pass,
            Some(result) => {
                let _ = self.expand_tx.send(result);
                TapAction::Suppress
            }
        }
    }
}
```

(Tests at this layer are integration-shaped — covered already by `detector` + `expander` unit tests. Skip dedicated unit test for the engine itself.)

- [ ] **Step 2: Build**

```bash
cargo build -p feature-input-services
```

- [ ] **Step 3: Commit**

```bash
git add crates/feature-input-services/src/snippet/engine.rs
git commit -m "feat(input-services): SnippetEngine wiring detector + expander via mpsc"
```

### Task 2.7: `SnippetRepo` with watch broadcaster

**Files:**
- Create: `crates/feature-input-services/src/repos/mod.rs`
- Create: `crates/feature-input-services/src/repos/snippet_repo.rs`

- [ ] **Step 1: Mod.rs**

```rust
pub mod snippet_repo;
```

- [ ] **Step 2: snippet_repo.rs**

```rust
use crate::snippet::types::{Snippet, SnippetTable};
use jiff::Timestamp;
use std::collections::HashMap;
use std::sync::Arc;
use storage::StoragePool;
use tokio::sync::watch;

pub struct SnippetRepo {
    pool: StoragePool,
    table_tx: watch::Sender<Arc<SnippetTable>>,
    table_rx: watch::Receiver<Arc<SnippetTable>>,
    marker: char,
}

impl SnippetRepo {
    pub async fn new(pool: StoragePool, marker: char) -> Result<Self, sqlx::Error> {
        let (tx, rx) = watch::channel(Arc::new(SnippetTable { marker, ..Default::default() }));
        let repo = Self { pool, table_tx: tx, table_rx: rx, marker };
        repo.rebuild_table().await?;
        Ok(repo)
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<SnippetTable>> { self.table_rx.clone() }

    pub async fn list(&self) -> Result<Vec<Snippet>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SnippetRow>(
            "SELECT id, trigger, body, description, enabled, created_at, updated_at FROM snippets ORDER BY trigger"
        ).fetch_all(self.pool.pool()).await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn create(
        &self, trigger: String, body: String, description: Option<String>,
    ) -> Result<Snippet, sqlx::Error> {
        let now = Timestamp::now().to_string();
        let id = sqlx::query("INSERT INTO snippets (trigger, body, description, enabled, created_at, updated_at) VALUES (?,?,?,1,?,?)")
            .bind(&trigger).bind(&body).bind(&description).bind(&now).bind(&now)
            .execute(self.pool.pool()).await?
            .last_insert_rowid();
        self.rebuild_table().await?;
        Ok(Snippet {
            id, trigger, body, description, enabled: true,
            created_at: now.parse().unwrap(), updated_at: now.parse().unwrap(),
        })
    }

    pub async fn update(
        &self, id: i64, trigger: String, body: String, description: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let now = Timestamp::now().to_string();
        sqlx::query("UPDATE snippets SET trigger=?, body=?, description=?, updated_at=? WHERE id=?")
            .bind(&trigger).bind(&body).bind(&description).bind(&now).bind(id)
            .execute(self.pool.pool()).await?;
        self.rebuild_table().await
    }

    pub async fn delete(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM snippets WHERE id=?")
            .bind(id).execute(self.pool.pool()).await?;
        self.rebuild_table().await
    }

    pub async fn toggle(&self, id: i64, enabled: bool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE snippets SET enabled=?, updated_at=? WHERE id=?")
            .bind(enabled as i32).bind(Timestamp::now().to_string()).bind(id)
            .execute(self.pool.pool()).await?;
        self.rebuild_table().await
    }

    async fn rebuild_table(&self) -> Result<(), sqlx::Error> {
        let snippets = self.list().await?;
        let mut by_trigger = HashMap::new();
        let mut longest = 0;
        for s in snippets {
            longest = longest.max(s.trigger.chars().count());
            by_trigger.insert(s.trigger.clone(), s);
        }
        self.table_tx.send_replace(Arc::new(SnippetTable {
            by_trigger, longest_trigger_len: longest, marker: self.marker,
        }));
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct SnippetRow {
    id: i64,
    trigger: String,
    body: String,
    description: Option<String>,
    enabled: i64,
    created_at: String,
    updated_at: String,
}

impl From<SnippetRow> for Snippet {
    fn from(r: SnippetRow) -> Self {
        Snippet {
            id: r.id, trigger: r.trigger, body: r.body, description: r.description,
            enabled: r.enabled != 0,
            created_at: r.created_at.parse().unwrap_or_else(|_| Timestamp::now()),
            updated_at: r.updated_at.parse().unwrap_or_else(|_| Timestamp::now()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh_pool() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        sqlx::query(include_str!("../../migrations/001_snippets.sql"))
            .execute(pool.pool()).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_then_list_returns_snippet() {
        let pool = fresh_pool().await;
        let repo = SnippetRepo::new(pool, ';').await.unwrap();
        let s = repo.create(";email".into(), "me@x.com".into(), None).await.unwrap();
        assert_eq!(s.trigger, ";email");
        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn rebuild_broadcasts_via_watch() {
        let pool = fresh_pool().await;
        let repo = SnippetRepo::new(pool, ';').await.unwrap();
        let mut rx = repo.subscribe();
        let t0 = rx.borrow().clone();
        assert_eq!(t0.by_trigger.len(), 0);
        repo.create(";a".into(), "A".into(), None).await.unwrap();
        let t1 = rx.borrow_and_update().clone();
        assert_eq!(t1.by_trigger.len(), 1);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-input-services repos
```
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-input-services/src/repos/
git commit -m "feat(input-services): SnippetRepo with watch-channel hot reload"
```

### Task 2.8: `InputServicesConfig` in config crate

**Files:**
- Modify: `crates/config/src/schema/mod.rs` (or wherever the top-level Config lives)

- [ ] **Step 1: Add config struct**

Add a new `input_services.rs` under `config/src/schema/`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InputServicesConfig {
    #[serde(default)]
    pub snippets: SnippetsConfig,
    #[serde(default)]
    pub hyper_key: HyperKeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_marker")]
    pub marker: char,
}

impl Default for SnippetsConfig {
    fn default() -> Self { Self { enabled: false, marker: ';' } }
}

fn default_marker() -> char { ';' }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperKeyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_caps_lock")]
    pub source_keycode: u16,
    #[serde(default = "default_hyper_flags")]
    pub hyper_modifier_flags: u64,
    #[serde(default = "default_tap_action")]
    pub tap_action: TapActionConfig,
    #[serde(default = "default_tap_timeout")]
    pub tap_timeout_ms: u64,
}

impl Default for HyperKeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source_keycode: 0x39,
            hyper_modifier_flags: (1<<17)|(1<<18)|(1<<19)|(1<<20),
            tap_action: TapActionConfig::Escape,
            tap_timeout_ms: 300,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TapActionConfig { Escape, Nothing, Original }

fn default_caps_lock() -> u16 { 0x39 }
fn default_hyper_flags() -> u64 { (1<<17)|(1<<18)|(1<<19)|(1<<20) }
fn default_tap_action() -> TapActionConfig { TapActionConfig::Escape }
fn default_tap_timeout() -> u64 { 300 }
```

Add to top-level `Config`:
```rust
#[serde(default)]
pub input_services: InputServicesConfig,
```

- [ ] **Step 2: Build**

```bash
cargo build -p config -p feature-input-services
```

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/
git commit -m "feat(config): InputServicesConfig for snippets + hyper key"
```

### Task 2.9: `InputServicesFeature` (FeaturePackage impl)

**Files:**
- Modify: `crates/feature-input-services/src/lib.rs`

- [ ] **Step 1: Add FeaturePackage impl**

```rust
use async_trait::async_trait;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, FeatureHealth, HealthContext};

pub struct InputServicesFeature;

#[async_trait]
impl FeaturePackage for InputServicesFeature {
    fn name(&self) -> &'static str { "input_services" }
    fn config_key(&self) -> &'static str { "inputServices" }
    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            version: 1,
            name: "snippets",
            sql: include_str!("../migrations/001_snippets.sql"),
        }]
    }
    fn tools(&self) -> Vec<DynTool> { vec![] }
    async fn health(&self, _ctx: &HealthContext<'_>) -> FeatureHealth {
        FeatureHealth::ok("input_services")
    }
}
```

(If `FeatureMigration` field names differ in the actual `tools_core` crate, mirror exactly.)

- [ ] **Step 2: Build**

```bash
cargo build -p feature-input-services
```

- [ ] **Step 3: Commit**

```bash
git add crates/feature-input-services/src/lib.rs
git commit -m "feat(input-services): FeaturePackage impl"
```

### Task 2.10: App-core init

**Files:**
- Create: `crates/app-core/src/init/input_services.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/state.rs` (or wherever AppCore is)

- [ ] **Step 1: init_input_services**

```rust
use std::sync::Arc;
use config::InputServicesConfig;
use feature_input_services::{
    snippet::{engine::SnippetEngine, expander::{Expander, CGEventPoster, NSPasteboardClipboard}},
    SnippetRepo,
};
use platform_macos::event_tap::{
    check_input_monitoring, EventTap, EventTapConfig, PermissionStatus, TapAction,
    TapEventType, TapMode,
};
use storage::StoragePool;

pub struct InputServicesHandle {
    pub event_tap: Option<EventTap>,
    pub snippet_repo: Arc<SnippetRepo>,
    pub snippet_engine: Arc<SnippetEngine>,
}

pub async fn init_input_services(
    pool: StoragePool, cfg: &InputServicesConfig,
) -> Result<InputServicesHandle, common::KlyntbotError> {
    let snippet_repo = Arc::new(SnippetRepo::new(pool, cfg.snippets.marker).await
        .map_err(|e| common::KlyntbotError::other(e.to_string()))?);
    let expander = Arc::new(Expander::new(
        Arc::new(CGEventPoster), Arc::new(NSPasteboardClipboard),
    ));
    let snippet_engine = SnippetEngine::start(snippet_repo.subscribe(), expander);

    if !cfg.snippets.enabled {
        return Ok(InputServicesHandle { event_tap: None, snippet_repo, snippet_engine });
    }
    if check_input_monitoring() != PermissionStatus::Granted {
        tracing::info!("input_services: enabled but permission not granted; deferring tap start");
        return Ok(InputServicesHandle { event_tap: None, snippet_repo, snippet_engine });
    }

    let snip_for_handler = snippet_engine.clone();
    let event_tap = EventTap::start(
        EventTapConfig {
            mode: TapMode::Modify,
            event_types: vec![TapEventType::KeyDown, TapEventType::KeyUp, TapEventType::FlagsChanged],
        },
        move |ev| snip_for_handler.on_event(&ev),
    ).map_err(|e| common::KlyntbotError::other(e.to_string()))?;
    tracing::info!("input_services.tap.started");

    Ok(InputServicesHandle { event_tap: Some(event_tap), snippet_repo, snippet_engine })
}
```

- [ ] **Step 2: Wire into init/mod.rs**

```rust
pub mod input_services;
```

In whatever the app-init function is, call:
```rust
let input_services = init_input_services(pool.clone(), &config.input_services).await?;
```

Store on `AppCore`:
```rust
pub input_services: input_services::InputServicesHandle,
```

- [ ] **Step 3: Build**

```bash
cargo build -p app-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ crates/app-core/src/state.rs
git commit -m "feat(app-core): wire feature-input-services into init"
```

### Task 2.11: Tauri commands

**Files:**
- Create: `crates/desktop/src/commands/input_services.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Implement commands**

```rust
use std::sync::Arc;
use desktop_shared::errors::ApiError;
use feature_input_services::Snippet;
use platform_macos::event_tap::{check_input_monitoring, request_input_monitoring, PermissionStatus};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_core::AppCore;

#[derive(Serialize, Deserialize)]
pub struct InputServicesStatus {
    pub event_tap_alive: bool,
    pub permission: PermissionStatus,
    pub snippet_count: usize,
    pub hyper_key_enabled: bool,
}

#[tauri::command]
pub async fn input_services_snippet_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<Snippet>, ApiError> {
    state.input_services.snippet_repo.list().await
        .map_err(|e| ApiError::new("DB", e.to_string()))
}

#[tauri::command]
pub async fn input_services_snippet_create(
    state: State<'_, Arc<AppCore>>, trigger: String, body: String, description: Option<String>,
) -> Result<Snippet, ApiError> {
    state.input_services.snippet_repo.create(trigger, body, description).await
        .map_err(|e| ApiError::new("DB", e.to_string()))
}

#[tauri::command]
pub async fn input_services_snippet_update(
    state: State<'_, Arc<AppCore>>, id: i64, trigger: String, body: String, description: Option<String>,
) -> Result<(), ApiError> {
    state.input_services.snippet_repo.update(id, trigger, body, description).await
        .map_err(|e| ApiError::new("DB", e.to_string()))
}

#[tauri::command]
pub async fn input_services_snippet_delete(
    state: State<'_, Arc<AppCore>>, id: i64,
) -> Result<(), ApiError> {
    state.input_services.snippet_repo.delete(id).await
        .map_err(|e| ApiError::new("DB", e.to_string()))
}

#[tauri::command]
pub async fn input_services_snippet_toggle(
    state: State<'_, Arc<AppCore>>, id: i64, enabled: bool,
) -> Result<(), ApiError> {
    state.input_services.snippet_repo.toggle(id, enabled).await
        .map_err(|e| ApiError::new("DB", e.to_string()))
}

#[tauri::command]
pub async fn input_services_permission_status() -> Result<PermissionStatus, ApiError> {
    Ok(check_input_monitoring())
}

#[tauri::command]
pub async fn input_services_request_permission() -> Result<(), ApiError> {
    request_input_monitoring();
    Ok(())
}

#[tauri::command]
pub async fn input_services_status(state: State<'_, Arc<AppCore>>) -> Result<InputServicesStatus, ApiError> {
    let alive = state.input_services.event_tap.as_ref().map(|t| t.is_alive()).unwrap_or(false);
    let count = state.input_services.snippet_repo.list().await
        .map_err(|e| ApiError::new("DB", e.to_string()))?
        .len();
    Ok(InputServicesStatus {
        event_tap_alive: alive,
        permission: check_input_monitoring(),
        snippet_count: count,
        hyper_key_enabled: false, // PR-3 sets this
    })
}

pub(crate) const DEV_COMMANDS: &[&str] = &[
    "input_services_snippet_list",
    "input_services_snippet_create",
    "input_services_snippet_update",
    "input_services_snippet_delete",
    "input_services_snippet_toggle",
    "input_services_permission_status",
    "input_services_request_permission",
    "input_services_status",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str, core: &Arc<AppCore>, body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "input_services_snippet_list" =>
            dev::val(core.input_services.snippet_repo.list().await
                .map_err(|e| ApiError::new("DB", e.to_string()))),
        "input_services_snippet_create" => {
            let trigger = dev::get(body, "trigger").unwrap_or_default();
            let bd = dev::get(body, "body").unwrap_or_default();
            let desc = dev::get::<String>(body, "description").ok();
            dev::val(core.input_services.snippet_repo.create(trigger, bd, desc).await
                .map_err(|e| ApiError::new("DB", e.to_string())))
        },
        "input_services_snippet_delete" => {
            let id = dev::get::<i64>(body, "id").unwrap_or(0);
            dev::val(core.input_services.snippet_repo.delete(id).await
                .map_err(|e| ApiError::new("DB", e.to_string())))
        },
        "input_services_permission_status" => dev::val(Ok::<_, ApiError>(check_input_monitoring())),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register in commands/mod.rs**

```rust
pub mod input_services;
```

- [ ] **Step 3: Add to invoke_handler in lib.rs**

```rust
.invoke_handler(tauri::generate_handler![
    // ...existing
    commands::input_services::input_services_snippet_list,
    commands::input_services::input_services_snippet_create,
    commands::input_services::input_services_snippet_update,
    commands::input_services::input_services_snippet_delete,
    commands::input_services::input_services_snippet_toggle,
    commands::input_services::input_services_permission_status,
    commands::input_services::input_services_request_permission,
    commands::input_services::input_services_status,
])
```

- [ ] **Step 4: Add to dev_server coverage**

In `crates/desktop/src/dev_server/mod.rs`, add `commands::input_services::DEV_COMMANDS` to the merged list and `dispatch_dev` chain.

- [ ] **Step 5: Build**

```bash
cargo build -p desktop
cargo nextest run -p desktop
```

- [ ] **Step 6: Commit**

```bash
git add -A crates/desktop
git commit -m "feat(desktop): input_services Tauri commands + dev coverage"
```

### Task 2.12: Settings UI — snippets tab + permission banner

**Files:**
- Create: `desktop-ui/src/features/settings/InputServicesPage.tsx`
- Create: `desktop-ui/src/features/settings/components/SnippetsTab.tsx`
- Create: `desktop-ui/src/features/settings/components/PermissionBanner.tsx`
- Modify: `desktop-ui/src/features/settings/SettingsNav.tsx` (or equivalent)

- [ ] **Step 1: PermissionBanner**

```tsx
import React from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useQuery } from '@/shared/hooks/useQuery'

export function PermissionBanner() {
  const { data: status } = useQuery('input_services_permission_status')
  if (status === 'granted') {
    return <div className="rounded bg-green-900/40 text-green-200 px-3 py-2 text-sm">Input Monitoring granted.</div>
  }
  return (
    <div className="rounded bg-amber-900/40 text-amber-200 px-3 py-2 text-sm flex items-center justify-between">
      <span>Klynt needs Input Monitoring to expand snippets and remap Hyper Key.</span>
      <button
        onClick={() => invoke('input_services_request_permission')}
        className="px-2 py-1 rounded bg-amber-200 text-amber-900"
      >Open System Settings</button>
    </div>
  )
}
```

- [ ] **Step 2: SnippetsTab**

```tsx
import React, { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useQuery, useMutation } from '@/shared/hooks/useQuery'

interface Snippet { id: number; trigger: string; body: string; description?: string; enabled: boolean }

export function SnippetsTab() {
  const { data: snippets, refetch } = useQuery<Snippet[]>('input_services_snippet_list')
  const [creating, setCreating] = useState(false)
  const [draft, setDraft] = useState({ trigger: ';', body: '', description: '' })

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button onClick={() => setCreating(true)} className="px-3 py-1 rounded bg-accent">+ New Snippet</button>
      </div>
      <table className="w-full text-sm">
        <thead><tr className="text-left text-muted">
          <th>Trigger</th><th>Body</th><th>Enabled</th><th></th>
        </tr></thead>
        <tbody>
          {snippets?.map((s) => (
            <tr key={s.id} className="border-t border-border">
              <td className="font-mono">{s.trigger}</td>
              <td className="truncate max-w-md">{s.body}</td>
              <td>
                <input type="checkbox" checked={s.enabled} onChange={async (e) => {
                  await invoke('input_services_snippet_toggle', { id: s.id, enabled: e.target.checked })
                  refetch()
                }} />
              </td>
              <td>
                <button onClick={async () => {
                  await invoke('input_services_snippet_delete', { id: s.id })
                  refetch()
                }}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {creating && (
        <div className="glass-panel p-4 rounded space-y-2">
          <input
            placeholder=";trigger"
            value={draft.trigger}
            onChange={(e) => setDraft({ ...draft, trigger: e.target.value })}
            className="w-full px-2 py-1 rounded bg-surface-base font-mono"
          />
          <textarea
            placeholder="Body"
            value={draft.body}
            onChange={(e) => setDraft({ ...draft, body: e.target.value })}
            className="w-full px-2 py-1 rounded bg-surface-base font-mono h-32"
          />
          <input
            placeholder="Description (optional)"
            value={draft.description}
            onChange={(e) => setDraft({ ...draft, description: e.target.value })}
            className="w-full px-2 py-1 rounded bg-surface-base"
          />
          <div className="flex justify-end gap-2">
            <button onClick={() => setCreating(false)}>Cancel</button>
            <button
              className="px-3 py-1 rounded bg-accent"
              disabled={!draft.trigger.startsWith(';') || !draft.body}
              onClick={async () => {
                await invoke('input_services_snippet_create', draft)
                setCreating(false)
                setDraft({ trigger: ';', body: '', description: '' })
                refetch()
              }}
            >Save</button>
          </div>
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 3: InputServicesPage**

```tsx
import React from 'react'
import { PermissionBanner } from './components/PermissionBanner'
import { SnippetsTab } from './components/SnippetsTab'

export function InputServicesPage() {
  return (
    <div className="space-y-4 p-4">
      <h2 className="text-lg font-semibold">Input Services</h2>
      <PermissionBanner />
      <SnippetsTab />
    </div>
  )
}
```

- [ ] **Step 4: Add to SettingsNav**

Find the settings router/nav. Add a route entry pointing to `InputServicesPage` with label "Input Services".

- [ ] **Step 5: Build + lint**

```bash
cd desktop-ui && bun run lint && bun run build
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/settings/
git commit -m "feat(desktop-ui): InputServicesPage with snippets tab and permission banner"
```

### Task 2.13: PR-2 final gates

- [ ] **Step 1: All gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

1. Enable snippets in `~/.klyntbot-dev/config.json`: `{ "inputServices": { "snippets": { "enabled": true } } }`.
2. Start `cargo tauri dev`. Grant Input Monitoring on first launch.
3. Open Settings → Input Services. Create snippet `;email` → `me@example.com`.
4. Open TextEdit. Type `;email ` → verify expansion.

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(input-services): snippet expansion + settings UI" --body "Implements PR-2."
```

---

# PR-3 — Hyper Key engine + UI

### Task 3.1: `HyperKeyEngine`

**Files:**
- Create: `crates/feature-input-services/src/hyper_key/mod.rs`
- Create: `crates/feature-input-services/src/hyper_key/config.rs`
- Create: `crates/feature-input-services/src/hyper_key/engine.rs`

- [ ] **Step 1: Mod + config**

```rust
// mod.rs
pub mod config;
pub mod engine;
pub use config::TapActionConfig;
pub use engine::HyperKeyEngine;

// config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperKeyConfig {
    pub enabled: bool,
    pub source_keycode: u16,
    pub hyper_modifier_flags: u64,
    pub tap_action: TapActionConfig,
    pub tap_timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TapActionConfig { Escape, Nothing, Original }

impl From<config::HyperKeyConfig> for HyperKeyConfig {
    fn from(c: config::HyperKeyConfig) -> Self {
        Self {
            enabled: c.enabled,
            source_keycode: c.source_keycode,
            hyper_modifier_flags: c.hyper_modifier_flags,
            tap_action: match c.tap_action {
                config::TapActionConfig::Escape => TapActionConfig::Escape,
                config::TapActionConfig::Nothing => TapActionConfig::Nothing,
                config::TapActionConfig::Original => TapActionConfig::Original,
            },
            tap_timeout_ms: c.tap_timeout_ms,
        }
    }
}
```

- [ ] **Step 2: engine.rs with tests**

```rust
use super::config::{HyperKeyConfig, TapActionConfig};
use parking_lot::{Mutex, RwLock};
use platform_macos::event_tap::{RawKeyEvent, TapAction};
use std::time::{Duration, Instant};

pub struct HyperKeyEngine {
    cfg: RwLock<HyperKeyConfig>,
    state: Mutex<HyperState>,
}

#[derive(Default, Debug, Clone)]
struct HyperState {
    held: bool,
    held_since: Option<Instant>,
    used_with_other_key: bool,
}

impl HyperKeyEngine {
    pub fn new(cfg: HyperKeyConfig) -> Self {
        Self { cfg: RwLock::new(cfg), state: Mutex::new(HyperState::default()) }
    }

    pub fn update_config(&self, cfg: HyperKeyConfig) {
        *self.cfg.write() = cfg;
        *self.state.lock() = HyperState::default();
    }

    pub fn on_event(&self, ev: &RawKeyEvent) -> TapAction {
        let cfg = self.cfg.read();
        if !cfg.enabled { return TapAction::Pass; }
        let mut st = self.state.lock();

        if ev.keycode == cfg.source_keycode && ev.is_down {
            st.held = true;
            st.held_since = Some(Instant::now());
            st.used_with_other_key = false;
            return TapAction::Suppress;
        }

        if ev.keycode == cfg.source_keycode && !ev.is_down {
            let elapsed = st.held_since.map(|t| t.elapsed()).unwrap_or(Duration::MAX);
            let was_tap = elapsed < Duration::from_millis(cfg.tap_timeout_ms) && !st.used_with_other_key;
            st.held = false;
            st.held_since = None;
            st.used_with_other_key = false;
            return if was_tap { tap_action_to_events(cfg.tap_action) } else { TapAction::Suppress };
        }

        if st.held {
            st.used_with_other_key = true;
            let new_flags = ev.flags | cfg.hyper_modifier_flags;
            return TapAction::Replace(vec![RawKeyEvent { flags: new_flags, ..ev.clone() }]);
        }
        TapAction::Pass
    }
}

fn tap_action_to_events(a: TapActionConfig) -> TapAction {
    match a {
        TapActionConfig::Escape => TapAction::Replace(vec![
            RawKeyEvent { keycode: 0x35, is_down: true,  flags: 0, characters: None, timestamp_ns: 0 },
            RawKeyEvent { keycode: 0x35, is_down: false, flags: 0, characters: None, timestamp_ns: 0 },
        ]),
        TapActionConfig::Nothing => TapAction::Suppress,
        TapActionConfig::Original => TapAction::Pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> HyperKeyConfig {
        HyperKeyConfig {
            enabled: true,
            source_keycode: 0x39,
            hyper_modifier_flags: (1<<17)|(1<<18)|(1<<19)|(1<<20),
            tap_action: TapActionConfig::Escape,
            tap_timeout_ms: 300,
        }
    }
    fn ev(kc: u16, down: bool, flags: u64) -> RawKeyEvent {
        RawKeyEvent { keycode: kc, is_down: down, flags, characters: None, timestamp_ns: 0 }
    }

    #[test]
    fn source_down_suppresses() {
        let e = HyperKeyEngine::new(cfg());
        assert!(matches!(e.on_event(&ev(0x39, true, 0)), TapAction::Suppress));
    }

    #[test]
    fn other_key_while_held_replaces_with_hyper_flags() {
        let e = HyperKeyEngine::new(cfg());
        e.on_event(&ev(0x39, true, 0));
        let action = e.on_event(&ev(0x25, true, 0)); // 'L' down
        match action {
            TapAction::Replace(ref evs) if evs.len() == 1 => {
                assert_eq!(evs[0].keycode, 0x25);
                assert_eq!(evs[0].flags, (1<<17)|(1<<18)|(1<<19)|(1<<20));
            }
            other => panic!("expected Replace, got {:?}", other),
        }
    }

    #[test]
    fn quick_tap_emits_escape() {
        let e = HyperKeyEngine::new(cfg());
        e.on_event(&ev(0x39, true, 0));
        let action = e.on_event(&ev(0x39, false, 0));
        match action {
            TapAction::Replace(ref evs) => {
                assert_eq!(evs.len(), 2);
                assert_eq!(evs[0].keycode, 0x35);  // Escape down
                assert!(evs[0].is_down);
                assert_eq!(evs[1].keycode, 0x35);  // Escape up
                assert!(!evs[1].is_down);
            }
            other => panic!("expected Replace, got {:?}", other),
        }
    }

    #[test]
    fn hold_with_other_key_no_escape_on_release() {
        let e = HyperKeyEngine::new(cfg());
        e.on_event(&ev(0x39, true, 0));
        e.on_event(&ev(0x25, true, 0));
        e.on_event(&ev(0x25, false, 0));
        let action = e.on_event(&ev(0x39, false, 0));
        assert!(matches!(action, TapAction::Suppress));
    }

    #[test]
    fn disabled_engine_passes_everything() {
        let mut c = cfg(); c.enabled = false;
        let e = HyperKeyEngine::new(c);
        assert!(matches!(e.on_event(&ev(0x39, true, 0)), TapAction::Pass));
    }

    #[test]
    fn tap_action_nothing_suppresses() {
        let mut c = cfg(); c.tap_action = TapActionConfig::Nothing;
        let e = HyperKeyEngine::new(c);
        e.on_event(&ev(0x39, true, 0));
        assert!(matches!(e.on_event(&ev(0x39, false, 0)), TapAction::Suppress));
    }
}
```

- [ ] **Step 3: Re-export**

In `crates/feature-input-services/src/lib.rs`:
```rust
pub mod hyper_key;
pub use hyper_key::HyperKeyEngine;
```

- [ ] **Step 4: Tests**

```bash
cargo nextest run -p feature-input-services hyper_key
```
Expected: 6 pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-input-services/src/hyper_key/ crates/feature-input-services/src/lib.rs
git commit -m "feat(input-services): HyperKeyEngine state machine with full unit coverage"
```

### Task 3.2: hidutil Caps Lock remap

**Files:**
- Create: `crates/feature-input-services/src/hyper_key/caps_lock.rs`

- [ ] **Step 1: Implement**

```rust
//! Caps Lock special handling.
//!
//! macOS handles Caps Lock at the HID layer before event taps see it.
//! We use `hidutil` to remap Caps Lock (0x39) to F18 (0x6B) so events flow
//! through the tap normally. The mapping is per-session and resets at logout.

use std::process::Command;

const CAPS_LOCK_SRC: &str = "0x700000039";
const F18_DST: &str = "0x70000006D";  // F18 keycode in usage page

pub fn enable_caps_lock_remap() -> std::io::Result<()> {
    let mapping = format!(
        r#"{{"UserKeyMapping":[{{"HIDKeyboardModifierMappingSrc":{CAPS_LOCK_SRC},"HIDKeyboardModifierMappingDst":{F18_DST}}}]}}"#
    );
    Command::new("hidutil").args(["property", "--set", &mapping]).status()?;
    Ok(())
}

pub fn disable_caps_lock_remap() -> std::io::Result<()> {
    Command::new("hidutil").args(["property", "--set", r#"{"UserKeyMapping":[]}"#]).status()?;
    Ok(())
}
```

In `hyper_key/mod.rs`:
```rust
pub mod caps_lock;
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p feature-input-services
git add crates/feature-input-services/src/hyper_key/caps_lock.rs crates/feature-input-services/src/hyper_key/mod.rs
git commit -m "feat(input-services): hidutil Caps Lock → F18 remap helpers"
```

### Task 3.3: Wire HyperKey into app-core

**Files:**
- Modify: `crates/app-core/src/init/input_services.rs`

- [ ] **Step 1: Construct engine, chain into handler**

```rust
use feature_input_services::{HyperKeyEngine, hyper_key::caps_lock};

// In init_input_services:
let hyper_engine = Arc::new(HyperKeyEngine::new(cfg.hyper_key.clone().into()));

if cfg.hyper_key.enabled && cfg.hyper_key.source_keycode == 0x39 {
    if let Err(e) = caps_lock::enable_caps_lock_remap() {
        tracing::warn!("caps_lock remap failed: {e}");
    }
}

let snip_for_handler = snippet_engine.clone();
let hyper_for_handler = hyper_engine.clone();
let event_tap = EventTap::start(
    EventTapConfig { /* same */ },
    move |ev| match hyper_for_handler.on_event(&ev) {
        TapAction::Pass => snip_for_handler.on_event(&ev),
        other => other,
    },
)?;

// Add to InputServicesHandle:
pub hyper_engine: Arc<HyperKeyEngine>,
```

Update startup condition to also check `cfg.hyper_key.enabled`:
```rust
if !cfg.snippets.enabled && !cfg.hyper_key.enabled {
    return Ok(InputServicesHandle { event_tap: None, ... });
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p app-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/input_services.rs
git commit -m "feat(app-core): wire HyperKeyEngine into event-tap handler chain"
```

### Task 3.4: Tauri command + frontend tab

**Files:**
- Modify: `crates/desktop/src/commands/input_services.rs`
- Create: `desktop-ui/src/features/settings/components/HyperKeyTab.tsx`
- Modify: `desktop-ui/src/features/settings/InputServicesPage.tsx`

- [ ] **Step 1: Add command**

```rust
#[tauri::command]
pub async fn input_services_set_hyper_key(
    state: State<'_, Arc<AppCore>>,
    enabled: bool, source_keycode: u16, tap_action: feature_input_services::hyper_key::TapActionConfig,
) -> Result<(), ApiError> {
    state.input_services.hyper_engine.update_config(
        feature_input_services::hyper_key::HyperKeyConfig {
            enabled, source_keycode,
            hyper_modifier_flags: (1<<17)|(1<<18)|(1<<19)|(1<<20),
            tap_action, tap_timeout_ms: 300,
        }
    );
    Ok(())
}
```

Add to `DEV_COMMANDS`, `dispatch_dev`, and `invoke_handler!`.

- [ ] **Step 2: HyperKeyTab**

```tsx
import React, { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export function HyperKeyTab() {
  const [enabled, setEnabled] = useState(false)
  const [sourceKey, setSourceKey] = useState(0x39)  // Caps Lock
  const [tapAction, setTapAction] = useState<'escape'|'nothing'|'original'>('escape')

  const save = async () => {
    await invoke('input_services_set_hyper_key', {
      enabled, sourceKeycode: sourceKey, tapAction,
    })
  }

  return (
    <div className="space-y-3">
      <label className="flex items-center gap-2">
        <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
        Enable Hyper Key
      </label>
      <label className="block">
        Source key:
        <select value={sourceKey} onChange={(e) => setSourceKey(parseInt(e.target.value))} className="ml-2 px-2 py-1 rounded bg-surface-base">
          <option value={0x39}>Caps Lock</option>
          <option value={0x36}>Right Command</option>
          <option value={0x3D}>Right Option</option>
          <option value={0x3E}>Right Control</option>
        </select>
      </label>
      <label className="block">
        Tap action:
        <select value={tapAction} onChange={(e) => setTapAction(e.target.value as any)} className="ml-2 px-2 py-1 rounded bg-surface-base">
          <option value="escape">Escape</option>
          <option value="nothing">Nothing</option>
          <option value="original">Original Key</option>
        </select>
      </label>
      <button onClick={save} className="px-3 py-1 rounded bg-accent">Save</button>
      <p className="text-muted text-xs">Note: changing source key requires restarting Klynt.</p>
    </div>
  )
}
```

- [ ] **Step 3: InputServicesPage tabs**

```tsx
import React, { useState } from 'react'
import { PermissionBanner } from './components/PermissionBanner'
import { SnippetsTab } from './components/SnippetsTab'
import { HyperKeyTab } from './components/HyperKeyTab'

export function InputServicesPage() {
  const [tab, setTab] = useState<'snippets'|'hyper'>('snippets')
  return (
    <div className="space-y-4 p-4">
      <h2 className="text-lg font-semibold">Input Services</h2>
      <PermissionBanner />
      <div className="flex gap-2 border-b border-border">
        <button onClick={() => setTab('snippets')} className={tab==='snippets' ? 'border-b-2 border-accent' : ''}>Snippets</button>
        <button onClick={() => setTab('hyper')} className={tab==='hyper' ? 'border-b-2 border-accent' : ''}>Hyper Key</button>
      </div>
      {tab === 'snippets' ? <SnippetsTab /> : <HyperKeyTab />}
    </div>
  )
}
```

- [ ] **Step 4: Build + lint**

```bash
cargo build -p desktop
cd desktop-ui && bun run lint && bun run build
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(input-services): Hyper Key settings tab + set_hyper_key command"
```

### Task 3.5: PR-3 gates

- [ ] **Step 1: All gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

1. Enable Hyper Key in settings.
2. Tap Caps Lock alone → Escape sent (verify in Key Codes app).
3. Hold Caps Lock + L → Cmd+Ctrl+Opt+Shift+L sent.
4. Disable, restart, verify Caps Lock returns to default.

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(input-services): Hyper Key engine + settings tab" --body "Implements PR-3."
```

---

# PR-4 — Permission polish + failure recovery

### Task 4.1: TapDied event surfacing

**Files:**
- Modify: `crates/platform-macos/src/event_tap/worker.rs`
- Modify: `crates/platform-macos/src/event_tap/mod.rs`

- [ ] **Step 1: Add TapDied callback parameter**

In `EventTap::start`, accept an optional `on_died: impl Fn() + Send + Sync + 'static` parameter. The worker stores it and invokes it from the C trampoline when the disabled-event types fire.

```rust
// mod.rs additions:
impl EventTap {
    pub fn start_with_lifecycle(
        cfg: EventTapConfig,
        handler: impl Fn(RawKeyEvent) -> TapAction + Send + Sync + 'static,
        on_died: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, EventTapError> { /* ... */ }
}
```

Keep the original `start()` as a thin wrapper that passes a no-op `on_died`.

In `worker.rs`, extend `HandlerCtx`:
```rust
struct HandlerCtx {
    handler: Handler,
    alive: Arc<AtomicBool>,
    on_died: Arc<dyn Fn() + Send + Sync>,
}
```

In `c_callback`, when detecting tap-disabled events, call `(ctx.on_died)()` after setting alive=false.

- [ ] **Step 2: Build + commit**

```bash
cargo build -p platform-macos
git add crates/platform-macos/src/event_tap/
git commit -m "feat(event_tap): on_died callback for tap lifecycle observability"
```

### Task 4.2: Desktop notification on tap death

**Files:**
- Modify: `crates/app-core/src/init/input_services.rs`

- [ ] **Step 1: Add notifier**

```rust
use tauri::AppHandle; // requires app handle threaded into init

let app_handle: AppHandle = /* obtain from caller */;
let hyper_enabled = cfg.hyper_key.enabled;
let event_tap = EventTap::start_with_lifecycle(
    EventTapConfig { /* ... */ },
    move |ev| { /* ... */ },
    move || {
        let body = if hyper_enabled {
            "Klynt input services stopped — your snippets and Hyper Key won't work. Caps Lock will return to its default behavior. Open Settings → Input Services to re-enable."
        } else {
            "Klynt input services stopped — your snippets won't work. Open Settings → Input Services to re-enable."
        };
        // Use osascript fallback if Tauri notification plugin not configured
        let _ = std::process::Command::new("osascript").arg("-e")
            .arg(format!(r#"display notification "{}" with title "Klynt""#, body))
            .spawn();
        tracing::warn!("input_services.tap.died");
    },
)?;
```

(If `init_input_services` doesn't currently take `AppHandle`, add it; this is a minor signature change.)

- [ ] **Step 2: Build + commit**

```bash
cargo build -p app-core
git add crates/app-core/src/init/input_services.rs
git commit -m "feat(input-services): desktop notification on tap death"
```

### Task 4.3: Frontend status polling

**Files:**
- Modify: `desktop-ui/src/features/settings/components/PermissionBanner.tsx`

- [ ] **Step 1: Poll status every 5s**

```tsx
import React, { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface Status { eventTapAlive: boolean; permission: 'granted'|'denied'|'unknown'; snippetCount: number }

export function PermissionBanner() {
  const [status, setStatus] = useState<Status | null>(null)
  const [granted, setGranted] = useState<boolean | null>(null)

  useEffect(() => {
    const tick = async () => {
      const s = await invoke<Status>('input_services_status')
      setStatus(s)
      const wasGranted = granted
      const isGranted = s.permission === 'granted'
      if (wasGranted === false && isGranted) {
        // Permission flipped — prompt restart
        alert('Input Monitoring granted. Please restart Klynt for changes to take effect.')
      }
      setGranted(isGranted)
    }
    tick()
    const id = setInterval(tick, 5000)
    return () => clearInterval(id)
  }, [granted])

  if (!status) return null
  if (status.permission === 'granted' && status.eventTapAlive) {
    return <div className="rounded bg-green-900/40 text-green-200 px-3 py-2 text-sm">Input services active.</div>
  }
  if (status.permission === 'granted' && !status.eventTapAlive) {
    return <div className="rounded bg-red-900/40 text-red-200 px-3 py-2 text-sm">Input services stopped. Restart Klynt.</div>
  }
  return (
    <div className="rounded bg-amber-900/40 text-amber-200 px-3 py-2 text-sm flex items-center justify-between">
      <span>Input Monitoring required.</span>
      <button onClick={() => invoke('input_services_request_permission')} className="px-2 py-1 rounded bg-amber-200 text-amber-900">Open System Settings</button>
    </div>
  )
}
```

- [ ] **Step 2: Build**

```bash
cd desktop-ui && bun run build
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/settings/components/PermissionBanner.tsx
git commit -m "feat(desktop-ui): polled permission/tap status with restart prompt on grant"
```

### Task 4.4: PR-4 gates

- [ ] **Step 1: All gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

1. Run with input services enabled and granted.
2. In System Settings, revoke Klynt's Input Monitoring permission.
3. Verify desktop notification appears within 5 seconds.
4. Verify settings page banner turns red.
5. Re-grant permission. Verify alert prompts restart.

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(input-services): permission polish + tap-death recovery" --body "Implements PR-4. Closes the loop on Spec 2 of the input services initiative."
```

---

## Self-Review Checklist (engineer to confirm before each PR ships)

- [ ] No `# arg:` placeholders or TODOs in code (out-of-scope items in spec are explicit, not deferred).
- [ ] All type names match between definition and use (`SnippetTable`, `HyperKeyConfig`, `RawKeyEvent`, `TapAction`, `LauncherExecuteResult` not relevant here).
- [ ] `DEV_COMMANDS` updated whenever a new `#[tauri::command]` lands.
- [ ] `crates/feature-input-services/PRIVACY.md` invariants hold: no keystroke contents in any `tracing` log.
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` is zero.
- [ ] No `unwrap()` in event-tap callbacks or expander hot paths.
- [ ] hidutil remap is reversed before Klynt exits when Hyper Key was enabled with Caps Lock source.
- [ ] Migrations: only PR-2 adds one (`001_snippets.sql`).

---

## Acceptance criteria (from spec)

- Type `;email ` in TextEdit → expansion within 200 ms; clipboard restored within 300 ms — manual after PR-2.
- Same in Slack desktop — manual after PR-2.
- Tap Caps Lock alone → Escape; hold Caps Lock + L → Cmd+Ctrl+Opt+Shift+L — manual after PR-3.
- Revoke Input Monitoring → notification within 5s, banner turns red — manual after PR-4.
- Re-grant Input Monitoring → restart prompt on next status poll — manual after PR-4.
- Both disabled → no event tap created — verify via absence of `input_services.tap.started` log.
- Add snippet via settings page → expands within ~50 ms on next trigger (watch-channel hot reload) — manual after PR-2.
- All 4 PRs ship independently green; main never broken.
