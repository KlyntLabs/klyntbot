# Memory Leak Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the 11+ GB memory footprint growth in the Klynt desktop app caused by unbounded in-memory accumulation across multiple subsystems.

**Architecture:** Six independent fixes targeting the highest-impact leak sources found by a 5-agent codebase scan. Changes span the `bus`, `session`, `agent`, `cognitive`, `app-core`, `desktop`, `mcp`, and `desktop-ui` crates. Each task is independently testable and committable.

**Tech Stack:** Rust (tokio broadcast, DashMap, sqlx), TypeScript (Tauri IPC, chatStreamStore)

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/bus/src/domain_events.rs` | Modify | Remove `content: String` from `NoteContentChanged` and `NoteEditingFinished` |
| `crates/app-core/src/handlers/notes/crud.rs` | Modify | Update publishers to not send content |
| `crates/app-core/src/handlers/notes/import_export.rs` | Modify | Update publisher |
| `crates/app-core/src/handlers/fabric.rs` | Modify | Update publisher |
| `crates/agent/src/adapters/note_tree_builder.rs` | Modify | Accept `NoteRepo`, fetch content on demand |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Pass `NoteRepo` to NoteTreeBuilder; wire CancellationTokens to parent |
| `crates/cognitive/src/services/atom_extraction.rs` | Modify | Fetch note content from DB instead of event |
| `crates/session/src/manager.rs` | Modify | Add in-memory message compaction |
| `crates/app-core/src/init/mod.rs` | Modify | Raise `DomainEventBus` capacity from 64 to 512 |
| `crates/desktop/src/app_core.rs` | Modify | Strip large payloads from debug dashboard emission |
| `crates/mcp/src/client/manager.rs` | Modify | Replace unbounded channel with bounded |
| `crates/app-core/src/infrastructure/file_watcher.rs` | Modify | Cap `project_cache` HashMap |
| `desktop-ui/src/shared/stores/chatStreamStore.ts` | Modify | Truncate tool result strings, clear segments on session deactivation |
| `desktop-ui/src/shared/hooks/useQuery.ts` | Modify | Reduce max cache entries |

---

### Task 1: Remove `content` from `NoteContentChanged` and `NoteEditingFinished` domain events

This is the single highest-impact fix. These events carry full note bodies (100KB+) through a `tokio::sync::broadcast(64)` channel with 23+ subscribers. Every subscriber gets a `Clone` of the event — meaning the full note body is retained in the ring buffer until the slowest subscriber (tree builders doing LanceDB embeds) drains it. Removing the content from the event eliminates the dominant allocation source.

**Files:**
- Modify: `crates/bus/src/domain_events.rs:252-259`
- Modify: `crates/app-core/src/handlers/notes/crud.rs:155-158,223-226,542-545`
- Modify: `crates/app-core/src/handlers/notes/import_export.rs:202-205`
- Modify: `crates/app-core/src/handlers/fabric.rs:476-479`

- [ ] **Step 1: Modify the `DomainEvent` enum to remove `content` field**

In `crates/bus/src/domain_events.rs`, replace the two variants:

```rust
// Before (lines 252-259):
NoteContentChanged {
    note_id: String,
    content: String,
},
NoteEditingFinished {
    note_id: String,
    content: String,
},

// After:
NoteContentChanged {
    note_id: String,
},
NoteEditingFinished {
    note_id: String,
},
```

- [ ] **Step 2: Fix the test that references content**

In `crates/bus/src/domain_events.rs`, update the `test_note_editing_finished_event` test (around line 933):

```rust
#[test]
fn test_note_editing_finished_event() {
    let bus = DomainEventBus::new(32);
    let mut rx = bus.subscribe();
    bus.publish(DomainEvent::NoteEditingFinished {
        note_id: "note-1".to_string(),
    });
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event, DomainEvent::NoteEditingFinished { note_id, .. } if note_id == "note-1")
    );
}
```

- [ ] **Step 3: Update all publishers in `crud.rs`**

In `crates/app-core/src/handlers/notes/crud.rs`:

At line ~155 (note_create):
```rust
// Before:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: id.clone(),
    content: created.body.clone(),
});

// After:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: id.clone(),
});
```

At line ~223 (note_update):
```rust
// Before:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: params.id.clone(),
    content: updated.body.clone(),
});

// After:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: params.id.clone(),
});
```

At line ~542 (note_editing_finished):
```rust
// Before:
bus.publish(bus::DomainEvent::NoteEditingFinished {
    note_id: params.note_id,
    content: note.body,
});

// After:
bus.publish(bus::DomainEvent::NoteEditingFinished {
    note_id: params.note_id,
});
```

- [ ] **Step 4: Update publisher in `import_export.rs`**

In `crates/app-core/src/handlers/notes/import_export.rs`, around line 202:

```rust
// Before:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: note.id.clone(),
    content: note.body.clone(),
});

// After:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: note.id.clone(),
});
```

- [ ] **Step 5: Update publisher in `fabric.rs`**

In `crates/app-core/src/handlers/fabric.rs`, around line 476:

```rust
// Before:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: note_id.clone(),
    content,
});

// After:
bus.publish(bus::DomainEvent::NoteContentChanged {
    note_id: note_id.clone(),
});
```

Note: the `content` variable was consumed by the publish call. Check if it's still needed elsewhere in the function. If the only use was the publish, remove the binding too. If it's used later, keep it — removing it from the event doesn't require removing the local variable.

- [ ] **Step 6: Build the bus and app-core crates to catch remaining compile errors**

Run: `cargo build -p bus -p app-core 2>&1 | head -50`

Fix any remaining references to the removed `content` field. Common patterns to search:
- `NoteContentChanged { note_id, content }` → `NoteContentChanged { note_id }`
- `NoteEditingFinished { note_id, content }` → `NoteEditingFinished { note_id }`

- [ ] **Step 7: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/app-core/src/handlers/notes/crud.rs crates/app-core/src/handlers/notes/import_export.rs crates/app-core/src/handlers/fabric.rs
git commit -m "fix(bus): remove content payload from NoteContentChanged and NoteEditingFinished events

These events carried full note bodies (100KB+) through a broadcast(64) channel
with 23+ subscribers. Each subscriber cloned the event, and the slowest subscriber
held the ring buffer open — causing unbounded memory growth. Subscribers that need
content now fetch from NoteRepo by note_id."
```

---

### Task 2: Update NoteTreeBuilder to fetch content from NoteRepo

The `NoteTreeBuilder` is the only subscriber that uses `content` from `NoteContentChanged`. It needs a `NoteRepo` to fetch content on demand. The struct lives in `crates/agent/src/adapters/note_tree_builder.rs` and is constructed in `crates/agent/src/agent_loop/builder.rs`.

**Files:**
- Modify: `crates/agent/src/adapters/note_tree_builder.rs:25-40,67-70,91`
- Modify: `crates/agent/src/agent_loop/builder.rs:791-807`

- [ ] **Step 1: Add `NoteRepo` field to `NoteTreeBuilder`**

In `crates/agent/src/adapters/note_tree_builder.rs`, add storage import and field:

```rust
// Add to imports at top:
use storage::NoteRepo;

// Modify struct (around line 25):
pub struct NoteTreeBuilder {
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    note_repo: NoteRepo,
}
```

- [ ] **Step 2: Update the constructor**

```rust
// Modify new() (around line 34):
pub fn new(
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    note_repo: NoteRepo,
) -> Self {
    Self {
        tree_repo,
        vector_store,
        embedder,
        context_update_queue,
        domain_event_bus,
        note_repo,
    }
}
```

- [ ] **Step 3: Update the event handler to fetch content**

In the `run` method (around line 67), change the match arm:

```rust
// Before:
Ok(DomainEvent::NoteContentChanged { note_id, content }) => {
    if let Err(e) = self.handle_note_changed(&note_id, &content).await {
        warn!(note_id = %note_id, "NoteTreeBuilder: failed to process note change: {e}");
    }
}

// After:
Ok(DomainEvent::NoteContentChanged { note_id }) => {
    match self.note_repo.get_by_id(&note_id).await {
        Ok(Some(note)) => {
            if let Err(e) = self.handle_note_changed(&note_id, &note.body).await {
                warn!(note_id = %note_id, "NoteTreeBuilder: failed to process note change: {e}");
            }
        }
        Ok(None) => {
            debug!(note_id = %note_id, "NoteTreeBuilder: note not found, skipping");
        }
        Err(e) => {
            warn!(note_id = %note_id, "NoteTreeBuilder: failed to fetch note: {e}");
        }
    }
}
```

- [ ] **Step 4: Update the builder callsite to pass `NoteRepo`**

In `crates/agent/src/agent_loop/builder.rs`, around line 791, where `NoteTreeBuilder::new` is called:

```rust
// Before:
let note_tree_builder =
    Arc::new(crate::adapters::note_tree_builder::NoteTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
    ));

// After:
let note_repo = storage::NoteRepo::new(storage_pool.inner().clone());
let note_tree_builder =
    Arc::new(crate::adapters::note_tree_builder::NoteTreeBuilder::new(
        tree_repo.clone(),
        Arc::new(vs.clone()),
        text_embedder.clone(),
        self.context_update_queue.clone(),
        self.domain_event_bus.clone(),
        note_repo,
    ));
```

Check that `storage_pool` is in scope at this point in `build()`. It should be — it's used by TaskTreeBuilder just below.

- [ ] **Step 5: Build and fix compile errors**

Run: `cargo build -p agent 2>&1 | head -50`

Check the `NoteRepo::get_by_id` method exists and returns the expected type. If the method is named differently (e.g., `get`, `find_by_id`), adjust accordingly. Search: `grep -n "fn get_by_id\|fn get\b\|fn find" crates/storage/src/repos/notes.rs | head -10`

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/adapters/note_tree_builder.rs crates/agent/src/agent_loop/builder.rs
git commit -m "fix(agent): NoteTreeBuilder fetches content from NoteRepo instead of event payload"
```

---

### Task 3: Update AtomExtractionService to fetch content from NoteRepo

The `AtomExtractionService` is the only subscriber that uses `content` from `NoteEditingFinished`. It already has access to `pool: sqlx::SqlitePool`, so it can construct a `NoteRepo`.

**Files:**
- Modify: `crates/cognitive/src/services/atom_extraction.rs:51-62,81`

- [ ] **Step 1: Create NoteRepo from pool and update the event handler**

In `crates/cognitive/src/services/atom_extraction.rs`, inside the `start` method where the task is spawned:

```rust
// Add near the top of the spawned async block, after pool is captured:
let note_repo = storage::NoteRepo::new(pool.clone());
```

Then update the match arm (around line 81):

```rust
// Before:
Ok(DomainEvent::NoteEditingFinished { note_id, content }) => {
    // Debounce: skip if we processed this note recently
    let now = tokio::time::Instant::now();
    if let Some(last) = debounce_map.get(&note_id) {
        // ... existing debounce logic using note_id and content

// After:
Ok(DomainEvent::NoteEditingFinished { note_id }) => {
    // Debounce: skip if we processed this note recently
    let now = tokio::time::Instant::now();
    if let Some(last) = debounce_map.get(&note_id) {
        // ... existing debounce logic stays the same
    }
    // Fetch content from DB
    let content = match note_repo.get_by_id(&note_id).await {
        Ok(Some(note)) => note.body,
        Ok(None) => {
            debug!(note_id = %note_id, "AtomExtraction: note not found, skipping");
            continue;
        }
        Err(e) => {
            warn!(note_id = %note_id, "AtomExtraction: failed to fetch note: {e}");
            continue;
        }
    };
    // ... rest of extraction logic uses `content` as before
```

The exact placement depends on where `content` is first used after the debounce check. Put the fetch right after the debounce guard passes (i.e., after the `if let Some(last) ... continue` block).

- [ ] **Step 2: Fix any other destructuring of `NoteEditingFinished` in cognitive crate**

Run: `grep -rn "NoteEditingFinished" crates/cognitive/src/ | grep -v "test\|comment\|//"`

Update any remaining `NoteEditingFinished { note_id, content }` destructures to `NoteEditingFinished { note_id }`.

- [ ] **Step 3: Build cognitive crate**

Run: `cargo build -p cognitive 2>&1 | head -50`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/atom_extraction.rs
git commit -m "fix(cognitive): AtomExtractionService fetches note content from DB instead of event"
```

---

### Task 4: Full workspace build to catch any remaining `content` references

After Tasks 1-3, any remaining code that destructures `content` from these events will fail to compile.

**Files:**
- Potentially any file matching `NoteContentChanged` or `NoteEditingFinished`

- [ ] **Step 1: Build entire workspace**

Run: `cargo build --workspace 2>&1 | head -80`

- [ ] **Step 2: Fix all remaining compile errors**

For each error of the form `field content not found in NoteContentChanged`:
- If the code uses `content`, add a NoteRepo fetch (same pattern as Tasks 2-3)
- If the code ignores content (`..`), the fix is free — already compiles

Known files that pattern-match on these events (from grep):
- `crates/cognitive/src/services/salience.rs:123` — uses `..`, should compile
- `crates/cognitive/src/services/background.rs:1181` — uses `..`, should compile
- `crates/agent/src/adapters/community_builder.rs:72` — uses `..`, should compile
- `crates/simulator/src/actions.rs` — check and fix

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

- [ ] **Step 4: Commit any remaining fixes**

```bash
git add -u
git commit -m "fix: resolve remaining NoteContentChanged/NoteEditingFinished content references"
```

---

### Task 5: Add in-memory compaction to Session

The `Session::messages` Vec grows unbounded for the active desktop session (which is never evicted from the LRU cache). SQL compaction fires only on `save()` during LRU eviction, but the in-memory Vec is never trimmed. Each message can be 10-100KB (tool calls, full LLM responses). Over a long session with hundreds of messages, this becomes hundreds of MB.

**Files:**
- Modify: `crates/session/src/manager.rs:64-106,196-200`

- [ ] **Step 1: Add the in-memory trim constant and helper**

In `crates/session/src/manager.rs`, after the existing constants (around line 200):

```rust
/// Compaction threshold: compact when entries exceed this count
const COMPACTION_THRESHOLD: usize = 1000;

/// Number of entries to keep after compaction
const COMPACTION_KEEP: usize = 500;

/// In-memory trim threshold: trim the Vec when it exceeds this.
/// Set higher than COMPACTION_KEEP to avoid trimming on every message
/// after a load from SQL.
const IN_MEMORY_TRIM_THRESHOLD: usize = 600;

/// Number of messages to keep after an in-memory trim.
const IN_MEMORY_TRIM_KEEP: usize = 500;
```

- [ ] **Step 2: Add a trim method to `Session`**

In `crates/session/src/manager.rs`, add to the `Session` impl block (after `clear` around line 118):

```rust
/// Trim in-memory messages if they exceed the threshold.
/// Keeps the most recent `IN_MEMORY_TRIM_KEEP` messages.
fn trim_if_needed(&mut self) {
    if self.messages.len() > IN_MEMORY_TRIM_THRESHOLD {
        let drain_count = self.messages.len() - IN_MEMORY_TRIM_KEEP;
        self.messages.drain(..drain_count);
    }
}
```

- [ ] **Step 3: Call trim after each message addition**

In `add_message_with_request_id` (around line 84, after `self.updated_at = Utc::now()`):

```rust
self.updated_at = Utc::now();
self.trim_if_needed();
```

In `add_structured_message` (around line 105, after `self.updated_at = Utc::now()`):

```rust
self.updated_at = Utc::now();
self.trim_if_needed();
```

- [ ] **Step 4: Write a test for in-memory trim**

Add to the `#[cfg(test)] mod tests` block in `manager.rs`:

```rust
#[test]
fn session_trims_messages_when_exceeding_threshold() {
    let mut session = Session::new("test");
    // Add more than IN_MEMORY_TRIM_THRESHOLD messages
    for i in 0..=IN_MEMORY_TRIM_THRESHOLD {
        session.add_message("user", format!("message {i}"));
    }
    // Should have been trimmed to IN_MEMORY_TRIM_KEEP
    assert_eq!(session.messages.len(), IN_MEMORY_TRIM_KEEP);
    // Most recent message should be the last one added
    assert_eq!(
        session.messages.last().unwrap().content,
        format!("message {IN_MEMORY_TRIM_THRESHOLD}")
    );
}
```

- [ ] **Step 5: Run the test**

Run: `cargo nextest run -p session -E 'test(trims_messages)' 2>&1`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session/src/manager.rs
git commit -m "fix(session): add in-memory message compaction to prevent unbounded Vec growth

The active desktop session stays in the LRU cache indefinitely, so its
messages Vec grew without bound. Now trims to 500 messages when exceeding
600, matching the SQL compaction behavior."
```

---

### Task 6: Wire tree builder CancellationTokens to parent shutdown

The 8 tree builder subscriber tasks in `builder.rs` are spawned with `CancellationToken::new()` (standalone tokens, not children of the agent's shutdown token). The `JoinHandle`s are dropped immediately (prefixed with `_`). If the agent is ever rebuilt, old tasks keep running alongside new ones — each holding a `broadcast::Receiver` that prevents the ring buffer from being reclaimed.

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:799-949`

- [ ] **Step 1: Identify the shutdown token available in `build()`**

Search `builder.rs` for the shutdown token passed into tree builder `run()` calls. The pattern is:

```rust
let tree_builder_shutdown = CancellationToken::new();  // PROBLEM: standalone token
let _tree_builder_handle = tokio::spawn({
    let shutdown = tree_builder_shutdown.clone();
    async move { builder.run(tree_builder_rx, shutdown).await; }
});
```

We need to find the agent's main shutdown token. Check if `self` has a `shutdown_token` field, or if there's a `CancellationToken` passed into `build()`. If not, the builder should accept one.

- [ ] **Step 2: Replace standalone tokens with child tokens**

For each of the 8 subscriber blocks (NoteTreeBuilder, TaskTreeBuilder, EntityTreeLinker, CommunityBuilder, FinanceTreeBuilder, ProductivityTreeBuilder, OkrTreeBuilder, LearningTreeBuilder), replace:

```rust
// Before (repeated 8 times):
let tree_builder_shutdown = CancellationToken::new();

// After (use the agent's token — either `self.shutdown_token` or a parameter):
let tree_builder_shutdown = agent_shutdown_token.child_token();
```

If no shutdown token exists on the builder, add one:

```rust
// In AgentLoopBuilder struct, add:
shutdown_token: CancellationToken,

// In AgentLoopBuilder::new(), accept it:
pub fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config, shutdown_token: CancellationToken) -> Self {
    // ...
    shutdown_token,
}
```

Then update all 8 occurrences. The exact variable names are:
1. `tree_builder_shutdown` (line 800)
2. `task_tree_shutdown` (line 821)
3. `linker_shutdown` (line 837)
4. `comm_builder_shutdown` (line 859)
5. `finance_tree_shutdown` (line 880)
6. `productivity_tree_shutdown` (line 900)
7. `okr_tree_shutdown` (line 920)
8. `learning_tree_shutdown` (line 941)

- [ ] **Step 3: Build and verify**

Run: `cargo build -p agent 2>&1 | head -50`

If the builder's constructor signature changed, fix callsites (likely in `crates/app-core/src/init/agent.rs` or wherever `AgentLoop::builder()` is called).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "fix(agent): wire tree builder CancellationTokens to parent shutdown

Standalone CancellationToken::new() meant subscriber tasks were never
cancelled on shutdown or agent rebuild. Using child_token() ensures they
are cancelled when the parent token is dropped, preventing duplicate
subscriber accumulation."
```

---

### Task 7: Increase DomainEventBus capacity and strip debug dashboard payloads

Two quick wins: (1) raise broadcast capacity from 64 to 512 so slow subscribers are less likely to hold up the ring buffer, and (2) stop serializing full event payloads in the debug dashboard forwarder.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:134`
- Modify: `crates/desktop/src/app_core.rs:319-324`

- [ ] **Step 1: Raise broadcast capacity**

In `crates/app-core/src/init/mod.rs`, line 134:

```rust
// Before:
let domain_event_bus = Arc::new(bus::DomainEventBus::new(64));

// After:
let domain_event_bus = Arc::new(bus::DomainEventBus::new(512));
```

- [ ] **Step 2: Strip large payloads from debug dashboard emission**

In `crates/desktop/src/app_core.rs`, around lines 319-324, replace the full event serialization with a lightweight payload:

```rust
// Before:
let payload = desktop_shared::cognitive_commands::DomainEventPayload {
    event_type,
    salience: salience_str.to_string(),
    domain: event.domain().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    payload: serde_json::to_value(&event).unwrap_or_default(),
};

// After:
let payload = desktop_shared::cognitive_commands::DomainEventPayload {
    event_type,
    salience: salience_str.to_string(),
    domain: event.domain().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    payload: serde_json::Value::Null,
};
```

This sets `payload` to `null` — the debug dashboard only displays `event_type`, `salience`, `domain`, and `timestamp` anyway. If the dashboard needs some payload data, a more targeted approach would be to serialize only small scalar fields, but `Null` is the safe starting point.

- [ ] **Step 3: Check if the frontend dashboard uses `payload`**

Run: `grep -rn "\.payload" desktop-ui/src/ | grep -i "domain.*event\|cognitive"` to check if any frontend code reads the `payload` field from `DomainEventPayload`. If it does, you may need to keep a truncated version instead of `Null`.

- [ ] **Step 4: Build**

Run: `cargo build -p app-core -p desktop 2>&1 | head -30`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/desktop/src/app_core.rs
git commit -m "fix(desktop): raise DomainEventBus capacity to 512, strip debug dashboard payloads

64 slots was too small for 23+ subscribers — slow consumers held up the
ring buffer. Also stops serializing full event payloads (including note
bodies) for the debug dashboard forwarder, eliminating a per-event
allocation of potentially 100KB+."
```

---

### Task 8: Add in-memory compaction to chatStreamStore (frontend)

The `chatStreamStore` keeps 20 idle sessions in the JS heap, each retaining full tool result strings (10-100KB per tool call). Over time this accumulates significantly.

**Files:**
- Modify: `desktop-ui/src/shared/stores/chatStreamStore.ts:133,204-208,460-487`

- [ ] **Step 1: Add a result truncation constant and helper**

At the top of `chatStreamStore.ts`, near the `MAX_IDLE_SESSIONS` constant:

```typescript
class ChatStreamStore {
  private static MAX_IDLE_SESSIONS = 20;
  private static MAX_TOOL_RESULT_LENGTH = 2000; // ~2KB display limit per tool result
```

- [ ] **Step 2: Truncate tool results in `onToolEnd`**

In the `onToolEnd` method (around line 460), truncate the result before storing:

```typescript
// In the segments push, truncate result:
{
  type: "tool" as const,
  name: payload.name,
  action: payload.action,
  success: payload.success,
  durationMs: payload.durationMs,
  result: payload.result && payload.result.length > ChatStreamStore.MAX_TOOL_RESULT_LENGTH
    ? payload.result.slice(0, ChatStreamStore.MAX_TOOL_RESULT_LENGTH) + "\n… (truncated)"
    : payload.result,
  estimatedTokens: payload.estimatedTokens,
  agent: payload.agent,
},
```

- [ ] **Step 3: Clear segments and transparency when a session becomes idle**

In the `onDone` method (around line 490), after setting `isStreaming: false`, clear the heavy data from the snapshot for non-active sessions. Modify `evictIdleSessions` to also trim retained sessions:

```typescript
private evictIdleSessions(): void {
  if (this.states.size <= ChatStreamStore.MAX_IDLE_SESSIONS) return;

  const idle: string[] = [];
  for (const [key, state] of this.states) {
    if (!state.isStreaming) idle.push(key);
  }
  if (idle.length <= ChatStreamStore.MAX_IDLE_SESSIONS) return;

  const toRemove = idle.length - ChatStreamStore.MAX_IDLE_SESSIONS;
  for (let i = 0; i < toRemove; i++) {
    this.deleteSessionState(idle[i]);
  }
}
```

Also reduce `MAX_IDLE_SESSIONS` from 20 to 5 — there's no reason to keep 20 full snapshots:

```typescript
private static MAX_IDLE_SESSIONS = 5;
```

- [ ] **Step 4: Verify the frontend still works**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/stores/chatStreamStore.ts
git commit -m "fix(desktop-ui): truncate tool results and reduce idle session cache in chatStreamStore

Tool results were stored untruncated (10-100KB each) across 20 idle sessions.
Now truncates to 2KB for display and keeps only 5 idle sessions."
```

---

### Task 9: Replace unbounded MCP channel with bounded

The `tool_list_changed` channel in `McpManager` uses `tokio::sync::mpsc::unbounded_channel`, which can grow without limit if the consumer isn't draining fast enough.

**Files:**
- Modify: `crates/mcp/src/client/manager.rs:73-74`

- [ ] **Step 1: Replace unbounded with bounded channel**

In `crates/mcp/src/client/manager.rs`, around line 73:

```rust
// Before:
let (tool_list_changed_tx, tool_list_changed_rx) =
    tokio::sync::mpsc::unbounded_channel::<String>();

// After:
let (tool_list_changed_tx, tool_list_changed_rx) =
    tokio::sync::mpsc::channel::<String>(32);
```

- [ ] **Step 2: Update the sender type in the struct and handler**

Search for `UnboundedSender<String>` in the mcp crate and change to `mpsc::Sender<String>`. The sender is stored in `KlyntbotClientHandler` and uses `.send()`. With a bounded channel, `send()` is async. If the current usage is `tx.send(name).ok()` (fire-and-forget), change to `tx.try_send(name).ok()` which is non-async and drops the message if the channel is full:

```rust
// Before (in the handler):
let _ = self.tool_list_changed_tx.send(server_name);

// After:
let _ = self.tool_list_changed_tx.try_send(server_name);
```

Similarly, update the receiver side. `UnboundedReceiver::recv()` becomes `Receiver::recv()` — same API, no change needed.

- [ ] **Step 3: Update the struct field type**

Search for `Option<UnboundedReceiver<String>>` and change to `Option<mpsc::Receiver<String>>`.

- [ ] **Step 4: Build**

Run: `cargo build -p mcp 2>&1 | head -30`

- [ ] **Step 5: Commit**

```bash
git add crates/mcp/src/client/manager.rs
git commit -m "fix(mcp): replace unbounded tool_list_changed channel with bounded(32)

Unbounded channel could grow without limit if MCP servers emit
notifications faster than the health check loop drains them."
```

---

### Task 10: Cap file_watcher project_cache HashMap

The `project_cache: HashMap<PathBuf, Option<String>>` in `FileWatcherService` grows one entry per unique parent directory of every filesystem event, with no eviction.

**Files:**
- Modify: `crates/app-core/src/infrastructure/file_watcher.rs:24`

- [ ] **Step 1: Add a size cap to `project_cache`**

Find the insertion point where `project_cache.entry(dir).or_insert_with_key(...)` is called. Add a capacity check before inserting:

```rust
// Before the entry() call:
if self.project_cache.len() > 2000 {
    self.project_cache.clear();
}
```

A simple `clear()` is fine here — the cache is purely a performance optimization, and clearing it causes a brief period of re-detection. An LRU would be more elegant but is unnecessary complexity for a cache that's only preventing repeated `detect_project()` syscalls.

- [ ] **Step 2: Build**

Run: `cargo build -p app-core 2>&1 | head -20`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/infrastructure/file_watcher.rs
git commit -m "fix(file-watcher): cap project_cache at 2000 entries to prevent unbounded growth

The HashMap grew one entry per unique directory path in filesystem events,
with no eviction. Now clears when exceeding 2000 entries."
```

---

### Task 11: Final workspace verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -20`

Expected: clean build, 0 errors.

- [ ] **Step 2: Clippy check**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`

Expected: 0 warnings (per project convention).

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

Expected: all tests pass.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`

Expected: no formatting issues.

- [ ] **Step 5: Frontend build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`

Expected: clean build.

- [ ] **Step 6: Frontend lint**

Run: `cd desktop-ui && bun run lint 2>&1 | tail -10`

Expected: no new errors.

- [ ] **Step 7: Final commit (if any fixups needed)**

```bash
git add -u
git commit -m "chore: fix clippy warnings and formatting from memory leak fixes"
```
