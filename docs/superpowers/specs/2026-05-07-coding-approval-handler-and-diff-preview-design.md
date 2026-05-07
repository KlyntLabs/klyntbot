# Coding Approval Handler + Diff-Preview Modal — Design

**Status:** Design (approved by user; transitioning to writing-plans)
**Date:** 2026-05-07
**Scope:** Fix Klynt's coding-mode approval system end-to-end (Phase 0.1 — security blocker) and add per-tool-type specialized preview rendering with Mirror-driven smart "Allow always" suggestions (Phase 3.1).
**Crates touched:** `approval`, `app-core`, `cognitive`, `bus`, `desktop`, `desktop-ui`
**Companion docs:**
- `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md` (the gate this work builds on)
- `docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md` (background — Phase 0.1 + 3.1 in the roadmap)
- `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §10 (event vocabulary)

---

## 1. Problem

Two related defects in Klynt's coding-mode approval system:

1. **`crates/app-core/src/coding/approval_handler.rs::respond_approval` returns `ApprovalHandlerError::NotAvailable` unconditionally** — a 25-line stub. Even though `ApprovalCard.tsx`, `useApprovalQueue.ts`, the `ApprovalGate`, and the persistent `ApprovalGrants` table all exist and work in isolation, the user-clicks-Approve → backend-routes-decision-back-to-gate path is broken. The agent loop's awaiting tool future never resolves.

2. **The existing `ApprovalCard` shows tool args via `summarizeArgs(item.args)`**, which is `JSON.stringify(args)` for everything except bash. Users approve risky operations (file edits, shell commands, web fetches) without seeing what's actually about to happen — the diff, the command, the URL — increasing the chance of blind-approval bugs.

## 2. Goals

1. **Approvals work end-to-end in coding mode.** Click Allow once → tool executes within 1 second. Click Deny → tool surfaces rejection. Click Allow always → grant persists; subsequent matching tool calls auto-allow.

2. **Per-tool-type specialized previews.** Edit/Write tools render a unified diff with line counts. Bash renders the command with risk-pattern badges. URL tools render method + URL + redacted headers. MCP tools render server + tool + args. Generic fallback for everything else.

3. **Mirror-driven "Allow always" suggestions.** After ≥3 prior approvals on the same tool+path-prefix combination (within a 30-day window, ≥80% approval rate), the next matching approval card shows a smart pattern suggestion that the user can commit with one click.

4. **Independently shippable waves.** Wave 1 (the wiring fix) ships as a standalone PR; Wave 2 (preview + smart grants) builds on it. Either can be reverted without touching the other.

## 3. Non-goals

- **Modal-vs-inline UX redesign.** The existing `ApprovalCard.tsx` is inline (rendered as a `ConversationItem` of `kind: "approval"`). Keep that.
- **Approval-system architecture overhaul.** The unified gate (`ApprovalGate`, `ApprovalChannel` trait, `ApprovalGrants` table, `CodingApprovalPolicy`) is fully built and works. Don't rewrite it.
- **MCP schema lookup.** Field present in the type so it's a non-breaking add later; populated as `None` in v1.
- **Full-diff fetcher for truncated diffs.** Footer is informational in v1; click-to-fetch deferred to Phase 2.
- **Multi-candidate Mirror suggestions.** v1 returns one suggestion + frontend-derived alternatives. Multi-candidate Mirror returns can come later.
- **Pattern UX for non-Diff tools** (Mirror suggesting `bash on cargo *` etc.). v1 only suggests for Diff-type tools where the value is highest.
- **Cross-restart pending-card recovery.** If the user kills the app while a card is pending, the pending state is lost. Documenting this is fine.

---

## 4. Architecture overview

### 4.1 End-to-end data flow

```
Coding-mode tool call (Edit src/components/Sidebar.tsx)
   │
   ▼
ExecutionCore.run_cycle preflight
   │
   ▼
ApprovalGate::check(req)                           [existing, unchanged]
   │
   ├─ check ApprovalGrantsRepo                     [existing — already works]
   │  └─ if matching grant → GateOutcome::Allow*; skip channel.request
   │
   ├─ NEW: mirror.suggest_pattern(req) → req.suggested_grant
   │
   └─ channel.request(req).await                   [DesktopApprovalChannel — NEW]
         │
         ├─ build_preview(tool_name, args, ctx)    [NEW — selects preview kind]
         ├─ insert (request_id, oneshot::Sender) into pending map
         ├─ emit Tauri event with extended payload
         └─ await oneshot recv with 600s timeout
             │
             ▼
ApprovalCard.tsx renders                           [existing, ~6 lines added]
   ├─ <PreviewRenderer preview={item.preview} />   [NEW]
   └─ <SmartAllowAlwaysButton                       [NEW]
        suggestedGrant={item.suggestedGrant}
        onRespond={...}
      />
   │
   ▼
User clicks Allow once / Allow always (Mirror's pick) / Deny / Add rule…
   │
   ▼
useApprovalQueue.respond → invoke chat_respond_approval
   │
   ▼
AppCore::respond_approval(request_id, decision)    [NEW — replaces 25-line stub]
   │
   ├─ on Always: ApprovalGrantsRepo::insert(grant)
   ├─ DesktopApprovalChannel::resolve(request_id, ApprovalDecision)
   └─ emit DomainEvent::ApprovalResolved           [NEW variant]
         │
         ├─ → MirrorPatternLearner.persist_observation
         └─ → tracing wire log (free via existing pipeline)
   │
   ▼ oneshot wakes channel.request future
   │
   ▼ ApprovalGate::check returns GateOutcome::Allow* or Cancel
   │
   ▼ ExecutionCore proceeds (executes tool) or aborts (skips tool)
```

### 4.2 What's new vs existing

| Component | Status |
|---|---|
| `ApprovalGate::check` flow | ✅ Works as-is |
| `ApprovalGrantsRepo` (persistent grants) | ✅ Works as-is |
| `ApprovalCard.tsx`, `useApprovalQueue.ts` | ✅ Works as-is; ~6-line diff for new fields |
| `respond_approval()` Rust handler | ❌ Rewrite the 25-line stub |
| `DesktopApprovalChannel` | ❌ Add (replaces whatever placeholder is wired today, likely `BlockingFallbackChannel`) |
| Pending-request map keyed by `request_id` | ❌ Add (lives inside DesktopApprovalChannel) |
| `ApprovalPreview` discriminated union | ❌ Add to `crates/approval/src/preview.rs` |
| `SuggestedGrant` + `GrantScope` types | ❌ Add alongside |
| Per-tool `build_preview()` builders | ❌ Add (5 variants + dispatch) |
| `Mirror::ApprovalPatternLearner` (7th signal source) | ❌ Add |
| `DomainEvent::ApprovalResolved` variant | ❌ Add |
| Frontend `<PreviewRenderer>` + per-kind components | ❌ Add (5 components) |
| `<SmartAllowAlwaysButton>` + `<PatternPicker>` | ❌ Add |
| `approval-preview.css` | ❌ Add |

### 4.3 Two-wave shipping

**Wave 1 — Phase 0.1 (~2 days):** Fix the wiring. Implement `DesktopApprovalChannel` with the pending-request map. Rewrite `respond_approval` to route decisions back. Verify the gate is plumbed into coding-mode tool execution. Smoke-test approve/deny end-to-end.

**Wave 2 — Phase 3.1 (~4 days):** Add the typed `preview` and `suggested_grant` fields to `ApprovalRequest`. Build the 5 per-tool preview builders. Build the Mirror pattern learner. Build the frontend renderers and the smart split button. Hook into the existing `ApprovalCard`.

The two waves share zero files-on-the-critical-path. Wave 1 stands alone as a security fix; Wave 2 layers richer payload on top.

---

## 5. Backend design

### 5.1 Rewritten `respond_approval` and `AppCore::respond_approval`

`crates/app-core/src/coding/approval_handler.rs` keeps the same enum names so the existing Tauri command shell doesn't change:

```rust
pub enum AppApprovalDecision {
    AllowOnce,
    AllowAlways { rule: Option<String> },
    Deny,
    AddRule { starlark_source: String },
}

pub enum ApprovalHandlerError {
    NotFound(String),
    Channel(String),
    Grants(common::KlyntbotError),
}
```

The function body becomes a real handler:
- Maps `AppApprovalDecision` → `approval::ApprovalDecision`.
- Persists grants on Always-class decisions **before** unblocking the gate (so the next tool call's grant lookup picks it up).
- Calls `DesktopApprovalChannel::resolve(request_id, decision)`.
- Returns `NotFound` idempotently when the request_id is already gone (timed out, cancelled).

`AppCore` gains a thin async method `respond_approval(request_id, decision)` that wraps this with access to the typed `desktop_approval_channel` and `grants_repo` from `self`. The Tauri command shell at `crates/desktop/src/commands/coding/approval.rs` (or wherever it lives) is the existing `chat_respond_approval` invoking this method.

### 5.2 The `DesktopApprovalChannel`

New file `crates/app-core/src/desktop_approval_channel.rs`. Implements `approval::ApprovalChannel`. Owns:

- `pending: Arc<DashMap<String, PendingEntry>>` — map keyed by request_id; values hold the oneshot sender, snapshot of tool/args/cwd, and any Mirror suggestion captured at request time.
- `emitter: Arc<dyn AppEventEmitter>` — the existing Tauri event emitter.

`request(req)`:
1. Generate `request_id = Uuid::new_v4().to_string()`.
2. Build preview if not already populated: `req.preview = build_preview(tool, args, ctx)`.
3. Insert pending entry with a `oneshot::channel::<ApprovalDecision>`.
4. Emit `agent:approval_requested` Tauri event with the extended payload (preview + suggested_grant included).
5. `tokio::time::timeout(600s, recv).await` — on timeout or sender drop, return `Decline`.

`resolve(request_id, decision)`:
1. `pending.remove(request_id)` → `PendingEntry`.
2. `entry.sender.send(decision)`.
3. Returns `NotFound` if the entry is gone.

`build_grant_row(request_id, decision)`:
- Looks up the pending entry to get tool_name + args + cwd.
- Builds a `GrantRow` matching the existing schema for `ApprovalGrantsRepo::insert`.

### 5.3 Type extensions

`crates/approval/src/request.rs`:
- `ApprovalContext` gains `cwd: PathBuf` (small refactor; ~5–10 call sites updated).
- `ApprovalRequest` gains `preview: Option<ApprovalPreview>` and `suggested_grant: Option<SuggestedGrant>`.

`crates/approval/src/preview.rs` (new):

```rust
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
        args: Value,
        schema: Option<Value>,
    },
    Generic {
        args: Value,
    },
}

pub struct SuggestedGrant {
    pub pattern: String,
    pub scope: GrantScope,
    pub reason: String,
}

pub enum GrantScope {
    ExactToolPath { tool: String, path: PathBuf },
    ToolFolder { tool: String, folder: PathBuf },
    ToolGlob { tool: String, glob: String },
    Custom { starlark_source: String },
}
```

### 5.4 Per-tool preview builders

`crates/approval/src/preview.rs::build_preview(tool_name, args, ctx) -> ApprovalPreview` dispatches via `classify_preview_kind(tool_name)`:

- File mutation tools (`edit`, `write`, `multi_edit`, `apply_patch`, `str_replace_file`, `create_file`, `write_file`, `edit_file`, `notebook_edit`) → `Diff`.
- Shell tools (`bash`, `shell`, `run_command`, `execute_command`) → `Command`.
- URL tools (`web_fetch`, `http_get`, `http_post`, `web_search`, `fetch`) → `Url`.
- Tools starting with `mcp_` → `Mcp`.
- Anything else → `Generic`.

**`build_diff_preview`:** reads existing file content (handling missing-file = new-file), applies `Edit` substitution or `Write` replacement to derive new content, runs `similar::TextDiff::from_lines` → `unified_diff().context_radius(3).to_string()`. Counts +/− lines via `iter_all_changes`. Truncates at 200 lines with informational footer.

**`build_command_preview`:** extracts `command` arg, scans against a hard-coded `RISK_PATTERNS` table (`rm -rf`, `curl ... | sh`, `sudo `, `chmod 777`, etc.), populates `risk_hits` and `is_dangerous`. Truncates at 4000 chars.

**`build_url_preview`:** extracts URL + method (default GET) + headers. Redacts sensitive header values (`authorization`, `cookie`, `x-api-key`, `x-auth-token`, `proxy-authorization`, `set-cookie`) to `<redacted>` **at preview-build time** so secrets never cross the IPC boundary. Truncates body at 500 chars.

**`build_mcp_preview`:** parses `mcp_{server}_{tool}` name format. Schema field `None` in v1.

**`build_generic_preview`:** wraps `args` as-is.

All builders are pure functions except `build_diff_preview` which makes one filesystem read. No async, no shared state. Trivially unit-testable.

### 5.5 Mirror — `ApprovalPatternLearner` (7th signal source)

New file `crates/cognitive/src/mirror/sources/approval_patterns.rs`. Implements existing `MirrorSignalSource` trait.

**Subscription side** (`run` loop): subscribes to `DomainEvent::ApprovalResolved`, persists each observation to `approval_pattern_history` table:

```sql
CREATE TABLE approval_pattern_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      TEXT NOT NULL DEFAULT 'default',
    tool_name    TEXT NOT NULL,
    path         TEXT,
    decision     TEXT NOT NULL,           -- 'once' | 'forever' | 'denied'
    pattern_used TEXT,                    -- the pattern committed (if Forever)
    occurred_at  TEXT NOT NULL
);
CREATE INDEX idx_aph_tool_path ON approval_pattern_history(user_id, tool_name, path);
CREATE INDEX idx_aph_recency   ON approval_pattern_history(user_id, tool_name, occurred_at);
```

**Query side** (`suggest_pattern(tool, path, ctx) -> Option<SuggestedGrant>`):

For each candidate pattern (exact path, parent folder, recursive glob `prefix/**`, extension glob `**/*.ext`):
1. Count approvals + total in last 30 days for this user.
2. Skip if `approval_count < 3` (cold start).
3. Skip if `approval_count / total < 0.80` (low confidence).
4. Score = `approval_count * specificity_weight` (Exact=4.0, Folder=3.0, Recursive=2.0, Extension=1.5).
5. Return highest-scoring candidate, or `None` if no candidate qualifies.

**Storage cost:** ~1 row per approval; ~10 MB/year heavy use; nightly cron prunes rows older than 90 days.

**Privacy:** path stored as plain string; lives in `~/.klyntbot/data.db`; same threat model as other personal data.

### 5.6 Wiring into Mirror

`MirrorEngine::start` registers the new source alongside the existing 6. `MirrorFacade` exposes `approval_patterns() -> &ApprovalPatternLearner`.

### 5.7 Wiring into the gate

`ApprovalGate` gets a new optional field `mirror_facade: Option<Arc<MirrorFacade>>`. In `check`, before calling `channel.request`:

```rust
if req.suggested_grant.is_none() {
    if let Some(mirror) = &self.mirror_facade {
        let path = extract_path_from_args(&req.args);
        req.suggested_grant = mirror
            .approval_patterns()
            .suggest_pattern(&req.tool_name, path.as_deref(), &req.ctx)
            .await;
    }
}
```

`extract_path_from_args` checks `args.get("path")` then `args.get("file_path")`.

### 5.8 New domain event

`crates/bus/src/domain_events.rs`:

```rust
DomainEvent::ApprovalResolved {
    user_id: Option<String>,
    tool_name: String,
    path: Option<String>,
    decision: String,             // "once" | "forever" | "denied"
    pattern_used: Option<String>,
    occurred_at: jiff::Timestamp,
},
```

Emitted from `respond_approval` after the gate's pending-request resolver fires. Subscribed by Mirror (for learning) and the tracing wire log (for replay).

---

## 6. Frontend design

### 6.1 Discriminated-union dispatch

`desktop-ui/src/features/coding/components/preview/PreviewRenderer.tsx` is a `switch` on `preview.kind` with an exhaustiveness `_exhaustive: never` line. Adding a Rust variant becomes a TypeScript compile error if the corresponding case isn't added.

### 6.2 Per-kind components

- **`<DiffPreview>`** — header with path + new-file badge + `+N`/`−N` counts. `<pre>` body with line-classifier (`+` lines → green, `-` lines → red, `@@` → hunk style, file headers → muted). Truncation footer when `is_truncated`.
- **`<CommandPreview>`** — `cwd` header + optional yellow `⚠ dangerous` badge + `<pre>` for the command + bullet list of `risk_hits`.
- **`<UrlPreview>`** — method badge + URL + headers list (redacted values shown verbatim as the string `<redacted>`) + body-preview block if present.
- **`<McpPreview>`** — `server / tool` header + JSON-stringified args. `<details><summary>Schema</summary>` collapsed when schema present (none in v1).
- **`<GenericPreview>`** — `<pre>{JSON.stringify(args, null, 2)}</pre>`.

No syntax-highlighting library in v1. Plain CSS classed lines. `prism-react-renderer` or `shiki` is a clean Phase 2 add.

### 6.3 `<SmartAllowAlwaysButton>`

Two visual states based on `suggestedGrant`:

**State A (no suggestion):** plain button identical to today's `Allow always (s)`.

**State B (Mirror has a suggestion):** split button — clicking the body commits the suggested pattern instantly; clicking the caret reveals an inline `<PatternPicker>` with the suggested pattern (highlighted), 1–2 frontend-derived alternatives, and a `Custom Starlark rule…` option.

Fast path = one click. Refinement path = two clicks. Fallback path (custom rule) = same `StarlarkRuleEditor` that exists today.

`PatternPicker.tsx` derives alternatives in TypeScript from the `SuggestedGrant.scope` shape — keeps the Tauri payload thin (single suggestion) and avoids round-trips when the picker opens. If Mirror later returns multiple candidates, this function shrinks to a passthrough.

### 6.4 `ApprovalCard.tsx` integration

~6-line diff against the existing 127-line file:
- Import `<PreviewRenderer>` and `<SmartAllowAlwaysButton>` and the new types.
- Replace the `<dd className="approval-card__args">{summarizeArgs(item.args)}</dd>` row with `{item.preview ? <PreviewRenderer preview={item.preview} /> : summarizeArgs(item.args)}`.
- Replace the `<button>Allow always (s)</button>` with `<SmartAllowAlwaysButton ... />`.

`useApprovalQueue.ts::toItem` extends to populate `preview` and `suggestedGrant` fields from the Tauri payload. `bindings.ts` is auto-regenerated via specta when `cargo tauri dev` runs after the Rust types land.

### 6.5 CSS

New file `desktop-ui/src/styles/approval-preview.css` imported via `desktop-ui/src/styles/index.css`. BEM-ish naming (`approval-preview__head`, `approval-preview__diff-line--added`, etc.). Reuses existing tokens: `--success`, `--danger`, `--success-bg-muted`, `--danger-bg-muted`, `--accent`, `--accent-muted`, `--font-mono`, `--space-1/2/3`, `--radius-md`. Any missing tokens added to `ds-tokens.css` per CLAUDE.md typography/color rules.

`max-height: 400px; overflow-y: auto` on `.approval-preview__diff` keeps large-diff cards from blowing out the conversation stream.

---

## 7. Testing strategy

### 7.1 Backend unit tests (`cargo nextest run`)

| Crate | Module | Coverage |
|---|---|---|
| `approval` | `preview::tests` | All 5 builders, classifier, redaction, truncation |
| `approval` | `gate::tests::with_mirror_suggestion` | Mirror suggester runs before channel.request |
| `app-core` | `desktop_approval_channel::tests` | Pending map, oneshot resolve, timeout, idempotent resolve |
| `app-core` | `coding::approval_handler::tests` | Decision routing, grant persistence, NotFound |
| `cognitive` | `mirror::sources::approval_patterns::tests` | Cold start, threshold, recency, denial ratio, specificity |

~25 unit tests. All sync or `tokio::test`. No mocks.

### 7.2 Backend integration tests

| Test | Coverage |
|---|---|
| `tests/approval_end_to_end.rs::approve_once_unblocks_tool` | Real gate + channel + grants. Spawn `gate.check` future; in parallel call `respond_approval(Once)`; assert future resolves. |
| `tests/approval_end_to_end.rs::approve_always_persists_grant` | Same setup with `AllowAlways`. Assert grant row inserted; subsequent matching `gate.check` returns from grant lookup, no second card fires. |
| `tests/approval_end_to_end.rs::deny_returns_decline` | Respond with `Deny`. Assert `GateOutcome::Cancel` propagates. |
| `tests/mirror_suggestion_end_to_end.rs::suggestion_appears_after_threshold` | Persist 3 prior approvals; trigger 4th request; assert `suggested_grant` is `Some`. |

### 7.3 Frontend tests (`bun run test`)

~12 component tests (Vitest + `@testing-library/react`):
- `DiffPreview.test.tsx` (line classification, new-file badge, truncation footer)
- `CommandPreview.test.tsx` (dangerous badge, risk_hits rendering)
- `UrlPreview.test.tsx` (header redaction)
- `McpPreview.test.tsx` (schema collapsed, server/tool extraction)
- `GenericPreview.test.tsx` (JSON pretty-print)
- `SmartAllowAlwaysButton.test.tsx` (fallback button without suggestion, body click, caret click, alternative click, custom click)
- `PatternPicker.test.tsx` (renders alternatives, commits)

### 7.4 Manual end-to-end verification

See §9 verification criteria.

---

## 8. File layout

**New Rust files:**
- `crates/approval/src/preview.rs` (~250 lines)
- `crates/app-core/src/desktop_approval_channel.rs` (~150 lines)
- `crates/cognitive/src/mirror/sources/approval_patterns.rs` (~280 lines)
- `crates/cognitive/migrations/00X_approval_pattern_history.sql`

**Modified Rust files:**
- `crates/approval/src/lib.rs` — re-exports
- `crates/approval/src/request.rs` — add `cwd` to `ApprovalContext`; add `preview` and `suggested_grant` to `ApprovalRequest`
- `crates/approval/src/gate.rs` — add Mirror integration
- `crates/app-core/src/coding/approval_handler.rs` — replace 25-line stub
- `crates/app-core/src/coding/mod.rs` — module declarations
- `crates/app-core/src/lib.rs` (or init) — wire `DesktopApprovalChannel` and Mirror into gate
- `crates/bus/src/domain_events.rs` — add `ApprovalResolved` variant
- `crates/cognitive/src/mirror/sources/mod.rs` — `pub mod approval_patterns`
- `crates/cognitive/src/mirror/engine.rs` — register source
- `crates/cognitive/src/mirror/facade.rs` — expose accessor
- `crates/desktop/src/commands/coding/approval.rs` (or wherever) — confirm Tauri command

**New TS files:**
- `desktop-ui/src/features/coding/components/preview/PreviewRenderer.tsx`
- `desktop-ui/src/features/coding/components/preview/DiffPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/CommandPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/UrlPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/McpPreview.tsx`
- `desktop-ui/src/features/coding/components/preview/GenericPreview.tsx`
- `desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.tsx`
- `desktop-ui/src/features/coding/components/PatternPicker.tsx`
- `desktop-ui/src/styles/approval-preview.css`
- `desktop-ui/src/features/coding/components/preview/DiffPreview.test.tsx`
- `desktop-ui/src/features/coding/components/preview/CommandPreview.test.tsx`
- `desktop-ui/src/features/coding/components/preview/UrlPreview.test.tsx`
- `desktop-ui/src/features/coding/components/SmartAllowAlwaysButton.test.tsx`
- `desktop-ui/src/features/coding/components/PatternPicker.test.tsx`

**Modified TS files:**
- `desktop-ui/src/types/bindings.ts` — auto-regenerated via specta
- `desktop-ui/src/features/coding/components/ApprovalCard.tsx` — ~6-line diff
- `desktop-ui/src/features/coding/hooks/useApprovalQueue.ts` — extend `toItem`
- `desktop-ui/src/styles/index.css` — `@import "./approval-preview.css"`

**Net total:** ~18 new files, ~12 modified files, ~1500 lines net code added.

---

## 9. Verification criteria

### Wave 1 (Phase 0.1):
- [ ] `cargo build --workspace` clean.
- [ ] `cargo nextest run --workspace` all pass.
- [ ] `cargo clippy --workspace --all-targets --all-features` zero warnings (existing `desktop` crate exceptions preserved).
- [ ] Manual: in coding mode, trigger a `Destructive` tool. The approval card appears. Click "Allow once". Tool executes within 1 second.
- [ ] Manual: same flow, click "Deny". Agent loop receives `Decline`, surfaces rejection as tool result, continues.
- [ ] Manual: same flow, do nothing for 600s. Card auto-resolves with `timed-out` status.
- [ ] Manual: trigger two parallel `Destructive` tool calls. Two cards appear; each resolves independently.

### Wave 2 (Phase 3.1):
- [ ] All Wave 1 criteria still pass.
- [ ] Manual: trigger `Edit src/components/Sidebar.tsx`. Card shows unified-diff with green/red lines and `+N`/`−N` counts.
- [ ] Manual: trigger `bash` with `rm -rf /tmp/foo`. Card shows command + yellow `⚠ dangerous` badge + risk hits list.
- [ ] Manual: trigger `web_fetch` with `Authorization: Bearer XXXX`. Card shows `Authorization: <redacted>`.
- [ ] Manual: approve `Edit src/components/{A,B,C}.tsx` three times. On 4th `Edit src/components/D.tsx`, smart-allow-always button shows `Allow always: Edit on src/components/**` with reason in tooltip.
- [ ] Manual: click smart-allow-always body. Subsequent matching tool calls auto-allow (no card).
- [ ] Manual: click caret. Picker opens with 3 options + Custom. Picking alternative commits that pattern.

---

## 10. Open questions

1. **Where does `chat_respond_approval` Tauri command live?** Likely `crates/desktop/src/commands/coding/approval.rs` — confirm in Wave 1 Task 1.
2. **Does `ApprovalGate` constructor already accept a Mirror handle?** If yes, just wire it; if no, signature change is part of Wave 2.
3. **Are `tool_profile` and `approval_mode` columns populated by coding-mode session creation today?** Affects nothing in this spec but worth a note for whoever extends `state_loader` later.
4. **Pending-card recovery on app restart** — explicitly out of scope. Document.
5. **Specta auto-generation** — verify `ApprovalPreview` discriminated union round-trips correctly. If not, hand-author once and add a CI check that the generated file matches.

---

## 11. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Gate is wired but `channel = BlockingFallbackChannel` (placeholder) | Medium | Wave 1 Task 1 explicitly verifies and rewires. |
| Specta doesn't auto-regenerate the discriminated union correctly | Low | Hand-author + CI check if observed. |
| Mirror engine registration shape doesn't fit a 7th source | Low | Read engine.rs first thing in Wave 2; adapt. |
| `similar` crate edge cases (binary files, trailing-newline) | Low | Test fixtures cover empty diff, all-additions, all-deletions, no-trailing-newline. |
| Approval card scrolling janky for large diffs | Low | `max-height: 400px; overflow-y: auto`. |

---

## 12. Effort estimate

| Block | Wave | Effort |
|---|---|---|
| `respond_approval` rewrite + `DesktopApprovalChannel` + `AppCore` wiring | 1 | 1.5 days |
| Wave 1 integration tests | 1 | 0.5 day |
| `ApprovalPreview` types + 5 builders | 2 | 1 day |
| Builder unit tests | 2 | 0.5 day |
| `ApprovalPatternLearner` + migration + Mirror wiring | 2 | 1 day |
| Pattern learner tests | 2 | 0.5 day |
| Frontend renderers (5 components) | 2 | 0.5 day |
| `SmartAllowAlwaysButton` + `PatternPicker` | 2 | 0.5 day |
| Frontend tests | 2 | 0.5 day |
| CSS + tokens | 2 | 0.25 day |
| Manual verification + polish | 2 | 0.25 day |

**Total: 6 days.** Wave 1 ships in ~2 days; Wave 2 takes another ~4 days.

---

*End of design.*
