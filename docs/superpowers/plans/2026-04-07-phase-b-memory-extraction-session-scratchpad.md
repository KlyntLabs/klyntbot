# Phase B: Post-Turn LLM Extraction + Session Memory Scratchpad

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Klynt learn from every conversation turn (B1) and maintain coherent long conversations via a live session scratchpad (B2).

**Architecture:** B1 enriches the existing `BackgroundConsolidationService` by injecting `SessionManager` so `event_to_observation` can load the full conversation context (user + assistant + recent history) instead of just the raw user text. B2 adds a new `SessionMemoryService` (DomainEventBus subscriber) that maintains a per-session structured summary via cheap LLM calls, injected into every turn via a new `SessionMemoryContextSource`.

**Tech Stack:** Rust, SQLite, tokio, serde, async-trait, cargo-nextest

---

## File Structure

### B1: Post-Turn LLM Extraction (Enriched Context)

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/src/services/background.rs` | Modify | Add `session_repo` to config; make `event_to_observation` async, load last N messages for ChatTurnCompleted |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `SessionRepo` into `BackgroundServiceConfig` |
| `crates/cognitive/src/services/background.rs` (tests) | Modify | Add test for enriched observation |

### B2: Session Memory Scratchpad

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/storage/migrations/001_initial.sql` | Modify | Add `session_memory` table (pre-release, direct schema edit) |
| `crates/storage/src/repos/session_memory.rs` | Create | `SessionMemoryRepo` — upsert/get/delete for scratchpad text |
| `crates/storage/src/repos/mod.rs` | Modify | Register `SessionMemoryRepo` in `Repos` |
| `crates/storage/src/rows/session_memory.rs` | Create | `SessionMemoryRow` — the DB row type |
| `crates/storage/src/rows/mod.rs` | Modify | Export `SessionMemoryRow` |
| `crates/cognitive/src/services/session_memory.rs` | Create | `SessionMemoryService` — DomainEventBus subscriber, triggers LLM summary updates every N turns |
| `crates/cognitive/src/services/mod.rs` | Modify | Export `session_memory` module |
| `crates/agent/src/context_sources/session_memory_source.rs` | Create | `SessionMemoryContextSource` — injects scratchpad into system prompt at priority 88 |
| `crates/agent/src/context_sources/mod.rs` | Modify | Export `session_memory_source` |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `SessionMemoryContextSource` + start `SessionMemoryService` |
| `crates/app-core/src/init/mod.rs` | Modify | Pass repos to builder for session memory |

---

## Part 1: B1 — Enriched Post-Turn Extraction

### Task 1: Add `session_repo` to `BackgroundServiceConfig`

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:200-215`
- Modify: `crates/agent/src/agent_loop/builder.rs` (BackgroundServiceConfig construction)

- [ ] **Step 1: Add the field to `BackgroundServiceConfig`**

In `crates/cognitive/src/services/background.rs`, add to the `BackgroundServiceConfig` struct after `context_update_queue`:

```rust
    pub session_repo: Option<storage::SessionRepo>,
```

Add `use storage;` at the top of the file if not already present (check existing imports).

- [ ] **Step 2: Destructure the new field in `start()`**

In the `start()` function (line 225), the destructure block at line 226 lists all fields. Add `session_repo` to it:

```rust
        let BackgroundServiceConfig {
            mut event_rx,
            extraction,
            consolidation,
            repo,
            episodic_repo,
            embedder,
            cancel,
            pipeline_tx,
            accum_repo,
            failed_obs_repo,
            promote_threshold,
            min_days,
            domain_bus,
            context_update_queue,
            session_repo,
        } = config;
```

- [ ] **Step 3: Wire `session_repo` in the builder**

In `crates/agent/src/agent_loop/builder.rs`, find where `BackgroundServiceConfig` is constructed (search for `BackgroundServiceConfig {`). Add:

```rust
                session_repo: Some(storage::SessionRepo::new(pool.clone())),
```

Use the same `pool` that's used for other repos in the builder.

- [ ] **Step 4: Build to verify**

```bash
cargo build -p cognitive -p agent 2>&1 | head -20
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(cognitive): add session_repo to BackgroundServiceConfig

Preparation for enriched post-turn extraction. The consolidation
service can now access session history to build richer observations."
```

---

### Task 2: Enrich `event_to_observation` with conversation context

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:735-749` (event_to_observation)
- Modify: `crates/cognitive/src/services/background.rs` (main loop — pass session_repo to the new function)

- [ ] **Step 1: Make `event_to_observation` async and add session_repo parameter**

The current `event_to_observation` is a sync `fn`. Change the signature and the `ChatTurnCompleted` arm to load session history.

Replace the function (starting at line 735):

```rust
/// Convert a `DomainEvent` to an `Observation` for processing.
/// For ChatTurnCompleted, loads recent session history for richer extraction context.
async fn event_to_observation(
    event: &DomainEvent,
    session_repo: &Option<storage::SessionRepo>,
) -> Option<Observation> {
    let now = Utc::now();
    match event {
        DomainEvent::ChatTurnCompleted {
            session_key,
            user_message,
        } => {
            let user_msg = user_message.as_deref()?;

            // Try to load recent session history for richer context
            let content = if let Some(repo) = session_repo {
                match repo.get_messages(session_key).await {
                    Ok(messages) => {
                        // Take the last 6 messages (3 turns of user+assistant)
                        let recent: Vec<_> = messages
                            .iter()
                            .rev()
                            .take(6)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();

                        if recent.len() >= 2 {
                            let mut context = String::new();
                            for msg in &recent {
                                context.push_str(&format!(
                                    "[{}]: {}\n",
                                    msg.role,
                                    &msg.content[..msg.content.len().min(500)]
                                ));
                            }
                            context
                        } else {
                            user_msg.to_string()
                        }
                    }
                    Err(_) => user_msg.to_string(),
                }
            } else {
                user_msg.to_string()
            };

            Some(Observation {
                domain: "general".into(),
                content,
                importance: 0.7,
                source_event: "ChatTurnCompleted".into(),
                timestamp: now,
            })
        }
```

Note: importance raised from 0.5 to 0.7 because the enriched context gives the extractor more to work with.

All other match arms remain unchanged — copy them as-is after the `ChatTurnCompleted` arm. The only change to the other arms is adding `session_repo` as an unused parameter (the `_` is implicit since only `ChatTurnCompleted` uses it).

- [ ] **Step 2: Update all call sites of `event_to_observation`**

In the main loop (around line 329 area, inside `classify_batch` or wherever `event_to_observation` is called), the call needs to become `.await`. Search for all calls:

```bash
grep -n "event_to_observation" crates/cognitive/src/services/background.rs
```

The `classify_batch` function calls `event_to_observation` in a loop. It needs to become async or the individual calls need to be awaited inline. The simplest approach: move the `event_to_observation` call into the main loop body where we already have an async context, rather than inside `classify_batch`.

Find where `classify_batch(batch)` is called (around line 329). The `classify_batch` function at the bottom of the file calls `event_to_observation` for each event. Refactor:

Replace the `classify_batch` call and the salience classification with inline code that handles the async:

```rust
                // Classify events: salience filter + convert to observations
                let mut to_extract: Vec<Observation> = Vec::new();
                let mut to_accumulate: Vec<(DomainEvent, SalienceVerdict)> = Vec::new();

                for event in &batch {
                    let verdict = evaluate_salience(event);
                    match verdict {
                        SalienceVerdict::Extract => {
                            if let Some(obs) = event_to_observation(event, &session_repo).await {
                                to_extract.push(obs);
                            }
                        }
                        SalienceVerdict::Accumulate => {
                            to_accumulate.push((event.clone(), verdict));
                        }
                        SalienceVerdict::Discard => {}
                    }
                }
```

Then handle `to_accumulate` the same way the existing code does (the accumulator logic that follows `classify_batch`). Check if `classify_batch` is used elsewhere — if it's only called from the main loop, you can remove the standalone function.

- [ ] **Step 3: Update tests**

In the test module, update the existing `event_to_observation_chat_turn_with_message` test to pass `&None` as the session_repo (no session lookup in tests):

```rust
#[tokio::test]
async fn event_to_observation_chat_turn_with_message() {
    let event = DomainEvent::ChatTurnCompleted {
        session_key: "session-1".into(),
        user_message: Some("I'm a software engineer working on Rust projects".into()),
    };
    let obs = event_to_observation(&event, &None).await;
    assert!(obs.is_some(), "should create observation when user_message is present");
    let obs = obs.unwrap();
    assert_eq!(obs.source_event, "ChatTurnCompleted");
    assert!(obs.content.contains("software engineer"));
    assert_eq!(obs.domain, "general");
}

#[tokio::test]
async fn event_to_observation_chat_turn_without_message() {
    let event = DomainEvent::ChatTurnCompleted {
        session_key: "session-1".into(),
        user_message: None,
    };
    let obs = event_to_observation(&event, &None).await;
    assert!(obs.is_none(), "should skip when user_message is None");
}
```

- [ ] **Step 4: Add test for enriched context with session repo**

```rust
#[tokio::test]
async fn event_to_observation_chat_turn_loads_session_history() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let session_repo = storage::SessionRepo::new(pool.pool().clone());

    // Create a session with some messages
    session_repo
        .upsert_session("test-session", "{}", None)
        .await
        .unwrap();
    session_repo
        .insert_message("test-session", "msg1", "user", "I work at Acme Corp", None, None, None)
        .await
        .unwrap();
    session_repo
        .insert_message("test-session", "msg2", "assistant", "Nice! What do you do there?", None, None, None)
        .await
        .unwrap();
    session_repo
        .insert_message("test-session", "msg3", "user", "I'm a backend engineer using Rust", None, None, None)
        .await
        .unwrap();

    let event = DomainEvent::ChatTurnCompleted {
        session_key: "test-session".into(),
        user_message: Some("I'm a backend engineer using Rust".into()),
    };

    let obs = event_to_observation(&event, &Some(session_repo)).await;
    assert!(obs.is_some());
    let obs = obs.unwrap();
    // Should contain both user and assistant messages from history
    assert!(obs.content.contains("Acme Corp"), "should include earlier user message");
    assert!(obs.content.contains("backend engineer"), "should include latest message");
    assert!(obs.content.contains("[assistant]"), "should include assistant response");
}
```

Note: check the exact `SessionRepo` method signatures — `insert_message` may have different parameter ordering. Use `grep -n "fn insert_message" crates/storage/src/repos/session.rs` to verify.

- [ ] **Step 5: Build and run tests**

```bash
cargo build -p cognitive -p agent
cargo nextest run -p cognitive -E 'test(event_to_observation)' --no-capture
```

Expected: all 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): enrich ChatTurnCompleted extraction with session history

event_to_observation now loads the last 6 messages (3 turns) from
SessionRepo when processing ChatTurnCompleted events. This gives
the LLM extractor full conversation context instead of just the
raw user text, enabling extraction of implicit facts and reference
resolution."
```

---

## Part 2: B2 — Session Memory Scratchpad

### Task 3: Add `session_memory` table and repo

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/rows/session_memory.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Create: `crates/storage/src/repos/session_memory.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Add table to schema**

In `crates/storage/migrations/001_initial.sql`, add after the `session_messages` table and its index (after line 152):

```sql
-- ============================================================
-- Session Memory (per-session scratchpad for conversation state)
-- ============================================================
CREATE TABLE session_memory (
    session_key TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    content     TEXT NOT NULL DEFAULT '',
    turn_count  INTEGER NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

- [ ] **Step 2: Create row type**

Create `crates/storage/src/rows/session_memory.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionMemoryRow {
    pub session_key: String,
    pub content: String,
    pub turn_count: i64,
    pub updated_at: String,
}
```

In `crates/storage/src/rows/mod.rs`, add:

```rust
pub mod session_memory;
pub use session_memory::SessionMemoryRow;
```

- [ ] **Step 3: Create repo**

Create `crates/storage/src/repos/session_memory.rs`:

```rust
use sqlx::SqlitePool;

use crate::rows::SessionMemoryRow;

#[derive(Clone)]
pub struct SessionMemoryRepo {
    pool: SqlitePool,
}

impl SessionMemoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get the scratchpad for a session. Returns None if no scratchpad exists.
    pub async fn get(&self, session_key: &str) -> common::Result<Option<SessionMemoryRow>> {
        let row = sqlx::query_as::<_, SessionMemoryRow>(
            "SELECT session_key, content, turn_count, updated_at FROM session_memory WHERE session_key = ?",
        )
        .bind(session_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Upsert the scratchpad content and increment turn count.
    pub async fn upsert(
        &self,
        session_key: &str,
        content: &str,
        turn_count: i64,
    ) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO session_memory (session_key, content, turn_count, updated_at)
             VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             ON CONFLICT(session_key) DO UPDATE SET
                content = excluded.content,
                turn_count = excluded.turn_count,
                updated_at = excluded.updated_at",
        )
        .bind(session_key)
        .bind(content)
        .bind(turn_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete scratchpad for a session.
    pub async fn delete(&self, session_key: &str) -> common::Result<()> {
        sqlx::query("DELETE FROM session_memory WHERE session_key = ?")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Register repo in Repos**

In `crates/storage/src/repos/mod.rs`, add:

```rust
pub mod session_memory;
pub use session_memory::SessionMemoryRepo;
```

Find the `Repos` struct and add the field:

```rust
    pub session_memory: SessionMemoryRepo,
```

Find `Repos::from_pool` (or however `Repos` is constructed) and add:

```rust
    session_memory: SessionMemoryRepo::new(pool.clone()),
```

- [ ] **Step 5: Build and test repo**

```bash
cargo build -p storage
```

Expected: clean build.

- [ ] **Step 6: Add repo test**

In `crates/storage/src/repos/session_memory.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (SessionMemoryRepo, SqlitePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = SessionMemoryRepo::new(pool.pool().clone());
        // Need to create a session first (FK constraint)
        sqlx::query("INSERT INTO sessions (key) VALUES ('test-session')")
            .execute(pool.pool())
            .await
            .unwrap();
        (repo, pool.pool().clone())
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let (repo, _) = setup().await;
        repo.upsert("test-session", "Current task: auth refactor", 5)
            .await
            .unwrap();

        let row = repo.get("test-session").await.unwrap();
        assert!(row.is_some());
        let row = row.unwrap();
        assert_eq!(row.content, "Current task: auth refactor");
        assert_eq!(row.turn_count, 5);
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        let (repo, _) = setup().await;
        repo.upsert("test-session", "v1", 1).await.unwrap();
        repo.upsert("test-session", "v2", 3).await.unwrap();

        let row = repo.get("test-session").await.unwrap().unwrap();
        assert_eq!(row.content, "v2");
        assert_eq!(row.turn_count, 3);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let (repo, _) = setup().await;
        let row = repo.get("nonexistent").await.unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let (repo, _) = setup().await;
        repo.upsert("test-session", "content", 1).await.unwrap();
        repo.delete("test-session").await.unwrap();
        let row = repo.get("test-session").await.unwrap();
        assert!(row.is_none());
    }
}
```

- [ ] **Step 7: Run tests**

```bash
cargo nextest run -p storage -E 'test(session_memory)' --no-capture
```

Expected: all 4 tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add session_memory table and SessionMemoryRepo

Per-session scratchpad storage for conversation state tracking.
Stores markdown content, turn count, and last update timestamp.
Uses ON CONFLICT upsert for idempotent updates."
```

---

### Task 4: Create `SessionMemoryService` (DomainEventBus subscriber)

**Files:**
- Create: `crates/cognitive/src/services/session_memory.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/lib.rs` (if needed for re-export)

- [ ] **Step 1: Create the service**

Create `crates/cognitive/src/services/session_memory.rs`:

```rust
//! Session memory service — maintains a per-session structured scratchpad
//! by subscribing to ChatTurnCompleted events and periodically running
//! a cheap LLM call to summarize the conversation state.

use std::collections::HashMap;
use std::sync::Arc;

use bus::DomainEvent;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::services::extraction::ExtractionHandler;

/// Minimum turns before the first scratchpad update.
const MIN_TURNS_BEFORE_UPDATE: i64 = 3;

/// Update scratchpad every N turns after the first update.
const UPDATE_INTERVAL_TURNS: i64 = 3;

/// Maximum number of recent messages to include in the summary prompt.
const MAX_MESSAGES_FOR_SUMMARY: usize = 20;

/// System prompt for the scratchpad summarizer.
const SESSION_MEMORY_SYSTEM_PROMPT: &str = "\
You are a conversation state tracker. Given recent messages from a conversation, \
produce a concise structured summary of the current conversation state.\n\n\
Format your response as markdown with these sections:\n\
## Current Task\nWhat the user is working on right now (1-2 sentences)\n\n\
## Key Decisions\nBullet list of decisions made during this conversation\n\n\
## Context\nImportant background info mentioned (names, tools, files, preferences)\n\n\
## Progress\nWhat has been accomplished so far\n\n\
Rules:\n\
- Be concise — each section should be 1-5 bullet points max\n\
- Only include sections that have content\n\
- Focus on information that would be needed to continue the conversation if earlier messages were lost\n\
- If the conversation is just starting or is casual chat, return a single line summary";

/// Configuration for the session memory service.
pub struct SessionMemoryConfig {
    pub event_rx: broadcast::Receiver<DomainEvent>,
    pub session_repo: storage::SessionRepo,
    pub memory_repo: storage::SessionMemoryRepo,
    pub provider: Option<providers::DynProvider>,
    pub cancel: CancellationToken,
}

/// Background service that maintains per-session memory scratchpads.
pub struct SessionMemoryService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl SessionMemoryService {
    /// Start the background service.
    pub fn start(config: SessionMemoryConfig) -> Self {
        let SessionMemoryConfig {
            mut event_rx,
            session_repo,
            memory_repo,
            provider,
            cancel,
        } = config;

        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            // Track turn counts per session (in-memory, resets on restart)
            let mut turn_counts: HashMap<String, i64> = HashMap::new();

            loop {
                let event = tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(event) => event,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Session memory service lagged {n} events");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                };

                // Only process ChatTurnCompleted events
                let session_key = match &event {
                    DomainEvent::ChatTurnCompleted { session_key, .. } => session_key.clone(),
                    _ => continue,
                };

                // Increment turn count
                let count = turn_counts.entry(session_key.clone()).or_insert(0);
                *count += 1;
                let current_count = *count;

                // Check if we should update the scratchpad
                let should_update = if current_count < MIN_TURNS_BEFORE_UPDATE {
                    false
                } else if current_count == MIN_TURNS_BEFORE_UPDATE {
                    true // First update
                } else {
                    (current_count - MIN_TURNS_BEFORE_UPDATE) % UPDATE_INTERVAL_TURNS == 0
                };

                if !should_update {
                    continue;
                }

                // Load recent session history
                let messages = match session_repo.get_messages(&session_key).await {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        warn!("Failed to load session messages for scratchpad: {e}");
                        continue;
                    }
                };

                if messages.is_empty() {
                    continue;
                }

                // Take last N messages
                let recent: Vec<_> = messages
                    .iter()
                    .rev()
                    .take(MAX_MESSAGES_FOR_SUMMARY)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                // Build conversation text for the summarizer
                let mut conversation = String::new();
                for msg in &recent {
                    let role_label = match msg.role.as_str() {
                        "user" => "User",
                        "assistant" => "Assistant",
                        "tool" => "Tool",
                        _ => &msg.role,
                    };
                    // Truncate very long messages (tool results)
                    let content = if msg.content.len() > 300 {
                        format!("{}... [truncated, {} chars total]", &msg.content[..300], msg.content.len())
                    } else {
                        msg.content.clone()
                    };
                    conversation.push_str(&format!("{role_label}: {content}\n\n"));
                }

                // Load existing scratchpad for context
                let existing = memory_repo.get(&session_key).await.ok().flatten();
                let existing_content = existing.map(|r| r.content).unwrap_or_default();

                let user_msg = if existing_content.is_empty() {
                    format!(
                        "Summarize the current state of this conversation:\n\n{conversation}"
                    )
                } else {
                    format!(
                        "Here is the previous conversation summary:\n{existing_content}\n\n\
                         Here are the latest messages:\n{conversation}\n\n\
                         Update the summary to reflect the current state. Remove outdated information."
                    )
                };

                // Call LLM (or use heuristic fallback)
                let new_content = if let Some(ref prov) = provider {
                    let msgs = vec![
                        providers::Message::system(SESSION_MEMORY_SYSTEM_PROMPT),
                        providers::Message::user(user_msg),
                    ];
                    let params = providers::ChatParams {
                        temperature: Some(0.2),
                        max_tokens: Some(800),
                        ..Default::default()
                    };
                    match prov.chat(&msgs, None, &params).await {
                        Ok(response) => response.content.unwrap_or_default(),
                        Err(e) => {
                            warn!("Session memory LLM call failed: {e}");
                            continue;
                        }
                    }
                } else {
                    // Heuristic fallback: just store the last few messages as context
                    let mut fallback = String::from("## Recent Context\n");
                    for msg in recent.iter().rev().take(4).rev() {
                        fallback.push_str(&format!("- [{}]: {}\n",
                            msg.role,
                            &msg.content[..msg.content.len().min(100)]
                        ));
                    }
                    fallback
                };

                if new_content.is_empty() {
                    continue;
                }

                // Persist
                if let Err(e) = memory_repo
                    .upsert(&session_key, &new_content, current_count)
                    .await
                {
                    warn!("Failed to persist session memory: {e}");
                }

                debug!(
                    session_key = %session_key,
                    turn = current_count,
                    content_len = new_content.len(),
                    "Session memory updated"
                );
            }

            info!("Session memory service stopped");
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    /// Get the cancellation token.
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }
}

impl Drop for SessionMemoryService {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
```

- [ ] **Step 2: Export the module**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod session_memory;
```

In `crates/cognitive/src/lib.rs`, if services are re-exported, add:

```rust
pub use services::session_memory::SessionMemoryService;
pub use services::session_memory::SessionMemoryConfig;
```

- [ ] **Step 3: Build**

```bash
cargo build -p cognitive 2>&1 | head -20
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/session_memory.rs crates/cognitive/src/services/mod.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add SessionMemoryService for per-session scratchpads

Background service subscribing to ChatTurnCompleted events. Every 3
turns, loads recent session history and runs a cheap LLM call (or
heuristic fallback) to produce a structured markdown summary.
Persisted via SessionMemoryRepo."
```

---

### Task 5: Create `SessionMemoryContextSource`

**Files:**
- Create: `crates/agent/src/context_sources/session_memory_source.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`

- [ ] **Step 1: Create the context source**

Create `crates/agent/src/context_sources/session_memory_source.rs`:

```rust
//! Session memory context source — injects the per-session scratchpad into
//! the system prompt so the LLM has awareness of conversation state even
//! after old messages are compacted.

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};

/// Injects session memory scratchpad into the system prompt.
///
/// Priority 88 — between identity (100) and session_context entity (85).
/// This ensures conversation state appears near the top of the context,
/// after the user model but before entity-specific context.
pub struct SessionMemoryContextSource {
    repo: storage::SessionMemoryRepo,
}

impl SessionMemoryContextSource {
    pub fn new(repo: storage::SessionMemoryRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ContextSource for SessionMemoryContextSource {
    fn name(&self) -> &str {
        "session_memory"
    }

    fn priority(&self) -> u8 {
        88
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let session_key = &ctx.chat_id;

        let row = self.repo.get(session_key).await.ok().flatten()?;

        if row.content.is_empty() {
            return None;
        }

        Some(format!(
            "# Conversation State (auto-tracked)\n\n{}\n\n\
             _Updated at turn {}_",
            row.content, row.turn_count
        ))
    }
}
```

- [ ] **Step 2: Export the module**

In `crates/agent/src/context_sources/mod.rs`, add:

```rust
pub mod session_memory_source;
```

- [ ] **Step 3: Build**

```bash
cargo build -p agent 2>&1 | head -20
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/context_sources/session_memory_source.rs crates/agent/src/context_sources/mod.rs
git commit -m "feat(agent): add SessionMemoryContextSource (priority 88)

Injects per-session scratchpad into the system prompt between
identity context (100) and entity context (85). Returns None
when no scratchpad exists yet, cleanly skipping injection."
```

---

### Task 6: Wire everything in the builder and init

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/app-core/src/init/mod.rs` (or `init/cognitive.rs`)

- [ ] **Step 1: Wire `SessionMemoryContextSource` into the context engine**

In `crates/agent/src/agent_loop/builder.rs`, find where context sources are registered (search for `SessionContextSource` or `context_sources`). After the `SessionContextSource` registration, add:

```rust
        // Session memory scratchpad (priority 88)
        if let Some(ref pool) = self.pool {
            sources.push(Box::new(
                crate::context_sources::session_memory_source::SessionMemoryContextSource::new(
                    storage::SessionMemoryRepo::new(pool.clone()),
                ),
            ));
        }
```

- [ ] **Step 2: Start `SessionMemoryService` in the builder**

Find where `BackgroundConsolidationService::start()` is called in the builder. After it, add:

```rust
        // Start session memory service
        if let Some(ref pool) = self.pool {
            if let Some(ref domain_bus) = self.domain_event_bus {
                let session_memory_service = cognitive::SessionMemoryService::start(
                    cognitive::SessionMemoryConfig {
                        event_rx: domain_bus.subscribe(),
                        session_repo: storage::SessionRepo::new(pool.clone()),
                        memory_repo: storage::SessionMemoryRepo::new(pool.clone()),
                        provider: self.cognitive_provider.clone(),
                        cancel: cancel_token.clone(),
                    },
                );
                // Store handle to prevent drop (which cancels the service)
                background_handles.push(session_memory_service);
            }
        }
```

Check how `BackgroundConsolidationService` handles are stored — follow the same pattern. If there's a `Vec<JoinHandle>` or similar, add the session memory service handle to it. If handles are stored individually, add a new field.

- [ ] **Step 3: Build full workspace**

```bash
cargo build --workspace 2>&1 | head -20
```

Expected: clean build.

- [ ] **Step 4: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation)' 2>&1 | tail -10
```

Expected: all pass (except the pre-existing `test_notes_tool_registered_and_discoverable`).

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/app-core/src/init/
git commit -m "feat(agent): wire SessionMemoryService + SessionMemoryContextSource

Starts the session memory background service alongside the
cognitive consolidation service. Session scratchpads are now
automatically maintained and injected into every LLM call."
```

---

### Task 7: Full validation

**Files:** None (validation only)

- [ ] **Step 1: Build**

```bash
cargo build --workspace
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Format**

```bash
cargo fmt --all --check
```

If issues, run `cargo fmt --all` and commit.

- [ ] **Step 4: Run relevant tests**

```bash
cargo nextest run -p storage -E 'test(session_memory)' --no-capture
cargo nextest run -p cognitive -E 'test(event_to_observation)' --no-capture
cargo nextest run -p agent --no-capture 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 5: Commit if needed**

```bash
cargo fmt --all
git add -A
git commit -m "style: format after Phase B changes"
```

---

## Summary

| Task | What it does | Key files |
|------|-------------|-----------|
| 1 | Add `session_repo` to BackgroundServiceConfig | background.rs, builder.rs |
| 2 | Enrich extraction with conversation context | background.rs (event_to_observation becomes async) |
| 3 | Session memory table + repo | storage migrations, repos |
| 4 | SessionMemoryService (event subscriber) | cognitive/services/session_memory.rs |
| 5 | SessionMemoryContextSource (priority 88) | agent/context_sources/session_memory_source.rs |
| 6 | Wire everything in builder + init | builder.rs, init/mod.rs |
| 7 | Full validation | build, clippy, tests |

## Expected Behavior After Implementation

**B1 — Enriched extraction:**
- User says "Yeah, I use Rust for that" (after discussing work)
- Extractor sees full context: `[user]: I work at Acme → [assistant]: What language? → [user]: Rust`
- Extracts: `user:employer = Acme Corp`, `user:primary_language = Rust`

**B2 — Session scratchpad:**
- After 3 turns, scratchpad appears in system prompt
- After 6 turns, scratchpad updates with latest state
- When MidLoopCompressor fires (long tool chains), old messages can be dropped because the scratchpad preserves the essential context
- Next turn after compaction: LLM still knows what the user was working on

## Simulator Metrics Impact

| Metric | Expected change | Why |
|--------|----------------|-----|
| `fact_extraction_accuracy` | +20-40% | Richer context = better extraction |
| `knowledge_retention` | +10-20% | More facts extracted = more retained |
| `personalization_score` | +15-25% | Composite of retention + precision + recall |
| `response_quality` | +5-10% | Session memory keeps conversations coherent |
