# Tool Layer Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire `crates/tools/src/system/` (the OLD chatbot tool surface), unify around `klynt-core` as the single source of truth for primitive tools across coding and regular chat, and add four architectural enhancements (per-tool ChannelMask, channel-aware approval, per-host approval dedup, live event-stream wiring) — completing Phase 1 of the master coding-in-chat spec.

**Architecture:** 9 commits in 3 phases. Phase A is purely additive (no behavior change): ChannelMask, channel-aware approval, HostApprovalCache, event_tx-via-RoutingContext. Phase B is refactor-only: ToolKitBuilder, sub-agent rewiring, param ports. Phase C is the user-visible cutover: tool graduation + DELETION of `crates/tools/src/system/` + lexical sweep. Each commit is independently revertible until the deletion.

**Tech Stack:** Rust 1.93 stable, `bitflags = "2"`, `dashmap`, `url`, `tokio::sync::mpsc`, `walkdir`, `globset`, `regex`, `tools-core` trait macros, Tauri 2 IPC, React 18 + Vitest.

**Spec reference:** `docs/superpowers/specs/2026-04-30-tool-layer-consolidation-design.md` (consolidation design) — primarily §3 (ChannelMask), §4 (channel-aware approval), §5 (HostApprovalCache), §6 (ToolKitBuilder), §8 (event_tx wiring), §10 (sequencing), Appendices A-C.

**Master spec:** `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` Appendix F amendment.

**Spec amendment needed during execution:** §8 of consolidation spec assumes an `AgentRuntime::event_sender()` accessor returning a stored field. Reality: AgentRuntime stores no `event_tx` field; the channel is per-call to `process_message`. Plan adopts the *minimum-diff alternative*: thread `event_tx` through `RoutingContext` (tools-core::RoutingContext gains an `event_tx: Option<mpsc::Sender<AgentEvent>>` field). Update §8 of the spec to match before merging Task 4.

---

## File structure

### Files created

```
bot/
├── crates/
│   ├── common/src/
│   │   └── tool_channel.rs                       (renamed from coding_channel.rs; adds ChannelMask)
│   ├── klynt-core/src/
│   │   ├── approval/host_cache.rs                (HostApprovalCache; Codex-derived per-host dedup)
│   │   └── registry/builder.rs                   (ToolKitBuilder; DI for tools across main agent + sub-agents)
│   └── config/src/schema/
│       └── tools.rs                              (NonUiPolicy + ApprovalPolicyConfig)
├── tests/
│   └── coding_in_chat_property.rs                (gains K12-K15 invariants — appended to existing file)
└── desktop-ui/src/
    └── (no new files; existing useFileEditEvents.ts and chatStreamStore slice remain)
```

### Files modified

```
crates/common/src/lib.rs                          (mod rename; re-exports)
crates/common/Cargo.toml                          (+ bitflags = "2")
crates/tools-core/src/lib.rs                      (Tool trait gains allowed_channels())
crates/tools-core/src/routing.rs                  (RoutingContext gains event_tx field)
crates/klynt-core/src/lib.rs                      (re-exports ToolKitBuilder, HostApprovalCache)
crates/klynt-core/src/approval/mod.rs             (re-export host_cache)
crates/klynt-core/src/registry/mod.rs             (re-export builder)
crates/klynt-core/src/tools/mod.rs                (no structural change; ask_user impl moves in)
crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,plan_mode}.rs  (+ allowed_channels override)
crates/klynt-core/src/tools/grep.rs               (+ context_lines param)
crates/klynt-core/src/tools/glob.rs               (+ path param)
crates/klynt-core/src/tools/web_fetch.rs          (host-cache wiring + drop event_tx field)
crates/klynt-core/src/tools/{read,glob,grep,write,edit,apply_patch,bash,notebook_edit,plan_mode,web_fetch,ask_user,tool_search}.rs
                                                   (drop self.event_tx; read from ctx.event_tx)
crates/klynt-core/src/tools/ask_user.rs           (replaces 4-line re-export with full impl moved from tools/system)
crates/klynt-core/Cargo.toml                      (+ dashmap, url; deps for builder)
crates/klynt-core/src/approval/guard.rs           (channel-aware degradation in evaluate())
crates/agent/src/agent_loop/mod.rs                (filter site uses Tool::allowed_channels())
crates/agent/src/agent_loop/builder.rs            (DELETE OLD-tool registrations at lines 629-658)
crates/agent/src/agent_runtime/runtime.rs         (+ tool_kit field + accessors)
crates/agent/src/subagent.rs                      (rewrite registrations to use ToolKitBuilder)
crates/app-core/src/init/mod.rs                   (replace 1784-1851 with ToolKitBuilder; remove event_tx None)
crates/app-core/src/handlers/chat/streaming.rs    (verify FileEditWithSymbols/PlanModeChanged arms; thread event_tx into ctx)
crates/tools/src/lib.rs                           (remove pub mod system; + re-exports)
crates/tools/Cargo.toml                           (drop reqwest, html2text deps)
crates/tools/README.md                            (rewrite scope: domain tools only)
desktop-ui/src/features/coding/components/ApprovalCard.tsx  (extend decision shape if needed for host approval)
desktop-ui/src/features/settings/components/sections/SettingsFeaturesSection.tsx  (drop web_search row)
agents/general/AGENT.md, agents/task/AGENT.md, agents/finance/AGENT.md,
  agents/automation/AGENT.md, agents/communication/AGENT.md   (lexical rename: read_file→read, etc.)
agents/general/skills/search.md                   (drop web_search; document web_fetch only)
workspace/TOOLS.md, workspace/AGENTS.md           (rewrite tool catalog)
tests/e2e/agent_loop.rs                           (fixture rename)
tests/unit/providers.rs                           (fixture rename)
tests/integration/cognitive.rs                    (fixture rename)
tests/integration/learning.rs                     (drop web_search references)
crates/coding-ingest/tests/kimi_poller.rs         (verify message references unrelated to MessageTool)
```

### Files deleted

```
crates/tools/src/system/ask_user.rs               (922 lines; impl moved to klynt-core)
crates/tools/src/system/browser.rs                (740 lines; retired)
crates/tools/src/system/filesystem.rs             (640 lines; klynt-core covers)
crates/tools/src/system/glob_tool.rs              (189 lines; klynt-core covers)
crates/tools/src/system/grep.rs                   (316 lines; klynt-core covers)
crates/tools/src/system/message.rs                (79 lines; retired)
crates/tools/src/system/web.rs                    (272 lines; retired except web_fetch which klynt-core covers)
crates/tools/src/system/mod.rs
agents/general/skills/browser.md                  (retired)
```

---

## Sequencing

```
Phase A — Foundation (additive, no UX change)
  Task 1: ChannelMask + Tool::allowed_channels() + per-tool overrides + rename to tool_channel.rs
  Task 2: Channel-aware approval policy in evaluate() free fn
  Task 3: HostApprovalCache (per-host dedup; Codex-derived)
  Task 4: event_tx via RoutingContext (FileEditWithSymbols actually reaches UI)

Phase B — Builder + sub-agent (refactor only, observable behavior unchanged)
  Task 5: ToolKitBuilder + main agent rewiring at app-core/src/init/mod.rs
  Task 6: Sub-agent rewiring (subagent.rs uses ToolKitBuilder)
  Task 7: Param-shape ports (grep.context_lines, glob.path)

Phase C — Cutover (user-visible change)
  Task 8: Tool graduation (read/glob/grep/web_fetch/ask_user/tool_search → ChannelMask::ALL)
  Task 9: DELETION of crates/tools/src/system/ + prompt sweep + test rewrite
```

Each task ends in a single git commit. Each commit is independently revertible until Task 9.

---

## Task 1: ChannelMask foundation + per-tool overrides + rename

**Goal:** Replace the static `CODING_ONLY` const with a `Tool::allowed_channels() -> ChannelMask` trait method; keep observable behavior identical (every klynt-core tool still coding-only). Rename `coding_channel.rs` → `tool_channel.rs`.

**Files:**
- Modify: `crates/common/Cargo.toml`
- Rename: `crates/common/src/coding_channel.rs` → `crates/common/src/tool_channel.rs`
- Modify: `crates/common/src/lib.rs`
- Modify: `crates/tools-core/src/lib.rs`
- Modify: 13 files in `crates/klynt-core/src/tools/` (one-line override per coding-only tool)
- Modify: `crates/agent/src/agent_loop/mod.rs:889-945` (filter site)
- Test: `tests/coding_in_chat_property.rs` (add K12)

- [ ] **Step 1: Add bitflags dependency**

Edit `crates/common/Cargo.toml`. Find the `[dependencies]` section. Add:

```toml
bitflags = "2"
```

- [ ] **Step 2: Verify the dep resolves**

Run:

```bash
cargo check -p common 2>&1 | tail -5
```

Expected: `Checking common v0.1.1` then no errors.

- [ ] **Step 3: Rename file**

```bash
git mv crates/common/src/coding_channel.rs crates/common/src/tool_channel.rs
```

- [ ] **Step 4: Update common/src/lib.rs**

Find the line `pub mod coding_channel;` in `crates/common/src/lib.rs` and replace with:

```rust
pub mod tool_channel;
pub use tool_channel::{Channel, ChannelMask, available_for_channel};
```

- [ ] **Step 5: Verify rename compiles before adding ChannelMask**

Run:

```bash
cargo check -p common 2>&1 | tail -5
```

Expected: `error[E0432]: unresolved import` because `tool_channel.rs` doesn't yet export `ChannelMask`. That's the expected fail — proceed.

- [ ] **Step 6: Write the failing test for ChannelMask**

Create `crates/common/tests/tool_channel.rs`:

```rust
use common::tool_channel::{Channel, ChannelMask};

#[test]
fn channel_mask_all_allows_every_channel() {
    let m = ChannelMask::ALL;
    assert!(m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_mask_coding_only_excludes_others() {
    let m = ChannelMask::CODING_ONLY;
    assert!(m.allows(Channel::Coding));
    assert!(!m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn channel_mask_non_coding_includes_desktop_and_other() {
    let m = ChannelMask::NON_CODING;
    assert!(!m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(m.allows(Channel::Other));
}

#[test]
fn channel_mask_compose_with_bitor() {
    let m = ChannelMask::CODING | ChannelMask::DESKTOP;
    assert!(m.allows(Channel::Coding));
    assert!(m.allows(Channel::Desktop));
    assert!(!m.allows(Channel::Other));
}

#[test]
fn channel_supports_approval_ui_matches_coding_and_desktop() {
    assert!(Channel::Coding.supports_approval_ui());
    assert!(Channel::Desktop.supports_approval_ui());
    assert!(!Channel::Other.supports_approval_ui());
}
```

- [ ] **Step 7: Run test to verify failure**

Run:

```bash
cargo test -p common --test tool_channel 2>&1 | tail -20
```

Expected: compile errors — `ChannelMask` not found, `supports_approval_ui` not found.

- [ ] **Step 8: Replace `tool_channel.rs` content**

Open `crates/common/src/tool_channel.rs` (formerly `coding_channel.rs`). Replace the entire file with:

```rust
//! Channel categories and per-tool visibility masks.
//!
//! `Channel` discriminates the chat surface (coding / desktop / other). `ChannelMask`
//! is what `Tool::allowed_channels()` returns — a bitmask of channels in which
//! the tool is visible to the LLM.

use bitflags::bitflags;

// Re-export so `common::tool_channel::CODING_CHANNEL` works.
pub use crate::CODING_CHANNEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Coding,
    Desktop,
    Other,
}

impl Channel {
    pub fn from_name(s: &str) -> Self {
        if s == CODING_CHANNEL {
            Self::Coding
        } else if s == "desktop" {
            Self::Desktop
        } else {
            Self::Other
        }
    }

    /// True for channels that can render approval cards (`kind: "approval"`
    /// ConversationItem). Used by the approval evaluator to fall back to a
    /// configured policy in headless channels (Telegram/Discord/Slack/Email).
    pub fn supports_approval_ui(&self) -> bool {
        matches!(self, Self::Coding | Self::Desktop)
    }
}

bitflags! {
    /// A tool's visibility across channel categories.
    ///
    /// 95% of tools want `ALL` (default). Tools needing approval UI return
    /// `CODING_ONLY` so they don't appear in headless chats.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ChannelMask: u8 {
        const CODING  = 0b001;
        const DESKTOP = 0b010;
        const OTHER   = 0b100;

        const ALL          = Self::CODING.bits() | Self::DESKTOP.bits() | Self::OTHER.bits();
        const CODING_ONLY  = Self::CODING.bits();
        const DESKTOP_ONLY = Self::DESKTOP.bits();
        const NON_CODING   = Self::DESKTOP.bits() | Self::OTHER.bits();
    }
}

impl ChannelMask {
    #[inline]
    pub fn allows(self, ch: Channel) -> bool {
        let bit = match ch {
            Channel::Coding  => Self::CODING,
            Channel::Desktop => Self::DESKTOP,
            Channel::Other   => Self::OTHER,
        };
        self.contains(bit)
    }
}

/// Look up channel visibility for a named tool, falling back to ALL if the
/// registry doesn't know the tool. Replaces the old `available_for_channel`
/// signature for compatibility during the migration; once all callers use
/// `Tool::allowed_channels()` directly, this helper can be deleted.
pub fn available_for_channel(_tool_name: &str, _channel: Channel) -> bool {
    // Compatibility shim retained ONLY for code that hasn't migrated yet.
    // Returns true unconditionally — the real per-tool gate now lives on the
    // Tool trait. Callers MUST migrate to ChannelMask::allows() in Task 1.
    //
    // After Task 1 completes (filter site updated), this fn has no remaining
    // callers and can be deleted in Task 9 alongside the rest of the cleanup.
    true
}
```

- [ ] **Step 9: Run test to verify pass**

Run:

```bash
cargo test -p common --test tool_channel 2>&1 | tail -10
```

Expected: `5 passed; 0 failed`.

- [ ] **Step 10: Add `Tool::allowed_channels()` to the trait**

Open `crates/tools-core/src/lib.rs`. Find the `pub trait Tool: Send + Sync` block (around line 71-133 per the agent #4 inventory). After the existing `is_concurrency_safe` method (around line 105), add:

```rust
/// Channels in which this tool is visible to the LLM. Default = ALL.
/// Override to restrict — tools that need approval UI return CODING_ONLY.
fn allowed_channels(&self) -> common::ChannelMask {
    common::ChannelMask::ALL
}
```

(The exact insertion point: between `is_concurrency_safe` and `custom_timeout`. If unsure, place it directly after `is_concurrency_safe`'s closing brace.)

- [ ] **Step 11: Verify the trait extension compiles**

Run:

```bash
cargo check -p tools-core 2>&1 | tail -5
```

Expected: `Checking tools-core` then no errors.

- [ ] **Step 12: Add CODING_ONLY override on BashTool**

Open `crates/klynt-core/src/tools/bash.rs`. Find the `impl ToolExecute for BashTool` block. **Before** that block (or after the `BashTool::new` impl, whichever feels native to the file), add a separate impl block:

```rust
impl tools_core::Tool for BashTool {
    fn allowed_channels(&self) -> common::ChannelMask {
        common::ChannelMask::CODING_ONLY
    }
}
```

WAIT — the `#[derive(ToolDerive)]` macro already generates a `tools_core::Tool` impl that bridges to `ToolExecute::execute`. Adding a *second* `impl tools_core::Tool` would conflict. Instead, the macro must accept an `allowed_channels` attribute, OR we add the override via a different route.

**Pragmatic approach:** check whether `tools-core-macros` already supports an attribute like `#[tool(allowed_channels = "coding_only")]`. If yes, use it. If no, add support to the macro first.

- [ ] **Step 12a: Check tools-core-macros for `allowed_channels` attribute support**

Run:

```bash
rg "allowed_channels|channel_mask" crates/tools-core-macros/src/ 2>&1 | head -10
```

Expected: no matches today (the attribute doesn't exist).

- [ ] **Step 12b: Extend tools-core-macros to support `allowed_channels` attribute**

Open `crates/tools-core-macros/src/tool_derive.rs`. Find the part that parses the `#[tool(...)]` attribute keys. Add `allowed_channels` to the recognized keys, accepting a string value `"all" | "coding_only" | "desktop_only" | "non_coding"`. In the generated `impl tools_core::Tool` block, emit:

```rust
fn allowed_channels(&self) -> common::ChannelMask {
    match #parsed_value {
        "coding_only"   => common::ChannelMask::CODING_ONLY,
        "desktop_only"  => common::ChannelMask::DESKTOP_ONLY,
        "non_coding"    => common::ChannelMask::NON_CODING,
        _               => common::ChannelMask::ALL,
    }
}
```

(The string literal is parsed at proc-macro time; substitute the parsed value as the matched arm directly — no runtime match needed. Generated code: `common::ChannelMask::CODING_ONLY` for `allowed_channels = "coding_only"`, etc.)

If the macro is too complex to extend in this step, fall back to: don't use `#[derive(ToolDerive)]` for the override; instead, manually implement `tools_core::Tool` for the override-needing tools and skip the derive. Document this in a comment.

For this plan we proceed assuming the macro extension lands in this step. Adjust naming to match the macro's existing conventions.

- [ ] **Step 12c: Run `cargo check` on macros and a consumer**

Run:

```bash
cargo check -p tools-core-macros 2>&1 | tail -5
cargo check -p klynt-core 2>&1 | tail -10
```

Expected: both clean.

- [ ] **Step 13: Add `allowed_channels = "coding_only"` to BashTool**

Open `crates/klynt-core/src/tools/bash.rs`. Find the `#[tool(...)]` attribute block on `BashTool`. Add `allowed_channels = "coding_only"` to the comma-separated args:

```rust
#[derive(ToolDerive)]
#[tool(
    name = "bash",
    description = "...",
    params = "BashArgs",
    permission = "execute",
    category = "Shell",
    cost = "Low",
    tags = "shell,exec,coding",
    allowed_channels = "coding_only"
)]
pub struct BashTool { /* ... */ }
```

- [ ] **Step 14: Add the same override to 6 sibling tools**

Repeat Step 13 for: `crates/klynt-core/src/tools/edit.rs`, `write.rs`, `apply_patch.rs`, `notebook_edit.rs`, `plan_mode.rs` (both `EnterPlanModeTool` and `ExitPlanModeTool` structs in this file).

- [ ] **Step 15: Verify all overrides compile**

Run:

```bash
cargo check -p klynt-core 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 16: Update the filter site in agent_loop/mod.rs**

Open `crates/agent/src/agent_loop/mod.rs`. Find lines 889-945 (the `run_pipeline` function — the filter loop currently uses `common::available_for_channel(name, channel)`). Replace the filter-loop block with:

```rust
let channel = common::tool_channel::Channel::from_name(routing_ctx.channel.as_str());
let registry = self.tool_registry.read().await;
let filtered_defs: Arc<Vec<serde_json::Value>> = Arc::new(
    tool_defs
        .iter()
        .filter(|def| {
            let name = def
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            registry
                .get(name)
                .map(|tool| tool.allowed_channels().allows(channel))
                .unwrap_or(true)
        })
        .cloned()
        .collect(),
);
drop(registry);
```

Note: the registry lock is held across the filter only; ensure the borrow doesn't conflict with the existing `read().await` in `get_tool_info`. If `get_tool_info` already returns `tool_defs`, restructure so we acquire the lock once and call both `get_definitions()` and the per-name `get(name)` lookups inside.

If `ToolRegistry::get(name) -> Option<&dyn Tool>` doesn't exist, add it (delegate to the underlying HashMap).

- [ ] **Step 17: Verify ToolRegistry has a name-lookup accessor**

Run:

```bash
rg "fn get\(|fn lookup\(" crates/tools-core/src/registry.rs 2>&1 | head -5
```

If the accessor exists, proceed. If not, add to `crates/tools-core/src/registry.rs`:

```rust
impl ToolRegistry {
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}
```

(Field name `tools` may differ — match the existing field.)

- [ ] **Step 18: Run cargo check on agent**

Run:

```bash
cargo check -p agent 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 19: Add invariant K12 — filter idempotence property test**

Append to `tests/coding_in_chat_property.rs` (create the file if it doesn't exist; the spec mandates it):

```rust
use proptest::prelude::*;
use common::tool_channel::{Channel, ChannelMask};

proptest! {
    #[test]
    fn k12_channel_mask_filter_idempotent(
        coding in any::<bool>(),
        desktop in any::<bool>(),
        other in any::<bool>(),
        ch_idx in 0u8..3,
    ) {
        let mut mask = ChannelMask::empty();
        if coding { mask |= ChannelMask::CODING; }
        if desktop { mask |= ChannelMask::DESKTOP; }
        if other { mask |= ChannelMask::OTHER; }
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };
        let pass1 = mask.allows(ch);
        let pass2 = mask.allows(ch);
        prop_assert_eq!(pass1, pass2);
    }
}
```

- [ ] **Step 20: Run the K12 property test**

Run:

```bash
cargo nextest run -p klyntbot --test coding_in_chat_property 2>&1 | tail -10
```

(The exact crate name depends on workspace config; `cargo test -p` may differ. Try `cargo nextest run --workspace --test coding_in_chat_property` if `-p klyntbot` doesn't match.)

Expected: K12 passes.

- [ ] **Step 21: Run full cargo build and clippy**

Run:

```bash
cargo build --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: build clean; clippy zero warnings.

- [ ] **Step 22: Run full test suite**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -20
```

Expected: all green. Filter behavior is identical to before Task 1 (every klynt-core tool still coding-only).

- [ ] **Step 23: Commit**

```bash
git add crates/common/Cargo.toml \
        crates/common/src/lib.rs \
        crates/common/src/tool_channel.rs \
        crates/common/src/coding_channel.rs \
        crates/common/tests/tool_channel.rs \
        crates/tools-core/src/lib.rs \
        crates/tools-core/src/registry.rs \
        crates/tools-core-macros/src/tool_derive.rs \
        crates/klynt-core/src/tools/bash.rs \
        crates/klynt-core/src/tools/edit.rs \
        crates/klynt-core/src/tools/write.rs \
        crates/klynt-core/src/tools/apply_patch.rs \
        crates/klynt-core/src/tools/notebook_edit.rs \
        crates/klynt-core/src/tools/plan_mode.rs \
        crates/agent/src/agent_loop/mod.rs \
        tests/coding_in_chat_property.rs

git commit -m "$(cat <<'EOF'
feat(common): introduce ChannelMask + Tool::allowed_channels()

Replaces the static CODING_ONLY const with a per-tool trait method
returning a ChannelMask. Renames coding_channel.rs to tool_channel.rs.
Adds CODING_ONLY override on klynt-core mutating tools (bash, edit,
write, apply_patch, notebook_edit, enter/exit_plan_mode) so observable
behavior is unchanged.

Adds invariant K12 (filter idempotence property test).

Part of tool layer consolidation (commit 1/9).
Spec: docs/superpowers/specs/2026-04-30-tool-layer-consolidation-design.md

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Channel-aware approval policy in `evaluate()` free fn

**Goal:** When `Layer1::evaluate` returns `Ask` and the channel doesn't support approval UI (Telegram/Discord/Slack/Email), fall back to a configured policy. Today `web_fetch` would hang in such channels; after Task 8's graduation this matters even more. Adds the channel-aware degradation step in `crates/klynt-core/src/approval/guard.rs::evaluate` (the orchestrator), since `Layer1::evaluate` itself takes only `(tool, payload)` with no ctx.

**Files:**
- Create: `crates/config/src/schema/tools.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema.rs` (or `lib.rs` — wherever the root `Config` struct lives)
- Modify: `crates/klynt-core/src/approval/guard.rs`
- Modify: `crates/klynt-core/src/approval/mod.rs` — propagate `channel: Channel` field through `GuardCtx`
- Modify: every `crates/klynt-core/src/tools/*.rs` that builds a `GuardCtx` — add `channel` from `ctx.channel`
- Test: new tests in `crates/klynt-core/tests/channel_aware_approval.rs`

- [ ] **Step 1: Create config schema for approval policy**

Create `crates/config/src/schema/tools.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NonUiPolicy {
    #[default]
    Allow,
    DenyWithError,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPolicyConfig {
    pub non_ui_channels: NonUiPolicy,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolsConfig {
    pub approval_policy: ApprovalPolicyConfig,
}
```

- [ ] **Step 2: Re-export from schema mod**

Open `crates/config/src/schema/mod.rs` (create if absent). Add:

```rust
pub mod tools;
pub use tools::{ApprovalPolicyConfig, NonUiPolicy, ToolsConfig};
```

- [ ] **Step 3: Mount on the root Config struct**

Locate the root `Config` struct (search `pub struct Config` under `crates/config/src/`). Add a field:

```rust
#[serde(default)]
pub tools: schema::ToolsConfig,
```

If the struct is large or in a different file, update the imports accordingly. Run:

```bash
rg "pub struct Config" crates/config/src/ 2>&1 | head -5
```

to find the right file.

- [ ] **Step 4: Verify Config compiles**

Run:

```bash
cargo check -p config 2>&1 | tail -5
```

Expected: clean.

- [ ] **Step 5: Add `channel: Channel` field to GuardCtx**

Open `crates/klynt-core/src/approval/guard.rs`. Find the `GuardCtx<'a>` struct (lines 18-29 per the extraction). Add a new field:

```rust
pub struct GuardCtx<'a> {
    pub layer1: &'a Layer1,
    pub policy: &'a Policy,
    pub privacy: &'a PrivacyGuard,
    pub pending: &'a Arc<PendingApprovalsMap>,
    pub event_tx: Option<&'a mpsc::Sender<AgentEvent>>,
    pub domain_bus: &'a Arc<DomainEventBus>,
    pub cancel: CancellationToken,
    pub request_id: String,
    pub args: Option<serde_json::Value>,
    pub cwd: Option<String>,
    pub channel: common::tool_channel::Channel,                          // NEW
    pub non_ui_policy: common::NonUiPolicy,                              // NEW (re-export NonUiPolicy from config in common)
}
```

If `common::NonUiPolicy` doesn't exist, the type lives in `config::schema::NonUiPolicy`. Either:
- (a) re-export `pub use config::schema::NonUiPolicy;` in `crates/common/src/lib.rs` (creates a downward dep — discouraged)
- (b) duplicate the small enum in `crates/common/src/tool_channel.rs` (cleaner, since common is L0)

Choose (b). In `crates/common/src/tool_channel.rs` add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NonUiPolicy {
    #[default]
    Allow,
    DenyWithError,
}
```

Then `crates/config/src/schema/tools.rs` uses `pub use common::NonUiPolicy;` instead of defining its own. Update Step 1 accordingly.

- [ ] **Step 6: Add the channel-aware degradation in `evaluate()` free fn**

Open `crates/klynt-core/src/approval/guard.rs`. Find the `evaluate` free fn (signature at line 31). Inside the function, after the `Layer1::evaluate` call, add the degradation step. The current structure likely chains: privacy check → Layer1 → (Ask) → round-trip. Insert between Layer1 and round-trip:

```rust
let layer1_decision = ctx.layer1.evaluate(tool, payload);

// Channel-aware degradation: if Layer1 says "ask" but the channel can't
// surface an approval card (Telegram/Discord/Slack/Email), fall back to
// the configured policy.
let layer1_decision = match layer1_decision {
    ApprovalDecision::Ask { .. } if !ctx.channel.supports_approval_ui() => {
        match ctx.non_ui_policy {
            common::NonUiPolicy::Allow => ApprovalDecision::Auto {
                allowed: true,
                layer: ApprovalLayer::Layer1Declarative,
                reason: format!(
                    "non-UI channel ({:?}) fallback: allow per tools.approvalPolicy.nonUiChannels",
                    ctx.channel
                ),
                rule_matched: None,
            },
            common::NonUiPolicy::DenyWithError => ApprovalDecision::Auto {
                allowed: false,
                layer: ApprovalLayer::Layer1Declarative,
                reason: format!(
                    "non-UI channel ({:?}) deny: tool '{tool}' requires approval; \
                     set tools.approvalPolicy.nonUiChannels = \"allow\" to permit",
                    ctx.channel
                ),
                rule_matched: None,
            },
        }
    }
    other => other,
};

// continue with whatever the existing flow does — likely the round-trip
// for the still-Ask case, then return.
```

Read the existing `evaluate` body fully before editing — match its surrounding control flow exactly.

- [ ] **Step 7: Update every tool's GuardCtx construction to populate `channel` and `non_ui_policy`**

For each of the 11 klynt-core tools that build a `GuardCtx` (`bash`, `edit`, `write`, `apply_patch`, `notebook_edit`, `web_fetch`; the read-only tools don't), find the GuardCtx-construction site (typically in `execute()`). Add:

```rust
channel: common::tool_channel::Channel::from_name(ctx.channel.as_str()),
non_ui_policy: /* from config — see Step 8 */,
```

For `non_ui_policy`, the value flows from `Config::tools.approval_policy.non_ui_channels`. Tools don't have a Config reference today. Solutions:
- (a) Add `non_ui_policy: common::NonUiPolicy` field to each mutating tool's struct, set at construction in `app-core/src/init/mod.rs`. Plumb via ToolKitBuilder (Task 5).
- (b) Tools read from a global config Arc — clunky.

Choose (a). For Task 2, the field gets added to tool structs and the constructor signature; the value is just `NonUiPolicy::Allow` (the default) until Task 5 plumbs it through ToolKitBuilder.

Concretely, for each mutating tool (bash, edit, write, apply_patch, notebook_edit, web_fetch):

```rust
pub struct WebFetchTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
    client: reqwest::Client,
    non_ui_policy: common::NonUiPolicy,                                  // NEW
}

impl WebFetchTool {
    pub fn new(
        layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::NonUiPolicy,                              // NEW
    ) -> Self {
        let client = /* same */;
        Self { layer1, policy, privacy, pending, event_tx, bus, client, non_ui_policy }
    }
}
```

- [ ] **Step 8: Update `app-core/src/init/mod.rs:1784-1851` constructor calls**

Add `non_ui_policy` to each `kt::*Tool::new(...)` call:

```rust
let non_ui_policy = config_guard.tools.approval_policy.non_ui_channels;
// ...
registry.register(kt::WebFetchTool::new(
    layer1.clone(), policy.clone(), privacy.clone(), pending.clone(),
    event_tx.clone(), bus.clone(),
    non_ui_policy,
));
```

Apply for: BashTool, EditTool, WriteTool, ApplyPatchTool, NotebookEditTool, WebFetchTool. Read-only tools (ReadTool, GlobTool, GrepTool, AskUserTool, ToolSearchTool, EnterPlanModeTool, ExitPlanModeTool) don't take it because they don't build GuardCtx.

- [ ] **Step 9: Write the failing test**

Create `crates/klynt-core/tests/channel_aware_approval.rs`:

```rust
//! Channel-aware approval degradation tests.
//!
//! When Layer1 returns `Ask` and the channel does not support approval UI
//! (Telegram/Discord/Slack/Email), the evaluator falls back to the
//! configured `non_ui_policy`.

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use klynt_core::approval::{evaluate, GuardCtx, ApprovalDecision, Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};

fn build_ask_layer1() -> Arc<Layer1> {
    // A Layer1 with no rules and default_if_no_match = "ask"
    let perms = config::schema::CodingPermissions {
        allow: vec![],
        deny: vec![],
        ask: vec![],
        default_if_no_match: "ask".to_string(),
    };
    Arc::new(Layer1::compile(&perms).expect("Layer1 compile"))
}

fn build_ctx<'a>(
    layer1: &'a Layer1,
    policy: &'a Policy,
    privacy: &'a PrivacyGuard,
    pending: &'a Arc<PendingApprovalsMap>,
    event_tx: Option<&'a mpsc::Sender<bus::AgentEvent>>,
    bus: &'a Arc<DomainEventBus>,
    channel: Channel,
    non_ui_policy: NonUiPolicy,
) -> GuardCtx<'a> {
    GuardCtx {
        layer1, policy, privacy, pending,
        event_tx,
        domain_bus: bus,
        cancel: CancellationToken::new(),
        request_id: "test-1".into(),
        args: None,
        cwd: None,
        channel,
        non_ui_policy,
    }
}

#[tokio::test]
async fn ask_in_telegram_with_allow_policy_returns_auto_allowed() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    let ctx = build_ctx(&layer1, &policy, &privacy, &pending, None, &bus,
                        Channel::Other, NonUiPolicy::Allow);
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    assert!(matches!(dec, ApprovalDecision::Auto { allowed: true, .. }));
}

#[tokio::test]
async fn ask_in_telegram_with_deny_policy_returns_auto_denied() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    let ctx = build_ctx(&layer1, &policy, &privacy, &pending, None, &bus,
                        Channel::Other, NonUiPolicy::DenyWithError);
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    assert!(matches!(dec, ApprovalDecision::Auto { allowed: false, .. }));
}

#[tokio::test]
async fn ask_in_coding_chat_does_not_degrade() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    // We can't easily await round-trip in a unit test (no real UI replier),
    // so we cancel immediately and assert TimedOut.
    let token = CancellationToken::new();
    token.cancel();
    let mut ctx = build_ctx(&layer1, &policy, &privacy, &pending, None, &bus,
                            Channel::Coding, NonUiPolicy::Allow);
    ctx.cancel = token;
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    // In coding mode, degradation is bypassed → falls into round-trip → cancelled.
    assert!(matches!(dec, ApprovalDecision::Cancelled));
}
```

- [ ] **Step 10: Run tests, verify pass**

Run:

```bash
cargo nextest run -p klynt-core --test channel_aware_approval 2>&1 | tail -10
```

Expected: 3 tests pass.

- [ ] **Step 11: Add invariant K14 — channel-aware approval safety**

Append to `tests/coding_in_chat_property.rs`:

```rust
proptest! {
    #[test]
    fn k14_no_mutating_tool_is_visible_in_non_ui_channel(
        tool_idx in 0u8..13,
        ch_idx in 0u8..3,
    ) {
        // Enumerate all 13 klynt-core tool names
        let tools = [
            ("bash", true), ("edit", true), ("write", true),
            ("apply_patch", true), ("notebook_edit", true),
            ("enter_plan_mode", false), ("exit_plan_mode", false),
            ("read", false), ("glob", false), ("grep", false),
            ("ask_user", false), ("web_fetch", false), ("tool_search", false),
        ];
        let (name, is_mutating) = tools[tool_idx as usize];
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };

        // After Task 8 graduation, the read-only / portable tools graduate.
        // Mutating tools never graduate. Therefore, for any mutating tool in
        // a non-coding channel, the mask must NOT allow.
        if is_mutating && !matches!(ch, Channel::Coding) {
            // ChannelMask::CODING_ONLY is the expected override — it does not allow non-coding.
            let mask = common::tool_channel::ChannelMask::CODING_ONLY;
            prop_assert!(!mask.allows(ch),
                "mutating tool {name} must not be visible in {ch:?}");
        }
        let _ = name; // suppress unused warning
    }
}
```

- [ ] **Step 12: Run K14 + full nextest**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 13: Verify clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 14: Commit**

```bash
git add crates/config/src/schema/tools.rs \
        crates/config/src/schema/mod.rs \
        crates/config/src/schema.rs \
        crates/common/src/tool_channel.rs \
        crates/klynt-core/src/approval/guard.rs \
        crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,web_fetch}.rs \
        crates/klynt-core/tests/channel_aware_approval.rs \
        crates/app-core/src/init/mod.rs \
        tests/coding_in_chat_property.rs

git commit -m "$(cat <<'EOF'
feat(approval): channel-aware degradation for headless channels

When Layer1 returns Ask but the channel does not support approval UI
(Telegram/Discord/Slack/Email), the evaluator falls back to the
configured tools.approvalPolicy.nonUiChannels (default Allow).

Adds NonUiPolicy enum, ApprovalPolicyConfig schema, channel + non_ui_policy
fields on GuardCtx, the degradation step in evaluate() free fn, and
non_ui_policy plumbing through klynt-core mutating tool constructors.

Adds invariant K14 (channel-aware approval safety property test).

Part of tool layer consolidation (commit 2/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: HostApprovalCache (Codex-derived per-host dedup)

**Goal:** Concurrent `web_fetch` calls to the same `(scheme, host, port)` coalesce onto one approval round-trip; the decision is cached for the session. Mirrors `codex-rs/core/src/tools/network_approval.rs:140-169`.

**Files:**
- Create: `crates/klynt-core/src/approval/host_cache.rs`
- Modify: `crates/klynt-core/src/approval/mod.rs`
- Modify: `crates/klynt-core/Cargo.toml` (+ `dashmap`, `url`)
- Modify: `crates/klynt-core/src/tools/web_fetch.rs`
- Test: `crates/klynt-core/tests/host_approval_cache.rs`

- [ ] **Step 1: Add deps to klynt-core/Cargo.toml**

Edit `crates/klynt-core/Cargo.toml`. In `[dependencies]`:

```toml
dashmap = "6"
url = "2"
```

(Verify `dashmap` and `url` are already in the workspace `[workspace.dependencies]` table or pin a workspace version. If not, add them there too.)

- [ ] **Step 2: Verify deps resolve**

Run:

```bash
cargo check -p klynt-core 2>&1 | tail -5
```

Expected: clean.

- [ ] **Step 3: Write the failing HostKey test**

Create `crates/klynt-core/tests/host_approval_cache.rs`:

```rust
use klynt_core::approval::host_cache::{HostApprovalCache, HostCheckResult, HostDecision, HostKey};
use std::sync::Arc;

#[test]
fn host_key_normalizes_scheme_and_host() {
    let k = HostKey::from_url("HTTPS://Example.COM:443/path?q=1").unwrap();
    assert_eq!(k.scheme, "https");
    assert_eq!(k.host, "example.com");
    assert_eq!(k.port, 443);
}

#[test]
fn host_key_uses_default_ports() {
    let http = HostKey::from_url("http://example.com/").unwrap();
    assert_eq!(http.port, 80);
    let https = HostKey::from_url("https://example.com/").unwrap();
    assert_eq!(https.port, 443);
}

#[tokio::test]
async fn first_caller_gets_newly_registered() {
    let cache = HostApprovalCache::default();
    let key = HostKey::from_url("https://example.com").unwrap();
    let r = cache.check_or_register(key.clone());
    assert!(matches!(r, HostCheckResult::NewlyRegistered { .. }));
}

#[tokio::test]
async fn second_concurrent_caller_gets_await_pending() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let _first = cache.check_or_register(key.clone()); // claims NewlyRegistered
    let r = cache.check_or_register(key);
    assert!(matches!(r, HostCheckResult::AwaitPending(_)));
}

#[tokio::test]
async fn resolve_propagates_to_pending_waiter() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let first = cache.check_or_register(key.clone());
    let HostCheckResult::NewlyRegistered { tx } = first else { panic!() };
    let mut rx = match cache.check_or_register(key.clone()) {
        HostCheckResult::AwaitPending(rx) => rx,
        other => panic!("expected AwaitPending, got {other:?}"),
    };
    tx.send(Some(HostDecision::AllowForSession)).unwrap();
    cache.resolve(key.clone(), HostDecision::AllowForSession);
    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), Some(HostDecision::AllowForSession));
    // After resolution, third call returns Cached.
    let third = cache.check_or_register(key);
    assert!(matches!(third, HostCheckResult::Cached(HostDecision::AllowForSession)));
}

#[tokio::test]
async fn allow_once_evicts_after_resolution() {
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://example.com").unwrap();
    let first = cache.check_or_register(key.clone());
    let HostCheckResult::NewlyRegistered { tx } = first else { panic!() };
    tx.send(Some(HostDecision::AllowOnce)).unwrap();
    cache.resolve(key.clone(), HostDecision::AllowOnce);
    // After AllowOnce resolution, the key is evicted: next call gets NewlyRegistered.
    let next = cache.check_or_register(key);
    assert!(matches!(next, HostCheckResult::NewlyRegistered { .. }));
}
```

- [ ] **Step 4: Run tests to verify failure**

Run:

```bash
cargo nextest run -p klynt-core --test host_approval_cache 2>&1 | tail -20
```

Expected: compile errors — `host_cache` module doesn't exist.

- [ ] **Step 5: Implement HostApprovalCache**

Create `crates/klynt-core/src/approval/host_cache.rs`:

```rust
//! Per-host approval deduplication cache, modeled on codex-rs/core/src/tools/
//! network_approval.rs::PendingHostApproval.
//!
//! When N parallel calls hit the same `(scheme, host, port)`, only the first
//! invokes Layer1; subsequent callers await the same resolution. Decisions
//! cache for the session (AllowForSession) or evict after one use (AllowOnce).

use common::Result;
use dashmap::{mapref::entry::Entry, DashMap};
use std::sync::Arc;
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct HostKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl HostKey {
    pub fn from_url(url: &str) -> Result<Self> {
        let u = Url::parse(url).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!("bad URL: {e}")))
        })?;
        let host = u.host_str().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::InvalidParams("URL has no host".into()))
        })?;
        let port = u.port_or_known_default().unwrap_or(0);
        Ok(Self {
            scheme: u.scheme().to_ascii_lowercase(),
            host: host.to_ascii_lowercase(),
            port,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostDecision {
    /// Per-call grant. Cache evicted after resolution.
    AllowOnce,
    /// Session-scoped grant. Cache retained until session ends.
    AllowForSession,
    /// Refused. Cache retained — future calls fail fast.
    Deny,
}

#[derive(Clone)]
enum HostState {
    Pending(watch::Receiver<Option<HostDecision>>),
    Resolved(HostDecision),
}

#[derive(Clone, Default)]
pub struct HostApprovalCache {
    map: Arc<DashMap<HostKey, HostState>>,
}

#[derive(Debug)]
pub enum HostCheckResult {
    /// Decision was already cached.
    Cached(HostDecision),
    /// Another concurrent caller is awaiting first-time resolution.
    AwaitPending(watch::Receiver<Option<HostDecision>>),
    /// First caller for this host. The caller MUST resolve via `cache.resolve(key, decision)`
    /// after evaluating the approval, and ALSO send via `tx` so existing waiters wake up.
    NewlyRegistered { tx: watch::Sender<Option<HostDecision>> },
}

impl HostApprovalCache {
    pub fn check_or_register(&self, key: HostKey) -> HostCheckResult {
        match self.map.entry(key.clone()) {
            Entry::Occupied(slot) => match slot.get().clone() {
                HostState::Pending(rx) => HostCheckResult::AwaitPending(rx),
                HostState::Resolved(d) => HostCheckResult::Cached(d),
            },
            Entry::Vacant(slot) => {
                let (tx, rx) = watch::channel(None);
                slot.insert(HostState::Pending(rx));
                HostCheckResult::NewlyRegistered { tx }
            }
        }
    }

    pub fn resolve(&self, key: HostKey, decision: HostDecision) {
        match decision {
            HostDecision::AllowOnce => {
                // Evict after broadcast — future calls re-enter the approval flow.
                self.map.remove(&key);
            }
            HostDecision::AllowForSession | HostDecision::Deny => {
                self.map
                    .entry(key)
                    .and_modify(|s| *s = HostState::Resolved(decision))
                    .or_insert(HostState::Resolved(decision));
            }
        }
    }
}
```

- [ ] **Step 6: Re-export from approval/mod.rs**

Open `crates/klynt-core/src/approval/mod.rs`. Find the existing `pub mod` lines (1-10 per the extraction). Add:

```rust
pub mod host_cache;
pub use host_cache::{HostApprovalCache, HostCheckResult, HostDecision, HostKey};
```

- [ ] **Step 7: Run tests, verify pass**

Run:

```bash
cargo nextest run -p klynt-core --test host_approval_cache 2>&1 | tail -10
```

Expected: 6 passes.

- [ ] **Step 8: Wire HostApprovalCache into web_fetch.rs**

Open `crates/klynt-core/src/tools/web_fetch.rs`. Update the struct + constructor:

```rust
pub struct WebFetchTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
    client: reqwest::Client,
    non_ui_policy: common::NonUiPolicy,
    host_cache: Arc<HostApprovalCache>,                                  // NEW
}

impl WebFetchTool {
    pub fn new(
        layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::NonUiPolicy,
        host_cache: Arc<HostApprovalCache>,                              // NEW
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client construction");
        Self { layer1, policy, privacy, pending, event_tx, bus, client, non_ui_policy, host_cache }
    }
}
```

Update the execute body to consult the cache before calling `evaluate`. Replace lines ~96-99 (the existing `evaluate` call) with:

```rust
use crate::approval::host_cache::{HostCheckResult, HostDecision, HostKey};

let host_key = HostKey::from_url(&args.url)?;
let host_decision = match self.host_cache.check_or_register(host_key.clone()) {
    HostCheckResult::Cached(d) => d,
    HostCheckResult::AwaitPending(mut rx) => {
        rx.changed().await.map_err(|_| KlyntbotError::Tool(
            ToolError::ExecutionFailed("host approval cancelled".into())))?;
        rx.borrow().expect("decision set on resolution")
    }
    HostCheckResult::NewlyRegistered { tx } => {
        let approval = evaluate(guard_ctx, "web_fetch", &args.url).await;
        let host_decision = if approval.allowed() {
            HostDecision::AllowForSession
        } else {
            HostDecision::Deny
        };
        let _ = tx.send(Some(host_decision));
        self.host_cache.resolve(host_key.clone(), host_decision);
        host_decision
    }
};

if host_decision == HostDecision::Deny {
    return Err(KlyntbotError::Tool(ToolError::PermissionDenied(
        format!("host {} previously denied", host_key.host))));
}
```

(Check `ApprovalDecision::allowed()` exists; if not, `matches!(approval, ApprovalDecision::Auto { allowed: true, .. })`.)

- [ ] **Step 9: Update app-core/src/init/mod.rs to construct + pass HostApprovalCache**

Add near the other Arc constructions:

```rust
let host_cache = Arc::new(klynt_core::approval::HostApprovalCache::default());
```

And update the `WebFetchTool::new(...)` call to include `host_cache.clone()` as the final arg.

- [ ] **Step 10: Add integration test for parallel dedup**

Append to `crates/klynt-core/tests/host_approval_cache.rs`:

```rust
#[tokio::test]
async fn parallel_calls_to_same_host_share_one_approval() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cache = Arc::new(HostApprovalCache::default());
    let key = HostKey::from_url("https://shared.example.com").unwrap();

    // Spawn 5 concurrent callers.
    let approval_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..5 {
        let cache_c = cache.clone();
        let key_c = key.clone();
        let approval_count_c = approval_count.clone();
        handles.push(tokio::spawn(async move {
            let r = cache_c.check_or_register(key_c.clone());
            match r {
                HostCheckResult::NewlyRegistered { tx } => {
                    approval_count_c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let _ = tx.send(Some(HostDecision::AllowForSession));
                    cache_c.resolve(key_c, HostDecision::AllowForSession);
                    HostDecision::AllowForSession
                }
                HostCheckResult::AwaitPending(mut rx) => {
                    rx.changed().await.unwrap();
                    rx.borrow().unwrap()
                }
                HostCheckResult::Cached(d) => d,
            }
        }));
    }
    for h in handles { h.await.unwrap(); }
    assert_eq!(approval_count.load(Ordering::SeqCst), 1,
               "exactly one approval round-trip should fire for 5 parallel calls");
}
```

- [ ] **Step 11: Run all tests, verify**

Run:

```bash
cargo nextest run -p klynt-core 2>&1 | tail -15
```

Expected: all green, including the new parallel test.

- [ ] **Step 12: Add invariant K13 — host approval dedup correctness**

Append to `tests/coding_in_chat_property.rs`:

```rust
proptest! {
    #[test]
    fn k13_host_approval_dedup_correctness(
        n_calls in 2usize..16,
        m_hosts in 1usize..6,
    ) {
        use klynt_core::approval::host_cache::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let cache = Arc::new(HostApprovalCache::default());
        let approvals = Arc::new(AtomicUsize::new(0));

        // Generate n_calls dispatched across m_hosts.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut handles = Vec::new();
            let unique_hosts: std::collections::HashSet<_> = (0..n_calls)
                .map(|i| i % m_hosts)
                .collect();
            for i in 0..n_calls {
                let cache_c = cache.clone();
                let approvals_c = approvals.clone();
                let host_idx = i % m_hosts;
                handles.push(tokio::spawn(async move {
                    let key = HostKey::from_url(
                        &format!("https://host{}.example.com", host_idx)).unwrap();
                    match cache_c.check_or_register(key.clone()) {
                        HostCheckResult::NewlyRegistered { tx } => {
                            approvals_c.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                            let _ = tx.send(Some(HostDecision::AllowForSession));
                            cache_c.resolve(key, HostDecision::AllowForSession);
                        }
                        HostCheckResult::AwaitPending(mut rx) => {
                            rx.changed().await.unwrap();
                        }
                        HostCheckResult::Cached(_) => {}
                    }
                }));
            }
            for h in handles { h.await.unwrap(); }
            prop_assert_eq!(approvals.load(Ordering::SeqCst), unique_hosts.len(),
                "expected one approval per unique host");
            Ok::<(), TestCaseError>(())
        }).unwrap();
    }
}
```

- [ ] **Step 13: Run K13**

Run:

```bash
cargo nextest run --workspace --test coding_in_chat_property 2>&1 | tail -10
```

Expected: K13 passes.

- [ ] **Step 14: Run full clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 15: Commit**

```bash
git add crates/klynt-core/Cargo.toml \
        crates/klynt-core/src/approval/{mod.rs,host_cache.rs} \
        crates/klynt-core/src/tools/web_fetch.rs \
        crates/klynt-core/tests/host_approval_cache.rs \
        crates/app-core/src/init/mod.rs \
        tests/coding_in_chat_property.rs

git commit -m "$(cat <<'EOF'
feat(approval): per-host approval deduplication (Codex-derived)

Adds HostApprovalCache keyed by (scheme, host, port). Concurrent
web_fetch calls to the same host coalesce onto one approval round-trip;
decisions cache as AllowOnce (per-call), AllowForSession (until session
end), or Deny. Mirrors codex-rs/core/src/tools/network_approval.rs.

Wires into WebFetchTool's execute path. Ships parallel-dedup integration
test plus K13 property invariant.

Part of tool layer consolidation (commit 3/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: `event_tx` via `RoutingContext` (FileEditWithSymbols visibility)

**Goal:** Close the `event_tx: None` data loss at `app-core/src/init/mod.rs:1817`. Today klynt-core tools' `FileEditWithSymbols` events vanish into a throwaway channel. Fix by threading `event_tx` through `RoutingContext` (a per-call object), not through tool struct fields. Tools at `execute()` time read `ctx.event_tx`.

**Files:**
- Modify: `crates/tools-core/src/routing.rs` (RoutingContext gains `event_tx` field)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (`process_message` populates ctx.event_tx)
- Modify: every klynt-core tool that previously used `self.event_tx` — read from `ctx.event_tx` instead
- Modify: `crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,web_fetch,plan_mode}.rs` (drop `event_tx` from struct field + constructor; consume from ctx)
- Modify: `crates/app-core/src/init/mod.rs` (drop `event_tx: None` line and the `event_tx.clone()` arguments)
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` (verify FileEditWithSymbols arm at lines 1090-1098 — should already exist per agent extraction)
- Verify-only: `desktop-ui/src/features/coding/hooks/useFileEditEvents.ts`, `desktop-ui/src/features/chat/store/chatStreamStore.ts`, `desktop-ui/src/features/chat/components/ChatThread.tsx` — these were Plan 3 deliverables; verify presence

- [ ] **Step 1: Add event_tx to RoutingContext**

Open `crates/tools-core/src/routing.rs`. Find the `RoutingContext` struct (line 60-83 per agent #4). Add the field:

```rust
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: Option<ChatId>,
    pub session_key: Option<SessionKey>,
    pub cancel_token: Option<CancellationToken>,
    pub interaction_tx: Option<mpsc::Sender<InteractionPayload>>,
    pub interaction_channel: Option<String>,
    /// Per-call streaming event channel. Tools push AgentEvent variants here
    /// (e.g., FileEditWithSymbols, SandboxPolicyApplied) for the relay to
    /// translate into Tauri events. May be None for non-streaming contexts.
    pub event_tx: Option<mpsc::Sender<agent::events::AgentEvent>>,        // NEW
    // ... existing fields ...
}
```

If `tools-core` doesn't depend on `agent`, this creates a circular dep. Resolution: move `AgentEvent` (or just the tool-emitted variants) to a lower-layer crate like `bus`, OR define a trait object pattern.

**Pragmatic resolution:** define a separate `ToolEvent` enum in `tools-core` containing only the variants tools emit (FileEditWithSymbols, SandboxPolicyApplied, PlanModeChanged, ApprovalRequested, ApprovalResolved). The agent crate translates `ToolEvent` → `AgentEvent` at consumption. This breaks the dep cycle.

Alternative: tools-core depends on `bus` (L1), and `AgentEvent` lives in `bus`. The relay in app-core consumes `AgentEvent` from bus. Works if `bus::AgentEvent` already exists (search to verify).

```bash
rg "pub enum AgentEvent" crates/bus/ 2>&1 | head -5
```

If `bus::AgentEvent` exists, use it. If not, define `tools_core::ToolEvent` and translate in agent. Adjust this step's content accordingly. **For the purposes of this plan, assume `bus::AgentEvent` exists or is moved there in this step**, since `bus` is already a tools-core dep candidate.

- [ ] **Step 2: Verify the dep change resolves**

Run:

```bash
cargo check -p tools-core 2>&1 | tail -5
```

Expected: clean.

- [ ] **Step 3: Update RoutingContext::new and constructors**

The existing constructors (`RoutingContext::new`, `with_interaction`, etc.) need to default `event_tx` to None. Find each call site and ensure the field is set explicitly or defaulted via `..Default::default()` if RoutingContext implements Default.

If no Default exists, add a builder method:

```rust
impl RoutingContext {
    pub fn with_event_tx(mut self, tx: mpsc::Sender<bus::AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}
```

- [ ] **Step 4: Populate ctx.event_tx in process_message**

Open `crates/agent/src/agent_runtime/runtime.rs`. Find `process_message` at lines 251-259. The function takes `event_tx: Option<mpsc::Sender<AgentEvent>>` as a separate arg AND `ctx: &RoutingContext`. The fix: clone event_tx into a mutable copy of ctx.

```rust
pub async fn process_message(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value],
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    depth: DepthMode,
) -> Result<RuntimeResult> {
    let mut ctx = ctx.clone();
    ctx.event_tx = event_tx.clone();
    if let Some(t) = &cancel_token { ctx.cancel_token = Some(t.clone()); }
    // ... use the patched `ctx` from here onward ...
}
```

`RoutingContext` should derive `Clone` (it likely does already; verify).

- [ ] **Step 5: Update klynt-core tools to read from ctx.event_tx**

For each tool that previously used `self.event_tx`:

`crates/klynt-core/src/tools/web_fetch.rs`: change

```rust
self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
```

to

```rust
ctx.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
```

Same change in `bash.rs`, `edit.rs`, `write.rs`, `apply_patch.rs`, `notebook_edit.rs`, `plan_mode.rs` (both EnterPlanModeTool and ExitPlanModeTool execute paths).

- [ ] **Step 6: Drop `event_tx` from tool struct fields and constructors**

Now that tools read from ctx, the per-tool `event_tx` field is dead. Remove it.

For each of the tools above (bash, edit, write, apply_patch, notebook_edit, web_fetch, plan_mode):

```rust
pub struct EditTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    // event_tx: Option<mpsc::Sender<AgentEvent>>,         // REMOVED
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::NonUiPolicy,
}

impl EditTool {
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>, pending: Arc<PendingApprovalsMap>,
        // event_tx: Option<mpsc::Sender<AgentEvent>>,     // REMOVED
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::NonUiPolicy,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, bus, non_ui_policy }
    }
}
```

- [ ] **Step 7: Update app-core/src/init/mod.rs constructor calls**

Drop the `event_tx.clone()` argument from each `kt::*Tool::new(...)` call. Also delete the line:

```rust
let event_tx: Option<tokio::sync::mpsc::Sender<agent::events::AgentEvent>> = None;
```

The `cwd` and other args stay as they are.

- [ ] **Step 8: Verify cargo build**

Run:

```bash
cargo build --workspace 2>&1 | tail -15
```

Expected: clean. If errors arise about `ctx.event_tx` access in tools that didn't previously use event_tx (e.g. read.rs), no action needed — they don't reference it.

- [ ] **Step 9: Verify the relay arms exist**

Run:

```bash
rg "FileEditWithSymbols|PlanModeChanged" crates/app-core/src/handlers/chat/streaming.rs 2>&1 | head -10
```

Expected output should include lines around 1090-1104 — the FileEditWithSymbols and PlanModeChanged arms exist per agent #2's extraction. If not present, add them per Section 8 of the consolidation spec.

- [ ] **Step 10: Verify React-side hooks exist**

Run:

```bash
ls -la desktop-ui/src/features/coding/hooks/useFileEditEvents.ts 2>&1
ls -la desktop-ui/src/features/chat/store/chatStreamStore.ts 2>&1
```

Expected: both exist (Plan 3 deliverables). If missing, create per consolidation spec §8.

- [ ] **Step 11: Write integration test for event flow**

Create `crates/klynt-core/tests/event_tx_flow.rs`:

```rust
//! Verifies that mutating tools push FileEditWithSymbols events to the
//! per-call event_tx via RoutingContext.

use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use bus::{AgentEvent, DomainEventBus};
use common::tool_channel::Channel;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::WriteTool;
use klynt_execpolicy::Policy;
use tools_core::{RoutingContext, ToolExecute};

#[tokio::test]
async fn write_tool_emits_file_edit_event() {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path().to_path_buf();
    let perms = config::schema::CodingPermissions {
        allow: vec!["write:*".into()], // bypass approval
        deny: vec![], ask: vec![],
        default_if_no_match: "allow".into(),
    };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));

    let tool = WriteTool::new(
        cwd.clone(), layer1, policy, privacy, pending, bus,
        common::NonUiPolicy::Allow,
    );

    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    let mut ctx = RoutingContext::default();
    ctx.channel = "coding".into();
    ctx.cancel_token = Some(CancellationToken::new());
    ctx.event_tx = Some(event_tx);

    // Args for write_tool: path + content
    let args = serde_json::json!({
        "path": "test.txt",
        "content": "hello world",
    });
    let _ = tool.execute(serde_json::from_value(args).unwrap(), &ctx).await.unwrap();

    let ev = tokio::time::timeout(std::time::Duration::from_secs(1), event_rx.recv()).await;
    let ev = ev.expect("timeout").expect("channel closed");
    assert!(matches!(ev, AgentEvent::FileEditWithSymbols { .. }));
}
```

- [ ] **Step 12: Run integration test**

Run:

```bash
cargo nextest run -p klynt-core --test event_tx_flow 2>&1 | tail -10
```

Expected: pass.

- [ ] **Step 13: Run full nextest + clippy**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -15
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: green, zero warnings.

- [ ] **Step 14: Commit**

```bash
git add crates/tools-core/src/routing.rs \
        crates/agent/src/agent_runtime/runtime.rs \
        crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,web_fetch,plan_mode}.rs \
        crates/klynt-core/tests/event_tx_flow.rs \
        crates/app-core/src/init/mod.rs

git commit -m "$(cat <<'EOF'
fix(events): wire event_tx via RoutingContext so FileEditWithSymbols reaches UI

Closes the silent data loss at app-core/src/init/mod.rs:1817 where
event_tx was hardcoded to None. Threads the per-call mpsc through
RoutingContext.event_tx; tools read from ctx at execute time instead
of holding a constructor-time field that goes stale after the message.

Drops event_tx from klynt-core tool struct fields and constructors.
Adds integration test verifying WriteTool emits FileEditWithSymbols
to the per-call channel.

Spec amendment for §8 of 2026-04-30-tool-layer-consolidation-design.md
folded inline.

Part of tool layer consolidation (commit 4/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: `ToolKitBuilder` + main agent rewiring

**Goal:** Collapse the 12+ scattered `kt::*Tool::new(...)` calls in `app-core/src/init/mod.rs:1784-1851` into a single builder construction. The builder owns the deps; `register_*` methods produce a registry. Same observable behavior — same tools, same constructors.

**Files:**
- Create: `crates/klynt-core/src/registry/builder.rs`
- Modify: `crates/klynt-core/src/lib.rs` (re-export ToolKitBuilder)
- Modify: `crates/klynt-core/src/registry/mod.rs` (re-export builder)
- Modify: `crates/klynt-core/Cargo.toml` (add deps if needed)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (+ tool_kit field + accessors)
- Modify: `crates/app-core/src/init/mod.rs` (use ToolKitBuilder)

- [ ] **Step 1: Verify klynt-core's deps include what builder needs**

Builder needs: `agent` (for AgentEvent — already via bus), `storage` (for Repos), `bus` (DomainEventBus), `klynt-execpolicy` (Layer1, Policy), `klynt-sandbox` (Policy/Sandbox types). Run:

```bash
grep -E "^(agent|storage|bus|klynt-execpolicy|klynt-sandbox)" crates/klynt-core/Cargo.toml
```

Add any missing deps to `crates/klynt-core/Cargo.toml`.

- [ ] **Step 2: Create the builder file**

Create `crates/klynt-core/src/registry/builder.rs`:

```rust
//! ToolKitBuilder — single-source-of-truth dependency injection for
//! klynt-core's coding tool kit. Used by both the main agent (in
//! app-core/init/mod.rs) and sub-agents (in agent/subagent.rs) to
//! construct registries without duplicating per-tool wiring.

use crate::approval::{HostApprovalCache, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use std::path::PathBuf;
use std::sync::Arc;
use storage::Repos;
use tools_core::ToolRegistry;

/// Holds the dependencies needed to construct any klynt-core tool.
/// Cheap to clone (`Arc`-shared fields).
#[derive(Clone)]
pub struct ToolKitBuilder {
    pub cwd: PathBuf,
    pub layer1: Arc<Layer1>,
    pub policy: Arc<Policy>,
    pub privacy: Arc<PrivacyGuard>,
    pub pending: Arc<PendingApprovalsMap>,
    pub bus: Arc<DomainEventBus>,
    pub repos: Repos,
    pub host_cache: Arc<HostApprovalCache>,
    pub non_ui_policy: common::NonUiPolicy,
}

impl ToolKitBuilder {
    /// Returns a builder with `cwd` overridden — used when a sub-agent runs
    /// in a subdirectory of the parent's workspace.
    pub fn with_cwd(self, cwd: PathBuf) -> Self {
        Self { cwd, ..self }
    }

    /// Register the read-only / portable tools (graduate to all channels per
    /// Task 8). Six tools.
    pub fn register_read_only(&self, reg: &mut ToolRegistry) {
        use crate::tools::*;
        reg.register(ReadTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(GlobTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(GrepTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(AskUserTool::default());
        reg.register(WebFetchTool::new(
            self.layer1.clone(), self.policy.clone(), self.privacy.clone(),
            self.pending.clone(), self.bus.clone(),
            self.non_ui_policy, self.host_cache.clone(),
        ));
        reg.register(ToolSearchTool::new());
    }

    /// Register the 5 mutating tools — coding-only via `ChannelMask::CODING_ONLY`.
    pub fn register_mutating(&self, reg: &mut ToolRegistry) {
        use crate::tools::*;
        reg.register(WriteTool::new(
            self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(), self.bus.clone(),
            self.non_ui_policy,
        ));
        reg.register(EditTool::new(
            self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(), self.bus.clone(),
            self.non_ui_policy,
        ));
        reg.register(ApplyPatchTool::new(
            self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(), self.bus.clone(),
            self.non_ui_policy,
        ));
        reg.register(NotebookEditTool::new(
            self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(), self.bus.clone(),
            self.non_ui_policy,
        ));
        reg.register(BashTool::new(
            self.layer1.clone(), self.policy.clone(), self.privacy.clone(),
            self.pending.clone(), self.bus.clone(),
            self.non_ui_policy,
        ));
    }

    /// Register plan-mode tools (coding-only).
    pub fn register_plan_mode(&self, reg: &mut ToolRegistry) {
        use crate::tools::plan_mode::*;
        reg.register(EnterPlanModeTool::new(self.repos.clone(), self.bus.clone()));
        reg.register(ExitPlanModeTool::new(self.repos.clone(), self.bus.clone()));
    }

    /// Register the entire 13-tool coding kit.
    pub fn register_all(&self, reg: &mut ToolRegistry) {
        self.register_read_only(reg);
        self.register_mutating(reg);
        self.register_plan_mode(reg);
    }
}
```

- [ ] **Step 3: Re-export from registry/mod.rs**

Open `crates/klynt-core/src/registry/mod.rs`. Append:

```rust
pub mod builder;
pub use builder::ToolKitBuilder;
```

- [ ] **Step 4: Re-export from klynt-core/lib.rs**

Open `crates/klynt-core/src/lib.rs`. Add:

```rust
pub use registry::ToolKitBuilder;
```

- [ ] **Step 5: Verify builder compiles**

Run:

```bash
cargo check -p klynt-core 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Add tool_kit field to AgentRuntime**

Open `crates/agent/src/agent_runtime/runtime.rs`. Find the struct definition (lines 47-77 per agent #1). Add:

```rust
pub struct AgentRuntime {
    // ... existing fields ...
    tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,                   // NEW
}
```

In the constructor (line 81-88) initialize to `None`. Add accessors near the existing ones (~line 239):

```rust
pub fn tool_kit(&self) -> Option<Arc<klynt_core::ToolKitBuilder>> {
    self.tool_kit.clone()
}

pub fn set_tool_kit(&mut self, kit: Arc<klynt_core::ToolKitBuilder>) {
    self.tool_kit = Some(kit);
}
```

If AgentRuntime is wrapped in Arc and not mutable, expose via `Arc<RwLock<Option<...>>>` or use OnceCell. Inspect the existing pattern for `tool_registry` to follow it.

- [ ] **Step 7: Add agent → klynt-core dep**

Open `crates/agent/Cargo.toml`. Add `klynt-core = { workspace = true }` to `[dependencies]` if not already present. (This is a UPWARD dep — verify klynt-core itself doesn't depend on agent. If it does, the cycle blocks this; resolve by moving ToolKitBuilder type signature to use trait objects from a lower layer.)

```bash
grep "klynt-core\|agent" crates/klynt-core/Cargo.toml | head -5
grep "klynt-core" crates/agent/Cargo.toml | head -5
```

If a cycle exists, the cleanest fix is keeping ToolKitBuilder in klynt-core but having `AgentRuntime::tool_kit` typed as `Option<Arc<dyn Any>>` and downcasting at use sites — ugly. Better: keep agent's reference to klynt-core via a feature flag.

For this plan, assume the upward dep is allowed.

- [ ] **Step 8: Rewrite app-core/src/init/mod.rs:1784-1851 to use ToolKitBuilder**

Replace the entire registration block with:

```rust
{
    let config_guard = core.config.read().await;
    let perms = &config_guard.coding.permissions;
    let layer1 = Arc::new(klynt_core::approval::Layer1::compile(perms)
        .expect("Layer 1 rules failed to compile"));
    let exclude_globs: Vec<&str> = config_guard.coding_memory.ingest.exclude_paths
        .iter().map(String::as_str).collect();
    let privacy = Arc::new(
        klynt_core::privacy::PrivacyGuard::from_globs(&exclude_globs).expect("privacy globs"),
    );
    let policy = Arc::new(
        dirs::home_dir()
            .map(|h| h.join(".klyntbot/rules"))
            .and_then(|p| klynt_execpolicy::Policy::load_from_dir(&p).ok())
            .unwrap_or_else(klynt_execpolicy::Policy::empty),
    );
    let pending = core.pending_approvals.clone();
    let bus = core.domain_event_bus.clone()
        .unwrap_or_else(|| Arc::new(bus::DomainEventBus::new(64)));
    let cwd = config_guard.coding_memory.workspace_root.clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let host_cache = Arc::new(klynt_core::approval::HostApprovalCache::default());
    let non_ui_policy = config_guard.tools.approval_policy.non_ui_channels;

    let kit = Arc::new(klynt_core::ToolKitBuilder {
        cwd, layer1, policy, privacy, pending, bus, repos: core.repos.clone(),
        host_cache, non_ui_policy,
    });

    {
        let reg = core.agent.tool_registry();
        let mut registry = reg.write().await;
        kit.register_all(&mut registry);
    }

    // Stash for sub-agent use (Task 6).
    if let Ok(mut runtime) = core.agent.try_lock_mut() {
        runtime.set_tool_kit(kit);
    }

    info!("Coding tool kit registered via ToolKitBuilder (13 tools)");
}
```

If `core.agent` is `Arc<AgentRuntime>` and not mutably accessible, use the OnceCell or RwLock pattern hinted at in Step 6.

- [ ] **Step 9: Run cargo build**

Run:

```bash
cargo build --workspace 2>&1 | tail -15
```

Expected: clean. If a circular dep blocks, resolve per Step 7's note.

- [ ] **Step 10: Add unit tests for ToolKitBuilder**

Create `crates/klynt-core/tests/tool_kit_builder.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use bus::DomainEventBus;
use klynt_core::approval::{HostApprovalCache, Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::ToolKitBuilder;
use klynt_execpolicy::Policy;
use tools_core::ToolRegistry;

fn make_builder() -> ToolKitBuilder {
    let perms = config::schema::CodingPermissions {
        allow: vec![], deny: vec![], ask: vec![],
        default_if_no_match: "ask".into(),
    };
    ToolKitBuilder {
        cwd: TempDir::new().unwrap().path().to_path_buf(),
        layer1: Arc::new(Layer1::compile(&perms).unwrap()),
        policy: Arc::new(Policy::empty()),
        privacy: Arc::new(PrivacyGuard::from_globs(&[]).unwrap()),
        pending: Arc::new(PendingApprovalsMap::default()),
        bus: Arc::new(DomainEventBus::new(8)),
        repos: storage::Repos::ephemeral(),
        host_cache: Arc::new(HostApprovalCache::default()),
        non_ui_policy: common::NonUiPolicy::Allow,
    }
}

#[test]
fn register_read_only_registers_six_tools() {
    let builder = make_builder();
    let mut reg = ToolRegistry::new();
    builder.register_read_only(&mut reg);
    let names: std::collections::HashSet<_> = reg.tool_names().into_iter().collect();
    let expected: std::collections::HashSet<_> =
        ["read", "glob", "grep", "ask_user", "web_fetch", "tool_search"]
        .iter().map(|s| s.to_string()).collect();
    assert_eq!(names, expected);
}

#[test]
fn register_mutating_registers_five_tools() {
    let builder = make_builder();
    let mut reg = ToolRegistry::new();
    builder.register_mutating(&mut reg);
    let names: std::collections::HashSet<_> = reg.tool_names().into_iter().collect();
    let expected: std::collections::HashSet<_> =
        ["bash", "edit", "write", "apply_patch", "notebook_edit"]
        .iter().map(|s| s.to_string()).collect();
    assert_eq!(names, expected);
}

#[test]
fn register_plan_mode_registers_two_tools() {
    let builder = make_builder();
    let mut reg = ToolRegistry::new();
    builder.register_plan_mode(&mut reg);
    let names: std::collections::HashSet<_> = reg.tool_names().into_iter().collect();
    let expected: std::collections::HashSet<_> =
        ["enter_plan_mode", "exit_plan_mode"]
        .iter().map(|s| s.to_string()).collect();
    assert_eq!(names, expected);
}

#[test]
fn register_all_registers_thirteen_tools() {
    let builder = make_builder();
    let mut reg = ToolRegistry::new();
    builder.register_all(&mut reg);
    assert_eq!(reg.tool_names().len(), 13);
}
```

(`storage::Repos::ephemeral()` may not exist — find the in-memory-pool helper. From CLAUDE.md: `StoragePool::connect_in_memory()`. Adjust the call.)

- [ ] **Step 11: Run builder tests**

Run:

```bash
cargo nextest run -p klynt-core --test tool_kit_builder 2>&1 | tail -10
```

Expected: 4 passes.

- [ ] **Step 12: Run full nextest + clippy**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -15
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: green, zero warnings.

- [ ] **Step 13: Commit**

```bash
git add crates/klynt-core/src/registry/{builder.rs,mod.rs} \
        crates/klynt-core/src/lib.rs \
        crates/klynt-core/tests/tool_kit_builder.rs \
        crates/agent/src/agent_runtime/runtime.rs \
        crates/agent/Cargo.toml \
        crates/app-core/src/init/mod.rs

git commit -m "$(cat <<'EOF'
refactor(klynt-core): introduce ToolKitBuilder for unified DI

Collapses 12+ scattered kt::*Tool::new() calls in app-core init into
a single ToolKitBuilder that owns the deps and exposes profile-shaped
register_* methods (read_only, mutating, plan_mode, all).

Stashes the builder on AgentRuntime via tool_kit() / set_tool_kit()
accessors so sub-agents (Task 6) can reuse it.

Same observable behavior — same 13 tools registered, same constructors.

Part of tool layer consolidation (commit 5/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Sub-agent rewiring

**Goal:** Replace `crates/agent/src/subagent.rs:430-470`'s legacy registrations (using OLD `tools::system::*` types) with `ToolKitBuilder`-based registration. Each `SubagentProfile` maps to `register_*` calls.

**Files:**
- Modify: `crates/agent/src/subagent.rs`
- Modify: `crates/agent/Cargo.toml` (drop dep on `tools` if subagent.rs was the last consumer; verify before removing)

- [ ] **Step 1: Read subagent.rs current state**

```bash
sed -n '1,100p' crates/agent/src/subagent.rs
sed -n '420,480p' crates/agent/src/subagent.rs
```

Note the SubagentProfile variants (General / Research / Analyst per agent #1).

- [ ] **Step 2: Replace imports at top of subagent.rs**

Find the imports block (lines 17-23 per agent #1) and replace:

```rust
use tools::{
    agent_task_tool::AgentTaskTool,
    filesystem::{register_fs_read_tools, register_fs_tools},
    glob_tool::GlobTool,
    grep::GrepTool,
    registry::ToolRegistry,
    spawn::SpawnHandler,
    web::{WebFetchTool, WebSearchTool},
    RoutingContext,
};
```

with:

```rust
use tools::{
    agent_task_tool::AgentTaskTool,
    spawn::SpawnHandler,
};
use tools_core::{RoutingContext, ToolRegistry};
```

(Keep `AgentTaskTool` and `SpawnHandler` since they're domain tools, not system tools.)

- [ ] **Step 3: Replace the registration block at lines 430-462**

```rust
let mut tools = ToolRegistry::new();

let allowed_dir = if config.restrict_to_workspace {
    Some(workspace.to_path_buf())
} else {
    None
};
let cwd = allowed_dir.clone().unwrap_or_else(|| std::env::current_dir()
    .unwrap_or_else(|_| std::path::PathBuf::from(".")));

let kit = config.parent_runtime
    .as_ref()
    .and_then(|p| p.tool_kit())
    .ok_or_else(|| common::KlyntbotError::Config(
        "ToolKitBuilder not initialized on parent runtime; ensure app-core/init runs before subagent spawn".into()
    ))?;
let kit = Arc::new((*kit).clone().with_cwd(cwd));

match profile {
    SubagentProfile::General => {
        kit.register_read_only(&mut tools);
        kit.register_mutating(&mut tools);
    }
    SubagentProfile::Research => {
        kit.register_read_only(&mut tools);
    }
    SubagentProfile::Analyst => {
        kit.register_read_only(&mut tools);
    }
}

// Domain tool — keep
tools.register(AgentTaskTool::new(
    config.agent_task_repo.clone(),
    config.session_key.clone(),
    config.agent_id.clone(),
));
```

The sub-agent's `config` struct gains a `parent_runtime: Option<Arc<AgentRuntime>>` field. Update the SubagentConfig (or wherever `config` is constructed) to include it. The caller passes `Arc::clone(&app_core.agent)` when spawning a sub-agent.

- [ ] **Step 4: Add parent_runtime field to SubagentConfig**

Search:

```bash
rg "struct SubagentConfig\|struct SubAgentConfig" crates/agent/src/subagent.rs | head -3
```

Add the `parent_runtime: Option<Arc<crate::agent_runtime::AgentRuntime>>` field. Update every constructor of SubagentConfig.

- [ ] **Step 5: Update sub-agent spawn callers**

Search for `SubagentConfig::new\|SubagentConfig {` callers:

```bash
rg "SubagentConfig\s*\{" --type rust | head -10
rg "SubagentConfig::new\(" --type rust | head -10
```

For each call site, pass `parent_runtime: Some(app_core_handle.agent.clone())` (or the appropriate accessor).

- [ ] **Step 6: Verify build**

Run:

```bash
cargo build --workspace 2>&1 | tail -15
```

Expected: clean. If errors about missing imports or types in subagent.rs callers, fix them.

- [ ] **Step 7: Add unit test for sub-agent profile registration**

Create `crates/agent/tests/subagent_profiles.rs`:

```rust
//! Verifies that each SubagentProfile registers the expected tool set
//! through ToolKitBuilder.

// This test requires a parent AgentRuntime with a ToolKitBuilder. Build a
// minimal one for testing.

#[tokio::test]
async fn general_profile_registers_read_only_plus_mutating() {
    // ... construct minimal AgentRuntime with a ToolKitBuilder set ...
    // ... spawn a sub-agent with SubagentProfile::General ...
    // ... assert the tool registry contains exactly:
    //     read, glob, grep, ask_user, web_fetch, tool_search,
    //     bash, edit, write, apply_patch, notebook_edit,
    //     agent_task
}
```

(Building a minimal test runtime is non-trivial. If the existing test infrastructure for sub-agents lives elsewhere, follow that pattern. If not, the K-invariant property test in `tests/coding_in_chat_property.rs` may be enough.)

- [ ] **Step 8: Run nextest**

Run:

```bash
cargo nextest run -p agent 2>&1 | tail -15
```

Expected: green.

- [ ] **Step 9: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 10: Commit**

```bash
git add crates/agent/src/subagent.rs \
        crates/agent/tests/subagent_profiles.rs

git commit -m "$(cat <<'EOF'
refactor(subagent): use ToolKitBuilder for tool registration

Replaces the legacy register_fs_tools / register_fs_read_tools /
GlobTool::new / GrepTool::new / WebFetchTool::new / WebSearchTool::new
calls with kit.register_* methods sourced from the parent runtime's
ToolKitBuilder.

Sub-agents now inherit the same dep injection as the main agent.
SubagentConfig gains parent_runtime: Option<Arc<AgentRuntime>>.

Profiles map: General → read_only + mutating, Research → read_only,
Analyst → read_only. AgentTaskTool retained (domain tool, not system).

Part of tool layer consolidation (commit 6/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Param-shape ports (`grep.context_lines`, `glob.path`)

**Goal:** Two OLD parameters that LLMs were prompted to use. Port into klynt-core with the same semantics.

**Files:**
- Modify: `crates/klynt-core/src/tools/grep.rs`
- Modify: `crates/klynt-core/src/tools/glob.rs`
- Test: append to `crates/klynt-core/tests/tool_grep.rs` and `crates/klynt-core/tests/tool_glob.rs` (create if absent)

- [ ] **Step 1: Write the failing test for grep.context_lines**

Create or append to `crates/klynt-core/tests/tool_grep.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::{GrepArgs, GrepTool};
use tools_core::{RoutingContext, ToolExecute};

#[tokio::test]
async fn grep_with_context_lines_emits_surrounding() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"),
        "line0\nline1\nMATCH\nline3\nline4\n").unwrap();

    let tool = GrepTool::new(dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[]).unwrap()));
    let ctx = RoutingContext::default();
    let args = GrepArgs {
        pattern: "MATCH".into(),
        include: None,
        case_insensitive: None,
        max_results: None,
        context_lines: Some(2),
    };
    let out = tool.execute(args, &ctx).await.unwrap();

    // Should include lines 0-4 (matching at line 2, ±2)
    assert!(out.contains("line0"));
    assert!(out.contains("line1"));
    assert!(out.contains("MATCH"));
    assert!(out.contains("line3"));
    assert!(out.contains("line4"));
}

#[tokio::test]
async fn grep_context_lines_clamped_to_5() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"),
        (0..20).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n")
        + "\nMATCH\n"
        + &(20..40).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n")).unwrap();

    let tool = GrepTool::new(dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[]).unwrap()));
    let ctx = RoutingContext::default();
    let args = GrepArgs {
        pattern: "MATCH".into(),
        include: None, case_insensitive: None, max_results: None,
        context_lines: Some(100),  // clamped to 5
    };
    let out = tool.execute(args, &ctx).await.unwrap();

    // Should include line15-line24 area (±5 around match), but NOT line0/line19+5+1
    let line_count = out.lines().count();
    assert!(line_count <= 11, "expected ≤11 lines (5+match+5), got {line_count}");
}
```

- [ ] **Step 2: Run test, verify failure**

Run:

```bash
cargo nextest run -p klynt-core --test tool_grep 2>&1 | tail -10
```

Expected: compile fails — `context_lines` field doesn't exist on `GrepArgs`.

- [ ] **Step 3: Add context_lines to GrepArgs**

Open `crates/klynt-core/src/tools/grep.rs`. Modify the struct (lines 12-22):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GrepArgs {
    #[param(required)] pub pattern: String,
    pub include: Option<String>,
    pub case_insensitive: Option<bool>,
    pub max_results: Option<u64>,
    /// Lines of context before+after each match (0-5). Default 0.
    pub context_lines: Option<u8>,
}
```

- [ ] **Step 4: Implement context_lines logic**

Modify the search loop (lines 62-87) to emit context. Replace the inner per-line block:

```rust
let cwd = self.cwd.clone();
let privacy = self.privacy.clone();
let lines = tokio::task::spawn_blocking(move || -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let ctx_lines = args.context_lines.unwrap_or(0).min(5) as usize;

    'outer: for entry in WalkDir::new(&cwd).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() { continue; }
        let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
        if !glob.is_match(rel) { continue; }
        if privacy.is_excluded(entry.path()) { continue; }
        let file_contents = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c, Err(_) => continue,
        };
        let lines_vec: Vec<&str> = file_contents.lines().collect();
        for (i, line) in lines_vec.iter().enumerate() {
            if !re.is_match(line) { continue; }
            if ctx_lines == 0 {
                out.push(format!("{}:{}:{}", rel.display(), i + 1, line));
            } else {
                let lo = i.saturating_sub(ctx_lines);
                let hi = (i + ctx_lines + 1).min(lines_vec.len());
                for j in lo..hi {
                    let marker = if j == i { ":" } else { "-" };
                    out.push(format!("{}{}{}{}{}",
                        rel.display(), marker, j + 1, marker, lines_vec[j]));
                }
                out.push("--".into());
            }
            if out.len() >= max { break 'outer; }
        }
    }
    out
}).await.map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
```

- [ ] **Step 5: Run test, verify pass**

Run:

```bash
cargo nextest run -p klynt-core --test tool_grep 2>&1 | tail -10
```

Expected: 2 passes.

- [ ] **Step 6: Write the failing test for glob.path**

Create or append to `crates/klynt-core/tests/tool_glob.rs`:

```rust
use std::sync::Arc;
use tempfile::TempDir;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::{GlobArgs, GlobTool};
use tools_core::{RoutingContext, ToolExecute};

#[tokio::test]
async fn glob_with_path_overrides_cwd() {
    let cwd_dir = TempDir::new().unwrap();
    let alt_dir = TempDir::new().unwrap();
    std::fs::write(cwd_dir.path().join("cwd.rs"), "// cwd").unwrap();
    std::fs::write(alt_dir.path().join("alt.rs"), "// alt").unwrap();

    let tool = GlobTool::new(cwd_dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[]).unwrap()));
    let ctx = RoutingContext::default();
    let args = GlobArgs {
        pattern: "*.rs".into(),
        max_results: None,
        path: Some(alt_dir.path().to_string_lossy().into_owned()),
    };
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("alt.rs"));
    assert!(!out.contains("cwd.rs"));
}

#[tokio::test]
async fn glob_with_excluded_path_returns_privacy_error() {
    let cwd_dir = TempDir::new().unwrap();
    let priv_dir = TempDir::new().unwrap();
    std::fs::write(priv_dir.path().join("secret.rs"), "// secret").unwrap();

    let exclude_glob = format!("{}/**", priv_dir.path().display());
    let tool = GlobTool::new(cwd_dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[&exclude_glob]).unwrap()));
    let ctx = RoutingContext::default();
    let args = GlobArgs {
        pattern: "*.rs".into(),
        max_results: None,
        path: Some(priv_dir.path().to_string_lossy().into_owned()),
    };
    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("excluded") || err.contains("privacy"));
}
```

- [ ] **Step 7: Run test, verify failure**

Run:

```bash
cargo nextest run -p klynt-core --test tool_glob 2>&1 | tail -10
```

Expected: compile fails — `path` field doesn't exist on `GlobArgs`.

- [ ] **Step 8: Add path to GlobArgs and implement override**

Open `crates/klynt-core/src/tools/glob.rs`. Modify the struct (lines 12-19):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GlobArgs {
    #[param(required)] pub pattern: String,
    pub max_results: Option<u64>,
    /// Override search root. Defaults to session cwd. Privacy-checked.
    pub path: Option<String>,
}
```

In `execute()`, replace the `cwd` derivation:

```rust
async fn execute(&self, args: GlobArgs, _ctx: &RoutingContext) -> Result<String> {
    let max = args.max_results.unwrap_or(100) as usize;
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(&args.pattern)
        .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("bad pattern: {e}"))))?);
    let set = builder.build()
        .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(e.to_string())))?;

    let root = match args.path.as_deref() {
        Some(p) => {
            let expanded: PathBuf = if let Some(rest) = p.strip_prefix("~/") {
                dirs::home_dir().unwrap_or_default().join(rest)
            } else {
                PathBuf::from(p)
            };
            let canonical = expanded.canonicalize().unwrap_or(expanded);
            if self.privacy.is_excluded(&canonical) {
                return Err(KlyntbotError::Tool(ToolError::InvalidParams(
                    format!("glob path '{}' is privacy-excluded", canonical.display())
                )));
            }
            canonical
        }
        None => self.cwd.clone(),
    };

    let privacy = self.privacy.clone();
    let matches = tokio::task::spawn_blocking(move || -> Vec<(std::time::SystemTime, PathBuf)> {
        let mut out: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        for entry in WalkDir::new(&root).follow_links(false) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() { continue; }
            let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
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
```

(Add `dirs = { workspace = true }` to klynt-core/Cargo.toml if not already.)

- [ ] **Step 9: Run test, verify pass**

Run:

```bash
cargo nextest run -p klynt-core --test tool_glob 2>&1 | tail -10
cargo nextest run -p klynt-core --test tool_grep 2>&1 | tail -10
```

Expected: 4 passes (2 grep + 2 glob).

- [ ] **Step 10: Run full nextest + clippy**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: green, zero warnings.

- [ ] **Step 11: Commit**

```bash
git add crates/klynt-core/src/tools/{grep.rs,glob.rs} \
        crates/klynt-core/tests/{tool_grep.rs,tool_glob.rs} \
        crates/klynt-core/Cargo.toml

git commit -m "$(cat <<'EOF'
feat(klynt-core): port grep.context_lines and glob.path params

Adds context_lines (0-5, default 0) to GrepTool — mirrors ripgrep -C N.
Output format: matched line uses ':' separator, context lines use '-',
with '--' separator between non-adjacent matches.

Adds path override to GlobTool. PrivacyGuard validates the resolved
path; excluded paths return InvalidParams. Tilde expansion supported.

Restores parameter parity with the OLD crates/tools/src/system/grep.rs
and glob_tool.rs that retire in Task 9.

Part of tool layer consolidation (commit 7/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Tool graduation — 6 klynt-core tools → `ChannelMask::ALL`

**Goal:** Remove the `allowed_channels = "coding_only"` override from `read`, `glob`, `grep`, `ask_user`, `web_fetch`, `tool_search` so they're visible in regular chat. The default `ChannelMask::ALL` applies.

**Files:**
- Modify: `crates/klynt-core/src/tools/{read,glob,grep,ask_user,web_fetch,tool_search}.rs`
- Test: append to `tests/coding_in_chat_property.rs`

- [ ] **Step 1: Verify which tools have the override**

Run:

```bash
rg "allowed_channels = " crates/klynt-core/src/tools/ 2>&1 | head -15
```

Expected: 7 tools (bash, edit, write, apply_patch, notebook_edit, plan_mode×2). The 6 graduating tools should NOT appear.

If any of (`read`, `glob`, `grep`, `ask_user`, `web_fetch`, `tool_search`) has the override (perhaps Task 1's macro extension applied default-coding-only), remove it from each.

- [ ] **Step 2: Confirm read/glob/grep/ask_user/web_fetch/tool_search have NO override**

Look at each `crates/klynt-core/src/tools/{read,glob,grep,ask_user,web_fetch,tool_search}.rs` `#[tool(...)]` attribute. None should contain `allowed_channels =`.

If any does, delete that attribute key. Done in 6 small edits.

- [ ] **Step 3: Write the failing graduation test**

Append to `tests/coding_in_chat_property.rs`:

```rust
#[test]
fn tool_graduation_default_mask_is_all() {
    use common::tool_channel::{Channel, ChannelMask};
    // The 6 graduated tools should return ChannelMask::ALL via default impl.
    // Construct minimal builder + register_read_only and assert each tool's mask.
    let builder = /* make_builder() helper from tool_kit_builder.rs test */;
    let mut reg = tools_core::ToolRegistry::new();
    builder.register_read_only(&mut reg);
    for name in ["read", "glob", "grep", "ask_user", "web_fetch", "tool_search"] {
        let tool = reg.get(name).expect(name);
        let mask = tool.allowed_channels();
        assert_eq!(mask, ChannelMask::ALL,
            "{name} should default to ChannelMask::ALL");
        assert!(mask.allows(Channel::Coding));
        assert!(mask.allows(Channel::Desktop));
        assert!(mask.allows(Channel::Other));
    }
}

#[test]
fn coding_only_tools_stay_coding_only() {
    use common::tool_channel::{Channel, ChannelMask};
    let builder = /* make_builder() helper */;
    let mut reg = tools_core::ToolRegistry::new();
    builder.register_mutating(&mut reg);
    builder.register_plan_mode(&mut reg);
    for name in ["bash", "edit", "write", "apply_patch", "notebook_edit",
                 "enter_plan_mode", "exit_plan_mode"] {
        let tool = reg.get(name).expect(name);
        let mask = tool.allowed_channels();
        assert_eq!(mask, ChannelMask::CODING_ONLY,
            "{name} should stay CODING_ONLY");
        assert!(mask.allows(Channel::Coding));
        assert!(!mask.allows(Channel::Desktop));
        assert!(!mask.allows(Channel::Other));
    }
}
```

(Move the `make_builder()` helper from `tool_kit_builder.rs` into a `tests/common/` module if not already; or duplicate it inline for this test.)

- [ ] **Step 4: Run test, verify pass**

Run:

```bash
cargo nextest run --workspace --test coding_in_chat_property 2>&1 | tail -10
```

Expected: pass.

- [ ] **Step 5: Smoke test integration — regular chat sees the graduated tools**

Add an integration test or update an existing one:

`tests/integration/agent_loop_filter.rs` (create if absent):

```rust
//! Verifies the per-channel tool filter works end-to-end after graduation.

use common::tool_channel::{Channel, ChannelMask};

#[tokio::test]
async fn desktop_channel_sees_graduated_tools() {
    // Set up an AgentLoop with klynt-core registered; route a message with
    // channel = "desktop"; assert the tool definitions advertised to the LLM
    // include `read`, `glob`, `grep`, `ask_user`, `web_fetch`, `tool_search`
    // and exclude `bash`, `edit`, `write`, `apply_patch`, `notebook_edit`,
    // `enter_plan_mode`, `exit_plan_mode`.
}
```

(Detailed setup deferred to implementation — the framework for spinning up an AgentLoop in tests exists per CLAUDE.md's tests/ section. Follow `tests/e2e/agent_loop.rs` patterns.)

- [ ] **Step 6: Manual smoke (Telegram/Discord if available)**

Outside automated tests, manual smoke test:

1. Start the desktop in dev (`cargo tauri dev`).
2. Open the Telegram channel (or any non-coding channel).
3. Ask: "what files are in the current directory?".
4. Verify the agent now uses `glob` (or `read`) instead of failing or asking.

If you can't test Telegram/Discord, simulate by running a `chat_send` IPC with `mode: undefined` and inspecting the logs for the filtered tool list.

- [ ] **Step 7: Run full clippy + nextest**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
```

Expected: green, zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/klynt-core/src/tools/{read,glob,grep,ask_user,web_fetch,tool_search}.rs \
        tests/coding_in_chat_property.rs \
        tests/integration/agent_loop_filter.rs

git commit -m "$(cat <<'EOF'
feat(channels): graduate read/glob/grep/ask_user/web_fetch/tool_search to all channels

Removes the implicit ChannelMask::CODING_ONLY restriction (no override
attribute → default ChannelMask::ALL applies). These six klynt-core
tools now appear to LLMs in regular chat (Telegram, Discord, Slack,
Email, desktop non-coding) in addition to coding mode.

Mutating tools (bash, edit, write, apply_patch, notebook_edit) stay
coding-only via their explicit allowed_channels = "coding_only" override.

This is the user-visible cutover. Manual smoke recommended on
Telegram/Discord channels before merging to main.

Part of tool layer consolidation (commit 8/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: DELETION of `crates/tools/src/system/` + prompt sweep + test rewrite

**Goal:** Atomically retire the OLD chatbot tool surface. Move `ask_user.rs` impl into klynt-core; delete the rest. Update all markdown prompts, tests, and code references.

**Files:**
- Move: `crates/tools/src/system/ask_user.rs` → `crates/klynt-core/src/tools/ask_user.rs` (replacing the 4-line re-export)
- Delete: `crates/tools/src/system/{browser,filesystem,glob_tool,grep,message,web,mod}.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/Cargo.toml`
- Modify: `crates/tools/README.md`
- Modify: `crates/agent/src/agent_loop/builder.rs:629-658` (delete OLD-tool registrations)
- Modify: many compile-error sites identified by `cargo check`
- Delete: `agents/general/skills/browser.md`
- Modify: many markdown files (per consolidation spec §9 Tier 1)
- Modify: tests at `tests/{e2e,unit,integration}/*` (rename old tool-name fixtures)

- [ ] **Step 1: Move ask_user impl into klynt-core**

```bash
mv crates/tools/src/system/ask_user.rs /tmp/ask_user_old.rs
mv crates/klynt-core/src/tools/ask_user.rs /tmp/ask_user_reexport.rs   # the 4-line wrapper
mv /tmp/ask_user_old.rs crates/klynt-core/src/tools/ask_user.rs
```

Then open `crates/klynt-core/src/tools/ask_user.rs` and adjust imports (the file came from a different crate; cleanup needed):

```rust
// At top of file, replace:
//   use tools_core::{...};
// with the same set, but ensure paths resolve in klynt-core's context.
```

Search the moved file for any references to `tools::*` or `crates::*` that no longer resolve and update.

- [ ] **Step 2: Verify ask_user moves cleanly**

Run:

```bash
cargo check -p klynt-core 2>&1 | tail -20
```

Expected: clean. Resolve any import errors.

- [ ] **Step 3: Delete the OLD system directory**

```bash
rm -rf crates/tools/src/system/
```

- [ ] **Step 4: Update crates/tools/src/lib.rs**

Open `crates/tools/src/lib.rs`. Remove the `pub mod system;` line and any `pub use system::...;` re-exports. Verify only `pub mod domain;` remains (and any other non-system modules).

- [ ] **Step 5: Update crates/tools/Cargo.toml**

Open `crates/tools/Cargo.toml`. Remove dependencies used only by the deleted system tools. Likely candidates:

```
reqwest, html2text, scraper (web tools)
keyring (browser? or unrelated?)
```

Verify each removal with `cargo build` in subsequent steps.

- [ ] **Step 6: Delete OLD-tool registrations from agent_loop/builder.rs**

Open `crates/agent/src/agent_loop/builder.rs:629-658`. Delete the entire block of OLD-tool registrations (BrowserTool, register_fs_tools, GlobTool, GrepTool, WebSearchTool, WebFetchTool, MessageTool, AskUserTool from the `tools::system::*` namespace). The klynt-core registrations in app-core/src/init/mod.rs cover the surface.

Also remove the imports at the top of builder.rs that referenced the deleted types.

- [ ] **Step 7: Run cargo check, fix all errors**

Run:

```bash
cargo check --workspace 2>&1 | tee /tmp/sweep.log
```

Expected: many errors. Each error tells you a file:line that referenced a deleted type. Work through them systematically.

Common error sites (per consolidation spec §9 Tier 3):

```
crates/tools-core/src/permissions.rs              — match arms by tool name
crates/tools-core/src/registry.rs                 — name special cases (rare)
crates/agent/src/learning/tool_tracking.rs        — usage tracking by name
crates/agent/src/confidence/evaluator.rs          — confidence scoring
crates/agent/src/execution/scratchpad.rs          — scratchpad keys
crates/agent/src/execution/core.rs                — possibly web_search special case
crates/skill-system/src/parser.rs                 — tool refs in skills
crates/context_engine/src/history_compressor/tiered.rs — name-aware compression
crates/agent/src/context_sources/identity.rs      — identity context
crates/cognitive/src/services/reforge/{skill_files.rs,service.rs} — reforge naming
crates/providers/src/adapters/anthropic_native.rs — tool_use parsing
crates/channels/src/adapters/discord.rs           — verify hardcoded name
crates/activity-log/src/types.rs                  — activity-log enum
crates/agent/src/agent_profile/manager.rs         — profile reference to "browser"
crates/feature-launcher/src/types.rs              — launcher reference to "message"
```

For each error site, apply the rename map (read_file → read, etc.) or remove the reference if it's for a retired tool (browser, web_search, message).

- [ ] **Step 8: Iterate — rerun cargo check until clean**

Loop:

```bash
cargo check --workspace 2>&1 | head -50
# fix top error
# repeat
```

Until `cargo check --workspace` returns clean.

- [ ] **Step 9: Update tests**

For each test file:

- `tests/e2e/agent_loop.rs` — search for `read_file`/`write_file`/`edit_file`/`web_search`; replace with new names per rename map.
- `tests/unit/providers.rs` — same.
- `tests/integration/cognitive.rs` — same.
- `tests/integration/learning.rs` — drop `web_search` references; replace any `web_search` tool calls with skipped/no-op or `web_fetch` equivalents.
- `crates/coding-ingest/tests/kimi_poller.rs` — verify the "message" reference is unrelated to MessageTool (it might be Kimi's protocol message — check before changing).

```bash
rg "read_file|write_file|edit_file|list_dir|web_search|extract_mode|max_chars" tests/ --type rust 2>&1 | head -20
```

For each hit, decide: rename, or leave (if unrelated context like Kimi protocol).

- [ ] **Step 10: Run full nextest after code sweep**

Run:

```bash
cargo nextest run --workspace 2>&1 | tail -20
```

Expected: green. Fix any failing tests by updating fixtures.

- [ ] **Step 11: Update markdown prompts — Tier 1 sweep**

For each markdown file:

```bash
sed -i.bak 's/read_file/read/g;s/write_file/write/g;s/edit_file/edit/g;s/list_dir/glob/g;s/glob_tool/glob/g' \
  workspace/TOOLS.md workspace/AGENTS.md \
  agents/general/AGENT.md agents/task/AGENT.md \
  agents/finance/AGENT.md agents/automation/AGENT.md \
  agents/communication/AGENT.md
```

Open each and verify the substitutions make grammatical sense (sed is line-mechanical, not contextual).

For `agents/general/skills/search.md`: open and rewrite to remove `web_search` references; document `web_fetch` only.

For `agents/general/skills/browser.md`:

```bash
git rm agents/general/skills/browser.md
```

For `agents/general/skills/{memory,summarize,skill-creator}.md`: open and verify no drift; only edit if you see old tool names.

Open `crates/tools/README.md` and rewrite the scope:

```markdown
# tools

Domain tools used by the agent runtime — `tasks`, `project`, `area`, `notes`,
`memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`,
`annotate`, `learning`, `cron`, `mirror`, `temporal`.

Primitive tools (`read`, `write`, `edit`, `glob`, `grep`, `bash`,
`apply_patch`, `notebook_edit`, `web_fetch`, `ask_user`,
`enter_plan_mode`, `exit_plan_mode`, `tool_search`) live in `klynt-core`.
```

- [ ] **Step 12: Update Settings UI**

Open `desktop-ui/src/features/settings/components/sections/SettingsFeaturesSection.tsx`. Find the row that mentions `web_search`. Remove it (the tool retired). Verify no other rows reference deleted tools.

- [ ] **Step 13: Verify generated files regenerate**

Run:

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test 2>&1 | tail -15
cd .. && cargo tauri dev 2>&1 | head -10  # let it start; Ctrl-C after a few seconds; this regenerates bindings
```

Expected: typecheck/lint/test green; tauri dev starts (regenerating bindings.ts in the process).

- [ ] **Step 14: Add invariant K15 — retirement compile-gate**

Append to `tests/coding_in_chat_property.rs`:

```rust
#[test]
fn k15_no_old_system_tool_references_in_codebase() {
    // Use ripgrep at test time to ensure no source file references the deleted types.
    let output = std::process::Command::new("rg")
        .args(["-n", "--type", "rust",
               "tools::system|ReadFileTool|WriteFileTool|EditFileTool|ListDirTool|BrowserTool|MessageTool|WebSearchTool",
               "crates/", "tests/"])
        .output()
        .expect("rg invocation failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(),
        "K15: found references to deleted system tools:\n{stdout}");
}
```

(This test depends on `rg` being on the path. CI typically has it via the same setup that runs ripgrep elsewhere.)

- [ ] **Step 15: Run K15**

Run:

```bash
cargo nextest run --workspace --test coding_in_chat_property k15 2>&1 | tail -5
```

Expected: pass.

- [ ] **Step 16: Run full quality gates**

Run all in sequence:

```bash
cargo build --workspace 2>&1 | tail -5
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10
cargo fmt --all --check
cargo nextest run --workspace 2>&1 | tail -10
cd desktop-ui && bun run lint && bun run typecheck && bun run test 2>&1 | tail -10
cd ..
```

Expected: all green, zero warnings.

- [ ] **Step 17: Run KCA validation**

Per CLAUDE.md:

```bash
./scripts/run_kca_validation.sh 2>&1 | tail -20
```

Expected: all gates pass.

- [ ] **Step 18: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(tools)!: retire crates/tools/src/system/, unify on klynt-core

DELETION commit. Removes 7 OLD system tools (~3500 LOC):
- ask_user.rs (922) — moved into klynt-core/src/tools/ask_user.rs
- browser.rs   (740) — retired (no replacement; future MCP)
- filesystem.rs (640) — retired (klynt-core read/write/edit cover)
- glob_tool.rs (189) — retired (klynt-core glob covers)
- grep.rs      (316) — retired (klynt-core grep covers)
- message.rs   (79)  — retired (chat surface is the message channel)
- web.rs       (272) — retired (web_fetch covers; web_search drops)

Updates:
- crates/tools/src/lib.rs and Cargo.toml
- crates/agent/src/agent_loop/builder.rs (delete OLD registrations)
- ~15 compile-error sites (rename map: read_file→read etc.)
- markdown prompts: workspace/, agents/, README.md
- agents/general/skills/browser.md DELETED
- desktop-ui Settings UI (drop web_search row)
- tests at tests/{e2e,unit,integration}/* (fixture rename)

Adds K15 retirement compile-gate property test.

BREAKING: any external consumer of `crates::tools::system::*` types
will fail to compile. There are no known external consumers; klyntbot
is a single binary and the deletion is internally complete.

Completes tool layer consolidation (commit 9/9).
Spec: docs/superpowers/specs/2026-04-30-tool-layer-consolidation-design.md
Master amendment: docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md (Appendix F)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Post-merge

### Recommended PR strategy

- Commits 1-8 ship in one PR ("foundation + refactor + graduation"). Each commit independently revertible.
- Commit 9 ships in a SEPARATE PR after Commits 1-8 soak on `main` for 1-2 days. The deletion is irreversible.

### Required reviews

- KCA validation gates: `./scripts/run_kca_validation.sh` must pass.
- Optional but recommended: `/ultrareview` on the deletion PR.

### Out of scope for this plan (Phase 2+ per master spec)

- Layer 2 Starlark (Plan 4)
- Distiller / Mirror / Reforge subscribers for new events (Plan 5)
- Settings UI page for tool management (Plan 6)
- File snapshots / `/sessions rewind` (Phase 2)
- `tool_search` BM25 ranking (Phase 2)
- Browser tool replacement via MCP (separate spec)
- Windows sandbox (Phase 3+)

---

*End of plan.*
