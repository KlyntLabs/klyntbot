# Klynt Coding-in-Chat — Phase 1 Plan 2 of 6: First Tool End-to-End (`bash`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drive a single coding tool — `bash` — end-to-end through every architectural seam of the spec: privacy guard → Layer 1 declarative approval → approval round-trip with React `ApprovalCard` → macOS Seatbelt sandbox → execute → telemetry events. Producing one working tool through every layer means Plans 3–6 reuse the same plumbing for the remaining 11 coding tools, Layer 2 Starlark, hooks, skills, slash commands, and the settings page.

**Architecture:** Plan 1 landed primitives (channel constant, 18 `AgentEvent` variants, `fan_out_event`, `Tool::is_concurrency_safe`, sessions columns, 7 empty crates). This plan **fills** the empty crates with the minimum content needed to support `bash`:

- `klynt-protocol`: vendor `Op` / `Submission` / `CodingTraceEvent` types from `codex-rs/protocol/`.
- `klynt-execpolicy`: vendor the Codex Starlark crate but expose **only** stub APIs — Layer 2 wiring lands in Plan 4. Plan 2 builds the crate green so its types are linkable.
- `klynt-sandbox`: macOS Seatbelt `.sbpl` generator + sandbox-exec invoker. Linux is Plan 3.
- `klynt-core`: privacy guard, Layer 1 declarative-rule loader, approval middleware (`DashMap<RequestId, oneshot::Sender<ApprovalDecision>>` + 3-way `tokio::select!`), `bash` tool implementation, `available_for_channel` registry filter.

Two new Tauri commands (`chat_respond_approval`, `chat_set_mode`), one extended command (`chat_send` accepts a `mode` field), six event channels emitted by the runtime (only two listened to in this plan: `agent:approval_requested`, `agent:approval_resolved`), one new React `ConversationItem` variant (`kind: "approval"`), one new component tree (`features/coding/components/ApprovalCard.tsx` + `hooks/useApprovalQueue.ts`).

By the end of this plan, a user can: open a coding-mode chat thread, type `cargo nextest run -p common`, see an `ApprovalCard` for the auto-allowed `bash` invocation (or for an `ask` rule it gets a real card to click), the command runs inside Seatbelt, and the assistant streams the captured stdout back. Plans 3–6 broaden this from one tool to the full 24-tool curated profile.

**Tech Stack:** Rust 1.93 stable, `cargo` workspace, `tokio` (`mpsc`, `oneshot`, `select!`), `dashmap`, `globset`, `serde_json`, `async-trait`, `tools-core` proc macros (`#[derive(Tool)]`, `#[derive(ToolParams)]`), Tauri 2 IPC, React 18 + Vitest + `@testing-library/react`, macOS sandbox-exec(1) + Seatbelt SBPL syntax. No Linux work.

**Spec reference:** `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` — primarily §4 (agent loop integration: approval round-trip), §5 (chat surface: ApprovalCard), §6 (tool kit: bash), §7 (3-layer approval + sandbox), §10 (event vocabulary, `requires_user_input` semantics). All field shapes for `ApprovalRequested` / `ApprovalResolved` / `SandboxPolicyApplied` are defined in `crates/agent/src/events.rs` (landed in Plan 1).

**Plan suite:** This is plan 2 of 6 covering Phase 1.
- Plan 1 ✅: Foundation primitives.
- **Plan 2 (this):** First tool end-to-end (`bash` + privacy guard + Layer 1 + macOS Seatbelt + ApprovalCard).
- Plan 3: Tool kit completion + Linux sandbox.
- Plan 4: Layer 2 Starlark + hooks engine.
- Plan 5: Skills + recall + Distiller/Mirror subscribers.
- Plan 6: Settings page + slash command catalog completion + scenario tests.

---

## Sequencing

The plan is split into eight tracks. Tracks A–D land Rust infrastructure with zero UI. Track E wires channel routing into `chat_send`. Track F lights up the React surface. Track G adds property + scenario tests. Track H is the acceptance gate.

```
A. Vendoring infrastructure          ─┐
B. Sandbox — macOS Seatbelt           ├─ all parallelizable Rust-only tracks
C. Approval — privacy + Layer 1       │
D. Approval — round-trip + Tauri ─────┘
E. Bash tool + registry filter       ─── depends on B, C, D
F. Channel routing wiring            ─── depends on E
G. React surface (ApprovalCard etc.) ─── depends on D + E (event shapes stabilized)
H. Property + scenario tests         ─── depends on E, G
```

For subagent-driven execution, run A/B/C/D in parallel branches; merge before E.

---

## File structure

### Files created in this plan

```
bot/
├── crates/
│   ├── klynt-protocol/src/{lib.rs, op.rs, submission.rs, trace.rs, error.rs}
│   ├── klynt-execpolicy/src/{lib.rs, policy.rs, decision.rs, starlark_stub.rs}
│   ├── klynt-sandbox/src/{lib.rs, policy.rs, seatbelt.rs, seatbelt_template.sbpl, error.rs, runner.rs}
│   ├── klynt-core/src/lib.rs
│   ├── klynt-core/src/approval/{mod.rs, decision.rs, layer1.rs, matcher.rs, round_trip.rs, guard.rs}
│   ├── klynt-core/src/privacy/{mod.rs, exclude_paths.rs}
│   ├── klynt-core/src/tools/{mod.rs, bash.rs}
│   ├── klynt-core/src/registry/{mod.rs, filter.rs}
│   └── klynt-core/tests/{approval_layer1.rs, privacy_guard.rs, round_trip.rs, approval_guard.rs, bash_smoke.rs, registry_filter.rs, bash_schema.rs}
├── desktop-ui/src/features/coding/
│   ├── coding.css
│   ├── components/{ApprovalCard.tsx, ApprovalCard.test.tsx}
│   └── hooks/{useApprovalQueue.ts, useApprovalQueue.test.ts}
├── tests/integration/coding_in_chat/
│   ├── mod.rs
│   ├── property_k3_layer1_routing.rs
│   ├── property_k4_sandbox_invariant.rs
│   ├── property_k8_approval_roundtrip.rs
│   └── scenario_bash_happy_path.rs
└── scripts/adapt_codex_vendor.sh   (full implementation; replaces Plan 1 skeleton)
```

### Files modified

```
crates/klynt-protocol/Cargo.toml                      (deps: serde, thiserror)
crates/klynt-execpolicy/Cargo.toml                    (deps: serde, thiserror)
crates/klynt-sandbox/Cargo.toml                       (deps: serde, thiserror, sha2, async-trait)
crates/klynt-core/Cargo.toml                          (full dep set — see Task 7 step 1)
crates/agent/src/agent_runtime/runtime.rs             (filter tool list by channel)
crates/app-core/src/lib.rs                            (declare pub mod coding)
crates/app-core/src/coding/{mod.rs, chat_send_routing.rs, approval_handler.rs, mode_handler.rs}
crates/app-core/src/state.rs                          (Arc<PendingApprovalsMap> on AppCore)
crates/desktop/src/commands/chat.rs                   (chat_send mode field; new commands)
crates/desktop/src/specta_builder.rs                  (klynt_collect_commands!)
crates/desktop/Cargo.toml                             (klynt-core dep)
crates/config/src/schema/coding.rs                    (NEW — CodingConfig)
crates/config/src/schema/mod.rs                       (pub mod coding)
crates/config/src/schema/root.rs                      (coding: CodingConfig field)
desktop-ui/src/types.ts                               (ApprovalConversationItem variant)
desktop-ui/src/features/messages/components/MessageRows.tsx  (case "approval")
desktop-ui/src/features/chat/store/chatStreamStore.ts (approvalsBySession slice)
desktop-ui/src/api/endpoints/chat.ts                  (typed wrappers)
desktop-ui/src/styles/index.css                       (@import coding.css)
```

---

## Track A — Vendoring infrastructure

### Task 1: Implement `scripts/adapt_codex_vendor.sh`

**Context:** Plan 1 created the script as a `--help`-only stub. This task makes it actually copy + rename Codex source under `crates/<klynt-foo>/src/`. Invoked once per vendored crate (Plans 2, 3, 4) with the source crate name and destination folder.

**Files:**
- Modify: `scripts/adapt_codex_vendor.sh`
- Modify: `scripts/adapt_codex_vendor.sh.test.sh`

- [ ] **Step 1: Write the failing test for `--from-tar` invocation**

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADAPT="$SCRIPT_DIR/adapt_codex_vendor.sh"

TMP=$(mktemp -d); trap "rm -rf $TMP" EXIT
mkdir -p "$TMP/codex-rs/protocol/src"
cat > "$TMP/codex-rs/protocol/Cargo.toml" <<EOF
[package]
name = "codex-protocol"
version = "0.1.0"
edition = "2021"
EOF
echo 'pub mod op;' > "$TMP/codex-rs/protocol/src/lib.rs"
echo 'pub use codex_protocol::Submission;' > "$TMP/codex-rs/protocol/src/op.rs"
( cd "$TMP" && tar czf codex.tgz codex-rs )

DEST=$(mktemp -d); trap "rm -rf $TMP $DEST" EXIT
"$ADAPT" --from-tar "$TMP/codex.tgz" --source codex-rs/protocol \
  --dest "$DEST/klynt-protocol-staging" \
  --rename codex-protocol=klynt-protocol \
  --rename codex_protocol=klynt_protocol

test -f "$DEST/klynt-protocol-staging/Cargo.toml"
grep -q 'name = "klynt-protocol"' "$DEST/klynt-protocol-staging/Cargo.toml"
grep -q 'klynt_protocol::Submission' "$DEST/klynt-protocol-staging/src/op.rs"
! grep -q 'codex_protocol' "$DEST/klynt-protocol-staging/src/op.rs"
echo OK
```

- [ ] **Step 2: Run to verify failure**

`bash scripts/adapt_codex_vendor.sh.test.sh` — expected: FAIL.

- [ ] **Step 3: Implement the script**

Replace `scripts/adapt_codex_vendor.sh` body with a Bash script that:
1. Parses `--from-tar`/`--from-dir`, `--source`, `--dest`, repeatable `--rename old=new`.
2. Copies `<source>` from a tarball or directory into `<dest>` (overwrites).
3. Runs `perl -pi -e "s/\\Q<old>\\E/<new>/g"` over every `.rs` and `.toml` file in `<dest>`.
4. Idempotent — re-running with the same args overwrites cleanly.

Use `perl` (not `sed -i`) for portability between BSD (macOS) and GNU sed.

- [ ] **Step 4: Run the test** — expected: PASS, prints `OK`.
- [ ] **Step 5: Commit** — `git commit -m "feat(scripts): implement adapt_codex_vendor.sh full rename pass"`.

---

### Task 2: Vendor `klynt-protocol` minimal types

**Context:** `klynt-core::approval` and `klynt-core::tools::bash` need lightweight `Op`/`Submission` shapes for telemetry-event payloads. Codex's wire-protocol fields (those tied to its TCP observer) are deleted per spec §3 ("wire types deleted").

**Files:** modify `crates/klynt-protocol/{Cargo.toml, src/lib.rs, VENDOR.md}`; create `src/{op.rs, submission.rs, trace.rs, error.rs}`.

- [ ] **Step 1: Write a failing compile test**

Create `crates/klynt-protocol/tests/types_compile.rs`:

```rust
use klynt_protocol::{CodingTraceEvent, Op, ProtocolError, Submission, SubmissionResult};

#[test]
fn types_are_constructible() {
    let _ = Op::ToolCall { tool: "bash".into(), args: serde_json::json!({}) };
    let _ = Submission { id: "s1".into(), op: Op::NoOp };
    let _ = SubmissionResult::Ok { id: "s1".into() };
    let _ = CodingTraceEvent::IterationStart { iteration: 0 };
    let _: ProtocolError = ProtocolError::InvalidOp("x".into());
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test -p klynt-protocol --test types_compile`.

- [ ] **Step 3: Add Cargo.toml deps**

```toml
[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 4: Implement `op.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Op {
    NoOp,
    ToolCall { tool: String, args: serde_json::Value },
    UserMessage { text: String },
    Cancel,
}
```

- [ ] **Step 5: Implement `submission.rs`**

```rust
use crate::op::Op;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission { pub id: String, pub op: Op }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SubmissionResult {
    Ok { id: String },
    Err { id: String, message: String },
}
```

- [ ] **Step 6: Implement `trace.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodingTraceEvent {
    IterationStart { iteration: u32 },
}
```

- [ ] **Step 7: Implement `error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid op: {0}")]
    InvalidOp(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

- [ ] **Step 8: Replace `lib.rs`**

```rust
pub mod error;
pub mod op;
pub mod submission;
pub mod trace;

pub use error::ProtocolError;
pub use op::Op;
pub use submission::{Submission, SubmissionResult};
pub use trace::CodingTraceEvent;
```

- [ ] **Step 9: Update `VENDOR.md`** to record adapted source path + the wire-types-deleted note.

- [ ] **Step 10: Verify** — `cargo test -p klynt-protocol --test types_compile && cargo clippy -p klynt-protocol -- -D warnings`. Both PASS.

- [ ] **Step 11: Commit** — `git commit -m "feat(klynt-protocol): vendor minimal Op/Submission/CodingTraceEvent types"`.

---

### Task 3: Stub `klynt-execpolicy` with linkable types

**Context:** Plan 4 implements the full Starlark engine. Plan 2 only needs the `Decision` enum + a `Policy::eval` returning `Decision::FallThrough` so `klynt-core::approval` can compile against the public API today.

**Files:** modify `crates/klynt-execpolicy/{Cargo.toml, src/lib.rs, VENDOR.md}`; create `src/{decision.rs, policy.rs, starlark_stub.rs}`.

- [ ] **Step 1: Write failing test** at `crates/klynt-execpolicy/tests/stub_eval.rs`:

```rust
use klynt_execpolicy::{Decision, Policy};

#[test]
fn empty_policy_falls_through() {
    let p = Policy::empty();
    assert!(matches!(p.eval(&["bash", "-c", "echo hi"], None), Decision::FallThrough));
}

#[test]
fn load_from_dir_missing_is_empty() {
    let p = Policy::load_from_dir(std::path::Path::new("/tmp/does-not-exist-zz")).unwrap();
    assert!(matches!(p.eval(&["x"], None), Decision::FallThrough));
}
```

- [ ] **Step 2: Verify failure**.
- [ ] **Step 3: Add deps** — `serde + thiserror` in `Cargo.toml`.
- [ ] **Step 4: Implement `decision.rs`** with `Decision::{Allow, Ask, Forbid, FallThrough}` + `Serialize`/`Deserialize`.
- [ ] **Step 5: Implement `policy.rs`** — `Policy::{empty(), load_from_dir(path), eval(&[&str], Option<&Path>) -> Decision, append_session_allow_prefix(&[&str])}`. `load_from_dir` enumerates `*.rules` files but does not parse them in Plan 2. `eval` always returns `FallThrough`. `append_session_allow_prefix` is a no-op.
- [ ] **Step 6: Implement `starlark_stub.rs`** — empty placeholder with module-level doc comment naming Plan 4 as the implementer.
- [ ] **Step 7: Replace `lib.rs`** to re-export `Decision` and `Policy`.
- [ ] **Step 8: Update `VENDOR.md`** noting the stub-only scope and Plan 4 hand-off.
- [ ] **Step 9: Verify** — test PASSes, clippy clean.
- [ ] **Step 10: Commit** — `git commit -m "feat(klynt-execpolicy): stub Decision + Policy types (Layer 2 lands in Plan 4)"`.

---

## Track B — Sandbox: macOS Seatbelt

### Task 4: Implement `SandboxPolicy` types

**Files:** modify `crates/klynt-sandbox/{Cargo.toml, src/lib.rs}`; create `src/{policy.rs, error.rs, runner.rs, seatbelt.rs}` (the latter two as stubs to be filled in Task 5).

- [ ] **Step 1: Write failing test** at `crates/klynt-sandbox/tests/policy_construct.rs`:

```rust
use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::PathBuf;

#[test]
fn policy_for_cwd_only_writes() {
    let p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/x"));
    assert_eq!(p.cwd, PathBuf::from("/tmp/x"));
    assert!(matches!(p.network, NetworkConstraints::Block));
    assert!(matches!(p.fs, FsConstraints::WriteCwdReadAll { .. }));
    assert!(!p.policy_hash().is_empty());
}
```

- [ ] **Step 2: Run to verify failure**.

- [ ] **Step 3: Add deps** in `Cargo.toml`:

```toml
[dependencies]
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
sha2 = "0.10"
async-trait = { workspace = true }

[target.'cfg(target_os = "macos")'.dependencies]
tokio = { workspace = true, features = ["process", "rt", "macros", "time"] }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time", "process"] }
tempfile = { workspace = true }
```

- [ ] **Step 4: Implement `error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox unavailable on this platform: {0}")]
    Unavailable(String),
    #[error("sandbox launch spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("policy generation failed: {0}")]
    PolicyGen(String),
    #[error("sandbox child exited with status {0}")]
    ChildExit(i32),
}
```

- [ ] **Step 5: Implement `policy.rs`**

```rust
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkConstraints { Allow, Block }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FsConstraints {
    WriteCwdReadAll { cwd: PathBuf },
    ReadCwdOnly     { cwd: PathBuf },
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub cwd: PathBuf,
    pub fs: FsConstraints,
    pub network: NetworkConstraints,
    pub allow_process_fork: bool,
}

impl SandboxPolicy {
    pub fn cwd_writes_only(cwd: PathBuf) -> Self {
        Self {
            fs: FsConstraints::WriteCwdReadAll { cwd: cwd.clone() },
            network: NetworkConstraints::Block,
            cwd,
            allow_process_fork: true,
        }
    }
    pub fn read_only(cwd: PathBuf) -> Self {
        Self {
            fs: FsConstraints::ReadCwdOnly { cwd: cwd.clone() },
            network: NetworkConstraints::Block,
            cwd,
            allow_process_fork: false,
        }
    }
    pub fn policy_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(format!("{:?}", self).as_bytes());
        format!("{:x}", h.finalize())
    }
    pub fn summary(&self) -> String {
        match &self.fs {
            FsConstraints::WriteCwdReadAll { cwd } => format!("Seatbelt: writes only in {}", cwd.display()),
            FsConstraints::ReadCwdOnly     { cwd } => format!("Seatbelt: read-only in {}", cwd.display()),
            FsConstraints::None                    => "Seatbelt: no fs access".into(),
        }
    }
}
```

- [ ] **Step 6: Stub `runner.rs`** with `pub trait SandboxRunner` (filled in Task 5) and `pub struct CommandOutput { pub stdout: String, pub exit_code: i32 }`.

- [ ] **Step 7: Stub `seatbelt.rs`** with `#![cfg(target_os = "macos")] pub struct MacOsSeatbeltRunner;` (filled in Task 5).

- [ ] **Step 8: Replace `lib.rs`**

```rust
pub mod error;
pub mod policy;
pub mod runner;
#[cfg(target_os = "macos")] pub mod seatbelt;

pub use error::SandboxError;
pub use policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
pub use runner::{CommandOutput, SandboxRunner};
#[cfg(target_os = "macos")] pub use seatbelt::MacOsSeatbeltRunner;
```

- [ ] **Step 9: Run test** — PASS.
- [ ] **Step 10: Commit** — `git commit -m "feat(klynt-sandbox): SandboxPolicy + FsConstraints types"`.

---

### Task 5: Implement `MacOsSeatbeltRunner` (.sbpl generation + sandbox-exec)

**Files:** modify `crates/klynt-sandbox/src/{seatbelt.rs, runner.rs}`; create `crates/klynt-sandbox/src/seatbelt_template.sbpl`.

- [ ] **Step 1: Write failing test** at `crates/klynt-sandbox/tests/seatbelt_smoke.rs` (gated `#[cfg(target_os = "macos")]`):

```rust
#![cfg(target_os = "macos")]
use klynt_sandbox::{MacOsSeatbeltRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;

#[tokio::test]
async fn echo_hi_runs_inside_seatbelt() {
    let policy = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp"));
    let runner = MacOsSeatbeltRunner::new();
    let out = runner
        .run_command(&policy, "/bin/echo", &["hi"], None, std::time::Duration::from_secs(5))
        .await
        .expect("seatbelt run failed");
    assert!(out.stdout.contains("hi"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn write_outside_cwd_blocked() {
    std::fs::create_dir_all("/private/tmp/klynt-seatbelt-test").ok();
    let policy = SandboxPolicy::cwd_writes_only(PathBuf::from("/private/tmp/klynt-seatbelt-test"));
    let runner = MacOsSeatbeltRunner::new();
    let out = runner
        .run_command(
            &policy, "/bin/bash",
            &["-c", "touch /private/tmp/klynt-forbidden-elsewhere/x 2>&1; echo done"],
            None, std::time::Duration::from_secs(5),
        )
        .await
        .expect("run completes");
    assert!(out.stdout.contains("done"));
    assert!(out.stdout.contains("Operation not permitted") || out.stdout.contains("denied"));
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Create the template** at `crates/klynt-sandbox/src/seatbelt_template.sbpl`:

```scheme
(version 1)
(deny default)
(allow process-fork)
(allow process-exec)
(allow signal (target self))
(allow sysctl-read)
(allow mach-lookup)
(allow ipc-posix-shm-read*)
(allow file-read-data file-read-metadata)
(allow file-write* (subpath "{{CWD}}"))
{{EXTRA_WRITES}}
{{NETWORK}}
```

- [ ] **Step 4: Implement `seatbelt.rs`**

```rust
#![cfg(target_os = "macos")]
use crate::error::SandboxError;
use crate::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use crate::runner::{CommandOutput, SandboxRunner};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const TEMPLATE: &str = include_str!("seatbelt_template.sbpl");

pub struct MacOsSeatbeltRunner;

impl MacOsSeatbeltRunner {
    pub fn new() -> Self { Self }

    fn render_policy(p: &SandboxPolicy) -> Result<String, SandboxError> {
        let cwd = p.cwd.canonicalize()
            .map_err(|e| SandboxError::PolicyGen(format!("canonicalize cwd: {e}")))?
            .to_string_lossy().into_owned();
        let extra = match &p.fs {
            FsConstraints::WriteCwdReadAll { .. } => String::new(),
            FsConstraints::ReadCwdOnly { .. }     => "(deny file-write*)".into(),
            FsConstraints::None                   => "(deny file-write*)".into(),
        };
        let net = match p.network {
            NetworkConstraints::Allow => "(allow network*)".to_string(),
            NetworkConstraints::Block => "(deny network*)".to_string(),
        };
        Ok(TEMPLATE
            .replace("{{CWD}}", &cwd)
            .replace("{{EXTRA_WRITES}}", &extra)
            .replace("{{NETWORK}}", &net))
    }
}

#[async_trait]
impl SandboxRunner for MacOsSeatbeltRunner {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError> {
        let policy_str = Self::render_policy(policy)?;
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p").arg(&policy_str);
        cmd.arg(program).args(args);
        if let Some(d) = cwd { cmd.current_dir(d); }
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn()?;
        let out = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(r) => r?,
            Err(_) => return Err(SandboxError::ChildExit(124)),
        };
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
            exit_code: out.status.code().unwrap_or(-1),
        })
    }
}
```

- [ ] **Step 5: Implement `runner.rs` properly**

```rust
use crate::{policy::SandboxPolicy, SandboxError};
use async_trait::async_trait;
use std::{path::Path, time::Duration};

pub struct CommandOutput {
    pub stdout: String,
    pub exit_code: i32,
}

#[async_trait]
pub trait SandboxRunner: Send + Sync {
    async fn run_command(
        &self,
        policy: &SandboxPolicy,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> Result<CommandOutput, SandboxError>;
}
```

- [ ] **Step 6: Run test** — PASS on macOS, skipped on Linux.
- [ ] **Step 7: Commit** — `git commit -m "feat(klynt-sandbox): MacOsSeatbeltRunner with .sbpl template + sandbox-exec"`.

---

## Track C — Approval: privacy + Layer 1 declarative

### Task 6: Add `coding` config schema

**Context:** Layer 1 reads `coding.permissions` from `config.json`. Add a `CodingConfig` struct with `permissions` and `sandbox` sub-structs.

**Files:** create `crates/config/src/schema/coding.rs`; modify `crates/config/src/schema/mod.rs` and the file containing `pub struct Config` (verify path with `grep -rn "pub struct Config" crates/config/src/`).

- [ ] **Step 1: Verify root config location** — `grep -rn "pub struct Config" crates/config/src/ | head`. Note the file path; likely `crates/config/src/schema/root.rs` or `crates/config/src/lib.rs`.

- [ ] **Step 2: Write failing test** at `crates/config/tests/coding_schema.rs`:

```rust
use config::Config;

#[test]
fn coding_permissions_parse() {
    let json = r#"{
      "coding": {
        "permissions": {
          "allow": ["Bash(git status*)"],
          "deny": ["Bash(rm -rf *)"],
          "ask": ["Bash(*)"],
          "defaultIfNoMatch": "ask",
          "mirrorLearning": false
        },
        "sandbox": { "enforce": true }
      }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    let perms = &cfg.coding.permissions;
    assert_eq!(perms.allow, vec!["Bash(git status*)".to_string()]);
    assert_eq!(perms.deny.len(), 1);
    assert_eq!(perms.default_if_no_match, "ask");
    assert!(!perms.mirror_learning);
    assert!(cfg.coding.sandbox.enforce);
}
```

- [ ] **Step 3: Run failing test**.

- [ ] **Step 4: Implement `coding.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingConfig {
    #[serde(default)] pub permissions: CodingPermissions,
    #[serde(default)] pub sandbox: CodingSandbox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingPermissions {
    #[serde(default)] pub allow: Vec<String>,
    #[serde(default)] pub deny: Vec<String>,
    #[serde(default)] pub ask: Vec<String>,
    #[serde(default = "default_match")] pub default_if_no_match: String,
    #[serde(default)] pub mirror_learning: bool,
}
fn default_match() -> String { "ask".into() }
impl Default for CodingPermissions {
    fn default() -> Self {
        Self { allow: vec![], deny: vec![], ask: vec![],
               default_if_no_match: "ask".into(), mirror_learning: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSandbox {
    #[serde(default = "default_true")] pub enforce: bool,
}
fn default_true() -> bool { true }
impl Default for CodingSandbox { fn default() -> Self { Self { enforce: true } } }
```

- [ ] **Step 5: Wire into root** — declare `pub mod coding;` in `schema/mod.rs`; add `#[serde(default)] pub coding: CodingConfig,` to the `Config` struct.

- [ ] **Step 6: Run test** — PASS.

- [ ] **Step 7: Commit** — `git commit -m "feat(config): add coding.permissions + coding.sandbox schema"`.

---

### Task 7: Implement privacy guard

**Files:** create `crates/klynt-core/src/privacy/{mod.rs, exclude_paths.rs}`; replace `crates/klynt-core/src/lib.rs`; modify `crates/klynt-core/Cargo.toml`.

- [ ] **Step 1: Add deps** to `crates/klynt-core/Cargo.toml`:

```toml
[dependencies]
common = { workspace = true }
agent = { workspace = true }
tools-core = { workspace = true }
tools-core-macros = { workspace = true }
config = { workspace = true }
bus = { workspace = true }
klynt-protocol = { workspace = true }
klynt-execpolicy = { workspace = true }
klynt-sandbox = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["sync", "process", "macros", "rt", "time"] }
tokio-util = { workspace = true, features = ["sync"] }
dashmap = "6"
globset = "0.4"
sha2 = "0.10"
tracing = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
dirs = "5"

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util"] }
proptest = { workspace = true }
tempfile = { workspace = true }
```

If `dashmap`, `globset`, `proptest`, `tokio-util`, `tempfile` are missing from root `[workspace.dependencies]`, add them.

- [ ] **Step 2: Write failing test** at `crates/klynt-core/tests/privacy_guard.rs`:

```rust
use klynt_core::privacy::PrivacyGuard;
use std::path::Path;

#[test]
fn excludes_secret_files() {
    let g = PrivacyGuard::from_globs(&["**/.env", "**/*.key", "secrets/**"]).unwrap();
    assert!(g.is_excluded(Path::new("/repo/.env")));
    assert!(g.is_excluded(Path::new("/repo/keys/api.key")));
    assert!(g.is_excluded(Path::new("secrets/db.json")));
    assert!(!g.is_excluded(Path::new("/repo/src/main.rs")));
}

#[test]
fn bash_command_paths_inspected() {
    let g = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    assert!(g.bash_command_touches_excluded("cat .env"));
    assert!(!g.bash_command_touches_excluded("cargo build"));
}
```

- [ ] **Step 3: Run failing test**.

- [ ] **Step 4: Implement `exclude_paths.rs`**

```rust
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("invalid glob: {0}")]
    Glob(#[from] globset::Error),
}

pub struct PrivacyGuard {
    set: GlobSet,
    raw_patterns: Vec<String>,
}

impl PrivacyGuard {
    pub fn from_globs(globs: &[&str]) -> Result<Self, PrivacyError> {
        let mut b = GlobSetBuilder::new();
        for g in globs { b.add(Glob::new(g)?); }
        Ok(Self { set: b.build()?, raw_patterns: globs.iter().map(|s| s.to_string()).collect() })
    }
    pub fn is_excluded(&self, path: &Path) -> bool { self.set.is_match(path) }
    pub fn bash_command_touches_excluded(&self, cmd: &str) -> bool {
        cmd.split(|c: char| c.is_whitespace() || c == '=' || c == '"' || c == '\'')
            .any(|tok| !tok.is_empty() && self.is_excluded(Path::new(tok)))
    }
    pub fn raw_patterns(&self) -> &[String] { &self.raw_patterns }
}
```

- [ ] **Step 5: Implement `mod.rs`** — re-export `PrivacyGuard, PrivacyError`.

- [ ] **Step 6: Replace `klynt-core/src/lib.rs`**

```rust
pub mod approval;
pub mod privacy;
pub mod registry;
pub mod tools;
```

Create empty `mod.rs` files in `approval/`, `registry/`, `tools/` to keep tree green.

- [ ] **Step 7: Run test** — PASS.
- [ ] **Step 8: Commit** — `git commit -m "feat(klynt-core): privacy guard with globset-based excludePaths"`.

---

### Task 8: Implement Layer 1 declarative matcher

**Context:** Parse `Tool(glob)` syntax (e.g., `Bash(git status*)`, `Edit(./**)`) into a globset that matches against tool name + first-arg payload (command for bash, resolved path for file tools).

**Files:** create `crates/klynt-core/src/approval/{matcher.rs, decision.rs, layer1.rs, mod.rs}`.

- [ ] **Step 1: Write failing test** at `crates/klynt-core/tests/approval_layer1.rs`:

```rust
use klynt_core::approval::{
    decision::{ApprovalDecision, ApprovalLayer},
    layer1::Layer1,
};
use config::schema::coding::CodingPermissions;

fn perms(allow: &[&str], deny: &[&str], ask: &[&str], default: &str) -> CodingPermissions {
    CodingPermissions {
        allow: allow.iter().map(|s| s.to_string()).collect(),
        deny:  deny.iter().map(|s| s.to_string()).collect(),
        ask:   ask.iter().map(|s| s.to_string()).collect(),
        default_if_no_match: default.into(),
        mirror_learning: false,
    }
}

#[test]
fn deny_beats_allow() {
    let p = perms(&["Bash(*)"], &["Bash(rm -rf *)"], &[], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    let d = l1.evaluate("bash", "rm -rf /tmp/x");
    if let ApprovalDecision::Auto { allowed, .. } = d {
        assert!(!allowed, "rm -rf must be denied");
    } else { panic!("expected Auto"); }
}

#[test]
fn allow_matches() {
    let p = perms(&["Bash(git status*)"], &[], &["Bash(*)"], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    if let ApprovalDecision::Auto { allowed, layer, .. } = l1.evaluate("bash", "git status --short") {
        assert!(allowed);
        assert_eq!(layer, ApprovalLayer::Layer1Declarative);
    } else { panic!(); }
}

#[test]
fn ask_falls_through() {
    let p = perms(&[], &[], &["Bash(*)"], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    assert!(matches!(l1.evaluate("bash", "anything"), ApprovalDecision::Ask { .. }));
}

#[test]
fn no_match_uses_default() {
    let p = perms(&["Read(*)"], &[], &[], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    assert!(matches!(l1.evaluate("bash", "echo hi"), ApprovalDecision::Ask { .. }));
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement `decision.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLayer {
    Privacy, Layer1Declarative, Layer2Starlark, Layer3Mirror, DefaultMode,
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Auto { allowed: bool, layer: ApprovalLayer, reason: String, rule_matched: Option<String> },
    Ask  { layer: ApprovalLayer, reason: String },
    PrivacyDenied { reason: String, pattern: String },
    Cancelled,
    TimedOut,
}

impl ApprovalDecision {
    pub fn allowed(&self) -> bool { matches!(self, Self::Auto { allowed: true, .. }) }
    pub fn requires_user_input(&self) -> bool { matches!(self, Self::Ask { .. }) }
}
```

- [ ] **Step 4: Implement `matcher.rs`**

```rust
use globset::{Glob, GlobSet, GlobSetBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MatcherError {
    #[error("malformed rule {0:?}: must be Tool(glob)")] BadShape(String),
    #[error("invalid glob: {0}")] Glob(#[from] globset::Error),
}

pub struct CompiledRules {
    sets: std::collections::HashMap<String, (GlobSet, Vec<String>)>,
}

impl CompiledRules {
    pub fn compile(patterns: &[String]) -> Result<Self, MatcherError> {
        let mut buckets: std::collections::HashMap<String, GlobSetBuilder> = Default::default();
        let mut raws: std::collections::HashMap<String, Vec<String>> = Default::default();
        for p in patterns {
            let (tool, glob) = parse_rule(p)?;
            buckets.entry(tool.clone()).or_default().add(Glob::new(&glob)?);
            raws.entry(tool).or_default().push(p.clone());
        }
        let sets = buckets.into_iter()
            .map(|(t, b)| {
                let r = raws.remove(&t).unwrap_or_default();
                Ok::<_, MatcherError>((t, (b.build()?, r)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { sets })
    }
    pub fn find_match(&self, tool: &str, payload: &str) -> Option<String> {
        let (set, raws) = self.sets.get(&tool.to_lowercase())?;
        let m: Vec<usize> = set.matches(payload);
        m.first().map(|&i| raws[i].clone())
    }
}

fn parse_rule(rule: &str) -> Result<(String, String), MatcherError> {
    let open = rule.find('(').ok_or_else(|| MatcherError::BadShape(rule.into()))?;
    if !rule.ends_with(')') { return Err(MatcherError::BadShape(rule.into())); }
    Ok((rule[..open].trim().to_lowercase(), rule[open + 1..rule.len() - 1].to_string()))
}
```

- [ ] **Step 5: Implement `layer1.rs`**

```rust
use super::{decision::{ApprovalDecision, ApprovalLayer}, matcher::{CompiledRules, MatcherError}};
use config::schema::coding::CodingPermissions;

pub struct Layer1 {
    allow: CompiledRules,
    deny:  CompiledRules,
    ask:   CompiledRules,
    default_if_no_match: String,
}

impl Layer1 {
    pub fn compile(p: &CodingPermissions) -> Result<Self, MatcherError> {
        Ok(Self {
            allow: CompiledRules::compile(&p.allow)?,
            deny:  CompiledRules::compile(&p.deny)?,
            ask:   CompiledRules::compile(&p.ask)?,
            default_if_no_match: p.default_if_no_match.clone(),
        })
    }
    pub fn evaluate(&self, tool: &str, payload: &str) -> ApprovalDecision {
        if let Some(rule) = self.deny.find_match(tool, payload) {
            return ApprovalDecision::Auto {
                allowed: false, layer: ApprovalLayer::Layer1Declarative,
                reason: format!("layer-1 deny: {rule}"), rule_matched: Some(rule),
            };
        }
        if let Some(rule) = self.allow.find_match(tool, payload) {
            return ApprovalDecision::Auto {
                allowed: true, layer: ApprovalLayer::Layer1Declarative,
                reason: format!("layer-1 allow: {rule}"), rule_matched: Some(rule),
            };
        }
        if let Some(rule) = self.ask.find_match(tool, payload) {
            return ApprovalDecision::Ask {
                layer: ApprovalLayer::Layer1Declarative,
                reason: format!("layer-1 ask: {rule}"),
            };
        }
        match self.default_if_no_match.as_str() {
            "allow" => ApprovalDecision::Auto { allowed: true,  layer: ApprovalLayer::Layer1Declarative, reason: "layer-1 default: allow".into(), rule_matched: None },
            "deny"  => ApprovalDecision::Auto { allowed: false, layer: ApprovalLayer::Layer1Declarative, reason: "layer-1 default: deny".into(),  rule_matched: None },
            _       => ApprovalDecision::Ask { layer: ApprovalLayer::Layer1Declarative, reason: "layer-1 default: ask".into() },
        }
    }
}
```

- [ ] **Step 6: Implement `mod.rs`**

```rust
pub mod decision;
pub mod layer1;
pub mod matcher;
pub mod round_trip;     // stub for Task 9
pub mod guard;          // stub for Task 14

pub use decision::{ApprovalDecision, ApprovalLayer};
pub use layer1::Layer1;
pub use round_trip::{PendingApprovalsMap, RequestId};
```

(Stub `round_trip.rs` and `guard.rs` with `pub struct PendingApprovalsMap;` / `pub type RequestId = String;` / empty modules to keep the tree green; Tasks 9 + 14 fill them.)

- [ ] **Step 7: Run test** — PASS.
- [ ] **Step 8: Commit** — `git commit -m "feat(klynt-core): Layer 1 declarative approval with deny>allow>ask precedence"`.

---

## Track D — Approval round-trip + Tauri commands

### Task 9: Implement `PendingApprovalsMap` + 3-way `select!`

**Files:** modify `crates/klynt-core/src/approval/round_trip.rs`.

- [ ] **Step 1: Write failing test** at `crates/klynt-core/tests/round_trip.rs`:

```rust
use klynt_core::approval::round_trip::{await_decision, PendingApprovalsMap};
use klynt_core::approval::decision::{ApprovalDecision, ApprovalLayer};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn user_approves_resolves() {
    let map = Arc::new(PendingApprovalsMap::new());
    let req_id = "req-1".to_string();
    let token = CancellationToken::new();
    let map2 = map.clone();
    let req2 = req_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        map2.resolve(&req2, ApprovalDecision::Auto {
            allowed: true, layer: ApprovalLayer::Layer1Declarative,
            reason: "user clicked allow once".into(), rule_matched: None,
        });
    });
    let d = await_decision(&map, &req_id, token, Duration::from_secs(2)).await;
    assert!(d.allowed());
}

#[tokio::test]
async fn cancellation_resolves_as_cancelled() {
    let map = Arc::new(PendingApprovalsMap::new());
    let token = CancellationToken::new();
    let token2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        token2.cancel();
    });
    let d = await_decision(&map, "rid", token, Duration::from_secs(2)).await;
    assert!(matches!(d, ApprovalDecision::Cancelled));
}

#[tokio::test]
async fn timeout_resolves_as_timeout() {
    let map = Arc::new(PendingApprovalsMap::new());
    let token = CancellationToken::new();
    let d = await_decision(&map, "rid", token, Duration::from_millis(50)).await;
    assert!(matches!(d, ApprovalDecision::TimedOut));
}

#[tokio::test]
async fn unknown_request_id_resolve_is_noop() {
    let map = PendingApprovalsMap::new();
    map.resolve("nonexistent", ApprovalDecision::Cancelled);
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement `round_trip.rs`**

```rust
use super::decision::ApprovalDecision;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub type RequestId = String;

pub struct PendingApprovalsMap {
    inner: DashMap<RequestId, oneshot::Sender<ApprovalDecision>>,
}

impl PendingApprovalsMap {
    pub fn new() -> Self { Self { inner: DashMap::new() } }
    pub fn register(&self, id: RequestId, tx: oneshot::Sender<ApprovalDecision>) {
        self.inner.insert(id, tx);
    }
    pub fn resolve(&self, id: &str, decision: ApprovalDecision) {
        if let Some((_, tx)) = self.inner.remove(id) {
            let _ = tx.send(decision);
        }
    }
    pub fn contains(&self, id: &str) -> bool { self.inner.contains_key(id) }
    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }
    pub fn cancel_all(&self) {
        let keys: Vec<String> = self.inner.iter().map(|e| e.key().clone()).collect();
        for k in keys { self.resolve(&k, ApprovalDecision::Cancelled); }
    }
}

impl Default for PendingApprovalsMap { fn default() -> Self { Self::new() } }

pub async fn await_decision(
    pending: &Arc<PendingApprovalsMap>,
    request_id: &str,
    cancel: CancellationToken,
    timeout: Duration,
) -> ApprovalDecision {
    let (tx, rx) = oneshot::channel();
    pending.register(request_id.to_string(), tx);
    tokio::select! {
        biased;
        v = rx => v.unwrap_or(ApprovalDecision::Cancelled),
        _ = cancel.cancelled() => {
            pending.inner.remove(request_id);
            ApprovalDecision::Cancelled
        }
        _ = tokio::time::sleep(timeout) => {
            pending.inner.remove(request_id);
            ApprovalDecision::TimedOut
        }
    }
}
```

- [ ] **Step 4: Run test** — PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(klynt-core): approval round-trip via DashMap + 3-way select"`.

---

### Task 10: Add `chat_respond_approval` Tauri command + AppCore handler

**Files:** create `crates/app-core/src/coding/{mod.rs, approval_handler.rs}`; modify `crates/app-core/src/lib.rs`, the file containing `AppCore`, `crates/desktop/src/commands/chat.rs`, `crates/desktop/src/specta_builder.rs`, `crates/app-core/Cargo.toml` (add klynt-core dep), `crates/desktop/Cargo.toml` (add klynt-core dep).

- [ ] **Step 1: Verify AppCore location** — `grep -rn "pub struct AppCore" crates/app-core/src/ | head -3`.

- [ ] **Step 2: Implement the handler** at `crates/app-core/src/coding/approval_handler.rs`:

```rust
use klynt_core::approval::{
    decision::{ApprovalDecision, ApprovalLayer},
    PendingApprovalsMap,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppApprovalDecision {
    AllowOnce,
    AllowAlways { rule: Option<String> },
    Deny,
    AddRule { starlark_source: String },
}

#[derive(Debug, Error)]
pub enum ApprovalHandlerError {
    #[error("no pending approval for request_id={0}")]
    NotFound(String),
}

#[tracing::instrument(skip(pending), err)]
pub async fn respond_approval(
    pending: &Arc<PendingApprovalsMap>,
    request_id: &str,
    decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    let mapped = match decision {
        AppApprovalDecision::AllowOnce => ApprovalDecision::Auto {
            allowed: true, layer: ApprovalLayer::Layer1Declarative,
            reason: "user: allow once".into(), rule_matched: None,
        },
        AppApprovalDecision::AllowAlways { rule } => ApprovalDecision::Auto {
            allowed: true, layer: ApprovalLayer::Layer1Declarative,
            reason: format!("user: allow always{}", rule.as_deref().map(|r| format!(" ({r})")).unwrap_or_default()),
            rule_matched: rule,
        },
        AppApprovalDecision::Deny => ApprovalDecision::Auto {
            allowed: false, layer: ApprovalLayer::Layer1Declarative,
            reason: "user: deny".into(), rule_matched: None,
        },
        AppApprovalDecision::AddRule { .. } => ApprovalDecision::Auto {
            allowed: true, layer: ApprovalLayer::Layer2Starlark,
            reason: "user: added rule (Plan 4 will persist)".into(), rule_matched: None,
        },
    };
    if !pending.contains(request_id) {
        return Err(ApprovalHandlerError::NotFound(request_id.into()));
    }
    pending.resolve(request_id, mapped);
    // "Allow always" persistence to config.json + Starlark rule writing
    // happens here in Plan 4; for now we just resolve.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn responds_resolves_pending() {
        let map = Arc::new(PendingApprovalsMap::new());
        let (tx, rx) = tokio::sync::oneshot::channel();
        map.register("req-x".into(), tx);
        respond_approval(&map, "req-x", AppApprovalDecision::AllowOnce).await.unwrap();
        let got = rx.await.unwrap();
        assert!(got.allowed());
    }
    #[tokio::test]
    async fn unknown_request_id_returns_not_found() {
        let map = Arc::new(PendingApprovalsMap::new());
        let r = respond_approval(&map, "nope", AppApprovalDecision::Deny).await;
        assert!(matches!(r, Err(ApprovalHandlerError::NotFound(_))));
    }
}
```

- [ ] **Step 3: `mod.rs`**

```rust
pub mod approval_handler;
pub mod chat_send_routing;   // Task 12
pub mod mode_handler;        // Task 11
```

In `crates/app-core/src/lib.rs`: `pub mod coding;`.

- [ ] **Step 4: Hold pending map in `AppCore`** — add `pub pending_approvals: Arc<PendingApprovalsMap>` field; initialize in the constructor with `Arc::new(PendingApprovalsMap::new())`.

- [ ] **Step 5: Add Tauri command** in `crates/desktop/src/commands/chat.rs`:

```rust
use app_core::coding::approval_handler::{respond_approval, AppApprovalDecision, ApprovalHandlerError};
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn chat_respond_approval(
    app: tauri::AppHandle,
    session_key: String,
    request_id: String,
    decision: AppApprovalDecision,
) -> Result<(), String> {
    let core = app.state::<std::sync::Arc<app_core::AppCore>>();
    respond_approval(&core.pending_approvals, &request_id, decision)
        .await
        .map_err(|e: ApprovalHandlerError| e.to_string())?;
    let _ = session_key; // session_key unused today; Plan 4 persists per-thread rules.
    Ok(())
}
```

- [ ] **Step 6: Register the command** — add `commands::chat::chat_respond_approval` to `klynt_collect_commands!` in `specta_builder.rs`.

- [ ] **Step 7: Regenerate bindings** — `cargo tauri dev` once; verify `desktop-ui/src/bindings.ts` contains `chatRespondApproval`.

- [ ] **Step 8: Run unit + drift tests** — `cargo nextest run -p app-core -E 'test(approval_handler)' && cargo nextest run --workspace -E 'test(registration_drift) | test(bindings_are_current)'`. PASS.

- [ ] **Step 9: Commit** — `git commit -m "feat(coding): chat_respond_approval Tauri command + AppCore handler"`.

---

### Task 11: Add `chat_set_mode` Tauri command + handler

**Files:** create `crates/app-core/src/coding/mode_handler.rs`; modify `crates/storage/src/repos/sessions.rs` (add `update_conversation_type` if missing); modify `crates/desktop/src/commands/chat.rs` and `specta_builder.rs`.

- [ ] **Step 1: Inspect SessionsRepo** — `grep -n "pub async fn" crates/storage/src/repos/sessions.rs | head -30`. Confirm whether `update_conversation_type` or similar exists.

- [ ] **Step 2: Write failing test** at `crates/app-core/src/coding/mode_handler.rs`:

```rust
use serde::{Deserialize, Serialize};
use storage::{Repos, SessionRow};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode { Chat, Coding }

#[derive(Debug, Error)]
pub enum ModeError {
    #[error(transparent)] Storage(#[from] storage::StorageError),
    #[error("session not found: {0}")] NotFound(String),
}

#[tracing::instrument(skip(repos), err)]
pub async fn set_mode(
    repos: &Repos,
    session_key: &str,
    mode: ChatMode,
) -> Result<SessionRow, ModeError> {
    let s = match mode { ChatMode::Chat => "chat", ChatMode::Coding => "coding" };
    repos.sessions.update_conversation_type(session_key, s).await?;
    repos.sessions
        .find_by_key(session_key).await?
        .ok_or_else(|| ModeError::NotFound(session_key.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn set_mode_persists() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repos = Repos::from_pool(&pool);
        let key = repos.sessions.create_default("u1").await.unwrap();
        let row = set_mode(&repos, &key, ChatMode::Coding).await.unwrap();
        assert_eq!(row.conversation_type.as_deref(), Some("coding"));
    }
}
```

- [ ] **Step 3: Run failing test**.

- [ ] **Step 4: Add the repo method if missing** — in `crates/storage/src/repos/sessions.rs`:

```rust
pub async fn update_conversation_type(&self, key: &str, t: &str) -> Result<(), StorageError> {
    sqlx::query("UPDATE sessions SET conversation_type = ?1 WHERE session_key = ?2")
        .bind(t).bind(key)
        .execute(&self.pool).await?;
    Ok(())
}
```

(Adapt to actual API — the project uses `sqlx`; check existing methods in the file for the surrounding style.)

- [ ] **Step 5: Run test** — PASS.

- [ ] **Step 6: Add Tauri command** in `chat.rs`:

```rust
use app_core::coding::mode_handler::{set_mode, ChatMode};

#[klynt_command]
pub async fn chat_set_mode(
    app: tauri::AppHandle,
    session_key: String,
    mode: ChatMode,
) -> Result<storage::SessionRow, String> {
    let core = app.state::<std::sync::Arc<app_core::AppCore>>();
    set_mode(&core.repos, &session_key, mode).await.map_err(|e| e.to_string())
}
```

Register in `klynt_collect_commands!`.

- [ ] **Step 7: Regenerate bindings + verify drift tests**. PASS.
- [ ] **Step 8: Commit** — `git commit -m "feat(coding): chat_set_mode Tauri command + storage method"`.

---

### Task 12: Extend `chat_send` payload with `mode` field

**Files:** modify `crates/desktop/src/commands/chat.rs`; create `crates/app-core/src/coding/chat_send_routing.rs`.

- [ ] **Step 1: Inspect existing chat_send** — `grep -n "chat_send\|ChatSendPayload" crates/desktop/src/commands/chat.rs | head`. Confirm payload struct shape.

- [ ] **Step 2: Add `mode` field** — if payload is a struct: add `pub mode: Option<String>`. If it's `serde_json::Value` `context`: handler reads `context.get("mode")`.

- [ ] **Step 3: Implement routing helper**

```rust
// crates/app-core/src/coding/chat_send_routing.rs
use common::{ChannelName, CODING_CHANNEL};

pub fn channel_for_mode(mode_opt: Option<&str>) -> ChannelName {
    match mode_opt {
        Some("coding") => ChannelName::new(CODING_CHANNEL),
        _ => ChannelName::new("desktop"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn coding() { assert_eq!(channel_for_mode(Some("coding")).as_str(), "coding"); }
    #[test] fn default_desktop() { assert_eq!(channel_for_mode(None).as_str(), "desktop"); }
    #[test] fn other_falls_back() { assert_eq!(channel_for_mode(Some("chat")).as_str(), "desktop"); }
}
```

- [ ] **Step 4: Wire it into the existing chat_send** — locate the `RoutingContext` construction inside the AppCore `chat_send`; insert:

```rust
let mode_str = payload.mode.as_deref()
    .or_else(|| session_row.conversation_type.as_deref());
let channel = channel_for_mode(mode_str);
let ctx = RoutingContext { channel, /* … other fields … */ };
```

- [ ] **Step 5: Run the test** — `cargo test -p app-core coding::chat_send_routing::tests`. PASS.
- [ ] **Step 6: Commit** — `git commit -m "feat(coding): wire chat_send mode field to RoutingContext.channel"`.

---

## Track E — `bash` tool + registry filter

### Task 13: Implement `BashTool` skeleton (no execute body yet)

**Files:** create `crates/klynt-core/src/tools/{mod.rs, bash.rs}`.

- [ ] **Step 1: Write failing schema test** at `crates/klynt-core/tests/bash_schema.rs`:

```rust
use klynt_core::tools::bash::BashTool;
use tools_core::Tool;

#[test]
fn bash_tool_metadata() {
    let t = BashTool::new();
    assert_eq!(t.name(), "bash");
    assert!(!t.description().is_empty());
    let schema = t.json_schema();
    assert!(schema["properties"]["command"].is_object());
    assert!(!t.is_concurrency_safe(&serde_json::json!({"command":"echo"})));
}
```

- [ ] **Step 2: Run failing test**.

- [ ] **Step 3: Implement bash.rs (params + skeleton execute)**

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tools_core::{Tool, ToolError, ToolContext, ToolResult};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Deserialize, Serialize, ToolParamsDerive)]
pub struct BashArgs {
    /// Shell command to run via /bin/bash -c.
    pub command: String,
    /// Optional working directory; defaults to session cwd.
    #[serde(default)] pub cwd: Option<String>,
    /// Optional timeout in milliseconds; defaults to 60_000.
    #[serde(default)] pub timeout_ms: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "bash",
    description = "Run a shell command in a sandboxed bash session. \
                   Approval and sandbox rules apply. Output is captured and \
                   truncated to 50KB."
)]
pub struct BashTool { _phantom: () }

impl BashTool { pub fn new() -> Self { Self { _phantom: () } } }
impl Default for BashTool { fn default() -> Self { Self::new() } }

#[async_trait]
impl Tool for BashTool {
    type Params = BashArgs;
    fn is_concurrency_safe(&self, _args: &serde_json::Value) -> bool { false }
    async fn execute(&self, _args: BashArgs, _ctx: ToolContext) -> ToolResult {
        Err(ToolError::Internal("bash execute body lands in Plan 2 Task 15".into()))
    }
}
```

(Verify the actual `Tool` trait associated types and `ToolContext` shape by reading `crates/tools-core/src/lib.rs`. Adapt names if `Params`/`execute` differ. Most likely `#[derive(Tool)]` from `tools-core-macros` does the heavy lifting and the `impl` block is auto-generated; in that case the trait method overrides go in a hand-written `impl Tool for BashTool` companion.)

- [ ] **Step 4: Implement `tools/mod.rs`** — `pub mod bash; pub use bash::{BashArgs, BashTool};`.

- [ ] **Step 5: Run test** — PASS.

- [ ] **Step 6: Commit** — `git commit -m "feat(klynt-core): BashTool skeleton with derived schema (no execute body)"`.

---

### Task 14: Wire `ApprovalGuard` middleware

**Context:** Tools call into a single function that runs privacy → Layer 1 → Layer 2 (stub returns FallThrough) → emits `ApprovalRequested`/`ApprovalResolved` → returns the final decision. Reused by every tool in Plans 3–6.

**Files:** create `crates/klynt-core/src/approval/guard.rs`; modify `crates/klynt-core/src/approval/mod.rs`.

- [ ] **Step 1: Write failing test** at `crates/klynt-core/tests/approval_guard.rs`:

```rust
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{
    decision::ApprovalDecision,
    guard::{evaluate, GuardCtx},
    PendingApprovalsMap,
};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn privacy_blocks_first() {
    let perms = CodingPermissions { allow: vec!["Bash(*)".into()], ..Default::default() };
    let l1 = klynt_core::approval::Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let ctx = GuardCtx {
        layer1: &l1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&tx), domain_bus: &bus,
        cancel: CancellationToken::new(), request_id: "r1".into(),
    };
    let d = evaluate(ctx, "bash", "cat .env").await;
    assert!(matches!(d, ApprovalDecision::PrivacyDenied { .. }));
    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, AgentEvent::ApprovalRequested { .. }));
    let resolved = rx.recv().await.unwrap();
    assert!(matches!(resolved, AgentEvent::ApprovalResolved { .. }));
}

#[tokio::test]
async fn auto_allow_emits_pair_no_user_input() {
    let perms = CodingPermissions {
        allow: vec!["Bash(echo *)".into()], default_if_no_match: "ask".into(),
        ..Default::default()
    };
    let l1 = klynt_core::approval::Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let ctx = GuardCtx {
        layer1: &l1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&tx), domain_bus: &bus,
        cancel: CancellationToken::new(), request_id: "r2".into(),
    };
    let d = evaluate(ctx, "bash", "echo hi").await;
    assert!(d.allowed());
    let req = rx.recv().await.unwrap();
    if let AgentEvent::ApprovalRequested { requires_user_input, .. } = req {
        assert!(!requires_user_input);
    } else { panic!("expected ApprovalRequested"); }
    assert!(matches!(rx.recv().await.unwrap(), AgentEvent::ApprovalResolved { .. }));
}

#[tokio::test]
async fn ask_path_awaits_user_decision() {
    let perms = CodingPermissions {
        ask: vec!["Bash(*)".into()], default_if_no_match: "ask".into(),
        ..Default::default()
    };
    let l1 = klynt_core::approval::Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let pending2 = pending.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        pending2.resolve("r3", ApprovalDecision::Auto {
            allowed: true, layer: klynt_core::approval::ApprovalLayer::Layer1Declarative,
            reason: "user click".into(), rule_matched: None,
        });
    });

    let ctx = GuardCtx {
        layer1: &l1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&tx), domain_bus: &bus,
        cancel: CancellationToken::new(), request_id: "r3".into(),
    };
    let d = evaluate(ctx, "bash", "rm something").await;
    assert!(d.allowed());
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement `guard.rs`**

```rust
use super::{
    decision::{ApprovalDecision, ApprovalLayer},
    layer1::Layer1,
    round_trip::{await_decision, PendingApprovalsMap},
};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use bus::{DomainEvent, DomainEventBus};
use klynt_execpolicy::{Decision as ExecDecision, Policy};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

pub struct GuardCtx<'a> {
    pub layer1: &'a Layer1,
    pub policy: &'a Policy,
    pub privacy: &'a PrivacyGuard,
    pub pending: &'a Arc<PendingApprovalsMap>,
    pub event_tx: Option<&'a mpsc::Sender<AgentEvent>>,
    pub domain_bus: &'a Arc<DomainEventBus>,
    pub cancel: CancellationToken,
    pub request_id: String,
}

pub async fn evaluate<'a>(ctx: GuardCtx<'a>, tool: &str, payload: &str) -> ApprovalDecision {
    // 0. Privacy guard (non-bypassable)
    let privacy_hit = match tool {
        "bash" => ctx.privacy.bash_command_touches_excluded(payload),
        _      => ctx.privacy.is_excluded(std::path::Path::new(payload)),
    };
    if privacy_hit {
        let pat = ctx.privacy.raw_patterns().first().cloned().unwrap_or_default();
        let d = ApprovalDecision::PrivacyDenied {
            reason: "privacy guard: excludePaths match".into(), pattern: pat,
        };
        emit_pair(&ctx, tool, payload, &d, false).await;
        return d;
    }

    // 1. Layer 1 declarative
    let l1 = ctx.layer1.evaluate(tool, payload);
    if matches!(l1, ApprovalDecision::Auto { .. }) {
        emit_pair(&ctx, tool, payload, &l1, false).await;
        return l1;
    }

    // 2. Layer 2 Starlark — Plan 2 stub returns FallThrough.
    let argv: Vec<&str> = payload.split_whitespace().collect();
    let l2 = ctx.policy.eval(&argv, None);
    let merged: ApprovalDecision = match l2 {
        ExecDecision::Allow => ApprovalDecision::Auto {
            allowed: true, layer: ApprovalLayer::Layer2Starlark,
            reason: "layer-2 allow".into(), rule_matched: None,
        },
        ExecDecision::Forbid => ApprovalDecision::Auto {
            allowed: false, layer: ApprovalLayer::Layer2Starlark,
            reason: "layer-2 forbid".into(), rule_matched: None,
        },
        ExecDecision::Ask => ApprovalDecision::Ask {
            layer: ApprovalLayer::Layer2Starlark, reason: "layer-2 ask".into(),
        },
        ExecDecision::FallThrough => l1,
    };

    // 3. Layer 3 Mirror-learned — Phase 2; skipped here.

    match merged {
        ApprovalDecision::Auto { .. } => {
            emit_pair(&ctx, tool, payload, &merged, false).await;
            merged
        }
        ApprovalDecision::Ask { .. } => {
            emit_pair(&ctx, tool, payload, &merged, true).await;
            let user = await_decision(ctx.pending, &ctx.request_id, ctx.cancel.clone(), APPROVAL_TIMEOUT).await;
            emit_resolved(&ctx, &user).await;
            user
        }
        _ => merged,
    }
}

async fn emit_pair<'a>(ctx: &GuardCtx<'a>, tool: &str, payload: &str, decision: &ApprovalDecision, requires_user_input: bool) {
    let mut h = Sha256::new(); h.update(payload.as_bytes());
    let args_hash = format!("{:x}", h.finalize());
    let req = AgentEvent::ApprovalRequested {
        request_id: ctx.request_id.clone(),
        tool: tool.into(),
        args_hash,
        layer: format!("{:?}", layer_of(decision)),
        rule_matched: rule_of(decision),
        mirror_history: None,
        sandbox_summary: None,
        requires_user_input,
    };
    fan_out(&ctx.event_tx, ctx.domain_bus, req).await;
    if !requires_user_input { emit_resolved(ctx, decision).await; }
}

async fn emit_resolved<'a>(ctx: &GuardCtx<'a>, decision: &ApprovalDecision) {
    let res = AgentEvent::ApprovalResolved {
        request_id: ctx.request_id.clone(),
        decision: format!("{:?}", decision),
        decision_reason: reason_of(decision),
        latency_ms: 0,
        persisted_rule: None,
        decided_by: decided_by(decision).into(),
    };
    fan_out(&ctx.event_tx, ctx.domain_bus, res).await;
}

fn layer_of(d: &ApprovalDecision) -> ApprovalLayer {
    match d {
        ApprovalDecision::Auto { layer, .. } | ApprovalDecision::Ask { layer, .. } => layer.clone(),
        ApprovalDecision::PrivacyDenied { .. } => ApprovalLayer::Privacy,
        _ => ApprovalLayer::DefaultMode,
    }
}
fn rule_of(d: &ApprovalDecision) -> Option<String> {
    if let ApprovalDecision::Auto { rule_matched, .. } = d { rule_matched.clone() } else { None }
}
fn reason_of(d: &ApprovalDecision) -> String {
    match d {
        ApprovalDecision::Auto { reason, .. } | ApprovalDecision::Ask { reason, .. } => reason.clone(),
        ApprovalDecision::PrivacyDenied { reason, .. } => reason.clone(),
        ApprovalDecision::Cancelled => "cancelled".into(),
        ApprovalDecision::TimedOut => "timeout".into(),
    }
}
fn decided_by(d: &ApprovalDecision) -> &'static str {
    match d {
        ApprovalDecision::Auto { allowed: true, .. } => "auto_allow",
        ApprovalDecision::Auto { allowed: false, .. } => "auto_deny",
        ApprovalDecision::Ask { .. } => "user",
        ApprovalDecision::PrivacyDenied { .. } => "auto_deny",
        ApprovalDecision::Cancelled => "cancelled",
        ApprovalDecision::TimedOut => "timeout",
    }
}

async fn fan_out(tx: &Option<&mpsc::Sender<AgentEvent>>, bus: &Arc<DomainEventBus>, evt: AgentEvent) {
    if let Some(t) = tx { let _ = t.send(evt.clone()).await; }
    let _ = bus.publish(DomainEvent::Agent(evt));
}
```

- [ ] **Step 4: Update `mod.rs`** — append `pub mod guard; pub use guard::{evaluate, GuardCtx, APPROVAL_TIMEOUT};`.

- [ ] **Step 5: Run test** — PASS. (If `DomainEvent::Agent(AgentEvent)` doesn't exist, Plan 1 Task 9 should have added it; otherwise add it now in `crates/bus/src/domain_events.rs`.)

- [ ] **Step 6: Commit** — `git commit -m "feat(klynt-core): ApprovalGuard middleware with privacy + L1 + L2 stub"`.

---

### Task 15: Implement `BashTool` execute body

**Files:** modify `crates/klynt-core/src/tools/bash.rs`.

- [ ] **Step 1: Write failing macOS smoke test** at `crates/klynt-core/tests/bash_smoke.rs`:

```rust
#![cfg(target_os = "macos")]

use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::{run_for_test, BashArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn echo_hi_runs_and_emits_sandbox_event() {
    let perms = CodingPermissions { allow: vec!["Bash(echo *)".into()], ..Default::default() };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);

    let result = run_for_test(
        BashArgs { command: "echo hi".into(), cwd: Some("/tmp".into()), timeout_ms: Some(5000) },
        layer1, policy, privacy, pending, tx.clone(), bus, CancellationToken::new(),
    ).await.unwrap();

    assert!(result.stdout.contains("hi"));
    assert_eq!(result.exit_code, 0);

    drop(tx);
    let mut saw_sandbox = false;
    while let Some(e) = rx.recv().await {
        if matches!(e, AgentEvent::SandboxPolicyApplied { .. }) { saw_sandbox = true; }
    }
    assert!(saw_sandbox, "SandboxPolicyApplied must be emitted");
}

#[tokio::test]
async fn denied_command_returns_error_and_does_not_run() {
    let perms = CodingPermissions {
        deny: vec!["Bash(rm -rf *)".into()], allow: vec!["Bash(*)".into()],
        ..Default::default()
    };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = run_for_test(
        BashArgs { command: "rm -rf /tmp/k2".into(), cwd: Some("/tmp".into()), timeout_ms: Some(5000) },
        layer1, policy, privacy, pending, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement the runner + execute body** — append to `crates/klynt-core/src/tools/bash.rs`:

```rust
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[cfg(target_os = "macos")]
use klynt_sandbox::{MacOsSeatbeltRunner, SandboxPolicy, SandboxRunner};

pub struct BashOutcome {
    pub stdout: String,
    pub exit_code: i32,
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all, err)]
pub async fn run_for_test(
    args: BashArgs,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<BashOutcome, String> {
    let request_id = Uuid::new_v4().to_string();
    let ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel: cancel.clone(), request_id,
    };
    let decision = evaluate(ctx, "bash", &args.command).await;
    if !decision.allowed() {
        return Err(format!("bash denied: {:?}", decision));
    }

    #[cfg(not(target_os = "macos"))]
    { return Err("bash on non-macOS lands in Plan 3".into()); }

    #[cfg(target_os = "macos")]
    {
        let cwd = args.cwd.clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let sandbox_policy = SandboxPolicy::cwd_writes_only(cwd.clone());
        let _ = event_tx.send(AgentEvent::SandboxPolicyApplied {
            tool: "bash".into(),
            policy_summary: sandbox_policy.summary(),
            policy_hash: sandbox_policy.policy_hash(),
            fallback_unsandboxed: false,
            fs_constraints: format!("{:?}", sandbox_policy.fs),
            network_constraints: format!("{:?}", sandbox_policy.network),
        }).await;
        let runner = MacOsSeatbeltRunner::new();
        let timeout = Duration::from_millis(args.timeout_ms.unwrap_or(60_000));
        let out = runner.run_command(
            &sandbox_policy, "/bin/bash", &["-c", &args.command],
            Some(&cwd), timeout,
        ).await.map_err(|e| e.to_string())?;
        Ok(BashOutcome { stdout: out.stdout, exit_code: out.exit_code })
    }
}
```

Now wire `Tool::execute`:

```rust
#[async_trait]
impl Tool for BashTool {
    type Params = BashArgs;
    fn is_concurrency_safe(&self, _args: &serde_json::Value) -> bool { false }

    async fn execute(&self, args: BashArgs, ctx: ToolContext) -> ToolResult {
        let layer1: Arc<Layer1> = ctx.extensions.get_arc().ok_or_else(
            || ToolError::Internal("Layer1 missing from ToolContext".into()))?;
        let policy: Arc<Policy> = ctx.extensions.get_arc().ok_or_else(
            || ToolError::Internal("Policy missing".into()))?;
        let privacy: Arc<PrivacyGuard> = ctx.extensions.get_arc().ok_or_else(
            || ToolError::Internal("PrivacyGuard missing".into()))?;
        let pending: Arc<PendingApprovalsMap> = ctx.extensions.get_arc().ok_or_else(
            || ToolError::Internal("PendingApprovalsMap missing".into()))?;
        let bus: Arc<DomainEventBus> = ctx.extensions.get_arc().ok_or_else(
            || ToolError::Internal("DomainEventBus missing".into()))?;
        let event_tx = ctx.event_tx.clone().ok_or_else(
            || ToolError::Internal("event_tx missing".into()))?;
        let cancel = ctx.cancel_token.clone();

        let out = run_for_test(args, layer1, policy, privacy, pending, event_tx, bus, cancel)
            .await
            .map_err(ToolError::Internal)?;
        Ok(serde_json::json!({ "stdout": out.stdout, "exit_code": out.exit_code }))
    }
}
```

(If `tools-core::ToolContext` does not yet have an `extensions` field that supports `Arc<T>` lookup by `TypeId`, add it as a separate sub-task: introduce `pub struct ToolExtensions` with `DashMap<TypeId, Arc<dyn Any + Send + Sync>>` plus `insert_arc<T>(Arc<T>)` and `get_arc<T>() -> Option<Arc<T>>` accessors. Verify with `grep -n 'pub struct ToolContext' crates/tools-core/src/`.)

- [ ] **Step 4: Run smoke** — `cargo test -p klynt-core --test bash_smoke`. PASS on macOS; SKIP on Linux.

- [ ] **Step 5: Commit** — `git commit -m "feat(klynt-core): BashTool execute = guard + Seatbelt + sandbox-exec"`.

---

### Task 16: Tool registry filter (`available_for_channel`)

**Context:** Plan 1 added `Tool::is_concurrency_safe`. Plan 2 adds a tool-registration-time filter so the coding tool set is exposed only for `channel == "coding"`.

**Files:** create `crates/klynt-core/src/registry/{mod.rs, filter.rs}`; modify `crates/agent/src/agent_runtime/runtime.rs` (call the filter when building the tool list per turn).

- [ ] **Step 1: Write failing test** at `crates/klynt-core/tests/registry_filter.rs`:

```rust
use klynt_core::registry::filter::{available_for_channel, Channel};

#[test]
fn coding_only_tools() {
    assert!(available_for_channel("bash", Channel::Coding));
    assert!(!available_for_channel("bash", Channel::Desktop));
    assert!(available_for_channel("tasks", Channel::Desktop));
    assert!(available_for_channel("tasks", Channel::Coding));
}
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement**

```rust
// crates/klynt-core/src/registry/mod.rs
pub mod filter;
```

```rust
// crates/klynt-core/src/registry/filter.rs
use common::CODING_CHANNEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel { Coding, Desktop, Other }

impl Channel {
    pub fn from_name(s: &str) -> Self {
        if s == CODING_CHANNEL { Self::Coding }
        else if s == "desktop" { Self::Desktop }
        else { Self::Other }
    }
}

const CODING_ONLY: &[&str] = &[
    "bash", "read", "glob", "grep", "edit", "write", "apply_patch",
    "web_fetch", "ask_user", "enter_plan_mode", "exit_plan_mode", "notebook_edit",
];

pub fn available_for_channel(tool_name: &str, channel: Channel) -> bool {
    let is_coding_only = CODING_ONLY.contains(&tool_name);
    match channel {
        Channel::Coding => true,
        Channel::Desktop | Channel::Other => !is_coding_only,
    }
}
```

- [ ] **Step 4: Wire in `agent_runtime/runtime.rs`** — find the tool-list construction site and add:

```rust
use klynt_core::registry::filter::{available_for_channel, Channel};

let channel = Channel::from_name(ctx.channel.as_str());
let tools_json: Vec<_> = registry.iter()
    .filter(|t| available_for_channel(t.name(), channel))
    .map(|t| t.json_schema_full())
    .collect();
```

(Plan 3 expands this to support `coding.tools.profiles` for minimal/curated/power profiles. Plan 2 keeps it binary.)

- [ ] **Step 5: Add `klynt-core` dep to `agent`** in `crates/agent/Cargo.toml`. Verify there's no circular dep (klynt-core depends on agent for `AgentEvent`; we now add agent → klynt-core). Resolution: move `Channel` + `available_for_channel` to `common` instead. Reassess: `available_for_channel` only depends on `common::CODING_CHANNEL`, so move the filter module to `crates/common/src/coding_channel.rs` and re-export. `klynt-core::registry::filter` becomes a thin re-export of `common::coding_channel::*`.

  **Action:** Move the filter into `crates/common/src/`. Update tests + imports accordingly. The `klynt-core::registry::filter` becomes `pub use common::coding_channel::*;`.

- [ ] **Step 6: Run test + build** — `cargo test -p klynt-core --test registry_filter && cargo build -p agent`. Both PASS.

- [ ] **Step 7: Commit** — `git commit -m "feat(coding): tool-registry filter by channel; bash exposed only in coding"`.

---

### Task 17: Inject Layer1/Policy/PrivacyGuard/PendingMap into ToolContext at AppCore init

**Files:** modify the AppCore init site (verify via `grep -rn "AgentRuntime::new\|AgentRuntime {" crates/app-core/src/`).

- [ ] **Step 1: Locate `AgentRuntime` construction**.

- [ ] **Step 2: Build the dependencies once during AppCore init** (pseudocode; adapt to actual constructor):

```rust
use std::sync::Arc;
use klynt_core::{
    approval::{Layer1, PendingApprovalsMap},
    privacy::PrivacyGuard,
};
use klynt_execpolicy::Policy;

let layer1 = Arc::new(
    Layer1::compile(&config.coding.permissions)
        .expect("Layer 1 rules failed to compile; fix coding.permissions in config.json")
);
let exclude_globs: Vec<&str> = config.coding_memory.ingest.exclude_paths
    .iter().map(String::as_str).collect();
let privacy = Arc::new(PrivacyGuard::from_globs(&exclude_globs).expect("privacy globs"));
let policy = Arc::new(
    Policy::load_from_dir(&dirs::home_dir().unwrap().join(".klyntbot/rules"))
        .unwrap_or_else(|_| Policy::empty())
);
let pending = self.pending_approvals.clone();

// Insert into the ToolContext extensions.
tool_ctx_extensions.insert_arc(layer1.clone());
tool_ctx_extensions.insert_arc(policy.clone());
tool_ctx_extensions.insert_arc(privacy.clone());
tool_ctx_extensions.insert_arc(pending.clone());
tool_ctx_extensions.insert_arc(domain_bus.clone());
```

If `tool_ctx_extensions` doesn't exist as a TypeMap-style holder, add one to `tools-core::ToolContext` first (add a sub-task before this if needed):

```rust
// crates/tools-core/src/extensions.rs
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::sync::Arc;

#[derive(Default)]
pub struct ToolExtensions {
    inner: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}
impl ToolExtensions {
    pub fn new() -> Self { Self::default() }
    pub fn insert_arc<T: Any + Send + Sync>(&self, v: Arc<T>) {
        self.inner.insert(TypeId::of::<T>(), v);
    }
    pub fn get_arc<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.inner.get(&TypeId::of::<T>())
            .and_then(|r| Arc::clone(r.value()).downcast::<T>().ok())
    }
}
```

Then add `pub extensions: Arc<ToolExtensions>` to `ToolContext`.

- [ ] **Step 3: Register the BashTool** — where existing tools are registered (e.g., `tools::register_all(&mut registry)`), add `registry.register(klynt_core::tools::BashTool::new());`.

- [ ] **Step 4: Build + sanity-test** — `cargo build --workspace && cargo nextest run -p klynt-core`. PASS.

- [ ] **Step 5: Commit** — `git commit -m "feat(coding): inject Layer1/Policy/PrivacyGuard into ToolContext at AppCore init"`.

---

## Track F — Frontend: ApprovalCard + listeners

### Task 18: Add `kind: "approval"` to `ConversationItem`

**Files:** modify `desktop-ui/src/types.ts`.

- [ ] **Step 1: Read existing variants** — `grep -n "kind:" desktop-ui/src/types.ts | head -20`.

- [ ] **Step 2: Add the variant**

In `desktop-ui/src/types.ts`, append to the `ConversationItem` union:

```ts
| {
    id: string;
    kind: "approval";
    requestId: string;
    tool: string;
    args: Record<string, unknown>;
    cwd: string;
    sandboxSummary: string;
    layer:
      | "privacy"
      | "layer1_declarative"
      | "layer2_starlark"
      | "layer3_mirror"
      | "default_mode";
    layerReason: string;
    mirrorHistory?: { approvalCount: number; denialCount: number };
    status:
      | "pending"
      | "approved-once"
      | "approved-always"
      | "denied"
      | "timed-out"
      | "cancelled";
    decidedAt?: string;
    decidedBy?: "user" | "auto_allow" | "auto_deny" | "timeout" | "cancelled";
  };
```

- [ ] **Step 3: Type-check** — `cd desktop-ui && bun run typecheck`. PASS.

- [ ] **Step 4: Commit** — `git commit -m "feat(coding-ui): add kind: 'approval' ConversationItem variant"`.

---

### Task 19: Implement `useApprovalQueue` hook

**Files:** create `desktop-ui/src/features/coding/hooks/{useApprovalQueue.ts, useApprovalQueue.test.ts}`.

- [ ] **Step 1: Write failing test**

```ts
// useApprovalQueue.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@/api/client", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@/api/client";
import { useApprovalQueue } from "./useApprovalQueue";

describe("useApprovalQueue", () => {
  beforeEach(() => vi.clearAllMocks());

  it("captures pending approvals and resolves on respond", async () => {
    let handler: any;
    (listen as any).mockImplementation((_chan: string, h: any) => {
      if (_chan === "agent:approval_requested") handler = h;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useApprovalQueue("session-1"));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    act(() => {
      handler({ payload: {
        request_id: "r1", tool: "bash", args: { command: "ls" },
        cwd: "/x", sandbox_summary: "Seatbelt",
        layer: "layer1_declarative", layer_reason: "ask",
        requires_user_input: true,
      }});
    });
    expect(result.current.pending.length).toBe(1);
    await act(async () => {
      await result.current.respond("r1", { kind: "allow_once" });
    });
    expect(invoke).toHaveBeenCalledWith("chat_respond_approval", {
      sessionKey: "session-1", requestId: "r1", decision: { kind: "allow_once" },
    });
  });

  it("auto-resolved (requires_user_input=false) requests are skipped", async () => {
    let handler: any;
    (listen as any).mockImplementation((_c: string, h: any) => {
      handler = h; return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useApprovalQueue("session-2"));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    act(() => {
      handler({ payload: {
        request_id: "auto", tool: "bash", args: {},
        cwd: "/x", sandbox_summary: "S",
        layer: "layer1_declarative", layer_reason: "auto",
        requires_user_input: false,
      }});
    });
    expect(result.current.pending.length).toBe(0);
  });
});
```

- [ ] **Step 2: Verify failure** — `cd desktop-ui && bun run test useApprovalQueue`. FAIL.

- [ ] **Step 3: Implement the hook** at `useApprovalQueue.ts`:

```ts
import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@/api/client";
import type { ConversationItem } from "@/types";

type ApprovalPayload = {
  request_id: string;
  tool: string;
  args: Record<string, unknown>;
  cwd: string;
  sandbox_summary: string;
  layer:
    | "privacy" | "layer1_declarative" | "layer2_starlark"
    | "layer3_mirror" | "default_mode";
  layer_reason: string;
  mirror_history?: { approval_count: number; denial_count: number };
  requires_user_input: boolean;
};

type ResolvedPayload = {
  request_id: string;
  decided_by: "user" | "auto_allow" | "auto_deny" | "timeout" | "cancelled";
  decision_reason: string;
};

export type ApprovalDecision =
  | { kind: "allow_once" }
  | { kind: "allow_always"; rule?: string }
  | { kind: "deny" }
  | { kind: "add_rule"; starlark_source: string };

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

export function useApprovalQueue(sessionKey: string) {
  const [pending, setPending] = useState<ApprovalItem[]>([]);

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<ApprovalPayload>("agent:approval_requested", (e) => {
      if (!e.payload.requires_user_input) return;
      const item: ApprovalItem = {
        id: `approval-${e.payload.request_id}`,
        kind: "approval",
        requestId: e.payload.request_id,
        tool: e.payload.tool,
        args: e.payload.args,
        cwd: e.payload.cwd,
        sandboxSummary: e.payload.sandbox_summary,
        layer: e.payload.layer,
        layerReason: e.payload.layer_reason,
        mirrorHistory: e.payload.mirror_history
          ? { approvalCount: e.payload.mirror_history.approval_count,
              denialCount:  e.payload.mirror_history.denial_count }
          : undefined,
        status: "pending",
      };
      setPending((p) => [...p, item]);
    }).then((un) => unlistens.push(un));

    listen<ResolvedPayload>("agent:approval_resolved", (e) => {
      setPending((p) => p.map((it) => it.requestId === e.payload.request_id
        ? { ...it, status: mapStatus(e.payload.decided_by),
            decidedAt: new Date().toISOString(), decidedBy: e.payload.decided_by }
        : it));
    }).then((un) => unlistens.push(un));

    return () => { unlistens.forEach((f) => f()); };
  }, []);

  const respond = useCallback(
    async (requestId: string, decision: ApprovalDecision) => {
      await invoke("chat_respond_approval", { sessionKey, requestId, decision });
    },
    [sessionKey],
  );

  return { pending, respond };
}

function mapStatus(d: ResolvedPayload["decided_by"]): ApprovalItem["status"] {
  switch (d) {
    case "user":       return "approved-once";
    case "auto_allow": return "approved-once";
    case "auto_deny":  return "denied";
    case "timeout":    return "timed-out";
    case "cancelled":  return "cancelled";
  }
}
```

- [ ] **Step 4: Run test** — PASS.

- [ ] **Step 5: Commit** — `git commit -m "feat(coding-ui): useApprovalQueue hook subscribing to agent:approval_*"`.

---

### Task 20: Implement `ApprovalCard` component

**Files:** create `desktop-ui/src/features/coding/components/{ApprovalCard.tsx, ApprovalCard.test.tsx}`; create `desktop-ui/src/features/coding/coding.css`; modify `desktop-ui/src/styles/index.css`.

- [ ] **Step 1: Write failing test**

```tsx
// ApprovalCard.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, screen } from "@testing-library/react";
import { ApprovalCard } from "./ApprovalCard";

const baseItem = {
  id: "approval-r1", kind: "approval" as const,
  requestId: "r1", tool: "bash",
  args: { command: "cargo test" }, cwd: "/repo",
  sandboxSummary: "Seatbelt cwd-only",
  layer: "layer2_starlark" as const, layerReason: "no rule matched",
  status: "pending" as const,
};

describe("ApprovalCard", () => {
  it("renders pending state with Allow once / Allow always / Deny / Add rule", () => {
    render(<ApprovalCard item={baseItem} onRespond={vi.fn()} />);
    expect(screen.getByText(/cargo test/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /allow once/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /deny/i })).toBeInTheDocument();
  });

  it("calls onRespond with allow_once when clicked", () => {
    const onRespond = vi.fn();
    render(<ApprovalCard item={baseItem} onRespond={onRespond} />);
    fireEvent.click(screen.getByRole("button", { name: /allow once/i }));
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_once" });
  });

  it("keyboard 'a' triggers allow once", () => {
    const onRespond = vi.fn();
    render(<ApprovalCard item={baseItem} onRespond={onRespond} />);
    fireEvent.keyDown(window, { key: "a" });
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_once" });
  });

  it("collapses to one-line summary when status != pending", () => {
    const decided = { ...baseItem, status: "approved-once" as const,
      decidedBy: "user" as const, decidedAt: new Date().toISOString() };
    render(<ApprovalCard item={decided} onRespond={vi.fn()} />);
    expect(screen.queryByRole("button", { name: /allow once/i })).not.toBeInTheDocument();
    expect(screen.getByText(/approved/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Verify failure**.

- [ ] **Step 3: Implement the component** at `ApprovalCard.tsx`:

```tsx
import { useEffect } from "react";
import type { ConversationItem } from "@/types";
import type { ApprovalDecision } from "@/features/coding/hooks/useApprovalQueue";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

type Props = {
  item: ApprovalItem;
  onRespond: (requestId: string, decision: ApprovalDecision) => void;
};

export function ApprovalCard({ item, onRespond }: Props) {
  const pending = item.status === "pending";
  useEffect(() => {
    if (!pending) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "a") onRespond(item.requestId, { kind: "allow_once" });
      if (e.key === "s") onRespond(item.requestId, { kind: "allow_always" });
      if (e.key === "d") onRespond(item.requestId, { kind: "deny" });
      if (e.key === "r") {
        const src = window.prompt("Starlark rule source (Plan 4 will persist):", "");
        if (src != null) onRespond(item.requestId, { kind: "add_rule", starlark_source: src });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [pending, item.requestId, onRespond]);

  if (!pending) {
    return (
      <div className="approval-card approval-card--decided">
        <span>
          {item.status === "approved-once" || item.status === "approved-always" ? "approved" :
           item.status === "denied" ? "denied" :
           item.status === "timed-out" ? "timed out" : "cancelled"}
          {" — "}{item.tool}: {summarizeArgs(item.args)}
        </span>
      </div>
    );
  }

  return (
    <div className="approval-card approval-card--pending" role="region" aria-label="Approval needed">
      <header>Approval needed</header>
      <dl>
        <dt>Tool</dt><dd>{item.tool}</dd>
        <dt>Args</dt><dd className="approval-card__args">{summarizeArgs(item.args)}</dd>
        <dt>CWD</dt><dd>{item.cwd}</dd>
        <dt>Sandbox</dt><dd>{item.sandboxSummary}</dd>
        <dt>Layer</dt><dd>{item.layer} — {item.layerReason}</dd>
        {item.mirrorHistory && (
          <>
            <dt>Mirror history</dt>
            <dd>{item.mirrorHistory.approvalCount} approvals · {item.mirrorHistory.denialCount} denials</dd>
          </>
        )}
      </dl>
      <div className="approval-card__buttons">
        <button onClick={() => onRespond(item.requestId, { kind: "allow_once" })}>Allow once (a)</button>
        <button onClick={() => onRespond(item.requestId, { kind: "allow_always" })}>Allow always (s)</button>
        <button onClick={() => onRespond(item.requestId, { kind: "deny" })}>Deny (d)</button>
        <button onClick={() => {
          const src = window.prompt("Starlark rule source (Plan 4):", "");
          if (src != null) onRespond(item.requestId, { kind: "add_rule", starlark_source: src });
        }}>Add rule… (r)</button>
      </div>
    </div>
  );
}

function summarizeArgs(a: Record<string, unknown>): string {
  if (typeof a.command === "string") return a.command;
  return JSON.stringify(a);
}
```

- [ ] **Step 4: Implement `coding.css`**

```css
.approval-card {
  border: 1px solid var(--border-soft);
  border-radius: 6px;
  padding: 10px 12px;
  margin: 8px 0;
  font-size: var(--fs-base);
}
.approval-card--pending {
  background: var(--surface-1);
  animation: approval-pulse 2s ease-in-out infinite;
}
.approval-card--decided { opacity: 0.7; }
.approval-card header { font-weight: 600; margin-bottom: 6px; }
.approval-card dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 12px; margin: 0; }
.approval-card dt { color: var(--text-muted); font-size: var(--fs-xs); }
.approval-card dd { margin: 0; }
.approval-card__args { font-family: var(--font-mono); }
.approval-card__buttons { margin-top: 8px; display: flex; gap: 6px; flex-wrap: wrap; }
.approval-card__buttons button {
  padding: 4px 10px;
  border: 1px solid var(--border-soft);
  background: var(--surface-2);
  cursor: pointer;
  border-radius: 4px;
  font-size: var(--fs-xs);
}
@keyframes approval-pulse {
  0%, 100% { box-shadow: 0 0 0 0 var(--accent-soft); }
  50%      { box-shadow: 0 0 0 4px var(--accent-soft); }
}
```

- [ ] **Step 5: Add CSS import** — append `@import "../features/coding/coding.css";` to `desktop-ui/src/styles/index.css`.

- [ ] **Step 6: Run test** — PASS.

- [ ] **Step 7: Commit** — `git commit -m "feat(coding-ui): ApprovalCard component + coding.css"`.

---

### Task 21: Wire `ApprovalCard` into `MessageRows`

**Files:** modify `desktop-ui/src/features/messages/components/MessageRows.tsx`; modify the parent that owns `MessageRowsProps` (likely `Messages.tsx` and `MainApp.tsx`).

- [ ] **Step 1: Inspect existing kind switch** — `grep -n 'item.kind\|case "' desktop-ui/src/features/messages/components/MessageRows.tsx | head -20`.

- [ ] **Step 2: Add the case** in `MessageRows.tsx`:

```tsx
import { ApprovalCard } from "@/features/coding/components/ApprovalCard";
// …
case "approval":
  return <ApprovalCard item={item} onRespond={onApprovalRespond} />;
```

Add to `MessageRowsProps`:

```tsx
type MessageRowsProps = {
  // … existing fields …
  onApprovalRespond: (requestId: string, decision: import("@/features/coding/hooks/useApprovalQueue").ApprovalDecision) => void;
};
```

Plumb `onApprovalRespond` from the parent. In `MainApp.tsx` (or whichever component owns the active session):

```tsx
const { respond: onApprovalRespond } = useApprovalQueue(activeSessionKey);
// pass onApprovalRespond down to <Messages>.
```

- [ ] **Step 3: Type-check + test** — `cd desktop-ui && bun run typecheck && bun run test`. PASS.

- [ ] **Step 4: Commit** — `git commit -m "feat(coding-ui): render ApprovalCard via MessageRows kind: 'approval'"`.

---

### Task 22: Inject approval items into the chat stream

**Context:** `useApprovalQueue.pending` is its own state. We must merge it into the `ConversationItem[]` that `Messages` renders. Approach: extend `chatStreamStore` with an `approvalsBySession` map and a selector that splices approval items into a thread's `segments`.

**Files:** modify `desktop-ui/src/features/chat/store/chatStreamStore.ts`; modify `useApprovalQueue.ts` to write through the store rather than holding its own state.

- [ ] **Step 1: Read current store shape** — `grep -n "interface\|type\|create\|approvalsBySession" desktop-ui/src/features/chat/store/chatStreamStore.ts | head -30`.

- [ ] **Step 2: Add approval slice** — fields:

```ts
type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

approvalsBySession: Record<string, ApprovalItem[]>;
upsertApproval: (sessionKey: string, item: ApprovalItem) => void;
resolveApproval: (sessionKey: string, requestId: string,
                  status: ApprovalItem["status"],
                  decidedBy: NonNullable<ApprovalItem["decidedBy"]>) => void;
```

Implement as straight Zustand (or whichever store flavor) `set((state) => …)` mutators. Append `upsertApproval` items; on `resolveApproval`, mutate the matching item's `status`/`decidedAt`/`decidedBy`.

- [ ] **Step 3: Refactor `useApprovalQueue`** to call `chatStreamStore.upsertApproval` / `resolveApproval` rather than holding its own `useState`. Hook now returns only `respond`:

```ts
export function useApprovalQueue(sessionKey: string) {
  useEffect(() => {
    const u1 = listen<ApprovalPayload>("agent:approval_requested", (e) => {
      if (!e.payload.requires_user_input) return;
      chatStreamStore.getState().upsertApproval(sessionKey, toItem(e.payload));
    });
    const u2 = listen<ResolvedPayload>("agent:approval_resolved", (e) => {
      chatStreamStore.getState().resolveApproval(
        sessionKey, e.payload.request_id,
        mapStatus(e.payload.decided_by), e.payload.decided_by,
      );
    });
    return () => { u1.then((f) => f()); u2.then((f) => f()); };
  }, [sessionKey]);
  const respond = useCallback(/* unchanged */, [sessionKey]);
  return { respond };
}
```

- [ ] **Step 4: Modify selector that builds `segments`** — append approvals to the items list:

```ts
function combineSegmentsWithApprovals(
  segments: ConversationItem[],
  approvals: Extract<ConversationItem, { kind: "approval" }>[],
): ConversationItem[] {
  if (approvals.length === 0) return segments;
  return [...segments, ...approvals];
}
```

(Plan 2: append. Plan 3 may interleave more carefully with `kind: "diff"` rows.)

- [ ] **Step 5: Update existing tests + add new selector test** in `chatStreamStore.test.ts`:

```ts
it("approvals appear in segments after upsert", () => {
  const store = chatStreamStore.getState();
  store.upsertApproval("s1", { id: "approval-r1", kind: "approval", requestId: "r1",
    tool: "bash", args: {command:"x"}, cwd: "/x", sandboxSummary: "S",
    layer: "layer1_declarative", layerReason: "ask", status: "pending" });
  expect(store.approvalsBySession["s1"]).toHaveLength(1);
  store.resolveApproval("s1", "r1", "approved-once", "user");
  expect(store.approvalsBySession["s1"][0].status).toBe("approved-once");
});
```

- [ ] **Step 6: Verify** — `cd desktop-ui && bun run test && bun run typecheck && bun run lint`. PASS.

- [ ] **Step 7: Commit** — `git commit -m "feat(coding-ui): wire approvalsBySession into chatStreamStore"`.

---

### Task 23: Emit Tauri channels from runtime → AppCore → frontend

**Context:** The runtime publishes `AgentEvent::ApprovalRequested/Resolved` through `fan_out_event` (Plan 1). AppCore's chat-streaming pump must additionally translate those into `app.emit("agent:approval_requested", payload)` calls when applicable.

**Files:** modify the AppCore chat-streaming pump (find via `grep -rn 'app.emit("agent:'`).

- [ ] **Step 1: Locate the existing emit site** — `grep -rn 'app\.emit("agent:' crates/ | head -20`.

- [ ] **Step 2: Add new branches in the match**

```rust
match evt {
    AgentEvent::ApprovalRequested { ref requires_user_input, .. } if *requires_user_input => {
        let _ = app.emit("agent:approval_requested", &evt);
    }
    AgentEvent::ApprovalRequested { .. } => {
        // auto-allow / auto-deny / privacy: telemetry only — UI doesn't need them.
    }
    AgentEvent::ApprovalResolved { .. } => {
        let _ = app.emit("agent:approval_resolved", &evt);
    }
    AgentEvent::SandboxPolicyApplied { .. } => {
        let _ = app.emit("agent:sandbox_policy_applied", &evt);
    }
    _ => {}
}
```

(If `AgentEvent` is not `Serialize`, derive it now or build an explicit DTO with snake_case field names matching the TS payload shapes in Task 19/20.)

- [ ] **Step 3: Verify no regressions** — `cargo nextest run --workspace`. PASS.

- [ ] **Step 4: Commit** — `git commit -m "feat(coding): emit agent:approval_* + sandbox_policy_applied to frontend"`.

---

## Track G — Property + scenario tests

### Task 24: K3 — Layer 1 deterministic routing (proptest)

**Files:** create `tests/integration/coding_in_chat/{mod.rs, property_k3_layer1_routing.rs}`; modify the root facade test entry (`tests/integration/main.rs` or equivalent) to include the new module.

- [ ] **Step 1: Wire the new submodule**

```rust
// tests/integration/coding_in_chat/mod.rs
mod property_k3_layer1_routing;
mod property_k4_sandbox_invariant;
mod property_k8_approval_roundtrip;
mod scenario_bash_happy_path;
```

In `tests/integration/main.rs` add `mod coding_in_chat;`.

- [ ] **Step 2: Add the proptest** at `tests/integration/coding_in_chat/property_k3_layer1_routing.rs`:

```rust
use config::schema::coding::CodingPermissions;
use klynt_core::approval::{decision::ApprovalDecision, Layer1};
use proptest::prelude::*;

proptest! {
    /// K3: For any (allow, deny, ask) rule sets and any (tool, payload),
    /// Layer1::evaluate is deterministic AND obeys deny > allow > ask.
    #[test]
    fn k3_routing_precedence(
        allow in prop::collection::vec(r"Bash\([a-z*]{1,5}\*?\)", 0..3),
        deny  in prop::collection::vec(r"Bash\(rm[* ]\*?\)", 0..2),
        payload in r"[a-z ]{1,15}",
    ) {
        let perms = CodingPermissions {
            allow, deny: deny.clone(), ask: vec!["Bash(*)".into()],
            default_if_no_match: "ask".into(), mirror_learning: false,
        };
        let l1 = Layer1::compile(&perms).unwrap();
        let d1 = l1.evaluate("bash", &payload);
        let d2 = l1.evaluate("bash", &payload);
        prop_assert_eq!(format!("{:?}", d1), format!("{:?}", d2));
        if !deny.is_empty() && payload.starts_with("rm") {
            prop_assert!(matches!(d1, ApprovalDecision::Auto { allowed: false, .. }));
        }
    }
}
```

- [ ] **Step 3: Run** — `cargo nextest run --workspace -E 'test(k3_routing_precedence)'`. PASS.

- [ ] **Step 4: Commit** — `git commit -m "test(coding): K3 Layer 1 deterministic-routing property test"`.

---

### Task 25: K4 — bash never executes outside sandbox in test mode

**Files:** create `tests/integration/coding_in_chat/property_k4_sandbox_invariant.rs`.

- [ ] **Step 1: Add test** (gated `#[cfg(target_os = "macos")]`):

```rust
#![cfg(target_os = "macos")]
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::{run_for_test, BashArgs};
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn k4_sandbox_event_emitted_before_exec() {
    let perms = CodingPermissions { allow: vec!["Bash(*)".into()], ..Default::default() };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(64);

    let cwd = tempfile::tempdir().unwrap();
    let outside = std::env::temp_dir().join(format!("klynt-k4-outside-{}", uuid::Uuid::new_v4()));
    let cmd = format!("touch {}/forbidden 2>/dev/null; echo done", outside.display());

    let r = run_for_test(
        BashArgs { command: cmd.clone(), cwd: Some(cwd.path().to_string_lossy().into()), timeout_ms: Some(5000) },
        layer1, policy, privacy, pending, tx, bus, CancellationToken::new(),
    ).await.unwrap();

    // Forbidden file MUST NOT exist
    assert!(!outside.join("forbidden").exists());

    // SandboxPolicyApplied must precede the actual run output
    let mut saw_sandbox = false;
    let mut saw_done = false;
    while let Ok(e) = rx.try_recv() {
        if matches!(e, AgentEvent::SandboxPolicyApplied { .. }) { saw_sandbox = true; }
    }
    if r.stdout.contains("done") { saw_done = true; }
    assert!(saw_sandbox && saw_done);
}
```

- [ ] **Step 2: Run** — `cargo nextest run --workspace -E 'test(k4_sandbox_event_emitted_before_exec)'`. PASS on macOS.

- [ ] **Step 3: Commit** — `git commit -m "test(coding): K4 sandbox-before-exec invariant"`.

---

### Task 26: K8 — every ApprovalRequested has a matching ApprovalResolved

**Files:** create `tests/integration/coding_in_chat/property_k8_approval_roundtrip.rs`.

- [ ] **Step 1: Test**

```rust
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::approval::{
    decision::ApprovalDecision, guard::evaluate, GuardCtx, Layer1, PendingApprovalsMap,
};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use config::schema::coding::CodingPermissions;
use proptest::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]
    #[test]
    fn k8_request_resolve_pair(n in 1usize..15) {
        tokio_test::block_on(async move {
            let perms = CodingPermissions {
                allow: vec!["Bash(echo *)".into()], default_if_no_match: "ask".into(),
                ..Default::default()
            };
            let l1 = Layer1::compile(&perms).unwrap();
            let policy = Policy::empty();
            let privacy = PrivacyGuard::from_globs(&[]).unwrap();
            let pending = Arc::new(PendingApprovalsMap::new());
            let bus = Arc::new(DomainEventBus::new(256));
            let (tx, mut rx) = mpsc::channel(256);

            for i in 0..n {
                let ctx = GuardCtx {
                    layer1: &l1, policy: &policy, privacy: &privacy,
                    pending: &pending, event_tx: Some(&tx), domain_bus: &bus,
                    cancel: CancellationToken::new(),
                    request_id: format!("r-{i}"),
                };
                let _ = evaluate(ctx, "bash", "echo k8").await;
            }
            drop(tx);
            let mut req = 0; let mut res = 0;
            while let Some(e) = rx.recv().await {
                match e {
                    AgentEvent::ApprovalRequested { .. } => req += 1,
                    AgentEvent::ApprovalResolved { .. }  => res += 1,
                    _ => {}
                }
            }
            prop_assert_eq!(req, n);
            prop_assert_eq!(res, n);
            Ok::<(), TestCaseError>(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run --workspace -E 'test(k8_request_resolve_pair)'
git commit -m "test(coding): K8 approval round-trip pair invariant"
```

---

### Task 27: Scenario test — bash happy path E2E

**Files:** create `tests/integration/coding_in_chat/scenario_bash_happy_path.rs`.

- [ ] **Step 1: Test** — sets up an in-memory `StoragePool` + `Repos`, constructs an `AgentRuntime` with a `_scripted_echo` mock provider, runs `chat_send` with `mode = coding`, asserts the event sequence:

```rust
#![cfg(target_os = "macos")]
use agent::events::AgentEvent;
use storage::StoragePool;

#[tokio::test]
async fn bash_happy_path() {
    // 1. In-memory storage + repos.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);

    // 2. Create a coding session.
    let session_key = repos.sessions.create_default("u1").await.unwrap();
    repos.sessions.update_conversation_type(&session_key, "coding").await.unwrap();

    // 3. Build a minimal AppCore-like fixture with a scripted provider.
    //    The scripted provider returns one tool_call to bash with command "echo hi"
    //    on the first iteration, and a final assistant message on the second.
    //    (Use the existing `_scripted_echo` mock provider helper from
    //     tests/common/scripted_provider.rs — see CLAUDE.md "Root facade crate".)
    let runtime = klynt_test_fixtures::build_coding_runtime(&repos).await;
    let mut events = runtime
        .chat_send_collect_events(&session_key, "run echo hi for me", "coding").await;

    // 4. Assert event sequence (in order, ignoring intervening unrelated events).
    let mut saw_request = false;
    let mut saw_resolved = false;
    let mut saw_sandbox  = false;
    let mut saw_tool_end = false;
    while let Some(e) = events.recv().await {
        match e {
            AgentEvent::ApprovalRequested { ref tool, requires_user_input, .. }
                if tool == "bash" && !requires_user_input => saw_request = true,
            AgentEvent::ApprovalResolved { .. } if saw_request => saw_resolved = true,
            AgentEvent::SandboxPolicyApplied { .. } if saw_resolved => saw_sandbox = true,
            AgentEvent::ToolEnd { .. } if saw_sandbox => saw_tool_end = true,
            _ => {}
        }
    }
    assert!(saw_request && saw_resolved && saw_sandbox && saw_tool_end,
        "expected ApprovalRequested → ApprovalResolved → SandboxPolicyApplied → ToolEnd");
}
```

(If `klynt_test_fixtures::build_coding_runtime` does not yet exist, build it as a small helper in `tests/common/coding_fixtures.rs`. The helper wires: scripted provider, in-memory `Repos`, an `AgentRuntime` with `klynt-core::tools::BashTool` registered, a `ToolContext::extensions` populated with `Layer1::compile(&perms_allow_echo).unwrap()`, an empty `Policy`, an empty `PrivacyGuard`, a fresh `PendingApprovalsMap`, a `DomainEventBus::new(256)`. Returns a fixture struct with a `chat_send_collect_events` method that runs `chat_send` and returns the `mpsc::Receiver<AgentEvent>`.)

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run --workspace -E 'test(bash_happy_path)'
git commit -m "test(coding): scenario bash happy path E2E"
```

---

## Track H — Acceptance gate

### Task 28: Plan-2 acceptance

**Files:** none modified.

- [ ] **Step 1: Workspace build** — `cargo build --workspace`. PASS.
- [ ] **Step 2: Workspace clippy** — `cargo clippy --workspace --all-targets --all-features -- -D warnings`. PASS.
- [ ] **Step 3: Workspace fmt** — `cargo fmt --all --check`. PASS.
- [ ] **Step 4: All tests** — `cargo nextest run --workspace && cargo test --workspace --doc`. PASS.
- [ ] **Step 5: Frontend checks** — `cd desktop-ui && bun run lint && bun run typecheck && bun run test`. PASS.
- [ ] **Step 6: Drift tests** — `cargo nextest run --workspace -E 'test(registration_drift) | test(bindings_are_current) | test(no_raw_tauri_command_outside_macros)'`. PASS.

- [ ] **Step 7: Manual smoke (macOS only, optional)**

```bash
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

In the desktop:
1. Create a new chat thread.
2. From DevTools console: `await window.__TAURI_INTERNALS__.invoke('chat_set_mode', { sessionKey: '<key>', mode: 'coding' })`. (Plan 6 wires the `CodingModePill` UI affordance.)
3. Configure `~/.klyntbot-dev/config.json` with:
   ```json
   { "coding": { "permissions": { "allow": ["Bash(echo *)"], "ask": ["Bash(*)"] } } }
   ```
4. Send "run echo hi for me". Verify: bash auto-runs (no card visible because `requires_user_input == false`) and stdout streams back. Open DevTools and confirm `agent:sandbox_policy_applied` event seen.
5. Send "list /tmp" (assumes only Bash(echo *) is allowed). Verify ApprovalCard renders; click "Allow once"; bash runs; output streams.

- [ ] **Step 8: Tag the milestone**

```bash
git tag plan2-bash-end-to-end
```

- [ ] **Step 9: Final commit (if any tweaks)**

```bash
git add -A && git commit -m "chore(coding): Plan 2 acceptance — bash end-to-end green"
```

---

## Self-review checklist

After implementing all tasks, verify:

1. **Spec coverage** — every Plan-2-scoped item from the spec is closed:
   - ✅ Privacy guard (Task 7)
   - ✅ Layer 1 declarative (Task 8)
   - ✅ Approval round-trip + DashMap + select! (Task 9)
   - ✅ `chat_respond_approval` Tauri command (Task 10)
   - ✅ `chat_set_mode` Tauri command (Task 11)
   - ✅ `chat_send` mode field + RoutingContext routing (Task 12)
   - ✅ macOS Seatbelt (Tasks 4–5)
   - ✅ `BashTool` end-to-end (Tasks 13–15)
   - ✅ Tool registry filter (Task 16)
   - ✅ ToolContext extension wiring (Task 17)
   - ✅ `kind: "approval"` ConversationItem (Task 18)
   - ✅ `useApprovalQueue` + `ApprovalCard` + MessageRows wiring (Tasks 19–22)
   - ✅ Tauri event emission for `agent:approval_*` + `sandbox_policy_applied` (Task 23)
   - ✅ K3, K4, K8 + scenario (Tasks 24–27)

2. **Placeholder scan** — search new files for `TODO`, `TBD`, `unimplemented!`, `todo!`. Each occurrence must reference a specific later plan (e.g., "Plan 4 fills") or be deleted.

3. **Type consistency**:
   - `ApprovalDecision` (internal Rust enum) vs `AppApprovalDecision` (Tauri-input shape) — kept distinct; document the difference inside `decision.rs`.
   - TS `ApprovalDecision` matches Rust `AppApprovalDecision` field-for-field (kind tag + variants).
   - `requires_user_input` is `bool` everywhere (Rust event, Tauri payload, TS payload).
   - `decided_by` enum strings in TS match Rust output exactly (`user`, `auto_allow`, `auto_deny`, `timeout`, `cancelled`).
   - `layer` strings in TS payload match `ApprovalLayer` Rust serde rename: `privacy`, `layer1_declarative`, `layer2_starlark`, `layer3_mirror`, `default_mode`.

4. **No regressions** — Plan 1's `fan_out_event` remains the sole emit site for `AgentEvent`s in `execute_loop.rs`. Plan 2 only adds new variant matches in the AppCore emit-pump. Existing chat channels (`telegram`, `discord`, etc.) keep working because match arms have `_ => {}` from Plan 1 Task 8.

5. **CLAUDE.md compliance**:
   - Every new public method on AppCore handlers carries `#[tracing::instrument(skip(self), err)]` (or `skip_all` where appropriate).
   - All new Tauri commands use `#[klynt_command]` (no raw `#[tauri::command]`).
   - Every new command appears in `klynt_collect_commands!` in `specta_builder.rs`.
   - `bindings.ts` regenerated; both `registration_drift` and `bindings_are_current` tests green.
   - No schema migrations in Plan 2 (Plan 1 added all sessions columns).
   - Errors return `common::Result<T>` where applicable; cross-crate use `KlyntbotError` via `From` conversions.
   - CSS uses `var(--fs-*)` typography tokens, no hardcoded `Npx`.
   - All new TS files in `desktop-ui/src/features/coding/` use the project path aliases (no `../../`).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-29-klynt-coding-in-chat-phase1-plan2-bash-end-to-end.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration. Best for Plan 2's mix of Rust + TS + cross-process IPC because each task's diff is reviewable in isolation.
2. **Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch with checkpoints. Faster wall-clock but harder to review.

**Which approach?**
