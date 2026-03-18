# Persona Squads Phase 3: Multi-Agent Collaboration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add multi-round debate between personas, shared blackboard working memory, memory promotion from persona→squad→global scope, consensus detection, and FSRS-based persona learning — turning squads from parallel responders into collaborative agents.

**Architecture:** Extend `SquadExecutor` with a debate loop that runs N rounds of persona invocation. Each round, personas see the blackboard (accumulated observations from prior rounds). The orchestrator detects consensus after each round and terminates early when personas agree. Post-debate, a promotion pipeline elevates high-confidence observations to squad/global memory. FSRS tracks per-persona accuracy over time, adjusting persona reliability scores.

**Tech Stack:** Rust (cognitive/agent/app-core/desktop-shared crates), SQLite, TypeScript/React (desktop-ui), Tauri IPC, SSE streaming.

**Spec:** `docs/superpowers/specs/2026-03-18-persona-squads-design.md` (Phase 3 section)

**Phase 1 reference:** `docs/superpowers/plans/2026-03-18-persona-squads.md` — schema, SquadRepo, parallel persona calls in Insight Review.

**Phase 2 reference:** `docs/superpowers/plans/2026-03-18-persona-squads-phase2.md` — SquadExecutor, squad chat mode, multi-voice/synthesized toggle.

---

## File Map

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/repos/blackboard.rs` | BlackboardRepo — transient shared working memory for debate rounds |
| `crates/cognitive/src/repos/persona_accuracy.rs` | PersonaAccuracyRepo — FSRS-based accuracy tracking per persona |
| `crates/cognitive/src/services/memory_promotion.rs` | MemoryPromotionPipeline — elevate observations persona→squad→global (no agent deps) |
| `crates/agent/src/intent_pipeline/engines/debate.rs` | DebateOrchestrator — multi-round debate loop with consensus detection |
| `desktop-ui/src/features/chat/components/DebateRound.tsx` | Renders a single debate round with persona responses |
| `desktop-ui/src/features/chat/components/ConsensusIndicator.tsx` | Visual indicator for consensus state (converging/divergent/reached) |
| `desktop-ui/src/features/chat/components/DebateView.tsx` | Container for multi-round debate visualization |

### Modified Files
| File | Changes |
|------|---------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `blackboard_entries` table, `persona_accuracy` table |
| `crates/cognitive/src/repos/mod.rs` | Export blackboard + persona_accuracy modules, bump migration version |
| `crates/cognitive/src/repos/semantic_fact.rs:23-68` | Add `scope_type`/`scope_id` to `upsert()` INSERT/UPDATE, add `list_by_scope()` |
| `crates/cognitive/src/services/retrieval.rs:80-86` | Add `scope_chain` parameter to `retrieve_relevant_facts()` for scoped filtering |
| `crates/cognitive/src/services/memory_retriever.rs:146-160` | Add `retrieve_scoped()` method accepting scope chain |
| `crates/agent/src/intent_pipeline/engines/squad.rs` | Extract shared types, re-export from debate module |
| `crates/agent/src/intent_pipeline/engines/mod.rs` | Export debate module |
| `crates/agent/src/agent_runtime/runtime.rs:602-696` | Detect debate mode, delegate to DebateOrchestrator |
| `crates/agent/src/events.rs` | Add `DebateRoundStarted`, `DebateRoundCompleted`, `ConsensusReached`, `MemoryPromoted` events |
| `crates/desktop-shared/src/events.rs` | Add debate event payload types |
| `crates/app-core/src/handlers/chat/streaming.rs` | Handle new debate events in SSE relay |
| `desktop-ui/src/shared/types/chat.ts` | Add debate payload interfaces |
| `desktop-ui/src/shared/stores/chatStreamStore.ts` | Accumulate debate round state |
| `desktop-ui/src/features/chat/hooks/useAgentStream.ts` | Expose debate round data |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Render DebateView when debate rounds present |

---

## Task 1: Scoped Memory Retrieval — Fix `upsert()` + Add `list_by_scope()`

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs:23-68`

This is the foundation. The `scope_type`/`scope_id` columns exist in the schema but `upsert()` doesn't write them, and no query filters by scope.

- [ ] **Step 1: Write test for scoped upsert + retrieval**

Add to the test module in `semantic_fact.rs`:

```rust
#[tokio::test]
async fn test_scoped_fact_upsert_and_list() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool.clone());

    // Insert a squad-scoped fact
    let mut fact = SemanticFact {
        id: "test-squad-fact-1".into(),
        domain: "finance".into(),
        subject: "index funds".into(),
        predicate: "recommended_by".into(),
        object: "Deep Analyst".into(),
        confidence: 0.9,
        source: "debate".into(),
        valid_from: chrono::Utc::now().to_rfc3339(),
        valid_until: None,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: "observation".into(),
        scope_type: "squad".into(),
        scope_id: Some("builtin-squad-finance".into()),
    };
    repo.upsert(&fact).await.unwrap();

    // Insert a system-scoped fact
    fact.id = "test-system-fact-1".into();
    fact.scope_type = "system".into();
    fact.scope_id = None;
    repo.upsert(&fact).await.unwrap();

    // list_by_scope should return only squad-scoped
    let squad_facts = repo.list_by_scope("squad", Some("builtin-squad-finance")).await.unwrap();
    assert_eq!(squad_facts.len(), 1);
    assert_eq!(squad_facts[0].id, "test-squad-fact-1");

    // list_by_scope_chain should return both system + squad
    let chain = vec![
        ("system", None),
        ("squad", Some("builtin-squad-finance")),
    ];
    let all = repo.list_by_scope_chain(&chain).await.unwrap();
    assert_eq!(all.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_scoped_fact)'`
Expected: FAIL — `scope_type`/`scope_id` not in upsert, methods don't exist.

- [ ] **Step 3: Add scope_type/scope_id to upsert()**

In `crates/cognitive/src/repos/semantic_fact.rs`, update the `upsert()` method to include `scope_type` and `scope_id` in the INSERT column list and bind params:

```rust
pub async fn upsert(&self, fact: &SemanticFact) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO semantic_facts (id, domain, subject, predicate, object, confidence, source,
            valid_from, valid_until, recorded_at, superseded_at, superseded_by,
            stability, last_accessed, access_count, project_id, memory_type,
            scope_type, scope_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
        ON CONFLICT (id) DO UPDATE SET
            domain = excluded.domain,
            subject = excluded.subject,
            predicate = excluded.predicate,
            object = excluded.object,
            confidence = excluded.confidence,
            source = excluded.source,
            valid_from = excluded.valid_from,
            valid_until = excluded.valid_until,
            superseded_at = excluded.superseded_at,
            superseded_by = excluded.superseded_by,
            stability = excluded.stability,
            last_accessed = excluded.last_accessed,
            access_count = excluded.access_count,
            project_id = excluded.project_id,
            memory_type = excluded.memory_type,
            scope_type = excluded.scope_type,
            scope_id = excluded.scope_id
        "#,
    )
    .bind(&fact.id)
    .bind(&fact.domain)
    .bind(&fact.subject)
    .bind(&fact.predicate)
    .bind(&fact.object)
    .bind(fact.confidence)
    .bind(&fact.source)
    .bind(&fact.valid_from)
    .bind(&fact.valid_until)
    .bind(&fact.recorded_at)
    .bind(&fact.superseded_at)
    .bind(&fact.superseded_by)
    .bind(fact.stability)
    .bind(&fact.last_accessed)
    .bind(fact.access_count)
    .bind(&fact.project_id)
    .bind(&fact.memory_type)
    .bind(&fact.scope_type)
    .bind(&fact.scope_id)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 4: Add list_by_scope() and list_by_scope_chain()**

```rust
/// List active facts for a specific scope.
pub async fn list_by_scope(
    &self,
    scope_type: &str,
    scope_id: Option<&str>,
) -> Result<Vec<SemanticFact>, sqlx::Error> {
    if let Some(sid) = scope_id {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE scope_type = ?1 AND scope_id = ?2 AND superseded_at IS NULL ORDER BY recorded_at DESC",
        )
        .bind(scope_type)
        .bind(sid)
        .fetch_all(&self.pool)
        .await
    } else {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE scope_type = ?1 AND scope_id IS NULL AND superseded_at IS NULL ORDER BY recorded_at DESC",
        )
        .bind(scope_type)
        .fetch_all(&self.pool)
        .await
    }
}

/// List active facts visible to a scope chain (e.g., system + squad + persona).
/// Returns facts matching ANY tier in the chain, deduplicated by ID.
///
/// Uses N separate `list_by_scope()` calls + dedup to avoid dynamic SQL bind issues.
pub async fn list_by_scope_chain(
    &self,
    chain: &[(&str, Option<&str>)],
) -> Result<Vec<SemanticFact>, sqlx::Error> {
    if chain.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    for (scope_type, scope_id) in chain {
        let facts = self.list_by_scope(scope_type, *scope_id).await?;
        for fact in facts {
            if seen.insert(fact.id.clone()) {
                results.push(fact);
            }
        }
    }
    results.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
    Ok(results)
}

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(test_scoped_fact)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): add scope_type/scope_id to upsert, add list_by_scope + list_by_scope_chain"
```

---

## Task 2: Scoped Retrieval in UnifiedMemoryService

**Files:**
- Modify: `crates/cognitive/src/services/retrieval.rs:80-86`
- Modify: `crates/cognitive/src/services/memory_retriever.rs:146-160`

Extend `retrieve_relevant_facts()` and `UnifiedMemoryService` to accept a scope chain so persona calls can see Global + Squad + Persona memory.

- [ ] **Step 1: Add scope_chain to RetrievalParams**

In `crates/cognitive/src/services/retrieval.rs`, extend `RetrievalParams`:

```rust
pub struct RetrievalParams {
    // ... existing fields ...
    /// Optional scope chain for filtering. When set, only facts matching
    /// these scopes are considered. When empty, all scopes are included (backwards-compatible).
    pub scope_chain: Vec<(String, Option<String>)>,
}
```

Update `RetrievalParams::new()` to set `scope_chain: Vec::new()`.

- [ ] **Step 2: Filter by scope in fallback_path()**

In the `fallback_path()` function (which loads facts from SQL), add scope filtering:

```rust
async fn fallback_path(
    repo: &SemanticFactRepo,
    domains: &[&str],
    situational_boost: f64,
    weights: &RelevanceWeights,
    scope_chain: &[(String, Option<String>)],
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let candidates = if scope_chain.is_empty() {
        // Backwards-compatible: load all domains
        let mut all = Vec::new();
        for domain in domains {
            all.extend(repo.list_active(domain).await?);
        }
        all
    } else {
        // Scoped: load facts visible to the scope chain
        let chain_refs: Vec<(&str, Option<&str>)> = scope_chain
            .iter()
            .map(|(st, sid)| (st.as_str(), sid.as_deref()))
            .collect();
        repo.list_by_scope_chain(&chain_refs).await?
    };
    // ... rest of scoring unchanged ...
}
```

Thread `scope_chain` through `retrieve_relevant_facts()` → `fallback_path()` and `vector_path()`.

- [ ] **Step 3: Add retrieve_scoped() to UnifiedMemoryService**

In `crates/cognitive/src/services/memory_retriever.rs`:

```rust
impl UnifiedMemoryService {
    /// Retrieve memories visible to a specific scope chain.
    ///
    /// `scope_chain` is e.g. `[("system", None), ("squad", Some("squad-id")), ("persona", Some("persona-id"))]`.
    /// Each persona sees global + its squad's + its own memories.
    pub async fn retrieve_scoped(
        &self,
        query: &str,
        limit: usize,
        scope_chain: Vec<(String, Option<String>)>,
    ) -> Vec<MemoryEntry> {
        // Same as retrieve() but passes scope_chain through to RetrievalParams
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Vec::new();
        }
        let situational_boost = self.current_situational_boost().await;
        let params = RetrievalParams {
            limit,
            scope_chain,
            // ... same as fetch_facts() ...
            vector_top_k: self.config.vector_top_k,
            min_similarity: self.config.min_similarity,
            situational_boost,
            max_stability: self.config.max_stability,
            relevance_weight_semantic: self.config.relevance_weight_semantic,
            relevance_weight_retrievability: self.config.relevance_weight_retrievability,
            relevance_weight_importance: self.config.relevance_weight_importance,
            relevance_weight_frequency: self.config.relevance_weight_frequency,
            relevance_weight_situation: self.config.relevance_weight_situation,
            relevance_weight_temporal: self.config.relevance_weight_temporal,
        };
        match retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            USER_MODEL_DOMAINS,
            &params,
        )
        .await
        {
            Ok(facts) => facts
                .into_iter()
                .filter(|f| f.score > 0.3)
                .map(|f| MemoryEntry {
                    id: f.fact.id,
                    content: format!("{}: {} = {}", f.fact.subject, f.fact.predicate, f.fact.object),
                    score: f.score,
                    source: MemorySource::CognitiveFact,
                })
                .collect(),
            Err(e) => {
                warn!("Scoped retrieval failed: {e}");
                Vec::new()
            }
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p cognitive 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/retrieval.rs crates/cognitive/src/services/memory_retriever.rs
git commit -m "feat(cognitive): add scoped retrieval to UnifiedMemoryService and retrieve_relevant_facts"
```

---

## Task 3: Blackboard Schema + Repo

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Create: `crates/cognitive/src/repos/blackboard.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

The blackboard is a transient shared working memory for a single debate session. Personas read from and write to it during debate rounds.

- [ ] **Step 1: Add blackboard_entries table to migration**

Append to `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
-- ── Blackboard (Phase 3: transient shared working memory for debate) ────
CREATE TABLE IF NOT EXISTS blackboard_entries (
    id          TEXT PRIMARY KEY,
    session_key TEXT NOT NULL,
    squad_id    TEXT NOT NULL,
    round       INTEGER NOT NULL,
    persona_id  TEXT NOT NULL,
    persona_name TEXT NOT NULL,
    entry_type  TEXT NOT NULL DEFAULT 'observation',  -- observation, claim, question, challenge, agreement
    content     TEXT NOT NULL,
    confidence  REAL NOT NULL DEFAULT 0.5,
    references_entry_id TEXT,   -- optional: which prior entry this responds to
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_blackboard_session ON blackboard_entries(session_key, round);
CREATE INDEX IF NOT EXISTS idx_blackboard_squad ON blackboard_entries(squad_id);
```

- [ ] **Step 2: Add persona_accuracy table**

Also append:

```sql
-- ── Persona Accuracy (Phase 3: FSRS-based persona learning) ─────────
CREATE TABLE IF NOT EXISTS persona_accuracy (
    id              TEXT PRIMARY KEY,
    persona_id      TEXT NOT NULL,
    squad_id        TEXT NOT NULL,
    domain          TEXT NOT NULL DEFAULT 'general',
    total_debates   INTEGER NOT NULL DEFAULT 0,
    consensus_hits  INTEGER NOT NULL DEFAULT 0,  -- times this persona was in the consensus
    stability       REAL NOT NULL DEFAULT 1.0,   -- FSRS stability
    difficulty      REAL NOT NULL DEFAULT 5.0,   -- FSRS difficulty
    last_debate_at  TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(persona_id, squad_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_persona_accuracy_persona ON persona_accuracy(persona_id);
```

- [ ] **Step 3: Write BlackboardRepo tests**

Create `crates/cognitive/src/repos/blackboard.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn test_blackboard_crud() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = BlackboardRepo::new(pool.clone());

        let entry = NewBlackboardEntry {
            session_key: "test-session",
            squad_id: "builtin-squad-finance",
            round: 1,
            persona_id: "builtin-deep-analyst",
            persona_name: "Deep Analyst",
            entry_type: "observation",
            content: "Index funds outperform 80% of active managers over 15 years.",
            confidence: 0.95,
            references_entry_id: None,
        };
        let row = repo.insert(&entry).await.unwrap();
        assert_eq!(row.round, 1);
        assert_eq!(row.entry_type, "observation");

        let entries = repo.list_for_round("test-session", 1).await.unwrap();
        assert_eq!(entries.len(), 1);

        let all = repo.list_for_session("test-session").await.unwrap();
        assert_eq!(all.len(), 1);

        repo.clear_session("test-session").await.unwrap();
        assert!(repo.list_for_session("test-session").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_blackboard_multi_round() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = BlackboardRepo::new(pool.clone());

        // Round 1: two personas observe
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1", squad_id: "sq1", round: 1,
            persona_id: "p1", persona_name: "Analyst",
            entry_type: "observation", content: "Claim A", confidence: 0.9,
            references_entry_id: None,
        }).await.unwrap();
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1", squad_id: "sq1", round: 1,
            persona_id: "p2", persona_name: "Skeptic",
            entry_type: "challenge", content: "Challenge A", confidence: 0.7,
            references_entry_id: None,
        }).await.unwrap();

        // Round 2: response
        repo.insert(&NewBlackboardEntry {
            session_key: "debate-1", squad_id: "sq1", round: 2,
            persona_id: "p1", persona_name: "Analyst",
            entry_type: "claim", content: "Revised A with evidence", confidence: 0.85,
            references_entry_id: None,
        }).await.unwrap();

        let round1 = repo.list_for_round("debate-1", 1).await.unwrap();
        assert_eq!(round1.len(), 2);
        let round2 = repo.list_for_round("debate-1", 2).await.unwrap();
        assert_eq!(round2.len(), 1);
        let all = repo.list_for_session("debate-1").await.unwrap();
        assert_eq!(all.len(), 3);
    }
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_blackboard)'`
Expected: FAIL — types don't exist.

- [ ] **Step 5: Implement BlackboardRepo**

```rust
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlackboardEntry {
    pub id: String,
    pub session_key: String,
    pub squad_id: String,
    pub round: i64,
    pub persona_id: String,
    pub persona_name: String,
    pub entry_type: String,
    pub content: String,
    pub confidence: f64,
    pub references_entry_id: Option<String>,
    pub created_at: String,
}

pub struct NewBlackboardEntry<'a> {
    pub session_key: &'a str,
    pub squad_id: &'a str,
    pub round: i64,
    pub persona_id: &'a str,
    pub persona_name: &'a str,
    pub entry_type: &'a str,
    pub content: &'a str,
    pub confidence: f64,
    pub references_entry_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct BlackboardRepo {
    pool: SqlitePool,
}

impl BlackboardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, entry: &NewBlackboardEntry<'_>) -> Result<BlackboardEntry, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query_as::<_, BlackboardEntry>(
            "INSERT INTO blackboard_entries (id, session_key, squad_id, round, persona_id, persona_name, entry_type, content, confidence, references_entry_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             RETURNING *"
        )
        .bind(&id)
        .bind(entry.session_key)
        .bind(entry.squad_id)
        .bind(entry.round)
        .bind(entry.persona_id)
        .bind(entry.persona_name)
        .bind(entry.entry_type)
        .bind(entry.content)
        .bind(entry.confidence)
        .bind(entry.references_entry_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_for_round(&self, session_key: &str, round: i64) -> Result<Vec<BlackboardEntry>, sqlx::Error> {
        sqlx::query_as::<_, BlackboardEntry>(
            "SELECT * FROM blackboard_entries WHERE session_key = ?1 AND round = ?2 ORDER BY created_at"
        )
        .bind(session_key)
        .bind(round)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_for_session(&self, session_key: &str) -> Result<Vec<BlackboardEntry>, sqlx::Error> {
        sqlx::query_as::<_, BlackboardEntry>(
            "SELECT * FROM blackboard_entries WHERE session_key = ?1 ORDER BY round, created_at"
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await
    }

    /// Build a formatted blackboard context string for persona prompts.
    pub fn format_for_prompt(entries: &[BlackboardEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }
        let mut rounds: std::collections::BTreeMap<i64, Vec<&BlackboardEntry>> = std::collections::BTreeMap::new();
        for e in entries {
            rounds.entry(e.round).or_default().push(e);
        }
        let mut out = String::from("\n\n--- Prior Debate Rounds ---\n");
        for (round, entries) in &rounds {
            out.push_str(&format!("\n## Round {round}\n"));
            for e in entries {
                out.push_str(&format!("**{}** ({}): {}\n", e.persona_name, e.entry_type, e.content));
            }
        }
        out
    }

    pub async fn clear_session(&self, session_key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM blackboard_entries WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
```

- [ ] **Step 6: Export from mod.rs**

In `crates/cognitive/src/repos/mod.rs`, add:

```rust
pub mod blackboard;
pub use blackboard::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry};
```

Bump `FeatureMigration` version from 8 to 9 in `cognitive_migrations()` (line 65 of `repos/mod.rs`).

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(test_blackboard)'`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/blackboard.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add blackboard working memory repo + persona_accuracy table"
```

---

## Task 4: Debate AgentEvents

**Files:**
- Modify: `crates/agent/src/events.rs`
- Modify: `crates/desktop-shared/src/events.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

Add events for debate lifecycle so the frontend can render rounds in real-time.

- [ ] **Step 1: Add debate event variants to AgentEvent**

In `crates/agent/src/events.rs`, add:

```rust
    /// A debate round started.
    DebateRoundStarted {
        round: u32,
        #[serde(rename = "totalRounds")]
        total_rounds: u32,
    },

    /// A debate round completed with all persona responses.
    DebateRoundCompleted {
        round: u32,
        #[serde(rename = "consensusScore")]
        consensus_score: f64,  // 0.0 = total disagreement, 1.0 = full consensus
    },

    /// Consensus was reached — debate terminates early.
    ConsensusReached {
        round: u32,
        #[serde(rename = "consensusScore")]
        consensus_score: f64,
        summary: String,
    },

    /// A memory was promoted from one scope to a higher scope.
    MemoryPromoted {
        #[serde(rename = "factId")]
        fact_id: String,
        #[serde(rename = "fromScope")]
        from_scope: String,
        #[serde(rename = "toScope")]
        to_scope: String,
        subject: String,
        predicate: String,
    },
```

- [ ] **Step 2: Add payload types to desktop-shared**

In `crates/desktop-shared/src/events.rs`, add event name constants:

```rust
pub const AGENT_DEBATE_ROUND_STARTED: &str = "agent:debate_round_started";
pub const AGENT_DEBATE_ROUND_COMPLETED: &str = "agent:debate_round_completed";
pub const AGENT_CONSENSUS_REACHED: &str = "agent:consensus_reached";
pub const AGENT_MEMORY_PROMOTED: &str = "agent:memory_promoted";
```

Add payload structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebateRoundStartedPayload {
    pub session_key: String,
    pub round: u32,
    pub total_rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebateRoundCompletedPayload {
    pub session_key: String,
    pub round: u32,
    pub consensus_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusReachedPayload {
    pub session_key: String,
    pub round: u32,
    pub consensus_score: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPromotedPayload {
    pub session_key: String,
    pub fact_id: String,
    pub from_scope: String,
    pub to_scope: String,
    pub subject: String,
    pub predicate: String,
}
```

- [ ] **Step 3: Handle new events in streaming relay**

In `crates/app-core/src/handlers/chat/streaming.rs`, add match arms for the new `AgentEvent` variants in the event relay loop (where `PersonaPerspective` is handled):

```rust
AgentEvent::DebateRoundStarted { round, total_rounds } => {
    emit!(events::AGENT_DEBATE_ROUND_STARTED, events::DebateRoundStartedPayload {
        session_key: sk.to_string(), round, total_rounds,
    });
}
AgentEvent::DebateRoundCompleted { round, consensus_score } => {
    emit!(events::AGENT_DEBATE_ROUND_COMPLETED, events::DebateRoundCompletedPayload {
        session_key: sk.to_string(), round, consensus_score,
    });
}
AgentEvent::ConsensusReached { round, consensus_score, summary } => {
    emit!(events::AGENT_CONSENSUS_REACHED, events::ConsensusReachedPayload {
        session_key: sk.to_string(), round, consensus_score, summary,
    });
}
AgentEvent::MemoryPromoted { fact_id, from_scope, to_scope, subject, predicate } => {
    emit!(events::AGENT_MEMORY_PROMOTED, events::MemoryPromotedPayload {
        session_key: sk.to_string(), fact_id, from_scope, to_scope, subject, predicate,
    });
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p app-core -p desktop-shared -p agent 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/events.rs crates/desktop-shared/src/events.rs crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(agent): add debate lifecycle events — round started/completed, consensus, memory promoted"
```

---

## Task 5: DebateOrchestrator — Multi-Round Debate with Consensus Detection

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/debate.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`

This is the core engine. It runs N rounds of debate, each round giving personas the blackboard context from prior rounds. After each round, it checks for consensus and terminates early if reached.

- [ ] **Step 1: Write tests for consensus detection**

Create `crates/agent/src/intent_pipeline/engines/debate.rs` with test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_score_full_agreement() {
        let responses = vec![
            ("Analyst".into(), "Index funds are better for long-term investing.".into()),
            ("Skeptic".into(), "I agree, index funds provide the best risk-adjusted returns.".into()),
            ("Strategist".into(), "Index funds are the optimal choice for most investors.".into()),
        ];
        let score = estimate_consensus(&responses);
        // All agree on index funds → high consensus
        assert!(score > 0.6, "Expected high consensus, got {score}");
    }

    #[test]
    fn test_consensus_score_disagreement() {
        let responses = vec![
            ("Analyst".into(), "Cryptocurrency mining rigs generate passive blockchain revenue through proof-of-work algorithms.".into()),
            ("Skeptic".into(), "Municipal government bonds provide guaranteed coupon payments backed by taxing authority.".into()),
            ("Strategist".into(), "Agricultural farmland produces rental income while appreciating through topsoil development.".into()),
        ];
        let score = estimate_consensus(&responses);
        // Completely different domains → low consensus
        assert!(score < 0.4, "Expected low consensus, got {score}");
    }

    #[test]
    fn test_build_debate_prompt_includes_blackboard() {
        let blackboard = vec![
            BlackboardEntry {
                id: "1".into(), session_key: "s".into(), squad_id: "sq".into(),
                round: 1, persona_id: "p1".into(), persona_name: "Analyst".into(),
                entry_type: "observation".into(), content: "Index funds beat 80% of managers.".into(),
                confidence: 0.9, references_entry_id: None, created_at: "now".into(),
            },
        ];
        let prompt = build_debate_round_prompt(
            "System context",
            "Should I invest in index funds?",
            "Skeptic",
            "Critical analyst",
            "Questions claims",
            "direct",
            &blackboard,
            2,
        );
        assert!(prompt.contains("Prior Debate Rounds"));
        assert!(prompt.contains("Analyst"));
        assert!(prompt.contains("Index funds beat 80%"));
        assert!(prompt.contains("Round 2"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_consensus)'`
Expected: FAIL — functions don't exist.

- [ ] **Step 3: Implement DebateOrchestrator**

```rust
//! DebateOrchestrator — multi-round persona debate with consensus detection.
//!
//! Flow:
//! 1. Round 1: parallel persona fan-out (same as squad.rs)
//! 2. Write persona outputs to blackboard
//! 3. Estimate consensus score
//! 4. If consensus < threshold AND round < max_rounds: goto step 1 with blackboard context
//! 5. Final synthesis incorporating all rounds

use cognitive::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry, PersonaRow};
use providers::{ChatParams, DynProvider, Message, UserContent};

use super::squad;

/// Default consensus threshold — debate stops when exceeded.
pub const DEFAULT_CONSENSUS_THRESHOLD: f64 = 0.75;
/// Default maximum debate rounds.
pub const DEFAULT_MAX_ROUNDS: u32 = 3;

/// Estimate consensus from persona responses using word-overlap heuristic.
///
/// Computes pairwise Jaccard similarity of response word sets, averaged.
/// Returns 0.0 (no overlap) to 1.0 (identical).
pub fn estimate_consensus(responses: &[(String, String)]) -> f64 {
    if responses.len() < 2 {
        return 1.0;
    }
    let word_sets: Vec<std::collections::HashSet<&str>> = responses
        .iter()
        .map(|(_, content)| {
            content
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                .filter(|w| w.len() > 3) // skip short words
                .collect()
        })
        .collect();

    let mut total_sim = 0.0;
    let mut pairs = 0;
    for i in 0..word_sets.len() {
        for j in (i + 1)..word_sets.len() {
            let intersection = word_sets[i].intersection(&word_sets[j]).count();
            let union = word_sets[i].union(&word_sets[j]).count();
            if union > 0 {
                total_sim += intersection as f64 / union as f64;
            }
            pairs += 1;
        }
    }
    if pairs == 0 { 1.0 } else { total_sim / pairs as f64 }
}

/// Build a debate-round persona prompt that includes blackboard context.
pub fn build_debate_round_prompt(
    orchestrator_context: &str,
    user_message: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
    blackboard: &[BlackboardEntry],
    current_round: u32,
) -> String {
    let blackboard_context = BlackboardRepo::format_for_prompt(blackboard);
    format!(
        r#"{orchestrator_context}
{blackboard_context}

---

You are now responding as **{persona_name}**, a {persona_role}.
Your perspective: {persona_perspective}
Your tone should be: {persona_tone}

This is **Round {current_round}** of a multi-round debate. You can see what other personas said in prior rounds above.

Rules for this round:
- Reference specific points from other personas' prior contributions
- If you agree with someone, say so explicitly and build on their point
- If you disagree, explain why with evidence
- If your position has changed due to others' arguments, acknowledge the shift
- Be direct and specific. Avoid generic statements.

Respond to: {user_message}"#
    )
}

/// Run a full multi-round debate.
///
/// Returns: (all_round_responses, final_consensus_score, final_round)
pub async fn run_debate(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    max_rounds: u32,
    consensus_threshold: f64,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(u32, Vec<(String, String)>, f64)> {
    let mut all_rounds: Vec<(u32, Vec<(String, String)>, f64)> = Vec::new();

    for round in 1..=max_rounds {
        // Emit round started
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::DebateRoundStarted {
                round,
                total_rounds: max_rounds,
            }).await;
        }

        // Load blackboard from prior rounds
        let blackboard = blackboard_repo
            .list_for_session(session_key)
            .await
            .unwrap_or_default();

        // Fan out to all personas with debate-aware prompts
        let futures: Vec<_> = personas
            .iter()
            .map(|persona| {
                let provider = provider.clone();
                let params = params.clone();
                let system = build_debate_round_prompt(
                    orchestrator_context,
                    user_message,
                    &persona.name,
                    &persona.role,
                    &persona.perspective,
                    &persona.tone,
                    &blackboard,
                    round,
                );
                let user_msg = user_message.to_string();
                let persona_name = persona.name.clone();
                let persona_id = persona.id.clone();
                let persona_icon = persona.icon.clone();
                let persona_role = persona.role.clone();
                let tx = event_tx.cloned();

                async move {
                    let messages = vec![
                        Message::System { content: system },
                        Message::User { content: UserContent::Text(user_msg) },
                    ];
                    let result = provider.chat(&messages, None, &params).await;
                    let text = match result {
                        Ok(r) => r.content.unwrap_or_default(),
                        Err(e) => {
                            tracing::warn!(persona = %persona_name, round, "Debate LLM call failed: {e}");
                            String::new()
                        }
                    };

                    if let Some(tx) = &tx {
                        let _ = tx.send(crate::AgentEvent::PersonaPerspective {
                            persona_id: persona_id.clone(),
                            persona_name: persona_name.clone(),
                            persona_icon: persona_icon.clone(),
                            persona_role: persona_role.clone(),
                            content: text.clone(),
                        }).await;
                    }

                    (persona_id, persona_name, text)
                }
            })
            .collect();

        let round_results = futures_util::future::join_all(futures).await;

        // Write to blackboard
        for (pid, pname, content) in &round_results {
            if !content.is_empty() {
                let _ = blackboard_repo.insert(&NewBlackboardEntry {
                    session_key,
                    squad_id,
                    round: round as i64,
                    persona_id: pid,
                    persona_name: pname,
                    entry_type: if round == 1 { "observation" } else { "response" },
                    content,
                    confidence: 0.8, // TODO: extract from LLM response
                    references_entry_id: None,
                }).await;
            }
        }

        // Build (name, content) pairs for consensus check
        let responses: Vec<(String, String)> = round_results
            .into_iter()
            .map(|(_, name, content)| (name, content))
            .collect();

        let consensus = estimate_consensus(&responses);

        // Emit round completed
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::DebateRoundCompleted {
                round,
                consensus_score: consensus,
            }).await;
        }

        all_rounds.push((round, responses, consensus));

        // Check for early termination
        if consensus >= consensus_threshold {
            if let Some(tx) = event_tx {
                let _ = tx.send(crate::AgentEvent::ConsensusReached {
                    round,
                    consensus_score: consensus,
                    summary: format!("Consensus reached after {round} rounds (score: {consensus:.2})"),
                }).await;
            }
            break;
        }
    }

    all_rounds
}
```

- [ ] **Step 4: Export module**

In `crates/agent/src/intent_pipeline/engines/mod.rs`, add:

```rust
pub mod debate;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_consensus)' && cargo nextest run -p agent -E 'test(test_build_debate)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/debate.rs crates/agent/src/intent_pipeline/engines/mod.rs
git commit -m "feat(agent): add DebateOrchestrator with multi-round debate, blackboard context, and consensus detection"
```

---

## Task 6: Integrate DebateOrchestrator into AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:602-696`

Add debate detection: when a squad chat has `debate_enabled` (configurable via session metadata or squad config), use `DebateOrchestrator` instead of single-pass `SquadExecutor`.

- [ ] **Step 1: Add debate config to SquadDeps**

In `runtime.rs`, extend `SquadDeps`:

```rust
pub(crate) struct SquadDeps {
    pub repo: cognitive::SquadRepo,
    pub provider: providers::DynProvider,
    pub chat_params: providers::ChatParams,
    pub blackboard_repo: Option<cognitive::BlackboardRepo>,
}
```

- [ ] **Step 2: Inject BlackboardRepo in builder**

In `crates/agent/src/agent_loop/builder.rs`, update the `SquadDeps` construction to include `BlackboardRepo`:

```rust
let blackboard_repo = pool.as_ref().map(|p| cognitive::BlackboardRepo::new(p.clone()));
// Pass to SquadDeps
```

- [ ] **Step 3: Update `with_squad_deps()` signature**

The `with_squad_deps()` method in `runtime.rs` (line ~165) must also accept the new `blackboard_repo` parameter:

```rust
pub fn with_squad_deps(
    mut self,
    repo: cognitive::SquadRepo,
    provider: providers::DynProvider,
    chat_params: providers::ChatParams,
    blackboard_repo: Option<cognitive::BlackboardRepo>,
) -> Self {
    self.squad_deps = Some(SquadDeps { repo, provider, chat_params, blackboard_repo });
    self
}
```

Update the builder callsite in `builder.rs` accordingly.

- [ ] **Step 4: Add debate path in run_squad_execution**

In `run_squad_execution()`, after resolving the squad, check if debate mode is enabled:

```rust
// Check if debate mode is requested (squad has 3+ personas — debate is useful)
let use_debate = resolved.personas.len() >= 3
    && deps.blackboard_repo.is_some();

if use_debate {
    let blackboard_repo = deps.blackboard_repo.as_ref().unwrap();
    let debate_session_key = format!("debate:{}:{}", squad_id, uuid::Uuid::new_v4());
    let debate_results = debate::run_debate(
        provider,
        &orchestrator_context,
        message,
        &resolved.personas,
        params,
        blackboard_repo,
        &debate_session_key,
        squad_id,
        debate::DEFAULT_MAX_ROUNDS,
        debate::DEFAULT_CONSENSUS_THRESHOLD,
        event_tx.as_ref(),
    ).await;

    // Post-debate: promote high-confidence blackboard entries to squad memory
    let fact_repo = cognitive::SemanticFactRepo::new(deps.repo.pool().clone());
    let all_entries = blackboard_repo.list_for_session(&debate_session_key).await.unwrap_or_default();
    let promoted = cognitive::services::memory_promotion::promote_from_blackboard(
        &fact_repo, &all_entries, squad_id, 0.85,
    ).await;

    // Emit MemoryPromoted events for each promoted fact
    if let Some(tx) = event_tx.as_ref() {
        for fact in &promoted {
            let _ = tx.send(crate::AgentEvent::MemoryPromoted {
                fact_id: fact.id.clone(),
                from_scope: "blackboard".into(),
                to_scope: "squad".into(),
                subject: fact.subject.clone(),
                predicate: fact.predicate.clone(),
            }).await;
        }
    }

    // Collect all responses from the final round for synthesis
    let final_round = debate_results.last().map(|(_, responses, _)| responses.clone()).unwrap_or_default();
    let persona_responses = final_round;

    // Synthesis (same as single-pass)
    let synthesis_prompt = squad::build_squad_synthesis_prompt(message, &persona_responses);
    // ... rest of synthesis unchanged ...
} else {
    // Existing single-pass fan-out (unchanged)
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p agent 2>&1 | tail -10`

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): integrate DebateOrchestrator into AgentRuntime with memory promotion"
```

---

## Task 7: Memory Promotion Pipeline

**Files:**
- Create: `crates/cognitive/src/services/memory_promotion.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

After a debate, high-confidence observations should be promoted: persona→squad→global.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn test_promote_persona_to_squad() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());

        // Insert a persona-scoped fact
        let fact = SemanticFact {
            id: "persona-fact-1".into(),
            domain: "finance".into(),
            subject: "index funds".into(),
            predicate: "risk_level".into(),
            object: "low".into(),
            confidence: 0.95,
            source: "debate".into(),
            valid_from: chrono::Utc::now().to_rfc3339(),
            valid_until: None,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            superseded_at: None,
            superseded_by: None,
            stability: 2.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: "observation".into(),
            scope_type: "persona".into(),
            scope_id: Some("builtin-deep-analyst".into()),
        };
        fact_repo.upsert(&fact).await.unwrap();

        // Promote to squad scope
        let promoted = promote_fact(&fact_repo, "persona-fact-1", "squad", Some("builtin-squad-finance")).await.unwrap();
        assert!(promoted.is_some());
        let p = promoted.unwrap();
        assert_eq!(p.scope_type, "squad");
        assert_eq!(p.scope_id, Some("builtin-squad-finance".into()));
        assert_ne!(p.id, "persona-fact-1"); // New ID
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_promote)'`

- [ ] **Step 3: Implement promotion**

```rust
use crate::repos::SemanticFactRepo;
use crate::types::SemanticFact;
use uuid::Uuid;

/// Promote a fact from one scope to a higher scope.
///
/// Creates a new fact in the target scope with a new ID, preserving the content.
/// The original fact is kept (not superseded) — both scopes retain the knowledge.
pub async fn promote_fact(
    repo: &SemanticFactRepo,
    fact_id: &str,
    target_scope_type: &str,
    target_scope_id: Option<&str>,
) -> Result<Option<SemanticFact>, sqlx::Error> {
    let original = repo.get(fact_id).await?;
    let Some(original) = original else {
        return Ok(None);
    };

    let promoted = SemanticFact {
        id: Uuid::new_v4().to_string(),
        scope_type: target_scope_type.to_string(),
        scope_id: target_scope_id.map(|s| s.to_string()),
        source: format!("promoted:{}", original.source),
        recorded_at: chrono::Utc::now().to_rfc3339(),
        ..original
    };

    repo.upsert(&promoted).await?;
    Ok(Some(promoted))
}

/// Promote high-confidence blackboard entries to squad-scoped semantic facts.
///
/// Entries with confidence ≥ threshold and entry_type "observation" or "claim"
/// are converted to semantic facts in squad scope.
/// Promote high-confidence blackboard entries to squad-scoped semantic facts.
///
/// Returns promoted facts. Event emission is the caller's responsibility (agent crate).
pub async fn promote_from_blackboard(
    fact_repo: &SemanticFactRepo,
    entries: &[crate::repos::BlackboardEntry],
    squad_id: &str,
    confidence_threshold: f64,
) -> Vec<SemanticFact> {
    let mut promoted = Vec::new();
    for entry in entries {
        if entry.confidence < confidence_threshold {
            continue;
        }
        if !matches!(entry.entry_type.as_str(), "observation" | "claim" | "agreement") {
            continue;
        }

        let fact = SemanticFact {
            id: Uuid::new_v4().to_string(),
            domain: "debate".into(),
            subject: entry.persona_name.clone(),
            predicate: "observed".into(),
            object: entry.content.clone(),
            confidence: entry.confidence,
            source: format!("debate:{}", entry.session_key),
            valid_from: chrono::Utc::now().to_rfc3339(),
            valid_until: None,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: "squad_knowledge".into(),
            scope_type: "squad".into(),
            scope_id: Some(squad_id.to_string()),
        };

        if fact_repo.upsert(&fact).await.is_ok() {
            promoted.push(fact);
        }
    }
    promoted
}
```

- [ ] **Step 4: Export from services/mod.rs**

```rust
pub mod memory_promotion;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(test_promote)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/memory_promotion.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): add memory promotion pipeline — persona→squad→global scope elevation"
```

---

## Task 8: FSRS Persona Learning

**Files:**
- Create: `crates/cognitive/src/repos/persona_accuracy.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

Track how often each persona's observations end up in the consensus. Use FSRS to model per-persona reliability over time.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn test_record_debate_outcome() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = PersonaAccuracyRepo::new(pool.clone());

        // Record a successful debate
        repo.record_outcome("builtin-deep-analyst", "builtin-squad-finance", "finance", true).await.unwrap();

        let acc = repo.get("builtin-deep-analyst", "builtin-squad-finance", "finance").await.unwrap().unwrap();
        assert_eq!(acc.total_debates, 1);
        assert_eq!(acc.consensus_hits, 1);
        assert!(acc.stability > 1.0); // FSRS increased stability

        // Record a miss
        repo.record_outcome("builtin-deep-analyst", "builtin-squad-finance", "finance", false).await.unwrap();
        let acc = repo.get("builtin-deep-analyst", "builtin-squad-finance", "finance").await.unwrap().unwrap();
        assert_eq!(acc.total_debates, 2);
        assert_eq!(acc.consensus_hits, 1);
    }

    #[test]
    fn test_accuracy_rate() {
        let acc = PersonaAccuracy {
            id: "1".into(), persona_id: "p".into(), squad_id: "s".into(),
            domain: "d".into(), total_debates: 10, consensus_hits: 7,
            stability: 2.0, difficulty: 5.0, last_debate_at: None,
            created_at: String::new(), updated_at: String::new(),
        };
        assert!((acc.accuracy_rate() - 0.7).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_record_debate)'`

- [ ] **Step 3: Implement PersonaAccuracyRepo**

```rust
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::services::fsrs5;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonaAccuracy {
    pub id: String,
    pub persona_id: String,
    pub squad_id: String,
    pub domain: String,
    pub total_debates: i64,
    pub consensus_hits: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub last_debate_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl PersonaAccuracy {
    pub fn accuracy_rate(&self) -> f64 {
        if self.total_debates == 0 { 0.0 } else { self.consensus_hits as f64 / self.total_debates as f64 }
    }
}

#[derive(Debug, Clone)]
pub struct PersonaAccuracyRepo {
    pool: SqlitePool,
}

impl PersonaAccuracyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(
        &self,
        persona_id: &str,
        squad_id: &str,
        domain: &str,
    ) -> Result<Option<PersonaAccuracy>, sqlx::Error> {
        sqlx::query_as::<_, PersonaAccuracy>(
            "SELECT * FROM persona_accuracy WHERE persona_id = ?1 AND squad_id = ?2 AND domain = ?3"
        )
        .bind(persona_id)
        .bind(squad_id)
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
    }

    /// Record the outcome of a debate for a persona.
    /// `in_consensus` = true if this persona's final position aligned with the group consensus.
    pub async fn record_outcome(
        &self,
        persona_id: &str,
        squad_id: &str,
        domain: &str,
        in_consensus: bool,
    ) -> Result<PersonaAccuracy, sqlx::Error> {
        let existing = self.get(persona_id, squad_id, domain).await?;

        let (total, hits, old_stability, old_difficulty) = match &existing {
            Some(a) => (a.total_debates, a.consensus_hits, a.stability, a.difficulty),
            None => (0, 0, 1.0, 5.0),
        };

        let new_total = total + 1;
        let new_hits = if in_consensus { hits + 1 } else { hits };

        // FSRS update: treat consensus hit as "Good" (3), miss as "Again" (1)
        let rating = if in_consensus { 3 } else { 1 };
        let w = fsrs5::DEFAULT_WEIGHTS;
        let new_stability = if total == 0 {
            fsrs5::initial_stability(rating, &w)
        } else {
            let elapsed = 1.0; // simplified: 1 day equivalent per debate
            let r = fsrs5::retrievability(elapsed, old_stability);
            if in_consensus {
                fsrs5::next_stability_success(old_stability, old_difficulty, r, rating, &w)
            } else {
                fsrs5::next_stability_failure(old_stability, old_difficulty, r, &w)
            }
        };
        let new_difficulty = if total == 0 {
            fsrs5::initial_difficulty(rating, &w)
        } else {
            fsrs5::next_difficulty(old_difficulty, rating, &w)
        };

        let id = existing.map(|a| a.id).unwrap_or_else(|| Uuid::new_v4().to_string());

        sqlx::query_as::<_, PersonaAccuracy>(
            "INSERT INTO persona_accuracy (id, persona_id, squad_id, domain, total_debates, consensus_hits, stability, difficulty, last_debate_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))
             ON CONFLICT (persona_id, squad_id, domain) DO UPDATE SET
               total_debates = ?5, consensus_hits = ?6, stability = ?7, difficulty = ?8,
               last_debate_at = datetime('now'), updated_at = datetime('now')
             RETURNING *"
        )
        .bind(&id)
        .bind(persona_id)
        .bind(squad_id)
        .bind(domain)
        .bind(new_total)
        .bind(new_hits)
        .bind(new_stability)
        .bind(new_difficulty)
        .fetch_one(&self.pool)
        .await
    }

    /// List accuracy records for a persona across all squads.
    pub async fn list_for_persona(&self, persona_id: &str) -> Result<Vec<PersonaAccuracy>, sqlx::Error> {
        sqlx::query_as::<_, PersonaAccuracy>(
            "SELECT * FROM persona_accuracy WHERE persona_id = ?1 ORDER BY updated_at DESC"
        )
        .bind(persona_id)
        .fetch_all(&self.pool)
        .await
    }
}
```

- [ ] **Step 4: Export from repos/mod.rs**

In `crates/cognitive/src/repos/mod.rs`, add:

```rust
pub mod persona_accuracy;
pub use persona_accuracy::{PersonaAccuracy, PersonaAccuracyRepo};
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(test_record_debate)' && cargo nextest run -p cognitive -E 'test(test_accuracy)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/persona_accuracy.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add FSRS-based persona accuracy tracking from debate outcomes"
```

---

## Task 9: Frontend — Debate Types + Store

**Files:**
- Modify: `desktop-ui/src/shared/types/chat.ts`
- Modify: `desktop-ui/src/shared/stores/chatStreamStore.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useAgentStream.ts`

- [ ] **Step 1: Add debate types**

In `desktop-ui/src/shared/types/chat.ts`:

```typescript
// ── Debate Events ──────────────────────────────────────────
export interface DebateRoundStartedPayload {
  sessionKey: string;
  round: number;
  totalRounds: number;
}

export interface DebateRoundCompletedPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
}

export interface ConsensusReachedPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
  summary: string;
}

export interface MemoryPromotedPayload {
  sessionKey: string;
  factId: string;
  fromScope: string;
  toScope: string;
  subject: string;
  predicate: string;
}

export interface DebateRound {
  round: number;
  personaMessages: PersonaSegment[];
  consensusScore: number | null;
}
```

- [ ] **Step 2: Add debate state to chatStreamStore**

In `StreamSnapshot` interface, add:

```typescript
debateRounds: DebateRound[];
currentDebateRound: number | null;
consensusReached: boolean;
consensusSummary: string | null;
```

In `DEFAULT_SNAPSHOT`, add:

```typescript
debateRounds: [],
currentDebateRound: null,
consensusReached: false,
consensusSummary: null,
```

Add event handlers:

```typescript
private onDebateRoundStarted(payload: DebateRoundStartedPayload): void {
  if (!this.isActive(payload.sessionKey)) return;
  this.updateState(payload.sessionKey, (s) => ({
    ...s,
    currentDebateRound: payload.round,
    debateRounds: [
      ...s.debateRounds,
      { round: payload.round, personaMessages: [], consensusScore: null },
    ],
  }));
}

private onDebateRoundCompleted(payload: DebateRoundCompletedPayload): void {
  if (!this.isActive(payload.sessionKey)) return;
  this.updateState(payload.sessionKey, (s) => ({
    ...s,
    debateRounds: s.debateRounds.map((r) =>
      r.round === payload.round ? { ...r, consensusScore: payload.consensusScore } : r,
    ),
  }));
}

private onConsensusReached(payload: ConsensusReachedPayload): void {
  if (!this.isActive(payload.sessionKey)) return;
  this.updateState(payload.sessionKey, (s) => ({
    ...s,
    consensusReached: true,
    consensusSummary: payload.summary,
  }));
}
```

Update `onPersonaPerspective` to also append to the current debate round's `personaMessages` when `currentDebateRound` is set.

Register the new event handlers in both Tauri and browser modes (same pattern as existing handlers).

- [ ] **Step 3: Expose debate data from useAgentStream**

In `useAgentStream.ts`, add to return type:

```typescript
debateRounds: state.debateRounds,
consensusReached: state.consensusReached,
consensusSummary: state.consensusSummary,
```

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/types/chat.ts desktop-ui/src/shared/stores/chatStreamStore.ts desktop-ui/src/features/chat/hooks/useAgentStream.ts
git commit -m "feat(ui): add debate round types, store state, and stream exposure"
```

---

## Task 10: Frontend — DebateView Components

**Files:**
- Create: `desktop-ui/src/features/chat/components/DebateRound.tsx`
- Create: `desktop-ui/src/features/chat/components/ConsensusIndicator.tsx`
- Create: `desktop-ui/src/features/chat/components/DebateView.tsx`
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`

- [ ] **Step 1: Create ConsensusIndicator**

A small visual bar showing consensus level:

```tsx
interface ConsensusIndicatorProps {
  score: number | null;  // 0.0 - 1.0
  reached: boolean;
}

export function ConsensusIndicator({ score, reached }: ConsensusIndicatorProps) {
  if (score === null) return null;
  const percent = Math.round(score * 100);
  const color = reached ? "bg-green-500" : score > 0.6 ? "bg-yellow-500" : "bg-red-400";
  return (
    <div className="flex items-center gap-2 text-[10px] text-dim">
      <span>Consensus</span>
      <div className="w-16 h-1.5 bg-white/[0.06] rounded-full overflow-hidden">
        <div className={`h-full ${color} rounded-full transition-all`} style={{ width: `${percent}%` }} />
      </div>
      <span>{percent}%</span>
      {reached && <span className="text-green-400">Reached</span>}
    </div>
  );
}
```

- [ ] **Step 2: Create DebateRound**

Renders a single round with its persona messages and consensus indicator:

```tsx
import type { DebateRound as DebateRoundType } from "@shared/types";
import { PersonaMessageList } from "./PersonaMessageList";
import { ConsensusIndicator } from "./ConsensusIndicator";

interface DebateRoundProps {
  round: DebateRoundType;
  isCurrentRound: boolean;
  isConsensusRound: boolean;
}

export function DebateRound({ round, isCurrentRound, isConsensusRound }: DebateRoundProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-medium text-muted-foreground">
          Round {round.round}
          {isCurrentRound && <span className="ml-1 text-purple-400 animate-pulse">Active</span>}
        </span>
        <ConsensusIndicator score={round.consensusScore} reached={isConsensusRound} />
      </div>
      <PersonaMessageList personaMessages={round.personaMessages} />
    </div>
  );
}
```

- [ ] **Step 3: Create DebateView**

Container that renders all debate rounds:

```tsx
import type { DebateRound as DebateRoundType } from "@shared/types";
import { DebateRound } from "./DebateRound";
import { ConsensusIndicator } from "./ConsensusIndicator";

interface DebateViewProps {
  rounds: DebateRoundType[];
  currentRound: number | null;
  consensusReached: boolean;
  consensusSummary: string | null;
}

export function DebateView({ rounds, currentRound, consensusReached, consensusSummary }: DebateViewProps) {
  if (rounds.length === 0) return null;
  return (
    <div className="space-y-4 border border-border/30 rounded-xl p-4 bg-white/[0.02]">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          Debate
        </span>
        {consensusReached && (
          <ConsensusIndicator score={rounds.at(-1)?.consensusScore ?? null} reached />
        )}
      </div>
      {rounds.map((round, i) => (
        <DebateRound
          key={round.round}
          round={round}
          isCurrentRound={round.round === currentRound}
          isConsensusRound={consensusReached && i === rounds.length - 1}
        />
      ))}
      {consensusSummary && (
        <div className="text-[11px] text-green-400/80 italic border-t border-border/20 pt-2">
          {consensusSummary}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Integrate into ChatPage**

In `ChatPage.tsx`, render `DebateView` when debate rounds are present (between user message and synthesized response):

```tsx
{chat.debateRounds.length > 0 && (
  <DebateView
    rounds={chat.debateRounds}
    currentRound={chat.currentDebateRound}
    consensusReached={chat.consensusReached}
    consensusSummary={chat.consensusSummary}
  />
)}
```

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/chat/components/DebateRound.tsx desktop-ui/src/features/chat/components/ConsensusIndicator.tsx desktop-ui/src/features/chat/components/DebateView.tsx desktop-ui/src/features/chat/pages/ChatPage.tsx
git commit -m "feat(ui): add DebateView with round visualization, consensus indicators"
```

---

## Task 11: Integration Test + Cleanup

**Files:**
- All modified files from Tasks 1-10

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 2: Run full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: All tests pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: No new warnings.

- [ ] **Step 4: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

- [ ] **Step 5: Smoke test**

Run: `cd desktop-ui && bun run dev` (+ `cargo tauri dev`)
Test flow:
1. Open Chat → New Squad Chat → select Finance Analysis (3 members)
2. Send: "Should I invest in real estate or stocks for retirement?"
3. Verify: Debate rounds appear (up to 3 rounds)
4. Verify: PersonaPerspective events arrive per round with persona icon/role
5. Verify: Consensus indicator updates after each round
6. Verify: Debate terminates early if consensus reached
7. Verify: Final synthesized response integrates all debate rounds
8. Switch to Merged mode → only synthesis shown

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(squads): Phase 3 complete — multi-agent debate, blackboard memory, consensus detection, FSRS learning"
```
