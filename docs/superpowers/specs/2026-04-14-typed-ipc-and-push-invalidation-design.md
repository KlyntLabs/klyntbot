# Typed IPC & Push Invalidation — Design Spec

**Date:** 2026-04-14
**Status:** Approved (pending user review of this document)
**Scope:** Replace string-keyed Tauri IPC with generated typed bindings (specta + tauri-specta), and build push-based TanStack Query invalidation on top of the resulting typed event channel.

## Motivation

The desktop UI currently calls into Rust via `ipc("command_name", args)` — a string-keyed dispatch with hand-written TS types that drift from Rust DTOs. Mutations manually invalidate query keys in `onSuccess`, which means every consumer must know which keys to touch, and mutations triggered outside the UI (agent loop, cron, channels) leave caches stale until the next poll.

This spec addresses both problems in a single initiative because the solutions share infrastructure: typed events are the substrate for push invalidation, and the same codegen pipeline produces both typed commands and typed events.

## Non-goals

- Multi-device sync (Zero, Replicache, ElectricSQL). Klyntbot is single-user, single-device; these solve a problem we don't have.
- Removing the dev server's HTTP path. It stays until the platform is near release; removal is out of scope.
- Structured observability / metrics for the IPC layer. Existing `tracing` logs are sufficient.

## Decisions locked

| Decision | Choice |
|---|---|
| Scope shape | Single spec, two phases (schema sharing → push invalidation) |
| Codegen tool | `specta` + `tauri-specta` |
| Event granularity | id+op default, full-payload for hot paths, with FE invalidation registry |
| Migration strategy | Big-bang cutover (pre-release, no users) |
| Dev server | Keep with dual-dispatch adapter; remove near release |

## Architecture

Two phases, sequenced within the same initiative.

### Phase 1 — Typed bindings (schema sharing)

- Add `#[derive(specta::Type)]` to all DTOs crossing the IPC boundary (`*Row` types in `storage`, response/request types in `app-core` and `desktop-shared`, feature DTOs).
- Add `tauri-specta` to `crates/desktop`. Commands gain `#[specta::specta]` alongside `#[tauri::command]`.
- A `build.rs` in `crates/desktop` invokes tauri-specta codegen and writes `desktop-ui/src/shared/ipc/bindings.ts`. An optional `cargo xtask gen-bindings` exists for manual runs.
- The generated file is checked in. CI runs `cargo build -p desktop` followed by `git diff --exit-code` on the bindings file — drift fails the build.
- `desktop-ui/src/shared/lib/ipc.ts` is replaced by a dispatch adapter at `desktop-ui/src/shared/ipc/dispatch.ts` that picks Tauri `invoke` vs `fetch("/api/{cmd}")` based on `window.__TAURI_INTERNALS__` presence.
- Big-bang sweep: every `useQuery("tasks_list", ...)` becomes `useQuery(commands.tasks.list, ...)`. String-keyed `ipc()` is deleted in the same PR series.
- Dev-server handlers continue using the same DTOs (no change required); an integration test ensures every command exported by tauri-specta has a dev-server route.

### Phase 2 — Push invalidation (built on Phase 1's typed events)

- New Rust type in `desktop-shared`:
  ```rust
  #[derive(Clone, Serialize, Deserialize, specta::Type)]
  pub enum EntityUpdate {
      IdOp { kind: EntityKind, id: String, op: EntityOp },
      FullPayload { kind: EntityKind, payload: serde_json::Value },
  }
  pub enum EntityOp { Create, Update, Delete }
  pub enum EntityKind { Task, Project, Note, Area, Okr, /* ... */ }
  ```
- `app-core::emit_updates(&app, &updates)` emits these via tauri-specta's typed event channel (desktop) and via the dev server's SSE stream (browser dev). A single serialization path ensures parity.
- New FE module `desktop-ui/src/shared/sync/`:
  - `EntitySyncProvider.tsx` mounts at app root, subscribes to `events.entityUpdated.listen(...)`.
  - `invalidationRegistry.ts` maps `(EntityKind, EntityOp)` → query keys to invalidate.
  - `fullPayloadDispatcher.ts` handles the full-payload variant via `queryClient.setQueryData()`.
- All `queryClient.invalidateQueries()` calls outside `shared/sync/` are removed. A CI grep check enforces this.

## File layout

### Rust
```
crates/
├── desktop-shared/src/
│   ├── lib.rs                  # DTOs gain #[derive(specta::Type)]
│   └── events.rs               # NEW: EntityUpdate, EntityKind, EntityOp
├── app-core/src/
│   └── events.rs               # NEW: emit_entity_update() helper
├── desktop/
│   ├── build.rs                # NEW: tauri-specta codegen
│   └── src/
│       ├── lib.rs              # collect!() registers commands + events
│       ├── commands/*.rs       # + #[specta::specta] on each command
│       └── dev_server/
│           ├── mod.rs          # SSE stream extended to emit EntityUpdate
│           └── tests.rs        # parity tests
└── xtask/                      # NEW (optional): manual codegen runner
```

### Frontend
```
desktop-ui/src/shared/
├── ipc/
│   ├── bindings.ts             # GENERATED — checked in
│   ├── dispatch.ts             # NEW: Tauri vs fetch adapter
│   └── index.ts                # re-exports commands + events
├── lib/
│   └── query.ts                # useQuery/useMutation take typed command refs
└── sync/
    ├── EntitySyncProvider.tsx
    ├── invalidationRegistry.ts
    ├── fullPayloadDispatcher.ts
    └── index.ts
```

### Generated bindings shape
```typescript
export const commands = {
  tasks: {
    list: (args: TasksListArgs) => dispatch<TasksListResponse>("tasks_list", args),
    update: (args: TasksUpdateArgs) => dispatch<void>("tasks_update", args),
  },
  // ... ~40 commands
};
export const events = {
  entityUpdated: { listen: (cb: (e: EntityUpdate) => void) => UnlistenFn },
  agentEvent:    { listen: (cb: (e: AgentEvent) => void) => UnlistenFn },
};
```

## Data flow

### Mutation path
1. UI triggers `useMutation(commands.tasks.update)`.
2. Typed dispatch → Tauri `invoke` → `commands/tasks.rs` → `AppCore::update_task()`.
3. `AppCore::update_task()` commits to DB, returns `Vec<EntityUpdate>`.
4. Handler calls `emit_updates(&app, &updates)` — emits typed events.
5. Mutation `onSuccess` handles UI concerns only (toast, navigation). No invalidation logic.

### Event → invalidation path
1. `EntitySyncProvider` subscribes at mount, unlistens at unmount.
2. On `variant: "IdOp"` → look up `invalidationRegistry[kind]` → call `queryClient.invalidateQueries({ queryKey })` for each entry. List queries always invalidate; detail queries match by id.
3. On `variant: "FullPayload"` → `fullPayloadDispatcher` writes `queryClient.setQueryData([kind, id], payload)` and splices into relevant list caches. No refetch.
4. Events for unmounted queries are no-ops.

### Invalidation registry example
```typescript
export const invalidationRegistry: Record<EntityKind, InvalidationRule> = {
  task: {
    listKeys: [["tasks", "list"], ["tasks", "today"]],
    detailKey: (id) => ["task", id],
    fullPayloadListUpdaters: [/* optional */],
  },
  // ...
};
```

### Dev server path
The dev server lacks a Tauri runtime. `EntitySyncProvider` detects browser mode and subscribes to `GET /api/events` (SSE), which streams the same `EntityUpdate` payloads. Extends the existing `PipelineEvent` SSE plumbing.

### Ordering
Tauri events and mutation responses are not ordered relative to each other. The design tolerates this: events arriving before the mutation promise resolves are safe because `onSuccess` doesn't depend on invalidation.

## Error handling & edge cases

**Codegen failures** — `build.rs` failure (missing derive, unsupported type) fails the Rust build cleanly. CI diff check forces bindings to stay committed in sync.

**Runtime dispatch failures** — Unknown commands, Tauri rejections, and dev-server `fetch` failures all normalize to the existing error shape; `useMutation` error handling is unchanged.

**Event subscription edge cases:**
- Events before mount are lost — harmless because queries fetch fresh on mount.
- Duplicate events are idempotent (invalidation) or last-write-wins (full payload, matches DB commit order).
- Unknown `EntityKind` logs `console.warn`; a TS exhaustiveness test prevents the case at compile time.
- HMR subscription leaks prevented by effect cleanup returning `unlisten`.

**Dev/prod parity** — SSE and Tauri events share one serialization path in `app-core::events`. Integration test mutates via HTTP, asserts SSE emits matching event.

**Backpressure** — `EntityUpdate` uses a separate channel from `AgentEvent`. Agent streaming (hundreds of events/sec) cannot drown out entity invalidation.

**Migration safety** — No feature flag. Pre-release, no users. CI matrix (clippy, nextest, vitest, biome, dev-server coverage test) is the net. Revert is one commit.

## Testing strategy

### Rust
1. Codegen smoke test in `cargo nextest run -p desktop` — asserts expected commands/events are exported.
2. DTO round-trip tests — serialize/deserialize each DTO variant, assert equality.
3. Event emission tests in `app-core` — call handlers against in-memory `StoragePool`, assert `Vec<EntityUpdate>` contents.
4. Dev-server SSE parity test in `crates/desktop/src/dev_server/tests.rs`.
5. Extended `dev_server_covers_all_tauri_commands` — also asserts tauri-specta commands ↔ dev-server routes.

### Frontend (Vitest)
1. Dispatch adapter tests — mock `window.__TAURI_INTERNALS__` and `fetch`.
2. Invalidation registry exhaustiveness via `assertNever` pattern over generated `EntityKind`.
3. `EntitySyncProvider` tests with mocked event channel — assert `invalidateQueries` calls.
4. Full-payload dispatcher tests — assert `setQueryData` shape and list cache updates.
5. CI check: `queryClient.invalidateQueries` outside `shared/sync/` fails the build.

### End-to-end
One Playwright (or Vitest-browser) test against the real dev server: mutate a task, assert a second mounted query updates without manual refetch.

### Deliberate non-tests
- tauri-specta's own codegen correctness (trust the library).
- Network-level event ordering (design tolerates it; testing is brittle).

## Open questions

None blocking. Implementation plan will surface any tactical questions during execution.

## Next step

Implementation plan via the `superpowers:writing-plans` skill.
