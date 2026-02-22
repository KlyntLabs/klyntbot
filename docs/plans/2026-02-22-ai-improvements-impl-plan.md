# AI System Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all 16 AI weaknesses across 5 domains (memory, session, token counting, planning, agentic) to bring the system from 69/100 to production-grade.

**Architecture:** Phased approach — foundations first (no cross-cutting deps), then memory/session in parallel, then LLM-dependent features, finally planning/goals. Each task is TDD: write failing test, implement, verify, commit.

**Tech Stack:** Rust, sqlx/PostgreSQL, pgvector (HNSW), tiktoken-rs, dashmap, sha2, fastembed, tokio

---

## Phase 1: Foundations

### Task 1: Stable Context Cache Hashing (#16)

**Files:**
- Modify: `crates/context_engine/Cargo.toml`
- Modify: `crates/context_engine/src/assembler.rs:259-287`
- Test: inline `#[cfg(test)] mod tests` in assembler.rs

**Step 1: Add sha2 dependency**

In `crates/context_engine/Cargo.toml`, add under `[dependencies]`:
```toml
sha2 = "0.10"
```

**Step 2: Write the failing test**

In `crates/context_engine/src/assembler.rs`, add to the existing test module:
```rust
#[test]
fn test_cache_key_is_deterministic() {
    use context_engine::ExecutionStrategy;
    let req = ContextRequest {
        system_prompt: "You are helpful.".to_string(),
        history: vec![Message::user("hello")],
        message_text: "test".to_string(),
        strategy: ExecutionStrategy::ToolAssisted { max_iterations: 5 },
        tool_definitions: vec![],
        context_window: 4096,
    };
    let key1 = ContextEngine::compute_cache_key(&req);
    let key2 = ContextEngine::compute_cache_key(&req);
    assert_eq!(key1, key2, "Cache key must be deterministic");
    // SHA-256 produces a hex string, not a u64
    assert!(key1.len() == 64, "Expected SHA-256 hex string");
}
```

**Step 3: Run test to verify it fails**

Run: `cargo nextest run -p context_engine -E 'test(cache_key_is_deterministic)'`
Expected: FAIL — return type mismatch (u64 vs String) or method signature change

**Step 4: Implement SHA-256 cache key**

Replace `compute_cache_key` in `assembler.rs:259-287`:
```rust
use sha2::{Sha256, Digest};

fn compute_cache_key(request: &ContextRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.system_prompt.as_bytes());
    hasher.update(request.history.len().to_le_bytes());
    if let Some(last) = request.history.last() {
        hasher.update(format!("{:?}", last).as_bytes());
    }
    hasher.update(request.message_text.as_bytes());
    hasher.update(std::mem::discriminant(&request.strategy).to_string().as_bytes());
    hasher.update(request.tool_definitions.len().to_le_bytes());
    if let Some(first) = request.tool_definitions.first() {
        if let Some(name) = first.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) {
            hasher.update(name.as_bytes());
        }
    }
    hasher.update(request.context_window.to_le_bytes());
    format!("{:x}", hasher.finalize())
}
```

Update `ContextCache` to use `String` keys instead of `u64` (the cache HashMap key type).

**Step 5: Run test to verify it passes**

Run: `cargo nextest run -p context_engine -E 'test(cache_key)'`
Expected: PASS

**Step 6: Run full context_engine tests**

Run: `cargo nextest run -p context_engine`
Expected: All PASS

**Step 7: Commit**

```bash
git add crates/context_engine/
git commit -m "fix(context_engine): replace DefaultHasher with SHA-256 for cache keys"
```

---

### Task 2: Configurable History Limit (#7)

**Files:**
- Modify: `crates/config/src/schema/conversation.rs`
- Modify: `crates/agent/src/agent_loop/mod.rs:36`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Test: inline tests in conversation.rs + agent_loop

**Step 1: Write the failing config test**

In `crates/config/src/schema/conversation.rs`, add to test module:
```rust
#[test]
fn test_session_history_limit_default() {
    let json = serde_json::json!({});
    let config: ConversationConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.session.history_limit, 50);
}

#[test]
fn test_session_history_limit_custom() {
    let json = serde_json::json!({
        "session": { "historyLimit": 100 }
    });
    let config: ConversationConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.session.history_limit, 100);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p config -E 'test(session_history_limit)'`
Expected: FAIL — `session` field doesn't exist on `ConversationConfig`

**Step 3: Implement config field**

In `crates/config/src/schema/conversation.rs`, add:
```rust
/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    /// Maximum number of history messages to load (default: 50)
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

fn default_history_limit() -> usize {
    50
}
```

Add to `ConversationConfig`:
```rust
#[serde(default)]
pub session: SessionConfig,
```

**Step 4: Run config test to verify it passes**

Run: `cargo nextest run -p config -E 'test(session_history_limit)'`
Expected: PASS

**Step 5: Wire into AgentLoop**

In `crates/agent/src/agent_loop/mod.rs`:
- Remove `const DEFAULT_HISTORY_LIMIT: usize = 50;`
- Add field `history_limit: usize` to `AgentLoop` struct
- Replace all `DEFAULT_HISTORY_LIMIT` usages with `self.history_limit`

In `crates/agent/src/agent_loop/builder.rs`:
- Read `config.conversation.session.history_limit` in `build()`
- Pass to `AgentLoop` constructor

**Step 6: Run full test suite**

Run: `cargo nextest run -p agent -p config`
Expected: All PASS

**Step 7: Commit**

```bash
git add crates/config/ crates/agent/
git commit -m "feat(config): make session history limit configurable"
```

---

### Task 3: Typed Session Reset API (#8)

**Files:**
- Modify: `crates/session/src/manager.rs`
- Modify: `crates/agent/src/agent_loop/mod.rs` (remove magic string check)
- Test: inline tests in manager.rs

**Step 1: Write the failing test**

In `crates/session/src/manager.rs` test module:
```rust
#[tokio::test]
async fn test_reset_session_removes_from_cache() {
    let manager = create_test_manager().await;
    let key = "test:reset";
    manager.get_or_create(key).await.unwrap();
    assert!(manager.has_session(key));
    manager.reset_session(key).await.unwrap();
    assert!(!manager.has_session(key));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p session -E 'test(reset_session)'`
Expected: FAIL — `reset_session` method doesn't exist

**Step 3: Implement reset_session**

In `crates/session/src/manager.rs`:
```rust
/// Reset (delete) a session — removes from cache and database.
pub async fn reset_session(&mut self, key: &str) -> Result<()> {
    // Remove from in-memory cache
    self.sessions.remove(key);
    self.lru_order.retain(|k| k != key);

    // Delete from database (cascade deletes messages)
    if let Some(ref repo) = self.repo {
        repo.delete_session(key).await?;
    }

    debug!("Session reset: {}", key);
    Ok(())
}

/// Check if a session exists in the cache.
pub fn has_session(&self, key: &str) -> bool {
    self.sessions.contains_key(key)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p session -E 'test(reset_session)'`
Expected: PASS

**Step 5: Remove magic string from agent_loop**

In `crates/agent/src/agent_loop/mod.rs`, find the `__RESET_SESSION__` check and remove it. Instead, the Telegram channel should call `session_manager.write().await.reset_session(key).await` directly.

**Step 6: Commit**

```bash
git add crates/session/ crates/agent/
git commit -m "refactor(session): add typed reset_session API, remove magic string"
```

---

### Task 4: HNSW Index Migration (#4)

**Files:**
- Create: `crates/storage/migrations/20260222000000_hnsw_indexes.sql`

**Step 1: Write the migration**

```sql
-- Upgrade pgvector indexes from IVFFlat to HNSW.
-- HNSW handles continuous inserts without needing VACUUM ANALYZE.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_todo_embeddings_ann') THEN
        DROP INDEX idx_todo_embeddings_ann;
        CREATE INDEX idx_todo_embeddings_ann ON todo_embeddings
            USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
        RAISE NOTICE 'Upgraded todo_embeddings to HNSW index';
    END IF;

    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_conv_embeddings_ann') THEN
        DROP INDEX idx_conv_embeddings_ann;
        CREATE INDEX idx_conv_embeddings_ann ON conversation_embeddings
            USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
        RAISE NOTICE 'Upgraded conversation_embeddings to HNSW index';
    END IF;
END $$;
```

**Step 2: Verify migration applies**

Run: `cargo nextest run -p storage` (migrations auto-run on test pool connect)
Expected: PASS — no application code changes needed

**Step 3: Commit**

```bash
git add crates/storage/migrations/
git commit -m "perf(storage): upgrade pgvector indexes from IVFFlat to HNSW"
```

---

### Task 5: tiktoken-rs Token Counter (#9)

**Files:**
- Modify: `crates/context_engine/Cargo.toml`
- Modify: `crates/context_engine/src/token_counter.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Test: inline tests in token_counter.rs

**Step 1: Add tiktoken-rs dependency**

In `crates/context_engine/Cargo.toml`:
```toml
tiktoken-rs = "0.6"
```

**Step 2: Write the failing test**

In `crates/context_engine/src/token_counter.rs`:
```rust
#[test]
fn test_tiktoken_counter_english() {
    let counter = TiktokenCounter::new().expect("tiktoken init");
    // "Hello, world!" is 4 tokens in cl100k_base
    let count = counter.estimate_text("Hello, world!");
    assert!(count >= 3 && count <= 5, "Expected ~4 tokens, got {}", count);
}

#[test]
fn test_tiktoken_counter_cjk() {
    let counter = TiktokenCounter::new().expect("tiktoken init");
    // CJK characters are ~1 token each in cl100k_base
    let count = counter.estimate_text("你好世界");
    assert!(count >= 2 && count <= 6, "CJK should be ~4 tokens, got {}", count);
}

#[test]
fn test_tiktoken_counter_empty() {
    let counter = TiktokenCounter::new().expect("tiktoken init");
    assert_eq!(counter.estimate_text(""), 0);
}
```

**Step 3: Run test to verify it fails**

Run: `cargo nextest run -p context_engine -E 'test(tiktoken_counter)'`
Expected: FAIL — `TiktokenCounter` not found

**Step 4: Implement TiktokenCounter**

In `crates/context_engine/src/token_counter.rs`:
```rust
use tiktoken_rs::CoreBPE;

/// Accurate BPE token counter using tiktoken (cl100k_base).
pub struct TiktokenCounter {
    bpe: CoreBPE,
}

impl TiktokenCounter {
    pub fn new() -> Option<Self> {
        tiktoken_rs::cl100k_base().ok().map(|bpe| Self { bpe })
    }
}

impl TokenCounter for TiktokenCounter {
    fn estimate_text(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }
}

/// Construct the best available token counter (tiktoken with char fallback).
pub fn best_token_counter() -> Arc<dyn TokenCounter> {
    match TiktokenCounter::new() {
        Some(tc) => Arc::new(tc),
        None => {
            tracing::warn!("tiktoken init failed, falling back to char-based counter");
            Arc::new(CharTokenCounter)
        }
    }
}
```

**Step 5: Run test to verify it passes**

Run: `cargo nextest run -p context_engine -E 'test(tiktoken_counter)'`
Expected: PASS

**Step 6: Wire into AgentLoopBuilder**

In `crates/agent/src/agent_loop/builder.rs`, replace `default_token_counter()` with `best_token_counter()`:
```rust
use context_engine::token_counter::best_token_counter;
// ...
let token_counter = best_token_counter();
```

**Step 7: Run full suite**

Run: `cargo nextest run -p context_engine -p agent`
Expected: All PASS

**Step 8: Commit**

```bash
git add crates/context_engine/ crates/agent/
git commit -m "feat(context_engine): add tiktoken-rs BPE token counter"
```

---

### Task 6: Targeted Hybrid Search (#15)

**Files:**
- Modify: `crates/storage/src/repos/todo_repo.rs`
- Modify: `crates/tools/src/todo/actions/search.rs`
- Test: inline tests in todo_repo.rs

**Step 1: Write the failing test**

In `crates/storage/src/repos/todo_repo.rs` test module:
```rust
#[sqlx::test]
async fn test_get_by_ids_returns_matching(pool: PgPool) {
    let repo = TodoRepo::new(pool);
    // Insert test todos first...
    let results = repo.get_by_ids(&["id1".to_string(), "id2".to_string()]).await.unwrap();
    assert!(results.len() <= 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(get_by_ids)'`
Expected: FAIL — method doesn't exist

**Step 3: Implement get_by_ids**

In `crates/storage/src/repos/todo_repo.rs`:
```rust
/// Fetch todos by a list of IDs.
pub async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<TodoRow>, StorageError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, TodoRow>(
        "SELECT * FROM todos WHERE id = ANY($1)"
    )
    .bind(ids)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

**Step 4: Refactor hybrid search to use get_by_ids**

In `crates/tools/src/todo/actions/search.rs`, replace the `repo.list(&TodoFilter::default())` call in `handle_search_hybrid`:
```rust
// Collect IDs from both keyword and semantic results
let keyword_ids: Vec<String> = keyword_results.iter().map(|r| r.id.clone()).collect();
let semantic_ids: Vec<String> = semantic_results.iter().map(|r| r.id.clone()).collect();
let all_ids: Vec<String> = keyword_ids.iter().chain(semantic_ids.iter()).cloned().collect();

// Fetch only the needed todos
let todos = self.repo.get_by_ids(&all_ids).await?;
let todos_by_id: HashMap<String, _> = todos.into_iter().map(|t| (t.id.clone(), t.into())).collect();
```

**Step 5: Run test to verify**

Run: `cargo nextest run -p storage -E 'test(get_by_ids)' && cargo nextest run -p tools -E 'test(hybrid)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/storage/ crates/tools/
git commit -m "perf(tools): targeted ID fetch for hybrid search instead of loading all todos"
```

---

## Phase 2: Memory

### Task 7: Full Content Storage for Conversation Embeddings (#1)

**Files:**
- Create: `crates/storage/migrations/20260222000001_conv_embedding_full_content.sql`
- Modify: `crates/tools/src/conversation_embedding.rs` — `ConversationEmbeddingRecord` struct
- Modify: `crates/storage/src/repos/conv_embedding.rs` — SQL queries
- Modify: `crates/agent/src/conversation_embedding_handler.rs:82` — store full content
- Modify: `crates/agent/src/conversation_memory_retriever.rs:73` — return full content

**Step 1: Write the migration**

Create `crates/storage/migrations/20260222000001_conv_embedding_full_content.sql`:
```sql
-- Add full message content alongside the 100-char preview.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'conversation_embeddings') THEN
        ALTER TABLE conversation_embeddings ADD COLUMN IF NOT EXISTS content_full TEXT NOT NULL DEFAULT '';
        RAISE NOTICE 'Added content_full column to conversation_embeddings';
    END IF;
END $$;
```

**Step 2: Write failing test for the record struct**

In `crates/tools/src/conversation_embedding.rs` test module:
```rust
#[test]
fn test_record_has_content_full() {
    let record = ConversationEmbeddingRecord {
        id: "test".to_string(),
        session_key: "cli:default".to_string(),
        role: "user".to_string(),
        content_preview: "Hello...".to_string(),
        content_full: "Hello, how are you doing today?".to_string(),
        embedding: vec![0.0; 384],
        model: "test".to_string(),
        embedded_at: chrono::Utc::now(),
    };
    assert_eq!(record.content_full, "Hello, how are you doing today?");
}
```

**Step 3: Run test to verify it fails**

Run: `cargo nextest run -p tools -E 'test(record_has_content_full)'`
Expected: FAIL — `content_full` field doesn't exist

**Step 4: Add content_full field to record struct**

In `crates/tools/src/conversation_embedding.rs`, add to `ConversationEmbeddingRecord`:
```rust
pub content_full: String,
```

**Step 5: Update SQL queries in conv_embedding.rs**

In `crates/storage/src/repos/conv_embedding.rs`:
- `insert()`: add `content_full` to INSERT column list and bind parameter
- `search_similar()`: add `content_full` to SELECT column list
- `row_to_record()`: map `content_full` from row

**Step 6: Update embedding handler to store full content**

In `crates/agent/src/conversation_embedding_handler.rs:78-86`, change:
```rust
let record = ConversationEmbeddingRecord {
    id: message_id.to_string(),
    session_key: session_key.to_string(),
    role: role.to_string(),
    content_preview: content.chars().take(100).collect(),
    content_full: content.to_string(),  // NEW: full content
    embedding,
    model: self.engine.model_name().to_string(),
    embedded_at: Utc::now(),
};
```

**Step 7: Update memory retriever to return full content**

In `crates/agent/src/conversation_memory_retriever.rs:72-76`, change:
```rust
.map(|(record, score)| MemoryEntry {
    id: record.id,
    content: record.content_full,  // Was: record.content_preview
    score,
})
```

**Step 8: Run full test suite**

Run: `cargo nextest run -p tools -p storage -p agent`
Expected: All PASS

**Step 9: Commit**

```bash
git add crates/storage/migrations/ crates/tools/ crates/storage/src/ crates/agent/
git commit -m "feat(memory): store full message content in conversation embeddings"
```

---

### Task 8: Subagent Concurrency Limiter (#12)

**Files:**
- Modify: `crates/agent/src/subagent.rs`
- Modify: `crates/config/src/schema/agents.rs`
- Test: inline test in subagent.rs

**Step 1: Write the failing test**

In `crates/agent/src/subagent.rs` test module:
```rust
#[test]
fn test_subagent_manager_has_semaphore() {
    // SubagentManager should accept max_concurrent config
    let builder = SubagentManagerBuilder::new(mock_provider(), PathBuf::from("/tmp"));
    let manager = builder
        .max_concurrent_subagents(3)
        .build()
        .unwrap();
    // Verify semaphore exists (compilation check)
    assert!(manager.semaphore_permits() == 3);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(subagent_manager_has_semaphore)'`
Expected: FAIL — method doesn't exist

**Step 3: Implement semaphore in SubagentManager**

In `crates/agent/src/subagent.rs`:
```rust
use tokio::sync::Semaphore;

pub struct SubagentManager {
    // ... existing fields ...
    semaphore: Arc<Semaphore>,
}

impl SubagentManager {
    pub fn semaphore_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}
```

In `SubagentManagerBuilder`:
```rust
max_concurrent: usize,  // default: 3

pub fn max_concurrent_subagents(mut self, n: usize) -> Self {
    self.max_concurrent = n;
    self
}
```

In `build()`:
```rust
semaphore: Arc::new(Semaphore::new(self.max_concurrent)),
```

In `spawn()`, wrap `tokio::spawn` with semaphore acquire:
```rust
let permit = self.semaphore.clone().acquire_owned().await
    .map_err(|_| common::ToolError::ExecutionFailed("Subagent semaphore closed".into()))?;
tokio::spawn(async move {
    let _permit = permit; // held until task completes
    run_subagent_task(config, ...).await;
});
```

**Step 4: Add config field**

In `crates/config/src/schema/agents.rs`, add to `AgentsConfig` or `AgentDefaultsConfig`:
```rust
#[serde(default = "default_max_concurrent_subagents")]
pub max_concurrent_subagents: usize,

fn default_max_concurrent_subagents() -> usize { 3 }
```

**Step 5: Wire config in builder.rs**

```rust
.max_concurrent_subagents(config.agents.defaults.max_concurrent_subagents)
```

**Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(subagent)'`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/agent/ crates/config/
git commit -m "feat(agent): add semaphore-based subagent concurrency limit"
```

---

## Phase 3: Session Management

### Task 9: Session TTL/Expiry with Background Cleanup (#5)

**Files:**
- Create: `crates/agent/src/session_cleanup_service.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/config/src/schema/conversation.rs`
- Modify: `crates/storage/src/repos/session.rs`
- Test: inline tests

**Step 1: Add config fields**

In `crates/config/src/schema/conversation.rs`, extend `SessionConfig`:
```rust
pub struct SessionConfig {
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u32,
    #[serde(default = "default_cleanup_interval_hours")]
    pub cleanup_interval_hours: u32,
}

fn default_ttl_days() -> u32 { 30 }
fn default_cleanup_interval_hours() -> u32 { 1 }
```

**Step 2: Write failing test for cleanup SQL**

In `crates/storage/src/repos/session.rs` test module:
```rust
#[sqlx::test]
async fn test_delete_stale_sessions(pool: PgPool) {
    let repo = SessionRepo::new(pool);
    let count = repo.delete_stale_sessions(30).await.unwrap();
    assert!(count >= 0); // No sessions exist yet, should return 0
}
```

**Step 3: Implement delete_stale_sessions**

In `crates/storage/src/repos/session.rs`:
```rust
/// Delete sessions older than `ttl_days` days. Returns count deleted.
pub async fn delete_stale_sessions(&self, ttl_days: u32) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM sessions WHERE updated_at < now() - make_interval(days => $1::int)"
    )
    .bind(ttl_days as i32)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected())
}
```

**Step 4: Write the cleanup service**

Create `crates/agent/src/session_cleanup_service.rs`:
```rust
use std::time::Duration;
use storage::SessionRepo;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub struct SessionCleanupService {
    repo: SessionRepo,
    ttl_days: u32,
    interval: Duration,
    cancel: CancellationToken,
}

impl SessionCleanupService {
    pub fn new(repo: SessionRepo, ttl_days: u32, interval_hours: u32, cancel: CancellationToken) -> Self {
        Self {
            repo,
            ttl_days,
            interval: Duration::from_secs(interval_hours as u64 * 3600),
            cancel,
        }
    }

    pub async fn run(&self) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(self.interval) => {
                    match self.repo.delete_stale_sessions(self.ttl_days).await {
                        Ok(count) if count > 0 => info!("Session cleanup: deleted {} stale sessions", count),
                        Ok(_) => {},
                        Err(e) => warn!("Session cleanup failed: {}", e),
                    }
                }
                _ = self.cancel.cancelled() => break,
            }
        }
    }
}
```

**Step 5: Wire in builder.rs**

Spawn cleanup service in `AgentLoopBuilder::build()` alongside other background services.

**Step 6: Run tests**

Run: `cargo nextest run -p storage -E 'test(stale_sessions)' && cargo nextest run -p agent`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/agent/ crates/config/ crates/storage/
git commit -m "feat(session): add TTL-based session cleanup service"
```

---

### Task 10: Per-Session Locking with DashMap (#6)

**Files:**
- Modify: `crates/session/Cargo.toml`
- Modify: `crates/session/src/manager.rs` (major refactor)
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Add dashmap dependency**

In `crates/session/Cargo.toml`:
```toml
dashmap = "6"
```

**Step 2: Write failing test**

```rust
#[tokio::test]
async fn test_concurrent_session_access() {
    let manager = SessionManager::from_repo(repo).await;
    // Clone is now possible
    let m1 = manager.clone();
    let m2 = manager.clone();

    let (s1, s2) = tokio::join!(
        m1.get_or_create("session:1"),
        m2.get_or_create("session:2"),
    );
    // Both should succeed without deadlock
    assert!(s1.is_ok());
    assert!(s2.is_ok());
}
```

**Step 3: Refactor SessionManager**

Replace internal `HashMap + VecDeque` with `DashMap`:
```rust
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<Mutex<Session>>>>,
    lru_order: Arc<std::sync::Mutex<VecDeque<String>>>,
    repo: Option<SessionRepo>,
    max_cache_size: usize,
}
```

Key changes:
- `get_or_create()` returns `Arc<Mutex<Session>>` — caller locks per-session
- `save()` takes `&self` (not `&mut self`) — can be called without write lock
- `DashMap` allows concurrent reads/writes to different session keys
- LRU tracking uses a separate lightweight `std::sync::Mutex` (only touched for eviction)

**Step 4: Update AgentLoop**

In `crates/agent/src/agent_loop/mod.rs`:
- Replace `session_manager: Arc<RwLock<SessionManager>>` with `session_manager: SessionManager`
- `process_message()`: `let session_lock = self.session_manager.get_or_create(key).await?;`
- `let mut session = session_lock.lock().await;`
- `save_to_session()`: clone session, drop lock, then `self.session_manager.save(&session_clone).await`

**Step 5: Run full test suite**

Run: `cargo nextest run -p session -p agent`
Expected: All PASS

**Step 6: Commit**

```bash
git add crates/session/ crates/agent/
git commit -m "refactor(session): per-session locking via DashMap, remove global RwLock"
```

---

## Phase 4: LLM-Dependent Features

### Task 11: Abstractive Compression with Caching (#2)

**Files:**
- Create: `crates/storage/migrations/20260222000002_history_summaries.sql`
- Create: `crates/context_engine/src/summary_provider.rs`
- Modify: `crates/context_engine/src/history_compressor.rs`
- Create: `crates/agent/src/llm_summary_provider.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Create summary table migration**

```sql
CREATE TABLE IF NOT EXISTS history_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key TEXT NOT NULL,
    range_start INT NOT NULL,
    range_end INT NOT NULL,
    summary_text TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_key, range_start, range_end)
);
```

**Step 2: Define SummaryProvider trait**

Create `crates/context_engine/src/summary_provider.rs`:
```rust
use async_trait::async_trait;
use providers::Message;

#[async_trait]
pub trait SummaryProvider: Send + Sync {
    async fn summarize(&self, messages: &[Message]) -> Result<String, String>;
}
```

**Step 3: Write failing test for abstractive mode**

In `crates/context_engine/src/history_compressor.rs`:
```rust
#[tokio::test]
async fn test_abstractive_mode_calls_summary_provider() {
    let provider = Arc::new(MockSummaryProvider::new("LLM summary of conversation"));
    let config = CompressorConfig {
        mode: CompressorMode::Abstractive,
        ..Default::default()
    };
    let compressor = HistoryCompressor::from_config(default_token_counter(), config)
        .with_summary_provider(provider.clone());
    let history = make_history(20);
    let result = compressor.compress_async(&history, 1000).await;
    assert!(result.summaries.iter().any(|s| s.content.contains("LLM summary")));
}
```

**Step 4: Implement abstractive compression**

Modify `HistoryCompressor`:
- Add `summary_provider: Option<Arc<dyn SummaryProvider>>` field
- Add `with_summary_provider()` builder method
- New `compress_async()` method:
  - When `mode == Abstractive && summary_provider.is_some()`:
    - For each chunk of old messages, call `summary_provider.summarize(chunk)`
    - Cache result (future: store in DB via a separate cache trait)
  - Fallback to extractive on error

**Step 5: Implement LlmSummaryProvider in agent crate**

Create `crates/agent/src/llm_summary_provider.rs`:
```rust
pub struct LlmSummaryProvider {
    provider: DynProvider,
    model: String,
}

#[async_trait]
impl SummaryProvider for LlmSummaryProvider {
    async fn summarize(&self, messages: &[Message]) -> Result<String, String> {
        let prompt = format!(
            "Summarize this conversation segment in 2-3 sentences, preserving key facts and decisions:\n\n{}",
            messages.iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>().join("\n")
        );
        // Call LLM...
    }
}
```

**Step 6: Wire in builder.rs**

```rust
let summary_provider = Arc::new(LlmSummaryProvider::new(provider.clone(), model.clone()));
context_engine = context_engine.with_summary_provider(summary_provider);
```

**Step 7: Run tests, commit**

```bash
git add crates/storage/migrations/ crates/context_engine/ crates/agent/
git commit -m "feat(context_engine): add abstractive compression with LLM + caching"
```

---

### Task 12: Memory Decay and Consolidation (#3)

**Files:**
- Create: `crates/agent/src/memory_maintenance_service.rs`
- Modify: `crates/storage/src/repos/conv_embedding.rs` — add time-decay to search
- Modify: `crates/config/src/schema/conversation.rs` — add memory config
- Modify: `crates/agent/src/agent_loop/builder.rs` — wire service

**Step 1: Add memory config**

In `crates/config/src/schema/conversation.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    #[serde(default = "default_decay_half_life_days")]
    pub decay_half_life_days: u32,
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    #[serde(default)]
    pub consolidation_enabled: bool,
    #[serde(default = "default_maintenance_interval_hours")]
    pub maintenance_interval_hours: u32,
}

fn default_decay_half_life_days() -> u32 { 138 }
fn default_max_age_days() -> u32 { 90 }
fn default_maintenance_interval_hours() -> u32 { 24 }
```

Add `#[serde(default)] pub memory: MemoryConfig` to `ConversationConfig`.

**Step 2: Write failing test for time-decayed search**

```rust
#[test]
fn test_decay_factor_calculation() {
    let decay_factor = 0.995_f64;
    let days = 138.0;
    let weight = decay_factor.powf(days);
    assert!((weight - 0.5).abs() < 0.01, "Half-life should give ~0.5 weight");
}
```

**Step 3: Implement time-decay in search query**

In `crates/storage/src/repos/conv_embedding.rs`, modify `search_similar()`:
```sql
SELECT id, session_key, role, content_preview, content_full, embedding, created_at,
       (1.0 - (embedding <=> $1)) * power($4, EXTRACT(EPOCH FROM now() - created_at) / 86400.0) AS score
FROM conversation_embeddings
WHERE (embedding <=> $1) <= $2
ORDER BY score DESC
LIMIT $3
```

Add `decay_factor: f64` parameter (default: 0.995).

**Step 4: Create maintenance service**

Create `crates/agent/src/memory_maintenance_service.rs`:
- Prune embeddings older than `max_age_days`
- Consolidate daily notes older than 30 days (if `consolidation_enabled`)
- Run on configurable interval

**Step 5: Wire, test, commit**

```bash
git commit -m "feat(memory): add time-decay search and background maintenance service"
```

---

### Task 13: LLM-Backed Enrichment (#13)

**Files:**
- Modify: `crates/agent/src/enrichment/engine.rs`
- Modify: `crates/config/src/schema/todo.rs`
- Test: inline tests

**Step 1: Add config flag**

In `crates/config/src/schema/todo.rs`, add to `TodoEnrichmentConfig`:
```rust
#[serde(default)]
pub use_llm: bool,
```

**Step 2: Write failing test**

```rust
#[tokio::test]
async fn test_enrichment_with_llm_provider() {
    let engine = EnrichmentEngine::new(enrichment_config())
        .with_provider(mock_provider(), "test-model".into());
    let task = mock_task("URGENT: Fix production auth bug");
    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_some());
}
```

**Step 3: Implement LLM enrichment path**

In `crates/agent/src/enrichment/engine.rs`:
```rust
pub struct EnrichmentEngine {
    config: TodoEnrichmentConfig,
    provider: Option<DynProvider>,
    model: Option<String>,
}

impl EnrichmentEngine {
    pub fn with_provider(mut self, provider: DynProvider, model: String) -> Self {
        self.provider = Some(provider);
        self.model = Some(model);
        self
    }

    async fn enrich_with_llm(&self, task: &TodoItem) -> Option<EnrichmentResult> {
        let provider = self.provider.as_ref()?;
        let model = self.model.as_ref()?;
        // Call LLM with structured prompt, parse JSON response
        // Fall back to keyword on failure
    }
}
```

**Step 4: Wire, test, commit**

```bash
git commit -m "feat(enrichment): add LLM-backed task enrichment (opt-in)"
```

---

## Phase 5: Planning & Goals

### Task 14: Plan Step Auto-Generation (#11)

**Files:**
- Create: `crates/agent/src/plan_step_generator.rs`
- Modify: `crates/tools/src/plan_tool.rs` — wire into create action

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_generate_plan_steps_returns_valid_steps() {
    let steps = generate_plan_steps(
        &mock_provider_with_json_response(),
        "test-model",
        "Build a REST API with user authentication",
        &[],
        &["shell", "file_write"],
    ).await.unwrap();
    assert!(!steps.is_empty());
    assert!(steps.len() <= 8);
}
```

**Step 2: Implement shared step generator**

Create `crates/agent/src/plan_step_generator.rs`:
```rust
pub struct PlanStepDraft {
    pub description: String,
    pub reasoning: String,
    pub expected_tools: Vec<String>,
}

pub async fn generate_plan_steps(
    provider: &DynProvider,
    model: &str,
    description: &str,
    context: &[Message],
    available_tools: &[String],
) -> Result<Vec<PlanStepDraft>> {
    let prompt = format!(
        "Break this task into 3-8 concrete, actionable steps.\n\
         Task: {}\n\
         Available tools: {}\n\
         Respond with JSON array: [{{\"description\": ..., \"reasoning\": ..., \"expected_tools\": [...]}}]",
        description,
        available_tools.join(", ")
    );
    // Call LLM, parse JSON, validate
}
```

**Step 3: Wire into PlanTool::handle_create**

In `crates/tools/src/plan_tool.rs`, after creating the plan:
```rust
// Auto-generate steps
if let Ok(steps) = plan_handler.generate_steps(plan_id).await {
    // Steps saved to DB via PlanRepo
}
```

**Step 4: Test, commit**

```bash
git commit -m "feat(planning): auto-generate plan steps from description via LLM"
```

---

### Task 15: AutonomousTask Plan Generation Engine (#10)

**Files:**
- Create: `crates/agent/src/execution/plan_generate.rs`
- Modify: `crates/agent/src/execution/dispatch.rs` — replace AutonomousTask handler
- Modify: `crates/agent/src/execution/mod.rs` — export

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_plan_generate_engine_creates_plan() {
    let engine = PlanGenerateEngine::new(
        core.clone(),
        plan_repo.clone(),
        mock_provider(),
        "test-model".into(),
    );
    let result = engine.execute(messages, routing_ctx).await.unwrap();
    assert!(!result.content.is_empty());
}
```

**Step 2: Implement PlanGenerateEngine**

Create `crates/agent/src/execution/plan_generate.rs`:
```rust
pub struct PlanGenerateEngine {
    core: Arc<ExecutionCore>,
    plan_repo: PlanRepo,
    provider: DynProvider,
    model: String,
}

impl PlanGenerateEngine {
    pub async fn execute(
        &self,
        messages: Arc<Vec<Message>>,
        routing_ctx: RoutingContext,
    ) -> Result<DispatchResult> {
        // 1. Generate plan steps via LLM
        let steps = generate_plan_steps(&self.provider, &self.model, ...).await?;

        // 2. Create Plan + PlanStep records in DB
        let plan = create_plan_from_steps(&self.plan_repo, steps, &routing_ctx).await?;

        // 3. Execute plan via existing PlanExecutor
        let result = run_plan_execution(plan.id, &self.core, ...).await?;

        Ok(DispatchResult {
            content: result,
            final_strategy: ExecutionStrategy::AutonomousTask { max_iterations: 50 },
            escalation_count: 0,
            usage: Usage::default(),
        })
    }
}
```

**Step 3: Update EngineDispatch**

In `crates/agent/src/execution/dispatch.rs`, replace the AutonomousTask arm:
```rust
ExecutionStrategy::AutonomousTask { .. } => {
    let engine = PlanGenerateEngine::new(self.core.clone(), ...);
    return engine.execute(messages, routing_ctx).await;
}
```

**Step 4: Test, commit**

```bash
git commit -m "feat(execution): wire AutonomousTask to plan generation engine"
```

---

### Task 16: Goal Decomposition (#14)

**Files:**
- Modify: `crates/tools/src/goal_tool.rs` — add decompose and status actions

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_goal_decompose_creates_linked_plan() {
    let tool = GoalTool::new(goal_handler);
    let params = json!({ "action": "decompose", "id": goal_id });
    let result = tool.execute(params, routing_ctx).await.unwrap();
    assert!(result.contains("plan"));
}
```

**Step 2: Implement decompose action**

In `crates/tools/src/goal_tool.rs`:
```rust
"decompose" => {
    let goal = self.handler.get_goal(&id).await?;
    let plan_id = self.handler.decompose_goal(&goal).await?;
    Ok(format!("Created plan {} from goal '{}'", plan_id, goal.title))
}
```

`GoalHandler` trait gets:
```rust
async fn decompose_goal(&self, goal: &Goal) -> Result<String>;
```

Implementation reuses `generate_plan_steps()` from Task 14, creates a plan with `goal_id` FK.

**Step 3: Implement status action**

```rust
"status" => {
    let progress = self.handler.goal_progress(&id).await?;
    Ok(format!("Goal '{}': {} plans, {}% complete", ...))
}
```

**Step 4: Test, commit**

```bash
git commit -m "feat(goals): add decompose and status actions for goal-plan linkage"
```

---

## Final Phase: Verification

### Task 17: Full Integration Verification

**Step 1: Run full workspace tests**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

**Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```
Expected: 0 warnings

**Step 3: Check formatting**

```bash
cargo fmt --all --check
```
Expected: No formatting issues

**Step 4: Final commit and summary**

```bash
git log --oneline -20  # Review all commits
```
