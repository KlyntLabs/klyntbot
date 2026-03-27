# Squad Debate Engine Upgrade — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify the squad system under a single debate engine with active FSRS-5 learning, three interaction modes, and all 10 implementation gaps closed.

**Architecture:** Pure-function debate engine (`run_debate`) with `DebateConfig` + `DebateContext` -> `DebateResult`. Chat and notes are thin callers with different config. Interaction mode detection (debate/direct/lead) routes messages before the engine. FSRS-5 feeds learned weights back into persona ordering.

**Tech Stack:** Rust, SQLite (sqlx), tokio (async/join_all/oneshot), futures-util, serde_json, chrono, uuid

**Spec:** `docs/superpowers/specs/2026-03-27-squad-debate-engine-upgrade-design.md`

---

### Task 1: Schema Migration — Add Columns and Indexes

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`

Pre-release, so we modify the migration in-place per CLAUDE.md conventions.

- [ ] **Step 1: Add `default_interaction_mode`, `last_smart_mode`, `last_smart_updated` to `squads` table**

In `001_cognitive_tables.sql`, find the `CREATE TABLE IF NOT EXISTS squads` block and add the three new columns:

```sql
CREATE TABLE IF NOT EXISTS squads (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    icon TEXT NOT NULL DEFAULT '',
    orchestrator_skill TEXT NOT NULL DEFAULT 'general',
    source TEXT NOT NULL DEFAULT 'user',
    domains TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    default_interaction_mode TEXT NOT NULL DEFAULT 'lead',
    last_smart_mode TEXT,
    last_smart_updated TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 2: Add `debate_transcript` to `insight_reviews` table**

Find the `CREATE TABLE IF NOT EXISTS insight_reviews` block and add:

```sql
    debate_transcript JSON,
```

after the existing columns (before `created_at`).

- [ ] **Step 3: Add indexes for hot query paths**

After the existing `blackboard_entries` indexes, add:

```sql
CREATE INDEX IF NOT EXISTS idx_persona_accuracy_lookup
    ON persona_accuracy(persona_id, squad_id, domain);
CREATE INDEX IF NOT EXISTS idx_blackboard_session_key
    ON blackboard_entries(session_key);
CREATE INDEX IF NOT EXISTS idx_blackboard_created_at
    ON blackboard_entries(created_at);
```

- [ ] **Step 4: Verify migration compiles**

Run: `cargo nextest run -p cognitive --no-tests`
Expected: compilation succeeds (no test execution needed yet)

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql
git commit -m "feat(squad): add schema columns for interaction modes, smart learning, debate transcript"
```

---

### Task 2: Core Types Module

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/debate_types.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`

All shared types from the spec — `DebateConfig`, `DebateContext`, `DebateResult`, `DebateEvent`, etc. — go in a dedicated types module so both the engine and callers can import them.

- [ ] **Step 1: Create `debate_types.rs` with all type definitions**

```rust
use crate::events::AgentEvent;
use cognitive::PersonaRow;
use providers::Message;
use serde::{Deserialize, Serialize};

// --- Config ---

#[derive(Debug, Clone)]
pub struct DebateConfig {
    pub output_mode: OutputMode,
    pub max_rounds: u8,
    pub timeout_seconds: u64,
    pub consensus_threshold: f64,
    pub temperature_override: Option<f32>,
    pub confidence_floor: Option<f32>,
    pub token_budget: Option<TokenBudget>,
    pub accuracy_blend: f32,
    pub respect_user_prefs: bool,
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            output_mode: OutputMode::Synthesized,
            max_rounds: 6,
            timeout_seconds: 120,
            consensus_threshold: 85.0,
            temperature_override: None,
            confidence_floor: None,
            token_budget: None,
            accuracy_blend: 0.3,
            respect_user_prefs: true,
        }
    }
}

impl DebateConfig {
    /// Chat defaults: synthesized output, 6 rounds, 120s timeout
    pub fn for_chat() -> Self {
        Self::default()
    }

    /// Notes defaults: structured per-persona, 3 rounds, 240s timeout, analytical temperature
    pub fn for_notes() -> Self {
        Self {
            output_mode: OutputMode::StructuredPerPersona,
            max_rounds: 3,
            timeout_seconds: 240,
            temperature_override: Some(0.3),
            confidence_floor: Some(0.6),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Synthesized,
    StructuredPerPersona,
}

#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub remaining_monthly: u64,
    pub daily_squad_cap: Option<u64>,
    pub estimated_tokens_per_round: u64,
}

// --- Context ---

#[derive(Debug, Clone)]
pub struct DebateContext {
    pub skill_prompt: String,
    pub conversation_history: Vec<Message>,
    pub user_message: String,
    pub cognitive_context: Option<String>,
    pub domains: Vec<String>,
}

// --- Result ---

#[derive(Debug, Clone, Serialize)]
pub struct DebateResult {
    pub persona_responses: Vec<PersonaResponse>,
    pub synthesis: String,
    pub rounds_completed: u8,
    pub total_rounds_planned: u8,
    pub consensus_reached: bool,
    pub final_consensus_score: f64,
    pub partial_reason: Option<PartialReason>,
    pub token_usage: TokenUsage,
    pub learned_weights_applied: Vec<LearnedWeight>,
    pub accuracy_outcomes: Vec<AccuracyOutcome>,
    pub rounds: Vec<DebateRound>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaResponse {
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub content: String,
    pub round: u8,
    pub phase: DebatePhase,
    pub confidence: f64,
    pub effective_confidence: f64,
    pub consensus_alignment: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebateRound {
    pub round: u8,
    pub phase: DebatePhase,
    pub responses: Vec<PersonaResponse>,
    pub judge_decision: Option<JudgeDecision>,
    pub persona_order: Vec<String>,
    pub order_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebatePhase {
    Opening,
    Discussion,
    Targeted,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeDecision {
    pub consensus_score: f64,
    pub decision: String,
    pub speaking_order: Vec<String>,
    pub challenges: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartialReason {
    Timeout { elapsed_seconds: u64 },
    BudgetCap { estimated_cost: u64, remaining: u64 },
    ConsensusEarly { at_round: u8 },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearnedWeight {
    pub persona_id: String,
    pub base_weight: f64,
    pub accuracy_weight: f64,
    pub blended_weight: f64,
    pub domain: String,
    pub debates_used: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccuracyOutcome {
    pub persona_id: String,
    pub squad_id: String,
    pub domain: String,
    pub consensus_alignment: f64,
    pub fsrs_rating: u8,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TokenUsage {
    pub persona_tokens: u64,
    pub judge_tokens: u64,
    pub synthesis_tokens: u64,
    pub total: u64,
}

impl TokenUsage {
    pub fn add_persona(&mut self, tokens: u64) {
        self.persona_tokens += tokens;
        self.total += tokens;
    }

    pub fn add_judge(&mut self, tokens: u64) {
        self.judge_tokens += tokens;
        self.total += tokens;
    }

    pub fn add_synthesis(&mut self, tokens: u64) {
        self.synthesis_tokens += tokens;
        self.total += tokens;
    }
}

// --- Events ---

#[derive(Debug, Clone)]
pub enum DebateEvent {
    BudgetCheck {
        estimated_cost: u64,
        remaining: u64,
        action: BudgetAction,
    },
    RoundStarted {
        round: u8,
        phase: DebatePhase,
        persona_order: Vec<String>,
        order_reason: String,
    },
    PersonaPerspective {
        persona_id: String,
        name: String,
        icon: String,
        role: String,
        content: String,
        round: u8,
        challenge: Option<String>,
    },
    JudgeDecision {
        round: u8,
        consensus_score: f64,
        speaking_order: Vec<String>,
        decision: String,
    },
    RoundCompleted {
        round: u8,
        phase: DebatePhase,
    },
    ConsensusReached {
        round: u8,
        score: f64,
    },
    DebatePartial {
        reason: PartialReason,
        completed_rounds: u8,
    },
    SynthesisStarted,
    SynthesisChunk {
        content: String,
    },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetAction {
    Proceed,
    ReducedRounds { from: u8, to: u8 },
    RequiresApproval,
}

// --- Interaction Modes ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SquadInteractionMode {
    Debate,
    DirectAddress { persona_id: String },
    LeadResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultInteractionMode {
    Lead,
    Debate,
    Smart,
}

impl Default for DefaultInteractionMode {
    fn default() -> Self {
        Self::Lead
    }
}

impl DefaultInteractionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Debate => "debate",
            Self::Smart => "smart",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "debate" => Self::Debate,
            "smart" => Self::Smart,
            _ => Self::Lead,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectAddressResult {
    pub response: PersonaResponse,
    pub token_usage: TokenUsage,
    pub context_used: Vec<String>,
}
```

- [ ] **Step 2: Export from `engines/mod.rs`**

Add to `crates/agent/src/intent_pipeline/engines/mod.rs`:

```rust
pub mod debate_types;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p agent`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/debate_types.rs crates/agent/src/intent_pipeline/engines/mod.rs
git commit -m "feat(squad): add core debate engine types"
```

---

### Task 3: PersonaAccuracyRepo — Add `reset` and Graduated Scoring

**Files:**
- Modify: `crates/cognitive/src/repos/persona_accuracy.rs`

The spec requires a `reset()` method (for "Reset learning" button) and the `record_outcome` method needs to accept a graduated f64 `consensus_alignment` score instead of just a boolean.

- [ ] **Step 1: Write tests for `reset` and graduated `record_outcome`**

Add to the `#[cfg(test)] mod tests` block in `persona_accuracy.rs`:

```rust
#[tokio::test]
async fn test_reset_persona_accuracy() {
    let pool = test_pool().await;
    let repo = PersonaAccuracyRepo::new(pool);

    // Record some outcomes to build up state
    repo.record_outcome("p1", "s1", "finance", 0.9).await.unwrap();
    repo.record_outcome("p1", "s1", "finance", 0.8).await.unwrap();

    let before = repo.get("p1", "s1", "finance").await.unwrap().unwrap();
    assert_eq!(before.total_debates, 2);
    assert!(before.stability != 1.0); // changed from default

    // Reset
    repo.reset("p1", "s1", "finance").await.unwrap();

    let after = repo.get("p1", "s1", "finance").await.unwrap().unwrap();
    assert_eq!(after.total_debates, 0);
    assert_eq!(after.consensus_hits, 0);
    assert!((after.stability - 1.0).abs() < f64::EPSILON);
    assert!((after.difficulty - 5.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_graduated_consensus_scoring() {
    let pool = test_pool().await;
    let repo = PersonaAccuracyRepo::new(pool);

    // High alignment (0.9) -> FSRS rating 4 "Easy"
    let result = repo.record_outcome("p1", "s1", "general", 0.9).await.unwrap();
    assert_eq!(result.total_debates, 1);
    assert_eq!(result.consensus_hits, 1); // >= 0.5 counts as hit

    // Low alignment (0.2) -> FSRS rating 1 "Again"
    let result = repo.record_outcome("p2", "s1", "general", 0.2).await.unwrap();
    assert_eq!(result.total_debates, 1);
    assert_eq!(result.consensus_hits, 0); // < 0.5 is not a hit
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(reset_persona_accuracy)' -E 'test(graduated_consensus)'`
Expected: compilation errors — `reset` method doesn't exist, `record_outcome` signature mismatch

- [ ] **Step 3: Update `record_outcome` to accept `consensus_alignment: f64`**

Change the signature from `in_consensus: bool` to `consensus_alignment: f64`. Map the f64 to FSRS-5 rating internally:

```rust
/// Records a debate outcome with graduated consensus alignment (0.0-1.0).
/// Maps to FSRS-5 ratings: 0.8+ = Easy(4), 0.5-0.8 = Good(3), 0.3-0.5 = Hard(2), <0.3 = Again(1)
pub async fn record_outcome(
    &self,
    persona_id: &str,
    squad_id: &str,
    domain: &str,
    consensus_alignment: f64,
) -> Result<PersonaAccuracy, sqlx::Error> {
    let in_consensus = consensus_alignment >= 0.5;
    let rating: u8 = if consensus_alignment >= 0.8 {
        4 // Easy
    } else if consensus_alignment >= 0.5 {
        3 // Good
    } else if consensus_alignment >= 0.3 {
        2 // Hard
    } else {
        1 // Again
    };
    // ... rest of FSRS-5 logic uses `rating` instead of hardcoded 3/1
    // ... `in_consensus` still used for consensus_hits increment
```

Update the FSRS-5 computation to use the graduated `rating` variable instead of the binary `if in_consensus { 3 } else { 1 }` pattern.

- [ ] **Step 4: Add `reset` method**

```rust
pub async fn reset(
    &self,
    persona_id: &str,
    squad_id: &str,
    domain: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE persona_accuracy SET
            total_debates = 0, consensus_hits = 0,
            stability = 1.0, difficulty = 5.0,
            last_debate_at = NULL,
            updated_at = datetime('now')
        WHERE persona_id = ?1 AND squad_id = ?2 AND domain = ?3",
    )
    .bind(persona_id)
    .bind(squad_id)
    .bind(domain)
    .execute(&*self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(reset_persona_accuracy)' -E 'test(graduated_consensus)'`
Expected: both PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/persona_accuracy.rs
git commit -m "feat(squad): add reset and graduated consensus scoring to PersonaAccuracyRepo"
```

---

### Task 4: BlackboardRepo — Add Prefix Delete

**Files:**
- Modify: `crates/cognitive/src/repos/blackboard.rs`

The spec requires `delete_by_session_prefix` for session-lifecycle cleanup (e.g., deleting all entries matching `"session:{key}:*"`).

- [ ] **Step 1: Write test for prefix delete**

```rust
#[tokio::test]
async fn test_delete_by_session_prefix() {
    let pool = test_pool().await;
    let repo = BlackboardRepo::new(pool);

    // Insert entries with different session keys
    let e1 = NewBlackboardEntry {
        session_key: "session:abc:squad1",
        squad_id: "s1", round: 1,
        persona_id: "p1", persona_name: "Test",
        entry_type: "opening", content: "hello",
        confidence: 0.5, references_entry_id: None,
    };
    let e2 = NewBlackboardEntry {
        session_key: "session:abc:squad2",
        ..e1.clone()
    };
    let e3 = NewBlackboardEntry {
        session_key: "session:xyz:squad1",
        ..e1.clone()
    };
    repo.insert(&e1).await.unwrap();
    repo.insert(&e2).await.unwrap();
    repo.insert(&e3).await.unwrap();

    // Delete all entries for session "abc"
    let deleted = repo.delete_by_session_prefix("session:abc:").await.unwrap();
    assert_eq!(deleted, 2);

    // "xyz" entries should remain
    let remaining = repo.list_for_session("session:xyz:squad1").await.unwrap();
    assert_eq!(remaining.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(delete_by_session_prefix)'`
Expected: FAIL — method doesn't exist

- [ ] **Step 3: Implement `delete_by_session_prefix`**

```rust
/// Delete all entries whose session_key starts with the given prefix.
/// Used for session-lifecycle cleanup: `delete_by_session_prefix("session:{key}:")`
pub async fn delete_by_session_prefix(&self, prefix: &str) -> Result<u64, sqlx::Error> {
    let like_pattern = format!("{}%", prefix);
    let result = sqlx::query(
        "DELETE FROM blackboard_entries WHERE session_key LIKE ?1",
    )
    .bind(&like_pattern)
    .execute(&*self.pool)
    .await?;
    Ok(result.rows_affected())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(delete_by_session_prefix)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/blackboard.rs
git commit -m "feat(squad): add delete_by_session_prefix to BlackboardRepo"
```

---

### Task 5: SquadRow/SquadRepo — New Columns and Update Method

**Files:**
- Modify: `crates/cognitive/src/repos/squad.rs`

Add `default_interaction_mode`, `last_smart_mode`, `last_smart_updated` to `SquadRow`. Extend `update()` to accept `orchestrator_skill` and `default_interaction_mode`. Update `seed_builtins` to include new defaults.

- [ ] **Step 1: Write test for new fields and update**

```rust
#[tokio::test]
async fn test_squad_default_interaction_mode() {
    let pool = test_pool().await;
    let repo = SquadRepo::new(pool);

    let squad = repo.create(&NewSquad {
        name: "Test Squad".into(),
        description: "test".into(),
        icon: "🧪".into(),
        orchestrator_skill: "general".into(),
        domains: vec![],
    }).await.unwrap();

    // Default should be "lead"
    assert_eq!(squad.default_interaction_mode, "lead");

    // Update interaction mode
    let updated = repo.update(
        &squad.id, Some("Test Squad"), None, None, None,
        Some("finance-management"), Some("debate"),
    ).await.unwrap().unwrap();
    assert_eq!(updated.orchestrator_skill, "finance-management");
    assert_eq!(updated.default_interaction_mode, "debate");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(squad_default_interaction_mode)'`
Expected: FAIL — field doesn't exist on `SquadRow`

- [ ] **Step 3: Add fields to `SquadRow`**

```rust
pub struct SquadRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub source: String,
    pub domains: String,
    pub is_active: i64,
    pub default_interaction_mode: String, // "lead" | "debate" | "smart"
    pub last_smart_mode: Option<String>,
    pub last_smart_updated: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 4: Update `create` to include new columns**

In the `INSERT INTO squads` query, add `default_interaction_mode` with default `'lead'`. The `last_smart_mode` and `last_smart_updated` default to `NULL` via the schema.

- [ ] **Step 5: Extend `update` method signature**

Change from:
```rust
pub async fn update(&self, id: &str, name: Option<&str>, description: Option<&str>, icon: Option<&str>, domains: Option<&[String]>) -> ...
```
To:
```rust
pub async fn update(&self, id: &str, name: Option<&str>, description: Option<&str>, icon: Option<&str>, domains: Option<&[String]>, orchestrator_skill: Option<&str>, default_interaction_mode: Option<&str>) -> ...
```

Use `COALESCE` for the new fields in the UPDATE SQL, same pattern as existing fields.

- [ ] **Step 6: Fix all call sites**

Update all callers of `repo.update()` in `crates/app-core/src/handlers/squads.rs` to pass the new parameters (initially `None` for both to preserve existing behavior).

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(squad)'`
Expected: all squad tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/repos/squad.rs crates/app-core/src/handlers/squads.rs
git commit -m "feat(squad): add default_interaction_mode, orchestrator_skill update to SquadRepo"
```

---

### Task 6: IPC Types — Update Shared Commands

**Files:**
- Modify: `crates/desktop-shared/src/commands/squads.rs`
- Modify: `crates/desktop-shared/src/commands/chat.rs`

Update `UpdateSquadParams` to include `orchestrator_skill` and `default_interaction_mode`. Remove `squad_mode` from `SessionContextInput`. Add new fields to `SquadResponse`.

- [ ] **Step 1: Update `SquadResponse` and `UpdateSquadParams`**

In `crates/desktop-shared/src/commands/squads.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub source: String,
    pub domains: Vec<String>,
    pub is_active: bool,
    pub default_interaction_mode: String, // NEW
    pub members: Vec<SquadMemberResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSquadParams {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub orchestrator_skill: Option<String>,           // NEW
    pub default_interaction_mode: Option<String>,      // NEW
    pub domains: Option<Vec<String>>,
}
```

- [ ] **Step 2: Remove `squad_mode` from `SessionContextInput`**

In `crates/desktop-shared/src/commands/chat.rs`, remove the `squad_mode: Option<String>` field from `SessionContextInput`. Keep `squad_id`.

- [ ] **Step 3: Fix all compilation errors from removal**

Search for `squad_mode` references in `crates/app-core/src/handlers/chat/streaming.rs` and `crates/agent/src/agent_loop/mod.rs`. Remove any extraction or forwarding of `squad_mode`. This includes:
- `streaming.rs:L234` — remove `let squad_mode = ...`
- `streaming.rs:L236` — remove `squad_mode` argument to `process_direct_streaming`
- The `process_direct_streaming` method signature — remove the `squad_mode` parameter

- [ ] **Step 4: Update `resolved_to_response` in `squads.rs` handler**

In `crates/app-core/src/handlers/squads.rs`, update `resolved_to_response` to include `default_interaction_mode`:

```rust
fn resolved_to_response(resolved: &cognitive::ResolvedSquad) -> SquadResponse {
    SquadResponse {
        // ... existing fields ...
        default_interaction_mode: resolved.squad.default_interaction_mode.clone(),
        // ...
    }
}
```

- [ ] **Step 5: Update `update_squad` handler to pass new fields**

```rust
pub async fn update_squad(&self, params: &UpdateSquadParams) -> Result<SquadResponse, ApiError> {
    let repo = self.squad_repo()?;
    let updated = repo.update(
        &params.id,
        params.name.as_deref(),
        params.description.as_deref(),
        params.icon.as_deref(),
        params.domains.as_deref(),
        params.orchestrator_skill.as_deref(),
        params.default_interaction_mode.as_deref(),
    ).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
    // ...
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build --workspace`
Expected: compiles cleanly (may need to fix other references to `squad_mode`)

- [ ] **Step 7: Commit**

```bash
git add crates/desktop-shared/src/commands/squads.rs crates/desktop-shared/src/commands/chat.rs crates/app-core/src/handlers/squads.rs crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(squad): update IPC types, remove squad_mode, add interaction mode fields"
```

---

### Task 7: Remove Dead Config — `squad_mode` from RoutingContext

**Files:**
- Modify: `crates/tools-core/src/routing.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/agent/src/agent_loop/mod.rs`

- [ ] **Step 1: Remove `squad_mode` from `RoutingContext`**

In `crates/tools-core/src/routing.rs`, remove the `pub squad_mode: Option<String>` field from `RoutingContext`. Update the `with_squad` constructor to no longer accept or set it.

- [ ] **Step 2: Remove `_squad_mode_str` from `run_squad_execution`**

In `crates/agent/src/agent_runtime/runtime.rs`, remove the `_squad_mode_str: Option<&str>` parameter from `run_squad_execution`. This function will be fully replaced in Task 10, but clean it up now so intermediate compilation works.

- [ ] **Step 3: Remove `squad_mode` from agent loop**

In `crates/agent/src/agent_loop/mod.rs`, find where `routing_ctx.squad_mode` is set from `session.squad_mode` or similar — remove it.

- [ ] **Step 4: Fix all compilation errors**

Run: `cargo build --workspace`
Grep for any remaining `squad_mode` references and remove them. Expected sources: `agent_loop/mod.rs`, `runtime.rs`, `routing.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/tools-core/src/routing.rs crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_loop/mod.rs
git commit -m "refactor(squad): remove dead squad_mode from RoutingContext and runtime"
```

---

### Task 8: Domain Events — Squad Debate Completed + Interaction Pattern

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add squad domain events**

Add to the `DomainEvent` enum in `crates/bus/src/domain_events.rs`:

```rust
SquadDebateCompleted {
    squad_id: String,
    session_key: String,
    rounds_completed: u8,
    consensus_score: f64,
    persona_accuracies: Vec<(String, f64)>, // (persona_id, consensus_alignment)
    was_partial: bool,
    token_cost: u64,
    average_consensus_score: f64,
    top_performer_persona_id: Option<String>,
},
SquadInteractionPattern {
    squad_id: String,
    mode: String, // "debate" | "direct" | "lead"
    persona_id: Option<String>,
    domain_hint: Option<String>,
},
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p bus`
Expected: compiles (enum variants are data-only, no match arms needed yet)

- [ ] **Step 3: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(squad): add SquadDebateCompleted and SquadInteractionPattern domain events"
```

---

### Task 9: Unified Debate Engine — `run_debate`

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/debate.rs`

This is the core rewrite. The existing `debate.rs` has the right 4-phase structure but needs to be refactored to:
1. Accept `DebateConfig` + `DebateContext` instead of raw params
2. Return `DebateResult` with full transcript instead of `DebateRounds` type alias
3. Add budget pre-check (Step 1)
4. Load FSRS-5 learned weights (Step 2)
5. Use `DebateEvent` stream instead of `AgentEvent` directly
6. Track `TokenUsage`
7. Graceful degradation on timeout
8. Post-synthesis consensus scoring (Step 6)
9. Blackboard cleanup (Step 7)

- [ ] **Step 1: Write integration test for `run_debate`**

Create or add to the test module in `debate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_pipeline::engines::debate_types::*;

    #[tokio::test]
    async fn test_run_debate_basic_flow() {
        // Setup: in-memory pool, seed personas, create squad, mock provider
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // ... run cognitive migrations ...
        let blackboard = BlackboardRepo::new(pool.sqlite().clone());
        let accuracy = PersonaAccuracyRepo::new(pool.sqlite().clone());
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
        let cancel = CancellationToken::new();

        let config = DebateConfig::default();
        let context = DebateContext {
            skill_prompt: String::new(),
            conversation_history: vec![],
            user_message: "What should I invest in?".into(),
            cognitive_context: None,
            domains: vec!["finance".into()],
        };

        // Use a mock/noop provider that returns canned responses
        let provider = create_test_provider();
        let squad = create_test_squad(&pool).await;

        let result = run_debate(
            config, context, squad, &provider,
            &blackboard, &accuracy, event_tx, None, cancel,
        ).await.unwrap();

        assert!(result.rounds_completed >= 1);
        assert!(!result.synthesis.is_empty());
        assert!(!result.persona_responses.is_empty());
        assert!(!result.rounds.is_empty());
        assert!(result.token_usage.total > 0);
    }
}
```

- [ ] **Step 2: Refactor the public API**

Replace the current `pub async fn run_room_debate(...)` with the new signature:

```rust
pub async fn run_debate(
    config: DebateConfig,
    context: DebateContext,
    squad: ResolvedSquad,
    provider: &DynProvider,
    blackboard_repo: &BlackboardRepo,
    accuracy_repo: &PersonaAccuracyRepo,
    event_tx: mpsc::Sender<DebateEvent>,
    approval_rx: Option<oneshot::Receiver<bool>>,
    cancel: CancellationToken,
) -> common::Result<DebateResult> {
```

Keep the internal helper functions (`fan_out_parallel`, `run_sequential_round`, `call_judge`, `build_phase_prompt`) largely intact but adapt them to:
- Accept `&DebateConfig` for temperature/confidence
- Return `Vec<PersonaResponse>` instead of `Vec<(String, String, String)>`
- Emit `DebateEvent` instead of `AgentEvent`
- Track token counts from `provider.chat()` `Usage` returns

- [ ] **Step 3: Implement Step 1 — Budget pre-check**

At the top of `run_debate`, before the debate loop:

```rust
// Step 1: Budget pre-check
if let Some(ref budget) = config.token_budget {
    let estimated = squad.personas.len() as u64
        * config.max_rounds as u64
        * budget.estimated_tokens_per_round;

    if estimated > budget.remaining_monthly {
        // Calculate reduced rounds
        let affordable_rounds = (budget.remaining_monthly / (squad.personas.len() as u64 * budget.estimated_tokens_per_round)) as u8;
        if affordable_rounds >= 2 {
            effective_max_rounds = affordable_rounds;
            let _ = event_tx.send(DebateEvent::BudgetCheck {
                estimated_cost: estimated,
                remaining: budget.remaining_monthly,
                action: BudgetAction::ReducedRounds { from: config.max_rounds, to: affordable_rounds },
            }).await;
        } else {
            let _ = event_tx.send(DebateEvent::BudgetCheck {
                estimated_cost: estimated,
                remaining: budget.remaining_monthly,
                action: BudgetAction::RequiresApproval,
            }).await;
            // Wait for approval
            if let Some(rx) = approval_rx {
                match rx.await {
                    Ok(true) => { /* proceed */ },
                    _ => return Ok(DebateResult::empty_cancelled()),
                }
            }
        }
    } else {
        let _ = event_tx.send(DebateEvent::BudgetCheck {
            estimated_cost: estimated,
            remaining: budget.remaining_monthly,
            action: BudgetAction::Proceed,
        }).await;
    }
}
```

- [ ] **Step 4: Implement Step 2 — Load learned weights**

```rust
// Step 2: Load learned weights
let mut learned_weights = Vec::new();
let primary_domain = context.domains.first().map(|s| s.as_str()).unwrap_or("general");

for persona in &squad.personas {
    let accuracy_row = accuracy_repo
        .get(&persona.id, &squad.squad.id, primary_domain)
        .await
        .unwrap_or(None);

    let base = persona.relevance_score;
    let (accuracy_score, debates_used) = match &accuracy_row {
        Some(row) => ((row.stability / 10.0).min(1.0), row.total_debates as u32),
        None => (0.5, 0),
    };
    let confidence_in_score = (debates_used as f32 / 20.0).min(1.0);
    let effective_blend = config.accuracy_blend * confidence_in_score;
    let blended = (1.0 - effective_blend as f64) * base + effective_blend as f64 * accuracy_score;

    learned_weights.push(LearnedWeight {
        persona_id: persona.id.clone(),
        base_weight: base,
        accuracy_weight: accuracy_score,
        blended_weight: blended,
        domain: primary_domain.to_string(),
        debates_used,
    });
}
```

- [ ] **Step 5: Implement Step 3 — Build persona system prompts**

Migrate `build_persona_prompt` from `squad.rs` and extend with `skill_prompt` and `cognitive_context`:

```rust
fn build_persona_system_prompt(
    persona: &PersonaRow,
    skill_prompt: &str,
    cognitive_context: Option<&str>,
    user_message: &str,
) -> String {
    let mut prompt = String::new();

    if !skill_prompt.is_empty() {
        prompt.push_str(skill_prompt);
        prompt.push_str("\n\n---\n\n");
    }

    prompt.push_str(&format!(
        "You are {}, a {}.\n\
         Expertise: {}\n\
         Perspective: {}\n\
         Tone: {}\n\
         Questioning style: {}\n\
         Cognitive bias: {}\n\
         Analysis frameworks: {}\n",
        persona.name, persona.role, persona.expertise,
        persona.perspective, persona.tone, persona.questioning_style,
        persona.cognitive_bias, persona.analysis_frameworks,
    ));

    if let Some(ctx) = cognitive_context {
        prompt.push_str("\n\n--- Context ---\n\n");
        prompt.push_str(ctx);
    }

    prompt.push_str("\n\n--- User Message ---\n\n");
    prompt.push_str(user_message);

    prompt
}
```

- [ ] **Step 6: Implement Step 4 — Debate loop with transcript capture**

Refactor the main loop to:
- Use `DebateEvent` instead of `AgentEvent`
- Capture each round as a `DebateRound` in a `Vec<DebateRound>`
- Use `effective_max_rounds` from budget check
- Sort personas by `blended_weight` for opening round
- Use judge `speaking_order` for subsequent rounds
- Track `TokenUsage` by summing `Usage` from each `provider.chat()` call
- Wrap in `tokio::time::timeout(Duration::from_secs(config.timeout_seconds), ...)`
- On timeout: emit `DebatePartial { reason: PartialReason::Timeout }`, proceed to synthesis with partial results

The internal structure stays similar to the current code but with the new types flowing through.

- [ ] **Step 7: Implement Step 5 — Synthesis**

Migrate `build_squad_synthesis_prompt` from `squad.rs` and extend:

```rust
fn build_synthesis_prompt(
    responses: &[PersonaResponse],
    learned_weights: &[LearnedWeight],
    config: &DebateConfig,
    partial_reason: Option<&PartialReason>,
) -> String {
    let mut prompt = String::from("You are synthesizing a multi-persona debate.\n\n");

    if let Some(reason) = partial_reason {
        prompt.push_str(&format!("Note: debate ended early — {:?}\n\n", reason));
    }

    match config.output_mode {
        OutputMode::Synthesized => {
            prompt.push_str("Integrate the strongest points into a single coherent response. ");
            prompt.push_str("Resolve contradictions by explaining trade-offs. ");
            prompt.push_str("Give actionable recommendations. Credit specific personas.\n\n");
        }
        OutputMode::StructuredPerPersona => {
            prompt.push_str("Preserve each persona's section with ## {Name} — {Role} headers. ");
            prompt.push_str("Add a brief synthesis section at the top highlighting key agreements and tensions.\n\n");
        }
    }

    for resp in responses {
        let weight = learned_weights.iter()
            .find(|w| w.persona_id == resp.persona_id)
            .map(|w| w.blended_weight)
            .unwrap_or(0.5);
        prompt.push_str(&format!(
            "### {} ({}) [weight: {:.2}]\n{}\n\n",
            resp.persona_name, resp.persona_role, weight, resp.content
        ));
    }

    prompt
}
```

Use `provider.chat_stream()` for streaming synthesis, emitting `DebateEvent::SynthesisChunk` for each chunk.

- [ ] **Step 8: Implement Step 6 — Post-synthesis consensus scoring**

After synthesis, make a separate lightweight LLM call:

```rust
async fn score_consensus_alignment(
    provider: &DynProvider,
    synthesis: &str,
    final_responses: &[PersonaResponse],
    token_usage: &mut TokenUsage,
) -> Vec<(String, f64)> {
    let prompt = format!(
        "Rate 0.0-1.0 how much each persona's core position was reflected in the synthesis.\n\
         Synthesis:\n{}\n\nPersona positions:\n{}",
        synthesis,
        final_responses.iter()
            .map(|r| format!("{}: {}", r.persona_name, &r.content[..r.content.len().min(200)]))
            .collect::<Vec<_>>().join("\n")
    );

    let params = ChatParams {
        temperature: Some(0.0),
        max_tokens: Some(300),
        ..Default::default()
    };

    // Parse JSON response: {"PersonaName": 0.82, ...}
    // Return Vec<(persona_id, alignment_score)>
    // On parse failure, return 0.5 for all (neutral default)
}
```

- [ ] **Step 9: Implement Step 7 — Blackboard cleanup**

At the end of `run_debate`, after building the result:

```rust
// Step 7: Clean up debate-scoped blackboard entries
blackboard_repo.clear_session(&debate_session_key).await.ok();
```

- [ ] **Step 10: Run tests**

Run: `cargo nextest run -p agent -E 'test(debate)'`
Expected: all debate tests PASS

- [ ] **Step 11: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/debate.rs
git commit -m "feat(squad): rewrite debate engine with DebateConfig/Result, FSRS-5 weights, budget gate, graceful degradation"
```

---

### Task 10: Delete Fan-Out Engine

**Files:**
- Delete: `crates/agent/src/intent_pipeline/engines/squad.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Remove `pub mod squad` from `engines/mod.rs`**

Delete the line `pub mod squad;` from `crates/agent/src/intent_pipeline/engines/mod.rs`.

- [ ] **Step 2: Delete `squad.rs`**

Remove `crates/agent/src/intent_pipeline/engines/squad.rs` entirely.

- [ ] **Step 3: Remove `run_squad_execution` from `runtime.rs`**

Delete the entire `run_squad_execution` method from `AgentRuntime`. Also remove:
- The `SquadDeps` struct definition (will be replaced with new deps in Task 12)
- The `with_squad_deps` builder method
- All `use` imports that reference `squad::*`
- The squad detection check at L405-L418 (`if ctx.squad_id.is_some() && self.squad_deps.is_some()`)

Leave a `// TODO: squad routing moved to chat handler` comment at the location where the check was.

- [ ] **Step 4: Fix compilation**

Run: `cargo build -p agent`
Fix any remaining references to `squad::*`, `SquadDeps`, `run_squad_execution`, or `with_squad_deps`. Key locations:
- `crates/agent/src/agent_loop/builder.rs` — remove `SquadDeps` wiring (L1561-L1573)
- `crates/agent/src/agent_loop/mod.rs` — remove any `squad_deps` references

- [ ] **Step 5: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: all PASS (squad tests that relied on `run_squad_execution` should have been updated or removed in Task 9)

- [ ] **Step 6: Commit**

```bash
git add -A crates/agent/src/
git commit -m "refactor(squad): delete fan-out engine, remove SquadDeps from runtime"
```

---

### Task 11: Interaction Mode Detection

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/interaction.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`

- [ ] **Step 1: Write tests for interaction mode detection**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_personas() -> Vec<PersonaRow> {
        vec![
            PersonaRow { id: "p1".into(), name: "Skeptic".into(), ..default_persona() },
            PersonaRow { id: "p2".into(), name: "Strategist".into(), ..default_persona() },
            PersonaRow { id: "p3".into(), name: "Deep Analyst".into(), ..default_persona() },
        ]
    }

    #[test]
    fn test_detect_group_trigger() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode("everyone, what do you think?", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::Debate,
        );
        assert_eq!(
            detect_interaction_mode("team, weigh in on this", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::Debate,
        );
    }

    #[test]
    fn test_detect_direct_address_at_sign() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode("@Skeptic what about taxes?", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::DirectAddress { persona_id: "p1".into() },
        );
    }

    #[test]
    fn test_detect_direct_address_comma() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode("Skeptic, what about taxes?", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::DirectAddress { persona_id: "p1".into() },
        );
    }

    #[test]
    fn test_detect_multi_word_name() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode("Deep Analyst, break this down", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::DirectAddress { persona_id: "p3".into() },
        );
    }

    #[test]
    fn test_no_substring_match() {
        let personas = test_personas();
        // "skeptical" should NOT match "Skeptic"
        assert_eq!(
            detect_interaction_mode("Can you be more skeptical about this?", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::LeadResponse,
        );
    }

    #[test]
    fn test_fallback_to_default() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode("What do you think about this?", &personas, DefaultInteractionMode::Lead),
            SquadInteractionMode::LeadResponse,
        );
        assert_eq!(
            detect_interaction_mode("What do you think about this?", &personas, DefaultInteractionMode::Debate),
            SquadInteractionMode::Debate,
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(detect_interaction)'`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement `detect_interaction_mode`**

```rust
use super::debate_types::*;
use cognitive::PersonaRow;

const GROUP_TRIGGERS: &[&str] = &["everyone", "squad", "team", "all of you"];

pub fn detect_interaction_mode(
    message: &str,
    personas: &[PersonaRow],
    default: DefaultInteractionMode,
) -> SquadInteractionMode {
    let msg_lower = message.to_lowercase();
    let msg_trimmed = msg_lower.trim();

    // 1. Check for group triggers at message start
    for trigger in GROUP_TRIGGERS {
        if msg_trimmed.starts_with(trigger) {
            return SquadInteractionMode::Debate;
        }
    }

    // 2. Check for @PersonaName anywhere
    for persona in personas {
        let at_name = format!("@{}", persona.name.to_lowercase());
        if msg_lower.contains(&at_name) {
            return SquadInteractionMode::DirectAddress {
                persona_id: persona.id.clone(),
            };
        }
    }

    // 3. Check for "PersonaName," or "PersonaName:" at message start (first word(s))
    for persona in personas {
        let name_lower = persona.name.to_lowercase();
        let comma_pattern = format!("{},", name_lower);
        let colon_pattern = format!("{}:", name_lower);
        if msg_trimmed.starts_with(&comma_pattern) || msg_trimmed.starts_with(&colon_pattern) {
            return SquadInteractionMode::DirectAddress {
                persona_id: persona.id.clone(),
            };
        }
    }

    // 4. Fallback to default mode
    match default {
        DefaultInteractionMode::Lead => SquadInteractionMode::LeadResponse,
        DefaultInteractionMode::Debate => SquadInteractionMode::Debate,
        DefaultInteractionMode::Smart => {
            // Smart mode requires interaction history — caller should resolve before calling.
            // Default to Lead if Smart wasn't pre-resolved.
            SquadInteractionMode::LeadResponse
        }
    }
}
```

- [ ] **Step 4: Implement `run_direct_address`**

```rust
pub async fn run_direct_address(
    persona: &PersonaRow,
    context: &DebateContext,
    config: &DebateConfig,
    provider: &DynProvider,
    history: &[Message],
) -> common::Result<DirectAddressResult> {
    let system_prompt = super::debate::build_persona_system_prompt(
        persona,
        &context.skill_prompt,
        context.cognitive_context.as_deref(),
        &context.user_message,
    );

    let mut messages = vec![Message::system(&system_prompt)];
    messages.extend_from_slice(history);
    messages.push(Message::user(&context.user_message));

    let params = ChatParams {
        temperature: config.temperature_override,
        max_tokens: Some(4096),
        ..Default::default()
    };

    let response = provider.chat(&messages, &params).await?;
    let content = response.content.unwrap_or_default();
    let tokens = response.usage.map(|u| u.total_tokens as u64).unwrap_or(0);

    let mut token_usage = TokenUsage::default();
    token_usage.add_persona(tokens);

    Ok(DirectAddressResult {
        response: PersonaResponse {
            persona_id: persona.id.clone(),
            persona_name: persona.name.clone(),
            persona_icon: persona.icon.clone(),
            persona_role: persona.role.clone(),
            content,
            round: 0,
            phase: DebatePhase::Opening,
            confidence: 1.0,
            effective_confidence: 1.0,
            consensus_alignment: None,
        },
        token_usage,
        context_used: vec![
            if !context.skill_prompt.is_empty() { "orchestrator_skill" } else { "none" }.into(),
        ],
    })
}
```

- [ ] **Step 5: Export from `engines/mod.rs`**

```rust
pub mod interaction;
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(detect_interaction)' -E 'test(direct_address)'`
Expected: all PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/interaction.rs crates/agent/src/intent_pipeline/engines/mod.rs
git commit -m "feat(squad): add interaction mode detection and direct address execution"
```

---

### Task 12: Chat Caller Integration

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`
- Modify: `crates/app-core/src/state.rs` (if needed for new deps)

This is where the chat handler routes squad messages through the new engine. The existing `chat_send` extracts `squad_id` — now it also needs to detect interaction mode and branch to either `run_debate` or `run_direct_address`.

- [ ] **Step 1: Add squad deps to `AppCore`**

In `crates/app-core/src/state.rs`, ensure `AppCore` has access to `PersonaAccuracyRepo` (it may already have it via `cognitive_repos` or similar). If not, add it as `Option<PersonaAccuracyRepo>`.

- [ ] **Step 2: Add squad execution helper to streaming.rs**

Create a new function `execute_squad_message` that encapsulates the full squad routing:

```rust
async fn execute_squad_message(
    core: &AppCore,
    content: &str,
    session_key: &str,
    squad_id: &str,
    emitter: &dyn EventEmitter,
) -> common::Result<()> {
    let squad_repo = core.squad_repo()?;
    let resolved = squad_repo.resolve_squad(squad_id).await?
        .ok_or_else(|| ApiError::new("NOT_FOUND", "Squad not found"))?;

    let default_mode = DefaultInteractionMode::from_str(&resolved.squad.default_interaction_mode);
    let mode = detect_interaction_mode(content, &resolved.personas, default_mode);

    // Load orchestrator skill for domain grounding
    let skill_prompt = core.skill_catalog()
        .and_then(|c| c.get_skill(&resolved.squad.orchestrator_skill))
        .map(|s| s.body.clone())
        .unwrap_or_default();

    let context = DebateContext {
        skill_prompt,
        conversation_history: load_session_history(core, session_key).await?,
        user_message: content.to_string(),
        cognitive_context: None,
        domains: serde_json::from_str(&resolved.squad.domains).unwrap_or_default(),
    };

    match mode {
        SquadInteractionMode::Debate => {
            let config = DebateConfig::for_chat();
            // ... setup event relay, call run_debate, persist results ...
        }
        SquadInteractionMode::DirectAddress { persona_id } => {
            let persona = resolved.personas.iter()
                .find(|p| p.id == persona_id)
                .ok_or_else(|| ApiError::new("NOT_FOUND", "Persona not in squad"))?;
            // ... call run_direct_address, persist result ...
        }
        SquadInteractionMode::LeadResponse => {
            let lead = resolved.members.iter()
                .find(|m| m.role_in_squad == "lead")
                .and_then(|m| resolved.personas.iter().find(|p| p.id == m.persona_id))
                .or_else(|| resolved.personas.first())
                .ok_or_else(|| ApiError::new("NOT_FOUND", "No lead persona"))?;
            // ... call run_direct_address with lead persona ...
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Wire into `chat_send`**

In `chat_send`, after extracting `squad_id`, replace the existing `process_direct_streaming` call when `squad_id` is present:

```rust
if let Some(squad_id) = context.squad_id.as_deref() {
    // Squad mode: route through unified debate engine
    return execute_squad_message(core, &content, &session_key, squad_id, &emitter).await;
}
// ... else: normal non-squad path continues ...
```

- [ ] **Step 4: Add debate event relay**

Map `DebateEvent` to Tauri events in the event relay:

```rust
async fn relay_debate_events(
    mut event_rx: mpsc::Receiver<DebateEvent>,
    emitter: &dyn EventEmitter,
) {
    while let Some(event) = event_rx.recv().await {
        match event {
            DebateEvent::PersonaPerspective { persona_id, name, icon, role, content, round, challenge } => {
                emitter.emit("agent:persona_perspective", &serde_json::json!({
                    "personaId": persona_id, "name": name, "icon": icon,
                    "role": role, "content": content, "round": round,
                    "challenge": challenge,
                }));
            }
            DebateEvent::RoundStarted { round, phase, persona_order, order_reason } => {
                emitter.emit("agent:debate_round_started", &serde_json::json!({
                    "round": round, "phase": phase,
                    "personaOrder": persona_order, "orderReason": order_reason,
                }));
            }
            DebateEvent::SynthesisChunk { content } => {
                emitter.emit("agent:content_chunk", &serde_json::json!({ "content": content }));
            }
            DebateEvent::Complete => {
                emitter.emit("agent:done", &serde_json::json!({}));
                break;
            }
            // ... handle remaining event variants ...
        }
    }
}
```

- [ ] **Step 5: Add post-processing — persist results, record accuracy, emit domain events**

After `run_debate` returns:

```rust
// Persist synthesis as session message
let message_metadata = serde_json::json!({
    "squad_mode": "debate",
    "squad_id": squad_id,
    "debate_rounds": result.rounds_completed,
    "partial": result.partial_reason.is_some(),
    "partial_reason": result.partial_reason,
});
// ... persist via session repo ...

// Record token usage
if let Some(cost_tracker) = core.cost_tracker() {
    cost_tracker.record(result.token_usage.total).await;
}

// Persist accuracy outcomes
if let Some(accuracy_repo) = core.persona_accuracy_repo() {
    for outcome in &result.accuracy_outcomes {
        accuracy_repo.record_outcome(
            &outcome.persona_id, &outcome.squad_id,
            &outcome.domain, outcome.consensus_alignment,
        ).await.ok();
    }
}

// Emit domain event
if let Some(bus) = core.event_bus() {
    bus.publish(DomainEvent::SquadDebateCompleted {
        squad_id: squad_id.to_string(),
        session_key: session_key.to_string(),
        rounds_completed: result.rounds_completed,
        consensus_score: result.final_consensus_score,
        persona_accuracies: result.accuracy_outcomes.iter()
            .map(|o| (o.persona_id.clone(), o.consensus_alignment))
            .collect(),
        was_partial: result.partial_reason.is_some(),
        token_cost: result.token_usage.total,
        average_consensus_score: result.final_consensus_score,
        top_performer_persona_id: result.accuracy_outcomes.iter()
            .max_by(|a, b| a.consensus_alignment.partial_cmp(&b.consensus_alignment).unwrap())
            .map(|o| o.persona_id.clone()),
    });
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build -p app-core -p desktop`
Expected: compiles cleanly

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs crates/app-core/src/state.rs
git commit -m "feat(squad): integrate unified debate engine into chat handler with interaction mode routing"
```

---

### Task 13: Notes Caller Integration

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`
- Modify: `crates/app-core/src/handlers/notes/insight_chat.rs`
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs`

Replace the parallel per-persona calls in `run_insight_pipeline` with `run_debate`. Delete `relay_squad_chat`, `perspectives_prompt`, `single_persona_prompt`.

- [ ] **Step 1: Replace `run_insight_pipeline` perspectives generation**

In `insight.rs`, find where `single_persona_prompt` futures are created and `join_all`'d. Replace with:

```rust
// Build DebateConfig for notes
let debate_config = DebateConfig::for_notes();

let debate_context = DebateContext {
    skill_prompt: load_orchestrator_skill(core, &resolved_squad).await,
    conversation_history: vec![],
    user_message: format!("{}\n\n{}", note.title, note.body),
    cognitive_context: Some(context_text.clone()),
    domains: note_domains.clone(),
};

let (debate_event_tx, mut debate_event_rx) = mpsc::channel(100);
let cancel = CancellationToken::new();

// Relay debate events to insight-specific Tauri events
tokio::spawn(async move {
    while let Some(event) = debate_event_rx.recv().await {
        match event {
            DebateEvent::PersonaPerspective { persona_id, name, icon, role, content, round, .. } => {
                emitter.emit("insight:persona-perspective", &serde_json::json!({
                    "personaId": persona_id, "name": name, "icon": icon,
                    "role": role, "content": content,
                }));
            }
            DebateEvent::SynthesisChunk { content } => {
                emitter.emit("insight:synthesis-chunk", &serde_json::json!({ "content": content }));
            }
            DebateEvent::RoundStarted { round, phase, .. } => {
                emitter.emit("insight:round-started", &serde_json::json!({ "round": round, "phase": phase }));
            }
            DebateEvent::Complete => {
                emitter.emit("insight:tab-done", &serde_json::json!({ "tab": "perspectives" }));
                break;
            }
            _ => {}
        }
    }
});

let result = run_debate(
    debate_config, debate_context, resolved_squad, provider,
    blackboard_repo, accuracy_repo, debate_event_tx, None, cancel,
).await?;

// Store perspectives from debate result
let perspectives = result.persona_responses.iter()
    .map(|r| format!("## {} — {}\n\n{}", r.persona_name, r.persona_role, r.content))
    .collect::<Vec<_>>()
    .join("\n\n---\n\n");

// Store debate_transcript JSON
let debate_transcript = serde_json::to_string(&result.rounds).ok();
```

- [ ] **Step 2: Update `note_insight_regenerate_tab` for "perspectives"**

Replace the `perspectives_prompt` single-call path with `run_debate`, same as initial generation. This eliminates the inconsistency (gap #9).

- [ ] **Step 3: Delete obsolete functions from `insight_prompts.rs`**

Remove `perspectives_prompt()` and `single_persona_prompt()`. Keep other prompt functions (`gaps_prompt`, `assessment_prompt`, etc.) that are still used for non-perspectives tabs.

- [ ] **Step 4: Delete `relay_squad_chat` from `insight_chat.rs`**

Remove the `relay_squad_chat` function. Replace its usage in `note_insight_tab_chat` with the debate engine (for full squad tabs) or `run_direct_address` (for persona-specific tabs).

- [ ] **Step 5: Store `debate_transcript` in `InsightReviewRow`**

Update the `store_insight` call to include the transcript:

```rust
// In InsightService or the storage call
insight_repo.insert_with_transcript(
    note_id, &content_json, &content_hash,
    &scope_config, &persona_ids, debate_transcript.as_deref(),
).await?;
```

Add the `debate_transcript` parameter to the insert query in the insight repo.

- [ ] **Step 6: Persist accuracy outcomes**

Same pattern as chat caller (Task 12, Step 5) — loop through `result.accuracy_outcomes` and call `accuracy_repo.record_outcome()`.

- [ ] **Step 7: Verify compilation**

Run: `cargo build -p app-core`
Expected: compiles cleanly

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs crates/app-core/src/handlers/notes/insight_chat.rs crates/app-core/src/handlers/notes/insight_prompts.rs
git commit -m "feat(squad): integrate unified debate engine into notes insight pipeline, delete obsolete prompts"
```

---

### Task 14: Thread Listing Fix — Squad Name/Icon JOIN

**Files:**
- Modify: `crates/app-core/src/handlers/chat/sessions.rs`
- Modify: `crates/desktop-shared/src/commands/chat.rs` (if `ChatThreadResponse` is defined here)

- [ ] **Step 1: Write test**

```rust
#[tokio::test]
async fn test_thread_list_includes_squad_name() {
    // Create a session with squad_id, then list sessions
    // Assert squad_name and squad_icon are populated
}
```

- [ ] **Step 2: Update the SQL query to LEFT JOIN squads**

In `chat_list_sessions_by_project`, change:

```sql
SELECT s.*, sq.name as squad_name, sq.icon as squad_icon, sq.default_interaction_mode as squad_default_mode
FROM sessions s
LEFT JOIN squads sq ON s.squad_id = sq.id
WHERE s.project_id = ?
ORDER BY s.updated_at DESC
```

- [ ] **Step 3: Populate the response fields**

Replace the hardcoded `None` values:

```rust
squad_name: row.get::<Option<String>, _>("squad_name"),
squad_icon: row.get::<Option<String>, _>("squad_icon"),
squad_default_mode: row.get::<Option<String>, _>("squad_default_mode"),
```

Add `squad_default_mode: Option<String>` to `ChatThreadResponse` if not present.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p app-core -E 'test(thread_list)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/chat/sessions.rs crates/desktop-shared/src/commands/chat.rs
git commit -m "fix(squad): populate squad_name/icon/default_mode in thread listings via LEFT JOIN"
```

---

### Task 15: Blackboard Cleanup Cron

**Files:**
- Modify: `crates/app-core/src/init/cognitive.rs` (or wherever cron jobs are registered)

- [ ] **Step 1: Add `JOB_BLACKBOARD_CLEANUP` cron job**

```rust
pub const JOB_BLACKBOARD_CLEANUP: &str = "mirror_blackboard_cleanup";

// Register in the cron setup, alongside existing mirror cleanup jobs
scheduler.register(
    JOB_BLACKBOARD_CLEANUP,
    "0 0 4 * * 0", // Sunday 4am UTC
    move |_| {
        let blackboard = blackboard_repo.clone();
        async move {
            let cutoff = chrono::Duration::days(30);
            match blackboard.cleanup_stale(cutoff).await {
                Ok(count) => tracing::info!("Blackboard cleanup: removed {} stale entries", count),
                Err(e) => tracing::warn!("Blackboard cleanup failed: {}", e),
            }
        }
    },
);
```

- [ ] **Step 2: Verify registration**

Run: `cargo build -p app-core`
Expected: compiles — the cron job is registered alongside existing mirror cron jobs.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/cognitive.rs
git commit -m "feat(squad): add weekly blackboard cleanup cron job (Sunday 4am UTC, 30-day retention)"
```

---

### Task 16: Smart Mode Learning

**Files:**
- Modify: `crates/cognitive/src/repos/squad.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` (or wherever Smart mode resolution happens)

- [ ] **Step 1: Write test for Smart mode detection**

```rust
#[tokio::test]
async fn test_smart_mode_detection() {
    // Create a squad with default_interaction_mode = "smart"
    // Simulate 10 direct-address interactions (by inserting session messages with persona_id)
    // Call detect_smart_default() — should return LeadResponse (>60% direct)
}
```

- [ ] **Step 2: Add `update_smart_cache` to SquadRepo**

```rust
pub async fn update_smart_cache(
    &self,
    squad_id: &str,
    mode: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE squads SET last_smart_mode = ?2, last_smart_updated = datetime('now'), updated_at = datetime('now') WHERE id = ?1"
    )
    .bind(squad_id)
    .bind(mode)
    .execute(&*self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 3: Implement `detect_smart_default`**

In the interaction module or a new helper:

```rust
pub async fn detect_smart_default(
    squad_id: &str,
    squad_repo: &SquadRepo,
    session_repo: &SessionRepo,
) -> DefaultInteractionMode {
    // Check cache first
    if let Ok(Some(squad)) = squad_repo.get(squad_id).await {
        if let Some(ref cached) = squad.last_smart_mode {
            if let Some(ref updated) = squad.last_smart_updated {
                // If cache is less than 1 day old, use it
                if is_recent(updated, chrono::Duration::days(1)) {
                    return DefaultInteractionMode::from_str(cached);
                }
            }
        }
    }

    // Query interaction history (last 30 days)
    // Count messages by mode type using session_messages
    // ... (SQL query from spec) ...

    let total = direct_count + lead_count + debate_count;
    if total < 5 {
        return DefaultInteractionMode::Lead;
    }

    let mode = if direct_count as f64 / total as f64 > 0.6 {
        DefaultInteractionMode::Lead
    } else if debate_count as f64 / total as f64 > 0.5 {
        DefaultInteractionMode::Debate
    } else {
        DefaultInteractionMode::Lead
    };

    // Cache the result
    squad_repo.update_smart_cache(squad_id, mode.as_str()).await.ok();

    mode
}
```

- [ ] **Step 4: Wire into interaction mode detection**

In the chat caller (Task 12), when `default_mode == Smart`, call `detect_smart_default` before `detect_interaction_mode`:

```rust
let effective_default = if default_mode == DefaultInteractionMode::Smart {
    detect_smart_default(&squad_id, squad_repo, session_repo).await
} else {
    default_mode
};
let mode = detect_interaction_mode(content, &resolved.personas, effective_default);
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(smart_mode)' && cargo nextest run -p app-core -E 'test(smart_mode)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/squad.rs crates/app-core/src/handlers/chat/streaming.rs crates/agent/src/intent_pipeline/engines/interaction.rs
git commit -m "feat(squad): implement Smart mode learning with cached interaction history"
```

---

### Task 17: Final Verification — Full Workspace Build and Test

**Files:** None (verification only)

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any that appear)

- [ ] **Step 2: Run format check**

Run: `cargo fmt --all --check`
Expected: no formatting issues

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: all PASS

- [ ] **Step 4: Run doctests**

Run: `cargo test --workspace --doc`
Expected: all PASS

- [ ] **Step 5: Verify deleted files are gone**

Confirm these files/functions no longer exist:
- `crates/agent/src/intent_pipeline/engines/squad.rs` — deleted
- `RoutingContext.squad_mode` — removed
- `SessionContextInput.squad_mode` — removed
- `run_squad_execution` — removed
- `SquadDeps` — removed
- `perspectives_prompt` — removed
- `single_persona_prompt` — removed
- `relay_squad_chat` — removed

- [ ] **Step 6: Commit any final fixes**

```bash
git add -A && git commit -m "chore(squad): final cleanup and verification"
```
