# Memory Optimization Plan (8-11GB → Target <2GB)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the Klyntbot desktop app's runtime memory from 8-11GB down to under 2GB through systematic optimization of the Rust backend and React frontend.

**Architecture:** The app has two main memory consumers: (1) the Rust Tauri backend holding LanceDB vector tables, ONNX embedding model, session caches, and Arrow columnar data in memory, and (2) the React WebView frontend with unvirtualized lists, unbounded stream state, per-node Three.js geometry allocation, and eagerly-loaded heavy dependencies. We attack both sides with targeted fixes ordered by estimated memory impact.

**Tech Stack:** Rust (LanceDB, fastembed/ONNX, SQLite, Arrow), React 19 + Tauri 2, Three.js, Vite

---

## Estimated Memory Budget

| Component | Current | Target | Savings |
|-----------|---------|--------|---------|
| LanceDB fragment mmap + Arrow | 2-4 GB | 200-400 MB | ~2.5 GB |
| ONNX embedding model | 420 MB (idle retained) | 0 MB (aggressive unload) | ~420 MB |
| conv_embeddings full_content | 500 MB-2 GB | 50 MB (preview only) | ~1 GB |
| Session cache (10 × 150 msgs) | 50-300 MB | 20-50 MB | ~100 MB |
| Frontend WebView (unvirtualized) | 500 MB-2 GB | 100-200 MB | ~1 GB |
| Three.js per-node geometry | 200-500 MB | 50-100 MB | ~300 MB |
| Heavy JS deps (eagerly loaded) | 200-400 MB | 50-100 MB | ~200 MB |
| ChatStreamStore unbounded state | 200-500 MB | 50 MB | ~300 MB |
| **Total** | **~8-11 GB** | **~1-1.5 GB** | **~6-8 GB** |

---

## Phase 1: Backend — High-Impact Quick Wins (est. savings: ~4 GB)

### Task 1: Remove `full_content` from `conv_embeddings` — store reference only

The `conv_embeddings` LanceDB table stores the **entire message content** alongside the 384-dim vector. For a user with thousands of conversation turns, this alone can consume 500MB-2GB. The full content is already in SQLite (`session_messages` table) — storing it again in LanceDB is redundant.

**Files:**
- Modify: `crates/storage/src/vector_store/schemas.rs:30-39`
- Modify: `crates/storage/src/vector_store/conv.rs:11-91`
- Modify: `crates/cognitive/src/services/conversation_recall.rs:78-103`
- Modify: `crates/cognitive/src/services/conversation_recall.rs:109-170` (search method)

- [ ] **Step 1: Write test for new conv_embeddings search without full_content**

In `crates/storage/src/vector_store/tests.rs` (or appropriate test module), add a test that verifies `search_conv_embeddings` returns `(id, session_key, role, content_preview, created_at, score)` — 6-tuple without `full_content`.

```rust
#[tokio::test]
async fn test_conv_search_returns_preview_not_full_content() {
    let store = test_vector_store().await;
    let vector = vec![0.1_f32; 384];
    store
        .upsert_embedding(
            "conv_embeddings",
            "msg-1",
            &vector,
            &[
                ("session_key", "test-session"),
                ("role", "user"),
                ("content_preview", "Hello, how are..."),
            ],
        )
        .await
        .unwrap();

    let results = store.search_conv_embeddings(&vector, 10, 0.0).await.unwrap();
    assert_eq!(results.len(), 1);
    let (id, session_key, role, preview, _created_at, score) = &results[0];
    assert_eq!(id, "msg-1");
    assert_eq!(session_key, "test-session");
    assert_eq!(role, "user");
    assert_eq!(preview, "Hello, how are...");
    assert!(*score > 0.9);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(conv_search_returns_preview_not_full_content)'`
Expected: FAIL — `full_content` column still in schema, return type is 7-tuple.

- [ ] **Step 3: Remove `full_content` from conv schema**

In `crates/storage/src/vector_store/schemas.rs`, remove the `full_content` field from `conv_schema()`:

```rust
pub(crate) fn conv_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("session_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("content_preview", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
    ])
}
```

- [ ] **Step 4: Update `search_conv_embeddings` to return 6-tuple**

In `crates/storage/src/vector_store/conv.rs`, remove `full_content` column read. Change the return type from `Vec<(String, String, String, String, String, String, f64)>` (7 items) to `Vec<(String, String, String, String, String, f64)>` (6 items — dropping `full_content`):

```rust
pub async fn search_conv_embeddings(
    &self,
    query: &[f32],
    limit: usize,
    threshold: f64,
) -> Result<Vec<(String, String, String, String, String, f64)>, StorageError> {
    let tbl = self.get_table("conv_embeddings").await?;

    let results = tbl
        .query()
        .nearest_to(query)
        .map_err(|e| StorageError::Vector(format!("nearest_to: {e}")))?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| StorageError::Vector(format!("LanceDB query conv_embeddings: {e}")))?;

    let batches: Vec<arrow_array::RecordBatch> = results
        .try_collect()
        .await
        .map_err(|e| StorageError::Vector(format!("collect conv results: {e}")))?;

    let mut out = Vec::new();
    for batch in &batches {
        let id_col = batch
            .column_by_name("id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let sk_col = batch
            .column_by_name("session_key")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let role_col = batch
            .column_by_name("role")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let preview_col = batch
            .column_by_name("content_preview")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let created_col = batch
            .column_by_name("created_at")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let dist_col = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

        let (Some(id_col), Some(sk_col), Some(role_col), Some(preview_col)) =
            (id_col, sk_col, role_col, preview_col)
        else {
            continue;
        };

        for i in 0..batch.num_rows() {
            let score = match dist_col {
                Some(d) => 1.0 - d.value(i) as f64,
                None => 1.0,
            };
            if score >= threshold {
                let created_at = created_col
                    .map(|c| c.value(i).to_string())
                    .unwrap_or_default();
                out.push((
                    id_col.value(i).to_string(),
                    sk_col.value(i).to_string(),
                    role_col.value(i).to_string(),
                    preview_col.value(i).to_string(),
                    created_at,
                    score,
                ));
            }
        }
    }
    Ok(out)
}
```

- [ ] **Step 5: Update `ConversationRecallService::store_message` — stop storing full_content**

In `crates/cognitive/src/services/conversation_recall.rs`, remove `full_content` from the `upsert_embedding` metadata:

```rust
pub async fn store_message(
    &self,
    id: &str,
    content: &str,
    metadata: RecallMetadata,
) -> common::Result<()> {
    let text = format!("{}: {}", metadata.role, content);
    let vector = self.embedder.embed(&text).await?;

    let preview = truncate_at_boundary(content, 100);

    self.vector_store
        .upsert_embedding(
            "conv_embeddings",
            id,
            &vector,
            &[
                ("session_key", metadata.session_key.as_str()),
                ("role", metadata.role.as_str()),
                ("content_preview", preview),
            ],
        )
        .await?;
    Ok(())
}
```

- [ ] **Step 6: Update `ConversationRecallService::search` to match new 6-tuple**

In the `search` method of the same file, update the destructuring of results from `search_conv_embeddings` to match the new 6-tuple (no `full_content`). The `RecallResult.content` should now use `preview` since full content is no longer available from the vector store:

```rust
// Update destructuring inside the search method — was 7-tuple, now 6-tuple:
for (id, session_key, role, preview, created_at_str, raw_score) in raw_results {
    // ... existing time-decay logic ...
    results.push(RecallResult {
        id,
        session_key,
        role,
        content: preview, // Was full_content, now preview
        score: decayed_score,
        created_at: parsed_created_at,
    });
}
```

- [ ] **Step 7: Fix all compilation errors from the 7→6 tuple change**

Run: `cargo build --workspace 2>&1 | head -50`

Search for any other callers of `search_conv_embeddings` and update them. Common pattern: `grep -r "search_conv_embeddings" crates/`.

- [ ] **Step 8: Run tests**

Run: `cargo nextest run --workspace -E 'test(conv)'`
Expected: All pass.

- [ ] **Step 9: Drop and recreate conv_embeddings table**

Since we're pre-release (no migration needed), the schema change requires dropping the old table. Add a migration or startup check. The simplest approach: in `VectorStore::connect`, if the `conv_embeddings` table exists but has a `full_content` column, drop and recreate it. This is safe pre-release.

In `crates/storage/src/vector_store/mod.rs`, add a helper in `connect()` right before `ensure_table("conv_embeddings", ...)`:

```rust
// Pre-release schema migration: drop conv_embeddings if it has the old full_content column.
if existing.iter().any(|t| t == "conv_embeddings") {
    let tbl = store.db.open_table("conv_embeddings").execute().await
        .map_err(|e| StorageError::Vector(format!("open conv_embeddings for migration: {e}")))?;
    let schema = tbl.schema().await
        .map_err(|e| StorageError::Vector(format!("conv_embeddings schema: {e}")))?;
    if schema.column_with_name("full_content").is_some() {
        store.db.drop_table("conv_embeddings").await
            .map_err(|e| StorageError::Vector(format!("drop old conv_embeddings: {e}")))?;
        tracing::info!("Dropped old conv_embeddings table (had full_content column)");
    }
}
```

Put this right before the `ensure_table("conv_embeddings", ...)` call. Then update the `existing` list to account for the drop (or refetch table names).

- [ ] **Step 10: Run full test suite and commit**

Run: `cargo nextest run --workspace`
Expected: All pass.

```bash
git add crates/storage/src/vector_store/schemas.rs crates/storage/src/vector_store/conv.rs crates/storage/src/vector_store/mod.rs crates/cognitive/src/services/conversation_recall.rs
git commit -m "perf(storage): remove full_content from conv_embeddings to reduce LanceDB memory

Store only content_preview (100 chars) instead of full message content
in the vector table. Full content is already in SQLite session_messages.
Saves 500MB-2GB depending on conversation history depth."
```

---

### Task 2: Reduce LanceDB cache sizes and add periodic compaction

The LanceDB index cache is 128MB and metadata cache is 32MB. For a single-user desktop app, these can be reduced. More importantly, LanceDB's copy-on-write storage creates new fragment files on every upsert — without frequent compaction, these get memory-mapped and RSS grows unboundedly.

**Files:**
- Modify: `crates/storage/src/vector_store/mod.rs:21-24`
- Modify: `crates/app-core/src/init/storage.rs:46-57`
- Modify: `crates/app-core/src/init/mod.rs` (add periodic compaction timer)

- [ ] **Step 1: Reduce LanceDB cache sizes**

In `crates/storage/src/vector_store/mod.rs`, reduce the cache constants:

```rust
/// Index cache: 32 MB is enough for hot partitions across 12 tables in a single-user app.
/// Default is 6 GiB (designed for cloud/server).
const LANCE_INDEX_CACHE_BYTES: usize = 32 * 1024 * 1024;
/// Metadata cache: 8 MB is ample for 12 tables worth of manifests.
const LANCE_METADATA_CACHE_BYTES: usize = 8 * 1024 * 1024;
```

- [ ] **Step 2: Add periodic compaction (every 30 minutes)**

In `crates/app-core/src/init/mod.rs`, after the embedding idle-unload timer (around line 299), add a compaction timer:

```rust
// Periodic LanceDB compaction — merge fragment files every 30 minutes
// to prevent unbounded RSS growth from copy-on-write fragment accumulation.
if let Some(vs) = &vector_store {
    let vs_compact = vs.clone();
    spawn_periodic_timer(&shutdown_token, 1800, move || {
        let vs = vs_compact.clone();
        tokio::spawn(async move {
            if let Err(e) = vs.optimize_all_tables().await {
                tracing::warn!("Periodic LanceDB compaction failed: {e}");
            }
        });
    });
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace -E 'test(vector) | test(lance) | test(storage)'`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/vector_store/mod.rs crates/app-core/src/init/mod.rs
git commit -m "perf(storage): reduce LanceDB caches (128→32MB index, 32→8MB metadata) and add periodic compaction

Adds a 30-minute compaction timer to merge fragment files and prevent
RSS growth from copy-on-write accumulation. Reduces total LanceDB cache
budget from 160MB to 40MB."
```

---

### Task 3: Aggressive ONNX model unloading

The embedding model (~420MB) has a 60-second idle timeout with 30-second polling. In practice, this means the model can stay in memory for up to 90 seconds after last use. For a desktop app, we should unload after 15 seconds.

**Files:**
- Modify: `crates/tools/src/embedding/embedding_engine.rs:27`
- Modify: `crates/app-core/src/init/mod.rs:296`

- [ ] **Step 1: Reduce idle timeout to 15 seconds**

In `crates/tools/src/embedding/embedding_engine.rs`, change:

```rust
/// Idle timeout before the ONNX model is unloaded from memory (15 seconds).
const EMBEDDING_IDLE_SECS: u64 = 15;
```

- [ ] **Step 2: Reduce poll interval to 10 seconds**

In `crates/app-core/src/init/mod.rs`, change the timer from 30s to 10s:

```rust
{
    let engine = Arc::clone(&embedding_engine);
    spawn_periodic_timer(&shutdown_token, 10, move || {
        engine.unload_if_idle();
    });
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace -E 'test(embed)'`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/embedding/embedding_engine.rs crates/app-core/src/init/mod.rs
git commit -m "perf(embedding): reduce ONNX model idle timeout from 60s to 15s

Poll every 10s instead of 30s. Model is now unloaded within ~25s of
last use instead of ~90s. Saves ~420MB when not actively embedding."
```

---

### Task 4: Reduce session cache size and trim threshold

Currently 10 sessions cached with up to 150 messages each. Reduce to 5 sessions and lower the trim threshold to 50 messages (since full history is in SQLite).

**Files:**
- Modify: `crates/config/src/schema/conversation.rs:40-42`
- Modify: `crates/session/src/manager.rs:208-219`

- [ ] **Step 1: Reduce default max_cache_size from 10 to 5**

In `crates/config/src/schema/conversation.rs`:

```rust
fn default_max_cache_size() -> usize {
    5
}
```

- [ ] **Step 2: Reduce in-memory trim threshold and keep count**

In `crates/session/src/manager.rs`:

```rust
/// In-memory trim threshold: trim the Vec when it exceeds this.
const IN_MEMORY_TRIM_THRESHOLD: usize = 60;

/// Number of messages to keep after an in-memory trim.
const IN_MEMORY_TRIM_KEEP: usize = 40;
```

- [ ] **Step 3: Update test assertions**

Run: `cargo nextest run -p config -E 'test(session_max_cache_size_default)'`

Update the test in `crates/config/src/schema/conversation.rs`:

```rust
#[test]
fn test_session_max_cache_size_default() {
    let config = SessionConfig::default();
    assert_eq!(config.max_cache_size, 5);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run --workspace -E 'test(session)'`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/conversation.rs crates/session/src/manager.rs
git commit -m "perf(session): reduce cache to 5 sessions, trim at 60 messages

Full history lives in SQLite — in-memory cache is just for hot sessions.
Reduces session memory footprint by ~60%."
```

---

## Phase 2: Frontend — High-Impact Optimizations (est. savings: ~2 GB)

### Task 5: Virtualize MessageList with `react-window`

The `MessageList` component renders ALL messages in the DOM. For long conversations (500+ messages), this creates thousands of DOM nodes + React fiber overhead. Using `react-window` with dynamic sizing avoids rendering off-screen messages.

**Files:**
- Modify: `desktop-ui/package.json` (add `react-window`)
- Create: `desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx`
- Modify: `desktop-ui/src/features/chat/components/MessageList.tsx`

- [ ] **Step 1: Install react-window and types**

Run: `cd desktop-ui && bun add react-window && bun add -D @types/react-window`

- [ ] **Step 2: Create VirtualizedMessageList component**

Create `desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx`:

```tsx
import { VariableSizeList, type ListOnScrollProps } from "react-window";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type {
  ActiveInteraction,
  ChatMessage,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "@shared/types";
import { CollapsedInteraction } from "./CollapsedInteraction";
import { MarkdownContent } from "./MarkdownContent";
import { SegmentedMessage } from "./SegmentedMessage";
import { TokenBadge } from "./TokenBadge";

interface VirtualizedMessageListProps {
  messages: ChatMessage[];
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  sessionKey: string;
  onInteractionSubmitted: () => void;
  liveTransparency: TransparencyData | null;
  activeDelegateAgent?: string | null;
  personaMessages?: PersonaSegment[];
  statusPhase?: string | null;
  height: number;
}

const ESTIMATED_ROW_HEIGHT = 80;

export function VirtualizedMessageList({
  messages,
  segments,
  isStreaming,
  activeTools,
  error,
  activeInteraction,
  sessionKey,
  onInteractionSubmitted,
  liveTransparency,
  activeDelegateAgent,
  personaMessages,
  statusPhase,
  height,
}: VirtualizedMessageListProps) {
  const listRef = useRef<VariableSizeList>(null);
  const rowHeights = useRef<Map<number, number>>(new Map());
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  // Extra items: streaming segment, error, interaction, scroll-to-bottom
  const extraCount =
    (segments.length > 0 || activeTools.length > 0 ? 1 : 0) +
    (isStreaming && segments.length === 0 && activeTools.length === 0 ? 1 : 0) +
    (error ? 1 : 0) +
    (activeInteraction ? 1 : 0);

  const itemCount = messages.length + extraCount;

  const getItemSize = useCallback(
    (index: number) => rowHeights.current.get(index) ?? ESTIMATED_ROW_HEIGHT,
    [],
  );

  const setRowHeight = useCallback((index: number, size: number) => {
    const prev = rowHeights.current.get(index);
    if (prev !== size) {
      rowHeights.current.set(index, size);
      listRef.current?.resetAfterIndex(index, false);
    }
  }, []);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    if (!userScrolledUp && itemCount > 0) {
      listRef.current?.scrollToItem(itemCount - 1, "end");
    }
  }, [itemCount, userScrolledUp]);

  const onScroll = useCallback(
    ({ scrollOffset, scrollUpdateWasRequested }: ListOnScrollProps) => {
      if (scrollUpdateWasRequested) return;
      const el = listRef.current;
      if (!el) return;
      // Detect if user scrolled away from bottom
      const outerEl = (el as unknown as { _outerRef: HTMLElement })._outerRef;
      if (outerEl) {
        const isNearBottom = outerEl.scrollHeight - scrollOffset - outerEl.clientHeight < 100;
        setUserScrolledUp(!isNearBottom);
      }
    },
    [],
  );

  const Row = useCallback(
    ({ index, style }: { index: number; style: React.CSSProperties }) => {
      const measureRef = useCallback(
        (el: HTMLDivElement | null) => {
          if (el) {
            const observed = new ResizeObserver((entries) => {
              for (const entry of entries) {
                setRowHeight(index, entry.contentRect.height + 24); // 24px gap
              }
            });
            observed.observe(el);
            // Initial measurement
            setRowHeight(index, el.getBoundingClientRect().height + 24);
          }
        },
        [index],
      );

      // Message rows
      if (index < messages.length) {
        const msg = messages[index];
        return (
          <div style={style}>
            <div ref={measureRef} className="pb-6">
              <div
                className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
              >
                {msg.role === "user" ? (
                  <div className="max-w-[85%] glass-bubble-user px-5 py-3.5">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-foreground">
                      {msg.content}
                    </p>
                  </div>
                ) : msg.role === "interaction" ? (
                  <CollapsedInteraction content={msg.content} />
                ) : (
                  <div className="w-full">
                    {msg.segments && msg.segments.length > 0 ? (
                      <SegmentedMessage
                        segments={msg.segments}
                        plan={msg.transparency?.plan}
                      />
                    ) : (
                      <MarkdownContent content={msg.content} />
                    )}
                    {msg.transparency && <TokenBadge transparency={msg.transparency} />}
                  </div>
                )}
              </div>
            </div>
          </div>
        );
      }

      // Streaming segments row
      const streamIndex = messages.length;
      if (
        index === streamIndex &&
        (segments.length > 0 || activeTools.length > 0)
      ) {
        return (
          <div style={style}>
            <div ref={measureRef} className="pb-6">
              <div className="flex justify-start">
                <div className="w-full">
                  <SegmentedMessage
                    segments={segments}
                    activeTools={activeTools}
                    isStreaming={isStreaming}
                    activeDelegateAgent={activeDelegateAgent}
                    plan={liveTransparency?.plan}
                  />
                  {liveTransparency && <TokenBadge transparency={liveTransparency} />}
                </div>
              </div>
            </div>
          </div>
        );
      }

      return <div style={style} />;
    },
    [
      messages,
      segments,
      activeTools,
      isStreaming,
      activeDelegateAgent,
      liveTransparency,
      setRowHeight,
    ],
  );

  return (
    <VariableSizeList
      ref={listRef}
      height={height}
      itemCount={itemCount}
      itemSize={getItemSize}
      width="100%"
      onScroll={onScroll}
      overscanCount={5}
    >
      {Row}
    </VariableSizeList>
  );
}
```

- [ ] **Step 3: Update MessageList to use virtualization for long conversations**

In `desktop-ui/src/features/chat/components/MessageList.tsx`, add a threshold: use the virtualized list when messages exceed 50, keep the simple list for shorter conversations (avoids unnecessary complexity for typical usage):

```tsx
import { VirtualizedMessageList } from "./VirtualizedMessageList";

// At the top of the MessageList component, add:
const containerRef = useRef<HTMLDivElement>(null);
const [containerHeight, setContainerHeight] = useState(600);

useEffect(() => {
  if (!containerRef.current) return;
  const observer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      setContainerHeight(entry.contentRect.height);
    }
  });
  observer.observe(containerRef.current);
  return () => observer.disconnect();
}, []);

// Virtualize when conversation is long
if (messages.length > 50) {
  return (
    <div ref={containerRef} className="h-full">
      <VirtualizedMessageList
        messages={messages}
        segments={segments}
        isStreaming={isStreaming}
        activeTools={activeTools}
        error={error}
        activeInteraction={activeInteraction}
        sessionKey={sessionKey}
        onInteractionSubmitted={onInteractionSubmitted}
        liveTransparency={liveTransparency}
        activeDelegateAgent={activeDelegateAgent}
        personaMessages={personaMessages}
        statusPhase={statusPhase}
        height={containerHeight}
      />
    </div>
  );
}

// ... existing simple render for short conversations ...
```

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx desktop-ui/src/features/chat/components/MessageList.tsx
git commit -m "perf(ui): virtualize MessageList for conversations >50 messages

Uses react-window VariableSizeList with dynamic row measurement.
Only activates for long conversations to avoid complexity overhead
for typical short chats. Reduces DOM nodes from N to ~15 visible."
```

---

### Task 6: Cap ChatStreamStore segments and add session eviction

The `ChatStreamStore` accumulates unlimited segments and only evicts 5 idle sessions on `agent:done`. For long streaming sessions, segments array grows without bound.

**Files:**
- Modify: `desktop-ui/src/shared/stores/chatStreamStore.ts:132-141, 248-257`

- [ ] **Step 1: Add segment cap constant and enforce in flushText**

In `desktop-ui/src/shared/stores/chatStreamStore.ts`, add a cap and enforce it:

```typescript
class ChatStreamStore {
  private static MAX_IDLE_SESSIONS = 5;
  private static MAX_TOOL_RESULT_LENGTH = 2000;
  private static MAX_SEGMENTS = 200; // Cap segments to prevent unbounded growth
```

In the `flushText` method (around line 248), after appending the new segment, trim if over cap:

```typescript
private flushText(sessionKey: string): void {
  this.cancelRaf(sessionKey);
  const text = this.textBuffers.get(sessionKey) || "";
  if (!text) return;

  this.updateState(sessionKey, (s) => {
    const last = s.segments[s.segments.length - 1];
    let newSegments: MessageSegment[];
    if (last && last.type === "text") {
      newSegments = [...s.segments.slice(0, -1), { type: "text", content: text }];
    } else {
      newSegments = [...s.segments, { type: "text", content: text }];
    }
    // Cap segments to prevent unbounded memory growth
    if (newSegments.length > ChatStreamStore.MAX_SEGMENTS) {
      newSegments = newSegments.slice(-ChatStreamStore.MAX_SEGMENTS);
    }
    return { ...s, segments: newSegments };
  });
}
```

- [ ] **Step 2: Evict idle sessions more aggressively on done**

In the `onDone` handler, reduce the idle session retention. Find the `onDone` method and after setting the state, evict sessions more aggressively:

```typescript
// After the existing idle session eviction logic, also clear text buffers
// and transparency data for the completed session to free memory:
this.textBuffers.delete(sessionKey);
```

- [ ] **Step 3: Add periodic cleanup of old sessions**

Add a method to prune sessions that haven't been active for 5 minutes:

```typescript
/** Remove sessions idle for more than 5 minutes. Called from outside or on a timer. */
pruneIdleSessions(): void {
  // Keep only the most recent MAX_IDLE_SESSIONS sessions
  if (this.states.size <= ChatStreamStore.MAX_IDLE_SESSIONS) return;
  const streaming = new Set<string>();
  for (const [key, state] of this.states) {
    if (state.isStreaming) streaming.add(key);
  }
  // Evict non-streaming sessions beyond the limit
  let toEvict = this.states.size - ChatStreamStore.MAX_IDLE_SESSIONS;
  for (const key of this.states.keys()) {
    if (toEvict <= 0) break;
    if (streaming.has(key)) continue;
    this.states.delete(key);
    this.textBuffers.delete(key);
    this.onDoneCallbacks.delete(key);
    toEvict--;
  }
  if (toEvict > 0) this.notify();
}
```

Call `this.pruneIdleSessions()` at the end of `startStream()`.

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/stores/chatStreamStore.ts
git commit -m "perf(ui): cap stream segments at 200 and prune idle sessions

Prevents unbounded segment array growth during long streaming sessions.
Aggressively prunes non-streaming sessions beyond the 5-session limit."
```

---

### Task 7: Pool Three.js geometry and materials in brain view

The brain view creates a new `Geometry` and `MeshStandardMaterial` for every node. For a 1000-node graph, that's 1000 geometries + 1000 materials in GPU memory. Pool common geometries by type/size and share materials by color.

**Files:**
- Create: `desktop-ui/src/features/notes/lib/geometryPool.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useBrainView.ts:36-131`

- [ ] **Step 1: Create geometry and material pool**

Create `desktop-ui/src/features/notes/lib/geometryPool.ts`:

```typescript
import type { BufferGeometry, MeshStandardMaterial } from "three";
import {
  createEntityGeometry,
  createEntityMaterial,
  createFinanceGeometry,
  createFinanceMaterial,
  createLearningGeometry,
  createLearningMaterial,
  createNodeGeometry,
  createNodeMaterial,
  createOkrGeometry,
  createOkrMaterial,
  createProductivityGeometry,
  createProductivityMaterial,
  createProjectGeometry,
  createProjectMaterial,
  createTreeMaterial,
} from "./graphMaterials";

/** Round to nearest bucket to reduce unique geometry instances. */
function bucketSize(value: number, step: number): number {
  return Math.round(value / step) * step;
}

const geometryCache = new Map<string, BufferGeometry>();
const materialCache = new Map<string, MeshStandardMaterial>();

export function getPooledGeometry(
  type: string,
  size: number,
  linkCount: number,
): BufferGeometry {
  const bucketed = bucketSize(size, 2);
  const key = `${type}:${bucketed}:${bucketSize(linkCount, 3)}`;

  let geo = geometryCache.get(key);
  if (geo) return geo;

  switch (type) {
    case "entity":
      geo = createEntityGeometry(linkCount);
      break;
    case "tree_section":
    case "tree_text": {
      const radius = type === "tree_text" ? 3 : 3 + (Math.min(linkCount, 5) / 5) * 2;
      geo = createNodeGeometry(radius * 2);
      break;
    }
    case "finance":
      geo = createFinanceGeometry(bucketed);
      break;
    case "productivity":
      geo = createProductivityGeometry(bucketed);
      break;
    case "okr":
      geo = createOkrGeometry(bucketed);
      break;
    case "learning":
      geo = createLearningGeometry(bucketed);
      break;
    case "project":
      geo = createProjectGeometry(bucketed);
      break;
    default:
      geo = createNodeGeometry(bucketed * 0.3);
      break;
  }

  geometryCache.set(key, geo);
  return geo;
}

export function getPooledMaterial(
  type: string,
  color: string,
  emissiveIntensity: number,
): MeshStandardMaterial {
  const key = `${type}:${color}:${emissiveIntensity.toFixed(1)}`;

  let mat = materialCache.get(key);
  if (mat) return mat;

  switch (type) {
    case "entity":
      mat = createEntityMaterial(color);
      break;
    case "tree_section":
      mat = createTreeMaterial(color, 0.6);
      break;
    case "tree_text":
      mat = createTreeMaterial(color, 0.3);
      break;
    case "finance":
      mat = createFinanceMaterial(color);
      break;
    case "productivity":
      mat = createProductivityMaterial(color);
      break;
    case "okr":
      mat = createOkrMaterial(color);
      break;
    case "learning":
      mat = createLearningMaterial(color);
      break;
    case "project":
      mat = createProjectMaterial(color);
      break;
    default:
      mat = createNodeMaterial(color, emissiveIntensity);
      break;
  }

  mat.userData = { baseEmissive: emissiveIntensity };
  materialCache.set(key, mat);
  return mat;
}

/** Dispose all cached geometries and materials. Call on unmount. */
export function disposePool(): void {
  for (const geo of geometryCache.values()) geo.dispose();
  for (const mat of materialCache.values()) mat.dispose();
  geometryCache.clear();
  materialCache.clear();
}
```

- [ ] **Step 2: Update useBrainView to use pooled geometry/materials**

In `desktop-ui/src/features/notes/hooks/useBrainView.ts`, replace direct `createNodeGeometry`/`createNodeMaterial` calls with pooled versions:

```typescript
import { getPooledGeometry, getPooledMaterial, disposePool } from "../lib/geometryPool";

// Replace the nodeThreeObject callback body:
const nodeThreeObject = useCallback((node: ForceNode) => {
  const nodeType = node.nodeType ?? "note";
  let emissiveIntensity: number;

  switch (nodeType) {
    case "entity": emissiveIntensity = 0.6; break;
    case "tree_section": emissiveIntensity = 0.2; break;
    case "tree_text": emissiveIntensity = 0.1; break;
    case "finance": emissiveIntensity = 0.55; break;
    case "productivity": emissiveIntensity = 0.5; break;
    case "okr": emissiveIntensity = 0.6; break;
    case "learning": emissiveIntensity = 0.5; break;
    case "project": emissiveIntensity = 0.55; break;
    default: emissiveIntensity = Math.min(0.3 + (0.7 * node.linkCount) / 15, 1); break;
  }

  const geometry = getPooledGeometry(nodeType, node.size, node.linkCount);
  const material = getPooledMaterial(nodeType, node.color, emissiveIntensity);
  const mesh = new Mesh(geometry, material);
  mesh.userData = { nodeId: node.id };
  return mesh;
}, []);
```

- [ ] **Step 3: Add cleanup on unmount**

In the component that uses `useBrainView`, add a cleanup effect:

```typescript
useEffect(() => {
  return () => disposePool();
}, []);
```

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/lib/geometryPool.ts desktop-ui/src/features/notes/hooks/useBrainView.ts
git commit -m "perf(ui): pool Three.js geometries and materials in brain view

Nodes with similar type/size/color share geometry and material instances
instead of creating unique objects per node. For a 1000-node graph,
reduces GPU objects from 2000 to ~50-100. Pool is disposed on unmount."
```

---

### Task 8: Lazy-load heavy dependencies (Three.js, Mermaid, Recharts)

Three.js (~400KB), Mermaid (~150KB), and Recharts (~100KB) are loaded at startup even if the user never visits the brain view or views a chart. Use `React.lazy` + dynamic imports.

**Files:**
- Modify: Wherever `BrainView3D`, `MermaidDiagram`, and chart components are imported

- [ ] **Step 1: Identify static imports of heavy components**

Run: `cd desktop-ui && grep -rn "from.*react-force-graph\|from.*mermaid\|from.*recharts\|from.*three" src/ --include="*.tsx" --include="*.ts" | grep -v node_modules | grep -v "\.test\."` to find all direct imports.

- [ ] **Step 2: Convert brain view to lazy import**

Find the file that imports the brain view component and convert to lazy:

```typescript
import { lazy, Suspense } from "react";

const BrainView3D = lazy(() => import("./BrainView3D"));

// In JSX:
<Suspense fallback={<div className="flex items-center justify-center h-full text-muted-foreground text-sm">Loading brain view...</div>}>
  <BrainView3D {...props} />
</Suspense>
```

- [ ] **Step 3: Convert mermaid rendering to lazy import**

Find where Mermaid is used and lazy-load it. Mermaid is likely in a markdown renderer component:

```typescript
const MermaidBlock = lazy(() => import("./MermaidBlock"));
```

- [ ] **Step 4: Convert recharts to lazy import**

Find chart components and lazy-load them:

```typescript
const FinanceChart = lazy(() => import("./FinanceChart"));
```

- [ ] **Step 5: Add code splitting config to Vite**

In `desktop-ui/vite.config.ts`, add manual chunks for heavy dependencies:

```typescript
build: {
  target: "esnext",
  minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
  sourcemap: !!process.env.TAURI_ENV_DEBUG,
  rollupOptions: {
    output: {
      manualChunks: {
        three: ["three", "react-force-graph-2d", "react-force-graph-3d"],
        mermaid: ["mermaid"],
        recharts: ["recharts"],
        tiptap: [
          "@tiptap/react",
          "@tiptap/starter-kit",
          "@tiptap/pm",
        ],
      },
    },
  },
},
```

- [ ] **Step 6: Run frontend tests and verify build**

Run: `cd desktop-ui && bun run test && bun run build`
Expected: All pass, build succeeds with chunked output.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/vite.config.ts desktop-ui/src/
git commit -m "perf(ui): lazy-load Three.js, Mermaid, Recharts and split vendor chunks

Heavy dependencies are now loaded on-demand instead of at startup.
Reduces initial JS payload by ~650KB and defers GPU memory allocation
until the brain view is actually opened."
```

---

## Phase 3: Backend — Medium-Impact Optimizations (est. savings: ~500 MB)

### Task 9: Add LanceDB table row count limits with automatic pruning

Without pruning, tables like `conv_embeddings` and `activity_embeddings` grow unboundedly. Add a configurable row limit per table with LRU-style pruning.

**Files:**
- Modify: `crates/storage/src/vector_store/maintenance.rs`
- Modify: `crates/app-core/src/init/storage.rs` (add pruning to startup)

- [ ] **Step 1: Add prune method to VectorStore**

In `crates/storage/src/vector_store/maintenance.rs`, add a method that deletes the oldest rows when a table exceeds a limit:

```rust
/// Prune a table to at most `max_rows` by deleting the oldest entries.
///
/// Uses the `ts_column` (a timestamp string in ISO-8601 format) to determine age.
/// Returns the number of rows deleted.
pub async fn prune_table(
    &self,
    table: &str,
    ts_column: &str,
    max_rows: usize,
) -> Result<usize, StorageError> {
    let tbl = match self.get_table(table).await {
        Ok(t) => t,
        Err(_) => return Ok(0),
    };

    let row_count = tbl
        .count_rows(None)
        .await
        .map_err(|e| StorageError::Vector(format!("count {table}: {e}")))?;

    if row_count <= max_rows {
        return Ok(0);
    }

    let to_delete = row_count - max_rows;

    // Find the cutoff timestamp: the Nth oldest row's timestamp
    let results = tbl
        .query()
        .select(Select::columns(&[ts_column]))
        .limit(to_delete)
        .execute()
        .await
        .map_err(|e| StorageError::Vector(format!("prune scan {table}: {e}")))?;

    let batches: Vec<arrow_array::RecordBatch> = results
        .try_collect()
        .await
        .map_err(|e| StorageError::Vector(format!("prune collect {table}: {e}")))?;

    let mut timestamps: Vec<String> = Vec::new();
    for batch in &batches {
        if let Some(col) = batch
            .column_by_name(ts_column)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        {
            for i in 0..batch.num_rows() {
                timestamps.push(col.value(i).to_string());
            }
        }
    }

    if let Some(cutoff) = timestamps.iter().max() {
        let safe_cutoff = sanitize_predicate_value(cutoff)?;
        let predicate = format!("{ts_column} <= '{safe_cutoff}'");
        tbl.delete(&predicate)
            .await
            .map_err(|e| StorageError::Vector(format!("prune delete {table}: {e}")))?;
        tracing::info!("Pruned {to_delete} rows from {table} (cutoff: {cutoff})");
    }

    Ok(to_delete)
}
```

- [ ] **Step 2: Add imports needed**

Add to the top of `maintenance.rs`:

```rust
use lancedb::query::Select;
```

(Verify this import is correct — `Select` may already be imported.)

- [ ] **Step 3: Add pruning to startup compaction**

In `crates/app-core/src/init/storage.rs`, inside the background spawn (around line 48-57), add pruning after compaction:

```rust
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    if let Err(e) = vs_bg.optimize_all_tables().await {
        warn!("LanceDB startup compaction failed (non-fatal): {e}");
    }
    // Prune large tables to prevent unbounded growth
    const MAX_CONV_ROWS: usize = 10_000;
    const MAX_ACTIVITY_ROWS: usize = 50_000;
    const MAX_COGNITIVE_ROWS: usize = 20_000;
    let _ = vs_bg.prune_table("conv_embeddings", "created_at", MAX_CONV_ROWS).await;
    let _ = vs_bg.prune_table("activity_embeddings", "updated_at", MAX_ACTIVITY_ROWS).await;
    let _ = vs_bg.prune_table("cognitive_fact_embeddings", "updated_at", MAX_COGNITIVE_ROWS).await;

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    if let Err(e) = vs_bg.ensure_indexes(256).await {
        warn!("ANN index creation failed (non-fatal): {e}");
    }
});
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run --workspace -E 'test(prune) | test(vector) | test(maintenance)'`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/vector_store/maintenance.rs crates/app-core/src/init/storage.rs
git commit -m "perf(storage): add row-count pruning for LanceDB tables

Prunes conv_embeddings to 10K rows, activity to 50K, cognitive facts
to 20K at startup. Prevents unbounded table growth that leads to
memory-mapped fragment accumulation."
```

---

### Task 10: Truncate tool result content in ChatStreamStore

Tool results (especially from large searches, note content, etc.) are stored in their entirety in `StreamSnapshot.segments`. The `MAX_TOOL_RESULT_LENGTH` constant exists but may not be applied everywhere.

**Files:**
- Modify: `desktop-ui/src/shared/stores/chatStreamStore.ts`

- [ ] **Step 1: Find the onToolEnd handler and verify truncation**

Search for the `onToolEnd` handler in the chatStreamStore. Verify that tool result content is truncated to `MAX_TOOL_RESULT_LENGTH` (2000 chars). If not, add truncation:

```typescript
private onToolEnd(p: ToolEndPayload): void {
  this.updateState(p.sessionKey, (s) => {
    // Truncate tool result to prevent large results from bloating memory
    const result = p.result && p.result.length > ChatStreamStore.MAX_TOOL_RESULT_LENGTH
      ? p.result.slice(0, ChatStreamStore.MAX_TOOL_RESULT_LENGTH) + "…"
      : p.result;
    // ... rest of handler using truncated `result` ...
  });
}
```

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/stores/chatStreamStore.ts
git commit -m "perf(ui): enforce tool result truncation in stream store

Large tool results (note content, search results) are now capped at
2000 chars in the stream snapshot to prevent memory bloat."
```

---

## Phase 4: Verification

### Task 11: Memory profiling and validation

After all optimizations are applied, profile the app to verify memory reduction.

- [ ] **Step 1: Build release binary**

Run: `cargo build --release -p desktop`

- [ ] **Step 2: Profile with Activity Monitor**

Launch the app and monitor RSS in Activity Monitor. Expected: < 2GB at idle after startup.

- [ ] **Step 3: Stress test conversation memory**

Send 100+ messages in a conversation and verify memory doesn't grow unboundedly. Check that session trimming kicks in at 60 messages.

- [ ] **Step 4: Verify embedding model unload**

After sending a message (which triggers embedding), wait 25 seconds and verify RSS drops by ~420MB as the model is unloaded.

- [ ] **Step 5: Verify brain view doesn't leak**

Open the brain view, close it, and verify GPU memory is released (check with `ioreg` or Activity Monitor GPU tab).

- [ ] **Step 6: Run full test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run test
```

- [ ] **Step 7: Final commit**

```bash
git add -A
git commit -m "perf: memory optimization complete — target <2GB achieved"
```
