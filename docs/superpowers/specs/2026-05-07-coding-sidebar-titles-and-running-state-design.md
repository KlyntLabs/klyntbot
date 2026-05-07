# Coding Sidebar — Auto-titles, Running State, and Real-Time Catch-Up

**Status:** Design (approved by user, pending spec review)
**Date:** 2026-05-07
**Scope:** Desktop UI — coding mode sidebar UX
**Crates touched:** `app-core`, `desktop`, `desktop-ui`

## Problem

The coding-mode sidebar today shows every session as `Untitled session`, doesn't visually distinguish the active session, and gives no signal when a session is processing in the background. Users running multiple parallel coding sessions can't tell which is in flight, which just finished, or which one they're currently in. When the user navigates back to a running session, late-arriving stream deltas can be missed because the per-thread Tauri listener only attaches on mount.

## Goals

1. Auto-generate a meaningful title from the first user message of a coding session.
2. Show a running indicator on the sidebar row for sessions with an in-flight turn.
3. Group sessions into `Running` / `Recently completed` / `Chats` so a user with many parallel sessions can see at a glance what's happening.
4. Make the active (selected) session visually distinct.
5. When the user switches back to a running session, replay any deltas that arrived while they were elsewhere — zero loss.

## Non-goals

- No new backend session-state infrastructure. Existing `agent:thread_event` channel and `ActiveStreams` DashMap are sufficient.
- No new WebSocket / SSE layer. Tauri events already provide realtime push.
- No persistence of "recently completed" / "running" state across app restarts. Both are in-memory frontend state and reset on launch (the agent loop on the Rust side resumes via `coding_thread_resume`).
- No multi-user / shared-session features.
- Assistant (non-coding) sidebar is out of scope. Its titling already works via `useThreadTitleAutogeneration`.

## Architecture overview

Three independent slices share one event channel:

| Slice | Layer | Producer of new work |
|---|---|---|
| A. Auto-titling | `app-core/coding/title_service.rs` (new) + `coding:thread_updated` Tauri emit | Rust `tokio::spawn` background task on first user message |
| B. Sidebar grouping UI | `desktop-ui/src/features/coding/` (extend sidebar) | Pure frontend |
| C. Real-time catch-up | `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts` (new global singleton) | Pure frontend |

Single existing event consumed by B and C: `agent:thread_event` (already streaming).
Single new event emitted by A: `coding:thread_updated` (frontend listener already exists at `useCodingSessions.ts:60` — currently no Rust producer).

### Sidebar layout

```
┌──────────────────────────┐
│  Running (n)             │  ← any session with an in-flight turn
│  ● Refactor sidebar      │
│  ● Build login flow      │
├──────────────────────────┤
│  Recently completed (n)  │  ← turn finished, user hasn't viewed it
│  ✓ Fix bug in repo       │
├──────────────────────────┤
│  Chats                   │  ← everything else (sorted by recency)
│    Untitled session      │
│    Refactor auth flow    │
└──────────────────────────┘
```

Empty groups collapse (no header rendered) so a fresh app with no activity looks identical to today.

### Group state machine (frontend, derived per-render)

```
session created ─────────────────────► Chats
                  │
                  ▼  turn_started for this session
               Running
                  │
                  ▼  turn_completed for this session
            Recently completed
                  │
                  │  user opens session  OR  30min timer expires
                  ▼
                Chats
```

## Slice A — Auto-titling

### Trigger

In `crates/app-core/src/coding/turn_handler.rs`, in the existing `coding_send` flow, before returning to the caller:

1. Read `metadata.title` for the session row.
2. Read message count for the session.
3. If `title is None` AND this is the first user message: `tokio::spawn(autogenerate_title(...))`. Don't await.

### Background task

New module: `crates/app-core/src/coding/title_service.rs`.

```rust
pub async fn autogenerate_title(
    pool: StoragePool,
    cognitive_provider: Arc<DynProvider>,
    app_handle: tauri::AppHandle,
    session_key: String,
    first_user_message: String,
) -> common::Result<()>
```

Flow:
1. Re-check `metadata.title` is still `None` (idempotent — user could have manually renamed in the meantime).
2. Build a one-shot prompt:
   > Generate a 3–6 word title for this coding session based on the user's first request. Output only the title — no quotes, no period, Title Case.
   >
   > USER MESSAGE: `{first_user_message[..500]}`
3. Call `cognitive_provider.complete()` with `temperature: 0.2`, `max_tokens: 24`, request timeout 5s.
4. Sanitize: trim whitespace, strip wrapping `"`/`'`, drop trailing `.`/`!`/`?`, hard-cap at 60 chars.
5. `repos.sessions.rename_session(&session_key, &title, jiff::Timestamp::now())`.
6. `app_handle.emit("coding:thread_updated", json!({ "thread_id": <id> }))`.

Annotated `#[tracing::instrument(skip(pool, cognitive_provider, app_handle, first_user_message), err)]` per the project's tracing convention.

### Provider selection

Use `config.cognitive.provider` (already wired as `Arc<DynProvider>` for Reforge). Sourced from `AppCore::cognitive_provider()`. No new config field.

### Frontend reaction

`useCodingSessions.ts` already listens for `coding:thread_updated` and refetches. Once Rust actually emits this event (it doesn't today), titles update automatically with no further frontend changes.

While the title is generating, the sidebar continues to show the existing `Untitled session` placeholder for ~1s. We do not show a separate "Generating…" state — the heuristic-then-LLM dual-path was rejected during brainstorming.

### Manual rename path

`coding_thread_set_name` (`thread_handler.rs:330`) gains one new line at the end:
```rust
app_handle.emit("coding:thread_updated", json!({ "thread_id": id }))?;
```

This makes manual renames update the sidebar in real time too — currently they only update on next `coding_thread_list` refetch.

### Failure modes

| Failure | Behavior |
|---|---|
| LLM timeout (5s) | `tracing::warn!`, leave title `None`, sidebar keeps showing `Untitled session`. |
| LLM error | Same as above — non-fatal. |
| Provider unconfigured | Skip titling silently. Preserves current behavior. |
| `rename_session` fails | `tracing::error!`, no emit, no retry. Manual rename remains available. |
| User manually renamed before LLM finished | Idempotent re-check at step 1 sees a non-null title and aborts. |

## Slice B — Sidebar grouping UI

### `ThreadEventBuffer` global singleton

**Path:** `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts` (new file).

Backed by zustand. Initialized once at app mount (in `App.tsx` or the coding-mode entry component) before any `useThreadEvents` consumer.

**Internal state:**
- `eventsByThread: Map<string, ThreadEvent[]>` — per-thread ring buffer. Cap: most recent 500 events per thread, dropped oldest-first.
- `runningThreadIds: Set<string>` — added on `turn_started`, removed on `turn_completed` / `error`.
- `recentlyCompleted: Map<string, { finishedAt: number; timer: ReturnType<typeof setTimeout> }>` — added on `turn_completed`, removed on user-open or 30-minute timeout.

**Single Tauri listener** attached at construction:
```ts
listen<ThreadEvent>('agent:thread_event', (msg) => {
  const ev = msg.payload;
  pushToBuffer(ev.thread_id, ev);
  applyToRunningSet(ev);
  applyToRecentlyCompleted(ev);
  fanOutToSubscribers(ev.thread_id, ev);
});
```

**Public API (zustand store + hook helpers):**
```ts
subscribeToThread(threadId: string, onEvent: (e: ThreadEvent) => void): () => void
useRunningIds(): Set<string>                  // reactive
useRecentlyCompleted(): Map<string, number>   // reactive (just finishedAt)
markThreadOpened(threadId: string): void      // clears recentlyCompleted entry + timer
```

`subscribeToThread` first synchronously drains the buffered events for that thread to the callback, then registers the live subscription. This is what gives "full catch-up — zero loss" on switch-back.

**Memory bound:** 500 events × ~1 KB × ~50 threads ≈ 25 MB worst case. Bounded; no leak.

### `useThreadEvents.ts` refactor

`crates/.../desktop-ui/src/features/coding/hooks/useThreadEvents.ts` becomes a thin wrapper around `subscribeToThread`. The existing `coding_thread_resume` seed call stays (it provides completed-turn history from SQLite, which the buffer doesn't hold). Behavior is preserved for already-open sessions; new behavior is full delta replay for sessions you switch back to.

### Sidebar rendering

Extend `desktop-ui/src/features/coding/components/CodingSidebar.tsx`:

```tsx
const sessions = useCodingSessions();
const runningIds = useRunningIds();
const recentlyCompleted = useRecentlyCompleted();
const activeId = useActiveCodingThreadId();

const { running, recent, chats } = useMemo(
  () => partitionSessions(sessions, runningIds, recentlyCompleted),
  [sessions, runningIds, recentlyCompleted]
);
```

`partitionSessions` is a pure function (testable) returning three disjoint arrays. A session in `runningIds` always lands in `running` regardless of `recentlyCompleted` membership (turn restarted while still in the recent-completed window — running takes precedence).

**Row click handler:**
```ts
onClick: () => {
  markThreadOpened(thread.id);   // clears recently-completed entry + cancels its timer
  navigateTo(thread.id);          // existing
}
```

### `ThreadRow` extension

Add two `data-*` attributes:

| State | `data-status` | `data-active` | Visual |
|---|---|---|---|
| Running | `"running"` | `"true"` or omitted | Pulsing 6px dot, `var(--accent)`, left of title |
| Recently completed | `"recent"` | `"true"` or omitted | Solid 6px dot, `var(--success-muted)`, no animation |
| Idle | omitted | `"true"` or omitted | No dot |
| Active | (any of above) | `"true"` | 3px left accent bar + `--bg-active` background tint + bold title |

Active and status are orthogonal — both visual treatments stack on the same row.

### CSS

New file: `desktop-ui/src/styles/coding-sidebar.css`, imported through `desktop-ui/src/styles/index.css`. New tokens added to `ds-tokens.css`:
- `--success-muted` (already follows the convention of dim-tinted brand colors)
- `--sidebar-row-active-bg`

Pulse keyframes:
```css
@keyframes klynt-pulse-dot {
  0%, 100% { opacity: 0.55; transform: scale(1); }
  50%      { opacity: 1.00; transform: scale(1.15); }
}
```

### Recently-completed timer hygiene

| Event | Action |
|---|---|
| `turn_completed` | `clearTimeout(existing); setTimeout(() => removeFromRecentlyCompleted(id), 30*60*1000)` |
| `turn_started` for a thread already in `recentlyCompleted` | Clear timer + remove from map (it's running again — belongs in Running group) |
| `markThreadOpened(id)` | Clear timer + remove from map |
| App unmount | `clearAllTimers()` (defensive; Tauri quit kills them anyway) |

## Slice C — Real-time catch-up

Covered in Slice B; the `ThreadEventBuffer` is the catch-up mechanism. Slice C exists conceptually as a goal but shares its implementation surface entirely with Slice B.

## Testing

### Rust (`cargo nextest`)

`crates/app-core/src/coding/title_service.rs` (or sibling `tests` mod):

- `autogenerate_title_skips_when_title_already_set` — pre-set title, call function, assert no change and no emit.
- `autogenerate_title_emits_thread_updated_on_success` — mock provider, assert `coding:thread_updated` emitted.
- `autogenerate_title_handles_provider_error` — provider returns `Err`, assert function returns `Ok(())` (non-fatal), no rename, no emit, log captured at `warn`.
- `autogenerate_title_sanitizes_output` — provider returns `"\"Refactor the Sidebar.\"\n"` → stored as `Refactor the Sidebar`.

`crates/app-core/src/coding/turn_handler.rs`:
- `coding_send_first_message_spawns_titling` — first user message triggers titling task.
- `coding_send_subsequent_message_does_not_spawn_titling` — second user message does not.

### Frontend (Vitest)

`desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`:
- `partitionSessions_produces_three_disjoint_groups` for any input.
- `turn_started_event_adds_to_running_set`.
- `turn_completed_event_moves_to_recently_completed`.
- `turn_started_after_completed_clears_recently_completed`.
- `markThreadOpened_clears_recently_completed_and_cancels_timer`.
- `30_min_timer_expiry_removes_from_recently_completed` (uses `vi.useFakeTimers()`).
- `late_subscriber_drains_buffered_events_in_order`.
- `ring_buffer_caps_at_500_events_per_thread`.

`desktop-ui/src/features/coding/components/CodingSidebar.test.tsx`:
- `renders_three_sections_with_correct_partition`.
- `clicking_recently_completed_row_demotes_it_to_chats_on_next_render`.
- `running_session_row_has_data_status_running`.
- `active_running_session_has_both_data_active_and_data_status_running`.
- `empty_groups_have_no_header`.

## Migration / rollout

- No DB migration. The `metadata.title` column path already exists; we just start populating it.
- No config schema change. Cognitive provider is already configured; new behavior gated on it being non-null.
- Pre-release — no user-data backwards compatibility constraint applies.
- After merge, one rebuild of `desktop` is required for the Rust title-service changes; the desktop UI changes ship via the embedded bundle.

## Open questions / future work

- **Title regeneration on demand.** A "Regenerate title" right-click action could re-run the LLM call on an existing session. Out of scope for this spec — easy follow-up since the service is already extracted.
- **Cross-restart "recently completed" persistence.** Currently lost on app close. Could persist `(thread_id, finishedAt)` pairs in local storage. Out of scope until users complain.
- **Per-row last-activity preview** (e.g., "Reading file…", "Wrote 12 lines"). The `agent:thread_event` stream already carries this; could surface it as a subtitle. Out of scope for this design — would expand the row visual contract.

## Files touched

### New
- `crates/app-core/src/coding/title_service.rs`
- `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts`
- `desktop-ui/src/features/coding/state/ThreadEventBuffer.test.ts`
- `desktop-ui/src/styles/coding-sidebar.css`

### Modified
- `crates/app-core/src/coding/turn_handler.rs` — spawn titling on first message
- `crates/app-core/src/coding/thread_handler.rs` — emit `coding:thread_updated` after `set_name`
- `crates/app-core/src/coding/mod.rs` — re-export `title_service`
- `desktop-ui/src/features/coding/hooks/useThreadEvents.ts` — refactor onto `ThreadEventBuffer`
- `desktop-ui/src/features/coding/components/CodingSidebar.tsx` — three-group rendering
- `desktop-ui/src/features/app/components/ThreadRow.tsx` — `data-status` / `data-active` attrs
- `desktop-ui/src/styles/ds-tokens.css` — new tokens
- `desktop-ui/src/styles/index.css` — import new sidebar CSS
- Initialization site for `ThreadEventBuffer` (likely `desktop-ui/src/features/app/App.tsx`)
