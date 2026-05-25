# Deep Dive: Project `RoutingContext` into narrow per-tool context views

> Status: **Phases A–E implemented** (2026-05-24/25). Derive path: 5 tools on `HookCtx`, 6 on `IoCtx`, `bash` on `FullCtx`. `#[tool_actions]` family (Phase E): 5 tools on `ctx = "()"`, `subagents` on `&RoutingContext` (boundary). Design recorded in ADR-0002; vocabulary in CONTEXT.md.
> Architecture candidate #1 of the 2026-05-24 review.
>
> **Scope reality (post-exploration):** narrowing is only possible where a tool has a *typed* layer to project at — the `ToolExecute` derive path (12 tools) and the `#[tool_actions]` macro (6 tools). Hand-written `impl Tool` tools and MCP take the untyped `Tool::execute(args, &RoutingContext)` directly (one fixed registry signature, no per-tool `Ctx` slot) and are the **deliberate, documented floor** — see ADR-0002. The review's "~42" counted every `Tool::execute` impl; most hand-written ones already ignore `ctx` (`_ctx`).

## Decisions Made

| Question | Decision | Rationale |
|----------|----------|-----------|
| Where can narrowing happen? | **Typed `ToolExecute` side only** | The untyped `Tool::execute(args, &RoutingContext)` is the single dispatch signature for all tools in `ToolRegistry`. It must keep `&RoutingContext`. Narrowing lives one layer in, on the typed trait. |
| Mechanism? | **GAT associated context type + projection at the derive bridge** | A tool declares `type Ctx<'a>`; `#[derive(Tool)]` projects `RoutingContext` into it. Delivers self-documenting deps, leak prevention, and compile-time test locality. |
| `async_trait` on `ToolExecute`? | **Keep `#[async_trait]`** (native AFIT attempted, reverted) | Native AFIT — both RPITIT and plain `async fn`, with elided and explicit lifetimes — rejected the GAT-projected borrowed argument `Self::Ctx<'c>` with **E0195** on every impl. `#[async_trait]` boxes the future and elaborates lifetimes, so the GAT is just an argument type. Box alloc is negligible for tool dispatch; `Send` still enforced via the `Tool` bridge. |
| View shape for multi-concern tools? | **Tiered superset ladder** `() ⊂ HookCtx ⊂ IoCtx ⊂ FullCtx` | A tool picks the smallest rung that fits. ~35 of 42 tools land tightly; the rest take the `FullCtx` escape hatch. |
| Top of the ladder? | **`FullCtx` escape hatch, no fat `AgentCtx` rung** | A rung wide enough for `bash` + `ask_user` + `subagents` would carry ~20 of 23 fields — not a real narrowing. The win concentrates on the 35 narrow tools; the ~5-7 wide tools declare `FullCtx` honestly (no worse than today, but explicit). |
| Rollout? | **Two-phase, always-green** | Phase A is mechanical (`type Ctx<'a> = FullCtx<'a>` everywhere). Phases B+ narrow per-tool in reviewable batches. No big-bang body rewrite. |

---

## Current State (the problem)

`RoutingContext` (`crates/tools-core/src/routing.rs`, 205 lines) is a **23-field god-struct** passed by reference to every `Tool::execute`. The fields accreted over time (some are commented "Phase 2.3a").

Field-usage across the 42 `Tool::execute` implementations:

- **18 tools take `_ctx`** — they receive all 23 fields and use **none**.
- **Filesystem tools** (`read`, `glob`, `grep`, `list_dir`, `write`, `apply_patch`, `notebook_edit`) use **only `hook_engine`** — 1 of 23.
- **`bash`** uses 9: `hook_engine, job_supervisor, chat_id, agent_id, agent_chain, event_tx, cancel_token, message_id, channel`.
- **`ask_user`** uses 4: `interaction_tx, interaction_channel, chat_id, hook_engine`.
- **`plan_mode`** uses 3: `chat_id, event_tx, hook_engine`.
- **Domain tools** (`area`, `okr`, `cron`, `project`) use 2-3: `channel, chat_id, entity_tx`.

Field-access frequency from the tools directory: `hook_engine` 23, `event_tx` 8, `entity_tx` 7, `chat_id`/`channel` 5-7, `cancel_token` 5, `job_supervisor` 1. Seven fields (`champion_params`, `is_direct_mode`, `delegation_depth`, `agent_profile`, `plan_session_id`, `workspace_cwd`, `same_turn_user_msg_emitted`) are read by the agent loop, **never by a tool**.

### Why this is shallow

- **Interface ≈ implementation.** The struct's interface is its 23 public fields; tools see all of them regardless of need.
- **No leak prevention.** `read` can reach `bash`'s `job_supervisor`. Tool A sees tool B's wiring.
- **Test surface is accidentally shallow, and that hides bugs.** `RoutingContext::new(channel, chat_id)` zero-defaults the other 21 fields, so a new field lands as `None` in every existing test with **no compile error** forcing anyone to consider it. `bash`, conversely, *panics* (`.ok_or_else` on `job_supervisor`) unless a test sets 7+ fields by hand.
- **Unbounded accretion.** Nothing scopes which concern a new field belongs to; the struct only grows.

### The two-trait structure (the key constraint)

```rust
// crates/tools-core/src/lib.rs:68 — typed, authored by tool writers
#[async_trait]
pub trait ToolExecute: Send + Sync {
    type Params: ToolParams;
    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> Result<String>;
}

// crates/tools-core/src/lib.rs:78 — untyped, dyn-dispatched by the registry
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    // … name/description/parameters/metadata/…
}
```

`#[derive(Tool)]` (`crates/tools-core-macros/src/tool_derive.rs:188-195`) generates the bridge:

```rust
async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
    let params = <#params_type as ToolParams>::from_args(args)?;
    <Self as ToolExecute>::execute(self, params, ctx).await
}
```

`RoutingContext` therefore *must* exist at the untyped boundary. Narrowing can only target the typed `ToolExecute` side, with the bridge acting as the projector.

---

## Proposed Design

### 1. `FromRoutingContext` — the projection seam

```rust
// crates/tools-core/src/routing.rs
pub trait FromRoutingContext<'a> {
    fn project(rc: &'a RoutingContext) -> Self;
}
```

### 2. `ToolExecute` — GAT context, native async fn

```rust
// crates/tools-core/src/lib.rs
#[async_trait]
pub trait ToolExecute: Send + Sync {
    type Params: ToolParams;
    type Ctx<'a>: FromRoutingContext<'a>;
    // `'c` is named explicitly; impls write `async fn execute<'c>(…, ctx: <Ctx><'c>)`.
    async fn execute<'c>(&self, params: Self::Params, ctx: Self::Ctx<'c>)
        -> common::Result<String>;
}
```

There are no associated-type defaults on stable Rust, so every impl states its `type Ctx<'a>` — this is mechanical and, deliberately, makes each tool's dependency surface explicit at the type level.

### 3. The view ladder

```rust
// crates/tools-core/src/routing.rs

// Rung 0 — nothing.
impl<'a> FromRoutingContext<'a> for () { fn project(_: &'a RoutingContext) {} }

// Rung 1 — owns cheap clones (field TYPES mirror RoutingContext, so tool
// bodies use them unchanged). No lifetime param.
pub struct HookCtx {
    pub hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    pub session_key: Option<SessionKey>,
}

// Rung 2 (⊃ HookCtx) — fields the write/edit/web/plan tools actually use.
// (No entity_tx: none of the migrated tools read it.)
pub struct IoCtx {
    pub hook_engine:  Option<Arc<klynt_hooks::HookEngine>>,
    pub session_key:  Option<SessionKey>,
    pub channel:      ChannelName,
    pub chat_id:      ChatId,
    pub event_tx:     Option<mpsc::Sender<ToolEvent>>,
    pub cancel_token: Option<CancellationToken>,
    pub message_id:   Option<String>,
}

// Rung 3 — escape hatch + transitional view (borrows).
pub struct FullCtx<'a>(pub &'a RoutingContext);
impl<'a> std::ops::Deref for FullCtx<'a> {
    type Target = RoutingContext;
    fn deref(&self) -> &RoutingContext { self.0 }
}
```

`HookCtx`/`IoCtx` own cheap clones (an `Arc` refcount bump + small values), so the `type Ctx<'a> = HookCtx` GAT carries no lifetime and tool bodies stay byte-for-byte identical (`ctx.hook_engine.as_ref()`, `ctx.session_key.clone()`, `ctx.channel.as_str()` all keep working). The trait method's `'c` is still declared (FullCtx needs it) and is consumed by `#[async_trait]`'s lifetime elaboration, so no unused-lifetime lint fires. `FullCtx` projects to `FullCtx(rc)` via `Deref`.

### 4. The bridge change (both macros)

`tool_derive.rs:188-195` and the `#[tool_actions]` bridge (`tool_actions.rs:219`) change from passing `ctx` to projecting it:

```rust
async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
    let params = <#params_type as ToolParams>::from_args(args)?;
    let view = <<Self as ToolExecute>::Ctx<'_> as FromRoutingContext>::project(ctx);
    <Self as ToolExecute>::execute(self, params, view).await
}
```

The bridge future stays `Send` (the `Tool` trait keeps `#[async_trait]`); the awaited native-AFIT future is `Send` because its captures are.

### 5. Tool impls — declare the rung

```rust
// read (filesystem)
impl ToolExecute for ReadTool {
    type Params = ReadParams;
    type Ctx<'a> = HookCtx<'a>;
    async fn execute(&self, p: ReadParams, ctx: HookCtx<'_>) -> Result<String> { /* uses ctx.hook_engine */ }
}

// any of the 18 zero-use tools
impl ToolExecute for FocusTool {
    type Params = FocusParams;
    type Ctx<'a> = ();
    async fn execute(&self, p: FocusParams, _: ()) -> Result<String> { /* … */ }
}

// bash / ask_user / subagents
impl ToolExecute for BashTool {
    type Params = BashParams;
    type Ctx<'a> = FullCtx<'a>;
    async fn execute(&self, p: BashParams, ctx: FullCtx<'_>) -> Result<String> { /* ctx.job_supervisor via Deref */ }
}
```

---

## Migration Phases

Each phase is a standalone, always-green PR.

| Phase | Scope | Status |
|-------|-------|--------|
| **A — machinery + mechanical sweep** | Add `FromRoutingContext`, `FullCtx`, `impl … for ()`. Add GAT `type Ctx<'a>` to `ToolExecute` (kept `#[async_trait]`). Update `tool_derive.rs` bridge to project. Set `type Ctx<'a> = FullCtx<'a>` on all 12 derive-path impls. | **Done.** Workspace builds; 142 tools-core+klynt-core tests pass. Behavior identical (`FullCtx` derefs). |
| **B — filesystem reads → `HookCtx`** | `read, glob, grep, list_dir, tool_search`. Add `HookCtx { hook_engine, session_key }`. | **Done.** 5 tools narrowed to a 2-field view; bodies unchanged; 142 tests pass, clippy clean. |
| **C — write/edit/web/plan → `IoCtx`** | `write, edit, apply_patch, notebook_edit, web_fetch, plan_mode`. Add `IoCtx { hook_engine, session_key, channel, chat_id, event_tx, cancel_token, message_id }` (no `entity_tx` — unused by these). | **Done.** 6 tools narrowed; bodies unchanged. |
| **D — `bash` stays `FullCtx`** | Verified honest (uses `job_supervisor`, `agent_chain`, …). | **Done.** Ladder complete for the derive path. |
| **E — `#[tool_actions]` family** | Add `ctx = "View"` to the `tool_actions` macro (projects at the top of the generated `execute`). `docs, temporal, annotate, mirror, launcher` → `ctx = "()"`; `subagents` stays `&RoutingContext` (forwards to `SubagentsHandler` trait). | **Done.** 5 tools narrowed to `()`; 266 tests pass; macro default unchanged (backward compatible). |

Beyond Phase E lies the untyped-`Tool` floor (hand-written + MCP tools) — intentionally not narrowed; see ADR-0002.

Only `tool_derive.rs` needed changing (not `tool_actions.rs` — that path doesn't use `ToolExecute`). `bus::InjectorContext` stays implemented on `RoutingContext`; the bus injection layer is unchanged. Each per-tool narrowing in B/C is independently revertible.

---

## What Tests Would Survive / Improve

Current: tool tests build a `RoutingContext` (2-liner for most via `new`; 7+ field assignments for `bash`). A new field is silently `None` in all of them.

With views:

```rust
// read — construct one rung, not 23 fields
#[tokio::test]
async fn read_uses_no_hooks() {
    let out = ReadTool::default()
        .execute(params, HookCtx { hook_engine: None, session_key: None })
        .await.unwrap();
    // …
}
```

- **Locality:** a tool's test constructs only its rung.
- **Compile-time visibility:** adding a field to `IoCtx` forces every `IoCtx` tool test to acknowledge it — no silent `None`.
- **Leak prevention is type-checked:** a `HookCtx` tool *cannot* reference `event_tx`; the body won't compile if it tries.
- Existing `RoutingContext`-based integration tests (registry/execution layer) are unaffected — they still drive the untyped `Tool::execute(args, &RoutingContext)` boundary.

---

## Risks / Open Questions

- **`#[tool_actions]` multi-action tools** carry a second bridge (`tool_actions.rs`). Confirm each action method projects to the *same* `Ctx` (a multi-action tool has one `ToolExecute`, so one `Ctx` for all its actions). If an action genuinely needs a wider slice than its siblings, it forces the whole tool up a rung — acceptable, but note it.
- **MCP tools** are constructed dynamically (not via `#[derive(Tool)]`) — verify they implement `Tool` directly and are unaffected, or set `FullCtx`.
- **`FullCtx` `Deref` ergonomics:** field access is transparent, but methods taking `&RoutingContext` accept `&*ctx`. Spot-check no tool relies on owning/cloning the context.
- **Native AFIT `Send` inference:** if any tool body produces a non-`Send` future, the `Tool` bridge fails to compile. This is desirable (surfaces a real `Send` violation) but may need a `trait-variant`/explicit bound if it bites widely. Validate in Phase A.
