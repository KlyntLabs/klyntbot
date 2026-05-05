# Unified Permission Gate — Design

**Date:** 2026-05-05
**Status:** Draft (pre-implementation)
**Scope:** Replace Klynt's two partial permission systems (coding-mode 3-layer guard + latent `PermissionLevel`) with a single pre-tool-execution gate that works across every mode and channel.

---

## 1. Motivation

Klynt currently has two disjoint permission mechanisms:

1. **Coding-mode 3-layer guard** (`crates/klynt-core/src/approval/guard.rs`) — runtime classification (`Privacy/Layer1/Layer2/Layer3`), persistent per-session grants, modal UI via `approval_respond`. Only fires for coding-mode tool calls.
2. **`PermissionLevel::{Safe, Elevated, Admin}`** on `Tool` metadata in `tools-core` — declarative, but no enforcement path. Latent.

Non-coding agent tools (`tasks`, `notes`, `finance`, `memory`, `okr`, `productivity`, `learning`, etc.) have *no* gate today. A model can delete every note or wipe finance history without confirmation. We have not released; this is the right window to unify.

## 2. Goals

- One pre-execution gate covering every mode (assistant, coding) and every channel (desktop, Telegram, Discord, Slack, Email, MCP).
- One enum (`ApprovalClass`) describing tool risk; one trait extension on `Tool`.
- One persistent grants table; the coding guard's grant logic generalized.
- Asymmetric per-channel UX (desktop modal, remote inline buttons, MCP structured error) without per-class duplication.
- No data migration burden — all changes pre-release.

## 3. Non-Goals

- **Not** a full RBAC / multi-user permission system. Klynt is single-user; "permission" here means "the user confirms before the agent does this thing."
- **Not** a sandboxing mechanism. The gate decides *whether* a call runs; it does not constrain *what* a call does once it runs.
- **Not** a replacement for `PreToolUse` hooks. Hooks remain the lower-level, customizable layer; the gate sits above them.

## 4. Architecture

### 4.1 Layering

```
┌────────────────────────────────────────────────────────┐
│ ExecutionCore::run_cycle  (crates/agent/src/execution) │
└──────────────┬─────────────────────────────────────────┘
               │ for each tool call
               ▼
┌────────────────────────────────────────────────────────┐
│ ApprovalGate::check(tool, args, ctx)  ← NEW central    │
│   1. classify → ApprovalClass                          │
│   2. compute scope key                                 │
│   3. lookup grants table (exact tuple match)           │
│   4. if no grant: route to ApprovalChannel             │
│   5. record decision, persist if lifetime > Once       │
└──────────────┬─────────────────────────────────────────┘
               │ Allowed
               ▼
        PreToolUse hooks → Tool::execute()
```

The gate is a **single function** invoked once per tool call from `ExecutionCore`, before the existing `PreToolUse` hook dispatch. This replaces the bespoke call site in coding-mode.

### 4.2 Crate placement

Gate machinery lives in a new crate `crates/approval/` at L3 (depends on `tools-core`, `storage`, `bus`; no agent/coding deps). The coding guard's existing types in `crates/klynt-core/src/approval/` are **moved** here verbatim and then refactored. Remote-channel adapter trait (`ApprovalChannel`) lives at the same layer; per-channel impls live in their respective `channels` crates.

### 4.3 Coding-mode integration

The coding 3-layer guard becomes `CodingApprovalPolicy` — an implementation of a `ClassifyHook` trait the gate consults *only* when the active mode is `coding` and the active tool's classification depends on shell/path inspection. For coding tools that don't need runtime classification (most domain tools wired into coding mode), the trait's default suffices.

## 5. Data Model

### 5.1 Tool-side API

```rust
// In tools-core
pub enum ApprovalClass { Safe, Sensitive, Destructive, Admin }

pub enum ApprovalScope {
    ToolAction,                       // grant covers all calls to (tool, action)
    ToolActionResource(String),       // grant covers (tool, action, resource_key)
}

pub trait Tool {
    // ... existing items ...

    const DEFAULT_APPROVAL: ApprovalClass = ApprovalClass::Safe;

    fn approval_class(&self, _args: &Value) -> ApprovalClass {
        Self::DEFAULT_APPROVAL
    }

    fn approval_scope(&self, _args: &Value) -> ApprovalScope {
        ApprovalScope::ToolAction
    }
}
```

Most tools declare `DEFAULT_APPROVAL` and stop. Tools with destructive variants override `approval_class`. Tools that benefit from per-resource grants (coding `bash`, `edit`; possibly `finance.transaction.delete`) override `approval_scope`.

### 5.2 Grants table

```sql
CREATE TABLE approval_grants (
    id              INTEGER PRIMARY KEY,
    class           TEXT NOT NULL,    -- 'safe' | 'sensitive' | 'destructive' | 'admin'
    tool_name       TEXT NOT NULL,
    action          TEXT,             -- nullable for single-action tools
    resource_key    TEXT,             -- nullable when scope = ToolAction
    lifetime        TEXT NOT NULL,    -- 'session' | 'forever'   (Once is never persisted)
    session_id      TEXT,             -- nullable when lifetime = forever
    granted_at      INTEGER NOT NULL,
    expires_at      INTEGER,          -- optional explicit TTL
    UNIQUE (class, tool_name, action, resource_key, lifetime, session_id)
);
```

Lookup is an exact-match SELECT on `(class, tool_name, action, resource_key, session_id)` — index covers it. The pre-release status means we **drop and recreate** the existing coding-only grant schema rather than migrate.

### 5.3 Gate decision flow

```
input: ToolCall { tool, action, args, ctx: { mode, channel, session_id, user_id } }

1. class    = tool.approval_class(args)
2. scope    = tool.approval_scope(args)
3. resource = match scope { ToolAction => None, ToolActionResource(k) => Some(k) }
4. if channel.is_remote() && class in {Safe, Sensitive} && !channel.capabilities().supports_classes.contains(&class): return Allow (log only)
5. existing = grants_repo.find(class, tool, action, resource, session_id)
6. if existing: return Allow
7. decision = ApprovalChannel::request(class, tool, action, resource, args)
8. match decision {
       Once       => Allow (no persist),
       Session    => persist(lifetime=Session, session_id=ctx.session_id); Allow,
       Forever    => persist(lifetime=Forever, session_id=NULL); Allow,
       Decline    => Deny { reason: "user declined" },
       Cancel     => Cancel { propagate to executor }
   }
```

## 6. Channel UX

### 6.1 The `ApprovalChannel` trait

```rust
#[async_trait]
pub trait ApprovalChannel: Send + Sync {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision;
    fn capabilities(&self) -> ApprovalCapabilities;
}

pub struct ApprovalCapabilities {
    pub supports_inline:  bool,   // can render buttons in channel
    pub supports_classes: HashSet<ApprovalClass>,
}
```

### 6.2 Per-channel strategy

| Channel  | Safe / Sensitive | Destructive / Admin |
|----------|------------------|---------------------|
| Desktop  | Auto-approve (logged) | Modal (existing UI, generalized) |
| Telegram | Auto-approve | Inline buttons: `[Once] [Session] [Always] [Decline]` |
| Discord  | Auto-approve | Inline buttons (component v2) |
| Slack    | Auto-approve | Block-Kit buttons |
| Email    | Auto-approve | Reply-token (`reply ACCEPT-<id>` / `DECLINE-<id>`) |
| MCP      | Auto-approve | Return structured `approval-required` error to client |

Channels without a finished `ApprovalChannel` impl fall back to **block + tell user to approve on desktop**. This keeps the gate functional during incremental rollout.

### 6.3 Why auto-approve `Sensitive` on remote channels

A user with their Telegram bot token has already crossed an authentication boundary; routine writes (`notes.create`, `tasks.update`) shouldn't require a button-tap each time. `Destructive`/`Admin` is where the safety net actually earns its cost. This is a deliberate, called-out trade-off — if a deployment wants stricter remote behavior, channel impls can opt in to `Sensitive` prompts via `capabilities()`.

## 7. Mode integration

- **Coding mode:** Gate runs every tool call. `CodingApprovalPolicy` provides arg-inspection logic for `bash`, `edit`, `read` (the existing 3-layer classifier, ported and renamed).
- **Assistant mode:** Gate runs every tool call. Most domain tools declare `Safe` or `Sensitive`; only deletion variants and reforge-apply hit `Destructive`.
- **MCP server (external clients):** Gate runs. `Safe`/`Sensitive` auto-approve; `Destructive`/`Admin` return the structured error so the calling client (Claude Code, etc.) can prompt its own user.

## 8. Migration plan (pre-release)

1. Create `crates/approval/`. Move coding guard types in.
2. Rename `Privacy/Layer1/Layer2/Layer3` → `Safe/Sensitive/Destructive/Admin` in code, UI strings, IPC types.
3. Add `approval_class` / `approval_scope` defaults to `Tool` trait.
4. Drop existing coding-only grant table; create unified `approval_grants` schema.
5. Wire `ApprovalGate::check` into `ExecutionCore::run_cycle` before `PreToolUse` dispatch.
6. Implement `ApprovalChannel` for desktop (port existing modal), Telegram, MCP. Defer Discord/Slack/Email to follow-on work — fall back to block+defer.
7. Per-tool: declare `DEFAULT_APPROVAL` for every wired tool. Default to `Safe` unless the tool has obviously mutating semantics.

No data migration: pre-release, no users hold grants.

## 9. Testing

- **Unit:** `ApprovalClass` defaults; `approval_scope` defaults; grants-table CRUD; gate-decision flow with mocked channel.
- **Integration:** End-to-end through `ExecutionCore` with a fake tool that declares each class; verify gate fires before `PreToolUse`, grants persist, `Session` lifetime expires on session end.
- **Coding-policy parity:** Snapshot tests that the new `CodingApprovalPolicy` returns the same class for a corpus of historical shell/edit calls as the old 3-layer guard did.
- **Channel UX:** Per-channel adapter test that `request()` returns each `ApprovalDecision` variant.
- **MCP error shape:** Wire-format assertion that `Destructive` returns the documented `approval-required` JSON-RPC error.

## 10. Open questions (parked)

- Per-tool overrides via config: should users be able to *demote* a tool's class (e.g., declare `notes.delete` as `Sensitive` for their deployment)? Defer until after first release.
- Audit log: do we need a dedicated `approval_decisions` log separate from the existing activity-log? Probably reuse activity-log for v1.
- Hook interaction: should `PreToolUse` hooks see the gate's decision (Allowed / Granted / NewlyApproved)? Useful for observability; defer.

## 11. Observability

Each gate decision emits one `ActivityLog` row:
```
{ kind: "approval", tool, action, class, decision, lifetime, channel, session_id }
```
Reuses the existing activity-log infrastructure; no new sink. Surfaced in tracing UI as a filter dimension.

## Appendix A — Why retire the coding 3-layer guard rather than wrap it

Wrapping (option A from brainstorm) keeps the `Privacy/Layer1/Layer2/Layer3` names in the codebase. Those names encode coding-domain semantics that don't translate (`Layer 1 = read-only filesystem` is meaningless for `tasks.list`). Pre-release rename is a one-shot mechanical change; post-release rename means a grants-table migration and IPC-shape break. Doing it now costs less.

## Appendix B — Why the hybrid trigger pattern

A purely declarative `PermissionLevel` (option A from brainstorm Q2) cannot distinguish `notes.delete { id }` from `notes.delete { all: true }`. A purely runtime classifier (option B) forces every tool author to write inspection logic. The hybrid (`DEFAULT_APPROVAL` constant + optional `approval_class(args)` override) lets simple CRUD tools declare-and-forget, while leaving an escape hatch for risky variants. It also matches the shape of the existing coding guard (most tools fall in a static tier; `bash` and `edit` inspect args), so the migration is local.
