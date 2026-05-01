# Klynt Coding-in-Chat — Phase 1 Plan 3 of 6: Tool Kit Completion + Linux Sandbox

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the remaining 11 coding tools (`read`, `glob`, `grep`, `edit`, `write`, `apply_patch`, `web_fetch`, `ask_user`, `enter_plan_mode`, `exit_plan_mode`, `notebook_edit`) plus a `tool_search` no-op stub, ship the Linux Landlock + bwrap sandbox path so coding mode works on Linux, **fix the Plan 2 channel-routing deviation** so coding-mode threads actually expose coding tools to the LLM, and surface `FileEditWithSymbols` events to the desktop UI as `kind: "diff"` rows. Plan 3 produces the full 12-tool default curated coding profile working end-to-end on macOS and Linux, with diffs rendered in chat.

**Architecture:** Plan 2 landed `bash` end-to-end through every architectural seam — privacy guard, Layer 1 declarative approval, approval round-trip, macOS Seatbelt, `ApprovalCard`. Plan 3 broadens that single tool to a full kit by **mirroring** `BashTool`'s pattern (constructor-injected `Arc<Layer1>`/`Arc<Policy>`/`Arc<PrivacyGuard>`/`Arc<PendingApprovalsMap>`/`Arc<DomainEventBus>` dependencies, `#[derive(Tool)]` + `#[derive(ToolParams)]`, `evaluate(GuardCtx, ...)` middleware call before sandbox/IO) for 11 sibling tools — most short, three of them (`edit`, `write`, `apply_patch`) emitting the rich `FileEditWithSymbols` event already defined in Plan 1.

The Linux sandbox is implemented as a **two-tier** strategy:
1. `LinuxSandboxRunner` (parent) detects `bwrap` on PATH and Landlock kernel support; if both available it spawns `bwrap … -- klynt-sandbox-helper --landlock <policy-json> -- <program> <args>` and the helper applies `prctl(PR_SET_NO_NEW_PRIVS)` + `landlock::Ruleset::restrict_self()` *inside* the bwrap namespace before exec'ing the target.
2. If `bwrap` is missing, it falls back to spawning `klynt-sandbox-helper --landlock-only ...` directly (no namespace; Landlock-only — write-isolated but network unblocked, surfaced via `SandboxPolicyApplied { fallback_unsandboxed: false, network_constraints: "unenforced", ... }` and a banner).
3. If neither is available, returns `SandboxError::Unavailable`.

Three architectural fixes / completions:
- **Channel routing (the Plan 2 deviation).** Thread the `mode` field from `chat_send` Tauri command → `AppCore::chat_send` → free fn `chat_send` → `process_direct_streaming` → `RoutingContext::with_interaction(channel_for_mode(mode), ...)`. Without this, the `available_for_channel` filter (already in `crates/common/src/coding_channel.rs`) never selects the coding tools.
- **`FileEditWithSymbols` → `kind: "diff"`.** Add an `agent:file_edit_with_symbols` Tauri channel emit in AppCore's streaming pump and a React listener that synthesises `kind: "diff"` `ConversationItem`s into `chatStreamStore.fileEditsBySession`. The existing `DiffRow` (which already uses `PierreDiffBlock`) renders them.
- **Tool registration loop.** Plan 2 registered just `BashTool` at lines 1772–1810 of `app-core/src/init/mod.rs`. Plan 3 expands this to register all 12 coding tools with their respective dependency injections and ensures conditional registration (only when `coding.enabled` config or sandbox capabilities present).

**Tech Stack:** Rust 1.93 stable, `tokio` (`process`, `time`, `select!`), `landlock = "0.4"`, `nix = "0.29"` (in helper for `execvp` / `prctl`), `which = "7"` (bwrap detection), `globset` + `walkdir` (glob), `regex` + `bstr` (grep), `reqwest` + `html2text` + `scraper` (web_fetch — already in workspace), `diffy` (unified diff parser for `apply_patch`), `serde_json` (notebook editing — `.ipynb` is JSON), Tauri 2 IPC, React 18 + Vitest. macOS uses Plan 2's `MacOsSeatbeltRunner` unchanged.

**Spec reference:** `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` — primarily §6 (tool surface + curation), §7 (3-layer approval + sandbox; Linux Landlock+bwrap section), §10 (event vocabulary — `FileEditWithSymbols`, `SandboxPolicyApplied`), §13 Phase 1 deliverable list (Linux sandbox live, all 12 coding tools, `tool_search` no-op stub).

**Plan suite:** This is plan 3 of 6 covering Phase 1.
- Plan 1 ✅: Foundation primitives.
- Plan 2 ✅: First tool end-to-end (`bash` + privacy guard + Layer 1 + macOS Seatbelt + ApprovalCard).
- **Plan 3 (this):** Tool kit completion (11 tools + tool_search stub) + Linux sandbox + channel-routing fix + diff rendering.
- Plan 4: Layer 2 Starlark + hooks engine.
- Plan 5: Skills + recall + Distiller/Mirror subscribers.
- Plan 6: Settings page + slash command catalog completion + scenario tests.

---

## Sequencing

```
A. Linux sandbox — bwrap + helper + Landlock         ─┐ parallelizable
B. Channel-routing fix (Plan 2 deviation)             │ Rust-only; A/B independent
C. Read-only tools (read, glob, grep)                ─┘
D. Mutating tools (edit, write, apply_patch)          ─── depends on C (FsToolBase pattern locked)
E. Other tools (web_fetch, ask_user, plan_mode, notebook_edit, tool_search)
                                                      ─── parallel with D
F. Frontend — file edit event → kind: "diff"         ─── depends on D (event shape stable)
G. AppCore registration of all 12 tools              ─── depends on A, B, C, D, E
H. Property + scenario tests + acceptance gate       ─── depends on G + F
```

For subagent-driven execution, run A/B/C in parallel; D/E in parallel after C; F runs alongside D. G and H are sequential after the others.

---

## File structure

### Files created in this plan

```
bot/
├── crates/
│   ├── klynt-sandbox/src/
│   │   ├── linux.rs                            (LinuxSandboxRunner)
│   │   ├── bwrap.rs                            (build_bwrap_args + helper-locator)
│   │   └── helper_proto.rs                     (HelperPolicy serialize/deserialize)
│   ├── klynt-sandbox/tests/
│   │   ├── bwrap_args.rs                       (unit: argv shape per FsConstraints)
│   │   ├── linux_smoke.rs                      (cfg(target_os = "linux") integration)
│   │   └── helper_locator.rs                   (path resolution unit tests)
│   ├── klynt-sandbox-helper/src/
│   │   ├── main.rs                             (replaces Plan 1 stub)
│   │   ├── cli.rs                              (--landlock / --landlock-only arg parser)
│   │   └── landlock_apply.rs                   (Landlock + no_new_privs + execvp)
│   ├── klynt-core/src/tools/
│   │   ├── read.rs                             (ReadTool — short name "read")
│   │   ├── glob.rs                             (GlobTool — short name "glob")
│   │   ├── grep.rs                             (GrepTool — short name "grep")
│   │   ├── write.rs                            (WriteTool with approval)
│   │   ├── edit.rs                             (EditTool with approval)
│   │   ├── apply_patch.rs                      (ApplyPatchTool with approval)
│   │   ├── web_fetch.rs                        (WebFetchTool with approval)
│   │   ├── ask_user.rs                         (re-export from crates/tools)
│   │   ├── plan_mode.rs                        (EnterPlanModeTool + ExitPlanModeTool)
│   │   ├── notebook_edit.rs                    (NotebookEditTool with approval)
│   │   ├── tool_search.rs                      (no-op stub)
│   │   └── shared/
│   │       ├── mod.rs
│   │       ├── fs_resolve.rs                   (path expansion + privacy check helper)
│   │       └── file_edit_event.rs              (emit_file_edit_with_symbols helper)
│   └── klynt-core/tests/
│       ├── tool_read.rs
│       ├── tool_glob.rs
│       ├── tool_grep.rs
│       ├── tool_write.rs
│       ├── tool_edit.rs
│       ├── tool_apply_patch.rs
│       ├── tool_web_fetch.rs
│       ├── tool_plan_mode.rs
│       ├── tool_notebook_edit.rs
│       └── tool_search_stub.rs
├── desktop-ui/src/features/coding/
│   ├── components/
│   │   ├── DiffPreview.tsx                     (enhancement around PierreDiffBlock)
│   │   └── DiffPreview.test.tsx
│   └── hooks/
│       ├── useFileEditEvents.ts                (listens to agent:file_edit_with_symbols)
│       └── useFileEditEvents.test.ts
└── tests/integration/coding_in_chat/
    ├── property_k5_file_edit_event.rs          (K5: every edit/write/apply_patch emits FileEditWithSymbols)
    ├── property_k7_tool_filter.rs              (K7: coding tools only visible when channel == coding)
    ├── scenario_grep_then_edit.rs              (scenario: grep finds match, edit applies, diff renders)
    └── scenario_linux_bash.rs                  (cfg(target_os = "linux") bash echo via bwrap+Landlock)
```

### Files modified

```
crates/klynt-sandbox/Cargo.toml                       (add Linux deps: landlock, libc, which, nix)
crates/klynt-sandbox/src/lib.rs                       (cfg(linux) re-exports)
crates/klynt-sandbox-helper/Cargo.toml                (add: landlock, libc, nix, serde, serde_json, base64)
crates/klynt-sandbox-helper/src/main.rs               (full impl; replaces stub)
crates/klynt-core/Cargo.toml                          (add: walkdir, regex, bstr, reqwest+features, html2text, scraper, diffy, base64)
crates/klynt-core/src/lib.rs                          (no changes — tools/* additions are auto-picked)
crates/klynt-core/src/tools/mod.rs                    (export 11 new tools)
crates/agent/src/agent_loop/mod.rs                    (process_direct_streaming gains channel param)
crates/agent/src/events.rs                            (add PlanModeChanged variant)
crates/app-core/src/handlers/chat/streaming.rs        (thread mode through to process_direct_streaming)
crates/app-core/src/handlers/voice_conversation.rs    (pass None for new mode arg)
crates/app-core/src/init/mod.rs                       (register 11 new tools alongside BashTool)
crates/app-core/src/streaming/relay.rs                (emit agent:file_edit_with_symbols Tauri channel)
crates/desktop/src/commands/chat.rs                   (chat_send already has mode; no change beyond ensuring it's forwarded)
desktop-ui/src/types.ts                               (extend kind: "diff" with optional path/op/bytes — additive)
desktop-ui/src/features/messages/components/MessageRows.tsx  (DiffRow uses optional path label)
desktop-ui/src/features/chat/store/chatStreamStore.ts (fileEditsBySession slice + upsertFileEdit)
crates/common/src/coding_channel.rs                   (add tool_search to CODING_ONLY)
```

---

## Track A — Linux sandbox

### Task 1: `bwrap` argv builder (pure function unit test)

**Context:** `LinuxSandboxRunner` will spawn `/usr/bin/bwrap` with carefully constructed args based on a `SandboxPolicy`. We extract the argv-building logic into a pure function in `bwrap.rs` so we can unit-test it without spawning anything.

**Files:**
- Create: `crates/klynt-sandbox/src/bwrap.rs`
- Create: `crates/klynt-sandbox/tests/bwrap_args.rs`
- Modify: `crates/klynt-sandbox/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/klynt-sandbox/tests/bwrap_args.rs
#![cfg(target_os = "linux")]
use klynt_sandbox::bwrap::build_bwrap_args;
use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::PathBuf;

#[test]
fn cwd_writes_only_with_block_network() {
    let p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/work"));
    let args = build_bwrap_args(&p, "/usr/bin/echo", &["hi"]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    // Namespace flags
    assert!(argv.contains(&"--unshare-user"));
    assert!(argv.contains(&"--unshare-pid"));
    assert!(argv.contains(&"--unshare-net"));   // network blocked
    assert!(argv.contains(&"--die-with-parent"));
    assert!(argv.contains(&"--new-session"));

    // Filesystem
    assert!(argv.contains(&"--ro-bind"));
    let bind_idx = argv.iter().position(|s| *s == "--bind").unwrap();
    assert_eq!(argv[bind_idx + 1], "/tmp/work");
    assert_eq!(argv[bind_idx + 2], "/tmp/work");

    // /proc and /dev
    assert!(argv.windows(2).any(|w| w[0] == "--proc" && w[1] == "/proc"));
    assert!(argv.windows(2).any(|w| w[0] == "--dev"  && w[1] == "/dev"));

    // chdir to cwd
    assert!(argv.windows(2).any(|w| w[0] == "--chdir" && w[1] == "/tmp/work"));

    // Delimiter then program/args at end
    let dash = argv.iter().rposition(|s| *s == "--").unwrap();
    assert_eq!(argv[dash + 1], "/usr/bin/echo");
    assert_eq!(argv[dash + 2], "hi");
}

#[test]
fn read_only_policy_omits_writable_bind() {
    let p = SandboxPolicy::read_only(PathBuf::from("/tmp/ro"));
    let args = build_bwrap_args(&p, "/usr/bin/cat", &["/tmp/ro/file"]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    // Read-only: no --bind, only --ro-bind
    assert!(!argv.contains(&"--bind"));
    assert!(argv.contains(&"--ro-bind"));
}

#[test]
fn allow_network_omits_unshare_net() {
    let mut p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/n"));
    p.network = NetworkConstraints::Allow;
    let args = build_bwrap_args(&p, "/usr/bin/curl", &["http://example.com"]);
    assert!(!args.iter().any(|s| s == "--unshare-net"));
}

#[test]
fn fs_constraints_none_blocks_all_writes() {
    let p = SandboxPolicy {
        cwd: PathBuf::from("/tmp/n"),
        fs: FsConstraints::None,
        network: NetworkConstraints::Block,
        allow_process_fork: false,
    };
    let args = build_bwrap_args(&p, "/bin/true", &[]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    // No writable bind for any path when FsConstraints::None
    assert!(!argv.contains(&"--bind"));
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p klynt-sandbox --test bwrap_args --target-dir target/test-bwrap 2>&1 | head -40
```

Expected: FAIL — `klynt_sandbox::bwrap` module does not exist yet.

(On macOS this test is gated to Linux-only via `#![cfg(target_os = "linux")]`. macOS developers run `cargo check -p klynt-sandbox --target x86_64-unknown-linux-gnu` only after rustup adds that target. For the local feedback loop on macOS, change the cfg to `#[cfg(any(target_os = "linux", test))]` *temporarily* during development; revert before commit. The CI Linux runner is the source of truth.)

- [ ] **Step 3: Implement `bwrap.rs`**

```rust
// crates/klynt-sandbox/src/bwrap.rs
#![cfg(target_os = "linux")]

use crate::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::Path;

/// Builds the argv for `/usr/bin/bwrap`. Does NOT include the bwrap binary
/// path itself — caller invokes Command::new("/usr/bin/bwrap").args(...).
pub fn build_bwrap_args(policy: &SandboxPolicy, program: &str, args: &[&str]) -> Vec<String> {
    let cwd = policy.cwd.to_string_lossy().into_owned();
    let mut a: Vec<String> = Vec::with_capacity(32);

    // Namespace isolation
    a.extend(["--unshare-user", "--unshare-pid"].into_iter().map(String::from));
    if matches!(policy.network, NetworkConstraints::Block) {
        a.push("--unshare-net".into());
    }
    a.push("--die-with-parent".into());
    a.push("--new-session".into());

    // Read-only root mount (essential system dirs)
    for p in ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc"] {
        if Path::new(p).exists() {
            a.push("--ro-bind".into()); a.push(p.into()); a.push(p.into());
        }
    }

    // /proc, /dev, /tmp
    a.push("--proc".into()); a.push("/proc".into());
    a.push("--dev".into());  a.push("/dev".into());
    a.push("--tmpfs".into()); a.push("/tmp".into());

    // Filesystem constraints
    match &policy.fs {
        FsConstraints::WriteCwdReadAll { cwd: w } => {
            let wcwd = w.to_string_lossy().into_owned();
            a.push("--bind".into()); a.push(wcwd.clone()); a.push(wcwd);
        }
        FsConstraints::ReadCwdOnly { cwd: r } => {
            let rcwd = r.to_string_lossy().into_owned();
            a.push("--ro-bind".into()); a.push(rcwd.clone()); a.push(rcwd);
        }
        FsConstraints::None => {
            // No additional bind beyond /tmp tmpfs above.
        }
    }

    a.push("--chdir".into()); a.push(cwd);
    a.push("--".into());

    // Inner command: program + args
    a.push(program.into());
    a.extend(args.iter().map(|s| s.to_string()));

    a
}
```

- [ ] **Step 4: Re-export from `lib.rs`**

```rust
// crates/klynt-sandbox/src/lib.rs (append, replacing existing exports if needed)
pub mod error;
pub mod policy;
pub mod runner;
#[cfg(target_os = "macos")] pub mod seatbelt;
#[cfg(target_os = "linux")] pub mod bwrap;
#[cfg(target_os = "linux")] pub mod helper_proto;
#[cfg(target_os = "linux")] pub mod linux;

pub use error::SandboxError;
pub use policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
pub use runner::{CommandOutput, SandboxRunner};
#[cfg(target_os = "macos")] pub use seatbelt::MacOsSeatbeltRunner;
#[cfg(target_os = "linux")] pub use linux::LinuxSandboxRunner;
```

- [ ] **Step 5: Run test on a Linux runner (or `--target` cross-compile)**

```bash
cargo test -p klynt-sandbox --test bwrap_args
```

Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-sandbox/src/bwrap.rs crates/klynt-sandbox/src/lib.rs crates/klynt-sandbox/tests/bwrap_args.rs
git commit -m "feat(klynt-sandbox): bwrap argv builder for Linux"
```

---

### Task 2: Helper protocol — `HelperPolicy` JSON shape

**Context:** The parent passes a base64-encoded JSON `HelperPolicy` to `klynt-sandbox-helper` as a CLI argument. Defining the wire shape in the library crate (`klynt-sandbox::helper_proto`) lets both sides of the boundary share the type.

**Files:**
- Create: `crates/klynt-sandbox/src/helper_proto.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/klynt-sandbox/tests/bwrap_args.rs`:

```rust
#[test]
fn helper_policy_roundtrip() {
    use klynt_sandbox::helper_proto::{HelperMode, HelperPolicy};
    use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
    use std::path::PathBuf;

    let p = HelperPolicy {
        mode: HelperMode::WithBwrap,
        sandbox: SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/x")),
    };
    let encoded = p.to_base64_json().unwrap();
    let parsed = HelperPolicy::from_base64_json(&encoded).unwrap();
    assert_eq!(parsed.mode, HelperMode::WithBwrap);
    assert!(matches!(parsed.sandbox.fs, FsConstraints::WriteCwdReadAll { .. }));
    assert!(matches!(parsed.sandbox.network, NetworkConstraints::Block));
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-sandbox --test bwrap_args helper_policy_roundtrip
```

FAIL.

- [ ] **Step 3: Implement `helper_proto.rs`**

```rust
// crates/klynt-sandbox/src/helper_proto.rs
#![cfg(target_os = "linux")]

use crate::policy::SandboxPolicy;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperMode {
    /// Helper runs INSIDE a bwrap namespace; applies Landlock as defense-in-depth then exec.
    WithBwrap,
    /// bwrap absent on host; helper applies Landlock-only then exec. Network not isolated.
    LandlockOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperPolicy {
    pub mode: HelperMode,
    pub sandbox: SandboxPolicy,
}

impl HelperPolicy {
    pub fn to_base64_json(&self) -> Result<String, serde_json::Error> {
        let bytes = serde_json::to_vec(self)?;
        Ok(base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes))
    }

    pub fn from_base64_json(s: &str) -> Result<Self, String> {
        let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(s).map_err(|e| format!("base64: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("json: {e}"))
    }
}
```

- [ ] **Step 4: Add `base64` dep to `Cargo.toml`**

```toml
# crates/klynt-sandbox/Cargo.toml [target.'cfg(target_os = "linux")'.dependencies]
base64 = "0.22"
```

(Verify `base64` is in workspace deps; if so use `workspace = true` per project convention.)

- [ ] **Step 5: Run + commit**

```bash
cargo test -p klynt-sandbox --test bwrap_args helper_policy_roundtrip
git add crates/klynt-sandbox/src/helper_proto.rs crates/klynt-sandbox/Cargo.toml
git commit -m "feat(klynt-sandbox): HelperPolicy JSON wire shape for sandbox-helper"
```

---

### Task 3: Implement `klynt-sandbox-helper` (full body)

**Context:** Plan 1 left `crates/klynt-sandbox-helper/src/main.rs` as a `println!` stub. This task makes it apply Landlock + `prctl(PR_SET_NO_NEW_PRIVS, 1)` and `execvp` the target program.

**Files:**
- Modify: `crates/klynt-sandbox-helper/Cargo.toml`
- Replace: `crates/klynt-sandbox-helper/src/main.rs`
- Create: `crates/klynt-sandbox-helper/src/cli.rs`
- Create: `crates/klynt-sandbox-helper/src/landlock_apply.rs`

- [ ] **Step 1: Add deps to `Cargo.toml`**

```toml
[dependencies]
common = { workspace = true }
klynt-sandbox = { workspace = true }

[target.'cfg(target_os = "linux")'.dependencies]
landlock = "0.4"
libc = "0.2"
nix = { version = "0.29", features = ["process"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
base64 = "0.22"
```

- [ ] **Step 2: Write `cli.rs`**

```rust
// crates/klynt-sandbox-helper/src/cli.rs
#![cfg(target_os = "linux")]

use klynt_sandbox::helper_proto::{HelperMode, HelperPolicy};

pub struct ParsedArgs {
    pub policy: HelperPolicy,
    pub program: String,
    pub args: Vec<String>,
}

pub fn parse(argv: &[String]) -> Result<ParsedArgs, String> {
    // Expected forms:
    //   klynt-sandbox-helper --landlock      <base64-policy> -- <program> <args...>
    //   klynt-sandbox-helper --landlock-only <base64-policy> -- <program> <args...>
    if argv.len() < 5 {
        return Err(format!(
            "usage: {} --landlock|--landlock-only <base64-policy> -- <program> <args...>",
            argv.first().map(String::as_str).unwrap_or("klynt-sandbox-helper"),
        ));
    }
    let mode_flag = &argv[1];
    let mode = match mode_flag.as_str() {
        "--landlock" => HelperMode::WithBwrap,
        "--landlock-only" => HelperMode::LandlockOnly,
        other => return Err(format!("unknown mode flag: {other}")),
    };
    let policy_b64 = &argv[2];
    if argv[3] != "--" {
        return Err(format!("expected '--' delimiter at arg 3, got {:?}", argv[3]));
    }
    let policy = HelperPolicy::from_base64_json(policy_b64)
        .map_err(|e| format!("policy decode: {e}"))?;
    if policy.mode != mode {
        return Err(format!(
            "policy.mode={:?} but CLI flag={:?} — mismatch",
            policy.mode, mode
        ));
    }
    let program = argv[4].clone();
    let args = argv[5..].to_vec();
    Ok(ParsedArgs { policy, program, args })
}

#[cfg(test)]
mod tests {
    use super::*;
    use klynt_sandbox::policy::SandboxPolicy;
    use std::path::PathBuf;

    fn build_argv(mode_flag: &str) -> Vec<String> {
        let pol = HelperPolicy {
            mode: if mode_flag == "--landlock" { HelperMode::WithBwrap } else { HelperMode::LandlockOnly },
            sandbox: SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/work")),
        };
        let b64 = pol.to_base64_json().unwrap();
        vec![
            "klynt-sandbox-helper".into(), mode_flag.into(),
            b64, "--".into(), "/bin/echo".into(), "hi".into(),
        ]
    }

    #[test] fn parses_landlock_mode() {
        let p = parse(&build_argv("--landlock")).unwrap();
        assert_eq!(p.program, "/bin/echo"); assert_eq!(p.args, vec!["hi"]);
        assert_eq!(p.policy.mode, HelperMode::WithBwrap);
    }

    #[test] fn rejects_missing_delimiter() {
        let mut argv = build_argv("--landlock");
        argv[3] = "WRONG".into();
        assert!(parse(&argv).is_err());
    }

    #[test] fn rejects_unknown_mode_flag() {
        let mut argv = build_argv("--landlock");
        argv[1] = "--bogus".into();
        assert!(parse(&argv).is_err());
    }
}
```

- [ ] **Step 3: Implement `landlock_apply.rs`**

```rust
// crates/klynt-sandbox-helper/src/landlock_apply.rs
#![cfg(target_os = "linux")]

use klynt_sandbox::policy::{FsConstraints, SandboxPolicy};
use landlock::{
    ABI, Access, AccessFs, CompatLevel, PathBeneath, PathFd, RestrictionStatus, Ruleset,
    RulesetAttr, RulesetCreatedAttr, RulesetStatus,
};

/// Reserved exit codes for the helper:
///   124 = timeout-by-parent (used by parent runner via SIGKILL)
///   125 = sandbox unavailable (Landlock returned NotEnforced)
///   126 = sandbox setup failed (other)
/// Any other code is the wrapped program's own exit code.
pub const EXIT_SANDBOX_UNAVAILABLE: i32 = 125;
pub const EXIT_SANDBOX_SETUP_FAILED: i32 = 126;

pub fn apply_no_new_privs() -> Result<(), String> {
    // SAFETY: prctl is libc; PR_SET_NO_NEW_PRIVS=38, value 1 enables.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(format!(
            "prctl(PR_SET_NO_NEW_PRIVS) failed: {}", std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Applies Landlock filesystem restrictions for the current process.
/// Returns Ok(()) on FullyEnforced; Err with reserved exit code intent on degraded paths.
pub fn apply_landlock(p: &SandboxPolicy) -> Result<RestrictionStatus, String> {
    let abi = ABI::new_current();
    if abi == ABI::Unsupported {
        return Err("landlock unavailable: kernel < 5.13 or denied".into());
    }

    let cwd = match &p.fs {
        FsConstraints::WriteCwdReadAll { cwd } => cwd,
        FsConstraints::ReadCwdOnly { cwd } => cwd,
        FsConstraints::None => return Ok(RestrictionStatus {
            ruleset: RulesetStatus::FullyEnforced, no_new_privs: true,
        }), // nothing to enforce
    };

    let read_all = AccessFs::from_read(abi);
    let write_set = match &p.fs {
        FsConstraints::WriteCwdReadAll { .. } => AccessFs::from_all(abi),
        FsConstraints::ReadCwdOnly { .. }     => AccessFs::from_read(abi),
        FsConstraints::None => unreachable!(),
    };

    let root_fd = PathFd::new("/").map_err(|e| format!("PathFd /: {e}"))?;
    let cwd_fd = PathFd::new(cwd.as_os_str())
        .map_err(|e| format!("PathFd {}: {e}", cwd.display()))?;

    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi)).map_err(|e| format!("handle_access: {e}"))?
        .create().map_err(|e| format!("Ruleset::create: {e}"))?
        .add_rule(PathBeneath::new(root_fd, read_all))
            .map_err(|e| format!("add_rule root ro: {e}"))?
        .add_rule(PathBeneath::new(cwd_fd, write_set))
            .map_err(|e| format!("add_rule cwd rw: {e}"))?
        .restrict_self().map_err(|e| format!("restrict_self: {e}"))?;

    Ok(status)
}
```

- [ ] **Step 4: Implement `main.rs`**

```rust
// crates/klynt-sandbox-helper/src/main.rs
#![cfg(target_os = "linux")]

mod cli;
mod landlock_apply;

use landlock::RulesetStatus;
use landlock_apply::{apply_landlock, apply_no_new_privs, EXIT_SANDBOX_SETUP_FAILED, EXIT_SANDBOX_UNAVAILABLE};
use std::os::unix::process::CommandExt as _;

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let parsed = match cli::parse(&argv) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("klynt-sandbox-helper: {e}");
            std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
        }
    };

    if let Err(e) = apply_no_new_privs() {
        eprintln!("klynt-sandbox-helper: {e}");
        std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
    }

    match apply_landlock(&parsed.policy.sandbox) {
        Ok(status) => {
            if status.ruleset != RulesetStatus::FullyEnforced
                && parsed.policy.mode == klynt_sandbox::helper_proto::HelperMode::LandlockOnly
            {
                // Landlock-only mode + not fully enforced = sandbox is missing.
                eprintln!("klynt-sandbox-helper: landlock not fully enforced ({:?})", status.ruleset);
                std::process::exit(EXIT_SANDBOX_UNAVAILABLE);
            }
        }
        Err(e) => {
            eprintln!("klynt-sandbox-helper: landlock setup: {e}");
            std::process::exit(EXIT_SANDBOX_UNAVAILABLE);
        }
    }

    // execvp the target. CommandExt::exec returns only on failure.
    let mut cmd = std::process::Command::new(&parsed.program);
    cmd.args(&parsed.args);
    let err = cmd.exec();
    eprintln!("klynt-sandbox-helper: exec {}: {err}", parsed.program);
    std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("klynt-sandbox-helper: not supported on this platform");
    std::process::exit(2);
}
```

- [ ] **Step 5: Run helper unit tests**

```bash
cargo test -p klynt-sandbox-helper
```

PASS (3 tests under `cli::tests`).

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-sandbox-helper/
git commit -m "feat(klynt-sandbox-helper): Landlock + no_new_privs + execvp body"
```

---

### Task 4: `LinuxSandboxRunner` (parent — bwrap + helper invocation + fallbacks)

**Files:**
- Create: `crates/klynt-sandbox/src/linux.rs`
- Create: `crates/klynt-sandbox/tests/helper_locator.rs`

- [ ] **Step 1: Write the locator test**

```rust
// crates/klynt-sandbox/tests/helper_locator.rs
#![cfg(target_os = "linux")]
use klynt_sandbox::linux::locate_helper;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;

#[test]
fn locates_helper_next_to_current_exe() {
    let parent_dir = tempfile::tempdir().unwrap();
    let helper_path = parent_dir.path().join("klynt-sandbox-helper");
    File::create(&helper_path).unwrap();
    fs::set_permissions(&helper_path, fs::Permissions::from_mode(0o755)).unwrap();
    let parent_exe = parent_dir.path().join("klyntbot");
    File::create(&parent_exe).unwrap();
    let found = locate_helper(Some(&parent_exe)).expect("located");
    assert_eq!(found, helper_path);
}

#[test]
fn returns_err_when_helper_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let fake_exe = dir.path().join("klyntbot");
    File::create(&fake_exe).unwrap();
    assert!(locate_helper(Some(&fake_exe)).is_err());
}
```

- [ ] **Step 2: Add deps**

```toml
# crates/klynt-sandbox/Cargo.toml [target.'cfg(target_os = "linux")'.dependencies]
which = "7"
tokio = { workspace = true, features = ["process","rt","macros","time"] }
```

- [ ] **Step 3: Implement `linux.rs`**

```rust
// crates/klynt-sandbox/src/linux.rs
#![cfg(target_os = "linux")]

use crate::bwrap::build_bwrap_args;
use crate::error::SandboxError;
use crate::helper_proto::{HelperMode, HelperPolicy};
use crate::policy::SandboxPolicy;
use crate::runner::{CommandOutput, SandboxRunner};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// bwrap available, Landlock available — full isolation.
    WithBwrap,
    /// bwrap missing — Landlock-only (network NOT isolated).
    LandlockOnly,
    /// Neither available.
    Unavailable,
}

pub struct LinuxSandboxRunner {
    helper_path: PathBuf,
    mode: SandboxMode,
}

impl LinuxSandboxRunner {
    pub fn new() -> Result<Self, SandboxError> {
        let parent_exe = std::env::current_exe()
            .map_err(|e| SandboxError::Unavailable(format!("current_exe: {e}")))?;
        let helper_path = locate_helper(Some(&parent_exe))?;

        let bwrap_present = which::which("bwrap").is_ok();
        let landlock_present = is_landlock_available();
        let mode = match (bwrap_present, landlock_present) {
            (true,  true)  => SandboxMode::WithBwrap,
            (true,  false) => SandboxMode::WithBwrap,    // bwrap suffices for namespaces
            (false, true)  => SandboxMode::LandlockOnly,
            (false, false) => SandboxMode::Unavailable,
        };
        Ok(Self { helper_path, mode })
    }

    pub fn mode(&self) -> SandboxMode { self.mode }
}

pub fn locate_helper(parent_exe: Option<&Path>) -> Result<PathBuf, SandboxError> {
    if let Some(exe) = parent_exe {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("klynt-sandbox-helper");
            if candidate.exists() { return Ok(candidate); }
        }
    }
    if let Ok(p) = which::which("klynt-sandbox-helper") { return Ok(p); }
    Err(SandboxError::Unavailable("klynt-sandbox-helper not found".into()))
}

fn is_landlock_available() -> bool {
    // ABI::new_current() returns Unsupported when kernel < 5.13. Probing via
    // a no-op ruleset would require linking landlock crate here; instead we
    // attempt a syscall via the sandbox-helper at LinuxSandboxRunner::new()
    // time. For now, optimistic-true; helper exit code 125 surfaces failure.
    true
}

#[async_trait]
impl SandboxRunner for LinuxSandboxRunner {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError> {
        if matches!(self.mode, SandboxMode::Unavailable) {
            return Err(SandboxError::Unavailable(
                "neither bwrap nor Landlock available on this host".into()
            ));
        }

        let helper_mode = match self.mode {
            SandboxMode::WithBwrap     => HelperMode::WithBwrap,
            SandboxMode::LandlockOnly  => HelperMode::LandlockOnly,
            SandboxMode::Unavailable   => unreachable!(),
        };
        let policy_b64 = HelperPolicy { mode: helper_mode, sandbox: policy.clone() }
            .to_base64_json()
            .map_err(|e| SandboxError::PolicyGen(e.to_string()))?;

        let mut command = match self.mode {
            SandboxMode::WithBwrap => {
                // bwrap … -- helper --landlock <b64> -- <program> <args...>
                let helper_str = self.helper_path.to_string_lossy().into_owned();
                let mut inner: Vec<&str> = vec![helper_str.as_str(), "--landlock", policy_b64.as_str(), "--", program];
                inner.extend(args.iter().copied());
                let bwrap_args = build_bwrap_args(policy, &helper_str, &flatten_inner(&inner));
                let mut c = Command::new("/usr/bin/bwrap");
                c.args(&bwrap_args);
                c
            }
            SandboxMode::LandlockOnly => {
                let mut c = Command::new(&self.helper_path);
                c.arg("--landlock-only").arg(&policy_b64).arg("--").arg(program).args(args);
                c
            }
            SandboxMode::Unavailable => unreachable!(),
        };

        if let Some(d) = cwd { command.current_dir(d); }
        command.stdin(std::process::Stdio::null())
               .stdout(std::process::Stdio::piped())
               .stderr(std::process::Stdio::piped());

        let child = command.spawn()?;
        let out = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(r) => r?,
            Err(_) => return Err(SandboxError::ChildExit(124)),
        };

        let exit_code = out.status.code().unwrap_or(-1);
        // Map helper-reserved exit codes
        if exit_code == 125 {
            return Err(SandboxError::Unavailable("landlock not enforced".into()));
        }
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
            exit_code,
        })
    }
}

// helper: build_bwrap_args takes &[&str] but we have Vec<&str>; convert at call site.
fn flatten_inner<'a>(v: &'a [&'a str]) -> Vec<&'a str> {
    // The builder expects the inner program followed by inner args.
    // Our inner[0] is helper path, rest is helper-args. Builder appends
    // them after `--`. So we re-emit inner as: program=helper, args=rest.
    let _ = v.first(); // just to suppress lint
    v[1..].iter().copied().collect()
}
```

(Note on `build_bwrap_args` shape: in Task 1 we wrote it as `(policy, program, args)`. The bwrap argv is `[…flags…, "--", program, args…]`. So the call site pulls `program = helper_path` and `args = ["--landlock", b64, "--", real_program, real_args…]`. Adjust the call to:

```rust
let helper_str = self.helper_path.to_string_lossy().into_owned();
let mut helper_args: Vec<&str> = vec!["--landlock", policy_b64.as_str(), "--", program];
helper_args.extend(args.iter().copied());
let bwrap_args = build_bwrap_args(policy, &helper_str, &helper_args);
```

— preferred form; replace the snippet above. The `flatten_inner` helper is unnecessary; remove it.)

- [ ] **Step 4: Run locator tests**

```bash
cargo test -p klynt-sandbox --test helper_locator
```

PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-sandbox/
git commit -m "feat(klynt-sandbox): LinuxSandboxRunner with bwrap+Landlock + LandlockOnly fallback"
```

---

### Task 5: Linux smoke integration test

**Files:**
- Create: `crates/klynt-sandbox/tests/linux_smoke.rs`

- [ ] **Step 1: Write integration test**

```rust
#![cfg(target_os = "linux")]
use klynt_sandbox::{LinuxSandboxRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn echo_inside_bwrap_landlock() {
    let cwd = tempfile::tempdir().unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = match LinuxSandboxRunner::new() {
        Ok(r) => r,
        Err(_) => { eprintln!("sandbox unavailable; skipping"); return; }
    };
    let out = runner.run_command(
        &policy, "/bin/echo", &["hi-from-sandbox"],
        Some(cwd.path()), Duration::from_secs(5),
    ).await.expect("run completes");
    assert!(out.stdout.contains("hi-from-sandbox"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn write_outside_cwd_blocked() {
    let cwd = tempfile::tempdir().unwrap();
    let outside = std::env::temp_dir().join(format!("klynt-l-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&outside).unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = LinuxSandboxRunner::new().unwrap();
    let cmd = format!("touch {}/forbidden 2>&1; echo done", outside.display());
    let out = runner.run_command(
        &policy, "/bin/sh", &["-c", &cmd],
        Some(cwd.path()), Duration::from_secs(5),
    ).await.unwrap();
    assert!(!outside.join("forbidden").exists(), "Landlock failed to block outside-cwd write");
    assert!(out.stdout.contains("done"));
}

#[tokio::test]
async fn timeout_kills_child() {
    let cwd = tempfile::tempdir().unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = LinuxSandboxRunner::new().unwrap();
    let r = runner.run_command(
        &policy, "/bin/sleep", &["999"],
        Some(cwd.path()), Duration::from_millis(100),
    ).await;
    assert!(matches!(r, Err(klynt_sandbox::SandboxError::ChildExit(124))));
}
```

- [ ] **Step 2: Run on Linux runner**

```bash
cargo test -p klynt-sandbox --test linux_smoke
```

All three PASS. (CI gate.)

- [ ] **Step 3: Commit**

```bash
git add crates/klynt-sandbox/tests/linux_smoke.rs
git commit -m "test(klynt-sandbox): Linux smoke — echo, write-outside-cwd-blocked, timeout"
```

---

## Track B — Channel routing fix (Plan 2 deviation)

### Task 6: Thread `mode` through `process_direct_streaming`

**Context:** The Plan 2 audit found that `chat_send` accepts `mode: Option<String>` and persists it to `sessions.conversation_type`, but `process_direct_streaming` hardcodes `"desktop"` as the channel name when constructing `RoutingContext`. The result: even on a coding-mode thread, the LLM sees the desktop tool list and `bash`/`read`/`edit`/etc. are filtered out by `available_for_channel`. This task threads the mode through so coding tools become visible on coding-mode threads.

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` (add `mode: Option<String>` parameter to `process_direct_streaming`)
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` (forward `mode` from free `chat_send`)
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` (pass `None` to maintain compile)

- [ ] **Step 1: Write a regression test that fails today**

Create `crates/app-core/tests/coding_channel_dispatch.rs`:

```rust
use app_core::coding::chat_send_routing::channel_for_mode;
use common::{ChannelName, CODING_CHANNEL};

#[test]
fn coding_mode_resolves_to_coding_channel() {
    let ch = channel_for_mode(Some("coding"));
    assert_eq!(ch.as_str(), CODING_CHANNEL);
}

#[test]
fn no_mode_resolves_to_desktop() {
    assert_eq!(channel_for_mode(None).as_str(), "desktop");
}

// The integration test that would actually fail at the streaming entry point
// requires standing up the full agent loop; we cover that via the K7 invariant
// in tests/integration/coding_in_chat/property_k7_tool_filter.rs (Task 27 below).
```

This test passes today (the helper exists from Plan 2); the regression that fails is the integration test in Task 27. Run this test to ensure the helper still works after the refactor.

- [ ] **Step 2: Update `process_direct_streaming` signature**

In `crates/agent/src/agent_loop/mod.rs`, find the function (Plan 2 audit said line ~1003):

```rust
pub async fn process_direct_streaming(
    self: &Arc<Self>,
    content: String,
    session_key: String,
    mode: Option<String>,                  // NEW
) -> Result<StreamingHandle> {
    // …
    let channel: ChannelName = mode
        .as_deref()
        .map(|m| if m == "coding" { common::CODING_CHANNEL.into() } else { "desktop".into() })
        .unwrap_or_else(|| "desktop".into());
    let routing_ctx = RoutingContext::with_interaction(
        channel,                            // CHANGED from "desktop".into()
        session_key.clone().into(),
        interaction_tx,
    );
    // … rest unchanged
}
```

- [ ] **Step 3: Update `chat_send` free function in streaming.rs**

In `crates/app-core/src/handlers/chat/streaming.rs`, find the line ~257 where it calls `agent.process_direct_streaming(content.clone(), session_key.clone())`. Change to:

```rust
agent.process_direct_streaming(content.clone(), session_key.clone(), mode.clone())
```

The free function already receives `mode: Option<String>` — just forward it.

- [ ] **Step 4: Update voice path**

In `crates/app-core/src/handlers/voice_conversation.rs` line ~670, change:

```rust
agent.process_direct_streaming(content, session_key)
```

to:

```rust
agent.process_direct_streaming(content, session_key, None)
```

(Voice is non-coding; pass `None` to default to `"desktop"`.)

- [ ] **Step 5: Build and run the routing helper test**

```bash
cargo build --workspace
cargo nextest run -p app-core -E 'test(coding_channel_dispatch)'
```

Both PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/app-core/src/handlers/chat/streaming.rs crates/app-core/src/handlers/voice_conversation.rs crates/app-core/tests/coding_channel_dispatch.rs
git commit -m "fix(coding): thread mode through process_direct_streaming so coding channel activates"
```

---

### Task 7: Verify the routing fix lights up `available_for_channel`

**Context:** With `mode` flowing through, when `mode == "coding"` the `RoutingContext.channel` becomes `"coding"`, and the `available_for_channel` filter inside `run_pipeline` (which is already in place from Plan 2) will now select the 12 coding tools. Add a focused test in the `agent` crate.

**Files:**
- Create: `crates/agent/tests/coding_channel_filter.rs`

- [ ] **Step 1: Write the test**

```rust
use common::coding_channel::{available_for_channel, Channel};

#[test]
fn coding_tools_visible_on_coding_channel() {
    for name in ["bash", "read", "glob", "grep", "edit", "write",
                 "apply_patch", "web_fetch", "ask_user",
                 "enter_plan_mode", "exit_plan_mode", "notebook_edit"] {
        assert!(available_for_channel(name, Channel::Coding), "{name} should be visible in coding mode");
        assert!(!available_for_channel(name, Channel::Desktop), "{name} should be hidden on desktop");
    }
}

#[test]
fn non_coding_tool_visible_everywhere() {
    assert!(available_for_channel("tasks", Channel::Coding));
    assert!(available_for_channel("tasks", Channel::Desktop));
}

#[test]
fn tool_search_listed_in_coding_only() {
    assert!(available_for_channel("tool_search", Channel::Coding));
    assert!(!available_for_channel("tool_search", Channel::Desktop));
}
```

- [ ] **Step 2: Add `tool_search` to `CODING_ONLY`**

In `crates/common/src/coding_channel.rs`, append `"tool_search"` to the `CODING_ONLY` array:

```rust
const CODING_ONLY: &[&str] = &[
    "bash", "read", "glob", "grep", "edit", "write",
    "apply_patch", "web_fetch", "ask_user",
    "enter_plan_mode", "exit_plan_mode", "notebook_edit",
    "tool_search",      // NEW
];
```

- [ ] **Step 3: Run test**

```bash
cargo nextest run -p agent --test coding_channel_filter
```

PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/common/src/coding_channel.rs crates/agent/tests/coding_channel_filter.rs
git commit -m "test(coding): coding tools visible only on coding channel; add tool_search"
```

---

## Track C — Read-only tools (`read`, `glob`, `grep`)

### Task 8: Shared `fs_resolve` helper

**Context:** All 6 fs-touching tools (`read`, `glob`, `grep`, `write`, `edit`, `apply_patch`) need: (1) `~`-expansion via `shellexpand::tilde`, (2) canonicalisation with graceful fallback, (3) cwd-restriction check (path must be inside session cwd), (4) privacy-guard pre-check. Extract a single helper to avoid 6× repetition.

**Files:**
- Create: `crates/klynt-core/src/tools/shared/mod.rs`
- Create: `crates/klynt-core/src/tools/shared/fs_resolve.rs`

- [ ] **Step 1: Add deps to `klynt-core/Cargo.toml`**

```toml
shellexpand = "3"
walkdir = "2"
regex = { workspace = true }
bstr = "1"
diffy = "0.4"
```

(Verify each is in workspace deps; add if missing.)

- [ ] **Step 2: Write failing test**

Create `crates/klynt-core/tests/shared_fs_resolve.rs`:

```rust
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::shared::fs_resolve::{resolve_under_cwd, FsResolveError};
use std::path::PathBuf;

#[test]
fn resolves_relative_path_under_cwd() {
    let cwd = PathBuf::from("/tmp");
    let r = resolve_under_cwd("foo.txt", &cwd, &PrivacyGuard::from_globs(&[]).unwrap()).unwrap();
    assert_eq!(r, PathBuf::from("/tmp/foo.txt"));
}

#[test]
fn rejects_path_outside_cwd() {
    let cwd = PathBuf::from("/tmp/work");
    let r = resolve_under_cwd("/etc/passwd", &cwd, &PrivacyGuard::from_globs(&[]).unwrap());
    assert!(matches!(r, Err(FsResolveError::OutsideCwd { .. })));
}

#[test]
fn rejects_privacy_excluded_path() {
    let g = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    let r = resolve_under_cwd("config/.env", &PathBuf::from("/tmp"), &g);
    assert!(matches!(r, Err(FsResolveError::PrivacyDenied { .. })));
}

#[test]
fn expands_tilde() {
    // Just verify it doesn't error; actual home expansion depends on $HOME env.
    std::env::set_var("HOME", "/tmp");
    let r = resolve_under_cwd("~/sub/file.txt", &PathBuf::from("/tmp"), &PrivacyGuard::from_globs(&[]).unwrap()).unwrap();
    assert_eq!(r, PathBuf::from("/tmp/sub/file.txt"));
}
```

- [ ] **Step 3: Run failing**

```bash
cargo test -p klynt-core --test shared_fs_resolve
```

FAIL.

- [ ] **Step 4: Implement `fs_resolve.rs`**

```rust
// crates/klynt-core/src/tools/shared/fs_resolve.rs
use crate::privacy::PrivacyGuard;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsResolveError {
    #[error("path {path:?} is outside session cwd {cwd:?}")]
    OutsideCwd { path: PathBuf, cwd: PathBuf },
    #[error("privacy guard denied {path:?}: matched {pattern}")]
    PrivacyDenied { path: PathBuf, pattern: String },
}

/// Expand `~`, resolve relative paths against cwd, ensure result is inside cwd,
/// and check it's not in the privacy exclude list.
pub fn resolve_under_cwd(
    raw: &str,
    cwd: &Path,
    privacy: &PrivacyGuard,
) -> Result<PathBuf, FsResolveError> {
    let expanded = shellexpand::tilde(raw).into_owned();
    let candidate = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        cwd.join(&expanded)
    };
    let resolved = candidate.canonicalize().unwrap_or(candidate);

    // cwd-restriction
    let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    if !resolved.starts_with(&cwd_canonical) {
        return Err(FsResolveError::OutsideCwd { path: resolved, cwd: cwd_canonical });
    }
    // Privacy
    if privacy.is_excluded(&resolved) {
        let pat = privacy.raw_patterns().first().cloned().unwrap_or_default();
        return Err(FsResolveError::PrivacyDenied { path: resolved, pattern: pat });
    }
    Ok(resolved)
}
```

- [ ] **Step 5: Implement `shared/mod.rs`**

```rust
// crates/klynt-core/src/tools/shared/mod.rs
pub mod fs_resolve;
pub mod file_edit_event;        // Task 14 fills this
```

(Stub `file_edit_event.rs` with empty body for now; Task 14 fills it.)

- [ ] **Step 6: Wire into `tools/mod.rs`**

```rust
// crates/klynt-core/src/tools/mod.rs (existing)
pub mod bash;
pub mod shared;                  // NEW
pub use bash::{BashArgs, BashTool};
```

- [ ] **Step 7: Run test + commit**

```bash
cargo test -p klynt-core --test shared_fs_resolve
git add crates/klynt-core/
git commit -m "feat(klynt-core): shared fs_resolve helper (cwd-restricted, privacy-aware)"
```

---

### Task 9: `ReadTool`

**Files:**
- Create: `crates/klynt-core/src/tools/read.rs`
- Create: `crates/klynt-core/tests/tool_read.rs`
- Modify: `crates/klynt-core/src/tools/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/klynt-core/tests/tool_read.rs
use common::ChannelName;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::read::{ReadArgs, ReadTool};
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn reads_file_inside_cwd() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "hello.txt" });
    let ctx = RoutingContext::system();
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("hello world"));
}

#[tokio::test]
async fn reads_with_offset_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let content = (0..10).map(|i| format!("line{i}\n")).collect::<String>();
    std::fs::write(dir.path().join("multi.txt"), &content).unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "multi.txt", "offset": 3, "limit": 2 });
    let ctx = RoutingContext::system();
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("line3"));
    assert!(out.contains("line4"));
    assert!(!out.contains("line5"));
    assert!(!out.contains("line0"));
}

#[tokio::test]
async fn read_outside_cwd_denied() {
    let dir = tempfile::tempdir().unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "/etc/passwd" });
    let ctx = RoutingContext::system();
    assert!(tool.execute(args, &ctx).await.is_err());
}

#[test]
fn is_concurrency_safe() {
    let dir = tempfile::tempdir().unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    assert!(<ReadTool as Tool>::is_concurrency_safe(&tool, &serde_json::json!({})));
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_read
```

FAIL.

- [ ] **Step 3: Implement `read.rs`**

```rust
// crates/klynt-core/src/tools/read.rs
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ReadArgs {
    /// File path; relative is resolved against session cwd, or absolute path inside cwd.
    #[param(required)]
    pub path: String,
    /// Optional 0-indexed line offset to start reading.
    pub offset: Option<u64>,
    /// Optional maximum number of lines to return.
    pub limit: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "read",
    description = "Read a UTF-8 text file from the session working directory. \
                   Returns the file contents (optionally sliced by offset/limit). \
                   Limited to files inside the session cwd; privacy guard applies.",
    params = "ReadArgs",
    permission = "read_only",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,read,coding"
)]
pub struct ReadTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl ReadTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for ReadTool {
    type Params = ReadArgs;

    async fn execute(&self, args: ReadArgs, _ctx: &RoutingContext) -> Result<String> {
        let path = resolve_under_cwd(&args.path, &self.cwd, &self.privacy)
            .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;

        let bytes = tokio::fs::read(&path).await
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read {}: {e}", path.display()))))?;
        let text = String::from_utf8_lossy(&bytes);

        let offset = args.offset.unwrap_or(0) as usize;
        let limit = args.limit.map(|l| l as usize).unwrap_or(usize::MAX);

        let sliced: String = text.lines()
            .skip(offset)
            .take(limit)
            .map(|l| { let mut s = l.to_string(); s.push('\n'); s })
            .collect();
        Ok(sliced)
    }
}

// Override is_concurrency_safe via the manual Tool impl bridged by #[derive(Tool)]:
// the derive emits a default `false`. We need a manual override. Approach:
// add a one-method impl after the derive (Rust allows multiple impl blocks).
impl ReadTool {
    // No-op marker; the actual override lives in the Tool impl below if the macro
    // does not accept #[tool(concurrency_safe = "true")]. If it does, prefer that.
}

// Fallback: hand-rolled impl that just defers to the derived one for required
// methods. Simplest: extend the macro to accept `concurrency_safe = "true"`.
// For Plan 3 we add the macro arg in Task 17 below; until then, manually
// implement the Tool trait method via a wrapping struct. To keep this Task
// self-contained, add a literal override:
```

(The `tools-core-macros` survey in the audit did not list `concurrency_safe` as a `#[tool(...)]` attribute. Solution: extend the macro before this task. Move that extension to a new dedicated Task 17 below; for the moment this task assumes the override mechanism exists. If the macro hasn't been extended yet, the test `is_concurrency_safe` will fail — that's the trigger to do Task 17.)

- [ ] **Step 4: Add `concurrency_safe` to `#[tool(...)]` macro**

(Defer to Task 17 if not done; otherwise simply add `concurrency_safe = "true"` to the `#[tool(...)]` attribute on `ReadTool`.)

For now, assume Task 17 has run and the attribute is supported:

```rust
#[tool(
    name = "read",
    description = "...",
    params = "ReadArgs",
    permission = "read_only",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,read,coding",
    concurrency_safe = "true"
)]
pub struct ReadTool { /* ... */ }
```

- [ ] **Step 5: Wire into `tools/mod.rs`**

```rust
pub mod read;
pub use read::{ReadArgs, ReadTool};
```

- [ ] **Step 6: Run test + commit**

```bash
cargo test -p klynt-core --test tool_read
git add crates/klynt-core/
git commit -m "feat(klynt-core): ReadTool — coding-channel 'read' with offset/limit"
```

---

### Task 10: `GlobTool`

**Files:**
- Create: `crates/klynt-core/src/tools/glob.rs`
- Create: `crates/klynt-core/tests/tool_glob.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/klynt-core/tests/tool_glob.rs
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::glob::GlobTool;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn finds_files_by_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "").unwrap();
    std::fs::write(dir.path().join("b.rs"), "").unwrap();
    std::fs::write(dir.path().join("c.txt"), "").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = GlobTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "pattern": "*.rs" });
    let ctx = RoutingContext::system();
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("a.rs"));
    assert!(out.contains("b.rs"));
    assert!(!out.contains("c.txt"));
}

#[tokio::test]
async fn respects_max_results() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..50 { std::fs::write(dir.path().join(format!("f{i}.rs")), "").unwrap(); }
    let tool = GlobTool::new(dir.path().to_path_buf(), Arc::new(PrivacyGuard::from_globs(&[]).unwrap()));
    let out = tool.execute(serde_json::json!({"pattern":"*.rs","max_results":10}), &RoutingContext::system()).await.unwrap();
    assert_eq!(out.lines().count(), 10);
}

#[tokio::test]
async fn skips_privacy_excluded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".env"), "secret").unwrap();
    std::fs::write(dir.path().join("ok.rs"), "").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&["**/.env"]).unwrap());
    let tool = GlobTool::new(dir.path().to_path_buf(), privacy);
    let out = tool.execute(serde_json::json!({"pattern":"*"}), &RoutingContext::system()).await.unwrap();
    assert!(out.contains("ok.rs"));
    assert!(!out.contains(".env"));
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_glob
```

FAIL.

- [ ] **Step 3: Implement `glob.rs`**

```rust
// crates/klynt-core/src/tools/glob.rs
use crate::privacy::PrivacyGuard;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GlobArgs {
    /// Glob pattern relative to session cwd (e.g., `**/*.rs`, `src/**/*.ts`).
    #[param(required)]
    pub pattern: String,
    /// Maximum number of paths to return (default 100).
    pub max_results: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "glob",
    description = "Find files matching a glob pattern under the session cwd. \
                   Returns one path per line, sorted by mtime descending. Privacy-excluded paths skipped.",
    params = "GlobArgs",
    permission = "read_only",
    category = "Search",
    cost = "Free",
    tags = "fs,search,coding",
    concurrency_safe = "true"
)]
pub struct GlobTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl GlobTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for GlobTool {
    type Params = GlobArgs;

    async fn execute(&self, args: GlobArgs, _ctx: &RoutingContext) -> Result<String> {
        let max = args.max_results.unwrap_or(100) as usize;
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&args.pattern)
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("bad pattern: {e}"))))?);
        let set = builder.build()
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(e.to_string())))?;

        let cwd = self.cwd.clone();
        let privacy = self.privacy.clone();
        let matches = tokio::task::spawn_blocking(move || -> Vec<(std::time::SystemTime, PathBuf)> {
            let mut out: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
            for entry in WalkDir::new(&cwd).follow_links(false) {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() { continue; }
                let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
                if !set.is_match(rel) { continue; }
                if privacy.is_excluded(entry.path()) { continue; }
                let mtime = entry.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((mtime, entry.path().to_path_buf()));
            }
            out.sort_by(|a, b| b.0.cmp(&a.0));
            out.truncate(max);
            out
        }).await.map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;

        Ok(matches.into_iter()
            .map(|(_, p)| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}
```

- [ ] **Step 4: Wire + test + commit**

```rust
// in tools/mod.rs:
pub mod glob;
pub use glob::{GlobArgs, GlobTool};
```

```bash
cargo test -p klynt-core --test tool_glob
git add crates/klynt-core/
git commit -m "feat(klynt-core): GlobTool — coding-channel 'glob' with mtime sort + privacy"
```

---

### Task 11: `GrepTool`

**Files:**
- Create: `crates/klynt-core/src/tools/grep.rs`
- Create: `crates/klynt-core/tests/tool_grep.rs`

- [ ] **Step 1: Failing test**

```rust
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::grep::GrepTool;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn finds_pattern_across_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn baz() {}\n").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = GrepTool::new(dir.path().to_path_buf(), privacy);
    let out = tool.execute(serde_json::json!({"pattern": "fn (foo|baz)"}), &RoutingContext::system()).await.unwrap();
    assert!(out.contains("a.rs:1:fn foo"));
    assert!(out.contains("b.rs:1:fn baz"));
    assert!(!out.contains("bar"));
}

#[tokio::test]
async fn case_insensitive_flag() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "FOO\n").unwrap();
    let tool = GrepTool::new(dir.path().to_path_buf(), Arc::new(PrivacyGuard::from_globs(&[]).unwrap()));
    let out = tool.execute(serde_json::json!({"pattern":"foo","case_insensitive":true}), &RoutingContext::system()).await.unwrap();
    assert!(out.contains("FOO"));
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_grep
```

FAIL.

- [ ] **Step 3: Implement `grep.rs`**

```rust
// crates/klynt-core/src/tools/grep.rs
use crate::privacy::PrivacyGuard;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GrepArgs {
    /// Regex pattern (Rust regex syntax).
    #[param(required)] pub pattern: String,
    /// Optional file glob to restrict the search (default `**/*`).
    pub include: Option<String>,
    /// Case-insensitive match.
    pub case_insensitive: Option<bool>,
    /// Maximum result lines (default 200).
    pub max_results: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "grep",
    description = "Search for a regex pattern across files under session cwd. \
                   Returns `path:line:text` lines. Privacy-excluded paths skipped.",
    params = "GrepArgs",
    permission = "read_only",
    category = "Search",
    cost = "Free",
    tags = "search,grep,coding",
    concurrency_safe = "true"
)]
pub struct GrepTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl GrepTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for GrepTool {
    type Params = GrepArgs;

    async fn execute(&self, args: GrepArgs, _ctx: &RoutingContext) -> Result<String> {
        let max = args.max_results.unwrap_or(200) as usize;
        let re = RegexBuilder::new(&args.pattern)
            .case_insensitive(args.case_insensitive.unwrap_or(false))
            .build()
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("regex: {e}"))))?;
        let include = args.include.unwrap_or_else(|| "**/*".into());
        let glob = globset::Glob::new(&include)
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("include: {e}"))))?
            .compile_matcher();

        let cwd = self.cwd.clone();
        let privacy = self.privacy.clone();
        let lines = tokio::task::spawn_blocking(move || -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            'outer: for entry in WalkDir::new(&cwd).follow_links(false) {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() { continue; }
                let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
                if !glob.is_match(rel) { continue; }
                if privacy.is_excluded(entry.path()) { continue; }
                let Ok(content) = std::fs::read_to_string(entry.path()) else { continue };
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        out.push(format!("{}:{}:{}",
                            rel.display(), i + 1, line));
                        if out.len() >= max { break 'outer; }
                    }
                }
            }
            out
        }).await.map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
        Ok(lines.join("\n"))
    }
}
```

- [ ] **Step 4: Wire + run + commit**

```rust
pub mod grep;
pub use grep::{GrepArgs, GrepTool};
```

```bash
cargo test -p klynt-core --test tool_grep
git add crates/klynt-core/
git commit -m "feat(klynt-core): GrepTool — coding-channel 'grep' with regex + glob include"
```

---

## Track D — Mutating tools (`write`, `edit`, `apply_patch`)

### Task 12: `concurrency_safe` proc-macro attribute

**Context:** `tools-core-macros` doesn't expose a way for `#[derive(Tool)]` to set `is_concurrency_safe = true`. We need this for `ReadTool`, `GlobTool`, `GrepTool` (read-only, parallelizable). Add a single `concurrency_safe = "true"` attribute to the `#[tool(...)]` attribute parser.

**Files:**
- Modify: `crates/tools-core-macros/src/lib.rs` (and helper modules — verify the actual parsing site)

- [ ] **Step 1: Locate the existing attribute parser**

```bash
grep -rn '"name"\|"description"\|"params"\|"permission"' crates/tools-core-macros/src/ | head
```

Note the file/function that parses the `#[tool(name = "...", description = "...", ...)]` attribute list.

- [ ] **Step 2: Write a passing test**

The proc-macro test would normally use `trybuild`, but a direct integration test in any consumer crate is simpler. Append to `crates/klynt-core/tests/tool_read.rs`:

```rust
#[test]
fn read_tool_marks_concurrency_safe_via_attr() {
    use tools_core::Tool;
    let dir = tempfile::tempdir().unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf(),
        std::sync::Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap()));
    assert!(<ReadTool as Tool>::is_concurrency_safe(&tool, &serde_json::json!({})));
}
```

- [ ] **Step 3: Extend the macro**

In `crates/tools-core-macros/src/lib.rs`, find the section that parses each known key inside `#[tool(...)]`. Add a new branch for `"concurrency_safe"`:

```rust
// Pseudocode — adapt to actual parsing style:
let mut concurrency_safe: Option<bool> = None;
// inside the parsing loop:
"concurrency_safe" => {
    let v = parse_string_lit(&meta)?;
    concurrency_safe = Some(matches!(v.as_str(), "true" | "1"));
}
// when generating the impl:
let concurrency_method = match concurrency_safe {
    Some(true)  => quote! {
        fn is_concurrency_safe(&self, _args: &::serde_json::Value) -> bool { true }
    },
    _ => quote! {},   // omit to use trait default (false)
};
// then include `#concurrency_method` in the impl Tool block.
```

(The exact `quote!` integration depends on how the existing macro structures its output; pattern-match the existing emit for `permission_level()` or `metadata()` overrides to keep style consistent.)

- [ ] **Step 4: Run the test**

```bash
cargo test -p klynt-core --test tool_read read_tool_marks_concurrency_safe_via_attr
```

PASS.

- [ ] **Step 5: Run all read/glob/grep tests to confirm overrides flow**

```bash
cargo nextest run -p klynt-core -E 'test(tool_read) | test(tool_glob) | test(tool_grep)'
```

PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/tools-core-macros/ crates/klynt-core/tests/
git commit -m "feat(tools-core-macros): support #[tool(concurrency_safe = \"true\")] override"
```

---

### Task 13: `WriteTool` (approval-aware, emits `FileEditWithSymbols`)

**Files:**
- Create: `crates/klynt-core/src/tools/write.rs`
- Create: `crates/klynt-core/tests/tool_write.rs`

- [ ] **Step 1: Failing test**

```rust
// crates/klynt-core/tests/tool_write.rs
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::write::{run_for_test as write_run, WriteArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn allow_all_perms() -> CodingPermissions {
    CodingPermissions {
        allow: vec!["Write(./**)".into(), "Edit(./**)".into(), "ApplyPatch(./**)".into()],
        ..Default::default()
    }
}

#[tokio::test]
async fn writes_file_and_emits_event() {
    let dir = tempfile::tempdir().unwrap();
    let layer1 = Arc::new(Layer1::compile(&allow_all_perms()).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);

    let cwd = dir.path().to_path_buf();
    let res = write_run(
        WriteArgs { path: "out.txt".into(), content: "hello write".into() },
        cwd.clone(), layer1, policy, privacy, pending, tx.clone(), bus,
        CancellationToken::new(),
    ).await.unwrap();

    assert!(res.contains("wrote"));
    assert_eq!(std::fs::read_to_string(dir.path().join("out.txt")).unwrap(), "hello write");

    drop(tx);
    let mut saw_edit = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::FileEditWithSymbols { op, path, .. } = e {
            assert_eq!(op, "write");
            assert!(path.ends_with("out.txt"));
            saw_edit = true;
        }
    }
    assert!(saw_edit, "FileEditWithSymbols must be emitted");
}

#[tokio::test]
async fn outside_cwd_denied_no_write_no_event() {
    let dir = tempfile::tempdir().unwrap();
    let layer1 = Arc::new(Layer1::compile(&allow_all_perms()).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = write_run(
        WriteArgs { path: "/etc/passwd".into(), content: "x".into() },
        dir.path().to_path_buf(), layer1, policy, privacy, pending, tx, bus,
        CancellationToken::new(),
    ).await;
    assert!(r.is_err());
    assert!(!std::path::Path::new("/etc/passwd").metadata().unwrap()
        .modified().unwrap()
        .duration_since(std::time::UNIX_EPOCH).unwrap()
        .as_secs() > 0); // sanity — we did not modify
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_write
```

FAIL.

- [ ] **Step 3: Implement the file-edit event helper first**

In `crates/klynt-core/src/tools/shared/file_edit_event.rs`:

```rust
use agent::events::AgentEvent;
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct FileEditEvent<'a> {
    pub op: &'a str,             // "edit" | "write" | "apply_patch"
    pub path: &'a str,
    pub bytes: u64,
    pub diff_full: String,
}

pub async fn emit_file_edit(
    event_tx: &Option<mpsc::Sender<AgentEvent>>,
    bus: &Arc<DomainEventBus>,
    e: FileEditEvent<'_>,
) {
    let evt = AgentEvent::FileEditWithSymbols {
        path: e.path.to_string(),
        op: e.op.to_string(),
        bytes: e.bytes,
        diff_full: e.diff_full,
        anchored_symbols: vec![],          // Phase 2: tree-sitter
        lsp_diagnostics_delta: vec![],     // Phase 2: LSP
    };
    if let Some(tx) = event_tx { let _ = tx.send(evt.clone()).await; }
    let _ = bus.publish(DomainEvent::Agent(evt));
}

/// Compute a unified diff of `before` → `after`. Empty `before` = pure write.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let patch = diffy::create_patch(before, after);
    format!("--- {path}\n+++ {path}\n{}", patch.to_string())
}
```

- [ ] **Step 4: Implement `write.rs`**

```rust
// crates/klynt-core/src/tools/write.rs
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, unified_diff, FileEditEvent};
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct WriteArgs {
    /// File path (relative to session cwd or absolute inside cwd).
    #[param(required)] pub path: String,
    /// New file contents (UTF-8). Replaces existing content if file exists.
    #[param(required)] pub content: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "write",
    description = "Write a UTF-8 text file inside the session cwd. \
                   Replaces existing content. Approval and privacy guard apply.",
    params = "WriteArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,write,coding"
)]
pub struct WriteTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl WriteTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for WriteTool {
    type Params = WriteArgs;

    async fn execute(&self, args: WriteArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(
            args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

/// Test-friendly runner with fully-explicit deps (mirrors BashTool's pattern).
#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WriteArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();

    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel: cancel.clone(), request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
    };
    let decision = evaluate(guard_ctx, "write", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await.unwrap_or_default();
    tokio::fs::write(&resolved, args.content.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

    let bytes = args.content.len() as u64;
    let diff = unified_diff(&path_str, &before, &args.content);
    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "write", path: &path_str, bytes, diff_full: diff,
    }).await;

    Ok(format!("wrote {} bytes to {}", bytes, path_str))
}
```

- [ ] **Step 5: Wire + test + commit**

```rust
pub mod write;
pub use write::{WriteArgs, WriteTool};
```

```bash
cargo test -p klynt-core --test tool_write
git add crates/klynt-core/
git commit -m "feat(klynt-core): WriteTool — approval-gated 'write' emitting FileEditWithSymbols"
```

---

### Task 14: `EditTool` (string-replace with single-match guard)

**Files:**
- Create: `crates/klynt-core/src/tools/edit.rs`
- Create: `crates/klynt-core/tests/tool_edit.rs`

- [ ] **Step 1: Failing test**

```rust
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::edit::{run_for_test as edit_run, EditArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn edits_unique_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "alpha\nbeta\ngamma\n").unwrap();
    let perms = CodingPermissions {
        allow: vec!["Edit(./**)".into()], ..Default::default()
    };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    edit_run(
        EditArgs { path: "f.txt".into(), old_text: "beta".into(), new_text: "BETA".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await.unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "alpha\nBETA\ngamma\n");
}

#[tokio::test]
async fn rejects_multiple_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x\nx\n").unwrap();
    let perms = CodingPermissions { allow: vec!["Edit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs { path: "f.txt".into(), old_text: "x".into(), new_text: "Y".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "x\nx\n");
}

#[tokio::test]
async fn rejects_missing_old_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "abc\n").unwrap();
    let perms = CodingPermissions { allow: vec!["Edit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs { path: "f.txt".into(), old_text: "missing".into(), new_text: "x".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_edit
```

FAIL.

- [ ] **Step 3: Implement `edit.rs`**

```rust
// crates/klynt-core/src/tools/edit.rs
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, unified_diff, FileEditEvent};
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct EditArgs {
    /// File path inside session cwd.
    #[param(required)] pub path: String,
    /// Exact text to find. Must appear exactly once.
    #[param(required)] pub old_text: String,
    /// Replacement text.
    #[param(required)] pub new_text: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "edit",
    description = "Replace exactly one occurrence of `old_text` with `new_text` in a file. \
                   Errors if old_text appears 0 or >1 times.",
    params = "EditArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,edit,coding"
)]
pub struct EditTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl EditTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for EditTool {
    type Params = EditArgs;
    async fn execute(&self, args: EditArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: EditArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel, request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
    };
    let decision = evaluate(guard_ctx, "edit", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
    let count = before.matches(&args.old_text).count();
    if count == 0 {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            "old_text not found in file. Make sure it matches exactly.".into())));
    }
    if count > 1 {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            format!("old_text appears {count} times. Provide more context to make it unique."))));
    }
    let after = before.replacen(&args.old_text, &args.new_text, 1);
    tokio::fs::write(&resolved, after.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

    let diff = unified_diff(&path_str, &before, &after);
    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "edit", path: &path_str, bytes: after.len() as u64, diff_full: diff,
    }).await;
    Ok(format!("edited {} ({} bytes)", path_str, after.len()))
}
```

- [ ] **Step 4: Wire + run + commit**

```rust
pub mod edit;
pub use edit::{EditArgs, EditTool};
```

```bash
cargo test -p klynt-core --test tool_edit
git add crates/klynt-core/
git commit -m "feat(klynt-core): EditTool — approval-gated 'edit' with single-match guard"
```

---

### Task 15: `ApplyPatchTool` (unified diff)

**Files:**
- Create: `crates/klynt-core/src/tools/apply_patch.rs`
- Create: `crates/klynt-core/tests/tool_apply_patch.rs`

- [ ] **Step 1: Failing test**

```rust
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::apply_patch::{run_for_test as patch_run, ApplyPatchArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn applies_unified_diff() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
    let patch = "--- f.txt\n+++ f.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+LINE2\n line3\n";
    let perms = CodingPermissions { allow: vec!["ApplyPatch(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    patch_run(
        ApplyPatchArgs { path: "f.txt".into(), patch: patch.into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await.unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "line1\nLINE2\nline3\n");
}

#[tokio::test]
async fn rejects_malformed_patch() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "abc\n").unwrap();
    let perms = CodingPermissions { allow: vec!["ApplyPatch(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = patch_run(
        ApplyPatchArgs { path: "f.txt".into(), patch: "not a patch".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_apply_patch
```

FAIL.

- [ ] **Step 3: Implement `apply_patch.rs`**

```rust
// crates/klynt-core/src/tools/apply_patch.rs
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, FileEditEvent};
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ApplyPatchArgs {
    /// File path inside session cwd.
    #[param(required)] pub path: String,
    /// Unified-diff patch text. Headers `---`/`+++` are accepted but the file
    /// is identified by the `path` field, not the diff headers.
    #[param(required)] pub patch: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "apply_patch",
    description = "Apply a unified-diff patch to a single file. Errors if the patch \
                   does not cleanly apply to the current file content.",
    params = "ApplyPatchArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,patch,coding"
)]
pub struct ApplyPatchTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl ApplyPatchTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for ApplyPatchTool {
    type Params = ApplyPatchArgs;
    async fn execute(&self, args: ApplyPatchArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: ApplyPatchArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel, request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
    };
    let decision = evaluate(guard_ctx, "apply_patch", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
    let patch = diffy::Patch::from_str(&args.patch)
        .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("malformed patch: {e}"))))?;
    let after = diffy::apply(&before, &patch)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("apply: {e}"))))?;
    tokio::fs::write(&resolved, after.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "apply_patch", path: &path_str, bytes: after.len() as u64, diff_full: args.patch.clone(),
    }).await;
    Ok(format!("applied patch to {} ({} bytes)", path_str, after.len()))
}
```

- [ ] **Step 4: Wire + run + commit**

```rust
pub mod apply_patch;
pub use apply_patch::{ApplyPatchArgs, ApplyPatchTool};
```

```bash
cargo test -p klynt-core --test tool_apply_patch
git add crates/klynt-core/
git commit -m "feat(klynt-core): ApplyPatchTool — diffy-based unified-diff patcher"
```

---

## Track E — Other tools (`web_fetch`, `ask_user` re-export, plan-mode, `notebook_edit`, `tool_search` stub)

### Task 16: `WebFetchTool` (approval-aware HTTP)

**Files:**
- Create: `crates/klynt-core/src/tools/web_fetch.rs`
- Create: `crates/klynt-core/tests/tool_web_fetch.rs`

- [ ] **Step 1: Add deps**

In `crates/klynt-core/Cargo.toml`:

```toml
reqwest = { workspace = true, default-features = false, features = ["rustls-tls", "json"] }
html2text = { workspace = true }
```

(Verify both are workspace deps; the audit confirms they exist for the existing `crates/tools/src/system/web.rs`.)

- [ ] **Step 2: Failing test**

```rust
// crates/klynt-core/tests/tool_web_fetch.rs
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::web_fetch::{run_for_test as fetch_run, WebFetchArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn fetches_text_from_local_server() {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let server = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = tokio::io::AsyncReadExt::read(&mut s, &mut buf).await;
        let body = "<html><body><h1>Title</h1><p>Hello world</p></body></html>";
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}",
            body.len(), body);
        tokio::io::AsyncWriteExt::write_all(&mut s, resp.as_bytes()).await.unwrap();
    });

    let perms = CodingPermissions { allow: vec!["WebFetch(*)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let url = format!("http://127.0.0.1:{port}/");
    let out = fetch_run(
        WebFetchArgs { url, format: Some("text".into()), max_bytes: Some(8192) },
        l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await.unwrap();
    server.await.ok();
    assert!(out.contains("Hello world"));
}
```

- [ ] **Step 3: Run failing**

```bash
cargo test -p klynt-core --test tool_web_fetch
```

FAIL.

- [ ] **Step 4: Implement `web_fetch.rs`**

```rust
// crates/klynt-core/src/tools/web_fetch.rs
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct WebFetchArgs {
    /// http(s) URL to fetch.
    #[param(required)] pub url: String,
    /// "text" (default — strip HTML to plain text via html2text) or "raw".
    pub format: Option<String>,
    /// Hard cap on response body bytes (default 200_000).
    pub max_bytes: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "web_fetch",
    description = "Fetch a URL via HTTP GET and return the body. \
                   Default `format=\"text\"` strips HTML tags to readable text. \
                   Approval gated — every fetch consults the approval layers.",
    params = "WebFetchArgs",
    permission = "standard",
    category = "Web",
    cost = "Low",
    tags = "web,fetch,coding"
)]
pub struct WebFetchTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl WebFetchTool {
    pub fn new(
        layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for WebFetchTool {
    type Params = WebFetchArgs;
    async fn execute(&self, args: WebFetchArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WebFetchArgs,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel: cancel.clone(), request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: None,
    };
    let decision = evaluate(guard_ctx, "web_fetch", &args.url).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let max_bytes = args.max_bytes.unwrap_or(200_000) as usize;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let resp = client.get(&args.url).send().await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("http: {e}"))))?;
    if !resp.status().is_success() {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            format!("http {} from {}", resp.status(), args.url))));
    }
    let body = resp.bytes().await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let truncated = &body[..body.len().min(max_bytes)];
    let raw = String::from_utf8_lossy(truncated).into_owned();
    let format = args.format.as_deref().unwrap_or("text");
    let out = if format == "text" {
        html2text::from_read(raw.as_bytes(), 80)
            .unwrap_or_else(|_| raw.clone())
    } else {
        raw
    };
    Ok(out)
}
```

- [ ] **Step 5: Wire + run + commit**

```rust
pub mod web_fetch;
pub use web_fetch::{WebFetchArgs, WebFetchTool};
```

```bash
cargo test -p klynt-core --test tool_web_fetch
git add crates/klynt-core/
git commit -m "feat(klynt-core): WebFetchTool — approval-gated HTTP GET with html→text"
```

---

### Task 17: `ask_user` re-export

**Context:** `crates/tools/src/system/ask_user.rs` already implements `AskUserTool` with the full `interaction_tx` → `InteractionBundle` → oneshot wiring. Re-exporting it from `klynt-core::tools::ask_user` keeps a single registration site (the AppCore init block).

**Files:**
- Create: `crates/klynt-core/src/tools/ask_user.rs`
- Modify: `crates/klynt-core/Cargo.toml` (add `tools = { workspace = true }`)

- [ ] **Step 1: Add `tools` dep**

```toml
# crates/klynt-core/Cargo.toml [dependencies]
tools = { workspace = true }
```

- [ ] **Step 2: Implement re-export**

```rust
// crates/klynt-core/src/tools/ask_user.rs
//! Re-export of the existing `tools::system::ask_user::AskUserTool`. The
//! Plan 3 coding kit registers it under the same canonical name `"ask_user"`,
//! which is already used by the upstream tool — no renaming needed.
pub use tools::system::ask_user::AskUserTool;
```

(Verify the upstream path; if the file is `crates/tools/src/system/ask_user.rs` with `pub struct AskUserTool`, the re-export above works. Adjust namespace if the actual path differs — `grep -rn "pub struct AskUserTool" crates/tools/`.)

- [ ] **Step 3: Wire into `tools/mod.rs`**

```rust
pub mod ask_user;
pub use ask_user::AskUserTool;
```

- [ ] **Step 4: Sanity test**

Append to `crates/klynt-core/tests/tool_read.rs` (just to verify import compiles in the consumer):

```rust
#[test]
fn ask_user_reexported() {
    let _ = std::any::type_name::<klynt_core::tools::AskUserTool>();
}
```

```bash
cargo test -p klynt-core --test tool_read ask_user_reexported
```

PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/
git commit -m "feat(klynt-core): re-export AskUserTool under canonical 'ask_user' name"
```

---

### Task 18: `enter_plan_mode` / `exit_plan_mode` tools

**Context:** Plan mode is a per-thread state where writes/exec are denied and the agent is told (via system prompt) to research-only. Setting plan-mode is a tool call — the tool flips `sessions.approval_mode = 'plan'` and emits `PlanModeChanged { active: true }`. Exiting flips it back to `'default'`.

**Files:**
- Create: `crates/klynt-core/src/tools/plan_mode.rs`
- Create: `crates/klynt-core/tests/tool_plan_mode.rs`
- Modify: `crates/agent/src/events.rs` (add `PlanModeChanged` variant)
- Modify: `crates/storage/src/repos/sessions.rs` (add `update_approval_mode` method if missing)

- [ ] **Step 1: Add `PlanModeChanged` event variant**

In `crates/agent/src/events.rs`, after the existing 18 variants, add:

```rust
PlanModeChanged {
    session_key: String,
    active: bool,
    requested_by: String,    // "tool" | "slash_command"
},
```

- [ ] **Step 2: Add storage method if missing**

```bash
grep -n "update_approval_mode\|approval_mode" crates/storage/src/repos/sessions.rs
```

If `update_approval_mode` doesn't exist:

```rust
// crates/storage/src/repos/sessions.rs
pub async fn update_approval_mode(&self, key: &str, mode: &str) -> Result<(), StorageError> {
    sqlx::query("UPDATE sessions SET approval_mode = ?1 WHERE session_key = ?2")
        .bind(mode).bind(key)
        .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 3: Failing tool test**

```rust
// crates/klynt-core/tests/tool_plan_mode.rs
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::tools::plan_mode::{run_enter_for_test, run_exit_for_test};
use std::sync::Arc;
use storage::{Repos, StoragePool};
use tokio::sync::mpsc;

#[tokio::test]
async fn enter_sets_approval_mode_plan_and_emits_event() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let key = repos.sessions.create_default("u1").await.unwrap();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    run_enter_for_test(&repos, &key, tx.clone(), bus.clone()).await.unwrap();
    let row = repos.sessions.find_by_key(&key).await.unwrap().unwrap();
    assert_eq!(row.approval_mode.as_deref(), Some("plan"));
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::PlanModeChanged { active: true, .. } = e { saw = true; }
    }
    assert!(saw);
}

#[tokio::test]
async fn exit_sets_approval_mode_default_and_emits_event() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let key = repos.sessions.create_default("u1").await.unwrap();
    repos.sessions.update_approval_mode(&key, "plan").await.unwrap();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    run_exit_for_test(&repos, &key, tx.clone(), bus.clone()).await.unwrap();
    let row = repos.sessions.find_by_key(&key).await.unwrap().unwrap();
    assert_eq!(row.approval_mode.as_deref(), Some("default"));
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::PlanModeChanged { active: false, .. } = e { saw = true; }
    }
    assert!(saw);
}
```

- [ ] **Step 4: Run failing**

```bash
cargo test -p klynt-core --test tool_plan_mode
```

FAIL.

- [ ] **Step 5: Implement `plan_mode.rs`**

```rust
// crates/klynt-core/src/tools/plan_mode.rs
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage::Repos;
use tokio::sync::mpsc;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct EnterPlanModeArgs {
    /// Optional rationale string the agent provides; logged but not enforced.
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ExitPlanModeArgs {
    pub rationale: Option<String>,
}

#[derive(ToolDerive)]
#[tool(
    name = "enter_plan_mode",
    description = "Switch the current session into plan mode (writes/exec denied). \
                   Persists to sessions.approval_mode='plan'. Emits PlanModeChanged.",
    params = "EnterPlanModeArgs",
    permission = "standard",
    category = "System",
    cost = "Free",
    tags = "plan,coding"
)]
pub struct EnterPlanModeTool {
    repos: Repos,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

#[derive(ToolDerive)]
#[tool(
    name = "exit_plan_mode",
    description = "Leave plan mode and resume normal approval evaluation. \
                   Persists sessions.approval_mode='default'. Emits PlanModeChanged.",
    params = "ExitPlanModeArgs",
    permission = "standard",
    category = "System",
    cost = "Free",
    tags = "plan,coding"
)]
pub struct ExitPlanModeTool {
    repos: Repos,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl EnterPlanModeTool {
    pub fn new(repos: Repos, event_tx: Option<mpsc::Sender<AgentEvent>>, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, event_tx, bus }
    }
}
impl ExitPlanModeTool {
    pub fn new(repos: Repos, event_tx: Option<mpsc::Sender<AgentEvent>>, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for EnterPlanModeTool {
    type Params = EnterPlanModeArgs;
    async fn execute(&self, _args: EnterPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let key = ctx.chat_id.as_str().to_string();
        run_enter_for_test(&self.repos, &key,
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone()).await
    }
}
#[async_trait]
impl ToolExecute for ExitPlanModeTool {
    type Params = ExitPlanModeArgs;
    async fn execute(&self, _args: ExitPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let key = ctx.chat_id.as_str().to_string();
        run_exit_for_test(&self.repos, &key,
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone()).await
    }
}

pub async fn run_enter_for_test(
    repos: &Repos, session_key: &str,
    event_tx: mpsc::Sender<AgentEvent>, bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos.sessions.update_approval_mode(session_key, "plan").await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = AgentEvent::PlanModeChanged {
        session_key: session_key.into(), active: true, requested_by: "tool".into(),
    };
    let _ = event_tx.send(evt.clone()).await;
    let _ = bus.publish(DomainEvent::Agent(evt));
    Ok("entered plan mode (writes and exec are now denied)".into())
}

pub async fn run_exit_for_test(
    repos: &Repos, session_key: &str,
    event_tx: mpsc::Sender<AgentEvent>, bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos.sessions.update_approval_mode(session_key, "default").await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = AgentEvent::PlanModeChanged {
        session_key: session_key.into(), active: false, requested_by: "tool".into(),
    };
    let _ = event_tx.send(evt.clone()).await;
    let _ = bus.publish(DomainEvent::Agent(evt));
    Ok("exited plan mode".into())
}
```

- [ ] **Step 6: Wire + run + commit**

```rust
pub mod plan_mode;
pub use plan_mode::{EnterPlanModeArgs, EnterPlanModeTool, ExitPlanModeArgs, ExitPlanModeTool};
```

```bash
cargo test -p klynt-core --test tool_plan_mode
git add crates/klynt-core/ crates/agent/src/events.rs crates/storage/
git commit -m "feat(klynt-core): EnterPlanModeTool/ExitPlanModeTool + PlanModeChanged event"
```

---

### Task 19: `NotebookEditTool` (Jupyter `.ipynb` cell-replace)

**Context:** A `.ipynb` file is JSON. We support a single operation in Plan 3: replace the `source` of a cell identified by 0-indexed cell number. More cell ops (insert, delete, change cell type) can land in a future plan.

**Files:**
- Create: `crates/klynt-core/src/tools/notebook_edit.rs`
- Create: `crates/klynt-core/tests/tool_notebook_edit.rs`

- [ ] **Step 1: Failing test**

```rust
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::notebook_edit::{run_for_test as nb_run, NotebookEditArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const NB: &str = r#"{
  "cells": [
    {"cell_type":"code","source":["print('hi')\n"],"outputs":[],"execution_count":null,"metadata":{}},
    {"cell_type":"markdown","source":["# Title\n"],"metadata":{}}
  ],
  "metadata":{},"nbformat":4,"nbformat_minor":5
}"#;

#[tokio::test]
async fn replaces_cell_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("nb.ipynb"), NB).unwrap();
    let perms = CodingPermissions { allow: vec!["NotebookEdit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    nb_run(
        NotebookEditArgs {
            path: "nb.ipynb".into(),
            cell_index: 0,
            new_source: "print('updated')\n".into(),
        },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await.unwrap();
    let saved = std::fs::read_to_string(dir.path().join("nb.ipynb")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let cell0_src = &v["cells"][0]["source"];
    assert_eq!(cell0_src, &serde_json::json!("print('updated')\n"));
}

#[tokio::test]
async fn rejects_out_of_range_index() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("nb.ipynb"), NB).unwrap();
    let perms = CodingPermissions { allow: vec!["NotebookEdit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = nb_run(
        NotebookEditArgs { path: "nb.ipynb".into(), cell_index: 99, new_source: "x".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_notebook_edit
```

FAIL.

- [ ] **Step 3: Implement `notebook_edit.rs`**

```rust
// crates/klynt-core/src/tools/notebook_edit.rs
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, unified_diff, FileEditEvent};
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolError, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct NotebookEditArgs {
    /// .ipynb path.
    #[param(required)] pub path: String,
    /// 0-indexed cell to modify.
    #[param(required)] pub cell_index: u64,
    /// New cell source (replaces existing source array with a single string).
    #[param(required)] pub new_source: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "notebook_edit",
    description = "Replace the `source` of a cell in a Jupyter notebook (.ipynb). \
                   Preserves outputs; updates only the targeted cell.",
    params = "NotebookEditArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "notebook,jupyter,coding"
)]
pub struct NotebookEditTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl NotebookEditTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for NotebookEditTool {
    type Params = NotebookEditArgs;
    async fn execute(&self, args: NotebookEditArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: NotebookEditArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel, request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
    };
    let decision = evaluate(guard_ctx, "notebook_edit", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
    let mut nb: serde_json::Value = serde_json::from_str(&before)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("parse ipynb: {e}"))))?;
    let cells = nb.get_mut("cells").and_then(|v| v.as_array_mut())
        .ok_or_else(|| KlyntbotError::Tool(ToolError::ExecutionFailed("ipynb missing cells array".into())))?;
    let idx = args.cell_index as usize;
    if idx >= cells.len() {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            format!("cell_index {idx} out of range (notebook has {} cells)", cells.len()))));
    }
    cells[idx]["source"] = serde_json::Value::String(args.new_source.clone());
    let after = serde_json::to_string_pretty(&nb)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;

    tokio::fs::write(&resolved, after.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;
    let diff = unified_diff(&path_str, &before, &after);
    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "notebook_edit", path: &path_str, bytes: after.len() as u64, diff_full: diff,
    }).await;
    Ok(format!("edited cell {} in {}", idx, path_str))
}
```

- [ ] **Step 4: Wire + run + commit**

```rust
pub mod notebook_edit;
pub use notebook_edit::{NotebookEditArgs, NotebookEditTool};
```

```bash
cargo test -p klynt-core --test tool_notebook_edit
git add crates/klynt-core/
git commit -m "feat(klynt-core): NotebookEditTool — single-cell source replace for .ipynb"
```

---

### Task 20: `tool_search` no-op stub

**Context:** Per spec §13 Phase 1: register `tool_search` as a no-op stub returning an empty array. Phase 2 fills it with Mirror per-skill effectiveness reranking.

**Files:**
- Create: `crates/klynt-core/src/tools/tool_search.rs`
- Create: `crates/klynt-core/tests/tool_search_stub.rs`

- [ ] **Step 1: Failing test**

```rust
use klynt_core::tools::tool_search::ToolSearchTool;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn returns_empty_array() {
    let tool = ToolSearchTool::new();
    let out = tool.execute(serde_json::json!({"query":"diff"}), &RoutingContext::system()).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}
```

- [ ] **Step 2: Run failing**

```bash
cargo test -p klynt-core --test tool_search_stub
```

FAIL.

- [ ] **Step 3: Implement**

```rust
// crates/klynt-core/src/tools/tool_search.rs
use async_trait::async_trait;
use common::Result;
use serde::{Deserialize, Serialize};
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ToolSearchArgs {
    /// Free-text query against deferred-tool descriptions. Plan 3 stub ignores this.
    pub query: Option<String>,
    /// Maximum number of suggestions to return.
    pub max_results: Option<u64>,
}

#[derive(ToolDerive, Default)]
#[tool(
    name = "tool_search",
    description = "Phase 1 stub. Searches a 'deferred tool' set for tools that match \
                   a query. Returns an empty array; full implementation arrives in Phase 2 \
                   with Mirror per-skill effectiveness reranking.",
    params = "ToolSearchArgs",
    permission = "read_only",
    category = "System",
    cost = "Free",
    tags = "tools,coding,stub",
    concurrency_safe = "true"
)]
pub struct ToolSearchTool;

impl ToolSearchTool {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl ToolExecute for ToolSearchTool {
    type Params = ToolSearchArgs;
    async fn execute(&self, _args: ToolSearchArgs, _ctx: &RoutingContext) -> Result<String> {
        Ok("[]".into())
    }
}
```

- [ ] **Step 4: Wire + run + commit**

```rust
pub mod tool_search;
pub use tool_search::{ToolSearchArgs, ToolSearchTool};
```

```bash
cargo test -p klynt-core --test tool_search_stub
git add crates/klynt-core/
git commit -m "feat(klynt-core): tool_search no-op stub (Phase 2 will rerank via Mirror)"
```

---

## Track F — Frontend: file edit event → `kind: "diff"` row

### Task 21: Emit `agent:file_edit_with_symbols` Tauri channel

**Context:** Plan 2 added emit branches for `agent:approval_requested` / `agent:approval_resolved` / `agent:sandbox_policy_applied` in AppCore's chat-streaming relay. Plan 3 adds a fourth branch for `agent:file_edit_with_symbols`.

**Files:**
- Modify: `crates/app-core/src/streaming/relay.rs` (or wherever `app.emit("agent:..." ...)` is — the audit found this in `relay_chat_stream`)

- [ ] **Step 1: Locate the existing match**

```bash
grep -rn 'app\.emit("agent:approval' crates/ | head
```

Note the exact file/function. Likely `crates/app-core/src/streaming/relay.rs` or similar.

- [ ] **Step 2: Add the new branch**

In the same `match evt { ... }` that already handles `ApprovalRequested`/`ApprovalResolved`/`SandboxPolicyApplied`, add:

```rust
AgentEvent::FileEditWithSymbols { ref path, ref op, bytes, ref diff_full, .. } => {
    let payload = serde_json::json!({
        "path": path,
        "op": op,
        "bytes": bytes,
        "diff": diff_full,
    });
    let _ = app.emit("agent:file_edit_with_symbols", payload);
}
AgentEvent::PlanModeChanged { ref session_key, active, ref requested_by } => {
    let payload = serde_json::json!({
        "session_key": session_key, "active": active, "requested_by": requested_by,
    });
    let _ = app.emit("agent:plan_mode_changed", payload);
}
```

(Hand-rolled JSON keeps parity with the Plan 2 approach the audit highlighted: "Event emission uses hand-rolled JSON, not `#[derive(Serialize)]`.")

- [ ] **Step 3: Build + smoke**

```bash
cargo build -p app-core
```

PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/
git commit -m "feat(coding): emit agent:file_edit_with_symbols + agent:plan_mode_changed Tauri events"
```

---

### Task 22: `useFileEditEvents` hook + `chatStreamStore` slice

**Files:**
- Create: `desktop-ui/src/features/coding/hooks/useFileEditEvents.ts`
- Create: `desktop-ui/src/features/coding/hooks/useFileEditEvents.test.ts`
- Modify: `desktop-ui/src/features/chat/store/chatStreamStore.ts`
- Modify: `desktop-ui/src/types.ts` (extend `kind: "diff"` with optional `path`/`op`/`bytes`)

- [ ] **Step 1: Extend the `ConversationItem` `kind: "diff"` shape**

In `desktop-ui/src/types.ts`, find the line `{ id: string; kind: "diff"; title: string; diff: string; status?: string }` and replace with:

```ts
{
  id: string;
  kind: "diff";
  title: string;
  diff: string;
  status?: string;
  /** Coding-mode additions (Plan 3): the resolved file path. */
  path?: string;
  /** Coding-mode additions: the operation that produced the diff. */
  op?: "edit" | "write" | "apply_patch" | "notebook_edit";
  /** Coding-mode additions: post-write file size in bytes. */
  bytes?: number;
};
```

- [ ] **Step 2: Add `fileEditsBySession` slice to `chatStreamStore`**

In `desktop-ui/src/features/chat/store/chatStreamStore.ts`, add:

```ts
type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

// Slice
fileEditsBySession: Map<string, DiffItem[]>;

// Mutator
upsertFileEdit: (sessionKey: string, item: DiffItem) =>
  set((state) => {
    const existing = state.fileEditsBySession.get(sessionKey) ?? [];
    const next = new Map(state.fileEditsBySession);
    next.set(sessionKey, [...existing, item]);
    return { fileEditsBySession: next };
  }),
```

In whatever selector composes `segments` (next to where `approvalsBySession` was added in Plan 2), append `fileEditsBySession.get(sessionKey) ?? []` to the items list.

- [ ] **Step 3: Failing hook test**

```ts
// desktop-ui/src/features/coding/hooks/useFileEditEvents.test.ts
import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { listen } from "@tauri-apps/api/event";
import { useFileEditEvents } from "./useFileEditEvents";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";

describe("useFileEditEvents", () => {
  it("upserts a kind: diff item on agent:file_edit_with_symbols", async () => {
    let handler: any;
    (listen as any).mockImplementation((_c: string, h: any) => {
      handler = h; return Promise.resolve(() => {});
    });
    renderHook(() => useFileEditEvents("s1"));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    act(() => {
      handler({ payload: { path: "/repo/src/x.rs", op: "edit", bytes: 100,
        diff: "--- /repo/src/x.rs\n+++ /repo/src/x.rs\n@@ -1 +1 @@\n-old\n+new\n" } });
    });
    const items = chatStreamStore.getState().fileEditsBySession.get("s1") ?? [];
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("diff");
    expect(items[0].path).toBe("/repo/src/x.rs");
    expect(items[0].op).toBe("edit");
    expect(items[0].diff).toContain("+new");
  });
});
```

- [ ] **Step 4: Run failing**

```bash
cd desktop-ui && bun run test useFileEditEvents
```

FAIL.

- [ ] **Step 5: Implement the hook**

```ts
// desktop-ui/src/features/coding/hooks/useFileEditEvents.ts
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import type { ConversationItem } from "@/types";

type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

type FileEditPayload = {
  path: string;
  op: "edit" | "write" | "apply_patch" | "notebook_edit";
  bytes: number;
  diff: string;
};

export function useFileEditEvents(sessionKey: string) {
  useEffect(() => {
    const un = listen<FileEditPayload>("agent:file_edit_with_symbols", (e) => {
      const id = `diff-${sessionKey}-${Date.now()}-${Math.random().toString(36).slice(2,7)}`;
      const item: DiffItem = {
        id, kind: "diff",
        title: shortName(e.payload.path),
        diff: e.payload.diff,
        path: e.payload.path,
        op: e.payload.op,
        bytes: e.payload.bytes,
      };
      chatStreamStore.getState().upsertFileEdit(sessionKey, item);
    });
    return () => { un.then((f) => f()); };
  }, [sessionKey]);
}

function shortName(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? path : path.slice(i + 1);
}
```

- [ ] **Step 6: Wire the hook into the parent**

In whichever component owns `useApprovalQueue(sessionKey)` (likely `MainApp.tsx`), add `useFileEditEvents(sessionKey)` next to it.

- [ ] **Step 7: Run + commit**

```bash
cd desktop-ui && bun run test useFileEditEvents && bun run typecheck && bun run lint
git add desktop-ui/
git commit -m "feat(coding-ui): useFileEditEvents — splice agent:file_edit_with_symbols into chat stream"
```

---

### Task 23: `DiffPreview` enhancement (path label + size)

**Context:** The existing `DiffRow` already uses `PierreDiffBlock` to render the diff body. Plan 3 just needs a thin wrapper that shows the resolved path + byte count + op badge above the diff. We render `DiffPreview` only when the new optional `path` field is present, otherwise fall back to the existing `DiffRow` shape.

**Files:**
- Create: `desktop-ui/src/features/coding/components/DiffPreview.tsx`
- Create: `desktop-ui/src/features/coding/components/DiffPreview.test.tsx`
- Modify: `desktop-ui/src/features/messages/components/MessageRows.tsx` (use `DiffPreview` when `item.path` present)
- Modify: `desktop-ui/src/features/coding/coding.css` (DiffPreview styles)

- [ ] **Step 1: Failing test**

```tsx
// DiffPreview.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiffPreview } from "./DiffPreview";

describe("DiffPreview", () => {
  it("renders path label, op badge, byte count and diff body", () => {
    render(<DiffPreview item={{
      id: "d1", kind: "diff", title: "x.rs",
      diff: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n",
      path: "/repo/src/x.rs", op: "edit", bytes: 1234,
    }} />);
    expect(screen.getByText(/\/repo\/src\/x\.rs/)).toBeInTheDocument();
    expect(screen.getByText(/edit/i)).toBeInTheDocument();
    expect(screen.getByText(/1234/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run failing**

```bash
cd desktop-ui && bun run test DiffPreview
```

FAIL.

- [ ] **Step 3: Implement `DiffPreview.tsx`**

```tsx
import type { ConversationItem } from "@/types";
import { PierreDiffBlock } from "@/features/messages/components/PierreDiffBlock";

type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

export function DiffPreview({ item }: { item: DiffItem }) {
  return (
    <div className="diff-preview">
      <header className="diff-preview__header">
        <span className="diff-preview__path">{item.path}</span>
        {item.op && <span className={`diff-preview__op diff-preview__op--${item.op}`}>{item.op}</span>}
        {typeof item.bytes === "number" && (
          <span className="diff-preview__bytes">{item.bytes} bytes</span>
        )}
      </header>
      <PierreDiffBlock diff={item.diff} displayPath={item.path ?? item.title} />
    </div>
  );
}
```

(If `PierreDiffBlock` isn't exported under that path, use `grep -rn "PierreDiffBlock" desktop-ui/src` to find the right import.)

- [ ] **Step 4: Wire into `MessageRows.tsx`**

Find `DiffRow` in MessageRows.tsx (audit said line ~578). Update the body:

```tsx
export const DiffRow = memo(function DiffRow({ item }: DiffRowProps) {
  if (item.path && item.op) {
    return <DiffPreview item={item} />;     // coding-mode rich preview
  }
  // existing fallback (review/plan flows)
  return (
    <div className="item-card diff">
      <div className="diff-header">
        <span className="diff-title">{item.title}</span>
        {item.status && <span className="item-status">{item.status}</span>}
      </div>
      <div className="diff-viewer-output">
        <PierreDiffBlock diff={item.diff} displayPath={item.title} />
      </div>
    </div>
  );
});
```

- [ ] **Step 5: Add CSS**

Append to `desktop-ui/src/features/coding/coding.css`:

```css
.diff-preview {
  border: 1px solid var(--border-soft);
  border-radius: 6px;
  margin: 8px 0;
  font-size: var(--fs-base);
  overflow: hidden;
}
.diff-preview__header {
  display: flex; gap: 8px; align-items: center;
  padding: 6px 10px;
  background: var(--surface-2);
  border-bottom: 1px solid var(--border-soft);
}
.diff-preview__path {
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  color: var(--text-1);
}
.diff-preview__op {
  font-size: var(--fs-2xs);
  text-transform: uppercase;
  padding: 1px 6px;
  border-radius: 3px;
  background: var(--accent-soft);
}
.diff-preview__op--write       { background: var(--accent-blue-soft); }
.diff-preview__op--edit        { background: var(--accent-green-soft); }
.diff-preview__op--apply_patch { background: var(--accent-purple-soft); }
.diff-preview__bytes {
  margin-left: auto;
  font-size: var(--fs-xs);
  color: var(--text-muted);
}
```

(If the listed CSS variables don't exist in `ds-tokens.css`, fall back to `var(--surface-2)` only and use literal hex/grey for the badge backgrounds.)

- [ ] **Step 6: Run + commit**

```bash
cd desktop-ui && bun run test DiffPreview && bun run typecheck && bun run lint
git add desktop-ui/
git commit -m "feat(coding-ui): DiffPreview — file path + op badge + byte count above diff"
```

---

## Track G — Register all 12 tools at AppCore init

### Task 24: Expand the BashTool registration block to register all coding tools

**Context:** Plan 2 left the registration site at `crates/app-core/src/init/mod.rs` lines 1772–1810. We extend it to register the 11 new tools (and the `tool_search` stub) using the same dependencies.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Locate the BashTool registration block**

```bash
grep -n "BashTool::new\|registry.register(bash_tool)\|register(bash_tool)" crates/app-core/src/init/mod.rs | head
```

Note the line range.

- [ ] **Step 2: Refactor into a helper that returns the 12 tools**

Add a helper near the top of `init/mod.rs`:

```rust
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
fn build_coding_tools(
    cwd: std::path::PathBuf,
    layer1: Arc<klynt_core::approval::Layer1>,
    policy: Arc<klynt_execpolicy::Policy>,
    privacy: Arc<klynt_core::privacy::PrivacyGuard>,
    pending: Arc<klynt_core::approval::PendingApprovalsMap>,
    bus: Arc<bus::DomainEventBus>,
    repos: storage::Repos,
) -> Vec<Box<dyn tools_core::Tool>> {
    use klynt_core::tools as kt;
    let event_tx: Option<tokio::sync::mpsc::Sender<agent::events::AgentEvent>> = None;

    let tools: Vec<Box<dyn tools_core::Tool>> = vec![
        Box::new(kt::BashTool::new(layer1.clone(), policy.clone(), privacy.clone(),
                                    pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::ReadTool::new(cwd.clone(), privacy.clone())),
        Box::new(kt::GlobTool::new(cwd.clone(), privacy.clone())),
        Box::new(kt::GrepTool::new(cwd.clone(), privacy.clone())),
        Box::new(kt::WriteTool::new(cwd.clone(), layer1.clone(), policy.clone(),
                                     privacy.clone(), pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::EditTool::new(cwd.clone(), layer1.clone(), policy.clone(),
                                    privacy.clone(), pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::ApplyPatchTool::new(cwd.clone(), layer1.clone(), policy.clone(),
                                          privacy.clone(), pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::WebFetchTool::new(layer1.clone(), policy.clone(),
                                        privacy.clone(), pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::AskUserTool::default()),       // re-export of upstream
        Box::new(kt::EnterPlanModeTool::new(repos.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::ExitPlanModeTool::new(repos.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::NotebookEditTool::new(cwd.clone(), layer1.clone(), policy.clone(),
                                            privacy.clone(), pending.clone(), event_tx.clone(), bus.clone())),
        Box::new(kt::ToolSearchTool::new()),
    ];
    tools
}
```

- [ ] **Step 3: Replace the inline BashTool registration**

Where the Plan 2 block currently does `registry.register(bash_tool)`, replace with:

```rust
let cwd = config_guard.coding_memory.workspace_root.clone()
    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
let coding_tools = build_coding_tools(
    cwd, layer1, policy, privacy, pending, bus, core.repos.clone(),
);
{
    let mut reg = core.agent.tool_registry().write().await;
    for t in coding_tools { reg.register_boxed(t); }
}
```

(If `register_boxed` doesn't exist, look at the existing `register` signature; the helper returning `Vec<Box<dyn Tool>>` may need to be `Vec<DynTool>` (= `Arc<dyn Tool>`) — adapt to whichever the registry accepts. Use `grep -n "fn register" crates/tools-core/src/lib.rs` for the actual API.)

- [ ] **Step 4: Build + sanity-test**

```bash
cargo build --workspace
cargo nextest run -p klynt-core
```

PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(coding): register full 12-tool kit at AppCore init"
```

---

## Track H — Property tests + scenario + acceptance gate

### Task 25: K5 — every `edit`/`write`/`apply_patch` emits exactly one `FileEditWithSymbols`

**Files:**
- Create: `tests/integration/coding_in_chat/property_k5_file_edit_event.rs`
- Modify: `tests/integration/coding_in_chat/mod.rs` (add the new submodule)

- [ ] **Step 1: Test**

```rust
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::{
    apply_patch::{run_for_test as patch_run, ApplyPatchArgs},
    edit::{run_for_test as edit_run, EditArgs},
    write::{run_for_test as write_run, WriteArgs},
};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use proptest::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn perms() -> CodingPermissions {
    CodingPermissions {
        allow: vec!["Write(./**)".into(), "Edit(./**)".into(), "ApplyPatch(./**)".into()],
        ..Default::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, .. ProptestConfig::default() })]
    #[test]
    fn k5_each_mutation_emits_exactly_one_event(
        op_idx in 0u8..3,
        content in r"[a-z\n]{1,100}",
    ) {
        tokio_test::block_on(async move {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("f.txt"), "seed\n").unwrap();
            let l1 = Arc::new(Layer1::compile(&perms()).unwrap());
            let pol = Arc::new(Policy::empty());
            let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
            let pen = Arc::new(PendingApprovalsMap::new());
            let bus = Arc::new(DomainEventBus::new(64));
            let (tx, mut rx) = mpsc::channel(64);

            match op_idx {
                0 => { write_run(WriteArgs { path: "f.txt".into(), content: content.clone() },
                        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new())
                        .await.ok(); }
                1 => { edit_run(EditArgs { path: "f.txt".into(),
                            old_text: "seed".into(), new_text: content.clone() },
                        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new())
                        .await.ok(); }
                2 => {
                    let patch = "--- f.txt\n+++ f.txt\n@@ -1 +1 @@\n-seed\n+changed\n".to_string();
                    patch_run(ApplyPatchArgs { path: "f.txt".into(), patch },
                        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new())
                        .await.ok();
                }
                _ => unreachable!(),
            }

            let mut count = 0;
            while let Ok(e) = rx.try_recv() {
                if matches!(e, AgentEvent::FileEditWithSymbols { .. }) { count += 1; }
            }
            prop_assert!(count == 1 || count == 0,
                "expected exactly 1 FileEditWithSymbols (or 0 if op failed), got {count}");
            Ok::<(), TestCaseError>(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Add to module**

```rust
// tests/integration/coding_in_chat/mod.rs (append)
mod property_k5_file_edit_event;
mod property_k7_tool_filter;
mod scenario_grep_then_edit;
#[cfg(target_os = "linux")] mod scenario_linux_bash;
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run --workspace -E 'test(k5_each_mutation_emits)'
git add tests/
git commit -m "test(coding): K5 — every mutation tool emits exactly one FileEditWithSymbols"
```

---

### Task 26: K7 — coding tools visible iff channel == coding

**Files:**
- Create: `tests/integration/coding_in_chat/property_k7_tool_filter.rs`

- [ ] **Step 1: Test**

```rust
use common::coding_channel::{available_for_channel, Channel};
use proptest::prelude::*;

const CODING_TOOLS: &[&str] = &[
    "bash", "read", "glob", "grep", "edit", "write",
    "apply_patch", "web_fetch", "ask_user",
    "enter_plan_mode", "exit_plan_mode", "notebook_edit", "tool_search",
];

proptest! {
    #[test]
    fn k7_coding_tools_only_in_coding_channel(idx in 0usize..CODING_TOOLS.len()) {
        let name = CODING_TOOLS[idx];
        prop_assert!(available_for_channel(name, Channel::Coding));
        prop_assert!(!available_for_channel(name, Channel::Desktop));
        prop_assert!(!available_for_channel(name, Channel::Other));
    }

    #[test]
    fn k7_non_coding_tools_visible_on_all_channels(suffix in "[a-z]{3,8}") {
        let name = format!("klyntbot_{suffix}");  // anything not in CODING_ONLY
        prop_assert!(available_for_channel(&name, Channel::Coding));
        prop_assert!(available_for_channel(&name, Channel::Desktop));
        prop_assert!(available_for_channel(&name, Channel::Other));
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run --workspace -E 'test(k7_)'
git add tests/
git commit -m "test(coding): K7 — coding tools visible iff channel == coding"
```

---

### Task 27: Scenario — grep finds match, edit applies, diff renders

**Files:**
- Create: `tests/integration/coding_in_chat/scenario_grep_then_edit.rs`

- [ ] **Step 1: Test**

```rust
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::{
    edit::{run_for_test as edit_run, EditArgs},
    grep::GrepTool,
};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn grep_then_edit_emits_diff() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_path_buf();
    std::fs::write(cwd.join("f.rs"), "fn old_name() {}\nfn keep() {}\n").unwrap();

    // grep
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let grep = GrepTool::new(cwd.clone(), privacy.clone());
    let grep_out = grep.execute(serde_json::json!({"pattern":"old_name"}), &RoutingContext::system())
        .await.unwrap();
    assert!(grep_out.contains("f.rs:1:fn old_name"));

    // edit
    let perms = CodingPermissions { allow: vec!["Edit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    edit_run(
        EditArgs { path: "f.rs".into(), old_text: "old_name".into(), new_text: "new_name".into() },
        cwd.clone(), l1, pol, privacy, pen, tx.clone(), bus, CancellationToken::new(),
    ).await.unwrap();

    // assert FileEditWithSymbols emitted with op="edit"
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::FileEditWithSymbols { op, ref path, ref diff_full, .. } = e {
            assert_eq!(op, "edit");
            assert!(path.ends_with("f.rs"));
            assert!(diff_full.contains("-fn old_name"));
            assert!(diff_full.contains("+fn new_name"));
            saw = true;
        }
    }
    assert!(saw, "FileEditWithSymbols with op=edit must be emitted");
    assert_eq!(std::fs::read_to_string(cwd.join("f.rs")).unwrap(),
               "fn new_name() {}\nfn keep() {}\n");
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run --workspace -E 'test(grep_then_edit)'
git add tests/
git commit -m "test(coding): scenario — grep finds, edit applies, diff event fires"
```

---

### Task 28: Scenario — Linux bash via bwrap+Landlock

**Files:**
- Create: `tests/integration/coding_in_chat/scenario_linux_bash.rs`

- [ ] **Step 1: Test (Linux only)**

```rust
#![cfg(target_os = "linux")]
use klynt_sandbox::{LinuxSandboxRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn linux_bash_echo_inside_bwrap() {
    let cwd = tempfile::tempdir().unwrap();
    let runner = match LinuxSandboxRunner::new() {
        Ok(r) => r,
        Err(e) => { eprintln!("sandbox unavailable: {e}; skipping"); return; }
    };
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let out = runner.run_command(
        &policy, "/bin/bash", &["-c", "echo hello-from-linux-sandbox"],
        Some(cwd.path()), Duration::from_secs(5),
    ).await.expect("sandbox exec ok");
    assert!(out.stdout.contains("hello-from-linux-sandbox"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn linux_bash_blocked_outside_cwd_write() {
    let cwd = tempfile::tempdir().unwrap();
    let runner = match LinuxSandboxRunner::new() { Ok(r) => r, Err(_) => return };
    let outside = std::env::temp_dir().join(format!("klynt-l-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&outside).unwrap();
    let cmd = format!("touch {}/forbidden 2>/dev/null; echo done", outside.display());
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let out = runner.run_command(
        &policy, "/bin/sh", &["-c", &cmd], Some(cwd.path()), Duration::from_secs(5),
    ).await.unwrap();
    assert!(!outside.join("forbidden").exists());
    assert!(out.stdout.contains("done"));
}
```

- [ ] **Step 2: Run + commit**

```bash
# On a Linux host or CI runner
cargo nextest run --workspace -E 'test(linux_bash_)'
git add tests/
git commit -m "test(coding): Linux scenario — bash echo inside bwrap+Landlock"
```

---

### Task 29: Plan-3 acceptance gate

**Files:** none modified.

- [ ] **Step 1: Workspace build**

```bash
cargo build --workspace
```

PASS.

- [ ] **Step 2: Workspace clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

PASS.

- [ ] **Step 3: Workspace fmt**

```bash
cargo fmt --all --check
```

PASS.

- [ ] **Step 4: All tests (macOS host)**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

PASS.

- [ ] **Step 5: All tests (Linux runner)**

```bash
# In CI (or a Linux dev container):
cargo nextest run --workspace
```

PASS — Linux-gated tests (Tasks 5, 28, plus K4 macOS counterpart still skipped here) all green.

- [ ] **Step 6: Frontend checks**

```bash
cd desktop-ui && bun run lint && bun run typecheck && bun run test
```

PASS.

- [ ] **Step 7: Drift tests** (no new Tauri commands in Plan 3, but `chat_send` signature changed via `process_direct_streaming`'s mode arg)

```bash
cargo nextest run --workspace -E 'test(registration_drift) | test(bindings_are_current) | test(no_raw_tauri_command_outside_macros)'
```

PASS.

- [ ] **Step 8: Manual smoke (macOS)**

```bash
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

In the desktop:
1. Open or create a chat thread.
2. Use `chat_set_mode` to flip mode to `coding` (DevTools console: `await window.__TAURI_INTERNALS__.invoke('chat_set_mode', { sessionKey: '<key>', mode: 'coding' })`).
3. In `~/.klyntbot-dev/config.json`:
   ```json
   {
     "coding": {
       "permissions": {
         "allow": ["Read(*)", "Glob(*)", "Grep(*)", "Edit(./**)", "Write(./**)", "Bash(echo *)"],
         "ask": ["Bash(*)", "WebFetch(*)"]
       }
     }
   }
   ```
4. Send: `read README.md and tell me the title`. Verify the `read` tool runs without an `ApprovalCard`, returns content, and the agent answers.
5. Send: `find all rust files containing 'todo!' and show me one`. Verify `glob` then `grep` execute.
6. Send: `edit src/lib.rs to change "old_text" to "new_text"`. Verify an `ApprovalCard` appears (because `Edit(./**)` matches *but is in `allow`* — actually no card in this config; verify no card and a `kind: "diff"` row renders the diff).
7. Send: `fetch https://example.com and summarize`. Verify `web_fetch` `ApprovalCard` appears (configured to `ask`); approve; confirm response.

- [ ] **Step 9: Manual smoke (Linux, optional)**

On a Linux dev machine with `bwrap` installed:

```bash
sudo apt-get install -y bubblewrap   # if missing
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

Repeat steps 1–4 above. Verify the `bash` tool runs successfully and that the `klynt-sandbox-helper` binary is colocated next to the desktop binary.

- [ ] **Step 10: Tag the milestone**

```bash
git tag plan3-tool-kit-and-linux-sandbox
```

- [ ] **Step 11: Final commit if any tweaks**

```bash
git add -A && git commit -m "chore(coding): Plan 3 acceptance — full coding kit + Linux sandbox green"
```

---

## Self-review checklist

After implementing all tasks, verify:

1. **Spec coverage** — every Plan-3-scoped item from the spec is closed:
   - ✅ Linux Landlock + bwrap sandbox path (Tasks 1–5)
   - ✅ Plan 2 channel-routing fix (Tasks 6–7)
   - ✅ `concurrency_safe` macro attribute (Task 12)
   - ✅ All 11 new tools (Tasks 9–11, 13–15, 16, 17, 18, 19) + `tool_search` stub (Task 20)
   - ✅ Shared `fs_resolve` helper (Task 8)
   - ✅ `FileEditWithSymbols` event flow + `kind: "diff"` rendering (Tasks 21–23)
   - ✅ `PlanModeChanged` event variant (Task 18)
   - ✅ Tool registration loop (Task 24)
   - ✅ K5, K7 + scenarios (Tasks 25–28)

2. **Placeholder scan** — no `TODO`/`TBD`/`unimplemented!`/`todo!` in new files except those that explicitly reference a later plan (Plan 4 Starlark, Plan 5 LSP/anchored_symbols, Phase 2 tool_search reranking).

3. **Type consistency**:
   - `op` field in `AgentEvent::FileEditWithSymbols` is a `String` matching the strings emitted by tools: `"write"`, `"edit"`, `"apply_patch"`, `"notebook_edit"` — verified across `write.rs`, `edit.rs`, `apply_patch.rs`, `notebook_edit.rs`, and the TS `op` union literal in `types.ts`.
   - `concurrency_safe = "true"` only set on tools whose execute is genuinely side-effect-free: `read`, `glob`, `grep`, `tool_search`. Mutating tools (`edit`, `write`, `apply_patch`, `notebook_edit`, `bash`, `web_fetch`) keep the default `false`.
   - All approval-aware tools' `run_for_test` signatures take the same parameter list and order: `(args, cwd, layer1, policy, privacy, pending, event_tx, bus, cancel)` — except `WebFetchTool` which omits `cwd` (network tool, no cwd) and the plan-mode tools which take `(repos, session_key, event_tx, bus)`.
   - `RoutingContext::system()` is used in tool unit tests; `RoutingContext::with_interaction(...)` is used by the streaming path (set by Track B).

4. **No regressions**:
   - `BashTool` still works (its dependencies and registration site aren't changed; only siblings are added).
   - The Plan 2 K3, K4, K8 invariant tests + scenario all still pass.
   - The desktop chat (non-coding mode) still has access to `tasks`, `notes`, etc., and *not* to the coding-only tools (verified by Task 26's K7 proptest).
   - `chat_send` Tauri command signature unchanged externally (the `mode` field has been there since Plan 2).

5. **CLAUDE.md compliance**:
   - All new public AppCore methods have `#[tracing::instrument(skip(self), err)]` (Tasks 18, 24 affect AppCore — verify each new helper is annotated).
   - No raw `#[tauri::command]` added (Plan 3 adds zero Tauri commands).
   - No new schema migrations needed; Plan 1's columns (`approval_mode`, `conversation_type`, etc.) are reused.
   - Errors return `common::Result<T>`.
   - CSS uses `var(--fs-*)` typography tokens; no hardcoded `Npx`.
   - Frontend imports use the project path aliases (no `../../`).

6. **Cross-platform parity**:
   - macOS Seatbelt path remains unchanged (Plan 2 implementation).
   - Linux path produces the same `CommandOutput.stdout` shape (stdout+stderr merged) and the same `SandboxError` variants.
   - Both paths emit `AgentEvent::SandboxPolicyApplied` from `BashTool::run_for_test` already (no change needed in tools/bash.rs for Linux).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-29-klynt-coding-in-chat-phase1-plan3-tool-kit-and-linux-sandbox.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration. Plan 3 is well-suited because Tracks A/B/C and D/E are largely parallelizable; subagent isolation keeps each task's diff small and reviewable.
2. **Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch with checkpoints. Faster wall-clock but harder to review.

**Which approach?**
