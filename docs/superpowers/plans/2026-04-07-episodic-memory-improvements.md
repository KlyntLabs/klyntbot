# Episodic Memory Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make episodic memories useful faster (lower reflection threshold), generate summaries at creation, wire FTS search into retrieval, and include recent episodes in per-turn LLM context.

**Architecture:** Four independent improvements to the existing episodic memory system. Task 1 is a constant change. Task 2 adds a summary generation step at episodic creation time. Task 3 adds an `EpisodicMemory` source to `MemorySource` and merges FTS results into `UnifiedMemoryService.retrieve()`. Task 4 adds a `fetch_episodes` method alongside `fetch_facts` and `fetch_recalls`.

**Tech Stack:** Rust, SQLite FTS5, tokio, async-trait, cargo-nextest

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/src/services/reflection.rs:51` | Modify | Lower `MIN_EPISODE_COUNT` from 20 to 8 |
| `crates/cognitive/src/services/background.rs:427-452` | Modify | Generate summary at episodic creation time |
| `crates/context_engine/src/memory_retriever.rs:5-12` | Modify | Add `EpisodicMemory` variant to `MemorySource` |
| `crates/cognitive/src/services/memory_retriever.rs` | Modify | Add `fetch_episodes`, `EpisodicMemoryRepo` field, merge into `retrieve()` |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `EpisodicMemoryRepo` into `UnifiedMemoryService` |

---

### Task 1: Lower reflection threshold from 20 to 8

**Files:**
- Modify: `crates/cognitive/src/services/reflection.rs:51`

- [ ] **Step 1: Change the constant**

In `crates/cognitive/src/services/reflection.rs`, line 51:

```rust
const MIN_EPISODE_COUNT: usize = 20;
```

Change to:

```rust
const MIN_EPISODE_COUNT: usize = 8;
```

- [ ] **Step 2: Build and verify**

```bash
cargo build -p cognitive
```

Expected: clean build.

- [ ] **Step 3: Run reflection tests**

```bash
cargo nextest run -p cognitive -E 'test(reflection)' --no-capture
```

Expected: all pass. If any test asserts on the old value of 20, update the test to match 8.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/reflection.rs
git commit -m "fix(cognitive): lower reflection MIN_EPISODE_COUNT from 20 to 8

20 episodes required weeks of high-importance events before the
first weekly reflection could run. Lowering to 8 enables the
reflection → procedural rules pipeline to activate much sooner,
especially for new users."
```

---

### Task 2: Generate summary at episodic memory creation

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:427-452`

- [ ] **Step 1: Add a summary generation helper**

In `crates/cognitive/src/services/background.rs`, add a helper function near the bottom of the file (before the `#[cfg(test)]` block):

```rust
/// Generate a concise one-line summary from observation content.
/// Truncates to ~100 chars and strips role prefixes from enriched context.
fn summarize_observation(content: &str) -> String {
    // For enriched ChatTurnCompleted content (multi-line [role]: text format),
    // extract just the last user message
    let last_user_line = content
        .lines()
        .rev()
        .find(|l| l.starts_with("[user]:"))
        .map(|l| l.trim_start_matches("[user]: ").trim());

    let base = last_user_line.unwrap_or(content);

    // Truncate to ~120 chars at a word boundary
    if base.len() <= 120 {
        base.to_string()
    } else {
        let truncated = &base[..120];
        match truncated.rfind(' ') {
            Some(pos) if pos > 60 => format!("{}...", &truncated[..pos]),
            _ => format!("{truncated}..."),
        }
    }
}
```

- [ ] **Step 2: Use the summary in episodic memory creation**

In the episodic memory creation block (lines 427-452), change line 436 from:

```rust
                                    summary: None,
```

to:

```rust
                                    summary: Some(summarize_observation(&obs.content)),
```

- [ ] **Step 3: Add test for the summarizer**

In the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn summarize_observation_short_text() {
    let result = summarize_observation("User prefers dark mode");
    assert_eq!(result, "User prefers dark mode");
}

#[test]
fn summarize_observation_long_text() {
    let long = "a ".repeat(100);
    let result = summarize_observation(&long);
    assert!(result.len() <= 125, "summary should be truncated: {}", result.len());
    assert!(result.ends_with("..."));
}

#[test]
fn summarize_observation_enriched_context() {
    let enriched = "[user]: What language do you use?\n[assistant]: I can help with many!\n[user]: I prefer Rust for backend work";
    let result = summarize_observation(enriched);
    assert_eq!(result, "I prefer Rust for backend work");
}
```

- [ ] **Step 4: Build and run tests**

```bash
cargo build -p cognitive
cargo nextest run -p cognitive -E 'test(summarize_observation)' --no-capture
```

Expected: all 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): generate summary at episodic memory creation

Episodic memories now have a concise one-line summary instead of
None. For enriched multi-turn context, extracts the last user
message. For regular observations, truncates at ~120 chars.
Improves reflection input quality and UI display."
```

---

### Task 3: Add `EpisodicMemory` variant to `MemorySource`

**Files:**
- Modify: `crates/context_engine/src/memory_retriever.rs:5-12`

- [ ] **Step 1: Add the variant**

In `crates/context_engine/src/memory_retriever.rs`, add a new variant to the `MemorySource` enum:

```rust
pub enum MemorySource {
    /// Extracted/consolidated semantic fact (FSRS-scored).
    CognitiveFact,
    /// Past conversation message (time-decay scored).
    ConversationRecall,
    /// Significant event record (episodic memory).
    EpisodicMemory,
    /// Domain-specific search result (notes, tasks, finance, graph).
    Domain { name: String },
}
```

- [ ] **Step 2: Build workspace to check for exhaustive match issues**

```bash
cargo build --workspace 2>&1 | head -30
```

If any `match` on `MemorySource` is non-exhaustive (no `_ =>` wildcard), add the `EpisodicMemory` arm. Most matches likely use `_` or are irrelevant.

- [ ] **Step 3: Commit**

```bash
git add crates/context_engine/src/memory_retriever.rs
git commit -m "feat(context_engine): add EpisodicMemory variant to MemorySource

Enables episodic memory results to be distinguished from semantic
facts and conversation recalls in the UnifiedMemoryService merge."
```

---

### Task 4: Wire episodic FTS search into UnifiedMemoryService

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Add `EpisodicMemoryRepo` field to `UnifiedMemoryService`**

In `crates/cognitive/src/services/memory_retriever.rs`, add the import at the top:

```rust
use crate::repos::EpisodicMemoryRepo;
use crate::search::bm25::search_episodic_memories;
```

Add the field to the struct (after `champion_overrides`):

```rust
    episodic_repo: Option<EpisodicMemoryRepo>,
```

Add to `new()` constructor:

```rust
            episodic_repo: None,
```

Add builder method:

```rust
    pub fn with_episodic_repo(mut self, repo: EpisodicMemoryRepo) -> Self {
        self.episodic_repo = Some(repo);
        self
    }
```

- [ ] **Step 2: Add `fetch_episodes` method**

Add to the `impl UnifiedMemoryService` block (after `fetch_recalls`):

```rust
    /// Fetch recent episodic memories matching the query via FTS5 BM25 search.
    async fn fetch_episodes(&self, query: &str, limit: usize) -> Vec<(String, f64, String)> {
        let Some(ref repo) = self.episodic_repo else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }

        match search_episodic_memories(repo.pool(), query, None, limit).await {
            Ok(results) => {
                // Load full episodic memories for content
                let mut entries = Vec::with_capacity(results.len());
                for bm25 in results {
                    if let Ok(Some(mem)) = repo.get(&bm25.id).await {
                        let content = mem.summary.unwrap_or(mem.content);
                        entries.push((bm25.id, bm25.score, content));
                    }
                }
                entries
            }
            Err(e) => {
                warn!("Episodic BM25 search failed: {e}");
                Vec::new()
            }
        }
    }
```

Note: check if `EpisodicMemoryRepo` has a `pool()` method. If not, use the pool from `self.fact_repo.pool()` or add `pool: SqlitePool` to `EpisodicMemoryRepo`. Also check if `EpisodicMemoryRepo` has a `get(id)` method — if not, add one or load content inline from the BM25 result. Use `grep -n "fn get\|fn pool\|pub fn" crates/cognitive/src/repos/episodic_memory.rs` to check.

- [ ] **Step 3: Merge episodes into `retrieve()`**

In the `impl MemoryRetriever for UnifiedMemoryService` block, update the `retrieve` method. Change:

```rust
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Fetch concurrently
        let (facts_raw, recalls_raw) = tokio::join!(
            self.fetch_facts(query, limit),
            self.fetch_recalls(query, limit)
        );

        if facts_raw.is_empty() && recalls_raw.is_empty() {
            return Vec::new();
        }
```

to:

```rust
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Fetch concurrently (facts, recalls, episodes)
        let (facts_raw, recalls_raw, episodes_raw) = tokio::join!(
            self.fetch_facts(query, limit),
            self.fetch_recalls(query, limit),
            self.fetch_episodes(query, 5)  // cap episodes at 5 to avoid dominating results
        );

        if facts_raw.is_empty() && recalls_raw.is_empty() && episodes_raw.is_empty() {
            return Vec::new();
        }
```

Then in the RRF merge section, after the `recalls_deduped` loop, add the episodic merge:

```rust
        for (rank, (id, raw_score, content)) in episodes_raw.iter().enumerate() {
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = rrf_scores.entry(id.clone()).or_insert((
                0.0,
                content.clone(),
                MemorySource::EpisodicMemory,
                *raw_score,
            ));
            entry.0 += rrf;
        }
```

Update the `capacity` calculation to include episodes:

```rust
        let capacity = facts_raw.len() + recalls_deduped.len() + episodes_raw.len();
```

- [ ] **Step 4: Wire `EpisodicMemoryRepo` in the builder**

In `crates/agent/src/agent_loop/builder.rs`, find where `UnifiedMemoryService` is constructed (search for `UnifiedMemoryService::new`). After the existing builder chain (`.with_recall_opt()`, `.with_embedder_opt()`, etc.), add:

```rust
        if let Some(ref pool) = self.pool {
            memory_service = memory_service.with_episodic_repo(
                cognitive::repos::EpisodicMemoryRepo::new(pool.clone()),
            );
        }
```

Check the exact type path — it might be `cognitive::EpisodicMemoryRepo` if re-exported.

- [ ] **Step 5: Build full workspace**

```bash
cargo build --workspace 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 6: Run all memory retriever tests**

```bash
cargo nextest run -p cognitive -E 'test(memory_retriever) | test(unified)' --no-capture
cargo nextest run -p context_engine --no-capture
```

Expected: all pass.

- [ ] **Step 7: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -5
```

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/memory_retriever.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(cognitive): wire episodic memory FTS search into UnifiedMemoryService

Episodic memories are now included in per-turn memory retrieval
via BM25 FTS5 search, merged with semantic facts and conversation
recalls via RRF. Capped at 5 results to avoid dominating the
merge. Uses summary field (when available) for concise injection."
```

---

### Task 5: Full validation

**Files:** None (validation only)

- [ ] **Step 1: Build**

```bash
cargo build --workspace
```

- [ ] **Step 2: Format**

```bash
cargo fmt --all --check
```

If issues: `cargo fmt --all`

- [ ] **Step 3: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -5
```

- [ ] **Step 4: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

Expected: only the pre-existing `test_notes_tool_registered_and_discoverable` failure.

- [ ] **Step 5: Commit if needed**

```bash
cargo fmt --all
git add -A
git commit -m "style: format after episodic memory improvements"
```

---

## Summary

| Task | What it does | User impact |
|------|-------------|-------------|
| 1 | Lower reflection threshold 20→8 | First procedural rules appear in ~1 week instead of ~3 weeks |
| 2 | Generate summary at creation | Cleaner episodic entries, better reflection input |
| 3 | Add `EpisodicMemory` to `MemorySource` | Type system support for episodic results |
| 4 | Wire FTS into `UnifiedMemoryService` | Significant events surface in per-turn context ("what happened this week?") |
| 5 | Full validation | No regressions |

## Expected Behavior After Implementation

**Before:** User says "What did I work on recently?" → agent only sees semantic facts (static knowledge) and conversation recall (raw messages). No awareness of significant events.

**After:** Agent also sees episodic memories like "Budget alert: groceries over limit", "Task deploy-script failed", "User corrected: said Python not Java" — concise event summaries that provide temporal context about what happened and when.
