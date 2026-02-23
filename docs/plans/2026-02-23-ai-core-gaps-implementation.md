# AI Core Gaps Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 7 identified gaps in the AI core: token-aware history, abstractive compression, subagent engine unification, memory relevance filtering, orphaned migration cleanup, JSON schema validator expansion, and cost tracker pricing table.

**Architecture:** Each gap is an independent task that can be implemented and tested in isolation. Dependencies: Task 2 (abstractive compression) should come after Task 1 (token-budget history) since both touch the assembler. Task 4 (memory filtering) requires a new migration. All others are independent.

**Tech Stack:** Rust, sqlx, pgvector, tokio, serde_json, regex, fastembed

---

### Task 1: Token-Budget History Truncation

**Files:**
- Modify: `crates/context_engine/src/assembler.rs:306-374`
- Test: `crates/context_engine/src/assembler.rs` (inline tests module)

**Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/context_engine/src/assembler.rs`:

```rust
#[tokio::test]
async fn test_token_budget_truncates_long_messages() {
    let engine = ContextEngine::new();
    // Create history with very long messages that would blow a small budget
    let mut history = Vec::new();
    for i in 0..10 {
        // Each message is ~250 tokens (1000 chars / 4 chars per token)
        let long_text = format!("Message {} {}", i, "x".repeat(1000));
        if i % 2 == 0 {
            history.push(Message::user(long_text));
        } else {
            history.push(Message::assistant(long_text));
        }
    }

    let request = ContextRequest {
        message_text: "test".to_string(),
        history,
        system_prompt: "System.".to_string(),
        strategy: ExecutionStrategy::DirectResponse,
        tool_definitions: vec![],
        context_window: 1000, // very small window — ~850 input budget
    };
    let result = engine.assemble(request).await;

    // Token count must stay within 85% of context_window
    assert!(
        result.token_count <= 850,
        "Token count {} should not exceed input budget 850",
        result.token_count
    );
    // Should still have at least the system message
    assert!(!result.messages.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p context_engine -E 'test(test_token_budget_truncates_long_messages)' --nocapture`
Expected: FAIL — current code does not enforce token budget on history

**Step 3: Implementation — already handled by existing compressor**

The existing `HistoryCompressor::compress()` already accepts a `budget_tokens` parameter and does token-aware splitting. The issue is that the history passed to `compress()` is the full session history (up to 50 messages from `get_history(50)`), and the compressor handles truncation within its budget.

However, the compressor's `compress()` always keeps at least `min_recent_messages` (4) verbatim — even if they exceed the budget. Fix this by adding a post-compression budget enforcement in `assemble_uncached()`.

In `crates/context_engine/src/assembler.rs`, replace lines 332-345:

```rust
// 4. Compress history to fit remaining budget
let history_budget = allocator.remaining();
let compressed = self.compressor.compress(&request.history, history_budget);
```

With:

```rust
// 4. Compress history to fit remaining budget (token-aware)
let history_budget = allocator.remaining();
let compressed = self.compressor.compress(&request.history, history_budget);

// Post-compression budget enforcement: if recent messages alone
// exceed the budget (e.g., very long tool results), truncate from oldest.
let mut recent_messages = compressed.recent_messages;
let mut recent_token_total: usize = recent_messages
    .iter()
    .map(|m| self.estimate_message_tokens(m))
    .sum();
while recent_token_total > history_budget && recent_messages.len() > 1 {
    let removed_tokens = self.estimate_message_tokens(&recent_messages[0]);
    recent_messages.remove(0);
    recent_token_total = recent_token_total.saturating_sub(removed_tokens);
}
```

Then update the references below from `compressed.recent_messages` to `recent_messages`:

```rust
// Track actual allocations
let recent_tokens: usize = recent_messages
    .iter()
    .map(|m| self.estimate_message_tokens(m))
    .sum();
allocator.allocate(Priority::RecentHistory, recent_tokens);

let summary_tokens: usize = compressed.summaries.iter().map(|s| s.token_count).sum();
// Only include summaries that fit in remaining budget
let remaining_after_recent = history_budget.saturating_sub(recent_tokens);
let summaries: Vec<_> = compressed.summaries.into_iter()
    .scan(0usize, |acc, s| {
        *acc += s.token_count;
        if *acc <= remaining_after_recent { Some(s) } else { None }
    })
    .collect();
let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();
allocator.allocate(Priority::CompressedHistory, summary_tokens);
```

And update the message assembly to use the new variables:

```rust
// Summaries as system-level context (if any)
for summary in &summaries {
    messages.push(Message::system(&summary.content));
}

// Recent messages verbatim
messages.extend(recent_messages);
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p context_engine -E 'test(test_token_budget_truncates_long_messages)' --nocapture`
Expected: PASS

**Step 5: Run all context_engine tests**

Run: `cargo nextest run -p context_engine --nocapture`
Expected: All tests pass

**Step 6: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "fix(context_engine): enforce token budget on history truncation"
```

---

### Task 2: Activate Abstractive Compression

**Files:**
- Modify: `crates/context_engine/src/assembler.rs:334`
- Test: `crates/context_engine/src/assembler.rs` (inline tests module)

**Step 1: Write the failing test**

Add to the tests module in `crates/context_engine/src/assembler.rs`:

```rust
#[tokio::test]
async fn test_abstractive_compression_used_when_provider_wired() {
    use crate::summary_provider::SummaryProvider;
    use crate::CompressorConfig;
    use crate::history_compressor::CompressorMode;

    struct TrackingProvider {
        called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl SummaryProvider for TrackingProvider {
        async fn summarize(&self, _messages: &[Message]) -> std::result::Result<String, String> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok("LLM summary".to_string())
        }
    }

    let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let provider = Arc::new(TrackingProvider { called: called.clone() });

    let config = CompressorConfig {
        mode: CompressorMode::Abstractive,
        min_recent_messages: 2,
        chunk_size: 3,
        ..Default::default()
    };

    let engine = ContextEngine::new()
        .with_compressor_config(config)
        .with_summary_provider(provider);

    // 20 messages to ensure some get compressed
    let mut history = Vec::new();
    for i in 0..20 {
        if i % 2 == 0 {
            history.push(Message::user(format!("User message {}", i)));
        } else {
            history.push(Message::assistant(format!("Response {}", i)));
        }
    }

    let request = ContextRequest {
        message_text: "test".to_string(),
        history,
        system_prompt: "System.".to_string(),
        strategy: ExecutionStrategy::DirectResponse,
        tool_definitions: vec![],
        context_window: 200, // small window to force compression
    };

    engine.assemble(request).await;

    assert!(
        called.load(std::sync::atomic::Ordering::SeqCst),
        "SummaryProvider should have been called via compress_async"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p context_engine -E 'test(test_abstractive_compression_used_when_provider_wired)' --nocapture`
Expected: FAIL — `SummaryProvider` never called because `compress()` (sync) is used

**Step 3: Make the change**

In `crates/context_engine/src/assembler.rs`, change the compression call (inside `assemble_uncached`). Replace:

```rust
let compressed = self.compressor.compress(&request.history, history_budget);
```

With:

```rust
let compressed = self.compressor.compress_async(&request.history, history_budget).await;
```

This is safe because `assemble_uncached` is already `async fn`.

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p context_engine -E 'test(test_abstractive_compression_used_when_provider_wired)' --nocapture`
Expected: PASS

**Step 5: Run all context_engine tests**

Run: `cargo nextest run -p context_engine --nocapture`
Expected: All tests pass

**Step 6: Commit**

```bash
git add crates/context_engine/src/assembler.rs
git commit -m "fix(context_engine): activate abstractive compression via compress_async"
```

---

### Task 3: Subagent Uses ReactPlusEngine

**Files:**
- Modify: `crates/agent/src/subagent.rs:262-375`
- Test: `crates/agent/src/subagent.rs` (inline tests module)

**Step 1: Write the failing test**

Add to the tests module in `crates/agent/src/subagent.rs`:

```rust
#[tokio::test]
async fn test_run_subagent_task_returns_text_response() {
    let provider: DynProvider = Arc::new(NoOpProvider);
    let config = SubagentConfig {
        brave_api_key: None,
        web_max_results: 5,
        exec_timeout: 60,
        restrict_to_workspace: false,
    };
    let result = run_subagent_task(
        &provider,
        std::path::Path::new("/tmp"),
        "no-op",
        "Say hello",
        config,
    )
    .await;
    assert!(result.is_ok());
    let (status, text) = result.unwrap();
    assert_eq!(status, "ok");
    assert_eq!(text, "ok"); // NoOpProvider returns "ok"
}
```

**Step 2: Run test to verify it passes (baseline)**

Run: `cargo nextest run -p agent -E 'test(test_run_subagent_task_returns_text_response)' --nocapture`
Expected: PASS (this confirms baseline behavior before refactor)

**Step 3: Refactor run_subagent_task to use ReactPlusEngine**

Replace the `run_subagent_task` function body (lines 262-375) in `crates/agent/src/subagent.rs` with:

```rust
async fn run_subagent_task(
    provider: &DynProvider,
    workspace: &std::path::Path,
    model: &str,
    task: &str,
    config: SubagentConfig,
) -> std::result::Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    use crate::execution::core::ExecutionCore;
    use crate::execution::react_plus::{ReactOutcome, ReactPlusEngine, ReflectionMode};
    use crate::execution::types::ExecutionParams;
    use providers::ChatParams;
    use tokio::sync::RwLock;

    // Build subagent tool registry with limited tools
    let mut tools = ToolRegistry::new();

    let allowed_dir = if config.restrict_to_workspace {
        Some(workspace.to_path_buf())
    } else {
        None
    };

    // Filesystem tools
    register_fs_tools(&mut tools, allowed_dir);

    // Shell tool
    tools.register(ExecTool::new(
        config.exec_timeout,
        Some(workspace.to_path_buf()),
        config.restrict_to_workspace,
    ));

    // Web tools
    tools.register(WebSearchTool::new(
        config.brave_api_key,
        config.web_max_results,
    ));
    tools.register(WebFetchTool::new());

    let tool_defs = tools.get_definitions();

    // Build execution engine
    let core = Arc::new(ExecutionCore::new(
        Arc::clone(provider),
        Arc::new(RwLock::new(tools)),
    ));
    let engine = ReactPlusEngine::new(core)
        .with_max_iterations(15)
        .with_reflection_mode(ReflectionMode::OnFailure);

    // Build system prompt and messages
    let system_prompt = build_subagent_prompt(workspace, task);
    let messages = vec![
        Message::system(system_prompt),
        Message::user(task.to_string()),
    ];

    let params = ExecutionParams {
        chat_params: ChatParams::new(model),
        tool_timeout_secs: config.exec_timeout,
    };
    let routing_ctx = RoutingContext::new("subagent".into(), "background".into());

    // Execute via ReactPlusEngine
    let outcome = engine
        .execute(Arc::new(messages), &tool_defs, &params, &routing_ctx, None)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    match outcome {
        ReactOutcome::Response { content, .. } => Ok(("ok".to_string(), content)),
        ReactOutcome::EscalateToAutonomous { reason, .. } => {
            Ok(("ok".to_string(), format!("Task requires more complex handling: {}", reason)))
        }
        ReactOutcome::MaxIterationsReached { partial_content, .. } => {
            let text = partial_content
                .unwrap_or_else(|| "Task completed but no final response was generated.".to_string());
            Ok(("ok".to_string(), text))
        }
    }
}
```

**Step 4: Run test to verify it still passes**

Run: `cargo nextest run -p agent -E 'test(test_run_subagent_task_returns_text_response)' --nocapture`
Expected: PASS

**Step 5: Run all agent tests**

Run: `cargo nextest run -p agent --nocapture`
Expected: All tests pass

**Step 6: Clippy check**

Run: `cargo clippy -p agent --all-targets --all-features`
Expected: 0 warnings

**Step 7: Commit**

```bash
git add crates/agent/src/subagent.rs
git commit -m "refactor(agent): replace subagent manual loop with ReactPlusEngine"
```

---

### Task 4: Embedding-Based Memory Relevance Filtering

**Files:**
- Create: `crates/storage/migrations/20260223000001_memory_note_embeddings.sql`
- Create: `crates/storage/src/repos/memory_note_embedding.rs`
- Modify: `crates/storage/src/repos/mod.rs` (register new repo)
- Modify: `crates/storage/src/lib.rs` (re-export)
- Modify: `crates/agent/src/memory.rs`
- Modify: `crates/agent/src/context_sources/memory.rs`
- Test: inline in each modified file

**Step 1: Create the migration**

Create `crates/storage/migrations/20260223000001_memory_note_embeddings.sql`:

```sql
-- Memory note embeddings for semantic relevance filtering
CREATE TABLE IF NOT EXISTS memory_note_embeddings (
    note_key   TEXT PRIMARY KEY REFERENCES memory_notes(note_key) ON DELETE CASCADE,
    embedding  vector(384) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- HNSW index for fast ANN search
CREATE INDEX IF NOT EXISTS idx_memory_note_embeddings_ann
    ON memory_note_embeddings USING hnsw (embedding vector_cosine_ops);
```

**Step 2: Create MemoryNoteEmbeddingRepo**

Create `crates/storage/src/repos/memory_note_embedding.rs`:

```rust
//! Repository for the `memory_note_embeddings` table.

use pgvector::Vector;
use sqlx::PgPool;

use crate::error::StorageError;

/// Row from `memory_note_embeddings`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MemoryNoteEmbeddingRow {
    pub note_key: String,
    pub embedding: Vector,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Search result with similarity score.
pub struct MemoryNoteMatch {
    pub note_key: String,
    pub content: String,
    pub similarity: f64,
}

/// Repository for memory note embeddings.
#[derive(Debug, Clone)]
pub struct MemoryNoteEmbeddingRepo {
    pool: PgPool,
}

impl MemoryNoteEmbeddingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert an embedding for a memory note.
    pub async fn upsert(
        &self,
        note_key: &str,
        embedding: &[f32],
    ) -> Result<(), StorageError> {
        let vec = Vector::from(embedding.to_vec());
        sqlx::query(
            r#"
            INSERT INTO memory_note_embeddings (note_key, embedding)
            VALUES ($1, $2)
            ON CONFLICT (note_key)
            DO UPDATE SET embedding = $2, updated_at = now()
            "#,
        )
        .bind(note_key)
        .bind(vec)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Find memory notes similar to a query embedding.
    /// Returns notes joined with memory_notes content, ordered by similarity (descending).
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: i64,
        threshold: f64,
    ) -> Result<Vec<MemoryNoteMatch>, StorageError> {
        let vec = Vector::from(query_embedding.to_vec());
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            r#"
            SELECT e.note_key, m.content,
                   (1.0 - (e.embedding <=> $1)) AS similarity
            FROM memory_note_embeddings e
            JOIN memory_notes m ON m.note_key = e.note_key
            WHERE (1.0 - (e.embedding <=> $1)) >= $3
            ORDER BY similarity DESC
            LIMIT $2
            "#,
        )
        .bind(vec)
        .bind(limit)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(note_key, content, similarity)| MemoryNoteMatch {
                note_key,
                content,
                similarity,
            })
            .collect())
    }

    /// Delete an embedding by note key.
    pub async fn delete(&self, note_key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM memory_note_embeddings WHERE note_key = $1")
            .bind(note_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
```

**Step 3: Register the new repo in storage**

In `crates/storage/src/repos/mod.rs`, add:

```rust
pub mod memory_note_embedding;
```

In `crates/storage/src/lib.rs`, add the re-export:

```rust
pub use repos::memory_note_embedding::{MemoryNoteEmbeddingRepo, MemoryNoteMatch};
```

**Step 4: Add relevance filtering to MemoryStore**

Modify `crates/agent/src/memory.rs` — add an `EmbeddingEngine` and `MemoryNoteEmbeddingRepo` field:

```rust
use storage::{MemoryNoteEmbeddingRepo, MemoryNoteMatch};

pub struct MemoryStore {
    repo: storage::MemoryNoteRepo,
    embedding_repo: Option<MemoryNoteEmbeddingRepo>,
    embedding_engine: Option<Arc<dyn crate::EmbeddingEnginePort>>,
    similarity_threshold: f64,
}
```

Add the trait to decouple from concrete engine (if not already present — check the codebase). Add a new method:

```rust
/// Get memory context filtered by relevance to the query.
/// Falls back to get_memory_context() if embeddings are unavailable.
pub async fn get_relevant_memory(&self, query: &str, limit: usize) -> String {
    // Try embedding-based retrieval first
    if let (Some(engine), Some(repo)) = (&self.embedding_engine, &self.embedding_repo) {
        if let Ok(query_vec) = engine.embed(query).await {
            if let Ok(matches) = repo.search_similar(&query_vec, limit as i64, self.similarity_threshold).await {
                if !matches.is_empty() {
                    let mut context = String::new();
                    context.push_str("# Relevant Memory\n\n");
                    for m in &matches {
                        context.push_str(&format!("## {} (relevance: {:.0}%)\n{}\n\n",
                            m.note_key, m.similarity * 100.0, m.content));
                    }
                    return context;
                }
            }
        }
    }

    // Fallback to dump-everything
    self.get_memory_context().await
}
```

**Step 5: Update MemorySource to pass query**

In `crates/agent/src/context_sources/memory.rs`, the `ContextSource` trait's `provide()` currently receives `SourceContext { channel, chat_id }`. This doesn't include the user's message.

Add a `message` field to `SourceContext` in `crates/context_engine/src/source.rs`:

```rust
pub struct SourceContext {
    pub channel: String,
    pub chat_id: String,
    pub message: Option<String>, // NEW: user message for relevance filtering
}
```

Update `MemorySource::provide()` to use it:

```rust
async fn provide(&self, ctx: &SourceContext) -> Option<String> {
    // ... cache check unchanged ...

    // Use relevance filtering if query available
    let content = if let Some(ref query) = ctx.message {
        self.memory.get_relevant_memory(query, 5).await
    } else {
        self.memory.get_memory_context().await
    };
    // ... rest unchanged ...
}
```

Update `build_system_prompt()` in `assembler.rs` and its callers to pass the message through `SourceContext`.

**Step 6: Embed memory notes on write**

In `MemoryStore::append_today()` and `write_long_term()`, after writing to SQL, fire-and-forget embed the content:

```rust
// Best-effort: embed the note for future relevance search
if let (Some(engine), Some(repo)) = (&self.embedding_engine, &self.embedding_repo) {
    let engine = Arc::clone(engine);
    let repo = repo.clone();
    let key = key.clone();
    let content = content.to_string();
    tokio::spawn(async move {
        if let Ok(vec) = engine.embed(&content).await {
            let _ = repo.upsert(&key, &vec).await;
        }
    });
}
```

**Step 7: Run tests**

Run: `cargo nextest run -p storage -p agent --nocapture`
Expected: All tests pass (new repo tests need database)

**Step 8: Commit**

```bash
git add crates/storage/migrations/20260223000001_memory_note_embeddings.sql \
    crates/storage/src/repos/memory_note_embedding.rs \
    crates/storage/src/repos/mod.rs \
    crates/storage/src/lib.rs \
    crates/agent/src/memory.rs \
    crates/agent/src/context_sources/memory.rs \
    crates/context_engine/src/source.rs
git commit -m "feat(memory): add embedding-based relevance filtering for memory notes"
```

---

### Task 5: Delete Orphaned history_summaries Migration

**Files:**
- Create: `crates/storage/migrations/20260223000002_drop_history_summaries.sql`

**Step 1: Create the drop migration**

Create `crates/storage/migrations/20260223000002_drop_history_summaries.sql`:

```sql
-- Drop orphaned history_summaries table (never wired to Rust code)
DROP TABLE IF EXISTS history_summaries;
```

**Step 2: Build to verify migration compiles**

Run: `cargo build -p storage`
Expected: Success (sqlx auto-runs migrations)

**Step 3: Commit**

```bash
git add crates/storage/migrations/20260223000002_drop_history_summaries.sql
git commit -m "chore(storage): drop orphaned history_summaries table"
```

---

### Task 6: Expand JSON Schema Validator

**Files:**
- Modify: `crates/tools-core/Cargo.toml` (add `regex` dependency)
- Modify: `crates/tools-core/src/lib.rs:125-245`
- Test: `crates/tools-core/src/lib.rs` (add inline tests)

**Step 1: Add regex dependency**

In `crates/tools-core/Cargo.toml`, add under `[dependencies]`:

```toml
regex.workspace = true
```

**Step 2: Write failing tests**

Add a `#[cfg(test)] mod tests` block (or extend existing) at the bottom of `crates/tools-core/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_one_of_matches_exactly_one() {
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        // String matches exactly one
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        // Integer matches exactly one
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        // Boolean matches none
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_any_of_matches_at_least_one() {
        let schema = json!({
            "anyOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_pattern() {
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+$"
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(!validate_value(&json!("Hello123"), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_additional_properties_false() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        // Only declared property
        assert!(validate_value(&json!({"name": "test"}), &schema, "").is_empty());
        // Extra property
        assert!(!validate_value(&json!({"name": "test", "extra": true}), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_min_max_items() {
        let schema = json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 2,
            "maxItems": 4
        });
        assert!(!validate_value(&json!([1]), &schema, "").is_empty()); // too few
        assert!(validate_value(&json!([1, 2]), &schema, "").is_empty()); // ok
        assert!(validate_value(&json!([1, 2, 3, 4]), &schema, "").is_empty()); // ok
        assert!(!validate_value(&json!([1, 2, 3, 4, 5]), &schema, "").is_empty()); // too many
    }

    #[test]
    fn test_validate_number_minimum_maximum() {
        let schema = json!({
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0
        });
        assert!(validate_value(&json!(0.5), &schema, "").is_empty());
        assert!(!validate_value(&json!(-0.1), &schema, "").is_empty());
        assert!(!validate_value(&json!(1.5), &schema, "").is_empty());
    }
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p tools-core --nocapture`
Expected: FAIL — `oneOf`, `anyOf`, `pattern`, `additionalProperties`, `minItems/maxItems`, number `minimum/maximum` not implemented

**Step 4: Implement the missing keywords**

In `crates/tools-core/src/lib.rs`, modify `validate_value()`. Add these checks:

After the `Some("number")` arm (line 176), add range checking:

```rust
Some("number") => {
    if !val.is_f64() && !val.is_i64() && !val.is_u64() {
        errors.push(format!("{} should be number", label));
        return errors;
    }
    let n = val.as_f64().unwrap_or(0.0);
    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        if n < min {
            errors.push(format!("{} must be >= {}", label, min));
        }
    }
    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        if n > max {
            errors.push(format!("{} must be <= {}", label, max));
        }
    }
}
```

After the `Some("array")` type check (line 183), add `minItems/maxItems`:

```rust
Some("array") => {
    if !val.is_array() {
        errors.push(format!("{} should be array", label));
        return errors;
    }
    if let Some(arr) = val.as_array() {
        if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
            if arr.len() < min as usize {
                errors.push(format!("{} must have at least {} items", label, min));
            }
        }
        if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
            if arr.len() > max as usize {
                errors.push(format!("{} must have at most {} items", label, max));
            }
        }
        if let Some(items_schema) = schema.get("items") {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                errors.extend(validate_value(item, items_schema, &item_path));
            }
        }
    }
}
```

In the `Some("object")` arm, add `additionalProperties` check after properties validation:

```rust
// Check additionalProperties
if let Some(false) = schema.get("additionalProperties").and_then(|v| v.as_bool()) {
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for key in obj.keys() {
            if !properties.contains_key(key) {
                let prop_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                errors.push(format!("unexpected property {}", prop_path));
            }
        }
    }
}
```

In the `Some("string")` arm, after `maxLength`, add `pattern`:

```rust
if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
    if let Ok(re) = regex::Regex::new(pattern) {
        if !re.is_match(s) {
            errors.push(format!("{} must match pattern {}", label, pattern));
        }
    }
}
```

After the `enum` check at the end (before `errors`), add `oneOf` and `anyOf`:

```rust
// oneOf: must match exactly one subschema
if let Some(schemas) = schema.get("oneOf").and_then(|s| s.as_array()) {
    let match_count = schemas.iter()
        .filter(|s| validate_value(val, s, path).is_empty())
        .count();
    if match_count != 1 {
        errors.push(format!("{} must match exactly one of oneOf schemas (matched {})", label, match_count));
    }
}

// anyOf: must match at least one subschema
if let Some(schemas) = schema.get("anyOf").and_then(|s| s.as_array()) {
    let matches_any = schemas.iter()
        .any(|s| validate_value(val, s, path).is_empty());
    if !matches_any {
        errors.push(format!("{} must match at least one of anyOf schemas", label));
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p tools-core --nocapture`
Expected: All tests pass

**Step 6: Clippy check**

Run: `cargo clippy -p tools-core --all-targets`
Expected: 0 warnings

**Step 7: Commit**

```bash
git add crates/tools-core/Cargo.toml crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add oneOf, anyOf, pattern, additionalProperties, minItems/maxItems validation"
```

---

### Task 7: Expanded Model Pricing Table

**Files:**
- Modify: `crates/agent/src/output/cost_tracker.rs:44-65`
- Test: `crates/agent/src/output/cost_tracker.rs` (inline tests)

**Step 1: Write failing tests**

Add to tests module in `crates/agent/src/output/cost_tracker.rs`:

```rust
#[test]
fn test_gpt4o_mini_has_own_pricing() {
    let (input, output) = model_pricing("gpt-4o-mini");
    // gpt-4o-mini should NOT match gpt-4o pricing
    assert!(input < 1.0, "gpt-4o-mini input should be < $1/MTok, got {}", input);
}

#[test]
fn test_cache_tokens_included_in_cost() {
    let usage = Usage {
        prompt_tokens: 100_000,
        completion_tokens: 10_000,
        total_tokens: 110_000,
        cache_read_tokens: 50_000,
        cache_write_tokens: 20_000,
    };
    let cost = estimate_cost(&usage, "claude-sonnet-4");
    // Cache tokens should contribute to cost
    // Without cache: input 0.1M * $3 + output 0.01M * $15 = $0.30 + $0.15 = $0.45
    // Cache read: 0.05M * some_rate, Cache write: 0.02M * some_rate
    assert!(cost > 0.45, "Cost should include cache tokens, got {}", cost);
}

#[test]
fn test_gemini_pricing_exists() {
    let (input, _) = model_pricing("gemini-2.0-flash");
    assert!(input > 0.0, "Gemini should have non-zero pricing");
}

#[test]
fn test_deepseek_pricing_exists() {
    let (input, _) = model_pricing("deepseek-chat");
    assert!(input > 0.0, "DeepSeek should have non-zero pricing");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(test_gpt4o_mini_has_own_pricing)' --nocapture`
Expected: FAIL — gpt-4o-mini matches gpt-4o substring

**Step 3: Replace model_pricing and estimate_cost**

Replace the `model_pricing` and `estimate_cost` functions:

```rust
/// Per-million-token pricing: (input, output, cache_read, cache_write).
struct ModelPricing {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

/// Get pricing for a model by exact ID match, then substring fallback.
fn model_pricing(model: &str) -> ModelPricing {
    let m = model.to_lowercase();

    // Exact match table (checked first)
    let exact = match m.as_str() {
        // Claude 4.x
        "claude-opus-4" | "claude-opus-4-20250514" =>
            ModelPricing { input: 15.0, output: 75.0, cache_read: 1.50, cache_write: 18.75 },
        "claude-sonnet-4" | "claude-sonnet-4-20250514" =>
            ModelPricing { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 },
        // Claude 3.5
        "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" =>
            ModelPricing { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 },
        "claude-3-5-haiku-20241022" | "claude-3-5-haiku-latest" =>
            ModelPricing { input: 0.80, output: 4.0, cache_read: 0.08, cache_write: 1.0 },
        // Claude 3
        "claude-3-haiku-20240307" =>
            ModelPricing { input: 0.25, output: 1.25, cache_read: 0.03, cache_write: 0.30 },
        // GPT-4o family
        "gpt-4o" | "gpt-4o-2024-11-20" | "gpt-4o-2024-08-06" =>
            ModelPricing { input: 2.50, output: 10.0, cache_read: 1.25, cache_write: 0.0 },
        "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" =>
            ModelPricing { input: 0.15, output: 0.60, cache_read: 0.075, cache_write: 0.0 },
        // Gemini
        "gemini-2.0-flash" | "gemini-2.0-flash-001" =>
            ModelPricing { input: 0.10, output: 0.40, cache_read: 0.025, cache_write: 0.0 },
        "gemini-1.5-pro" | "gemini-1.5-pro-002" =>
            ModelPricing { input: 1.25, output: 5.0, cache_read: 0.315, cache_write: 0.0 },
        "gemini-1.5-flash" | "gemini-1.5-flash-002" =>
            ModelPricing { input: 0.075, output: 0.30, cache_read: 0.01875, cache_write: 0.0 },
        // DeepSeek
        "deepseek-chat" | "deepseek-v3" =>
            ModelPricing { input: 0.27, output: 1.10, cache_read: 0.07, cache_write: 0.0 },
        "deepseek-reasoner" | "deepseek-r1" =>
            ModelPricing { input: 0.55, output: 2.19, cache_read: 0.14, cache_write: 0.0 },
        // Mistral
        "mistral-large-latest" =>
            ModelPricing { input: 2.0, output: 6.0, cache_read: 0.0, cache_write: 0.0 },
        "mistral-small-latest" =>
            ModelPricing { input: 0.10, output: 0.30, cache_read: 0.0, cache_write: 0.0 },
        _ => {
            // Substring fallback for unknown exact IDs
            return substring_fallback(&m);
        }
    };
    exact
}

/// Fallback pricing using substring matching (legacy behavior).
fn substring_fallback(model: &str) -> ModelPricing {
    if model.contains("opus") {
        ModelPricing { input: 15.0, output: 75.0, cache_read: 1.50, cache_write: 18.75 }
    } else if model.contains("sonnet") {
        ModelPricing { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 }
    } else if model.contains("haiku") {
        ModelPricing { input: 0.25, output: 1.25, cache_read: 0.03, cache_write: 0.30 }
    } else if model.contains("gpt-4o-mini") {
        ModelPricing { input: 0.15, output: 0.60, cache_read: 0.075, cache_write: 0.0 }
    } else if model.contains("gpt-4o") {
        ModelPricing { input: 2.50, output: 10.0, cache_read: 1.25, cache_write: 0.0 }
    } else if model.contains("gemini") {
        ModelPricing { input: 0.10, output: 0.40, cache_read: 0.025, cache_write: 0.0 }
    } else if model.contains("deepseek") {
        ModelPricing { input: 0.27, output: 1.10, cache_read: 0.07, cache_write: 0.0 }
    } else {
        ModelPricing { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 }
    }
}

fn estimate_cost(usage: &Usage, model: &str) -> f64 {
    let pricing = model_pricing(model);
    let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * pricing.input;
    let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * pricing.output;
    let cache_read_cost = (usage.cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read;
    let cache_write_cost = (usage.cache_write_tokens as f64 / 1_000_000.0) * pricing.cache_write;
    input_cost + output_cost + cache_read_cost + cache_write_cost
}
```

**Step 4: Update existing tests to match new behavior**

Update `test_model_pricing_gpt4o` since `gpt-4o-mini` no longer matches gpt-4o:

```rust
#[test]
fn test_model_pricing_gpt4o() {
    let pricing = model_pricing("gpt-4o");
    assert!((pricing.input - 2.50).abs() < 0.001);
    assert!((pricing.output - 10.0).abs() < 0.001);
}

#[test]
fn test_model_pricing_gpt4o_mini_separate() {
    let pricing = model_pricing("gpt-4o-mini");
    assert!((pricing.input - 0.15).abs() < 0.001);
    assert!((pricing.output - 0.60).abs() < 0.001);
}
```

Also update all existing tests that call `model_pricing()` to use the struct instead of tuple, and `estimate_cost` tests to account for cache tokens.

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(cost)' --nocapture`
Expected: All tests pass

**Step 6: Clippy check**

Run: `cargo clippy -p agent --all-targets`
Expected: 0 warnings

**Step 7: Commit**

```bash
git add crates/agent/src/output/cost_tracker.rs
git commit -m "feat(cost_tracker): expanded pricing table with cache tokens and exact model IDs"
```

---

### Final: Full Build Verification

**Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Success with 0 errors

**Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

**Step 4: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

**Step 5: Final commit (if any formatting fixes needed)**

```bash
git add -A
git commit -m "chore: fix formatting after AI core gaps implementation"
```
