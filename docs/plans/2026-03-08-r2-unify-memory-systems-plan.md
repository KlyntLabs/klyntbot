# R2: Unify Memory Systems — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the `cognitive` crate the single owner of all memory — conversation recall, semantic facts, episodic memories, and procedural rules — eliminating the parallel `MemoryStore` system.

**Architecture:** Add `TextEmbedder` trait + `ConversationRecallService` + `CognitiveMemoryRetriever` to cognitive crate. Rename `ConversationEmbeddingHandler` → `ConversationRecallHandler` in tools. Rewire `AgentLoopBuilder`. Delete `MemoryStore`, `LearningContextSource`, `MemorySource`, `ConversationMemoryRetriever`, `ConversationEmbeddingStore`, `MemoryNoteRepo`.

**Tech Stack:** Rust, async-trait, LanceDB (via `storage::VectorStore`), SQLite (via `storage::SqlitePool`), fastembed (via `tools::EmbeddingEngine`)

---

## Task 1: Add `TextEmbedder` trait to cognitive crate

**Files:**
- Modify: `crates/cognitive/src/embedder.rs` (currently lines 1-47)
- Modify: `crates/cognitive/src/lib.rs` (line 22: `pub use embedder::SemanticFactEmbedder`)

**Step 1: Add `TextEmbedder` trait to `crates/cognitive/src/embedder.rs`**

Add before the `SemanticFactEmbedder` trait (before line 18):

```rust
/// Generic text-to-vector embedding.
///
/// Implemented in `agent` crate wrapping `EmbeddingEngine`.
/// Used by `ConversationRecallService` and potentially `SemanticFactEmbedder` in the future.
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> common::Result<Vec<f32>>;
}
```

**Step 2: Export from `crates/cognitive/src/lib.rs`**

Change line 22 from:
```rust
pub use embedder::SemanticFactEmbedder;
```
to:
```rust
pub use embedder::{SemanticFactEmbedder, TextEmbedder};
```

**Step 3: Verify it compiles**

Run: `cargo build -p cognitive`
Expected: success, no warnings

**Step 4: Commit**

```bash
git add crates/cognitive/src/embedder.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add TextEmbedder trait for generic text-to-vector embedding"
```

---

## Task 2: Create `ConversationRecallService` in cognitive

**Files:**
- Create: `crates/cognitive/src/conversation_recall.rs`
- Modify: `crates/cognitive/src/lib.rs`

**Step 1: Write the tests first**

Create `crates/cognitive/src/conversation_recall.rs` with the full module including tests at the bottom. The tests use a mock `TextEmbedder`.

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use storage::VectorStore;
use tracing::warn;

use crate::embedder::TextEmbedder;

/// Configuration for conversation recall time-decay and search defaults.
#[derive(Debug, Clone)]
pub struct RecallConfig {
    pub decay_half_life_days: f64,
    pub default_threshold: f32,
    pub default_limit: usize,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            decay_half_life_days: 138.0, // ~0.995/day
            default_threshold: 0.4,
            default_limit: 5,
        }
    }
}

/// A single conversation recall result with time-decayed score.
#[derive(Debug, Clone)]
pub struct RecallResult {
    pub id: String,
    pub content: String,
    pub score: f64,
    pub raw_similarity: f64,
    pub created_at: DateTime<Utc>,
}

/// Metadata stored alongside each conversation embedding.
#[derive(Debug, Clone)]
pub struct RecallMetadata {
    pub session_key: String,
    pub channel: String,
    pub role: String,
    pub timestamp: DateTime<Utc>,
}

/// Owns all conversation recall operations: embed, search, prune.
///
/// Lives in the cognitive crate as the single owner of conversation memory.
/// Embedding is delegated to a `TextEmbedder` (implemented in `agent`).
pub struct ConversationRecallService {
    vector_store: VectorStore,
    embedder: std::sync::Arc<dyn TextEmbedder>,
    config: RecallConfig,
}

impl ConversationRecallService {
    pub fn new(
        vector_store: VectorStore,
        embedder: std::sync::Arc<dyn TextEmbedder>,
        config: RecallConfig,
    ) -> Self {
        Self {
            vector_store,
            embedder,
            config,
        }
    }

    pub fn config(&self) -> &RecallConfig {
        &self.config
    }

    /// Embed and store a conversation message for future recall.
    ///
    /// Composes text as "{role}: {content}" before embedding, matching the
    /// convention from the previous `ConversationEmbeddingHandlerImpl`.
    pub async fn store_message(
        &self,
        id: &str,
        content: &str,
        metadata: RecallMetadata,
    ) -> common::Result<()> {
        let text = format!("{}: {}", metadata.role, content);
        let vector = self.embedder.embed(&text).await?;

        let preview = if content.len() > 100 {
            &content[..100]
        } else {
            content
        };

        self.vector_store
            .upsert_embedding(
                "conv_embeddings",
                id,
                vector,
                &[
                    ("session_key", metadata.session_key.as_str()),
                    ("role", metadata.role.as_str()),
                    ("content_preview", preview),
                    ("full_content", content),
                ],
            )
            .await?;
        Ok(())
    }

    /// Search past conversations with time-decay scoring.
    ///
    /// `decay_factor = 0.5^(1/half_life_days)`, `score = similarity × decay_factor^days_old`.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> common::Result<Vec<RecallResult>> {
        let vector = self.embedder.embed(query).await?;

        let raw_results = self
            .vector_store
            .search_conv_embeddings(vector, limit * 2, threshold as f64)
            .await?;

        let decay_factor =
            0.5_f64.powf(1.0 / self.config.decay_half_life_days);
        let now = Utc::now();

        let mut results: Vec<RecallResult> = raw_results
            .into_iter()
            .filter_map(|(id, similarity, content, created_at)| {
                let days_old = (now - created_at).num_seconds() as f64 / 86400.0;
                let decayed_score = similarity * decay_factor.powf(days_old.max(0.0));

                if decayed_score >= threshold as f64 {
                    Some(RecallResult {
                        id,
                        content,
                        score: decayed_score,
                        raw_similarity: similarity,
                        created_at,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }

    /// Delete conversation embeddings older than the given cutoff.
    pub async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> common::Result<u64> {
        let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        self.vector_store
            .delete_where("conv_embeddings", &format!("created_at < '{cutoff_str}'"))
            .await
    }

    /// Count total stored conversation embeddings.
    pub async fn count(&self) -> common::Result<usize> {
        self.vector_store.count("conv_embeddings").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockEmbedder;

    #[async_trait]
    impl TextEmbedder for MockEmbedder {
        async fn embed(&self, _text: &str) -> common::Result<Vec<f32>> {
            // Return a deterministic 384-dim vector
            Ok(vec![0.1; 384])
        }
    }

    #[test]
    fn test_recall_config_defaults() {
        let config = RecallConfig::default();
        assert!((config.decay_half_life_days - 138.0).abs() < f64::EPSILON);
        assert!((config.default_threshold - 0.4).abs() < f32::EPSILON);
        assert_eq!(config.default_limit, 5);
    }

    #[test]
    fn test_decay_math() {
        let half_life = 138.0;
        let decay_factor = 0.5_f64.powf(1.0 / half_life);
        // At exactly half_life days, score should be halved
        let score_at_half_life = decay_factor.powf(half_life);
        assert!((score_at_half_life - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConversationRecallService>();
    }
}
```

**Step 2: Register the module**

Add to `crates/cognitive/src/lib.rs` after the existing module declarations:

```rust
pub mod conversation_recall;
```

And add to the exports:

```rust
pub use conversation_recall::{ConversationRecallService, RecallConfig, RecallMetadata, RecallResult};
```

**Step 3: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(conversation_recall)'`
Expected: 3 tests pass

**Step 4: Commit**

```bash
git add crates/cognitive/src/conversation_recall.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add ConversationRecallService for unified conversation memory"
```

---

## Task 3: Create `CognitiveMemoryRetriever` in cognitive

**Files:**
- Create: `crates/cognitive/src/memory_retriever.rs`
- Modify: `crates/cognitive/src/lib.rs`

**Step 1: Write the module with tests**

Create `crates/cognitive/src/memory_retriever.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever};

use crate::conversation_recall::ConversationRecallService;

/// Implements `MemoryRetriever` by delegating to `ConversationRecallService`.
///
/// Plugs into `ContextEngine::with_memory_retriever()` to inject conversation
/// recall into the message list during context assembly.
pub struct CognitiveMemoryRetriever {
    recall: Arc<ConversationRecallService>,
}

impl CognitiveMemoryRetriever {
    pub fn new(recall: Arc<ConversationRecallService>) -> Self {
        Self { recall }
    }
}

#[async_trait]
impl MemoryRetriever for CognitiveMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        match self
            .recall
            .search(query, limit, self.recall.config().default_threshold)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|r| MemoryEntry {
                    id: r.id,
                    content: r.content,
                    score: r.score,
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Conversation recall search failed: {e}");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriever_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CognitiveMemoryRetriever>();
    }
}
```

**Step 2: Register module and export**

Add to `crates/cognitive/src/lib.rs`:

```rust
pub mod memory_retriever;
```

And export:

```rust
pub use memory_retriever::CognitiveMemoryRetriever;
```

**Step 3: Verify it compiles and tests pass**

Run: `cargo nextest run -p cognitive -E 'test(memory_retriever)'`
Expected: 1 test passes

**Step 4: Commit**

```bash
git add crates/cognitive/src/memory_retriever.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add CognitiveMemoryRetriever implementing MemoryRetriever trait"
```

---

## Task 4: Refactor `conversation_embedding.rs` in tools → `conversation_recall.rs`

This renames the trait, removes `ConversationEmbeddingStore` (logic moved to cognitive), and keeps only the trait + types needed by `MemoryTool`.

**Files:**
- Rename: `crates/tools/src/conversation_embedding.rs` → `crates/tools/src/conversation_recall.rs`
- Modify: `crates/tools/src/lib.rs` (update module name + re-exports)
- Modify: `crates/tools/src/memory_tool.rs` (update imports + trait name)

**Step 1: Create `crates/tools/src/conversation_recall.rs`**

This file keeps only the trait, types, and tests. `ConversationEmbeddingStore` is removed (its logic lives in `cognitive::ConversationRecallService` now).

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Metadata stored alongside each conversation embedding.
#[derive(Debug, Clone)]
pub struct RecallMetadata {
    pub session_key: String,
    pub channel: String,
    pub role: String,
    pub timestamp: DateTime<Utc>,
}

/// A single conversation recall search result.
#[derive(Debug, Clone)]
pub struct RecallSearchResult {
    pub id: String,
    pub session_key: String,
    pub role: String,
    pub content_preview: String,
    pub content_full: String,
    pub score: f64,
    pub created_at: DateTime<Utc>,
}

/// Filter for purging conversation embeddings.
#[derive(Debug, Clone)]
pub enum PurgeFilter {
    BySessionKey(String),
    Before(DateTime<Utc>),
    All,
}

/// Status of the conversation recall system.
#[derive(Debug, Clone)]
pub struct ConversationRecallStatus {
    pub total_embeddings: usize,
    pub is_available: bool,
}

/// Interface for conversation recall operations.
///
/// Defined in `tools` (L4) for use by `MemoryTool`.
/// Implemented in `agent` (L5) delegating to `cognitive::ConversationRecallService`.
#[async_trait]
pub trait ConversationRecallHandler: Send + Sync {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> common::Result<()>;

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> common::Result<Vec<RecallSearchResult>>;

    async fn purge(&self, filter: PurgeFilter) -> common::Result<usize>;

    async fn status(&self) -> common::Result<ConversationRecallStatus>;

    fn is_available(&self) -> bool;
}
```

**Step 2: Delete old file**

```bash
rm crates/tools/src/conversation_embedding.rs
```

**Step 3: Update `crates/tools/src/lib.rs`**

Replace the old module declaration and re-exports. Change `conversation_embedding` → `conversation_recall`, `ConversationEmbeddingHandler` → `ConversationRecallHandler`, `ConversationEmbeddingStore` → removed, `ConversationEmbeddingStatus` → `ConversationRecallStatus`, `ConversationEmbeddingRecord` → removed (replaced by `RecallSearchResult`).

Find all `conversation_embedding` references and update to `conversation_recall`. Find all `ConversationEmbeddingHandler` references and update to `ConversationRecallHandler`.

**Step 4: Update `crates/tools/src/memory_tool.rs`**

Replace imports: `ConversationEmbeddingHandler` → `ConversationRecallHandler`, `ConversationEmbeddingRecord` → `RecallSearchResult`, `ConversationEmbeddingStatus` → `ConversationRecallStatus`, `PurgeFilter` stays the same name.

Update the struct field (line 23):
```rust
// was: conversation_handler: Option<Arc<dyn ConversationEmbeddingHandler>>
conversation_handler: Option<Arc<dyn ConversationRecallHandler>>,
```

Update all method signatures that reference the old types. The `search_conversations` and `search_all` methods need to use `RecallSearchResult` instead of `(ConversationEmbeddingRecord, f64)`.

Update the builder method:
```rust
// was: with_conversation_handler(handler: Arc<dyn ConversationEmbeddingHandler>)
pub fn with_conversation_handler(mut self, handler: Arc<dyn ConversationRecallHandler>) -> Self
```

Update mock in tests (lines 526-588): rename `MockConversationHandler` to implement `ConversationRecallHandler` instead of `ConversationEmbeddingHandler`.

**Step 5: Verify it compiles**

Run: `cargo build -p tools`
Expected: success

**Step 6: Run tools tests**

Run: `cargo nextest run -p tools -E 'test(memory)'`
Expected: all pass

**Step 7: Commit**

```bash
git add -A crates/tools/src/
git commit -m "refactor(tools): rename ConversationEmbeddingHandler to ConversationRecallHandler"
```

---

## Task 5: Add `TextEmbedderImpl` in agent

**Files:**
- Modify: `crates/agent/src/cognitive_embedder.rs` (add `TextEmbedderImpl` alongside existing `SemanticFactEmbedderImpl`)
- Modify: `crates/agent/src/lib.rs` (export)

**Step 1: Add `TextEmbedderImpl` to `crates/agent/src/cognitive_embedder.rs`**

Add after the existing `SemanticFactEmbedderImpl` (after line 99):

```rust
/// Generic text-to-vector embedder wrapping `EmbeddingEngine`.
///
/// Used by `ConversationRecallService` in the cognitive crate.
pub struct TextEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
}

impl TextEmbedderImpl {
    pub fn new(engine: Arc<EmbeddingEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl cognitive::TextEmbedder for TextEmbedderImpl {
    async fn embed(&self, text: &str) -> common::Result<Vec<f32>> {
        self.engine.embed_async(text).await
    }
}
```

**Step 2: Export from `crates/agent/src/lib.rs`**

Add:
```rust
pub use cognitive_embedder::TextEmbedderImpl;
```

**Step 3: Verify it compiles**

Run: `cargo build -p agent`
Expected: success

**Step 4: Commit**

```bash
git add crates/agent/src/cognitive_embedder.rs crates/agent/src/lib.rs
git commit -m "feat(agent): add TextEmbedderImpl wrapping EmbeddingEngine for cognitive crate"
```

---

## Task 6: Create `ConversationRecallHandlerImpl` in agent

Replaces the old `ConversationEmbeddingHandlerImpl`. Delegates to `ConversationRecallService`.

**Files:**
- Create: `crates/agent/src/conversation_recall_handler.rs` (replaces `conversation_embedding_handler.rs`)
- Delete: `crates/agent/src/conversation_embedding_handler.rs`
- Modify: `crates/agent/src/lib.rs`

**Step 1: Create `crates/agent/src/conversation_recall_handler.rs`**

```rust
use std::sync::Arc;

use async_trait::async_trait;
use cognitive::conversation_recall::{ConversationRecallService, RecallMetadata};
use tools::conversation_recall::{
    ConversationRecallHandler, ConversationRecallStatus, PurgeFilter, RecallSearchResult,
};

/// Implements `ConversationRecallHandler` (from tools L4) by delegating
/// to `ConversationRecallService` (from cognitive L5).
pub struct ConversationRecallHandlerImpl {
    service: Arc<ConversationRecallService>,
}

impl ConversationRecallHandlerImpl {
    pub fn new(service: Arc<ConversationRecallService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ConversationRecallHandler for ConversationRecallHandlerImpl {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> common::Result<()> {
        let metadata = RecallMetadata {
            session_key: session_key.to_string(),
            channel: session_key
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string(),
            role: role.to_string(),
            timestamp: chrono::Utc::now(),
        };
        self.service
            .store_message(message_id, content, metadata)
            .await
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> common::Result<Vec<RecallSearchResult>> {
        let results = self
            .service
            .search(query, limit, threshold as f32)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| RecallSearchResult {
                id: r.id,
                session_key: String::new(), // Not tracked in recall results
                role: String::new(),
                content_preview: if r.content.len() > 100 {
                    r.content[..100].to_string()
                } else {
                    r.content.clone()
                },
                content_full: r.content,
                score: r.score,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn purge(&self, filter: PurgeFilter) -> common::Result<usize> {
        match filter {
            PurgeFilter::Before(cutoff) => {
                self.service.delete_older_than(cutoff).await.map(|n| n as usize)
            }
            PurgeFilter::All => {
                // Delete everything by using a far-future cutoff
                let far_future = chrono::Utc::now() + chrono::Duration::days(36500);
                self.service.delete_older_than(far_future).await.map(|n| n as usize)
            }
            PurgeFilter::BySessionKey(key) => {
                self.service
                    .vector_store()
                    .delete_where("conv_embeddings", &format!("session_key = '{key}'"))
                    .await
                    .map(|n| n as usize)
            }
        }
    }

    async fn status(&self) -> common::Result<ConversationRecallStatus> {
        let count = self.service.count().await.unwrap_or(0);
        Ok(ConversationRecallStatus {
            total_embeddings: count,
            is_available: true,
        })
    }

    fn is_available(&self) -> bool {
        true
    }
}
```

Note: `ConversationRecallService` needs a `pub fn vector_store(&self) -> &VectorStore` getter added for the `BySessionKey` purge. Add this to `crates/cognitive/src/conversation_recall.rs`:

```rust
pub fn vector_store(&self) -> &VectorStore {
    &self.vector_store
}
```

**Step 2: Delete old handler**

```bash
rm crates/agent/src/conversation_embedding_handler.rs
```

**Step 3: Update `crates/agent/src/lib.rs`**

Replace:
```rust
pub mod conversation_embedding_handler;
pub use conversation_embedding_handler::ConversationEmbeddingHandlerImpl;
```
With:
```rust
pub mod conversation_recall_handler;
pub use conversation_recall_handler::ConversationRecallHandlerImpl;
```

**Step 4: Verify it compiles**

Run: `cargo build -p agent`
Expected: will fail until builder is rewired (Task 8), but the module itself should parse

**Step 5: Commit**

```bash
git add -A crates/agent/src/ crates/cognitive/src/conversation_recall.rs
git commit -m "feat(agent): add ConversationRecallHandlerImpl delegating to cognitive service"
```

---

## Task 7: Absorb confidence threshold into `CognitiveContextSource`

**Files:**
- Modify: `crates/cognitive/src/context_source.rs` (lines 57-63: struct fields, lines 156-257: provide method)

**Step 1: Add confidence threshold field**

Add to `CognitiveContextSource` struct (line 57-63):
```rust
confidence_bits: Option<Arc<std::sync::atomic::AtomicU32>>,
```

Add builder method:
```rust
pub fn with_confidence_threshold(mut self, bits: Arc<std::sync::atomic::AtomicU32>) -> Self {
    self.confidence_bits = Some(bits);
    self
}
```

**Step 2: Append confidence instructions in `provide()`**

At the end of the `provide()` method (before returning), if `confidence_bits` is set, append a section like `LearningContextSource` currently does (see `crates/agent/src/context_sources/learning.rs` lines ~150-180 for the exact format). Extract the threshold from `AtomicU32` as `f32::from_bits(bits.load(Ordering::Relaxed))` and format as:

```
## Confidence Calibration
Current confidence threshold: {threshold:.2}. When uncertain about user intent, ask for clarification rather than guessing.
```

**Step 3: Verify it compiles**

Run: `cargo build -p cognitive`

**Step 4: Commit**

```bash
git add crates/cognitive/src/context_source.rs
git commit -m "feat(cognitive): absorb confidence threshold instructions into CognitiveContextSource"
```

---

## Task 8: Rewire `AgentLoopBuilder`

This is the critical integration task. Replace all old memory wiring with the new cognitive-based pipeline.

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Remove old memory wiring**

In `builder.rs`, remove these sections:

1. **MemoryStore creation** (lines 168-184): Delete entirely
2. **LearningContextSource wiring** (lines 186-200): Delete entirely
3. **ConversationMemoryRetriever wiring** (lines 439-459): Delete entirely
4. **Old ConversationEmbeddingHandler wiring** (lines 565-597): Replace with new

**Step 2: Add new wiring**

After the `embedding_engine` creation (line 169) and within the cognitive section (lines 226-314), add:

```rust
// Create TextEmbedder for conversation recall
let text_embedder: Option<Arc<dyn cognitive::TextEmbedder>> =
    if config.conversation.embedding.enabled {
        Some(Arc::new(crate::cognitive_embedder::TextEmbedderImpl::new(
            Arc::clone(&embedding_engine),
        )))
    } else {
        None
    };

// Create ConversationRecallService
let recall_service: Option<Arc<cognitive::ConversationRecallService>> =
    if let (Some(embedder), Some(ref vs)) = (text_embedder, &self.vector_store) {
        Some(Arc::new(cognitive::ConversationRecallService::new(
            vs.clone(),
            embedder,
            cognitive::RecallConfig {
                decay_half_life_days: config.conversation.memory.decay_half_life_days as f64,
                default_threshold: config.conversation.search.semantic_threshold as f32,
                default_limit: 5,
            },
        )))
    } else {
        None
    };
```

Wire `CognitiveMemoryRetriever` into `ContextEngine` (replacing the old ConversationMemoryRetriever block):

```rust
let context_engine = if let Some(ref recall) = recall_service {
    let retriever = Arc::new(
        cognitive::CognitiveMemoryRetriever::new(Arc::clone(recall))
    );
    context_engine.with_memory_retriever(retriever)
} else {
    context_engine
};
```

Wire confidence threshold into `CognitiveContextSource` (in the cognitive section where the source is created, ~line 267-271):

```rust
let cognitive_source = cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
    .with_embedder_opt(cognitive_embedder.clone())
    .with_config(retrieval_config)
    .with_confidence_threshold(Arc::clone(&confidence_bits));
sources.push(Box::new(cognitive_source));
```

Wire `ConversationRecallHandlerImpl` for `MemoryTool` (replacing old block):

```rust
let conversation_recall_handler: Option<Arc<dyn tools::ConversationRecallHandler>> =
    recall_service.as_ref().map(|service| {
        Arc::new(crate::conversation_recall_handler::ConversationRecallHandlerImpl::new(
            Arc::clone(service),
        )) as Arc<dyn tools::ConversationRecallHandler>
    });

if config.conversation.search.enabled {
    if let Some(ref handler) = conversation_recall_handler {
        let mut memory_tool = tools::MemoryTool::new()
            .with_conversation_handler(Arc::clone(handler))
            .with_todo_repo(repos.actions.clone())
            .with_threshold(config.conversation.search.semantic_threshold)
            .with_rrf_k(config.todo.search.rrf_k);

        if let Some(ref h) = todo_embedding_handler {
            memory_tool = memory_tool.with_todo_embedding_handler(Arc::clone(h));
        }

        tool_registry.register(memory_tool);
    }
}
```

Update `spawn_embed_message` in `AgentLoop` to use the new handler type. Find where `ConversationEmbeddingHandler` is referenced in the agent loop's message processing and update to `ConversationRecallHandler`.

**Step 3: Remove old imports**

Remove all imports of: `MemoryStore`, `LearningContextSource`, `ConversationMemoryRetriever`, `ConversationEmbeddingStore`, `ConversationEmbeddingHandler`, `ConversationEmbeddingHandlerImpl`.

**Step 4: Verify it compiles**

Run: `cargo build -p agent`
Expected: may still fail if old files exist — that's okay, Task 9 handles deletion.

**Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): rewire AgentLoopBuilder to use cognitive-based memory pipeline"
```

---

## Task 9: Delete old files and clean up exports

**Files:**
- Delete: `crates/agent/src/memory.rs`
- Delete: `crates/agent/src/context_sources/learning.rs`
- Delete: `crates/agent/src/context_sources/memory.rs`
- Delete: `crates/agent/src/conversation_memory_retriever.rs`
- Modify: `crates/agent/src/context_sources/mod.rs` (remove `memory` and `learning` modules)
- Modify: `crates/agent/src/lib.rs` (remove old re-exports)
- Modify: `crates/storage/src/repos/mod.rs` (remove `memory_note` module)
- Modify: `crates/storage/src/repos/memory_note.rs` — delete file
- Modify: `crates/storage/src/lib.rs` (remove `MemoryNoteRepo` re-export)

**Step 1: Delete files**

```bash
rm crates/agent/src/memory.rs
rm crates/agent/src/context_sources/learning.rs
rm crates/agent/src/context_sources/memory.rs
rm crates/agent/src/conversation_memory_retriever.rs
rm crates/storage/src/repos/memory_note.rs
```

**Step 2: Update `crates/agent/src/context_sources/mod.rs`**

Remove:
```rust
pub mod memory;
pub mod learning;
pub use memory::MemorySource;
pub use learning::LearningContextSource;
```

**Step 3: Update `crates/agent/src/lib.rs`**

Remove:
```rust
pub mod memory;
pub use memory::MemoryStore;
pub use conversation_memory_retriever::ConversationMemoryRetriever;
```

**Step 4: Update `crates/storage/src/repos/mod.rs`**

Remove `pub mod memory_note;` and any re-export of `MemoryNoteRepo`.

**Step 5: Update `crates/storage/src/lib.rs`**

Remove any `pub use repos::memory_note::MemoryNoteRepo` or similar re-export.

**Step 6: Search for any remaining references**

Run: `grep -r "MemoryStore\|MemoryNoteRepo\|LearningContextSource\|MemorySource\|ConversationEmbeddingStore\|ConversationEmbeddingHandler\|ConversationMemoryRetriever" crates/ --include="*.rs" -l`

Fix any remaining references. Common places:
- Integration tests in `tests/`
- `crates/app-core/` (if it references memory types)
- `crates/klyntbot/src/lib.rs` (re-export facade)

**Step 7: Verify it compiles**

Run: `cargo build --workspace`
Expected: success

**Step 8: Commit**

```bash
git add -A
git commit -m "refactor: remove old MemoryStore, LearningContextSource, and parallel memory system"
```

---

## Task 10: Remove `memory_notes` table migration

**Files:**
- Modify or create: migration SQL to drop `memory_notes` table
- Modify: `crates/storage/src/vector_store.rs` (remove `memory_note_embeddings` table references)

**Step 1: Add migration to drop `memory_notes`**

Check how migrations work in this codebase (likely `crates/storage/src/migrations/`). Add a new migration that drops the table:

```sql
DROP TABLE IF EXISTS memory_notes;
```

**Step 2: Remove `memory_note_embeddings` references from VectorStore**

In `crates/storage/src/vector_store.rs`, find and remove any methods that reference `memory_note_embeddings` table. This includes any `upsert_embedding`, `search_similar`, `delete` calls that use `"memory_note_embeddings"` as the table name.

**Step 3: Verify**

Run: `cargo nextest run --workspace`
Expected: all pass

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove memory_notes table and memory_note_embeddings vector table"
```

---

## Task 11: Update integration tests

**Files:**
- Modify: `tests/integration/memory.rs` (or wherever integration tests reference old types)
- Any other test files found in Task 9 Step 6

**Step 1: Find all affected test files**

```bash
grep -r "MemoryStore\|MemorySource\|LearningContextSource\|ConversationEmbeddingHandler\|memory_note" tests/ --include="*.rs" -l
```

**Step 2: Update each test**

Replace `MemorySource` with `CognitiveContextSource` in context engine test setups. Replace `ConversationEmbeddingHandler` with `ConversationRecallHandler`. Remove any tests that directly test deleted components.

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: all pass

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (except pre-existing desktop exceptions)

**Step 4: Commit**

```bash
git add -A
git commit -m "test: update integration tests for unified memory system"
```

---

## Task 12: Final verification

**Step 1: Full build**

Run: `cargo build --workspace`

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`

**Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

**Step 4: Format check**

Run: `cargo fmt --all --check`

**Step 5: Verify no stale references**

```bash
grep -r "MemoryStore\|MemoryNoteRepo\|LearningContextSource\|MemorySource\|ConversationEmbeddingStore\|ConversationEmbeddingHandler\b\|ConversationMemoryRetriever\|memory_note_embeddings" crates/ tests/ --include="*.rs" -l
```

Expected: no results (or only in comments/docs that should also be cleaned up)

**Step 6: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final cleanup for R2 memory system unification"
```
