# Klynt Tool Layer Consolidation — Design

**Date:** 2026-04-30
**Status:** Draft (pre-implementation)
**Scope:** Single design — completes Phase 1 of the coding-in-chat master spec by retiring the OLD chatbot tool surface and unifying around `klynt-core` as the single source of truth for primitive tools across all chat channels.
**Pre-release policy:** Per CLAUDE.md — no user data to migrate, no backward-compat shims, no feature-flag gating. Schema/registry changes consolidated.
**Companion spec:** [`docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`](./2026-04-29-klynt-coding-in-chat-design.md) — this design amends §3, §6, §7, §10, §13 and Appendices A/B/C/E. See Appendix F of the master spec for the cross-reference summary.
**Supersedes:** Nothing — extends the master design.

---

## 1. Vision, goals, non-goals

### Vision

A single tool registration path: every primitive tool (`read`, `write`, `edit`, `glob`, `grep`, `web_fetch`, `ask_user`, `bash`, `apply_patch`, `notebook_edit`, `enter_plan_mode`, `exit_plan_mode`, `tool_search`) lives in `crates/klynt-core/src/tools/`. Each tool declares its own channel visibility via `Tool::allowed_channels()` returning a `ChannelMask`. Regular chat (Telegram / Discord / Slack / Email / desktop non-coding) and coding chat both consume the same registry — the only difference is *which subset is visible per channel*. The OLD `crates/tools/src/system/` directory retires entirely, taking three orphan tools (`browser`, `web_search`, `message`) with it.

### Goals

- **Single source of truth.** Every primitive tool implementation lives in `crates/klynt-core/src/tools/`. The OLD `crates/tools/src/system/` directory deletes wholesale.
- **Per-tool channel visibility.** Replace the static `CODING_ONLY` const in `coding_channel.rs` with a `Tool::allowed_channels()` trait method returning a `ChannelMask`. Default is `ChannelMask::ALL`; coding-only tools opt-in via a one-line override.
- **No regular-chat regression.** Read-only primitives (`read`, `glob`, `grep`) plus interaction-portable tools (`ask_user`, `web_fetch`, `tool_search`) graduate to all channels. Telegram/Discord users keep the capabilities they have today.
- **Builder-pattern dependency injection.** A `ToolKitBuilder` in `klynt-core` owns the 6-7 `Arc<…>` deps (Layer1, Policy, PrivacyGuard, PendingApprovalsMap, event_tx, DomainEventBus, Repos, HostApprovalCache) and exposes profile-shaped registration methods (`register_read_only`, `register_mutating`, `register_plan_mode`, `register_all`). Both the main agent and sub-agents construct registries through this builder.
- **Per-host approval deduplication** (Codex-derived game-changer). Concurrent `web_fetch` calls to the same `(scheme, host, port)` coalesce onto one user approval prompt; the decision is cached for the session.
- **Channel-aware approval degradation.** Layer1 detects channels that cannot surface approval cards (Telegram, Discord, Slack, Email) and falls back to a configured default for read-network tools. No infinite hangs in headless channels.
- **Live event streaming.** `FileEditWithSymbols` (and `PlanModeChanged`) actually reach the agent stream and the React `DiffRow` — fixes the silent `event_tx: None` data loss at `app-core/src/init/mod.rs:1817`.
- **Each commit green.** 9-commit migration sequence; the deletion is the final atomic step. Every commit before it keeps both code paths alive and reversible.

### Non-goals

- **No new tool kinds.** Anything beyond the 13-tool klynt-core inventory is out of scope (e.g., a hypothetical `js_repl` or browser MCP wrapper).
- **No browser tool replacement.** `BrowserTool` retires; its replacement (MCP browser tools) is a separate, future spec.
- **No web-search tool replacement.** `WebSearchTool` retires; the LLM uses `web_fetch` plus its own reasoning to fetch known URLs.
- **No `MessageTool` replacement.** Outbound dispatch is absorbed into the existing chat surface — the chat composer / channel adapter IS the message channel.
- **No coding-memory / mirror / distiller / reforge changes** beyond what threads through `event_tx`.
- **No Phase 2+ work.** Layer 3 Mirror-learned approval, file snapshots, `tool_search` ranking, sessions export — all unchanged by this spec.
- **No new channel categories.** `Channel` stays at 3 buckets (Coding, Desktop, Other); per-platform discrimination (Telegram-only, Discord-only) deferred until a real use case emerges.

---

## 2. Master spec deltas (where this changes the coding-in-chat design)

| Master section | Change |
|---|---|
| §3 Crate layout | `klynt-core`'s purpose extends from "coding tool kit" to "primitive tool kit for both coding and regular chat". Pool 1 of §6 keeps its name but its visibility is now per-tool, not per-pool. |
| §6 Tool surface | Static `CODING_ONLY` const replaced by `Tool::allowed_channels()` trait method; per-tool override table added. The 24-tool curated profile remains the coding-mode default; regular chat sees ~21 tools (6 graduating klynt-core + 15 domain). |
| §7 Approval | Adds *channel-aware degradation* — Layer1 evaluates `ctx.channel.supports_approval_ui()` and short-circuits to the configured `nonUiChannels` policy. Adds `HostApprovalCache` with `(scheme, host, port)` keying and `AllowOnce` / `AllowForSession` decisions. |
| §10 Event vocabulary | `agent:file_edit_with_symbols` channel becomes live — `event_tx` wiring (today `None` at `app-core/src/init/mod.rs:1817`) is fixed via `AgentRuntime::event_sender()` accessor. |
| §13 Phase 1 | Adds tool-layer consolidation as the final pre-Phase-2 deliverable (the work this spec describes). |
| Appendix A | Adds 4 new locked decisions (see this spec's Appendix C). |
| Appendix B | Adds: rename `coding_channel.rs` → `tool_channel.rs`, retire `crates/tools/src/system/`, add `Tool::allowed_channels()`, add `bitflags` workspace dep. |
| Appendix C | Adds 4 new invariants K12-K15 (this spec's Appendix B). |
| Appendix E | Adds amendment row "2026-04-30 | Tool layer consolidation". |

---

## 3. Channel visibility model — `ChannelMask`

### Type definition

Lives in `crates/common/src/tool_channel.rs` (renamed from `coding_channel.rs` — the "coding" prefix is misleading once visibility is per-tool).

```rust
use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Coding,
    Desktop,
    Other,
}

impl Channel {
    pub fn from_name(name: &str) -> Self {
        match name {
            crate::CODING_CHANNEL => Self::Coding,
            "desktop"             => Self::Desktop,
            _                     => Self::Other,
        }
    }

    /// True for channels that can render approval cards (`kind: "approval"`
    /// ConversationItem). Used by Layer1 to allow read-network tools without
    /// human consent in headless channels.
    pub fn supports_approval_ui(&self) -> bool {
        matches!(self, Self::Coding | Self::Desktop)
    }
}

bitflags! {
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
```

### Trait extension

```rust
// crates/tools-core/src/lib.rs

#[async_trait]
pub trait Tool: Send + Sync {
    // ... existing methods unchanged ...

    /// Channels in which this tool is visible to the LLM. Default = ALL.
    /// Override to restrict — e.g. tools that need approval UI return CODING_ONLY.
    fn allowed_channels(&self) -> common::ChannelMask {
        common::ChannelMask::ALL
    }
}
```

### Filter call site

```rust
// crates/agent/src/agent_loop/mod.rs:889-950 (replace existing
// `available_for_channel(name, channel)` invocation)

let channel = common::Channel::from_name(routing_ctx.channel.as_str());
let filtered_defs: Vec<_> = registry
    .definitions()
    .into_iter()
    .filter(|def| {
        registry
            .get_tool(&def.name)
            .map(|tool| tool.allowed_channels().allows(channel))
            .unwrap_or(true)  // unknown tool ⇒ allow; lookup miss is a registry bug
    })
    .collect();
```

### Per-tool override table

Klynt-core's 13 tools split into two groups by visibility:

| Tool | `allowed_channels()` | Reason |
|------|---------------------|--------|
| `BashTool` | `CODING_ONLY` | sandbox + approval UI required |
| `EditTool` | `CODING_ONLY` | approval UI required |
| `WriteTool` | `CODING_ONLY` | approval UI required |
| `ApplyPatchTool` | `CODING_ONLY` | approval UI required |
| `NotebookEditTool` | `CODING_ONLY` | approval UI required |
| `EnterPlanModeTool` | `CODING_ONLY` | coding-only concept |
| `ExitPlanModeTool` | `CODING_ONLY` | coding-only concept |
| `ReadTool` | (default `ALL`) | read-only, privacy-checked |
| `GlobTool` | (default `ALL`) | read-only, privacy-checked |
| `GrepTool` | (default `ALL`) | read-only, privacy-checked |
| `WebFetchTool` | (default `ALL`) | read-only network with channel-aware approval |
| `AskUserTool` | (default `ALL`) | uses interaction round-trip — works in every channel |
| `ToolSearchTool` | (default `ALL`) | useful everywhere once fully implemented |

Domain tools in `crates/tools/src/domain/` (15 tools — `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror`, `temporal`) inherit the `ALL` default — matches existing behavior.

### Migration delta

- DELETE `pub const CODING_ONLY: &[&str]` (`coding_channel.rs:23-37`).
- DELETE `pub fn available_for_channel(name: &str, channel: Channel) -> bool` (`coding_channel.rs:39`).
- RENAME `crates/common/src/coding_channel.rs` → `crates/common/src/tool_channel.rs`.
- EDIT `crates/common/src/lib.rs` — change `pub mod coding_channel;` to `pub mod tool_channel;`; re-export `pub use tool_channel::{Channel, ChannelMask, CODING_CHANNEL};`.
- ADD `bitflags = "2"` to `crates/common/Cargo.toml`.
- ADD `Tool::allowed_channels()` default-impl method to `tools_core::Tool`.
- ADD `Channel::supports_approval_ui()` helper.
- ADD 7 one-line overrides in klynt-core tool files.
- CHANGE filter site at `agent/src/agent_loop/mod.rs:901-913`.
- GREP-AND-FIX every `use common::coding_channel::*` (or `::Channel` / `::available_for_channel`) across the workspace.

### Test coverage

- Unit: `ChannelMask::allows` for every `Channel` × every preset.
- Property (K12): re-applying the filter to filtered output is idempotent.
- Property: every klynt-core tool's `allowed_channels()` returns a non-empty mask (no tool is invisible everywhere).
- Integration: regular chat sees `read`/`glob`/`grep`/`ask_user`/`web_fetch`/`tool_search` + 15 domain tools (= 21). Coding chat sees the spec-mandated 24-tool curated set.

---

## 4. Approval flow — channel-aware degradation

### Problem

The OLD `WebFetchTool` had no approval gate. The NEW `klynt_core::WebFetchTool` calls `Layer1::evaluate(ctx, "web_fetch", url)` which can return `RequiresApproval` → emits `ApprovalRequested` → waits for `chat_respond_approval`. That event has no listener in Telegram/Discord/Slack/Email; a Telegram fetch hangs the agent loop. Once `web_fetch` graduates to all channels (per §3), this regression is real.

### Model

Layer1's evaluation already takes a `GuardCtx` carrying the channel. Add a single channel-aware degradation step *inside Layer1* — every approval-aware tool benefits, individual tools stay uniform.

```rust
// crates/klynt-execpolicy/src/lib.rs

impl Layer1 {
    pub fn evaluate(&self, ctx: &GuardCtx, tool: &str, target: &str) -> ApprovalDecision {
        let decision = self.evaluate_inner(ctx, tool, target);

        if matches!(decision, ApprovalDecision::RequiresApproval)
            && !ctx.channel.supports_approval_ui()
        {
            return self.non_ui_fallback_for(tool, target);
        }

        decision
    }

    fn non_ui_fallback_for(&self, tool: &str, _target: &str) -> ApprovalDecision {
        match self.config.non_ui_channels {
            NonUiPolicy::Allow         => ApprovalDecision::Allow,
            NonUiPolicy::DenyWithError => ApprovalDecision::Deny {
                reason: format!(
                    "Tool '{tool}' requires approval, but channel does not support \
                     approval UI. Set tools.approvalPolicy.nonUiChannels = \"allow\" \
                     in config.json to permit."
                ),
            },
        }
    }
}
```

Mutating tools (`bash`, `edit`, `write`, `apply_patch`, `notebook_edit`) all have `ChannelMask::CODING_ONLY` so they're invisible in non-UI channels — the fallback only ever fires for `web_fetch` in practice.

### Config

```json
{
  "tools": {
    "approvalPolicy": {
      "nonUiChannels": "allow"
    }
  }
}
```

Type signature in `crates/config/src/schema/tools.rs`:

```rust
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NonUiPolicy {
    Allow,
    DenyWithError,
}

impl Default for NonUiPolicy {
    fn default() -> Self { Self::Allow }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPolicyConfig {
    pub non_ui_channels: NonUiPolicy,
}
```

### Touch points

- ADD `NonUiPolicy` + `ApprovalPolicyConfig` in `crates/config/src/schema/tools.rs`.
- ADD `evaluate` wrapper logic in `crates/klynt-execpolicy/src/lib.rs`.
- The existing `evaluate_inner` keeps current logic verbatim.

### Test coverage

- Unit: Layer1 returns `RequiresApproval` in `Coding`, `Allow` in `Other` when policy=`Allow`, `Deny` in `Other` when policy=`DenyWithError`.
- Property (K14): for every (tool, channel, policy), Layer1's output is one of `{Allow, Deny, RequiresApproval}` — never panics, never hangs.
- Integration: Telegram conversation calls `web_fetch` with `policy=Allow` → succeeds and returns body. Same call with `policy=DenyWithError` → tool returns descriptive error string.

---

## 5. Per-host approval deduplication (Codex-derived)

### Problem

Today every `web_fetch` call independently calls `Layer1::evaluate(...)`. Two concurrent fetches to `https://docs.python.org/3/foo` and `https://docs.python.org/3/bar` produce two `ApprovalRequested` events, two approval cards, two clicks. The LLM commonly parallelizes fetches — "summarize these 5 wiki pages" with 3 sharing `en.wikipedia.org` produces 3 prompts.

### Model (adapted from `codex-rs/core/src/tools/network_approval.rs:140-169`)

```rust
// crates/klynt-core/src/approval/host_cache.rs (new)

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct HostKey {
    pub scheme: String,    // "http" | "https"
    pub host: String,      // lowercased
    pub port: u16,         // explicit or scheme default
}

impl HostKey {
    pub fn from_url(url: &str) -> common::Result<Self> {
        let u = Url::parse(url).map_err(/* ... */)?;
        let host = u.host_str().ok_or_else(/* ... */)?;
        let port = u.port_or_known_default().unwrap_or(0);
        Ok(Self {
            scheme: u.scheme().to_string(),
            host: host.to_ascii_lowercase(),
            port,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostDecision {
    AllowOnce,         // grant for the in-flight call only; cache evicted after
    AllowForSession,   // grant for the rest of the session; cache retained
    Deny,
}

enum HostState {
    Pending(watch::Receiver<Option<HostDecision>>),
    Resolved(HostDecision),
}

#[derive(Clone, Default)]
pub struct HostApprovalCache {
    map: Arc<DashMap<HostKey, HostState>>,
}

pub enum HostCheckResult {
    Cached(HostDecision),
    AwaitPending(watch::Receiver<Option<HostDecision>>),
    NewlyRegistered { tx: watch::Sender<Option<HostDecision>> },
}

impl HostApprovalCache {
    pub fn check_or_register(&self, key: HostKey) -> HostCheckResult { /* atomic */ }
    pub fn resolve(&self, key: HostKey, decision: HostDecision) { /* dispatch + persist */ }
}
```

### Wiring `web_fetch.rs`

```rust
let key = HostKey::from_url(&args.url)?;
let decision = match host_cache.check_or_register(key.clone()) {
    HostCheckResult::Cached(d) => d,
    HostCheckResult::AwaitPending(mut rx) => {
        rx.changed().await.map_err(|_| /* cancelled */)?;
        rx.borrow().expect("decision set on resolution")
    }
    HostCheckResult::NewlyRegistered { tx } => {
        let layer1_decision = evaluate(guard_ctx, "web_fetch", &args.url).await;
        let host_decision = match layer1_decision {
            ApprovalDecision::Allow            => HostDecision::AllowForSession,
            ApprovalDecision::AllowOnce        => HostDecision::AllowOnce,
            ApprovalDecision::Deny { .. }      => HostDecision::Deny,
            ApprovalDecision::RequiresApproval => unreachable!("§4 ensures terminal"),
        };
        tx.send(Some(host_decision)).ok();
        host_cache.resolve(key, host_decision);
        host_decision
    }
};

if decision == HostDecision::Deny {
    return Err(/* permission denied */);
}
// proceed with fetch
```

### UI extension

The `kind: "approval"` ConversationItem (per master spec line 1596) gains two buttons:
- **Allow once** → `HostDecision::AllowOnce`
- **Allow for session** → `HostDecision::AllowForSession`

(`Deny` is the default-on-dismiss action.)

### Lifetime

- Per-session — instance lives on `AgentRuntime`, reset when session ends.
- Per-host, not per-URL — `https://en.wikipedia.org/wiki/A` and `…/wiki/B` share the cache.
- Scheme-distinguishing — `http://` and `https://` to the same host are separate keys (security posture differs).

### Touch points

- CREATE `crates/klynt-core/src/approval/host_cache.rs` (~150 lines).
- MODIFY `crates/klynt-core/src/approval/mod.rs` — re-export.
- MODIFY `crates/klynt-core/src/tools/web_fetch.rs` — replace single `evaluate()` with cache-aware flow.
- MODIFY `crates/klynt-core/src/registry/builder.rs` — add `host_approvals` field; pass to `WebFetchTool::new`.
- EXTEND `klynt_execpolicy::ApprovalDecision` — add `AllowOnce` variant if absent.
- MODIFY `desktop-ui/src/features/coding/components/ApprovalCard.tsx` — Allow-for-session button.

### Test coverage

- Unit: `HostKey::from_url("https://Example.com:443/x")` → key with `host="example.com"`, `port=443`, `scheme="https"`.
- Unit: `check_or_register` first call returns `NewlyRegistered`; second concurrent call returns `AwaitPending`; both resolve to the same decision.
- Property (K13): for any sequence of `(host, decision)` resolutions, the cache state is consistent across N concurrent readers.
- Integration: 5 parallel `web_fetch` calls to 3 distinct hosts → exactly 3 `ApprovalRequested` events.
- Integration: AllowOnce → next fetch to same host produces a new approval event; AllowForSession → no new events.

---

## 6. `ToolKitBuilder` + sub-agent rewiring

### Problem

klynt-core tools have constructor arity 6-7 (`Arc<Layer1>`, `Arc<Policy>`, `Arc<PrivacyGuard>`, `Arc<PendingApprovalsMap>`, `Option<Sender<AgentEvent>>`, `Arc<DomainEventBus>`, `Repos`, `Arc<HostApprovalCache>` per §5). Every call site that constructs tools must thread these through. Sub-agent profiles at `crates/agent/src/subagent.rs:439-460` use simple constructors today (`register_fs_tools`, `GlobTool::new()`, etc.) and cannot supply the deps without restructuring.

### Pattern

```rust
// crates/klynt-core/src/registry/builder.rs (new)

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::ToolRegistry;
use klynt_execpolicy::Layer1;
use klynt_sandbox::Policy;
use crate::privacy::PrivacyGuard;
use crate::approval::{PendingApprovalsMap, HostApprovalCache};
use bus::DomainEventBus;
use storage::Repos;
use agent::events::AgentEvent;

#[derive(Clone)]
pub struct ToolKitBuilder {
    pub cwd: PathBuf,
    pub layer1: Arc<Layer1>,
    pub policy: Arc<Policy>,
    pub privacy: Arc<PrivacyGuard>,
    pub pending: Arc<PendingApprovalsMap>,
    pub event_tx: Option<mpsc::Sender<AgentEvent>>,
    pub bus: Arc<DomainEventBus>,
    pub repos: Repos,
    pub host_approvals: Arc<HostApprovalCache>,
}

impl ToolKitBuilder {
    pub fn with_cwd(self, cwd: PathBuf) -> Self { Self { cwd, ..self } }

    /// Six tools, all default-`ChannelMask::ALL`: read, glob, grep, ask_user, web_fetch, tool_search.
    pub fn register_read_only(&self, reg: &mut ToolRegistry) { /* ... */ }

    /// Five tools, all `ChannelMask::CODING_ONLY`: write, edit, apply_patch, notebook_edit, bash.
    pub fn register_mutating(&self, reg: &mut ToolRegistry) { /* ... */ }

    /// Two tools, `ChannelMask::CODING_ONLY`: enter_plan_mode, exit_plan_mode.
    pub fn register_plan_mode(&self, reg: &mut ToolRegistry) { /* ... */ }

    /// All thirteen.
    pub fn register_all(&self, reg: &mut ToolRegistry) {
        self.register_read_only(reg);
        self.register_mutating(reg);
        self.register_plan_mode(reg);
    }
}
```

### Main agent wiring

Replaces `crates/app-core/src/init/mod.rs:1784-1851`:

```rust
let kit = klynt_core::ToolKitBuilder {
    cwd: config.coding_memory.workspace_root.clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap()),
    layer1: layer1.clone(),
    policy: policy.clone(),
    privacy: privacy.clone(),
    pending: pending_approvals.clone(),
    event_tx: Some(core.agent.event_sender()),    // §8 fix
    bus: bus.clone(),
    repos: repos.clone(),
    host_approvals: host_approvals.clone(),       // §5 fix
};
{
    let mut reg = core.agent.tool_registry().write();
    kit.register_all(&mut reg);
}
core.agent.set_tool_kit(Arc::new(kit));
```

### Sub-agent wiring

Replaces `crates/agent/src/subagent.rs:430-470`:

```rust
#[derive(Clone, Copy, Debug)]
pub enum SubAgentProfile { ReadOnly, ReadWrite, Full }

pub fn build_subagent_registry(
    parent: &AgentRuntime,
    profile: SubAgentProfile,
    cwd: Option<PathBuf>,
) -> common::Result<ToolRegistry> {
    let kit = parent.tool_kit()
        .ok_or_else(|| KlyntbotError::Config("ToolKit not initialized".into()))?;
    let kit = match cwd {
        Some(c) => Arc::new((*kit).clone().with_cwd(c)),
        None    => kit,
    };
    let mut reg = ToolRegistry::new();
    register_domain_tools_for_subagent(&mut reg, &parent.repos());
    match profile {
        SubAgentProfile::ReadOnly  => kit.register_read_only(&mut reg),
        SubAgentProfile::ReadWrite => { kit.register_read_only(&mut reg); kit.register_mutating(&mut reg); }
        SubAgentProfile::Full      => kit.register_all(&mut reg),
    }
    Ok(reg)
}
```

### `AgentRuntime` accessor

```rust
// crates/agent/src/agent_runtime/runtime.rs

pub(crate) tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,

pub fn tool_kit(&self) -> Option<Arc<klynt_core::ToolKitBuilder>> { self.tool_kit.clone() }
pub fn set_tool_kit(&mut self, kit: Arc<klynt_core::ToolKitBuilder>) { self.tool_kit = Some(kit); }
```

### Touch points

- CREATE `crates/klynt-core/src/registry/builder.rs` (~120 lines).
- MODIFY `crates/klynt-core/src/lib.rs` — re-export `ToolKitBuilder`.
- MODIFY `crates/klynt-core/Cargo.toml` — add deps if missing (`agent`, `storage`, `bus`, `klynt-execpolicy`, `klynt-sandbox`).
- MODIFY `crates/agent/src/agent_runtime/runtime.rs` — `tool_kit` field + accessors.
- REWRITE `crates/agent/src/subagent.rs:430-470`.
- REWRITE `crates/app-core/src/init/mod.rs:1784-1851`.

### Test coverage

- Unit: `ToolKitBuilder::register_read_only` registers exactly 6 names; `register_mutating` exactly 5; `register_plan_mode` exactly 2; `register_all` exactly 13.
- Property: `parent.tool_kit().is_some()` ⇒ every sub-agent profile builds without panicking.
- Integration: spawn a `ReadOnly` sub-agent in coding mode → only the 6 read-only tools advertised. Spawn `Full` → 13 + domain.

---

## 7. Param-shape ports

Two OLD parameters that the LLM was prompted to use have no klynt-core equivalent. Port them.

### Decisions

| OLD param | NEW (today) | Decision | Reason |
|---|---|---|---|
| `grep.context_lines` (Option\<i64\> 0-5) | absent | **PORT** as `Option<u8>` clamped to 0-5 | Real productivity feature; `ripgrep -C N` parity |
| `glob.path` (Option\<String\>) | absent (cwd-fixed) | **PORT** with privacy enforcement | Multi-root workflows; PrivacyGuard validates |
| `web_fetch.extract_mode` | already `format` ("text"/"raw") | **NO CHANGE** | klynt-core already has the better names |
| `web_fetch.max_chars` | already `max_bytes` | **NO CHANGE** | klynt-core already has the better names |

### grep.context_lines impl sketch

```rust
// crates/klynt-core/src/tools/grep.rs (additive)

#[derive(Debug, Deserialize, ToolParams)]
pub struct GrepArgs {
    pub pattern: String,
    pub include: Option<String>,
    pub case_insensitive: Option<bool>,
    pub context_lines: Option<u8>,  // 0-5; clamped
}

// Search loop emits, for each match at line `i` in `lines`:
let ctx = args.context_lines.unwrap_or(0).min(5) as usize;
if ctx == 0 {
    out.push_str(&format!("{path}:{}: {}\n", i + 1, lines[i]));
} else {
    let lo = i.saturating_sub(ctx);
    let hi = (i + ctx + 1).min(lines.len());
    for j in lo..hi {
        let marker = if j == i { ":" } else { "-" };
        out.push_str(&format!("{path}{marker}{}{} {}\n", j + 1, marker, lines[j]));
    }
    if !was_adjacent_to_prev { out.push_str("--\n"); }
}
```

### glob.path impl sketch

```rust
// crates/klynt-core/src/tools/glob.rs (additive)

#[derive(Debug, Deserialize, ToolParams)]
pub struct GlobArgs {
    pub pattern: String,
    pub path: Option<String>,
}

let root = match args.path.as_deref() {
    Some(p) => {
        let p = shared::fs_resolve::expand_and_canonicalize(p)?;
        if self.privacy.is_excluded(&p) {
            return Err(KlyntbotError::Privacy(format!("glob path '{p:?}' is excluded")));
        }
        p
    }
    None => self.cwd.clone(),
};
```

### Test coverage

- Unit: `grep` with `context_lines: 2` on a 10-line file with 1 match at line 5 → emits lines 3-7 with proper markers.
- Unit: `grep` with `context_lines: 10` → clamped to 5 (no error).
- Unit: `glob` with `path: "/etc"` → returns `KlyntbotError::Privacy`.
- Unit: `glob` with `path: "~/proj"` → expands, walks if not excluded.
- Property: `grep(p, ctx_lines)` output ⊇ `grep(p, 0)` output for any p, ctx_lines.

---

## 8. Event channel wiring (`event_tx`-is-None fix)

### Problem

`crates/app-core/src/init/mod.rs:1817` wires `event_tx: None` for every klynt-core tool. Tools call `event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0)` (see `web_fetch.rs:68`) — they create a throwaway sender and the events vanish silently. `FileEditWithSymbols` is constructed but never reaches the streaming relay or the React `DiffRow`.

### Chain

```
[klynt-core tool]  ───►  [agent runtime]  ───►  [app-core relay]  ───►  [Tauri]  ───►  [React]
   emit_file_edit         mpsc<AgentEvent>       match relay              app.emit         useFileEditEvents
   (write/edit/etc)        (existing channel)     (route variant)          ("agent:file_     (subscribes)
                                                                            edit_with_       ─► chatStreamStore
                                                                            symbols")          .fileEditsBySession
                                                                                              ─► DiffRow renders
                                                                                                PierreDiffBlock
```

### Fix

```rust
// crates/agent/src/agent_runtime/runtime.rs (additive accessor)

impl AgentRuntime {
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }
}
```

```rust
// crates/app-core/src/init/mod.rs:1817 — replace None
let agent_event_tx = core.agent.event_sender();
// ... in ToolKitBuilder construction (per §6):
event_tx: Some(agent_event_tx),
```

### Streaming relay

Add (or verify present per PR #40) the `FileEditWithSymbols` and `PlanModeChanged` arms in `crates/app-core/src/streaming/relay.rs`:

```rust
AgentEvent::FileEditWithSymbols { session_id, path, op, bytes_delta, hunks, .. } => {
    let payload = serde_json::json!({
        "sessionId": session_id, "path": path, "op": op,
        "bytesDelta": bytes_delta, "hunks": hunks,
    });
    let _ = app.emit("agent:file_edit_with_symbols", payload);
}
AgentEvent::PlanModeChanged { session_id, mode } => {
    let _ = app.emit("agent:plan_mode_changed",
        serde_json::json!({ "sessionId": session_id, "mode": mode }));
}
```

### React listener + store slice

```typescript
// desktop-ui/src/features/coding/hooks/useFileEditEvents.ts

export function useFileEditEvents(sessionId: string) {
    const upsertFileEdit = useChatStreamStore((s) => s.upsertFileEdit);
    useEffect(() => {
        const p = listen<FileEditPayload>("agent:file_edit_with_symbols", (e) => {
            if (e.payload.sessionId !== sessionId) return;
            upsertFileEdit(sessionId, {
                kind: "diff", path: e.payload.path, op: e.payload.op,
                bytes: e.payload.bytesDelta, hunks: e.payload.hunks,
            });
        });
        return () => { p.then((u) => u()); };
    }, [sessionId, upsertFileEdit]);
}
```

### Touch points

- MODIFY `crates/agent/src/agent_runtime/runtime.rs` — `event_sender()` accessor.
- MODIFY `crates/app-core/src/init/mod.rs:1817` — `Some(...)` instead of `None`.
- VERIFY-OR-ADD `crates/app-core/src/streaming/relay.rs` — `FileEditWithSymbols` + `PlanModeChanged` arms.
- VERIFY-OR-ADD `desktop-ui/src/features/coding/hooks/useFileEditEvents.ts`.
- VERIFY-OR-ADD `desktop-ui/src/features/chat/store/chatStreamStore.ts` — `fileEditsBySession` slice.
- VERIFY `desktop-ui/src/features/chat/components/ChatThread.tsx` — calls `useFileEditEvents(sessionKey)`.

### Test coverage

- Unit: `AgentRuntime::event_sender()` returns a usable sender; receiver gets the event.
- Integration: `EditTool::execute(args)` emits exactly one `FileEditWithSymbols` for a real edit; zero for a no-op edit.
- Vitest: hook upserts to store on mock event; `DiffRow` renders.
- E2E manual: in coding mode, ask agent to "rewrite README.md to mention Tauri 2"; verify diff card appears.

---

## 9. Lexical migration (prompts + tests + code)

### Rename map

| OLD | NEW |
|---|---|
| `read_file` | `read` |
| `write_file` | `write` |
| `edit_file` | `edit` |
| `list_dir` | (use `glob` with pattern `**`) |
| `glob_tool` | `glob` |
| `web_search` | retired |
| `browser` | retired |
| `message` | retired |
| `extract_mode` | `format` |
| `max_chars` | `max_bytes` |

### Three-tier sweep

**Tier 1 — LLM-facing markdown (must update lexically):**

| File | Action |
|---|---|
| `workspace/TOOLS.md` | rewrite tool catalog |
| `workspace/AGENTS.md` | grep + replace |
| `agents/general/AGENT.md` | grep + replace |
| `agents/task/AGENT.md` | grep + replace |
| `agents/finance/AGENT.md` | grep + replace |
| `agents/automation/AGENT.md` | grep + replace |
| `agents/communication/AGENT.md` | grep + replace |
| `agents/general/skills/search.md` | drop `web_search`; document `web_fetch` only |
| `agents/general/skills/browser.md` | DELETE |
| `agents/general/skills/memory.md` | verify no drift |
| `agents/general/skills/summarize.md` | verify no drift |
| `agents/general/skills/skill-creator.md` | verify no drift |
| `crates/tools/README.md` | rewrite scope to "domain tools only" |

**Note:** `crates/skill-system/src/soul.rs::DEFAULT_SOUL` and `skills/*/SKILL.md` (the 5 orchestrator skills) do **not** mention old tool names — verified by grep. No edits required there.

**Tier 2 — Source files that retire (deletion):**

```
crates/tools/src/system/ask_user.rs       (922) — moved into klynt-core
crates/tools/src/system/browser.rs        (740) — DELETE
crates/tools/src/system/filesystem.rs     (640) — DELETE
crates/tools/src/system/glob_tool.rs      (189) — DELETE
crates/tools/src/system/grep.rs           (316) — DELETE
crates/tools/src/system/message.rs        (79)  — DELETE
crates/tools/src/system/web.rs            (272) — DELETE
crates/tools/src/system/mod.rs            ___   — DELETE
```

After deletion:
- EDIT `crates/tools/src/lib.rs` — remove `pub mod system;` + re-exports.
- EDIT `crates/tools/Cargo.toml` — drop deps used only by `system/*`.
- The `ask_user.rs` source moves to `crates/klynt-core/src/tools/ask_user.rs` (replacing the 4-line re-export wrapper).

**Tier 3 — Compiler-driven sweep (cargo check enumerates):**

Expected error sites (from grep evidence):

```
crates/tools-core/src/permissions.rs              — permission-by-name match arms
crates/tools-core/src/registry.rs                 — name special cases
crates/agent/src/agent_loop/builder.rs:629-658    — DELETE OLD-tool registrations
crates/agent/src/learning/tool_tracking.rs        — usage tracking by name
crates/agent/src/confidence/evaluator.rs          — confidence scoring by name
crates/agent/src/execution/scratchpad.rs          — scratchpad keys
crates/agent/src/execution/core.rs                — possibly web_search special case
crates/skill-system/src/parser.rs                 — tool refs in skill bodies
crates/context_engine/src/history_compressor/tiered.rs — name-aware compression
crates/agent/src/context_sources/identity.rs      — identity context
crates/cognitive/src/services/reforge/skill_files.rs — reforge naming
crates/cognitive/src/services/reforge/service.rs  — reforge synthesis
crates/providers/src/adapters/anthropic_native.rs — tool_use parsing
crates/channels/src/adapters/discord.rs           — verify hardcoded name
crates/activity-log/src/types.rs                  — activity-log enum
crates/agent/src/agent_profile/manager.rs         — profile reference to "browser"
crates/feature-launcher/src/types.rs              — launcher reference to "message"
desktop-ui/src/features/settings/components/sections/SettingsFeaturesSection.tsx — settings UI list
```

**Tier 4 — Tests (must update fixtures):**

```
tests/e2e/agent_loop.rs
tests/unit/providers.rs
tests/integration/cognitive.rs
tests/integration/learning.rs
crates/coding-ingest/tests/kimi_poller.rs
```

**Tier 5 — Generated files (auto-regenerate, no hand edit):**

```
crates/desktop-ui/src/bindings.ts
desktop-ui/src/bindings.ts
```

After edits, run `cargo tauri dev` once to regenerate.

### False-positive list (do NOT touch — `max_chars` is a generic helper, not web_fetch's old param):

```
crates/cognitive/src/services/session_memory.rs
crates/coding-memory/src/reforge/session_end.rs
crates/common/src/helpers.rs
crates/context_engine/src/history_compressor/snippet.rs
crates/context_engine/src/insight_forge/note_tree_navigator.rs
crates/channels/src/adapters/email.rs
crates/agent/src/context_sources/bootstrap.rs
```

---

## 10. Migration sequencing

### Phasing

```
Phase A — Foundation (additive only, no UX change)
  Commit 1: ChannelMask + Tool::allowed_channels() + per-tool overrides + rename to tool_channel.rs
  Commit 2: Channel-aware approval policy (Layer1 wrapper + config schema)
  Commit 3: HostApprovalCache (per-host dedup; Codex game-changer)
  Commit 4: event_tx wiring (FileEditWithSymbols actually reaches UI)

Phase B — Builder + sub-agent (refactor only, observable behavior unchanged)
  Commit 5: ToolKitBuilder + main agent rewiring at app-core/src/init/mod.rs
  Commit 6: Sub-agent rewiring (subagent.rs uses ToolKitBuilder)
  Commit 7: Param-shape ports (grep.context_lines, glob.path)

Phase C — Cutover (the user-visible change)
  Commit 8: Tool graduation (read/glob/grep/web_fetch/ask_user/tool_search → ChannelMask::ALL)
  Commit 9: DELETION of crates/tools/src/system/ + prompt sweep + test rewrite
```

### Per-commit summary

| # | Title | LOC est. | Behavior change | Test gate |
|---|---|---|---|---|
| 1 | ChannelMask foundation | ~250 | None | `cargo nextest run -p common -p tools-core -p klynt-core` |
| 2 | Channel-aware approval | ~120 | None | `cargo nextest run -p klynt-execpolicy -p config` |
| 3 | HostApprovalCache | ~250 | UX win on parallel `web_fetch` | `cargo nextest run -p klynt-core` |
| 4 | event_tx wiring | ~150 | Diffs visible in coding chat | `cargo nextest run --workspace` + `bun run test` |
| 5 | ToolKitBuilder + main agent | ~350 | None | `cargo nextest run --workspace` |
| 6 | Sub-agent rewiring | ~200 | None | `cargo nextest run -p agent` |
| 7 | grep/glob param ports | ~80 | New params usable | `cargo nextest run -p klynt-core` |
| 8 | Tool graduation | ~30 | **Regular chat gains 6 klynt-core tools** | `cargo nextest run -p agent`; manual smoke |
| 9 | DELETION + sweep | ~−3500 +200 | OLD retires | `cargo nextest run --workspace` + `bun run lint typecheck test`; KCA gates |

Total: net ~−1800 LOC.

### Rollback strategy

Each commit independently revertible until #9. Recommend Commit 9 ship in its own PR after Commits 1-8 soak on `main`.

### KCA + ultrareview

Per CLAUDE.md, before merging the deletion PR:
- `./scripts/run_kca_validation.sh` (Klynt Cognitive Architecture gates)
- Recommended: `/ultrareview` on the deletion PR (broadest blast radius)

---

## 11. Testing, invariants, gates

### New invariants (K12-K15)

- **K12 — ChannelMask filter idempotence**: For any (registry, channel), `filter(filter(R, c)) == filter(R, c)`. Property test in `tests/coding_in_chat_property.rs`.
- **K13 — Host approval dedup correctness**: For N concurrent `web_fetch` calls hitting M unique `(scheme, host, port)` tuples, exactly M `ApprovalRequested` events are emitted. Property test in `tests/coding_in_chat_property.rs`.
- **K14 — Channel-aware approval safety**: For every (tool with `ChannelMask` allowing non-coding channels) × (channel where `!supports_approval_ui`) × (`NonUiPolicy::Allow`), the tool is read-only or read-network — never mutating. Property test enumerating klynt-core tool inventory.
- **K15 — Retirement compile-gate**: After Commit 9, no reference to `crates::tools::system` or any of {`ReadFileTool`, `WriteFileTool`, `EditFileTool`, `ListDirTool`, `BrowserTool`, `MessageTool`, `WebSearchTool`} exists in the workspace. Enforced by `rg` check in CI.

### Quality gates (every commit)

| Gate | Check |
|---|---|
| Compilation | `cargo build --workspace` |
| Lint | `cargo clippy --workspace --all-targets --all-features` zero warnings |
| Format | `cargo fmt --all --check` |
| Tests | `cargo nextest run --workspace` |
| Frontend | `cd desktop-ui && bun run lint && bun run typecheck && bun run test` |
| K-invariants | `cargo nextest run --test coding_in_chat_property` |

### Quality gates (Commit 9 only)

| Gate | Check |
|---|---|
| KCA validation | `./scripts/run_kca_validation.sh` |
| K15 retirement | `rg "tools::system|ReadFileTool|WriteFileTool|EditFileTool|ListDirTool|BrowserTool|MessageTool|WebSearchTool"` returns nothing under `crates/` |
| Manual smoke | Telegram + coding-mode both work |

---

## Appendix A — Surgical changes vs master spec

### `crates/common/`

```
RENAME crates/common/src/coding_channel.rs → crates/common/src/tool_channel.rs
EDIT   crates/common/src/lib.rs                 (mod rename + re-exports)
EDIT   crates/common/Cargo.toml                  (+ bitflags = "2")
DELETE pub const CODING_ONLY: &[&str]            (in tool_channel.rs)
DELETE pub fn available_for_channel(...)         (in tool_channel.rs)
ADD    pub struct ChannelMask                    (bitflags)
ADD    impl Channel { fn supports_approval_ui }
```

### `crates/tools-core/`

```
ADD    fn allowed_channels(&self) -> ChannelMask  (default impl in Tool trait)
```

### `crates/tools/`

```
DELETE crates/tools/src/system/                  (entire directory)
EDIT   crates/tools/src/lib.rs                   (remove pub mod system; + re-exports)
EDIT   crates/tools/Cargo.toml                   (drop reqwest, html2text, others)
EDIT   crates/tools/README.md                    (rewrite scope: domain tools only)
```

### `crates/klynt-core/`

```
CREATE crates/klynt-core/src/registry/builder.rs   (ToolKitBuilder)
CREATE crates/klynt-core/src/approval/host_cache.rs (HostApprovalCache)
EDIT   crates/klynt-core/src/lib.rs                 (re-export builder + cache)
EDIT   crates/klynt-core/src/registry/mod.rs        (no longer just re-exports common)
EDIT   crates/klynt-core/src/approval/mod.rs        (re-export cache)
EDIT   crates/klynt-core/src/tools/mod.rs           (move ask_user impl in)
EDIT   crates/klynt-core/src/tools/ask_user.rs      (replace re-export with full impl)
EDIT   crates/klynt-core/src/tools/web_fetch.rs     (host-cache wiring + new constructor)
EDIT   crates/klynt-core/src/tools/grep.rs          (+ context_lines param)
EDIT   crates/klynt-core/src/tools/glob.rs          (+ path param + privacy)
EDIT   crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,plan_mode}.rs
                                                    (+ allowed_channels override)
EDIT   crates/klynt-core/Cargo.toml                  (deps: agent, storage, bus, klynt-execpolicy,
                                                     klynt-sandbox, dashmap, url)
```

### `crates/klynt-execpolicy/`

```
EDIT   crates/klynt-execpolicy/src/lib.rs            (evaluate wrapper for channel-aware degradation)
EDIT   crates/klynt-execpolicy/src/decision.rs       (+ AllowOnce variant if absent)
```

### `crates/config/`

```
ADD    crates/config/src/schema/tools.rs             (NonUiPolicy + ApprovalPolicyConfig)
EDIT   crates/config/src/schema/mod.rs               (re-export)
EDIT   crates/config/src/lib.rs                      (Config struct gains tools.approvalPolicy)
```

### `crates/agent/`

```
EDIT   crates/agent/src/agent_runtime/runtime.rs     (+ tool_kit field, event_sender, tool_kit accessor)
EDIT   crates/agent/src/agent_loop/mod.rs:889-950    (filter rewrite using Tool::allowed_channels)
EDIT   crates/agent/src/agent_loop/builder.rs:629-658 (DELETE OLD-tool registrations)
REWRITE crates/agent/src/subagent.rs:430-470         (ToolKitBuilder usage + SubAgentProfile)
```

### `crates/app-core/`

```
REWRITE crates/app-core/src/init/mod.rs:1784-1851   (ToolKitBuilder; event_tx Some; host_approvals)
VERIFY crates/app-core/src/streaming/relay.rs       (FileEditWithSymbols + PlanModeChanged arms)
```

### `desktop-ui/`

```
ADD/VERIFY desktop-ui/src/features/coding/hooks/useFileEditEvents.ts
ADD/VERIFY desktop-ui/src/features/chat/store/chatStreamStore.ts (fileEditsBySession slice)
EDIT       desktop-ui/src/features/coding/components/ApprovalCard.tsx (Allow-for-session button)
EDIT       desktop-ui/src/features/settings/components/sections/SettingsFeaturesSection.tsx (drop web_search)
VERIFY     desktop-ui/src/features/chat/components/ChatThread.tsx (calls useFileEditEvents)
```

### Markdown files

```
EDIT   workspace/TOOLS.md, workspace/AGENTS.md
EDIT   agents/general/AGENT.md, agents/task/AGENT.md, agents/finance/AGENT.md,
       agents/automation/AGENT.md, agents/communication/AGENT.md
EDIT   agents/general/skills/search.md, agents/general/skills/{memory,summarize,skill-creator}.md
DELETE agents/general/skills/browser.md
```

### Tests

```
EDIT tests/e2e/agent_loop.rs
EDIT tests/unit/providers.rs
EDIT tests/integration/cognitive.rs
EDIT tests/integration/learning.rs
EDIT crates/coding-ingest/tests/kimi_poller.rs
```

---

## Appendix B — New invariants (K12-K15) detail

### K12 — ChannelMask filter idempotence

**Statement**: For any `ToolRegistry R` and `Channel c`, `filter(filter(R, c), c) == filter(R, c)`. The filter is a pure function of (tool, channel) and applying it twice yields the same set.

**Why**: prevents subtle bugs where filtering is partially applied or where mask state could leak between iterations.

**Test**: proptest in `tests/coding_in_chat_property.rs`. Generates arbitrary registries (subset of {all 13 klynt-core + 15 domain}) × arbitrary channels, asserts equality.

### K13 — Host approval dedup correctness

**Statement**: For N concurrent `web_fetch` calls partitioned across M unique `(scheme, host, port)` tuples, exactly M `ApprovalRequested` events are emitted, and all N calls receive the same `HostDecision` per tuple.

**Why**: directly verifies the Codex game-changer; a regression here is the original bug we're fixing.

**Test**: proptest with arbitrary (N, M) up to 32 concurrent calls. Spawns N futures, counts `ApprovalRequested` events, asserts == M.

### K14 — Channel-aware approval safety

**Statement**: For every klynt-core tool T such that `T.allowed_channels()` includes a non-coding channel (`Desktop` or `Other`), AND that tool calls `Layer1::evaluate`, T must be classified read-only or read-network. No tool with mutating effects is ever allowed in a non-UI channel under `NonUiPolicy::Allow`.

**Why**: the channel-aware degradation is safe only if `CODING_ONLY` correctly tags every mutating tool. If a future contributor adds a mutating tool with default `ChannelMask::ALL` and calls Layer1, the degradation would auto-allow destructive operations in Telegram. K14 is the compile-time / proptest tripwire.

**Test**: proptest enumerates klynt-core tool inventory; for each tool whose mask allows non-coding, asserts the tool is in a known-safe set (`{read, glob, grep, ask_user, web_fetch, tool_search}`).

### K15 — Retirement compile-gate

**Statement**: After Commit 9, no Rust source file under `crates/` references any of: `tools::system`, `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `ListDirTool`, `BrowserTool`, `MessageTool`, `WebSearchTool`.

**Why**: prevents accidental re-introduction or zombie code references.

**Test**: CI step `rg "tools::system|ReadFileTool|WriteFileTool|EditFileTool|ListDirTool|BrowserTool|MessageTool|WebSearchTool" crates/ && exit 1 || exit 0`.

---

## Appendix C — Locked decisions (additive to master Appendix A)

| # | Axis | Decision |
|---|---|---|
| 13 | Tool source of truth | `klynt-core` is the sole crate hosting primitive tools. `crates/tools/src/domain/` continues to host domain tools (tasks/notes/memory/etc.). `crates/tools/src/system/` retires. |
| 14 | Channel visibility | Per-tool via `Tool::allowed_channels() -> ChannelMask`. Static `CODING_ONLY` const deleted. |
| 15 | Approval in headless channels | Channel-aware degradation in `Layer1::evaluate`; `Channel::supports_approval_ui()` discriminates. Default `NonUiPolicy::Allow` for read-network tools. |
| 16 | Network approval coalescing | `HostApprovalCache` keyed by `(scheme, host, port)`; `AllowOnce` / `AllowForSession` / `Deny` semantics matching Codex's `network_approval.rs:140-169`. |
| 17 | Builder pattern | `klynt_core::ToolKitBuilder` owns deps; main agent + sub-agents call `register_*` methods. |

---

## Appendix D — Cross-spec amendment list (master spec amendments)

### `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` amendments

1. **§3 Crate layout** — Update `klynt-core` row's "Purpose" column from "Coding tool registry" to "Primitive tool registry (coding + regular chat); sandbox/approval glue; slash dispatch". Add note pointing to this spec for the tool layer consolidation details.
2. **§3 Required surgical changes** — Add three rows: rename `coding_channel.rs` → `tool_channel.rs`, add `Tool::allowed_channels()`, retire `crates/tools/src/system/`.
3. **§6 Tool surface** — Replace the "channel filter" subsection with a pointer to this spec's §3 (ChannelMask). Update Pool 1 paragraph to clarify per-tool visibility.
4. **§7 Approval** — Add subsection "Channel-aware degradation" pointing to this spec's §4. Add subsection "Per-host deduplication" pointing to this spec's §5.
5. **§10 Event vocabulary** — Note that `agent:file_edit_with_symbols` is wired live as part of this consolidation (closes the `event_tx: None` gap at `app-core/src/init/mod.rs:1817`).
6. **§13 Phase 1 deliverables** — Add row: "Tool layer consolidation (this spec) — completes Phase 1 by retiring `crates/tools/src/system/` and unifying around `klynt-core`. 9-commit migration."
7. **Appendix A** — Add 5 rows (decisions #13-#17 from this spec's Appendix C).
8. **Appendix B** — Add bullet for `bitflags = "2"` workspace dep, `Tool::allowed_channels()`, retirement of `crates/tools/src/system/`.
9. **Appendix C** — Add invariants K12-K15 to the "From this spec §14" list (renumber as needed; Phase 2 invariants K10-K11 already there).
10. **Appendix E** — Add row: "2026-04-30 | Amendment 4: tool layer consolidation; spec at `docs/superpowers/specs/2026-04-30-tool-layer-consolidation-design.md`".
11. **Appendix F** (new) — One-page summary of the consolidation amendments. See master-spec amendment for full text.

---

*End of design.*
