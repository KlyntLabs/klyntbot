# Coding Approval Handler + Diff-Preview Modal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Klynt's coding-mode approval system end-to-end (the broken stub) and add per-tool-type specialized preview rendering with Mirror-driven smart "Allow always" suggestions.

**Architecture:** Two independently-shippable waves. Wave 1: replace the 25-line `respond_approval` stub with a real handler and wire `DesktopApprovalChannel` (with a pending-request map) into `AppCore`. Wave 2: extend `ApprovalRequest` with typed `preview` and `suggested_grant` fields, build 5 per-tool preview builders, add `ApprovalPatternLearner` as Mirror's 7th signal source, and ship per-kind frontend components with a smart split button.

**Tech Stack:** Rust 1.93 (workspace crates), Tauri 2, sqlx, similar (diff library), DashMap, tokio::sync::oneshot, React 19 + TypeScript, Vitest + @testing-library/react.

**Spec:** `docs/superpowers/specs/2026-05-07-coding-approval-handler-and-diff-preview-design.md`

---

## File Structure

### New Rust files
- `crates/approval/src/preview.rs` — types + 5 builders + classifier
- `crates/app-core/src/desktop_approval_channel.rs` — channel impl with pending-request map
- `crates/cognitive/src/mirror/sources/approval_patterns.rs` — 7th signal source
- `crates/cognitive/migrations/00X_approval_pattern_history.sql` — schema

### Modified Rust files
- `crates/approval/src/lib.rs` — re-exports
- `crates/approval/src/request.rs` — `cwd` on `ApprovalContext`; `preview` + `suggested_grant` on `ApprovalRequest`
- `crates/approval/src/gate.rs` — Mirror integration before `channel.request`
- `crates/app-core/src/coding/approval_handler.rs` — replace 25-line stub
- `crates/app-core/src/lib.rs` (or `init/mod.rs`) — wire `DesktopApprovalChannel` + Mirror into gate
- `crates/app-core/src/coding/mod.rs` — declare new modules if needed
- `crates/bus/src/domain_events.rs` — `ApprovalResolved` variant
- `crates/cognitive/src/mirror/sources/mod.rs` — declare new module
- `crates/cognitive/src/mirror/engine.rs` — register 7th source
- `crates/cognitive/src/mirror/facade.rs` — expose `approval_patterns()` accessor
- `crates/desktop/src/commands/coding/approval.rs` (or wherever Tauri command lives) — verify

### New TypeScript files
- `desktop-ui/src/features/coding/components/preview/PreviewRenderer.tsx`
- `desktop-ui/src/features/coding/components/preview/DiffPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/CommandPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/UrlPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/McpPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/GenericPreview.tsx`
- `desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.tsx`
- `desktop-ui/src/features/coding/components/PatternPicker.tsx`
- `desktop-ui/src/styles/approval-preview.css`
- All `.test.tsx` siblings

### Modified TypeScript files
- `desktop-ui/src/types/bindings.ts` — auto-regenerated via specta
- `desktop-ui/src/features/coding/components/ApprovalCard.tsx` — ~6-line diff
- `desktop-ui/src/features/coding/hooks/useApprovalQueue.ts` — extend `toItem`
- `desktop-ui/src/styles/index.css` — `@import "./approval-preview.css"`

---

# Phase 1 — Wave 1: Wire the approval system end-to-end

## Task 1: Diagnose what's actually broken

**Files:**
- Read-only investigation

- [ ] **Step 1: Verify what's wired today**

```bash
# Find the Tauri command shell
grep -rn "chat_respond_approval\|approval_respond" \
  /Users/jayden/Projects/Klynt/bot/crates/desktop/src/commands/ \
  /Users/jayden/Projects/Klynt/bot/crates/app-core/src/

# Find which channel is wired into ApprovalGate at AppCore init
grep -rn "ApprovalGate::new\|ApprovalGate {" \
  /Users/jayden/Projects/Klynt/bot/crates/app-core/src/ \
  /Users/jayden/Projects/Klynt/bot/crates/approval/src/

# Find current channel impls
grep -rn "impl ApprovalChannel for" /Users/jayden/Projects/Klynt/bot/crates/
```

Expected: identifies (a) the existing Tauri command shell file, (b) where `ApprovalGate` is constructed, (c) whether any channel impl exists today besides `BlockingFallbackChannel`.

- [ ] **Step 2: Document findings inline as comments**

Add a comment to `crates/app-core/src/coding/approval_handler.rs` at line 1:
```rust
//! WAVE 1 NOTE (2026-05-07): The Tauri command shell is at <PATH FOUND IN STEP 1>.
//! ApprovalGate is constructed at <PATH FOUND IN STEP 1>.
//! Today's channel is <CHANNEL FOUND IN STEP 1>.
```

This is a temporary marker — remove in Step 6 of Task 7 when wiring is complete.

- [ ] **Step 3: Verify the gate is actually called from coding-mode tool execution**

Read `crates/agent/src/execution/core.rs` lines 750–800 (per CLAUDE.md, the preflight is around line 756–792). Confirm `approval_gate.check(req).await` is invoked. If it's only invoked for assistant mode (gated by channel name), document this as a Wave 1 sub-fix.

- [ ] **Step 4: Commit the investigation note**

```bash
git add crates/app-core/src/coding/approval_handler.rs
git commit -m "chore(approval): note investigation findings before wave 1 fix"
```

---

## Task 2: Add `cwd` to `ApprovalContext`

**Files:**
- Modify: `crates/approval/src/request.rs`
- Modify: all `ApprovalContext` constructor sites (TBD per Step 1)

- [ ] **Step 1: Find all `ApprovalContext` constructor sites**

```bash
grep -rn "ApprovalContext {" /Users/jayden/Projects/Klynt/bot/crates/
grep -rn "ApprovalContext::new\|ApprovalContext::default" /Users/jayden/Projects/Klynt/bot/crates/
```

Expected: ~5–10 sites in `crates/agent/`, `crates/approval/`, `crates/app-core/`, `crates/channels/`, plus tests.

- [ ] **Step 2: Add the field**

Edit `crates/approval/src/request.rs` — extend the struct:

```rust
#[derive(Debug, Clone)]
pub struct ApprovalContext {
    pub mode: common::SessionMode,
    pub channel: ChannelKind,
    pub session_id: String,
    pub user_id: Option<String>,
    pub cwd: std::path::PathBuf,
}
```

- [ ] **Step 3: Update each call site**

For each constructor found in Step 1, add `cwd: <appropriate path>`. For agent runtime sites: thread the cwd through from the session/repo context. For tests: `cwd: PathBuf::from(".")` is sufficient.

- [ ] **Step 4: Build to verify**

```bash
cargo build --workspace 2>&1 | tail -30
```

Expected: clean build. If errors are call sites missed in Step 3, add `cwd` there.

- [ ] **Step 5: Run tests**

```bash
cargo nextest run -p approval
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/
git commit -m "feat(approval): add cwd to ApprovalContext"
```

---

## Task 3: Add `preview` and `suggested_grant` fields to `ApprovalRequest`

**Files:**
- Modify: `crates/approval/src/request.rs`

- [ ] **Step 1: Add the fields with placeholder types**

Edit `crates/approval/src/request.rs`. After the existing `ApprovalRequest` struct, leave a `TODO` for now:

```rust
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub action: Option<String>,
    pub args: Value,
    pub class: ApprovalClass,
    pub scope: ApprovalScope,
    pub ctx: ApprovalContext,
    /// Preview metadata for the approval card. Populated by the channel boundary
    /// at request emission time. None for internal flows that don't need UI.
    pub preview: Option<crate::preview::ApprovalPreview>,
    /// Mirror-suggested grant pattern. None if Mirror has no signal yet
    /// or no Mirror facade is wired.
    pub suggested_grant: Option<crate::preview::SuggestedGrant>,
}
```

The `crate::preview` module doesn't exist yet — this will fail compile until Task 4. That's fine; Tasks 3 + 4 commit together.

- [ ] **Step 2: Defer build until Task 4**

The struct change references types that are added in Task 4. We'll commit these together at end of Task 4.

---

## Task 4: Add `ApprovalPreview`, `SuggestedGrant`, `GrantScope` types

**Files:**
- Create: `crates/approval/src/preview.rs`
- Modify: `crates/approval/src/lib.rs`

- [ ] **Step 1: Create the preview module skeleton**

Write `crates/approval/src/preview.rs`:

```rust
//! Per-tool preview metadata + grant suggestion types attached to ApprovalRequest.
//! Builders are added in subsequent tasks; this file defines the data model.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Per-tool-kind preview metadata. Frontend renders one of five components
/// based on the discriminant.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalPreview {
    Diff {
        path: PathBuf,
        unified_diff: String,
        lines_added: u32,
        lines_removed: u32,
        is_new_file: bool,
        is_truncated: bool,
    },
    Command {
        command: String,
        cwd: PathBuf,
        is_dangerous: bool,
        risk_hits: Vec<String>,
    },
    Url {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body_preview: Option<String>,
    },
    Mcp {
        server: String,
        tool: String,
        args: serde_json::Value,
        schema: Option<serde_json::Value>,
    },
    Generic {
        args: serde_json::Value,
    },
}

/// Mirror-driven suggestion for the smart "Allow always" button.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SuggestedGrant {
    /// Human-readable form: e.g., "Edit on src/components/**".
    pub pattern: String,
    /// Machine-readable: structured scope used to build the GrantRow.
    pub scope: GrantScope,
    /// Why Mirror suggested this; shown in the button tooltip.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GrantScope {
    ExactToolPath { tool: String, path: PathBuf },
    ToolFolder { tool: String, folder: PathBuf },
    ToolGlob { tool: String, glob: String },
    Custom { starlark_source: String },
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Edit `crates/approval/src/lib.rs` — add `pub mod preview;` and re-exports:

```rust
pub mod channel;
pub mod class;
pub mod coding_policy;
pub mod gate;
pub mod grants;
pub mod policy;
pub mod preview;       // NEW
pub mod request;

pub use channel::{ApprovalCapabilities, ApprovalChannel, BlockingFallbackChannel};
pub use class::{ApprovalClass, ApprovalDecision, ApprovalLifetime, ApprovalScope};
pub use coding_policy::CodingApprovalPolicy;
pub use gate::{ApprovalGate, GateOutcome};
pub use grants::{ApprovalGrantsRepo, GrantRow};
pub use policy::ClassifyHook;
pub use preview::{ApprovalPreview, GrantScope, SuggestedGrant};   // NEW
pub use request::{ApprovalContext, ApprovalRequest, ChannelKind};
```

- [ ] **Step 3: Build to verify**

```bash
cargo build -p approval
```

Expected: clean build. Both Tasks 3 and 4 compile together.

- [ ] **Step 4: Commit**

```bash
git add crates/approval/src/preview.rs \
        crates/approval/src/lib.rs \
        crates/approval/src/request.rs
git commit -m "feat(approval): add ApprovalPreview + SuggestedGrant types and wire into ApprovalRequest"
```

---

## Task 5: Build `DesktopApprovalChannel` skeleton

**Files:**
- Create: `crates/app-core/src/desktop_approval_channel.rs`
- Modify: `crates/app-core/src/lib.rs`

- [ ] **Step 1: Create the file with the struct + `ApprovalChannel` stub impl**

Write `crates/app-core/src/desktop_approval_channel.rs`:

```rust
//! Desktop-channel impl of ApprovalChannel.
//!
//! Owns a pending-request map keyed by request_id and a oneshot::Sender per
//! pending request. The gate's `channel.request().await` future blocks on the
//! oneshot until the user clicks Approve/Deny in the UI, at which point
//! `respond_approval` calls `resolve(...)` to wake the future.

use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest,
    GrantRow, SuggestedGrant,
};
use dashmap::DashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use uuid::Uuid;

const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("no pending approval found for request_id {0}")]
    NotFound(String),
    #[error("oneshot send failed (recipient dropped)")]
    SendFailed,
}

#[allow(dead_code)]
struct PendingEntry {
    sender: oneshot::Sender<ApprovalDecision>,
    tool_name: String,
    args: serde_json::Value,
    cwd: PathBuf,
    suggested: Option<SuggestedGrant>,
}

pub struct DesktopApprovalChannel {
    pending: Arc<DashMap<String, PendingEntry>>,
    emitter: Arc<dyn crate::events::AppEventEmitter>,
}

impl DesktopApprovalChannel {
    pub fn new(emitter: Arc<dyn crate::events::AppEventEmitter>) -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            emitter,
        }
    }

    /// Wake the awaiting `request()` future for `request_id` with `decision`.
    /// Returns NotFound if no pending entry exists (timed out, cancelled, or
    /// already resolved).
    pub fn resolve(&self, request_id: &str, decision: ApprovalDecision) -> Result<(), ResolveError> {
        let (_id, entry) = self
            .pending
            .remove(request_id)
            .ok_or_else(|| ResolveError::NotFound(request_id.to_string()))?;
        entry.sender.send(decision).map_err(|_| ResolveError::SendFailed)
    }

    /// Build a GrantRow for persistence on Always-class decisions.
    /// Returns None if request_id has already been resolved/removed.
    pub fn build_grant_row(
        &self,
        request_id: &str,
        rule: Option<&str>,
    ) -> Option<GrantRow> {
        let entry = self.pending.get(request_id)?;
        Some(GrantRow {
            // Populate per the existing GrantRow shape — adapt this in Task 6
            // after reading crates/approval/src/grants.rs:GrantRow.
            ..Default::default()
        })
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, _req: ApprovalRequest) -> ApprovalDecision {
        // Implemented in Task 6.
        ApprovalDecision::Decline {
            reason: "stub — Task 6 not yet executed".into(),
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}
```

- [ ] **Step 2: Wire into `app-core` lib**

Edit `crates/app-core/src/lib.rs` (or wherever the module declarations live) — add:

```rust
pub mod desktop_approval_channel;
```

- [ ] **Step 3: Confirm `crate::events::AppEventEmitter` exists**

```bash
grep -rn "trait AppEventEmitter\|pub.*AppEventEmitter" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/events*.rs
```

If not, the import path adapts to the actual emitter trait location. Current crates have an existing emitter pattern (see `coding/turn_handler.rs` for usage).

- [ ] **Step 4: Add `dashmap` and `uuid` dependencies if missing**

```bash
grep -n "dashmap\|uuid" /Users/jayden/Projects/Klynt/bot/crates/app-core/Cargo.toml
```

If missing, add to `[dependencies]`:
```toml
dashmap = "6"
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 5: Build**

```bash
cargo build -p app-core
```

Expected: clean build (with the stub `request` impl).

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/desktop_approval_channel.rs \
        crates/app-core/src/lib.rs \
        crates/app-core/Cargo.toml
git commit -m "feat(app-core): scaffold DesktopApprovalChannel with pending-request map"
```

---

## Task 6: Implement `DesktopApprovalChannel::request`

**Files:**
- Modify: `crates/app-core/src/desktop_approval_channel.rs`
- Modify: `crates/app-core/src/events.rs` (if `emit_approval_requested` doesn't exist)

- [ ] **Step 1: Read existing AppEventEmitter API**

```bash
cat /Users/jayden/Projects/Klynt/bot/crates/app-core/src/events.rs 2>/dev/null | head -100
```

Find the existing `emit_*` methods. Add a new method signature:

```rust
fn emit_approval_requested(
    &self,
    request_id: &str,
    req: &approval::ApprovalRequest,
) -> common::Result<()>;
```

Implement it in the existing emitter struct(s). The Tauri event name is `agent:approval_requested`. Payload shape:

```rust
#[derive(serde::Serialize, specta::Type)]
struct ApprovalRequestedPayload<'a> {
    request_id: &'a str,
    tool: &'a str,
    args: &'a serde_json::Value,
    cwd: &'a std::path::Path,
    sandbox_summary: String,
    layer: &'a str,
    layer_reason: &'a str,
    requires_user_input: bool,
    preview: Option<&'a approval::ApprovalPreview>,
    suggested_grant: Option<&'a approval::SuggestedGrant>,
}
```

- [ ] **Step 2: Replace stub `request` with real impl**

In `crates/app-core/src/desktop_approval_channel.rs`, replace the `request` body:

```rust
#[async_trait::async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, mut req: ApprovalRequest) -> ApprovalDecision {
        let request_id = Uuid::new_v4().to_string();

        // Build preview at the channel boundary if not already populated.
        if req.preview.is_none() {
            req.preview = Some(approval::preview::build_preview(
                &req.tool_name,
                &req.args,
                &req.ctx,
            ));
        }

        let (tx, rx) = oneshot::channel::<ApprovalDecision>();
        self.pending.insert(
            request_id.clone(),
            PendingEntry {
                sender: tx,
                tool_name: req.tool_name.clone(),
                args: req.args.clone(),
                cwd: req.ctx.cwd.clone(),
                suggested: req.suggested_grant.clone(),
            },
        );

        if let Err(e) = self.emitter.emit_approval_requested(&request_id, &req) {
            tracing::error!(?e, %request_id, "failed to emit approval_requested event");
            self.pending.remove(&request_id);
            return ApprovalDecision::Decline {
                reason: format!("internal: emit failed: {e}"),
            };
        }

        match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                self.pending.remove(&request_id);
                ApprovalDecision::Decline {
                    reason: "internal: oneshot dropped".into(),
                }
            }
            Err(_) => {
                self.pending.remove(&request_id);
                ApprovalDecision::Decline {
                    reason: "Approval timed out (600s)".into(),
                }
            }
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        // unchanged from Task 5
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}
```

- [ ] **Step 3: Add a stub `build_preview`**

The real impl lands in Task 11–17. For now, in `crates/approval/src/preview.rs` add a stub:

```rust
pub fn build_preview(
    _tool_name: &str,
    args: &serde_json::Value,
    _ctx: &crate::request::ApprovalContext,
) -> ApprovalPreview {
    // Tasks 11–17 replace this with real per-tool dispatch.
    ApprovalPreview::Generic { args: args.clone() }
}
```

- [ ] **Step 4: Build to verify**

```bash
cargo build -p app-core -p approval
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/desktop_approval_channel.rs \
        crates/app-core/src/events.rs \
        crates/approval/src/preview.rs
git commit -m "feat(approval): implement DesktopApprovalChannel::request with pending oneshot"
```

---

## Task 7: Read `GrantRow` shape and complete `build_grant_row`

**Files:**
- Modify: `crates/app-core/src/desktop_approval_channel.rs`

- [ ] **Step 1: Read the GrantRow struct**

```bash
grep -n "pub struct GrantRow" /Users/jayden/Projects/Klynt/bot/crates/approval/src/grants.rs
```

Read enough of the surrounding code to understand the exact field shape (likely tool_name, scope_data, lifetime, channel_filter, etc.).

- [ ] **Step 2: Update `build_grant_row` with real construction**

In `crates/app-core/src/desktop_approval_channel.rs`, replace the stub:

```rust
pub fn build_grant_row(
    &self,
    request_id: &str,
    rule: Option<&str>,
) -> Option<GrantRow> {
    let entry = self.pending.get(request_id)?;
    Some(GrantRow {
        // Populated per the actual GrantRow struct fields. Common shape:
        tool_name: entry.tool_name.clone(),
        // path / pattern from rule arg or from entry.args
        scope_pattern: rule
            .map(String::from)
            .or_else(|| extract_path_str_from_args(&entry.args)),
        // ...other fields per the existing schema
        ..Default::default()
    })
}

fn extract_path_str_from_args(args: &serde_json::Value) -> Option<String> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .and_then(|v| v.as_str())
        .map(String::from)
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p app-core
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/desktop_approval_channel.rs
git commit -m "feat(app-core): build_grant_row construction for AllowAlways persistence"
```

---

## Task 8: Rewrite `respond_approval`

**Files:**
- Modify: `crates/app-core/src/coding/approval_handler.rs`

- [ ] **Step 1: Replace the entire file**

Write `crates/app-core/src/coding/approval_handler.rs` (overwriting the 25-line stub):

```rust
//! Approval response handler — routes user decisions back to the gate's
//! pending-request map so the awaiting tool future resolves.

use approval::{ApprovalDecision, ApprovalGrantsRepo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::desktop_approval_channel::DesktopApprovalChannel;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppApprovalDecision {
    AllowOnce,
    AllowAlways { rule: Option<String> },
    Deny,
    AddRule { starlark_source: String },
}

#[derive(Debug, Error)]
pub enum ApprovalHandlerError {
    #[error("no pending approval found for request_id {0} (likely already resolved or timed out)")]
    NotFound(String),
    #[error("internal channel error: {0}")]
    Channel(String),
    #[error("grants repo error: {0}")]
    Grants(#[from] common::KlyntbotError),
}

#[tracing::instrument(skip(channel, grants_repo), fields(request_id = %request_id))]
pub async fn respond_approval(
    channel: Arc<DesktopApprovalChannel>,
    grants_repo: Arc<ApprovalGrantsRepo>,
    request_id: &str,
    decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    let core_decision = match &decision {
        AppApprovalDecision::AllowOnce => ApprovalDecision::Once,
        AppApprovalDecision::AllowAlways { rule } => ApprovalDecision::Forever {
            rule: rule.clone(),
        },
        AppApprovalDecision::Deny => ApprovalDecision::Decline {
            reason: "User denied".into(),
        },
        AppApprovalDecision::AddRule { starlark_source } => ApprovalDecision::Forever {
            rule: Some(starlark_source.clone()),
        },
    };

    // Persist Forever-class decisions BEFORE unblocking the gate so the very
    // next iteration's grant lookup picks up the new row.
    if matches!(
        decision,
        AppApprovalDecision::AllowAlways { .. } | AppApprovalDecision::AddRule { .. }
    ) {
        let rule_str = match &decision {
            AppApprovalDecision::AllowAlways { rule } => rule.as_deref(),
            AppApprovalDecision::AddRule { starlark_source } => Some(starlark_source.as_str()),
            _ => None,
        };
        let row = channel
            .build_grant_row(request_id, rule_str)
            .ok_or_else(|| ApprovalHandlerError::NotFound(request_id.to_string()))?;
        grants_repo.insert(row).await?;
    }

    channel
        .resolve(request_id, core_decision)
        .map_err(|e| match e {
            crate::desktop_approval_channel::ResolveError::NotFound(id) => {
                ApprovalHandlerError::NotFound(id)
            }
            crate::desktop_approval_channel::ResolveError::SendFailed => {
                ApprovalHandlerError::Channel("oneshot recipient dropped".into())
            }
        })
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p app-core
```

Expected: clean build. Adapt `ApprovalGrantsRepo::insert` signature if it differs.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/coding/approval_handler.rs
git commit -m "feat(coding): rewrite respond_approval to route decisions to the gate"
```

---

## Task 9: Wire `DesktopApprovalChannel` into `AppCore`

**Files:**
- Modify: `crates/app-core/src/lib.rs` or `crates/app-core/src/init/mod.rs` (per Task 1 finding)

- [ ] **Step 1: Add the typed handle to AppCore**

In whichever file holds the `AppCore` struct, add the field:

```rust
pub struct AppCore {
    // ...existing fields...
    pub approval_gate: Arc<approval::ApprovalGate>,
    pub desktop_approval_channel: Arc<crate::desktop_approval_channel::DesktopApprovalChannel>,
    pub grants_repo: Arc<approval::ApprovalGrantsRepo>,
}
```

(`approval_gate` and `grants_repo` may already be present per the existing assistant-mode wiring.)

- [ ] **Step 2: Construct the channel at AppCore init**

In `AppCore::new` (or whichever constructor is used), add:

```rust
let desktop_approval_channel = Arc::new(
    crate::desktop_approval_channel::DesktopApprovalChannel::new(emitter.clone()),
);
let approval_gate = Arc::new(
    approval::ApprovalGate::new(desktop_approval_channel.clone() as Arc<dyn approval::ApprovalChannel>)
        .with_grants_repo(grants_repo.clone()),
);
```

If `ApprovalGate::new` and `with_grants_repo` don't match the actual API, adapt — Task 1's investigation should have noted the existing pattern.

- [ ] **Step 3: Add `AppCore::respond_approval` method**

In the impl block:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn respond_approval(
    &self,
    request_id: &str,
    decision: crate::coding::approval_handler::AppApprovalDecision,
) -> common::Result<()> {
    crate::coding::approval_handler::respond_approval(
        self.desktop_approval_channel.clone(),
        self.grants_repo.clone(),
        request_id,
        decision,
    )
    .await
    .map_err(|e| common::KlyntbotError::Other(e.to_string()))
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p app-core
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/
git commit -m "feat(app-core): wire DesktopApprovalChannel into AppCore + add respond_approval method"
```

---

## Task 10: Verify Tauri command shell exists / wire if missing

**Files:**
- Modify: `crates/desktop/src/commands/coding/approval.rs` (or wherever found in Task 1)

- [ ] **Step 1: Confirm the existing command shell**

Per Task 1, find the file. Read it.

- [ ] **Step 2: Update or add the command**

The command shell should look like:

```rust
use std::sync::Arc;
use tauri::State;
use desktop_macros::klynt_command;
use klynt_app_core::AppCore;
use klynt_app_core::coding::approval_handler::AppApprovalDecision;

#[klynt_command]
pub async fn chat_respond_approval(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    request_id: String,
    decision: AppApprovalDecision,
) -> common::Result<()> {
    let _ = session_key; // session_key kept for future use; current routing is global
    state.respond_approval(&request_id, decision).await
}
```

- [ ] **Step 3: Verify the command is listed in `klynt_collect_commands![...]`**

```bash
grep -n "chat_respond_approval" /Users/jayden/Projects/Klynt/bot/crates/desktop/src/specta_builder.rs
```

If missing, add it to the macro list.

- [ ] **Step 4: Build to regenerate bindings**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p desktop
```

If the bindings file regeneration is gated by `cargo tauri dev`, run it briefly:

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev &
sleep 10
kill %1
```

Verify `desktop-ui/src/bindings.ts` mentions `chat_respond_approval`.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/coding/approval.rs \
        crates/desktop/src/specta_builder.rs \
        desktop-ui/src/bindings.ts
git commit -m "feat(desktop): wire chat_respond_approval Tauri command to AppCore"
```

---

## Task 11: Wave 1 integration test — approve_once

**Files:**
- Create: `crates/app-core/tests/approval_end_to_end.rs`

- [ ] **Step 1: Create the test file**

Write `crates/app-core/tests/approval_end_to_end.rs`:

```rust
//! End-to-end tests for the approval flow.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use klynt_app_core::desktop_approval_channel::DesktopApprovalChannel;
use approval::{ApprovalChannel, ApprovalClass, ApprovalContext, ApprovalRequest, ApprovalScope, ChannelKind};
use common::SessionMode;

struct NoopEmitter;

impl klynt_app_core::events::AppEventEmitter for NoopEmitter {
    fn emit_approval_requested(
        &self,
        _request_id: &str,
        _req: &ApprovalRequest,
    ) -> common::Result<()> {
        Ok(())
    }
    // ...stub other methods as needed; copy from existing test fixtures
}

fn make_request() -> ApprovalRequest {
    ApprovalRequest {
        tool_name: "edit".to_string(),
        action: None,
        args: serde_json::json!({"path": "src/main.rs"}),
        class: ApprovalClass::Destructive,
        scope: ApprovalScope::ToolAction,
        ctx: ApprovalContext {
            mode: SessionMode::Coding,
            channel: ChannelKind::Desktop,
            session_id: "test-session".to_string(),
            user_id: None,
            cwd: std::path::PathBuf::from("."),
        },
        preview: None,
        suggested_grant: None,
    }
}

#[tokio::test]
async fn approve_once_unblocks_tool() {
    let emitter = Arc::new(NoopEmitter) as Arc<dyn klynt_app_core::events::AppEventEmitter>;
    let channel = Arc::new(DesktopApprovalChannel::new(emitter));

    // Spawn channel.request which will block on oneshot
    let channel_clone = channel.clone();
    let req = make_request();
    let handle = tokio::spawn(async move { channel_clone.request(req).await });

    // Give the request task time to insert its pending entry
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Find the pending request_id by inspecting the dashmap
    let request_id = {
        // Access pattern: channel.pending().iter().next().unwrap().key().clone()
        // Adapt to actual API; may need to expose a test-only `pending_ids()` helper.
        channel
            .pending_ids()
            .pop()
            .expect("expected one pending entry")
    };

    // Resolve with Once
    channel
        .resolve(&request_id, approval::ApprovalDecision::Once)
        .expect("resolve");

    // Future should complete with Once
    let decision = timeout(Duration::from_secs(1), handle)
        .await
        .expect("timeout")
        .expect("join");
    assert!(matches!(decision, approval::ApprovalDecision::Once));
}
```

If `pending_ids()` helper doesn't exist, add it to `DesktopApprovalChannel` gated by `#[cfg(test)]`:

```rust
#[cfg(test)]
impl DesktopApprovalChannel {
    pub fn pending_ids(&self) -> Vec<String> {
        self.pending.iter().map(|e| e.key().clone()).collect()
    }
}
```

- [ ] **Step 2: Run the test**

```bash
cargo nextest run -p app-core approve_once_unblocks_tool
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/tests/approval_end_to_end.rs \
        crates/app-core/src/desktop_approval_channel.rs
git commit -m "test(app-core): integration test — approve_once unblocks tool future"
```

---

## Task 12: Wave 1 integration test — approve_always_persists_grant

**Files:**
- Modify: `crates/app-core/tests/approval_end_to_end.rs`

- [ ] **Step 1: Add the test**

In `crates/app-core/tests/approval_end_to_end.rs`, add:

```rust
#[tokio::test]
async fn approve_always_persists_grant_then_resolves() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    // Apply approval-grants migration
    storage::migrations::run_all(&pool).await.unwrap();

    let grants_repo = Arc::new(approval::ApprovalGrantsRepo::new(pool.clone()));
    let emitter = Arc::new(NoopEmitter) as Arc<dyn klynt_app_core::events::AppEventEmitter>;
    let channel = Arc::new(DesktopApprovalChannel::new(emitter));

    let req = make_request();
    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move { channel_clone.request(req).await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let request_id = channel.pending_ids().pop().expect("pending");

    let decision = klynt_app_core::coding::approval_handler::AppApprovalDecision::AllowAlways {
        rule: Some("edit on src/main.rs".to_string()),
    };
    klynt_app_core::coding::approval_handler::respond_approval(
        channel.clone(),
        grants_repo.clone(),
        &request_id,
        decision,
    )
    .await
    .expect("respond_approval");

    // Assert grant row inserted
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM approval_grants")
        .fetch_one(pool.acquire().await.unwrap().as_mut())
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Assert future resolved with Forever
    let result = timeout(Duration::from_secs(1), handle)
        .await
        .expect("timeout")
        .expect("join");
    assert!(matches!(result, approval::ApprovalDecision::Forever { .. }));
}
```

- [ ] **Step 2: Run**

```bash
cargo nextest run -p app-core approve_always_persists_grant_then_resolves
```

Expected: PASS. Adapt query column names if migrations differ.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/tests/approval_end_to_end.rs
git commit -m "test(app-core): integration test — approve_always persists grant + resolves"
```

---

## Task 13: Wave 1 integration test — deny_returns_decline

**Files:**
- Modify: `crates/app-core/tests/approval_end_to_end.rs`

- [ ] **Step 1: Add the test**

```rust
#[tokio::test]
async fn deny_returns_decline() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::migrations::run_all(&pool).await.unwrap();
    let grants_repo = Arc::new(approval::ApprovalGrantsRepo::new(pool));
    let emitter = Arc::new(NoopEmitter) as Arc<dyn klynt_app_core::events::AppEventEmitter>;
    let channel = Arc::new(DesktopApprovalChannel::new(emitter));

    let req = make_request();
    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move { channel_clone.request(req).await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let request_id = channel.pending_ids().pop().expect("pending");

    klynt_app_core::coding::approval_handler::respond_approval(
        channel.clone(),
        grants_repo,
        &request_id,
        klynt_app_core::coding::approval_handler::AppApprovalDecision::Deny,
    )
    .await
    .expect("respond");

    let result = timeout(Duration::from_secs(1), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, approval::ApprovalDecision::Decline { .. }));
}
```

- [ ] **Step 2: Run**

```bash
cargo nextest run -p app-core deny_returns_decline
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/tests/approval_end_to_end.rs
git commit -m "test(app-core): integration test — deny returns Decline"
```

---

## Task 14: Wave 1 manual smoke test

**Files:** none.

- [ ] **Step 1: Start dev environment**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run dev:vite &
sleep 3
cd /Users/jayden/Projects/Klynt/bot && KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

- [ ] **Step 2: Open coding mode and trigger an approval**

Switch to coding mode in the desktop app. Send: "Edit README.md to add a single line at the top saying 'Hello'."

- [ ] **Step 3: Verify the approval card appears**

Expected: an approval card shows up inline in the chat for the `edit` tool call. The args row shows the JSON of the tool call.

- [ ] **Step 4: Click "Allow once"**

Expected: card transitions to "approved" state within 1 second; tool executes; assistant continues.

- [ ] **Step 5: Test "Deny"**

Send another edit command; click Deny. Expected: agent reports the rejection and either retries or stops gracefully.

- [ ] **Step 6: Test timeout**

Send another edit command. Wait 600 seconds without clicking. Expected: card auto-resolves with `timed-out` status.

- [ ] **Step 7: Stop dev environment**

```bash
# In the terminal running cargo tauri dev: Ctrl+C
```

- [ ] **Step 8: Commit (no changes; this is verification only)**

If anything failed, fix and commit. Otherwise tag the wave-1 milestone:

```bash
git tag wave-1-approval-fix
```

---

# Phase 2 — Wave 2A: Backend preview builders

## Task 15: Add `classify_preview_kind` and `build_preview` dispatch

**Files:**
- Modify: `crates/approval/src/preview.rs`

- [ ] **Step 1: Replace stub `build_preview`**

Edit `crates/approval/src/preview.rs`. Add the constants and classifier:

```rust
const MAX_DIFF_LINES: usize = 200;
const MAX_BODY_CHARS: usize = 500;
const MAX_COMMAND_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy)]
enum PreviewKind {
    Diff,
    Command,
    Url,
    Mcp,
    Generic,
}

fn classify_preview_kind(tool_name: &str) -> PreviewKind {
    if tool_name.starts_with("mcp_") {
        return PreviewKind::Mcp;
    }
    match tool_name {
        "edit" | "write" | "multi_edit" | "multiedit" | "notebook_edit"
        | "apply_patch" | "str_replace_file" | "str_replace_based_edit_tool"
        | "create_file" | "write_file" | "edit_file" => PreviewKind::Diff,
        "bash" | "shell" | "run_command" | "execute_command" => PreviewKind::Command,
        "web_fetch" | "http_get" | "http_post" | "web_search" | "fetch" => PreviewKind::Url,
        _ => PreviewKind::Generic,
    }
}

pub fn build_preview(
    tool_name: &str,
    args: &serde_json::Value,
    ctx: &crate::request::ApprovalContext,
) -> ApprovalPreview {
    match classify_preview_kind(tool_name) {
        PreviewKind::Diff => build_diff_preview(args, ctx)
            .unwrap_or_else(|| ApprovalPreview::Generic { args: args.clone() }),
        PreviewKind::Command => build_command_preview(args, ctx),
        PreviewKind::Url => build_url_preview(args),
        PreviewKind::Mcp => build_mcp_preview(tool_name, args),
        PreviewKind::Generic => ApprovalPreview::Generic { args: args.clone() },
    }
}

// Stubs for the next 4 tasks:
fn build_diff_preview(_args: &serde_json::Value, _ctx: &crate::request::ApprovalContext) -> Option<ApprovalPreview> { None }
fn build_command_preview(args: &serde_json::Value, _ctx: &crate::request::ApprovalContext) -> ApprovalPreview {
    ApprovalPreview::Generic { args: args.clone() }
}
fn build_url_preview(args: &serde_json::Value) -> ApprovalPreview {
    ApprovalPreview::Generic { args: args.clone() }
}
fn build_mcp_preview(_tool: &str, args: &serde_json::Value) -> ApprovalPreview {
    ApprovalPreview::Generic { args: args.clone() }
}
```

- [ ] **Step 2: Add classifier unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_edit_tools() {
        assert!(matches!(classify_preview_kind("edit"), PreviewKind::Diff));
        assert!(matches!(classify_preview_kind("str_replace_file"), PreviewKind::Diff));
        assert!(matches!(classify_preview_kind("apply_patch"), PreviewKind::Diff));
        assert!(matches!(classify_preview_kind("write_file"), PreviewKind::Diff));
    }

    #[test]
    fn classifies_shell_tools() {
        assert!(matches!(classify_preview_kind("bash"), PreviewKind::Command));
        assert!(matches!(classify_preview_kind("execute_command"), PreviewKind::Command));
    }

    #[test]
    fn classifies_url_tools() {
        assert!(matches!(classify_preview_kind("web_fetch"), PreviewKind::Url));
        assert!(matches!(classify_preview_kind("http_post"), PreviewKind::Url));
    }

    #[test]
    fn classifies_mcp_prefix() {
        assert!(matches!(classify_preview_kind("mcp_linear_create_issue"), PreviewKind::Mcp));
    }

    #[test]
    fn classifies_unknown_to_generic() {
        assert!(matches!(classify_preview_kind("custom_tool"), PreviewKind::Generic));
    }
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p approval preview::tests
```

Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/approval/src/preview.rs
git commit -m "feat(approval): add classify_preview_kind dispatch + classifier tests"
```

---

## Task 16: Implement `build_diff_preview`

**Files:**
- Modify: `crates/approval/src/preview.rs`
- Modify: `crates/approval/Cargo.toml` (if `similar` not yet a dep)

- [ ] **Step 1: Verify `similar` is a workspace dep**

```bash
grep "similar" /Users/jayden/Projects/Klynt/bot/Cargo.toml /Users/jayden/Projects/Klynt/bot/crates/approval/Cargo.toml
```

If absent, add to `crates/approval/Cargo.toml`:
```toml
[dependencies]
similar = "2"
```

- [ ] **Step 2: Replace the stub with real impl**

Replace `build_diff_preview` in `crates/approval/src/preview.rs`:

```rust
fn build_diff_preview(
    args: &serde_json::Value,
    ctx: &crate::request::ApprovalContext,
) -> Option<ApprovalPreview> {
    let path_str = args
        .get("path")
        .or_else(|| args.get("file_path"))
        .and_then(serde_json::Value::as_str)?;
    let path = std::path::PathBuf::from(path_str);
    let resolved = if path.is_absolute() {
        path.clone()
    } else {
        ctx.cwd.join(&path)
    };

    let (old_text, is_new_file) = match std::fs::read_to_string(&resolved) {
        Ok(s) => (s, false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (String::new(), true),
        Err(_) => return None,
    };

    let new_text = if let Some(content) = args.get("content").and_then(serde_json::Value::as_str) {
        content.to_string()
    } else if let (Some(old_s), Some(new_s)) = (
        args.get("old_string").and_then(serde_json::Value::as_str),
        args.get("new_string").and_then(serde_json::Value::as_str),
    ) {
        if old_text.matches(old_s).count() == 0 {
            return None;
        }
        old_text.replacen(old_s, new_s, 1)
    } else {
        return None;
    };

    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
    let mut added: u32 = 0;
    let mut removed: u32 = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }

    let mut unified = diff
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", path.display()), &format!("b/{}", path.display()))
        .to_string();

    let mut is_truncated = false;
    let line_count = unified.lines().count();
    if line_count > MAX_DIFF_LINES {
        let truncated: Vec<&str> = unified.lines().take(MAX_DIFF_LINES).collect();
        unified = truncated.join("\n");
        unified.push_str(&format!(
            "\n... ({} more lines truncated)",
            line_count - MAX_DIFF_LINES
        ));
        is_truncated = true;
    }

    Some(ApprovalPreview::Diff {
        path,
        unified_diff: unified,
        lines_added: added,
        lines_removed: removed,
        is_new_file,
        is_truncated,
    })
}
```

- [ ] **Step 3: Add unit tests**

In the existing `tests` module:

```rust
#[test]
fn diff_preview_for_new_file() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_path_buf();
    let ctx = test_ctx(cwd.clone());

    let args = serde_json::json!({
        "path": "new.txt",
        "content": "hello\nworld\n",
    });
    let result = build_diff_preview(&args, &ctx).expect("preview");
    match result {
        ApprovalPreview::Diff { is_new_file, lines_added, .. } => {
            assert!(is_new_file);
            assert!(lines_added >= 2);
        }
        _ => panic!("expected Diff variant"),
    }
}

#[test]
fn diff_preview_for_existing_file_edit() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.txt");
    std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
    let ctx = test_ctx(dir.path().to_path_buf());

    let args = serde_json::json!({
        "path": "existing.txt",
        "old_string": "line2",
        "new_string": "line2_modified",
    });
    let result = build_diff_preview(&args, &ctx).expect("preview");
    match result {
        ApprovalPreview::Diff { lines_added, lines_removed, .. } => {
            assert_eq!(lines_added, 1);
            assert_eq!(lines_removed, 1);
        }
        _ => panic!("expected Diff"),
    }
}

fn test_ctx(cwd: std::path::PathBuf) -> crate::request::ApprovalContext {
    crate::request::ApprovalContext {
        mode: common::SessionMode::Coding,
        channel: crate::request::ChannelKind::Desktop,
        session_id: "test".into(),
        user_id: None,
        cwd,
    }
}
```

Add `tempfile` to dev-dependencies if missing:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Run**

```bash
cargo nextest run -p approval preview
```

Expected: PASS (7 tests now).

- [ ] **Step 5: Commit**

```bash
git add crates/approval/src/preview.rs crates/approval/Cargo.toml
git commit -m "feat(approval): implement build_diff_preview using similar crate"
```

---

## Task 17: Implement `build_command_preview`

**Files:**
- Modify: `crates/approval/src/preview.rs`

- [ ] **Step 1: Replace the stub**

```rust
const RISK_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf", "destructive recursive delete"),
    ("rm -fr", "destructive recursive delete"),
    ("curl", "network fetch (consider what's being downloaded)"),
    ("wget", "network fetch (consider what's being downloaded)"),
    ("| sh", "piped to shell — executes downloaded content"),
    ("| bash", "piped to shell — executes downloaded content"),
    ("sudo ", "elevated privileges"),
    ("chmod 777", "world-writable permissions"),
    ("dd if=", "raw disk operation"),
    (":(){", "fork bomb signature"),
    ("> /dev/sda", "raw device write"),
];

fn build_command_preview(
    args: &serde_json::Value,
    ctx: &crate::request::ApprovalContext,
) -> ApprovalPreview {
    let mut command = args
        .get("command")
        .or_else(|| args.get("cmd"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    if command.len() > MAX_COMMAND_CHARS {
        command.truncate(MAX_COMMAND_CHARS);
        command.push_str(" ...(truncated)");
    }

    let mut risk_hits: Vec<String> = Vec::new();
    for (needle, label) in RISK_PATTERNS {
        if command.contains(needle) {
            risk_hits.push((*label).to_string());
        }
    }

    ApprovalPreview::Command {
        command,
        cwd: ctx.cwd.clone(),
        is_dangerous: !risk_hits.is_empty(),
        risk_hits,
    }
}
```

- [ ] **Step 2: Add tests**

```rust
#[test]
fn command_preview_flags_rm_rf() {
    let ctx = test_ctx(std::path::PathBuf::from("."));
    let preview = build_command_preview(
        &serde_json::json!({"command": "rm -rf /tmp/foo"}),
        &ctx,
    );
    match preview {
        ApprovalPreview::Command { is_dangerous, risk_hits, .. } => {
            assert!(is_dangerous);
            assert!(risk_hits.iter().any(|s| s.contains("recursive delete")));
        }
        _ => panic!(),
    }
}

#[test]
fn command_preview_flags_curl_pipe_sh() {
    let ctx = test_ctx(std::path::PathBuf::from("."));
    let preview = build_command_preview(
        &serde_json::json!({"command": "curl https://example.com/install.sh | sh"}),
        &ctx,
    );
    match preview {
        ApprovalPreview::Command { is_dangerous, risk_hits, .. } => {
            assert!(is_dangerous);
            assert!(risk_hits.iter().any(|s| s.contains("piped to shell")));
        }
        _ => panic!(),
    }
}

#[test]
fn command_preview_truncates_long_command() {
    let ctx = test_ctx(std::path::PathBuf::from("."));
    let big = "a".repeat(MAX_COMMAND_CHARS + 1000);
    let preview = build_command_preview(
        &serde_json::json!({"command": big}),
        &ctx,
    );
    match preview {
        ApprovalPreview::Command { command, .. } => {
            assert!(command.contains("...(truncated)"));
        }
        _ => panic!(),
    }
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p approval preview
```

Expected: PASS (10 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/approval/src/preview.rs
git commit -m "feat(approval): implement build_command_preview with risk-pattern hits"
```

---

## Task 18: Implement `build_url_preview` with redaction

**Files:**
- Modify: `crates/approval/src/preview.rs`

- [ ] **Step 1: Replace the stub**

```rust
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "x-api-key",
    "x-auth-token",
    "proxy-authorization",
    "set-cookie",
];

fn redact_header_value(name: &str, value: &str) -> String {
    if SENSITIVE_HEADERS.iter().any(|h| h.eq_ignore_ascii_case(name)) {
        "<redacted>".to_string()
    } else {
        value.to_string()
    }
}

fn build_url_preview(args: &serde_json::Value) -> ApprovalPreview {
    let url = args
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let method = args
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();

    let headers: Vec<(String, String)> = args
        .get("headers")
        .and_then(serde_json::Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    let raw = v.as_str().unwrap_or("").to_string();
                    (k.clone(), redact_header_value(k, &raw))
                })
                .collect()
        })
        .unwrap_or_default();

    let body_preview = args
        .get("body")
        .and_then(serde_json::Value::as_str)
        .map(|b| {
            if b.chars().count() > MAX_BODY_CHARS {
                let truncated: String = b.chars().take(MAX_BODY_CHARS).collect();
                format!("{truncated}... (truncated)")
            } else {
                b.to_string()
            }
        });

    ApprovalPreview::Url {
        method,
        url,
        headers,
        body_preview,
    }
}
```

- [ ] **Step 2: Add tests**

```rust
#[test]
fn url_preview_redacts_authorization_header() {
    let preview = build_url_preview(&serde_json::json!({
        "url": "https://api.example.com/x",
        "method": "POST",
        "headers": {"Authorization": "Bearer secret123"},
        "body": "hello",
    }));
    match preview {
        ApprovalPreview::Url { headers, .. } => {
            let auth = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("authorization"));
            assert_eq!(auth.unwrap().1, "<redacted>");
        }
        _ => panic!(),
    }
}

#[test]
fn url_preview_keeps_non_sensitive_headers() {
    let preview = build_url_preview(&serde_json::json!({
        "url": "https://api.example.com/x",
        "headers": {"User-Agent": "Klynt/1.0"},
    }));
    match preview {
        ApprovalPreview::Url { headers, .. } => {
            let ua = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("user-agent"));
            assert_eq!(ua.unwrap().1, "Klynt/1.0");
        }
        _ => panic!(),
    }
}

#[test]
fn url_preview_truncates_long_body() {
    let big = "x".repeat(MAX_BODY_CHARS + 100);
    let preview = build_url_preview(&serde_json::json!({
        "url": "https://example.com",
        "body": big,
    }));
    match preview {
        ApprovalPreview::Url { body_preview, .. } => {
            assert!(body_preview.unwrap().contains("(truncated)"));
        }
        _ => panic!(),
    }
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p approval preview
```

Expected: PASS (13 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/approval/src/preview.rs
git commit -m "feat(approval): implement build_url_preview with header redaction"
```

---

## Task 19: Implement `build_mcp_preview`

**Files:**
- Modify: `crates/approval/src/preview.rs`

- [ ] **Step 1: Replace the stub**

```rust
fn build_mcp_preview(tool_name: &str, args: &serde_json::Value) -> ApprovalPreview {
    let after_prefix = tool_name.trim_start_matches("mcp_");
    let (server, tool) = after_prefix
        .split_once('_')
        .unwrap_or((after_prefix, ""));

    ApprovalPreview::Mcp {
        server: server.to_string(),
        tool: tool.to_string(),
        args: args.clone(),
        schema: None, // Phase 2: fetch from crates/mcp-bridge cached descriptors
    }
}
```

- [ ] **Step 2: Add a test**

```rust
#[test]
fn mcp_preview_extracts_server_and_tool() {
    let preview = build_mcp_preview(
        "mcp_linear_create_issue",
        &serde_json::json!({"title": "test"}),
    );
    match preview {
        ApprovalPreview::Mcp { server, tool, .. } => {
            assert_eq!(server, "linear");
            assert_eq!(tool, "create_issue");
        }
        _ => panic!(),
    }
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p approval preview
```

Expected: PASS (14 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/approval/src/preview.rs
git commit -m "feat(approval): implement build_mcp_preview"
```

---

# Phase 3 — Wave 2B: Mirror pattern learner

## Task 20: Add `approval_pattern_history` migration + DomainEvent

**Files:**
- Create: `crates/cognitive/migrations/00X_approval_pattern_history.sql`
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Find the next migration number**

```bash
ls /Users/jayden/Projects/Klynt/bot/crates/cognitive/migrations/
```

Pick the next sequential number (e.g., `004_` if last is `003_`).

- [ ] **Step 2: Write the migration**

Write `crates/cognitive/migrations/004_approval_pattern_history.sql`:

```sql
CREATE TABLE IF NOT EXISTS approval_pattern_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      TEXT NOT NULL DEFAULT 'default',
    tool_name    TEXT NOT NULL,
    path         TEXT,
    decision     TEXT NOT NULL,
    pattern_used TEXT,
    occurred_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_aph_tool_path
    ON approval_pattern_history(user_id, tool_name, path);

CREATE INDEX IF NOT EXISTS idx_aph_recency
    ON approval_pattern_history(user_id, tool_name, occurred_at);
```

- [ ] **Step 3: Add the DomainEvent variant**

In `crates/bus/src/domain_events.rs`, add to the `DomainEvent` enum:

```rust
ApprovalResolved {
    user_id: Option<String>,
    tool_name: String,
    path: Option<String>,
    decision: String,
    pattern_used: Option<String>,
    occurred_at: jiff::Timestamp,
},
```

- [ ] **Step 4: Build**

```bash
cargo build -p bus -p cognitive
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/migrations/004_approval_pattern_history.sql \
        crates/bus/src/domain_events.rs
git commit -m "feat: add approval_pattern_history table + ApprovalResolved DomainEvent"
```

---

## Task 21: Build `ApprovalPatternLearner` skeleton

**Files:**
- Create: `crates/cognitive/src/mirror/sources/approval_patterns.rs`
- Modify: `crates/cognitive/src/mirror/sources/mod.rs`

- [ ] **Step 1: Read existing source structure**

```bash
ls /Users/jayden/Projects/Klynt/bot/crates/cognitive/src/mirror/sources/
cat /Users/jayden/Projects/Klynt/bot/crates/cognitive/src/mirror/sources/mod.rs
```

Note the trait name and signature pattern.

- [ ] **Step 2: Write the new source file**

Write `crates/cognitive/src/mirror/sources/approval_patterns.rs` (skeleton):

```rust
//! Approval-pattern learning — the 7th Mirror signal source.

use crate::mirror::source::MirrorSignalSource;
use bus::domain_events::DomainEvent;
use common::Result;
use jiff::Timestamp;
use std::path::Path;
use std::sync::Arc;
use storage::StoragePool;
use tokio::sync::broadcast;

const MIN_APPROVAL_COUNT: u32 = 3;
const MIN_APPROVAL_RATE: f32 = 0.80;
const RECENCY_WINDOW_DAYS: i64 = 30;

pub struct ApprovalPatternLearner {
    pool: StoragePool,
}

impl ApprovalPatternLearner {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn suggest_pattern(
        &self,
        _tool_name: &str,
        _path: Option<&Path>,
        _ctx: &approval::ApprovalContext,
    ) -> Option<approval::SuggestedGrant> {
        // Implemented in Task 22
        None
    }

    pub async fn persist_observation(
        &self,
        _user_id: &str,
        _tool_name: &str,
        _path: Option<&str>,
        _decision: &str,
        _pattern_used: Option<&str>,
        _occurred_at: Timestamp,
    ) -> Result<()> {
        // Implemented in Task 22
        Ok(())
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for ApprovalPatternLearner {
    fn id(&self) -> &'static str {
        "approval_patterns"
    }

    async fn run(
        &self,
        mut bus_rx: broadcast::Receiver<DomainEvent>,
        _shutdown: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        while let Ok(event) = bus_rx.recv().await {
            if let DomainEvent::ApprovalResolved {
                user_id,
                tool_name,
                path,
                decision,
                pattern_used,
                occurred_at,
            } = event
            {
                let _ = self
                    .persist_observation(
                        user_id.as_deref().unwrap_or("default"),
                        &tool_name,
                        path.as_deref(),
                        &decision,
                        pattern_used.as_deref(),
                        occurred_at,
                    )
                    .await;
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 3: Wire into `sources/mod.rs`**

```rust
pub mod approval_patterns;
```

- [ ] **Step 4: Build**

```bash
cargo build -p cognitive
```

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/sources/approval_patterns.rs \
        crates/cognitive/src/mirror/sources/mod.rs
git commit -m "feat(cognitive): scaffold ApprovalPatternLearner signal source"
```

---

## Task 22: Implement `persist_observation` and `suggest_pattern`

**Files:**
- Modify: `crates/cognitive/src/mirror/sources/approval_patterns.rs`

- [ ] **Step 1: Implement `persist_observation`**

```rust
impl ApprovalPatternLearner {
    pub async fn persist_observation(
        &self,
        user_id: &str,
        tool_name: &str,
        path: Option<&str>,
        decision: &str,
        pattern_used: Option<&str>,
        occurred_at: Timestamp,
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query!(
            r#"
            INSERT INTO approval_pattern_history
              (user_id, tool_name, path, decision, pattern_used, occurred_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            user_id,
            tool_name,
            path,
            decision,
            pattern_used,
            occurred_at,
        )
        .execute(conn.as_mut())
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 2: Add `PatternCandidate` struct + builders**

```rust
#[derive(Debug, Clone)]
struct PatternCandidate {
    path_filter: String,
    human_readable: String,
    scope: approval::GrantScope,
    kind: PatternKind,
}

#[derive(Debug, Clone, Copy)]
enum PatternKind {
    Exact,
    Folder,
    RecursiveGlob,
    ExtensionGlob,
}

impl PatternCandidate {
    fn specificity_weight(&self) -> f32 {
        match self.kind {
            PatternKind::Exact => 4.0,
            PatternKind::Folder => 3.0,
            PatternKind::RecursiveGlob => 2.0,
            PatternKind::ExtensionGlob => 1.5,
        }
    }

    fn exact_path(tool: &str, path: &Path) -> Option<Self> {
        Some(Self {
            path_filter: path.display().to_string(),
            human_readable: format!("{} on {}", tool, path.display()),
            scope: approval::GrantScope::ExactToolPath {
                tool: tool.to_string(),
                path: path.to_path_buf(),
            },
            kind: PatternKind::Exact,
        })
    }

    fn parent_folder(tool: &str, path: &Path) -> Option<Self> {
        let folder = path.parent()?.to_path_buf();
        Some(Self {
            path_filter: format!("{}/%", folder.display()),
            human_readable: format!("{} in {}/", tool, folder.display()),
            scope: approval::GrantScope::ToolFolder {
                tool: tool.to_string(),
                folder,
            },
            kind: PatternKind::Folder,
        })
    }

    fn recursive_glob(tool: &str, path: &Path) -> Option<Self> {
        let segs: Vec<_> = path.iter().collect();
        if segs.len() < 3 {
            return None;
        }
        let prefix: std::path::PathBuf = segs[..2].iter().collect();
        let glob = format!("{}/**", prefix.display());
        Some(Self {
            path_filter: format!("{}/%", prefix.display()),
            human_readable: format!("{} on {}", tool, glob),
            scope: approval::GrantScope::ToolGlob {
                tool: tool.to_string(),
                glob,
            },
            kind: PatternKind::RecursiveGlob,
        })
    }

    fn extension_glob(tool: &str, path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        let glob = format!("**/*.{}", ext);
        Some(Self {
            path_filter: format!("%.{}", ext),
            human_readable: format!("{} on {}", tool, glob),
            scope: approval::GrantScope::ToolGlob {
                tool: tool.to_string(),
                glob,
            },
            kind: PatternKind::ExtensionGlob,
        })
    }
}
```

- [ ] **Step 3: Implement `suggest_pattern`**

```rust
impl ApprovalPatternLearner {
    pub async fn suggest_pattern(
        &self,
        tool_name: &str,
        path: Option<&Path>,
        ctx: &approval::ApprovalContext,
    ) -> Option<approval::SuggestedGrant> {
        let path = path?;

        let candidates = vec![
            PatternCandidate::exact_path(tool_name, path),
            PatternCandidate::parent_folder(tool_name, path),
            PatternCandidate::recursive_glob(tool_name, path),
            PatternCandidate::extension_glob(tool_name, path),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        let mut best: Option<(PatternCandidate, u32)> = None;
        for candidate in candidates {
            let (approval_count, total) = self
                .count_history(tool_name, &candidate.path_filter, ctx)
                .await
                .unwrap_or((0, 0));

            if approval_count < MIN_APPROVAL_COUNT || total == 0 {
                continue;
            }
            let rate = approval_count as f32 / total as f32;
            if rate < MIN_APPROVAL_RATE {
                continue;
            }
            let score = approval_count as f32 * candidate.specificity_weight();
            match &best {
                Some((_, prev_score)) if (*prev_score as f32 * candidate.specificity_weight()) >= score => {}
                _ => best = Some((candidate, approval_count)),
            }
        }

        best.map(|(candidate, approvals)| approval::SuggestedGrant {
            pattern: candidate.human_readable.clone(),
            scope: candidate.scope.clone(),
            reason: format!(
                "Mirror has seen {} prior approvals matching `{}`",
                approvals, candidate.human_readable
            ),
        })
    }

    async fn count_history(
        &self,
        tool_name: &str,
        path_like: &str,
        ctx: &approval::ApprovalContext,
    ) -> Result<(u32, u32)> {
        let cutoff = Timestamp::now() - jiff::Span::new().days(RECENCY_WINDOW_DAYS);
        let user = ctx.user_id.as_deref().unwrap_or("default");
        let mut conn = self.pool.acquire().await?;
        let row = sqlx::query!(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN decision IN ('once','forever') THEN 1 ELSE 0 END), 0) as "approvals!: i64",
              COUNT(*) as "total!: i64"
            FROM approval_pattern_history
            WHERE user_id = ?1
              AND tool_name = ?2
              AND path LIKE ?3
              AND occurred_at >= ?4
            "#,
            user,
            tool_name,
            path_like,
            cutoff,
        )
        .fetch_one(conn.as_mut())
        .await?;
        Ok((row.approvals as u32, row.total as u32))
    }
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p cognitive
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/sources/approval_patterns.rs
git commit -m "feat(cognitive): implement persist + suggest_pattern in ApprovalPatternLearner"
```

---

## Task 23: Pattern learner unit tests

**Files:**
- Modify: `crates/cognitive/src/mirror/sources/approval_patterns.rs`

- [ ] **Step 1: Add test module**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> approval::ApprovalContext {
        approval::ApprovalContext {
            mode: common::SessionMode::Coding,
            channel: approval::ChannelKind::Desktop,
            session_id: "test".to_string(),
            user_id: None,
            cwd: std::path::PathBuf::from("."),
        }
    }

    async fn pool_with_migration() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        sqlx::query(
            r#"CREATE TABLE approval_pattern_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL DEFAULT 'default',
                tool_name TEXT NOT NULL,
                path TEXT,
                decision TEXT NOT NULL,
                pattern_used TEXT,
                occurred_at TEXT NOT NULL
            )"#,
        )
        .execute(pool.acquire().await.unwrap().as_mut())
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn cold_start_returns_no_suggestion() {
        let pool = pool_with_migration().await;
        let learner = ApprovalPatternLearner::new(pool);
        let ctx = test_ctx();
        let result = learner
            .suggest_pattern("edit", Some(Path::new("src/main.rs")), &ctx)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn suggests_folder_after_three_approvals_same_dir() {
        let pool = pool_with_migration().await;
        let learner = ApprovalPatternLearner::new(pool);
        let ctx = test_ctx();
        for path in &[
            "src/components/A.tsx",
            "src/components/B.tsx",
            "src/components/C.tsx",
        ] {
            learner
                .persist_observation(
                    "default",
                    "edit",
                    Some(path),
                    "once",
                    None,
                    Timestamp::now(),
                )
                .await
                .unwrap();
        }
        let result = learner
            .suggest_pattern("edit", Some(Path::new("src/components/D.tsx")), &ctx)
            .await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn no_suggestion_when_below_threshold() {
        let pool = pool_with_migration().await;
        let learner = ApprovalPatternLearner::new(pool);
        let ctx = test_ctx();
        // 2 approvals — below MIN_APPROVAL_COUNT
        for path in &["src/x/A.tsx", "src/x/B.tsx"] {
            learner
                .persist_observation("default", "edit", Some(path), "once", None, Timestamp::now())
                .await
                .unwrap();
        }
        let result = learner
            .suggest_pattern("edit", Some(Path::new("src/x/C.tsx")), &ctx)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn no_suggestion_when_approval_rate_below_threshold() {
        let pool = pool_with_migration().await;
        let learner = ApprovalPatternLearner::new(pool);
        let ctx = test_ctx();
        // 3 approvals + 2 denials = 60% < 80%
        for path in &["src/y/A.tsx", "src/y/B.tsx", "src/y/C.tsx"] {
            learner
                .persist_observation("default", "edit", Some(path), "once", None, Timestamp::now())
                .await
                .unwrap();
        }
        for path in &["src/y/D.tsx", "src/y/E.tsx"] {
            learner
                .persist_observation("default", "edit", Some(path), "denied", None, Timestamp::now())
                .await
                .unwrap();
        }
        let result = learner
            .suggest_pattern("edit", Some(Path::new("src/y/F.tsx")), &ctx)
            .await;
        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run**

```bash
cargo nextest run -p cognitive approval_patterns
```

Expected: PASS (4 tests).

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/mirror/sources/approval_patterns.rs
git commit -m "test(cognitive): unit tests for ApprovalPatternLearner"
```

---

## Task 24: Wire `ApprovalPatternLearner` into Mirror engine + facade

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Read existing engine source registration pattern**

```bash
grep -n "MirrorSignalSource\|sources" /Users/jayden/Projects/Klynt/bot/crates/cognitive/src/mirror/engine.rs | head -30
```

- [ ] **Step 2: Add ApprovalPatternLearner to engine**

In `MirrorEngine::start` (or wherever sources are constructed):

```rust
let approval_patterns = Arc::new(
    crate::mirror::sources::approval_patterns::ApprovalPatternLearner::new(pool.clone()),
);
sources.push(approval_patterns.clone() as Arc<dyn MirrorSignalSource>);
```

Save the `Arc<ApprovalPatternLearner>` for the facade.

- [ ] **Step 3: Expose accessor in facade**

In `crates/cognitive/src/mirror/facade.rs`, add field + accessor:

```rust
pub struct MirrorFacade {
    // existing fields...
    approval_patterns: Arc<crate::mirror::sources::approval_patterns::ApprovalPatternLearner>,
}

impl MirrorFacade {
    pub fn approval_patterns(&self) -> &crate::mirror::sources::approval_patterns::ApprovalPatternLearner {
        &self.approval_patterns
    }
}
```

Update the constructor accordingly.

- [ ] **Step 4: Build**

```bash
cargo build -p cognitive
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/engine.rs \
        crates/cognitive/src/mirror/facade.rs
git commit -m "feat(cognitive): register ApprovalPatternLearner in MirrorEngine + expose via facade"
```

---

## Task 25: Wire Mirror suggester into `ApprovalGate`

**Files:**
- Modify: `crates/approval/src/gate.rs`

- [ ] **Step 1: Add Mirror handle field**

The `approval` crate must NOT depend on the `cognitive` crate (would create a cycle). Solution: define a thin trait in `approval` that `MirrorFacade` implements.

In `crates/approval/src/gate.rs`, add at the top:

```rust
#[async_trait::async_trait]
pub trait ApprovalSuggester: Send + Sync {
    async fn suggest(
        &self,
        tool_name: &str,
        path: Option<&std::path::Path>,
        ctx: &crate::request::ApprovalContext,
    ) -> Option<crate::SuggestedGrant>;
}
```

Add a field to `ApprovalGate`:

```rust
pub struct ApprovalGate {
    // existing fields...
    suggester: Option<Arc<dyn ApprovalSuggester>>,
}

impl ApprovalGate {
    pub fn with_suggester(mut self, s: Arc<dyn ApprovalSuggester>) -> Self {
        self.suggester = Some(s);
        self
    }
}
```

- [ ] **Step 2: Call suggester in `check`**

In `ApprovalGate::check` (or wherever the channel.request is called), before that call:

```rust
if req.suggested_grant.is_none() {
    if let Some(s) = &self.suggester {
        let path = extract_path(&req.args);
        req.suggested_grant = s.suggest(&req.tool_name, path.as_deref(), &req.ctx).await;
    }
}
```

Helper:

```rust
fn extract_path(args: &serde_json::Value) -> Option<std::path::PathBuf> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
}
```

- [ ] **Step 3: Implement `ApprovalSuggester` for `MirrorFacade`**

In `crates/cognitive/src/mirror/facade.rs`:

```rust
#[async_trait::async_trait]
impl approval::ApprovalSuggester for MirrorFacade {
    async fn suggest(
        &self,
        tool_name: &str,
        path: Option<&std::path::Path>,
        ctx: &approval::ApprovalContext,
    ) -> Option<approval::SuggestedGrant> {
        self.approval_patterns().suggest_pattern(tool_name, path, ctx).await
    }
}
```

- [ ] **Step 4: Wire at AppCore init**

In `AppCore::new`, after constructing both gate and mirror:

```rust
let approval_gate = if let Some(mirror_facade) = &mirror_facade {
    Arc::new(
        approval::ApprovalGate::new(desktop_approval_channel.clone() as Arc<dyn approval::ApprovalChannel>)
            .with_grants_repo(grants_repo.clone())
            .with_suggester(mirror_facade.clone() as Arc<dyn approval::ApprovalSuggester>),
    )
} else {
    Arc::new(/* without suggester */)
};
```

- [ ] **Step 5: Build**

```bash
cargo build --workspace
```

Expected: clean build.

- [ ] **Step 6: Commit**

```bash
git add crates/approval/src/gate.rs \
        crates/cognitive/src/mirror/facade.rs \
        crates/app-core/src/
git commit -m "feat(approval): consult Mirror suggester for grant patterns before channel.request"
```

---

## Task 26: Emit `ApprovalResolved` from `respond_approval`

**Files:**
- Modify: `crates/app-core/src/coding/approval_handler.rs`
- Modify: `crates/app-core/src/lib.rs` (or AppCore method)

- [ ] **Step 1: Pass DomainEventBus into respond_approval**

Extend the function signature:

```rust
pub async fn respond_approval(
    channel: Arc<DesktopApprovalChannel>,
    grants_repo: Arc<ApprovalGrantsRepo>,
    bus: Arc<bus::DomainEventBus>,
    request_id: &str,
    decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    // ...existing logic...

    // After successful resolve, emit ApprovalResolved
    let entry_snapshot = channel.peek(&request_id); // need to add a non-removing peek before resolve
    let event = bus::DomainEvent::ApprovalResolved {
        user_id: None, // or from ctx
        tool_name: entry_snapshot.as_ref().map(|e| e.tool_name.clone()).unwrap_or_default(),
        path: entry_snapshot.as_ref().and_then(|e| extract_path_str_from_args(&e.args)),
        decision: match &decision {
            AppApprovalDecision::AllowOnce => "once",
            AppApprovalDecision::AllowAlways { .. } => "forever",
            AppApprovalDecision::Deny => "denied",
            AppApprovalDecision::AddRule { .. } => "forever",
        }.to_string(),
        pattern_used: match &decision {
            AppApprovalDecision::AllowAlways { rule } => rule.clone(),
            AppApprovalDecision::AddRule { starlark_source } => Some(starlark_source.clone()),
            _ => None,
        },
        occurred_at: jiff::Timestamp::now(),
    };
    let _ = bus.send(event);

    Ok(())
}
```

Note: `peek` must be added to `DesktopApprovalChannel` (read-only access without removing). Or restructure to capture the snapshot before calling resolve.

- [ ] **Step 2: Add `peek` helper to DesktopApprovalChannel**

In `crates/app-core/src/desktop_approval_channel.rs`:

```rust
impl DesktopApprovalChannel {
    pub fn peek(&self, request_id: &str) -> Option<PendingSnapshot> {
        self.pending.get(request_id).map(|e| PendingSnapshot {
            tool_name: e.tool_name.clone(),
            args: e.args.clone(),
            cwd: e.cwd.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PendingSnapshot {
    pub tool_name: String,
    pub args: serde_json::Value,
    pub cwd: std::path::PathBuf,
}
```

- [ ] **Step 3: Update AppCore::respond_approval to pass bus**

```rust
pub async fn respond_approval(
    &self,
    request_id: &str,
    decision: crate::coding::approval_handler::AppApprovalDecision,
) -> common::Result<()> {
    crate::coding::approval_handler::respond_approval(
        self.desktop_approval_channel.clone(),
        self.grants_repo.clone(),
        self.domain_event_bus.clone(),
        request_id,
        decision,
    )
    .await
    .map_err(|e| common::KlyntbotError::Other(e.to_string()))
}
```

- [ ] **Step 4: Build**

```bash
cargo build --workspace
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/approval_handler.rs \
        crates/app-core/src/desktop_approval_channel.rs \
        crates/app-core/src/lib.rs
git commit -m "feat(coding): emit ApprovalResolved DomainEvent on approval resolution"
```

---

# Phase 4 — Wave 2C: Frontend renderers

## Task 27: Create `approval-preview.css` + tokens

**Files:**
- Create: `desktop-ui/src/styles/approval-preview.css`
- Modify: `desktop-ui/src/styles/index.css`
- Modify: `desktop-ui/src/styles/ds-tokens.css` (if tokens missing)

- [ ] **Step 1: Write the CSS**

Write `desktop-ui/src/styles/approval-preview.css`:

```css
.approval-preview {
  margin-top: var(--space-2);
  border: 1px solid var(--border-muted);
  border-radius: var(--radius-md);
  overflow: hidden;
  font-size: var(--fs-xs);
}

.approval-preview__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2);
  background: var(--bg-subtle);
  border-bottom: 1px solid var(--border-muted);
}

.approval-preview__path { font-family: var(--font-mono); }
.approval-preview__cwd { color: var(--text-muted); font-family: var(--font-mono); }
.approval-preview__lines-added { color: var(--success); margin-left: auto; }
.approval-preview__lines-removed { color: var(--danger); }
.approval-preview__badge { font-size: var(--fs-2xs); padding: 0 var(--space-1); border-radius: var(--radius-sm); }
.approval-preview__badge--danger { background: var(--danger-bg-muted); color: var(--danger); }
.approval-preview__badge--new { background: var(--accent-bg-muted); color: var(--accent); }

.approval-preview__diff {
  margin: 0;
  padding: var(--space-2);
  font-family: var(--font-mono);
  white-space: pre;
  overflow-x: auto;
  max-height: 400px;
  overflow-y: auto;
}

.approval-preview__diff-line { display: block; }
.approval-preview__diff-line--added { background: var(--success-bg-muted); color: var(--success); }
.approval-preview__diff-line--removed { background: var(--danger-bg-muted); color: var(--danger); }
.approval-preview__diff-line--hunk { color: var(--accent); font-weight: 600; }
.approval-preview__diff-line--filehead { color: var(--text-muted); font-weight: 600; }

.approval-preview__truncated {
  padding: var(--space-1) var(--space-2);
  background: var(--bg-subtle);
  color: var(--text-muted);
  font-size: var(--fs-2xs);
  text-align: center;
}

.approval-preview__command {
  margin: 0;
  padding: var(--space-2);
  font-family: var(--font-mono);
  white-space: pre-wrap;
  word-break: break-all;
}

.approval-preview__risks {
  margin: 0;
  padding: var(--space-1) var(--space-2);
  list-style: none;
  background: var(--danger-bg-muted);
  color: var(--danger);
  font-size: var(--fs-2xs);
}

.approval-card__split-button {
  display: inline-flex;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
}
.approval-card__split-primary { padding: var(--space-1) var(--space-2); cursor: pointer; }
.approval-card__split-caret { padding: var(--space-1); border-left: 1px solid var(--border); cursor: pointer; }

.approval-card__pattern-picker {
  margin: var(--space-1) 0 0 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  overflow: hidden;
}
.approval-card__pattern-picker-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: var(--space-1) var(--space-2);
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--border-muted);
  cursor: pointer;
}
.approval-card__pattern-picker-item:last-child { border-bottom: 0; }
.approval-card__pattern-picker-item:hover { background: var(--bg-subtle); }
.approval-card__pattern-picker-item--suggested { background: var(--accent-bg-muted); font-weight: 600; }
.approval-card__pattern-reason { display: block; font-size: var(--fs-2xs); color: var(--text-muted); font-weight: 400; }
```

- [ ] **Step 2: Add `@import` in `index.css`**

Edit `desktop-ui/src/styles/index.css`. Add:

```css
@import "./approval-preview.css";
```

- [ ] **Step 3: Add any missing tokens to `ds-tokens.css`**

Check for `--success-bg-muted`, `--danger-bg-muted`, `--accent-bg-muted`, `--bg-elevated`. Add to `ds-tokens.css` if missing.

- [ ] **Step 4: Verify load**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run typecheck
```

Expected: no TS errors. CSS errors won't surface in typecheck but will at runtime.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/styles/approval-preview.css \
        desktop-ui/src/styles/index.css \
        desktop-ui/src/styles/ds-tokens.css
git commit -m "feat(ui): add approval-preview.css with token-driven styles"
```

---

## Task 28: Build `<PreviewRenderer>` dispatcher

**Files:**
- Create: `desktop-ui/src/features/coding/components/preview/PreviewRenderer.tsx`

- [ ] **Step 1: Write the component**

```tsx
import type { ApprovalPreview } from "@/types/bindings";
import { CommandPreview } from "./CommandPreview";
import { DiffPreview } from "./DiffPreview";
import { GenericPreview } from "./GenericPreview";
import { McpPreview } from "./McpPreview";
import { UrlPreview } from "./UrlPreview";

export function PreviewRenderer({ preview }: { preview: ApprovalPreview }) {
  switch (preview.kind) {
    case "diff":
      return <DiffPreview {...preview} />;
    case "command":
      return <CommandPreview {...preview} />;
    case "url":
      return <UrlPreview {...preview} />;
    case "mcp":
      return <McpPreview {...preview} />;
    case "generic":
      return <GenericPreview {...preview} />;
    default: {
      const _exhaustive: never = preview;
      return null;
    }
  }
}
```

- [ ] **Step 2: Commit (can't build until siblings exist)**

```bash
git add desktop-ui/src/features/coding/components/preview/PreviewRenderer.tsx
git commit -m "feat(ui): add PreviewRenderer dispatcher"
```

---

## Task 29: Build `<DiffPreview>`

**Files:**
- Create: `desktop-ui/src/features/coding/components/preview/DiffPreview.tsx`

- [ ] **Step 1: Write the component**

```tsx
import type { ApprovalPreview } from "@/types/bindings";

type DiffProps = Extract<ApprovalPreview, { kind: "diff" }>;

export function DiffPreview({
  path,
  unified_diff,
  lines_added,
  lines_removed,
  is_new_file,
  is_truncated,
}: DiffProps) {
  return (
    <div className="approval-preview approval-preview--diff">
      <header className="approval-preview__head">
        <span className="approval-preview__path">{path}</span>
        {is_new_file && (
          <span className="approval-preview__badge approval-preview__badge--new">new file</span>
        )}
        <span className="approval-preview__lines-added">+{lines_added}</span>
        <span className="approval-preview__lines-removed">−{lines_removed}</span>
      </header>
      <pre className="approval-preview__diff">
        {unified_diff.split("\n").map((line, idx) => (
          <DiffLine key={idx} text={line} />
        ))}
      </pre>
      {is_truncated && (
        <footer className="approval-preview__truncated">
          Truncated — approve to see full diff in the editor.
        </footer>
      )}
    </div>
  );
}

function DiffLine({ text }: { text: string }) {
  let className = "approval-preview__diff-line";
  if (text.startsWith("+++") || text.startsWith("---")) {
    className += " approval-preview__diff-line--filehead";
  } else if (text.startsWith("@@")) {
    className += " approval-preview__diff-line--hunk";
  } else if (text.startsWith("+")) {
    className += " approval-preview__diff-line--added";
  } else if (text.startsWith("-")) {
    className += " approval-preview__diff-line--removed";
  }
  return <span className={className}>{text || " "}</span>;
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/coding/components/preview/DiffPreview.tsx
git commit -m "feat(ui): DiffPreview component"
```

---

## Task 30: Build `<CommandPreview>`, `<UrlPreview>`, `<McpPreview>`, `<GenericPreview>`

**Files:**
- Create: 4 component files

- [ ] **Step 1: Write CommandPreview**

```tsx
// desktop-ui/src/features/coding/components/preview/CommandPreview.tsx
import type { ApprovalPreview } from "@/types/bindings";

type CommandProps = Extract<ApprovalPreview, { kind: "command" }>;

export function CommandPreview({ command, cwd, is_dangerous, risk_hits }: CommandProps) {
  return (
    <div className="approval-preview approval-preview--command">
      <header className="approval-preview__head">
        <span className="approval-preview__cwd">{cwd}</span>
        {is_dangerous && (
          <span className="approval-preview__badge approval-preview__badge--danger">⚠ dangerous</span>
        )}
      </header>
      <pre className="approval-preview__command">{command}</pre>
      {risk_hits.length > 0 && (
        <ul className="approval-preview__risks">
          {risk_hits.map((hit, idx) => (
            <li key={idx}>{hit}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Write UrlPreview**

```tsx
// desktop-ui/src/features/coding/components/preview/UrlPreview.tsx
import type { ApprovalPreview } from "@/types/bindings";

type UrlProps = Extract<ApprovalPreview, { kind: "url" }>;

export function UrlPreview({ method, url, headers, body_preview }: UrlProps) {
  return (
    <div className="approval-preview approval-preview--url">
      <header className="approval-preview__head">
        <span className="approval-preview__badge">{method}</span>
        <span className="approval-preview__path">{url}</span>
      </header>
      {headers.length > 0 && (
        <dl className="approval-preview__headers">
          {headers.map(([k, v]) => (
            <div key={k}>
              <dt>{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
        </dl>
      )}
      {body_preview && <pre className="approval-preview__command">{body_preview}</pre>}
    </div>
  );
}
```

- [ ] **Step 3: Write McpPreview**

```tsx
// desktop-ui/src/features/coding/components/preview/McpPreview.tsx
import type { ApprovalPreview } from "@/types/bindings";

type McpProps = Extract<ApprovalPreview, { kind: "mcp" }>;

export function McpPreview({ server, tool, args, schema }: McpProps) {
  return (
    <div className="approval-preview approval-preview--mcp">
      <header className="approval-preview__head">
        <span className="approval-preview__path">{server} / {tool}</span>
      </header>
      <pre className="approval-preview__command">{JSON.stringify(args, null, 2)}</pre>
      {schema && (
        <details>
          <summary>Schema</summary>
          <pre className="approval-preview__command">{JSON.stringify(schema, null, 2)}</pre>
        </details>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Write GenericPreview**

```tsx
// desktop-ui/src/features/coding/components/preview/GenericPreview.tsx
import type { ApprovalPreview } from "@/types/bindings";

type GenericProps = Extract<ApprovalPreview, { kind: "generic" }>;

export function GenericPreview({ args }: GenericProps) {
  return (
    <pre className="approval-preview__command">{JSON.stringify(args, null, 2)}</pre>
  );
}
```

- [ ] **Step 5: Typecheck**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run typecheck
```

Expected: clean. (Requires `bindings.ts` to have been regenerated with the new types — Task 6's tauri dev step covers that.)

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding/components/preview/
git commit -m "feat(ui): CommandPreview, UrlPreview, McpPreview, GenericPreview"
```

---

## Task 31: Build `<PatternPicker>` and `<SmartAllowAlwaysButton>`

**Files:**
- Create: `desktop-ui/src/features/coding/components/PatternPicker.tsx`
- Create: `desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.tsx`

- [ ] **Step 1: Write PatternPicker**

```tsx
// desktop-ui/src/features/coding/components/PatternPicker.tsx
import type { SuggestedGrant } from "@/types/bindings";

type Alternative = { pattern: string; label: string };

function deriveAlternatives(suggested: SuggestedGrant): Alternative[] {
  if (suggested.scope.kind === "tool_folder") {
    return [
      { pattern: `${suggested.scope.tool} on ${suggested.scope.folder}/**`, label: "deeper recursion" },
    ];
  }
  if (suggested.scope.kind === "exact_tool_path") {
    return [
      { pattern: `${suggested.scope.tool} in same folder`, label: "broaden to folder" },
    ];
  }
  return [];
}

type Props = {
  suggested: SuggestedGrant;
  onCommit: (rule: string) => void;
  onCustom: () => void;
};

export function PatternPicker({ suggested, onCommit, onCustom }: Props) {
  const alternatives = deriveAlternatives(suggested);
  return (
    <ul className="approval-card__pattern-picker" role="radiogroup">
      <li>
        <button
          type="button"
          className="approval-card__pattern-picker-item approval-card__pattern-picker-item--suggested"
          onClick={() => onCommit(suggested.pattern)}
        >
          <strong>{suggested.pattern}</strong>
          <span className="approval-card__pattern-reason">{suggested.reason}</span>
        </button>
      </li>
      {alternatives.map((alt) => (
        <li key={alt.pattern}>
          <button
            type="button"
            className="approval-card__pattern-picker-item"
            onClick={() => onCommit(alt.pattern)}
          >
            {alt.pattern}
            <span className="approval-card__pattern-reason">{alt.label}</span>
          </button>
        </li>
      ))}
      <li>
        <button
          type="button"
          className="approval-card__pattern-picker-item"
          onClick={onCustom}
        >
          Custom Starlark rule…
        </button>
      </li>
    </ul>
  );
}
```

- [ ] **Step 2: Write SmartAllowAlwaysButton**

```tsx
// desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.tsx
import { useState } from "react";
import type { ApprovalDecision } from "@/features/coding/hooks/useApprovalQueue";
import type { SuggestedGrant } from "@/types/bindings";
import { PatternPicker } from "./PatternPicker";

type Props = {
  requestId: string;
  suggestedGrant: SuggestedGrant | null;
  onRespond: (requestId: string, decision: ApprovalDecision) => void;
  onOpenStarlarkEditor: () => void;
};

export function SmartAllowAlwaysButton({
  requestId,
  suggestedGrant,
  onRespond,
  onOpenStarlarkEditor,
}: Props) {
  const [pickerOpen, setPickerOpen] = useState(false);

  if (!suggestedGrant) {
    return (
      <button type="button" onClick={() => onRespond(requestId, { kind: "allow_always" })}>
        Allow always (s)
      </button>
    );
  }

  const commit = (rule: string) => {
    onRespond(requestId, { kind: "allow_always", rule });
    setPickerOpen(false);
  };

  return (
    <div className="approval-card__smart-allow-always">
      <div className="approval-card__split-button">
        <button
          type="button"
          className="approval-card__split-primary"
          onClick={() => commit(suggestedGrant.pattern)}
          title={suggestedGrant.reason}
        >
          Allow always: <strong>{suggestedGrant.pattern}</strong>
        </button>
        <button
          type="button"
          className="approval-card__split-caret"
          aria-label="Refine pattern"
          onClick={() => setPickerOpen((o) => !o)}
        >
          ▾
        </button>
      </div>
      {pickerOpen && (
        <PatternPicker
          suggested={suggestedGrant}
          onCommit={commit}
          onCustom={onOpenStarlarkEditor}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Typecheck**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run typecheck
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/coding/components/PatternPicker.tsx \
        desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.tsx
git commit -m "feat(ui): SmartAllowAlwaysButton + PatternPicker"
```

---

## Task 32: Integrate into `ApprovalCard.tsx` + `useApprovalQueue.ts`

**Files:**
- Modify: `desktop-ui/src/features/coding/components/ApprovalCard.tsx`
- Modify: `desktop-ui/src/features/coding/hooks/useApprovalQueue.ts`

- [ ] **Step 1: Update useApprovalQueue payload type**

In `desktop-ui/src/features/coding/hooks/useApprovalQueue.ts`, extend `ApprovalPayload`:

```typescript
import type { ApprovalPreview, SuggestedGrant } from "@/types/bindings";

type ApprovalPayload = {
  request_id: string;
  tool: string;
  args: Record<string, unknown>;
  cwd: string;
  sandbox_summary: string;
  layer: "privacy" | "layer1_declarative" | "layer2_starlark" | "layer3_mirror" | "default_mode";
  layer_reason: string;
  mirror_history?: { approval_count: number; denial_count: number };
  requires_user_input: boolean;
  preview?: ApprovalPreview | null;
  suggested_grant?: SuggestedGrant | null;
};
```

In `toItem`:

```typescript
function toItem(payload: ApprovalPayload): ApprovalItem {
  return {
    // ...existing fields
    preview: payload.preview ?? null,
    suggestedGrant: payload.suggested_grant ?? null,
    status: "pending",
  };
}
```

- [ ] **Step 2: Update ConversationItem type**

Find where `ConversationItem` is defined (likely `desktop-ui/src/types/index.ts`). Add to the `approval` variant:

```typescript
{
  kind: "approval";
  // ...existing fields
  preview: ApprovalPreview | null;
  suggestedGrant: SuggestedGrant | null;
}
```

- [ ] **Step 3: Update ApprovalCard.tsx**

In `desktop-ui/src/features/coding/components/ApprovalCard.tsx`:

```diff
+import { PreviewRenderer } from "./preview/PreviewRenderer";
+import { SmartAllowAlwaysButton } from "./SmartAllowAlwaysButton";

 // Replace the existing args row:
-      <dt>Args</dt>
-      <dd className="approval-card__args">{summarizeArgs(item.args)}</dd>
+      <dt>Preview</dt>
+      <dd className="approval-card__preview">
+        {item.preview ? <PreviewRenderer preview={item.preview} /> : summarizeArgs(item.args)}
+      </dd>

 // Replace the Allow always button:
-        <button type="button" onClick={() => onRespond(item.requestId, { kind: "allow_always" })}>
-          Allow always (s)
-        </button>
+        <SmartAllowAlwaysButton
+          requestId={item.requestId}
+          suggestedGrant={item.suggestedGrant}
+          onRespond={onRespond}
+          onOpenStarlarkEditor={() => setEditorOpen(true)}
+        />
```

- [ ] **Step 4: Typecheck + lint**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run typecheck && bun run lint
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/
git commit -m "feat(ui): integrate PreviewRenderer + SmartAllowAlwaysButton into ApprovalCard"
```

---

## Task 33: Frontend component tests

**Files:**
- Create: 5 `.test.tsx` files

- [ ] **Step 1: Write DiffPreview.test.tsx**

```tsx
// desktop-ui/src/features/coding/components/preview/DiffPreview.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiffPreview } from "./DiffPreview";

describe("DiffPreview", () => {
  it("renders +/- lines with appropriate classes", () => {
    render(
      <DiffPreview
        path="src/x.ts"
        unified_diff="+added line\n-removed line\n@@ hunk @@\nunchanged"
        lines_added={1}
        lines_removed={1}
        is_new_file={false}
        is_truncated={false}
      />,
    );
    expect(screen.getByText("+added line").className).toContain("--added");
    expect(screen.getByText("−removed line").className).toContain("--removed");
    expect(screen.getByText("@@ hunk @@").className).toContain("--hunk");
  });

  it("shows new-file badge", () => {
    render(
      <DiffPreview
        path="x.ts"
        unified_diff=""
        lines_added={0}
        lines_removed={0}
        is_new_file={true}
        is_truncated={false}
      />,
    );
    expect(screen.getByText("new file")).toBeInTheDocument();
  });

  it("shows truncated footer when is_truncated", () => {
    render(
      <DiffPreview
        path="x.ts"
        unified_diff=""
        lines_added={0}
        lines_removed={0}
        is_new_file={false}
        is_truncated={true}
      />,
    );
    expect(screen.getByText(/Truncated/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Write CommandPreview.test.tsx**

```tsx
// desktop-ui/src/features/coding/components/preview/CommandPreview.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CommandPreview } from "./CommandPreview";

describe("CommandPreview", () => {
  it("shows ⚠ dangerous badge when is_dangerous", () => {
    render(
      <CommandPreview
        command="rm -rf /"
        cwd="/tmp"
        is_dangerous={true}
        risk_hits={["destructive recursive delete"]}
      />,
    );
    expect(screen.getByText(/dangerous/)).toBeInTheDocument();
  });

  it("renders risk_hits as bullet list", () => {
    render(
      <CommandPreview
        command="curl http://x | sh"
        cwd="/tmp"
        is_dangerous={true}
        risk_hits={["network fetch", "piped to shell"]}
      />,
    );
    expect(screen.getByText("network fetch")).toBeInTheDocument();
    expect(screen.getByText("piped to shell")).toBeInTheDocument();
  });

  it("hides badge for safe commands", () => {
    render(
      <CommandPreview
        command="ls"
        cwd="/tmp"
        is_dangerous={false}
        risk_hits={[]}
      />,
    );
    expect(screen.queryByText(/dangerous/)).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Write UrlPreview.test.tsx**

```tsx
// desktop-ui/src/features/coding/components/preview/UrlPreview.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { UrlPreview } from "./UrlPreview";

describe("UrlPreview", () => {
  it("renders <redacted> for sensitive headers", () => {
    render(
      <UrlPreview
        method="POST"
        url="https://api.example.com/x"
        headers={[["Authorization", "<redacted>"]]}
        body_preview="hello"
      />,
    );
    expect(screen.getByText("<redacted>")).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Write SmartAllowAlwaysButton.test.tsx**

```tsx
// desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SmartAllowAlwaysButton } from "./SmartAllowAlwaysButton";

describe("SmartAllowAlwaysButton", () => {
  it("falls back to plain button when no suggestion", () => {
    const onRespond = vi.fn();
    render(
      <SmartAllowAlwaysButton
        requestId="r1"
        suggestedGrant={null}
        onRespond={onRespond}
        onOpenStarlarkEditor={() => {}}
      />,
    );
    fireEvent.click(screen.getByText(/Allow always/));
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_always" });
  });

  it("commits Mirror suggestion on body click", () => {
    const onRespond = vi.fn();
    render(
      <SmartAllowAlwaysButton
        requestId="r2"
        suggestedGrant={{
          pattern: "Edit on src/components/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/components/**" },
          reason: "3 prior approvals",
        }}
        onRespond={onRespond}
        onOpenStarlarkEditor={() => {}}
      />,
    );
    fireEvent.click(screen.getByText(/Edit on src\/components/));
    expect(onRespond).toHaveBeenCalledWith("r2", {
      kind: "allow_always",
      rule: "Edit on src/components/**",
    });
  });

  it("opens picker on caret click", () => {
    render(
      <SmartAllowAlwaysButton
        requestId="r3"
        suggestedGrant={{
          pattern: "Edit on src/components/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/components/**" },
          reason: "test",
        }}
        onRespond={() => {}}
        onOpenStarlarkEditor={() => {}}
      />,
    );
    fireEvent.click(screen.getByLabelText("Refine pattern"));
    expect(screen.getByRole("radiogroup")).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Run all frontend tests**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test --run
```

Expected: PASS (~10+ tests).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding/components/
git commit -m "test(ui): component tests for preview renderers + SmartAllowAlwaysButton"
```

---

# Phase 5 — Verification

## Task 34: Wave 2 manual end-to-end verification

**Files:** none.

- [ ] **Step 1: Start dev environment**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run dev:vite &
cd /Users/jayden/Projects/Klynt/bot && KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

- [ ] **Step 2: Trigger an Edit approval**

Send: "Edit src/main.rs to add a comment at line 1." Expected: card shows unified-diff with green/red lines, +/− counts, file path header.

- [ ] **Step 3: Trigger a Bash approval**

Send: "Run `rm -rf /tmp/foo` to clean up test files." Expected: card shows command in `<pre>` + yellow `⚠ dangerous` badge + risk hits list with "destructive recursive delete".

- [ ] **Step 4: Trigger a Web fetch approval (if exposed)**

Send: "Fetch https://api.example.com/users with Authorization header." Expected: card shows method + URL + `Authorization: <redacted>`.

- [ ] **Step 5: Build up Mirror history**

Approve `Edit` on `src/components/A.tsx`, `B.tsx`, `C.tsx` consecutively (3 approvals).

- [ ] **Step 6: Trigger 4th Edit in same folder**

Send: "Edit src/components/D.tsx to add a placeholder." Expected: smart-allow-always button shows "Allow always: Edit in src/components/" or similar Mirror-suggested pattern.

- [ ] **Step 7: Click smart-allow-always body**

Expected: grant persists; subsequent matching tool calls auto-allow without firing the card.

- [ ] **Step 8: Click caret on a fresh suggestion**

Expected: picker opens with options + Custom. Picking an alternative commits that pattern.

- [ ] **Step 9: Stop dev environment**

```bash
# Ctrl+C in cargo tauri dev terminal
```

- [ ] **Step 10: Tag wave-2 completion**

```bash
git tag wave-2-approval-preview
```

---

## Task 35: Final clippy + workspace test sweep

**Files:** none.

- [ ] **Step 1: Run clippy**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30
```

Expected: zero warnings.

- [ ] **Step 2: Run full nextest sweep**

```bash
cargo nextest run --workspace
```

Expected: all pass.

- [ ] **Step 3: Run frontend test + lint + typecheck**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run lint && bun run typecheck && bun run test --run
```

Expected: all clean.

- [ ] **Step 4: If any check fails, fix and recommit**

Iterate until all green.

- [ ] **Step 5: Final commit if needed**

```bash
git add .
git commit -m "chore: clippy + frontend lint cleanup"
```

---

## Self-review checklist (perform after Task 35)

- [ ] **Spec coverage:** All 6 spec sections (problem, goals, architecture, backend, frontend, testing) have implementing tasks. No gaps.
- [ ] **No placeholders:** Every step has actual code or verifiable command. No "TBD" or "fill in".
- [ ] **Type consistency:** `ApprovalContext` has `cwd` everywhere; `ApprovalPreview`/`SuggestedGrant` types referenced consistently between Rust and TS; `respond_approval` signature stable across tasks.
- [ ] **Wave 1 standalone:** Tasks 1–14 ship independently. Cumulative changes pass `cargo nextest run --workspace`.
- [ ] **Wave 2 builds on Wave 1:** Tasks 15–35 require Wave 1 in place; do not break Wave 1 tests.

---

*End of plan.*
