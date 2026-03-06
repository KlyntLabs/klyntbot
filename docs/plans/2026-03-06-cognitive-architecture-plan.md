# Cognitive Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a unified cognitive memory system and proactive intelligence engine that transforms the AI from reactive analytics to a continuously learning, cross-domain personal AI assistant.

**Architecture:** Three layers built bottom-up: (1) DomainEvent bus for cross-feature communication, (2) `cognitive` crate at L3 for unified memory with FSRS decay, bi-temporal facts, Mem0-style consolidation, and weekly reflection, (3) `feature-coaching` crate at L4 for proactive intelligence with signal accumulation, UserSituation world model, LLM-powered coaching, and closed-loop feedback.

**Tech Stack:** Rust, SQLite (sqlx), LanceDB, fastembed (384-dim), tokio broadcast channels, FSRS decay model

**Design doc:** `docs/plans/2026-03-06-cognitive-architecture-design.md`

---

## Phase 1: DomainEvent Bus (Layer C)

Foundation — all features emit events, cognitive layer subscribes. No LLM calls yet.

### Task 1.1: DomainEvent types

**Files:**
- Create: `crates/bus/src/domain_events.rs`
- Modify: `crates/bus/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests`

**Step 1: Write the failing test**

```rust
// In crates/bus/src/domain_events.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_domain_event_bus_publish_subscribe() {
        let bus = DomainEventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(DomainEvent::ProductivityScoreComputed {
            date: "2026-03-06".into(),
            score: 74.0,
        });

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, DomainEvent::ProductivityScoreComputed { score, .. } if score == 74.0));
    }

    #[tokio::test]
    async fn test_domain_event_bus_multiple_subscribers() {
        let bus = DomainEventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(30),
            estimated_duration_mins: Some(45),
        });

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_domain_event_serialization() {
        let event = DomainEvent::UserStatedFact {
            fact: "I prefer morning work".into(),
            domain: "productivity".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, DomainEvent::UserStatedFact { .. }));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p bus --test-threads=1 -E 'test(domain_event)'`
Expected: FAIL — module does not exist

**Step 3: Write minimal implementation**

```rust
// crates/bus/src/domain_events.rs
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted by feature crates for cross-domain communication.
///
/// The cognitive layer subscribes to all events. Feature crates emit
/// via `DomainEventBus::publish()` without knowing about consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    // -- Productivity --
    ActivitySessionCompleted {
        date: String,
        total_active_secs: i64,
        productive_secs: i64,
        distracting_secs: i64,
    },
    FocusSessionEnded {
        duration_secs: i64,
        quality: f64,
        interruptions: i32,
    },
    DistractionDetected {
        app: String,
        duration_secs: i64,
        context: String,
    },
    ProductivityScoreComputed {
        date: String,
        score: f64,
    },

    // -- Tasks --
    TaskCreated {
        task_id: String,
        project: Option<String>,
        estimate_mins: Option<i64>,
    },
    TaskCompleted {
        task_id: String,
        actual_duration_mins: Option<i64>,
        estimated_duration_mins: Option<i64>,
    },
    TaskDeferred {
        task_id: String,
        times_deferred: i32,
    },
    GoalProgress {
        objective_id: String,
        progress: f64,
        target: f64,
    },

    // -- Finance --
    TransactionRecorded {
        category: String,
        amount: f64,
        is_over_budget: bool,
    },
    BudgetAlert {
        category: String,
        spent: f64,
        limit: f64,
    },

    // -- Cross-domain --
    UserStatedFact {
        fact: String,
        domain: String,
    },
    UserCorrectedAI {
        original: String,
        correction: String,
    },

    // -- Coaching feedback --
    CoachingFeedback {
        intervention_id: String,
        response: FeedbackResponse,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackResponse {
    Helpful,
    Dismissed,
    StopSuggesting,
}

/// Broadcast bus for DomainEvents. Clone-safe via Arc internally.
pub struct DomainEventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl DomainEventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }
}
```

Update `crates/bus/src/lib.rs`:
```rust
pub mod domain_events;
pub use domain_events::{DomainEvent, DomainEventBus, FeedbackResponse};
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p bus -E 'test(domain_event)'`
Expected: PASS (3 tests)

**Step 5: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/bus/src/lib.rs
git commit -m "feat(bus): add DomainEvent types and broadcast bus for cross-feature communication"
```

---

### Task 1.2: Emit DomainEvents from feature-productivity

**Files:**
- Modify: `crates/feature-productivity/Cargo.toml` (add `bus` dependency)
- Modify: `crates/feature-productivity/src/engine.rs` (inject DomainEventBus)
- Modify: `crates/feature-productivity/src/aggregator.rs` (emit ProductivityScoreComputed)
- Test: inline tests in engine.rs

**Step 1: Write failing test**

```rust
// In crates/feature-productivity/src/engine.rs tests
#[tokio::test]
async fn test_engine_emits_domain_events() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    // ... setup engine with bus injected ...
    // Trigger a daily score computation
    // Assert: rx.recv() returns ProductivityScoreComputed
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(domain_event)'`
Expected: FAIL — no DomainEventBus on ProductivityEngine

**Step 3: Implement**

Add `bus` to `crates/feature-productivity/Cargo.toml` dependencies. Add `Option<Arc<DomainEventBus>>` field to `ProductivityEngine` with a builder method `with_domain_bus()`. In `DailyAggregator::compute_today()`, after computing the score, emit:

```rust
if let Some(bus) = &self.domain_bus {
    bus.publish(DomainEvent::ProductivityScoreComputed {
        date: date.to_string(),
        score,
    });
}
```

Similarly emit `FocusSessionEnded` from the focus session completion path and `DistractionDetected` from the distraction analyzer.

**Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(domain_event)'`
Expected: PASS

**Step 5: Commit**

```bash
git commit -m "feat(productivity): emit DomainEvents from engine and aggregator"
```

---

### Task 1.3: Emit DomainEvents from feature-todo

**Files:**
- Modify: `crates/feature-todo/Cargo.toml`
- Modify: `crates/feature-todo/src/handler.rs` or `tool/actions/add.rs`, `tool/actions/update.rs`, `tool/actions/delete.rs`
- Test: inline

**Step 1-5:** Same pattern as 1.2. Emit `TaskCreated` from add action, `TaskCompleted` from update action (when status changes to done), `TaskDeferred` when a task due date is pushed.

**Commit:** `feat(todo): emit DomainEvents for task lifecycle`

---

### Task 1.4: Emit DomainEvents from feature-finance

**Files:**
- Modify: `crates/feature-finance/` (same pattern)

**Step 1-5:** Emit `TransactionRecorded` and `BudgetAlert` from finance tool actions.

**Commit:** `feat(finance): emit DomainEvents for transactions and budget alerts`

---

## Phase 2: Cognitive Crate — Storage Layer

New `crates/cognitive` with schema, repos, and core types. No LLM logic yet.

### Task 2.1: Create cognitive crate scaffold

**Files:**
- Create: `crates/cognitive/Cargo.toml`
- Create: `crates/cognitive/src/lib.rs`
- Create: `crates/cognitive/src/types.rs`
- Modify: `Cargo.toml` (workspace members + workspace.dependencies)

**Step 1: Scaffold crate**

```toml
# crates/cognitive/Cargo.toml
[package]
name = "cognitive"
version.workspace = true
edition.workspace = true

[dependencies]
common = { workspace = true }
storage = { workspace = true }
bus = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
```

**Step 2: Define core types in `types.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Memory operation result from consolidation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryOp {
    Add { id: String },
    Update { id: String, old_id: String },
    Delete { id: String, superseded_by: String },
    Noop,
}

/// A semantic fact with bi-temporal markers and FSRS decay.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SemanticFact {
    pub id: String,
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,

    pub valid_from: String,
    pub valid_until: Option<String>,
    pub recorded_at: String,
    pub superseded_at: Option<String>,
    pub superseded_by: Option<String>,

    pub stability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
}

/// An episodic memory entry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EpisodicMemory {
    pub id: String,
    pub domain: String,
    pub content: String,
    pub summary: Option<String>,
    pub importance: f64,
    pub occurred_at: String,
    pub recorded_at: String,
    pub stability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
}

/// A procedural rule learned from reflection.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProceduralRule {
    pub id: String,
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub source: String,
    pub signal_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
}

/// Salience verdict for event filtering.
#[derive(Debug, Clone, PartialEq)]
pub enum SalienceVerdict {
    Extract,
    Accumulate,
    Discard,
}

/// Observation extracted from a DomainEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub domain: String,
    pub content: String,
    pub importance: f64,
    pub source_event: String,
    pub timestamp: DateTime<Utc>,
}

/// The structured user model — queryable, domain-organized.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserModel {
    pub identity: Vec<SemanticFact>,
    pub energy: Vec<SemanticFact>,
    pub work: Vec<SemanticFact>,
    pub finance: Vec<SemanticFact>,
    pub learning: Vec<SemanticFact>,
    pub preferences: Vec<SemanticFact>,
}
```

**Step 3: Wire into workspace**

Add to root `Cargo.toml`:
```toml
# In [workspace] members:
"crates/cognitive",

# In [workspace.dependencies]:
cognitive = { path = "crates/cognitive" }
```

**Step 4: Verify build**

Run: `cargo build -p cognitive`
Expected: PASS

**Step 5: Commit**

```bash
git commit -m "feat(cognitive): scaffold crate with core types"
```

---

### Task 2.2: Cognitive storage — migrations and repos

**Files:**
- Create: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Create: `crates/cognitive/src/repos/mod.rs`
- Create: `crates/cognitive/src/repos/semantic_fact.rs`
- Create: `crates/cognitive/src/repos/episodic_memory.rs`
- Create: `crates/cognitive/src/repos/procedural_rule.rs`
- Test: inline per repo

**Step 1: Write migration**

```sql
-- crates/cognitive/migrations/001_cognitive_tables.sql

CREATE TABLE IF NOT EXISTS semantic_facts (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    subject         TEXT NOT NULL,
    predicate       TEXT NOT NULL,
    object          TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    source          TEXT NOT NULL DEFAULT 'observed',
    valid_from      TEXT NOT NULL,
    valid_until     TEXT,
    recorded_at     TEXT NOT NULL DEFAULT (datetime('now')),
    superseded_at   TEXT,
    superseded_by   TEXT,
    stability       REAL NOT NULL DEFAULT 1.0,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_semantic_facts_domain ON semantic_facts(domain);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_subject ON semantic_facts(subject, predicate);
CREATE INDEX IF NOT EXISTS idx_semantic_facts_active ON semantic_facts(valid_until) WHERE valid_until IS NULL;

CREATE TABLE IF NOT EXISTS episodic_memories (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    content         TEXT NOT NULL,
    summary         TEXT,
    importance      REAL NOT NULL DEFAULT 0.5,
    occurred_at     TEXT NOT NULL,
    recorded_at     TEXT NOT NULL DEFAULT (datetime('now')),
    stability       REAL NOT NULL DEFAULT 1.0,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_episodic_domain ON episodic_memories(domain);
CREATE INDEX IF NOT EXISTS idx_episodic_occurred ON episodic_memories(occurred_at);

CREATE TABLE IF NOT EXISTS procedural_rules (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    rule_text       TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    source          TEXT NOT NULL DEFAULT 'reflected',
    signal_count    INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    active          INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_procedural_domain ON procedural_rules(domain);
CREATE INDEX IF NOT EXISTS idx_procedural_active ON procedural_rules(active) WHERE active = 1;

-- Archive tables (identical schema, cold storage)
CREATE TABLE IF NOT EXISTS semantic_facts_archive (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,
    subject         TEXT NOT NULL,
    predicate       TEXT NOT NULL,
    object          TEXT NOT NULL,
    confidence      REAL NOT NULL,
    source          TEXT NOT NULL,
    valid_from      TEXT NOT NULL,
    valid_until     TEXT,
    recorded_at     TEXT NOT NULL,
    superseded_at   TEXT,
    superseded_by   TEXT,
    stability       REAL NOT NULL,
    last_accessed   TEXT,
    access_count    INTEGER NOT NULL,
    archived_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS coaching_strategies (
    id              TEXT PRIMARY KEY,
    strategy_type   TEXT NOT NULL,
    domain          TEXT NOT NULL,
    times_used      INTEGER NOT NULL DEFAULT 0,
    times_accepted  INTEGER NOT NULL DEFAULT 0,
    times_led_to_improvement INTEGER NOT NULL DEFAULT 0,
    avg_improvement_magnitude REAL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    last_used       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Step 2: Write repo tests**

```rust
// crates/cognitive/src/repos/semantic_fact.rs
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        // Run cognitive migrations
        sqlx::query(include_str!("../../migrations/001_cognitive_tables.sql"))
            .execute(&inner).await.unwrap();
        inner
    }

    #[tokio::test]
    async fn test_upsert_and_get_active_facts() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let fact = SemanticFact {
            id: "f1".into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: "peak_hours".into(),
            object: "10am-12pm".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
        };

        repo.upsert(&fact).await.unwrap();
        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].predicate, "peak_hours");
    }

    #[tokio::test]
    async fn test_supersede_fact() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert original fact
        let old = SemanticFact { id: "f1".into(), /* ... */ };
        repo.upsert(&old).await.unwrap();

        // Supersede it
        repo.supersede("f1", "f2").await.unwrap();
        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 0); // old fact no longer active
    }

    #[tokio::test]
    async fn test_record_access_increases_stability() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let fact = SemanticFact { id: "f1".into(), stability: 1.0, /* ... */ };
        repo.upsert(&fact).await.unwrap();

        repo.record_access("f1", 1.2).await.unwrap(); // new stability
        let updated = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(updated.stability, 1.2);
        assert_eq!(updated.access_count, 1);
    }
}
```

**Step 3: Implement repos**

Each repo follows the existing pattern: `new(pool: SqlitePool)` + CRUD + domain-specific queries. Key methods:

- `SemanticFactRepo`: `upsert`, `get`, `list_active(domain)`, `find_similar(subject, predicate)`, `supersede(old_id, new_id)`, `record_access(id, new_stability)`, `list_low_retrievability(threshold)`, `archive_superseded(older_than_days)`
- `EpisodicMemoryRepo`: `insert`, `list_range(start, end)`, `list_by_domain(domain, limit)`, `record_access`, `archive_old(older_than_days, min_access_count)`
- `ProceduralRuleRepo`: `upsert`, `list_active(domain)`, `increment_signal_count(id)`, `deactivate(id)`

**Step 4: Run tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS

**Step 5: Commit**

```bash
git commit -m "feat(cognitive): add storage migrations and repos for semantic/episodic/procedural memory"
```

---

### Task 2.3: FSRS decay and relevance scoring

**Files:**
- Create: `crates/cognitive/src/decay.rs`
- Test: inline

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrievability_fresh_memory() {
        // 0 days elapsed, stability 1.0 → retrievability ~1.0
        let r = retrievability(0.0, 1.0);
        assert!((r - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_retrievability_decays_over_time() {
        let r1 = retrievability(1.0, 1.0);
        let r7 = retrievability(7.0, 1.0);
        assert!(r1 > r7);
    }

    #[test]
    fn test_higher_stability_resists_decay() {
        let low_stability = retrievability(7.0, 1.0);
        let high_stability = retrievability(7.0, 10.0);
        assert!(high_stability > low_stability);
    }

    #[test]
    fn test_relevance_score_combines_factors() {
        let score = relevance_score(0.8, 0.9, 0.7, 0.5, 0.6);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_new_stability_after_successful_retrieval() {
        let old = 1.0;
        let new = update_stability(old, true);
        assert!(new > old);
    }

    #[test]
    fn test_stability_unchanged_on_failed_retrieval() {
        let old = 5.0;
        let new = update_stability(old, false);
        assert!(new <= old);
    }
}
```

**Step 2: Implement**

```rust
// crates/cognitive/src/decay.rs

/// FSRS-inspired retrievability: probability of successful recall.
/// R = exp(ln(0.9) * elapsed_days / stability)
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    if stability <= 0.0 { return 0.0; }
    (0.9_f64.ln() * elapsed_days / stability).exp()
}

/// Combined relevance score for memory retrieval.
/// Weights: semantic 0.3, retrievability 0.2, importance 0.15, frequency 0.1, situation 0.25
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
) -> f64 {
    (semantic_similarity * 0.3
        + retrievability * 0.2
        + importance * 0.15
        + access_frequency * 0.1
        + situational_boost * 0.25)
        .clamp(0.0, 1.0)
}

/// Update stability after a retrieval event.
/// Successful retrieval increases stability; failed does not.
pub fn update_stability(current: f64, success: bool) -> f64 {
    if success {
        // Stability grows with diminishing returns (log curve)
        current + (1.0 + current).ln().max(0.1)
    } else {
        current
    }
}
```

**Step 3: Run tests, commit**

```bash
cargo nextest run -p cognitive -E 'test(retrievability|relevance|stability)'
git commit -m "feat(cognitive): FSRS-inspired decay and relevance scoring"
```

---

### Task 2.4: Salience filter

**Files:**
- Create: `crates/cognitive/src/salience.rs`
- Test: inline

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bus::DomainEvent;

    #[test]
    fn test_user_stated_fact_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::UserStatedFact {
            fact: "I work best in mornings".into(),
            domain: "productivity".into(),
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_user_correction_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::UserCorrectedAI {
            original: "You like afternoon work".into(),
            correction: "No, I prefer mornings".into(),
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_budget_alert_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::BudgetAlert {
            category: "food".into(),
            spent: 450.0,
            limit: 500.0,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_normal_productivity_score_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::ProductivityScoreComputed {
            date: "2026-03-06".into(),
            score: 72.0,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_task_completed_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(30),
            estimated_duration_mins: Some(45),
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }
}
```

**Step 2: Implement**

Heuristic function that classifies each event variant. User-explicit events are always `Extract`. Threshold-crossing events (budget alerts, over-budget transactions) are `Extract`. Routine events (score computed, task completed, session ended) are `Accumulate`. The accumulator tracks counts and can promote accumulated events to `Extract` when patterns emerge (handled in Phase 3).

**Step 3: Run tests, commit**

```bash
cargo nextest run -p cognitive -E 'test(salience)'
git commit -m "feat(cognitive): salience filter for DomainEvent triage"
```

---

## Phase 3: Cognitive Crate — Memory Lifecycle

LLM-powered extraction, consolidation, and the core `CognitiveMemory` API.

### Task 3.1: Memory extraction (hot-path)

**Files:**
- Create: `crates/cognitive/src/extraction.rs`
- Test: inline with mock LLM

**Purpose:** Takes an `Observation` (from a domain event that passed salience filter as `Extract`) and uses an LLM call to extract structured `SemanticFact` candidates.

**Key design:**
- Define `ExtractionHandler` trait (like `ProductivityHandler`) — implemented in `agent` crate to access the LLM provider
- Extraction prompt: "Given this observation about the user, extract any facts as subject-predicate-object triples. Return JSON array."
- Max 256 tokens per extraction call
- Each extracted fact goes through consolidation (Task 3.2)

**Test with mock handler that returns pre-defined facts.**

**Commit:** `feat(cognitive): LLM-backed memory extraction from observations`

---

### Task 3.2: Memory consolidation (Mem0 ADD/UPDATE/DELETE/NOOP)

**Files:**
- Create: `crates/cognitive/src/consolidation.rs`
- Test: inline with mock

**Purpose:** For each extracted fact candidate:
1. Embed it and search LanceDB for top-5 similar existing facts
2. If no matches, ADD
3. If matches exist, call LLM with candidate + existing facts: "Should this ADD a new fact, UPDATE an existing one, DELETE (supersede) an existing one, or is it a NOOP?"
4. Execute the operation

**Key design:**
- `ConsolidationHandler` trait for LLM calls (implemented in agent)
- `consolidate_fact(candidate: &SemanticFact, existing: &[SemanticFact]) -> MemoryOp`
- On UPDATE: supersede old fact, insert new with `superseded_by` link
- On DELETE: set `valid_until` on contradicted fact, insert replacement
- **Pattern validation:** only promote to semantic memory if `signal_count >= 5` for inferred patterns

**Commit:** `feat(cognitive): Mem0-style memory consolidation with ADD/UPDATE/DELETE/NOOP`

---

### Task 3.3: Background consolidation service

**Files:**
- Create: `crates/cognitive/src/background.rs`
- Test: inline

**Purpose:** Tokio background task that:
1. Drains the accumulated `Observation` buffer (events that were `Accumulate`)
2. Checks for emerging patterns (same event type >= 5 occurrences across >= 3 days)
3. Promotes patterns to extraction → consolidation pipeline
4. Runs after conversation ends or on a timer (every 5 minutes)

**Commit:** `feat(cognitive): background consolidation service for accumulated events`

---

### Task 3.4: Memory retrieval with FSRS + situational boost

**Files:**
- Create: `crates/cognitive/src/retrieval.rs`
- Modify: `crates/context_engine/src/memory_retriever.rs` (extend `MemoryRetriever` trait or create new impl)
- Test: inline

**Purpose:** Implement `CognitiveMemoryRetriever` that:
1. Embeds the query via fastembed
2. Searches LanceDB for semantically similar memories
3. Scores each result using `relevance_score()` with FSRS retrievability + situational boost
4. Records access on retrieved memories (increases stability)
5. Returns top-N ranked results

**Situational boost:** Accept an optional `UserSituation` (Phase 4) to bias retrieval. Initially pass `None` — pure semantic + FSRS scoring.

**Commit:** `feat(cognitive): FSRS-scored memory retrieval with situational boost`

---

### Task 3.5: Memory compaction and decay

**Files:**
- Create: `crates/cognitive/src/compaction.rs`
- Test: inline

**Purpose:** Background job (daily):
1. Recalculate `retrievability` for all active memories
2. Archive semantic facts where `valid_until IS NOT NULL` and `retrievability < 0.1` and `superseded_at` older than 90 days
3. Merge supersession chains (A→B→C: archive A and B, keep C)
4. Summarize and archive episodic memories older than 90 days with low access count
5. Enforce size budget: if active semantic facts > 10,000, aggressively compact lowest-retrievability

**Commit:** `feat(cognitive): memory compaction, archival, and FSRS decay`

---

### Task 3.6: CognitiveContextSource — unified context injection

**Files:**
- Create: `crates/cognitive/src/context_source.rs` (or in `crates/agent/src/context_sources/cognitive.rs`)
- Test: inline

**Purpose:** Single `ContextSource` (priority 60) that replaces `LearningContextSource` + `ProductivityContextSource`:
1. Load structured `UserModel` (key facts per domain)
2. Load active procedural rules
3. Load today's productivity summary (from feature-productivity, cached)
4. Format as `# User Understanding\n## Identity\n...\n## Patterns\n...\n## Preferences\n...`

This is the single point where the cognitive system injects knowledge into the LLM prompt.

**Commit:** `feat(cognitive): unified CognitiveContextSource replacing scattered context sources`

---

### Task 3.7: Weekly reflection

**Files:**
- Create: `crates/cognitive/src/reflection.rs`
- Test: inline with mock LLM

**Purpose:** Scheduled weekly (or on-demand):
1. Load all episodic memories from past 7 days
2. Load coaching intervention history + feedback
3. Load current user model + procedural rules
4. LLM prompt: "Review this week's observations. Identify cross-domain patterns, evaluate coaching effectiveness, suggest user model updates and procedural rule changes."
5. Run consolidation on each LLM-generated fact/rule update
6. **Pattern validation:** only apply updates backed by >= 5 signals across >= 3 days
7. Store the reflection itself as an episodic memory

**Commit:** `feat(cognitive): weekly LLM reflection with cross-domain pattern synthesis`

---

## Phase 4: UserSituation World Model

Derived intermediate layer between raw memory and coaching.

### Task 4.1: UserSituation types and computation

**Files:**
- Create: `crates/cognitive/src/situation.rs`
- Test: inline

**Purpose:** Define `UserSituation` struct and `SituationComputer` that assembles it from:
- Current productivity state (from `feature-productivity` repos: active time, focus state, distraction rate)
- Task pressure (from `feature-todo` repos: overdue tasks, approaching deadlines)
- User model (from cognitive semantic facts: energy profile, patterns)
- Recent coaching history (from coaching_strategies table)

Recomputed every 60 seconds or on significant DomainEvent.

Key fields: `energy_level`, `focus_state`, `deadline_pressure`, `distraction_risk`, `coaching_receptivity`, `task_avoidance_detected`.

**Commit:** `feat(cognitive): UserSituation world model computation`

---

## Phase 5: Proactive Intelligence Engine (feature-coaching)

### Task 5.1: Scaffold feature-coaching crate

**Files:**
- Create: `crates/feature-coaching/Cargo.toml`
- Create: `crates/feature-coaching/src/lib.rs`
- Modify: `Cargo.toml` (workspace)

**Commit:** `feat(coaching): scaffold feature-coaching crate`

---

### Task 5.2: Signal accumulator + trigger conditions

**Files:**
- Create: `crates/feature-coaching/src/signal_accumulator.rs`
- Test: inline

**Purpose:** Subscribes to DomainEventBus. Maintains a rolling `SituationBuffer` (last 30 minutes of events). Evaluates trigger conditions against `UserSituation`:

Triggers (heuristic, no LLM):
- `distraction_streak >= 3` in 15min window
- `productive_ratio < user_baseline * 0.7`
- `deadline < 24h && no_activity_on_task`
- `focus_quality declining 3+ consecutive sessions`
- `context_switches > 2x personal_average`
- `budget > 80% of limit`
- Custom triggers from procedural memory

Each trigger has a cooldown to prevent rapid re-firing.

**Commit:** `feat(coaching): signal accumulator with heuristic trigger conditions`

---

### Task 5.3: Pattern detection layer

**Files:**
- Create: `crates/feature-coaching/src/pattern_detector.rs`
- Test: inline

**Purpose:** Aggregates accumulated signals into higher-level patterns:
- Raw distraction events after 3pm → "afternoon_energy_drop"
- Task X deferred 3+ times → "task_avoidance::{task_type}"
- High scores on days with morning exercise → "exercise_productivity_correlation"

Patterns are the input to the coaching reasoner (not raw events). Each pattern has a `confidence` and `signal_count`.

**Commit:** `feat(coaching): pattern detection layer between signals and coaching`

---

### Task 5.4: Coaching reasoner (LLM-powered)

**Files:**
- Create: `crates/feature-coaching/src/reasoner.rs`
- Test: inline with mock LLM

**Purpose:** When a trigger fires:
1. Load `UserSituation` snapshot
2. Load detected patterns relevant to the trigger
3. Retrieve memories via `CognitiveMemoryRetriever` (with situational boost from UserSituation)
4. Load active procedural rules
5. Load recent intervention history (avoid repetition)
6. Build coaching prompt and call LLM
7. Parse `CoachingDecision`: intervention, confidence, reasoning, observations
8. Feed observations back to cognitive layer

The prompt includes: "You are a productivity coach. Given the user's current situation, patterns, and history, decide whether to intervene and how. Consider the user's coaching receptivity. Be specific and actionable."

**Commit:** `feat(coaching): LLM-powered coaching reasoner with situation-aware prompts`

---

### Task 5.5: Intervention router + rate limiting

**Files:**
- Create: `crates/feature-coaching/src/router.rs`
- Test: inline

**Purpose:** Routes `CoachingDecision` to the appropriate channel based on:
- `confidence × coaching_intensity` matrix (from design doc)
- `coaching_receptivity` modulation (from UserSituation)
- Rate limits: max 3/hour, 10/day, 30min cooldown on dismissed types
- Exponential backoff on repeatedly dismissed intervention types
- Never interrupt active focus sessions with low-priority nudges

Channels:
- `DashboardCard` — write to `insight_cards` table (existing UI supports this)
- `ChatMessage` — emit via MessageBus as proactive outbound message
- `Notification` — emit Tauri notification event
- `Overlay` — emit Tauri overlay event (existing distraction overlay infrastructure)

**Commit:** `feat(coaching): intervention router with rate limiting and adaptive tolerance`

---

### Task 5.6: Feedback tracker

**Files:**
- Create: `crates/feature-coaching/src/feedback.rs`
- Test: inline

**Purpose:** Three feedback channels:
1. **Explicit:** User responds to intervention (helpful/dismiss/stop) — captured via DomainEvent::CoachingFeedback
2. **Behavioral:** After nudge delivery, monitor DomainEvents for 2 minutes. If user switches from distraction to productive app → `behavioral_positive`. If no change → `behavioral_negative`.
3. **Outcome:** Compare daily productivity score on days with interventions vs without (rolling 14-day window)

All feedback:
- Stored as episodic memory in cognitive layer
- Updates `coaching_strategies` table (times_used, times_accepted, times_led_to_improvement)
- Updates `coaching_receptivity` in UserSituation
- Fed into weekly reflection for procedural rule adjustment

**Commit:** `feat(coaching): closed-loop feedback tracker with explicit, behavioral, and outcome channels`

---

### Task 5.7: Strategy effectiveness tracking

**Files:**
- Modify: `crates/feature-coaching/src/feedback.rs`
- Modify: `crates/cognitive/src/reflection.rs` (add strategy review to weekly reflection)
- Test: inline

**Purpose:** During weekly reflection, the LLM reviews `coaching_strategies` table:
- Strategies with high acceptance + improvement rate → increase confidence, prioritize
- Strategies with low acceptance → decrease confidence, reduce usage
- Strategies with zero improvement → flag for retirement

This creates natural selection of coaching approaches tailored to the individual.

**Commit:** `feat(coaching): strategy effectiveness tracking with weekly LLM review`

---

## Phase 6: Integration and Wiring

### Task 6.1: Wire cognitive into agent builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/Cargo.toml`

**Purpose:**
- Add `cognitive` dependency
- In `AgentLoopBuilder::build()`:
  - Create `CognitiveMemory` instance from storage pool
  - Register `CognitiveContextSource` (replacing `LearningContextSource` + `ProductivityContextSource`)
  - Wire `ExtractionHandler` and `ConsolidationHandler` implementations
  - Start background consolidation service
  - Schedule weekly reflection

**Commit:** `feat(agent): wire cognitive memory system into agent builder`

---

### Task 6.2: Wire coaching into desktop app

**Files:**
- Modify: `crates/desktop/src/app_core.rs`
- Create: `crates/desktop/src/commands/coaching.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

**Purpose:**
- Initialize `DomainEventBus` in app startup
- Pass bus to `ProductivityEngine`, todo handler, finance handler
- Initialize `feature-coaching` with bus subscription
- Wire coaching intervention channels to Tauri events
- Add Tauri commands: `coaching_set_intensity`, `coaching_history`, `coaching_dismiss`

**Commit:** `feat(desktop): wire coaching engine into Tauri app`

---

### Task 6.3: Deprecate replaced systems

**Files:**
- Modify: `crates/agent/src/context_sources/learning.rs` (remove or gate behind feature flag)
- Modify: `crates/agent/src/learning/service.rs` (remove or gate)
- Modify: `crates/agent/src/agent_loop/builder.rs` (remove old wiring)

**Purpose:** Remove `LearningContextSource` registration. Remove `LearningService` startup. The cognitive system now handles everything these did, plus much more.

Keep `ProductivityContextSource` behind a feature flag initially — cognitive source replaces it, but we want a safe rollback path.

**Commit:** `refactor(agent): deprecate LearningService and LearningContextSource in favor of cognitive system`

---

### Task 6.4: Integration tests

**Files:**
- Create: `tests/integration/cognitive.rs`
- Modify: `tests/integration/main.rs`

**Purpose:** End-to-end tests:
1. Emit DomainEvents → verify cognitive memory stores facts
2. Verify salience filtering (Extract vs Accumulate vs Discard)
3. Verify consolidation (ADD a new fact, UPDATE when fact changes, NOOP on duplicate)
4. Verify retrieval returns FSRS-ranked results
5. Verify coaching trigger fires on distraction pattern
6. Verify rate limiting prevents over-nudging
7. Verify feedback updates coaching strategy confidence

**Commit:** `test: integration tests for cognitive memory and coaching pipeline`

---

## Build Order Summary

```
Phase 1 (bus)     → Phase 2 (cognitive storage) → Phase 3 (cognitive lifecycle)
                                                      ↓
Phase 4 (situation) → Phase 5 (coaching) → Phase 6 (integration)
```

Each phase is independently testable. Phase 1-2 have zero LLM dependency. Phase 3+ requires LLM but uses trait injection so tests use mocks.

**Estimated tasks:** 20 tasks across 6 phases
**Each task:** independently compilable, testable, committable
