# Intelligent Scoring (SP2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the 3 dead retrieval scoring factors (knowledge depth, co-activation strength, multi-source convergence) so the 10-factor `relevance_score()` formula operates at full capacity, and add retrieval feedback for autotuner optimization.

**Architecture:** Factor 1 (`hierarchy_score`) computes knowledge depth via a `count_related` SQL query cached with 60s TTL. Factor 2 (`community_score`) tracks co-activation in a new `co_activation` table, updated after each retrieval, with two-pass re-ranking. Factor 3 (`cross_note_boost`) reads the existing `convergence_score` field (populated by SP1's pipeline). Retrieval feedback logs precision per query for autotuner consumption.

**Tech Stack:** Rust, SQLite, tokio, cargo-nextest

**Spec:** `docs/superpowers/specs/2026-04-07-memory-unification-design.md` (sections: "The 3 Scoring Factors", "Retrieval -> Autotuner Feedback")

**Depends on:** SP1 (Memory Bridge Layer) -- `convergence_score` column on `semantic_facts`, pipeline writer populating it.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Modify | Add `co_activation` table with indexes |
| `crates/cognitive/src/repos/co_activation.rs` | Create | `CoActivationRepo` -- upsert pairs, sum strengths, decay, prune |
| `crates/cognitive/src/repos/mod.rs` | Modify | Export `CoActivationRepo` |
| `crates/cognitive/src/services/scoring.rs` | Create | `knowledge_depth()`, `co_activation_score()` functions with caching |
| `crates/cognitive/src/services/mod.rs` | Modify | Export `scoring` module |
| `crates/cognitive/src/services/retrieval.rs` | Modify | Wire 3 factors into `relevance_score()` calls, add two-pass re-ranking |
| `crates/cognitive/src/services/memory_retriever.rs` | Modify | Trigger co-activation recording after retrieval |
| `crates/cognitive/src/repos/semantic_fact.rs` | Modify | Add `count_related()` query |
| `crates/storage/migrations/001_initial.sql` | Modify | Add `retrieval_feedback` table |
| `crates/storage/src/repos/retrieval_feedback.rs` | Create | `RetrievalFeedbackRepo` -- insert, query by date range |
| `crates/storage/src/repos/mod.rs` | Modify | Export `RetrievalFeedbackRepo` |
| `crates/agent/src/autotuner/metric_collector.rs` | Modify | Read retrieval precision from feedback table |

---

### Task 1: co_activation Table + CoActivationRepo

Create the co-activation tracking infrastructure.

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Create: `crates/cognitive/src/repos/co_activation.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add co_activation table to migration**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, add at the end:

```sql
-- Co-activation tracking: facts retrieved together strengthen each other
CREATE TABLE IF NOT EXISTS co_activation (
    fact_id_a TEXT NOT NULL,
    fact_id_b TEXT NOT NULL,
    strength  REAL NOT NULL DEFAULT 1.0,
    last_fired TEXT NOT NULL,
    PRIMARY KEY (fact_id_a, fact_id_b)
);
CREATE INDEX IF NOT EXISTS idx_co_activation_a ON co_activation(fact_id_a);
CREATE INDEX IF NOT EXISTS idx_co_activation_b ON co_activation(fact_id_b);
```

- [ ] **Step 2: Create CoActivationRepo**

Create `crates/cognitive/src/repos/co_activation.rs` with:

- `new(pool)` constructor
- `record_co_retrieval(fact_ids: &[String])` -- inserts/increments strength for every pair (a, b) where a < b (sorted to ensure consistent ordering). Uses `ON CONFLICT DO UPDATE SET strength = strength + 1.0`.
- `sum_strength_with_peers(fact_id, peer_ids)` -- queries both directions `(fact_id, peer)` and `(peer, fact_id)`, returns sum of strengths.
- `decay_all(factor: f64, min_strength: f64)` -- multiplies all strengths by factor, deletes pairs below min_strength.
- `count_all()` -- for metrics.

Include 5 tests: `test_record_co_retrieval` (3 facts = 3 pairs), `test_sum_strength_with_peers` (record twice = strength 2.0), `test_sum_strength_no_peers` (empty = 0.0), `test_decay_and_prune` (decay to below threshold = deleted), `test_single_fact_no_pairs` (1 fact = 0 pairs).

Each test should create the table inline since `cognitive_test_pool()` may not include it:
```sql
CREATE TABLE IF NOT EXISTS co_activation (fact_id_a TEXT NOT NULL, fact_id_b TEXT NOT NULL, strength REAL NOT NULL DEFAULT 1.0, last_fired TEXT NOT NULL, PRIMARY KEY (fact_id_a, fact_id_b))
```

- [ ] **Step 3: Register in repos module**

In `crates/cognitive/src/repos/mod.rs`, add:
```rust
pub mod co_activation;
pub use co_activation::CoActivationRepo;
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p cognitive 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(co_activation)' --no-fail-fast 2>&1
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/co_activation.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add co_activation table and CoActivationRepo

Tracks co-retrieval strength between fact pairs. Supports record,
sum-with-peers, decay (0.95x weekly), and pruning (strength < 0.1).
5 unit tests covering all operations."
```

---

### Task 2: count_related Query + Scoring Functions

Add the `count_related` SQL query and the three scoring functions.

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`
- Create: `crates/cognitive/src/services/scoring.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Add `count_related` to SemanticFactRepo**

In `crates/cognitive/src/repos/semantic_fact.rs`, add:

```rust
    /// Count active facts with the same subject in the same domain (excluding self).
    pub async fn count_related(
        &self,
        subject: &str,
        domain: &str,
        exclude_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM semantic_facts
             WHERE domain = ?1 AND subject = ?2 AND id != ?3 AND superseded_at IS NULL",
        )
        .bind(domain)
        .bind(subject)
        .bind(exclude_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
```

- [ ] **Step 2: Create scoring module**

Create `crates/cognitive/src/services/scoring.rs` with:

- `KnowledgeDepthCache` struct with `Mutex<HashMap<String, (f64, Instant)>>` and 60s TTL. Method: `get_or_compute(&self, fact, repo) -> f64`.
- `knowledge_depth(fact, repo) -> f64` -- calls `count_related`, returns `(count as f64).ln_1p() / 3.0_f64.ln_1p()`. Normalization: 0=0.0, 1=0.50, 3=1.0.
- `co_activation_score(fact_id, peer_ids, repo) -> f64` -- returns 0.0 for empty peers, otherwise `1.0 / (1.0 + (-0.5 * (total - 3.0)).exp())`. Sigmoid: 0=0.18, 3=0.50, 6=0.82, 10+=0.97.
- `convergence_score(fact) -> f64` -- returns `fact.convergence_score` (passthrough, included for documentation).

Include 4 tests: `test_knowledge_depth_normalization`, `test_co_activation_sigmoid`, `test_co_activation_empty_peers`, `test_convergence_passthrough`.

- [ ] **Step 3: Register scoring module**

In `crates/cognitive/src/services/mod.rs`, add:
```rust
pub mod scoring;
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p cognitive 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(knowledge_depth) or test(co_activation_sigmoid) or test(convergence_passthrough)' --no-fail-fast 2>&1
```

Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/semantic_fact.rs crates/cognitive/src/services/scoring.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): add scoring functions for 3 retrieval factors

knowledge_depth: log-normalized count of related facts (cached 60s TTL).
co_activation_score: sigmoid over sum of peer strengths.
convergence_score: passthrough from fact.convergence_score.
count_related SQL query on SemanticFactRepo."
```

---

### Task 3: Wire 3 Factors into retrieval.rs

Replace the hardcoded 0.0 values with actual computed scores using two-pass retrieval.

**Files:**
- Modify: `crates/cognitive/src/services/retrieval.rs`

- [ ] **Step 1: Add new parameters to `retrieve_relevant_facts`**

Add `co_activation_repo: Option<&CoActivationRepo>` and `depth_cache: Option<&KnowledgeDepthCache>` parameters to the function signature. Add the necessary imports at the top of the file:

```rust
use crate::repos::CoActivationRepo;
use crate::services::scoring::{self, KnowledgeDepthCache};
```

- [ ] **Step 2: Implement two-pass scoring in vector path**

In the vector path (around line 247), after the first-pass scores are computed with 0.0 placeholders, add a second pass. The key change: collect all scored facts, compute the 3 factors, then recompute scores and re-sort.

Replace the hardcoded `0.0` values:
- `hierarchy_score`: compute via `depth_cache.get_or_compute(&fact, repo).await` (or 0.0 if no cache)
- `community_score`: compute via `scoring::co_activation_score(&fact.id, &peer_ids, co_repo).await` (or 0.0 if no repo)
- `cross_note_boost`: read `fact.convergence_score`

After recomputing all scores, re-sort the results by the new scores.

- [ ] **Step 3: Apply same changes to fallback path**

The fallback path (around line 300) has the same hardcoded 0.0 values. Apply identical changes.

- [ ] **Step 4: Update all call sites**

```bash
grep -rn "retrieve_relevant_facts" crates/
```

Add `co_activation_repo: None, depth_cache: None` to each call site. The primary call site in `memory_retriever.rs` `fetch_facts()` should pass the actual repos (wired in Task 4).

- [ ] **Step 5: Build and fix compilation**

```bash
cargo build -p cognitive 2>&1 | tail -30
```

Fix issues: `situational_boost` and `weights` variables need to be accessible in the second pass. Variables like `elapsed_days()` and `access_frequency()` helpers need to be available. May need to capture these before the first loop.

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p cognitive --no-fail-fast 2>&1 | tail -20
```

Update any tests calling `retrieve_relevant_facts` to include the 2 new parameters.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/services/retrieval.rs
git commit -m "feat(cognitive): wire 3 scoring factors into retrieval pipeline

hierarchy_score from knowledge_depth (log-normalized related count).
community_score from co_activation_score (sigmoid over peer strengths).
cross_note_boost from fact.convergence_score. Two-pass retrieval:
base score then re-rank with computed factors."
```

---

### Task 4: Co-Activation Recording in UnifiedMemoryService

After each retrieval, record which facts were co-retrieved.

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add CoActivationRepo and depth cache to UnifiedMemoryService**

Add fields to the struct:
```rust
    co_activation_repo: Option<crate::repos::CoActivationRepo>,
    depth_cache: crate::services::scoring::KnowledgeDepthCache,
```

Update the constructor to accept `co_activation_repo` and initialize `depth_cache: KnowledgeDepthCache::new(60)`.

- [ ] **Step 2: Pass repos to retrieve_relevant_facts in fetch_facts()**

In the `fetch_facts()` method, pass `self.co_activation_repo.as_ref()` and `Some(&self.depth_cache)` to `retrieve_relevant_facts`.

- [ ] **Step 3: Record co-activation after retrieval**

In the `retrieve()` method, after RRF merge and truncation, add fire-and-forget co-activation recording:

```rust
        if let Some(ref co_repo) = self.co_activation_repo {
            let fact_ids: Vec<String> = final_results.iter()
                .filter_map(|e| e.fact_id.clone())
                .collect();
            if fact_ids.len() >= 2 {
                let co_repo = co_repo.clone();
                tokio::spawn(async move {
                    let _ = co_repo.record_co_retrieval(&fact_ids).await;
                });
            }
        }
```

Note: `MemoryEntry` may not have a `fact_id` field. Check the struct -- if not, add `pub fact_id: Option<String>` and populate it when creating entries from scored facts.

- [ ] **Step 4: Update UnifiedMemoryService construction sites**

```bash
grep -rn "UnifiedMemoryService::new\|UnifiedMemoryService {" crates/agent/ crates/app-core/
```

Add `co_activation_repo: Some(CoActivationRepo::new(pool.clone()))` at each site.

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive -p agent -p app-core 2>&1 | tail -30
cargo nextest run -p cognitive --no-fail-fast 2>&1 | tail -20
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/memory_retriever.rs
git commit -m "feat(cognitive): record co-activation after retrieval

Fire-and-forget tokio::spawn records co-retrieved fact pairs. Passes
CoActivationRepo and KnowledgeDepthCache to retrieve_relevant_facts
for two-pass scoring with actual factor values."
```

---

### Task 5: Retrieval Feedback Table + Repo

Track retrieval precision for autotuner consumption.

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/repos/retrieval_feedback.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Add retrieval_feedback table**

In `crates/storage/migrations/001_initial.sql`, add at the end:

```sql
CREATE TABLE IF NOT EXISTS retrieval_feedback (
    id TEXT PRIMARY KEY,
    retrieved_fact_ids TEXT NOT NULL,
    referenced_fact_ids TEXT NOT NULL,
    precision REAL NOT NULL,
    session_key TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_session ON retrieval_feedback(session_key);
CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_created ON retrieval_feedback(created_at);
```

- [ ] **Step 2: Create RetrievalFeedbackRepo**

Create `crates/storage/src/repos/retrieval_feedback.rs` with:

- `new(pool)` constructor
- `insert(retrieved_ids, referenced_ids, session_key)` -- computes precision = referenced/retrieved, stores as JSON arrays
- `avg_precision_since(days: i64) -> f64` -- average precision over past N days
- `count_since(days: i64) -> i64` -- count entries in period

- [ ] **Step 3: Register in storage repos**

In `crates/storage/src/repos/mod.rs`, add:
```rust
pub mod retrieval_feedback;
pub use retrieval_feedback::RetrievalFeedbackRepo;
```

- [ ] **Step 4: Build**

```bash
cargo build -p storage 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/repos/retrieval_feedback.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add retrieval_feedback table and RetrievalFeedbackRepo

Tracks which retrieved facts the LLM referenced. Records precision
(referenced / retrieved) per query. Supports avg_precision_since
for autotuner evaluation."
```

---

### Task 6: Retrieval Feedback Detection + Autotuner Integration

Detect which facts the LLM referenced and wire into autotuner.

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`
- Modify: `crates/agent/src/autotuner/metric_collector.rs`

- [ ] **Step 1: Add feedback detection function**

In `crates/cognitive/src/services/memory_retriever.rs`, add:

```rust
/// Heuristically detect which facts the LLM referenced in its response.
pub fn detect_referenced_facts(
    response_text: &str,
    retrieved_facts: &[(String, String, String)], // (id, subject, predicate)
) -> Vec<String> {
    let response_lower = response_text.to_lowercase();
    retrieved_facts
        .iter()
        .filter(|(_, subject, _)| {
            let words: Vec<&str> = subject.to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .collect();
            if words.is_empty() { return false; }
            let matches = words.iter()
                .filter(|w| response_lower.contains(*w))
                .count();
            matches as f64 / words.len() as f64 > 0.5
        })
        .map(|(id, _, _)| id.clone())
        .collect()
}
```

- [ ] **Step 2: Add test for detection**

```rust
    #[test]
    fn test_detect_referenced_facts() {
        let response = "Jayden is working on a Rust project and prefers morning sessions.";
        let retrieved = vec![
            ("f1".into(), "Jayden".into(), "occupation".into()),
            ("f2".into(), "Rust project".into(), "language".into()),
            ("f3".into(), "afternoon breaks".into(), "schedule".into()),
        ];
        let referenced = detect_referenced_facts(response, &retrieved);
        assert!(referenced.contains(&"f1".to_string()));
        assert!(!referenced.contains(&"f3".to_string()));
    }
```

- [ ] **Step 3: Wire feedback into autotuner metric collector**

In `crates/agent/src/autotuner/metric_collector.rs`:

Add `feedback_repo: Option<storage::RetrievalFeedbackRepo>` to the collector struct. In `collect_metrics()`, read avg precision:

```rust
        let feedback_precision = if let Some(ref repo) = self.feedback_repo {
            repo.avg_precision_since(7).await.unwrap_or(0.0)
        } else {
            0.0
        };
```

Incorporate `feedback_precision` into the trial score. Find the existing `retrieval_precision` field in the metrics and augment or replace it.

- [ ] **Step 4: Update metric collector construction**

Search and add `feedback_repo: Some(RetrievalFeedbackRepo::new(pool.clone()))`.

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive -p agent 2>&1 | tail -20
cargo nextest run -p cognitive -E 'test(detect_referenced)' --no-fail-fast 2>&1
cargo nextest run -p agent -E 'test(autotuner) or test(metric)' --no-fail-fast 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/memory_retriever.rs crates/agent/src/autotuner/metric_collector.rs
git commit -m "feat(cognitive): add retrieval feedback detection + autotuner integration

Heuristic subject-keyword scanning detects which retrieved facts the
LLM referenced. Autotuner reads avg_precision_since(7) from feedback
table during trial evaluation."
```

---

### Task 7: Full Validation

- [ ] **Step 1: Add new tables to dev DB**

```bash
sqlite3 ~/.klyntbot-dev/data.db "CREATE TABLE IF NOT EXISTS co_activation (fact_id_a TEXT NOT NULL, fact_id_b TEXT NOT NULL, strength REAL NOT NULL DEFAULT 1.0, last_fired TEXT NOT NULL, PRIMARY KEY (fact_id_a, fact_id_b));"
sqlite3 ~/.klyntbot-dev/data.db "CREATE INDEX IF NOT EXISTS idx_co_activation_a ON co_activation(fact_id_a);"
sqlite3 ~/.klyntbot-dev/data.db "CREATE INDEX IF NOT EXISTS idx_co_activation_b ON co_activation(fact_id_b);"
sqlite3 ~/.klyntbot-dev/data.db "CREATE TABLE IF NOT EXISTS retrieval_feedback (id TEXT PRIMARY KEY, retrieved_fact_ids TEXT NOT NULL, referenced_fact_ids TEXT NOT NULL, precision REAL NOT NULL, session_key TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')));"
sqlite3 ~/.klyntbot-dev/data.db "CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_session ON retrieval_feedback(session_key);"
sqlite3 ~/.klyntbot-dev/data.db "CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_created ON retrieval_feedback(created_at);"
```

- [ ] **Step 2: Build workspace**

```bash
cargo build --workspace 2>&1 | tail -10
```

- [ ] **Step 3: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -10
```

- [ ] **Step 4: Format**

```bash
cargo fmt --all --check
```

- [ ] **Step 5: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 6: Format commit if needed**

```bash
cargo fmt --all
git add -A && git diff --cached --stat
```

If changes: `git commit -m "style: format after intelligent scoring implementation"`

---

## Summary

| Task | What It Builds | Key Output |
|------|---------------|------------|
| 1 | `co_activation` table + `CoActivationRepo` | DB schema, CRUD with decay/prune, 5 tests |
| 2 | `count_related` query + scoring functions | `knowledge_depth()`, `co_activation_score()`, `convergence_score()`, 4 tests |
| 3 | Wire 3 factors into `retrieval.rs` | Replace hardcoded 0.0s, two-pass re-ranking |
| 4 | Co-activation recording in `UnifiedMemoryService` | Fire-and-forget `record_co_retrieval` after retrieval |
| 5 | `retrieval_feedback` table + repo | DB schema, insert/query, avg_precision_since |
| 6 | Feedback detection + autotuner integration | Heuristic scanning + autotuner reads precision |
| 7 | Full validation | Build, clippy, format, tests, dev DB migration |

## How to Verify SP2

After implementation, send 3+ messages in the app then check:

```bash
# Co-activation pairs formed?
sqlite3 ~/.klyntbot-dev/data.db "SELECT * FROM co_activation LIMIT 10"

# Retrieval feedback logged?
sqlite3 ~/.klyntbot-dev/data.db "SELECT precision FROM retrieval_feedback ORDER BY created_at DESC LIMIT 5"

# Scoring factors non-zero? (check tracing logs)
# Look for "relevance_score" or "hierarchy" or "community" in app logs
```
