# KCA Phase B — Continuous Learning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Klynt's memory learn *during* a session — not 24h later. Add (4) a mid-session "micro-Reforge" promotion of stable patterns to procedural rules, (5) an async critic loop that catches hallucinated extractions before they leak into retrieval, and (11) online community membership so new facts cluster same-session instead of waiting for the nightly Louvain run.

**Architecture:** Three independent tracks sharing a common pattern — a fresh `ServiceBuilder`-style service in `cognitive::services::`, a domain trait + LLM impl in `agent::adapters::`, a Mirror table to record the service's verdicts, and a cron or event-bus subscriber to drive it. Track 4 is timer-driven (every 10 turns OR 30 minutes); Track 5 is fire-and-forget after every extraction; Track 11 fires on `SessionEnded`.

**Tech Stack:** Rust stable 1.93, `cargo-nextest`, `tokio`, `tokio-util` (CancellationToken), `DynProvider`, `petgraph` (Track 11), `MirrorFacade` (existing), `cron` (existing).

**Spec:** [`docs/superpowers/specs/2026-04-29-klynt-cognitive-architecture-design.md`](../specs/2026-04-29-klynt-cognitive-architecture-design.md), §4 (Tier 2 warm path + critic ring), §5 (Tracks 4, 5, 11), §10 risk register entries for Tracks 4/5/11.

**Prerequisite:** Phase A merged. Tracks 1+2 (graph linker, neighborhood prefetch) and Track 9-typing (EdgeType) are required by Track 11.

---

## File Structure

**Track 4 — Micro-Reforge**
- Create: `crates/cognitive/src/services/micro_reforge.rs`
- Create: `crates/cognitive/src/services/micro_reforge_types.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmMicroReforgeHandler`)
- Modify: `crates/agent/src/adapters/prompts.rs` (`MICRO_REFORGE_SYSTEM_PROMPT`)
- Modify: `crates/app-core/src/init/cron.rs` (register the micro-Reforge cron)
- Modify: `crates/config/src/schema/cognitive.rs` (`MicroReforgeConfig`)
- Create: `crates/cognitive/migrations/011_micro_reforge_state.sql`

**Track 5 — Self-critique loop**
- Create: `crates/cognitive/src/services/extraction_critic.rs`
- Create: `crates/cognitive/src/services/extraction_critic_types.rs`
- Modify: `crates/cognitive/src/services/background.rs` (call critic after extraction)
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmExtractionCriticHandler`)
- Modify: `crates/agent/src/adapters/prompts.rs` (`EXTRACTION_CRITIC_SYSTEM_PROMPT`)
- Modify: `crates/agent/src/agent_loop/builder.rs` (handler wiring)
- Create: `crates/cognitive/migrations/012_extraction_critic_log.sql`
- Modify: `crates/cognitive/src/repos/mod.rs` (new `ExtractionCriticLogRepo`)
- Create: `crates/cognitive/src/repos/extraction_critic_log.rs`

**Track 11 — Online community membership**
- Create: `crates/cognitive/src/services/community_membership_online.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/repos/community.rs` (or wherever community ops live; add `find_by_seed_entities`)
- Modify: `crates/coding-memory/src/reforge/session_end.rs` (call online membership)
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmCommunityMembershipHandler` — confirms cluster choice)
- Modify: `crates/agent/src/adapters/prompts.rs` (`COMMUNITY_MEMBERSHIP_SYSTEM_PROMPT`)

**Phase B integration tests**
- Create: `crates/cognitive/tests/phase_b_continuous_learning.rs`
- Create: `crates/cognitive/tests/phase_b_critic_proptest.rs`

---

# Track 4 — Online procedural rule promotion (micro-Reforge)

Reforge nightly synthesizes patterns into `procedural_rules`. Track 4 adds a lighter sister cycle that fires on a timer: every 10 chat turns OR every 30 minutes (whichever first), look at recent episodics and accumulated_observations, and ask an LLM "is there a stable pattern here worth promoting?" If yes, write to `procedural_rules` with `source='reflected_online'`.

### Task B4.1: Define `MicroReforgeInput` / `MicroReforgeOutput` types

**Files:**
- Create: `crates/cognitive/src/services/micro_reforge_types.rs`

- [ ] **Step 1: Create file.**

```rust
//! Types for the micro-Reforge timer (KCA Track 4).
//!
//! Micro-Reforge fires every N turns or every M minutes (whichever first) and
//! synthesizes recent episodics + observations into candidate procedural rules.
//! Conservative: only ADD operations, no UPDATE or DELETE. Nightly Reforge
//! handles refinement and pruning.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroReforgeInput {
    pub recent_episodics: Vec<EpisodicRef>,
    pub recent_observations: Vec<ObservationRef>,
    pub existing_rules_summary: Vec<RuleSummary>,
    pub session_count: u32,
    pub turn_count_since_last_run: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicRef {
    pub id: String,
    pub domain: String,
    pub summary: String,
    pub importance: f64,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRef {
    pub content_truncated: String,
    pub domain: String,
    pub importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MicroReforgeOutput {
    pub proposed_rules: Vec<ProposedRule>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedRule {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub signal_count: u32,
    pub evidence_episodic_ids: Vec<String>,
}
```

- [ ] **Step 2: Register.**

In `crates/cognitive/src/services/mod.rs`:

```rust
pub mod micro_reforge_types;
pub use micro_reforge_types::{
    MicroReforgeInput, MicroReforgeOutput, EpisodicRef, ObservationRef, RuleSummary, ProposedRule,
};
```

- [ ] **Step 3: Build.**

```bash
cargo build -p cognitive
```

Expected: clean.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/micro_reforge_types.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): MicroReforge types (KCA Track 4)"
```

---

### Task B4.2: Define `MicroReforgeHandler` trait + Noop impl

**Files:**
- Create: `crates/cognitive/src/services/micro_reforge.rs`

- [ ] **Step 1: Create.**

```rust
//! Micro-Reforge service (KCA Track 4).

use async_trait::async_trait;
use crate::services::micro_reforge_types::{MicroReforgeInput, MicroReforgeOutput};

#[async_trait]
pub trait MicroReforgeHandler: Send + Sync {
    async fn synthesize(&self, input: MicroReforgeInput) -> common::Result<MicroReforgeOutput>;
}

pub struct NoopMicroReforgeHandler;

#[async_trait]
impl MicroReforgeHandler for NoopMicroReforgeHandler {
    async fn synthesize(&self, _input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
        Ok(MicroReforgeOutput::default())
    }
}
```

- [ ] **Step 2: Register.**

In `services/mod.rs`:

```rust
pub mod micro_reforge;
pub use micro_reforge::{MicroReforgeHandler, NoopMicroReforgeHandler};
```

- [ ] **Step 3: Build + commit.**

```bash
cargo build -p cognitive
git add crates/cognitive/src/services/micro_reforge.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): MicroReforgeHandler trait + Noop (KCA Track 4)"
```

---

### Task B4.3: `MicroReforgeConfig` in config schema

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[test]
    fn micro_reforge_config_defaults() {
        let cfg: CognitiveConfig = serde_json::from_str("{}").unwrap();
        assert!(cfg.micro_reforge.enabled);
        assert_eq!(cfg.micro_reforge.turn_threshold, 10);
        assert_eq!(cfg.micro_reforge.minute_threshold, 30);
        assert_eq!(cfg.micro_reforge.min_confidence, 0.65);
    }

    #[test]
    fn micro_reforge_config_overrides() {
        let json = r#"{
            "microReforge": {
                "enabled": false,
                "turnThreshold": 20,
                "minuteThreshold": 60,
                "minConfidence": 0.8
            }
        }"#;
        let cfg: CognitiveConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.micro_reforge.enabled);
        assert_eq!(cfg.micro_reforge.turn_threshold, 20);
    }
```

- [ ] **Step 2: Run, expect compile error.**

```bash
cargo nextest run -p config -E 'test(micro_reforge_config)'
```

- [ ] **Step 3: Add to `CognitiveConfig`.**

```rust
    /// Micro-Reforge timer config (KCA Track 4).
    #[serde(default)]
    pub micro_reforge: MicroReforgeConfig,
```

Add the struct at the bottom of the file:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicroReforgeConfig {
    /// Enabled by default.
    pub enabled: bool,
    /// Fire after this many chat turns since the last run.
    pub turn_threshold: u32,
    /// Fire after this many minutes since the last run, regardless of turn count.
    pub minute_threshold: u32,
    /// Minimum LLM confidence required to actually write a rule.
    pub min_confidence: f64,
    /// LLM model override (defaults to cognitive.model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for MicroReforgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            turn_threshold: 10,
            minute_threshold: 30,
            min_confidence: 0.65,
            model: None,
        }
    }
}
```

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p config -E 'test(micro_reforge_config)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/config/src/schema/cognitive.rs
git commit -m "feat(config): MicroReforgeConfig with safe defaults (KCA Track 4)"
```

---

### Task B4.4: State table — track last run + counters

**Files:**
- Create: `crates/cognitive/migrations/011_micro_reforge_state.sql`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Migration.**

```sql
-- KCA Track 4: track last micro-Reforge run + counters.
CREATE TABLE IF NOT EXISTS micro_reforge_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_run_at TEXT,
    turns_since_last_run INTEGER NOT NULL DEFAULT 0,
    last_turn_count_at_run INTEGER NOT NULL DEFAULT 0,
    total_runs INTEGER NOT NULL DEFAULT 0,
    total_rules_promoted INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO micro_reforge_state (id, last_run_at, turns_since_last_run) VALUES (1, NULL, 0);

-- Audit log of each micro-Reforge invocation.
CREATE TABLE IF NOT EXISTS micro_reforge_runs (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    trigger TEXT NOT NULL CHECK (trigger IN ('turn_threshold', 'minute_threshold', 'manual')),
    turn_count_at_run INTEGER NOT NULL,
    proposed_rule_count INTEGER NOT NULL DEFAULT 0,
    accepted_rule_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_micro_reforge_runs_started ON micro_reforge_runs(started_at DESC);
```

- [ ] **Step 2: Register migration.**

In `cognitive/src/lib.rs`:

```rust
        FeatureMigration { version: 11, sql: include_str!("../migrations/011_micro_reforge_state.sql"), name: "micro_reforge_state" },
```

- [ ] **Step 3: Run migration tests.**

```bash
cargo nextest run -p cognitive -E 'test(/migration/)'
```

Expected: clean.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/migrations/011_micro_reforge_state.sql crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): migration 011 micro_reforge_state + runs (KCA Track 4)"
```

---

### Task B4.5: Failing test — `MicroReforgeService::should_run`

**Files:**
- Test: in `crates/cognitive/src/services/micro_reforge.rs`

- [ ] **Step 1: Add test module.**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::micro_reforge_types::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn should_run_returns_true_after_turn_threshold() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let service = MicroReforgeService::new(pool.clone(), MicroReforgeConfig::default());

        for _ in 0..10 { service.note_turn().await.unwrap(); }
        assert!(service.should_run().await.unwrap());
    }

    #[tokio::test]
    async fn should_run_returns_false_below_threshold() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let service = MicroReforgeService::new(pool.clone(), MicroReforgeConfig::default());

        for _ in 0..9 { service.note_turn().await.unwrap(); }
        assert!(!service.should_run().await.unwrap());
    }

    #[tokio::test]
    async fn should_run_returns_false_when_disabled() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let mut cfg = MicroReforgeConfig::default();
        cfg.enabled = false;
        let service = MicroReforgeService::new(pool.clone(), cfg);

        for _ in 0..50 { service.note_turn().await.unwrap(); }
        assert!(!service.should_run().await.unwrap());
    }
}
```

- [ ] **Step 2: Run, expect compile failure (`MicroReforgeService` undefined).**

```bash
cargo nextest run -p cognitive -E 'test(/micro_reforge::tests::/)'
```

---

### Task B4.6: Implement `MicroReforgeService`

**Files:**
- Modify: `crates/cognitive/src/services/micro_reforge.rs`

- [ ] **Step 1: Add the service.**

```rust
use config::schema::cognitive::MicroReforgeConfig;
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;

pub struct MicroReforgeService {
    pool: StoragePool,
    cfg: MicroReforgeConfig,
}

impl MicroReforgeService {
    pub fn new(pool: StoragePool, cfg: MicroReforgeConfig) -> Self {
        Self { pool, cfg }
    }

    pub async fn note_turn(&self) -> common::Result<()> {
        sqlx::query!("UPDATE micro_reforge_state SET turns_since_last_run = turns_since_last_run + 1 WHERE id = 1")
            .execute(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("note_turn: {e}")))?;
        Ok(())
    }

    pub async fn should_run(&self) -> common::Result<bool> {
        if !self.cfg.enabled {
            return Ok(false);
        }
        let row = sqlx::query!("SELECT turns_since_last_run, last_run_at FROM micro_reforge_state WHERE id = 1")
            .fetch_one(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("should_run: {e}")))?;

        if (row.turns_since_last_run as u32) >= self.cfg.turn_threshold {
            return Ok(true);
        }

        if let Some(last) = row.last_run_at {
            let last_ts: Timestamp = last.parse().unwrap_or_else(|_| Timestamp::now());
            let elapsed_min = (Timestamp::now() - last_ts).total(jiff::Unit::Minute).unwrap_or(0.0);
            if elapsed_min >= self.cfg.minute_threshold as f64 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn record_run(
        &self,
        trigger: &str,
        proposed_count: u32,
        accepted_count: u32,
        error: Option<&str>,
    ) -> common::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let started = Timestamp::now().to_string();
        let row = sqlx::query!("SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1")
            .fetch_one(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let pc = proposed_count as i64;
        let ac = accepted_count as i64;
        sqlx::query!(
            r#"INSERT INTO micro_reforge_runs
               (id, started_at, finished_at, trigger, turn_count_at_run, proposed_rule_count, accepted_rule_count, error)
               VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            id, started, trigger, row.turns_since_last_run, pc, ac, error
        )
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        sqlx::query!(
            r#"UPDATE micro_reforge_state SET
                  last_run_at = ?1,
                  turns_since_last_run = 0,
                  total_runs = total_runs + 1,
                  total_rules_promoted = total_rules_promoted + ?2
               WHERE id = 1"#,
            started, ac
        )
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/micro_reforge::tests::/)'
```

Expected: all 3 PASS.

- [ ] **Step 3: Commit.**

```bash
git add crates/cognitive/src/services/micro_reforge.rs
git commit -m "feat(cognitive): MicroReforgeService with should_run + counters (KCA Track 4)"
```

---

### Task B4.7: Failing test — `LlmMicroReforgeHandler` writes proposed rules

**Files:**
- Test: `crates/agent/src/adapters/cognitive_handlers.rs`

- [ ] **Step 1: Add test.**

```rust
    #[tokio::test]
    async fn llm_micro_reforge_handler_extracts_high_confidence_rules() {
        use cognitive::services::micro_reforge_types::*;
        use cognitive::services::micro_reforge::MicroReforgeHandler;

        let json = r#"{
            "proposed_rules": [
                {"domain": "coding", "rule_text": "When user runs cargo nextest, also run clippy.",
                 "confidence": 0.85, "signal_count": 4,
                 "evidence_episodic_ids": ["ep1", "ep2"]}
            ],
            "notes": null
        }"#;
        let provider = providers::test_helpers::FakeProvider::with_text(json);
        let handler = LlmMicroReforgeHandler::new(std::sync::Arc::new(provider), "m".into(), 2048);

        let input = MicroReforgeInput {
            recent_episodics: vec![EpisodicRef {
                id: "ep1".into(), domain: "coding".into(),
                summary: "user ran cargo nextest".into(), importance: 0.7,
                recorded_at: "2026-04-29T00:00:00Z".into(),
            }],
            recent_observations: vec![],
            existing_rules_summary: vec![],
            session_count: 3,
            turn_count_since_last_run: 10,
        };

        let out = handler.synthesize(input).await.unwrap();
        assert_eq!(out.proposed_rules.len(), 1);
        assert!(out.proposed_rules[0].confidence > 0.8);
    }
```

- [ ] **Step 2: Run, expect compile error.**

```bash
cargo nextest run -p agent -E 'test(llm_micro_reforge_handler_extracts_high_confidence_rules)'
```

---

### Task B4.8: Implement `LlmMicroReforgeHandler` and prompt

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs` (or wherever)

- [ ] **Step 1: Prompt.**

```rust
pub(crate) const MICRO_REFORGE_SYSTEM_PROMPT: &str = r#"You are a procedural-rule synthesizer running mid-session.

You are given recent episodic memories, recent low-level observations, and a summary of existing procedural rules. Your job: identify NEW stable behavioral patterns worth promoting to a rule. Rules are short imperative statements ("When X, do Y").

Conservative criteria — only propose a rule when ALL of the following hold:
1. The pattern appears in ≥3 recent episodic memories.
2. The pattern is not already represented in existing_rules_summary (paraphrasing counts as duplicate).
3. The rule is actionable for an AI assistant (not a description; not a fact about the user).
4. Confidence ≥ 0.6.

Output JSON exactly:
{"proposed_rules": [{"domain": "...", "rule_text": "...", "confidence": 0.0-1.0, "signal_count": N, "evidence_episodic_ids": ["..."]}], "notes": "optional"}

If nothing meets the bar, return {"proposed_rules": [], "notes": "no stable patterns this cycle"}. Never invent episodic ids; reference only those provided in the input."#;
```

- [ ] **Step 2: Handler.**

```rust
use cognitive::services::micro_reforge::MicroReforgeHandler;
use cognitive::services::micro_reforge_types::{MicroReforgeInput, MicroReforgeOutput};

pub struct LlmMicroReforgeHandler {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
    max_tokens: u32,
}

impl LlmMicroReforgeHandler {
    pub fn new(
        provider: std::sync::Arc<dyn providers::DynProvider>,
        model: String,
        max_tokens: u32,
    ) -> Self {
        Self { provider, model, max_tokens }
    }
}

#[async_trait::async_trait]
impl MicroReforgeHandler for LlmMicroReforgeHandler {
    async fn synthesize(&self, input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
        let user = serde_json::to_string(&input).map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(MICRO_REFORGE_SYSTEM_PROMPT.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(self.max_tokens),
            temperature: Some(0.2),
            response_format: Some(providers::ResponseFormat::JsonObject),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("micro_reforge: {e}")))?;
        let text = resp.text();
        match serde_json::from_str::<MicroReforgeOutput>(&text) {
            Ok(out) => Ok(out),
            Err(e) => {
                tracing::warn!(error = %e, raw = %text, "micro_reforge: parse failed; returning empty");
                Ok(MicroReforgeOutput::default())
            }
        }
    }
}
```

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p agent -E 'test(llm_micro_reforge_handler_extracts_high_confidence_rules)'
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmMicroReforgeHandler + prompt (KCA Track 4)"
```

---

### Task B4.9: Failing test — `MicroReforgeService::run` writes accepted rules to procedural_rules

**Files:**
- Test: `crates/cognitive/src/services/micro_reforge.rs`

- [ ] **Step 1: Add.**

```rust
    use std::sync::Arc;
    use cognitive::repos::procedural_rule::ProceduralRuleRepo;

    struct ScriptedHandler(MicroReforgeOutput);

    #[async_trait::async_trait]
    impl MicroReforgeHandler for ScriptedHandler {
        async fn synthesize(&self, _input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn run_writes_proposed_rule_above_confidence_threshold() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let cfg = MicroReforgeConfig::default();
        let service = MicroReforgeService::new(pool.clone(), cfg);
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        let handler = Arc::new(ScriptedHandler(MicroReforgeOutput {
            proposed_rules: vec![ProposedRule {
                domain: "coding".into(),
                rule_text: "When tests fail, run clippy first.".into(),
                confidence: 0.85,
                signal_count: 4,
                evidence_episodic_ids: vec!["ep1".into()],
            }],
            notes: None,
        }));

        let accepted = service.run("manual", handler, &rule_repo, &cognitive::repos::episodic::EpisodicMemoryRepo::new(pool.clone()), &cognitive::repos::accumulated_observation::AccumulatedObservationRepo::new(pool.clone())).await.unwrap();

        assert_eq!(accepted, 1);
        let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].source, "reflected_online");
    }

    #[tokio::test]
    async fn run_skips_rule_below_confidence_threshold() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let mut cfg = MicroReforgeConfig::default();
        cfg.min_confidence = 0.8;
        let service = MicroReforgeService::new(pool.clone(), cfg);
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        let handler = Arc::new(ScriptedHandler(MicroReforgeOutput {
            proposed_rules: vec![ProposedRule {
                domain: "coding".into(),
                rule_text: "low conf rule".into(),
                confidence: 0.6,
                signal_count: 2,
                evidence_episodic_ids: vec![],
            }],
            notes: None,
        }));

        let accepted = service.run("manual", handler, &rule_repo, &cognitive::repos::episodic::EpisodicMemoryRepo::new(pool.clone()), &cognitive::repos::accumulated_observation::AccumulatedObservationRepo::new(pool.clone())).await.unwrap();

        assert_eq!(accepted, 0);
        let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
        assert_eq!(rules.len(), 0);
    }
```

- [ ] **Step 2: Run, expect compile failure.**

```bash
cargo nextest run -p cognitive -E 'test(/micro_reforge::tests::run_/)'
```

---

### Task B4.10: Implement `MicroReforgeService::run`

**Files:**
- Modify: `crates/cognitive/src/services/micro_reforge.rs`

- [ ] **Step 1: Add method.**

```rust
impl MicroReforgeService {
    pub async fn run(
        &self,
        trigger: &str,
        handler: Arc<dyn MicroReforgeHandler>,
        rule_repo: &crate::repos::procedural_rule::ProceduralRuleRepo,
        episodic_repo: &crate::repos::episodic::EpisodicMemoryRepo,
        observation_repo: &crate::repos::accumulated_observation::AccumulatedObservationRepo,
    ) -> common::Result<u32> {
        // 1. Collect input.
        let recent_episodics: Vec<crate::services::micro_reforge_types::EpisodicRef> = episodic_repo
            .list_recent(50)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|e| crate::services::micro_reforge_types::EpisodicRef {
                id: e.id,
                domain: e.domain,
                summary: e.summary.unwrap_or_default(),
                importance: e.importance,
                recorded_at: e.recorded_at.to_string(),
            })
            .collect();

        let recent_observations: Vec<crate::services::micro_reforge_types::ObservationRef> = observation_repo
            .list_recent(100)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|o| crate::services::micro_reforge_types::ObservationRef {
                content_truncated: truncate(&o.content, 200),
                domain: o.domain,
                importance: o.importance,
            })
            .collect();

        let existing_rules: Vec<crate::services::micro_reforge_types::RuleSummary> = rule_repo
            .list_active(50)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| crate::services::micro_reforge_types::RuleSummary {
                domain: r.domain,
                rule_text: r.rule_text,
                confidence: r.confidence,
            })
            .collect();

        let turn_row = sqlx::query!("SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1")
            .fetch_one(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let input = crate::services::micro_reforge_types::MicroReforgeInput {
            recent_episodics,
            recent_observations,
            existing_rules_summary: existing_rules,
            session_count: 0,
            turn_count_since_last_run: turn_row.turns_since_last_run as u32,
        };

        // 2. Synthesize.
        let out = handler.synthesize(input).await?;

        // 3. Apply: write rules above min_confidence, dedup against existing.
        let mut accepted = 0u32;
        for proposed in &out.proposed_rules {
            if proposed.confidence < self.cfg.min_confidence { continue; }
            if rule_text_already_exists(rule_repo, &proposed.domain, &proposed.rule_text).await {
                continue;
            }
            if let Err(e) = rule_repo.upsert(
                &cognitive_compat_rule_row(proposed),
            ).await {
                tracing::warn!(error = %e, "micro_reforge: rule upsert failed");
                continue;
            }
            accepted += 1;
        }

        // 4. Record run.
        self.record_run(trigger, out.proposed_rules.len() as u32, accepted, None).await?;
        Ok(accepted)
    }
}

async fn rule_text_already_exists(
    repo: &crate::repos::procedural_rule::ProceduralRuleRepo,
    domain: &str,
    rule_text: &str,
) -> bool {
    repo.list_by_domain(domain, 100)
        .await
        .unwrap_or_default()
        .iter()
        .any(|r| normalize(&r.rule_text) == normalize(rule_text))
}

fn normalize(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

fn cognitive_compat_rule_row(p: &crate::services::micro_reforge_types::ProposedRule) -> crate::repos::procedural_rule::ProceduralRule {
    crate::repos::procedural_rule::ProceduralRule {
        id: uuid::Uuid::new_v4().to_string(),
        domain: p.domain.clone(),
        rule_text: p.rule_text.clone(),
        confidence: p.confidence,
        signal_count: p.signal_count as i32,
        source: "reflected_online".into(),
        active: true,
        created_at: jiff::Timestamp::now(),
        updated_at: jiff::Timestamp::now(),
        // ...rest match real schema; if `ProceduralRule` has more fields, fill via Default::default()
        ..Default::default()
    }
}
```

If `ProceduralRule` doesn't impl `Default`, build the struct explicitly with all fields. Search:

```bash
rg -n "pub struct ProceduralRule" crates/cognitive/src/repos/procedural_rule.rs
```

Adjust the constructor accordingly.

- [ ] **Step 2: Add `list_recent` and `list_active` if missing.**

For `EpisodicMemoryRepo` and `ProceduralRuleRepo`, these may not exist. Search:

```bash
rg -n "fn list_recent\|fn list_active\|fn list_by_domain" crates/cognitive/src/repos/ --type rust
```

If missing, add minimal SQL implementations.

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/micro_reforge::tests::run_/)'
```

Expected: both PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/micro_reforge.rs crates/cognitive/src/repos/
git commit -m "feat(cognitive): MicroReforgeService::run with confidence + dedup gate (KCA Track 4)"
```

---

### Task B4.11: Cron registration

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Failing test (cron registration).**

In `app-core/src/init/cron.rs` `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn micro_reforge_cron_registered_when_enabled() {
        let cfg = test_config_with_micro_reforge_enabled();
        let cron = init_cron_for_test(&cfg).await.unwrap();
        let jobs = cron.list_jobs().await.unwrap();
        assert!(
            jobs.iter().any(|j| j.name == "__klyntbot_micro_reforge"),
            "expected micro_reforge cron, got {:?}", jobs.iter().map(|j| &j.name).collect::<Vec<_>>()
        );
    }
```

If `init_cron_for_test` doesn't exist, add a small helper.

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Register the cron.**

In `init_cron`:

```rust
    if config.cognitive.micro_reforge.enabled {
        // Fire every 5 minutes; the service itself decides via should_run() whether to actually run.
        cron_executor.upsert_default_job(
            "__klyntbot_micro_reforge",
            "0 */5 * * * *",
            "Micro-Reforge timer (KCA Track 4)",
        ).await?;
    }
```

And in the cron callback registration:

```rust
    cron_executor.register_callback("__klyntbot_micro_reforge", {
        let pool = pool.clone();
        let micro_cfg = config.cognitive.micro_reforge.clone();
        let handler: Arc<dyn MicroReforgeHandler> = if let Some(prov) = cognitive_provider.clone() {
            Arc::new(crate::adapters::cognitive_handlers::LlmMicroReforgeHandler::new(
                prov,
                micro_cfg.model.clone().unwrap_or_else(|| config.cognitive.model.clone()),
                4096,
            ))
        } else {
            Arc::new(NoopMicroReforgeHandler)
        };
        let rule_repo = ProceduralRuleRepo::new(pool.clone());
        let ep_repo = EpisodicMemoryRepo::new(pool.clone());
        let obs_repo = AccumulatedObservationRepo::new(pool.clone());
        Box::new(move || {
            let pool = pool.clone();
            let cfg = micro_cfg.clone();
            let handler = handler.clone();
            let rule_repo = rule_repo.clone();
            let ep_repo = ep_repo.clone();
            let obs_repo = obs_repo.clone();
            Box::pin(async move {
                let svc = MicroReforgeService::new(pool, cfg);
                if !svc.should_run().await.unwrap_or(false) { return Ok(()); }
                let trigger = "minute_threshold";
                match svc.run(trigger, handler, &rule_repo, &ep_repo, &obs_repo).await {
                    Ok(n) => tracing::info!(accepted = n, "micro_reforge ran"),
                    Err(e) => tracing::warn!(error = %e, "micro_reforge failed"),
                }
                Ok(())
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = common::Result<()>> + Send>>
        })
    }).await;
```

Adjust to match the actual `register_callback` signature in your codebase.

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p app-core -E 'test(micro_reforge_cron_registered_when_enabled)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "feat(app-core): register __klyntbot_micro_reforge cron (KCA Track 4)"
```

---

### Task B4.12: Per-turn `note_turn` invocation

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Failing test.**

In `runtime.rs` tests:

```rust
    #[tokio::test]
    async fn process_message_increments_micro_reforge_counter() {
        let app = test_app_core_with_micro_reforge().await;
        let key = SessionKey::new("test", "session1");
        app.chat_send("hi", key.clone(), None).await.unwrap();

        let row = sqlx::query!("SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1")
            .fetch_one(app.pool().inner()).await.unwrap();
        assert_eq!(row.turns_since_last_run, 1);
    }
```

- [ ] **Step 2: Run, expect either compile or assertion failure.**

- [ ] **Step 3: Wire counter increment in Phase 3 (record).**

In `runtime.rs::process_message` Phase 3 (after-response section), add:

```rust
            // KCA Track 4: bump micro-Reforge turn counter (fire-and-forget).
            if let Some(svc) = self.micro_reforge_service.as_ref() {
                let svc = svc.clone();
                tokio::spawn(async move { let _ = svc.note_turn().await; });
            }
```

Add `micro_reforge_service: Option<Arc<MicroReforgeService>>` to `AgentRuntime` struct + builder.

- [ ] **Step 4: Run.**

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): bump micro_reforge turn counter per chat turn (KCA Track 4)"
```

---

# Track 5 — Self-critique loop on extraction

After every extraction, an async critic LLM reads the extracted facts plus the original turn content and verdict-decides each fact: GROUNDED, HALLUCINATED, AMBIGUOUS. Hallucinated → demote stability ×0.5 and write to a Mirror table for nightly re-evaluation. Conservative — never hard-supersede; just down-weight.

### Task B5.1: Failing test — `ExtractionCriticOutput` types

**Files:**
- Create: `crates/cognitive/src/services/extraction_critic_types.rs`

- [ ] **Step 1: Create.**

```rust
//! Types for the extraction critic (KCA Track 5).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionCriticInput {
    pub turn_text: String,
    pub extracted_facts: Vec<FactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactRef {
    pub fact_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionCriticOutput {
    pub verdicts: Vec<Verdict>,
    pub missed_facts: Vec<MissedFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub fact_id: String,
    /// "grounded" | "hallucinated" | "ambiguous"
    pub verdict: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}
```

- [ ] **Step 2: Register in `services/mod.rs`.**

```rust
pub mod extraction_critic_types;
pub use extraction_critic_types::*;
```

- [ ] **Step 3: Build + commit.**

```bash
cargo build -p cognitive
git add crates/cognitive/src/services/extraction_critic_types.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): ExtractionCritic types (KCA Track 5)"
```

---

### Task B5.2: `ExtractionCriticHandler` trait + Noop

**Files:**
- Create: `crates/cognitive/src/services/extraction_critic.rs`

- [ ] **Step 1: Create.**

```rust
use async_trait::async_trait;
use crate::services::extraction_critic_types::*;

#[async_trait]
pub trait ExtractionCriticHandler: Send + Sync {
    async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput>;
}

pub struct NoopExtractionCriticHandler;

#[async_trait]
impl ExtractionCriticHandler for NoopExtractionCriticHandler {
    async fn judge(&self, _: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
        Ok(ExtractionCriticOutput::default())
    }
}
```

- [ ] **Step 2: Register in `services/mod.rs`.**

```rust
pub mod extraction_critic;
pub use extraction_critic::{ExtractionCriticHandler, NoopExtractionCriticHandler};
```

- [ ] **Step 3: Build + commit.**

```bash
cargo build -p cognitive
git add crates/cognitive/src/services/extraction_critic.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): ExtractionCriticHandler trait (KCA Track 5)"
```

---

### Task B5.3: Migration — critic verdict log

**Files:**
- Create: `crates/cognitive/migrations/012_extraction_critic_log.sql`

- [ ] **Step 1: Migration.**

```sql
-- KCA Track 5: log every critic verdict for nightly re-evaluation by Reforge.
CREATE TABLE IF NOT EXISTS extraction_critic_log (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL,
    verdict TEXT NOT NULL CHECK (verdict IN ('grounded', 'hallucinated', 'ambiguous')),
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_by_reforge_at TEXT,
    FOREIGN KEY (fact_id) REFERENCES semantic_facts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_extraction_critic_unreviewed
    ON extraction_critic_log(reviewed_by_reforge_at) WHERE reviewed_by_reforge_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_extraction_critic_fact
    ON extraction_critic_log(fact_id);
```

- [ ] **Step 2: Register.**

In `cognitive/src/lib.rs`:

```rust
        FeatureMigration { version: 12, sql: include_str!("../migrations/012_extraction_critic_log.sql"), name: "extraction_critic_log" },
```

- [ ] **Step 3: Commit.**

```bash
git add crates/cognitive/migrations/012_extraction_critic_log.sql crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): migration 012 extraction_critic_log (KCA Track 5)"
```

---

### Task B5.4: `ExtractionCriticLogRepo`

**Files:**
- Create: `crates/cognitive/src/repos/extraction_critic_log.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Failing test.**

Inline in the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn insert_and_list_unreviewed() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ExtractionCriticLogRepo::new(pool.clone());

        // Need a fact to reference.
        let f = cognitive::repos::semantic_fact::SemanticFact::new("A", "p", "B", 0.5, "t");
        cognitive::repos::semantic_fact::SemanticFactRepo::new(pool.clone()).upsert(&f).await.unwrap();

        repo.insert(&f.id, "hallucinated", "no anchor").await.unwrap();
        let list = repo.list_unreviewed(10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].verdict, "hallucinated");
    }
}
```

- [ ] **Step 2: Run, expect compile failure.**

```bash
cargo nextest run -p cognitive -E 'test(/extraction_critic_log::tests::/)'
```

- [ ] **Step 3: Implement.**

```rust
use storage::StoragePool;

#[derive(Debug, Clone)]
pub struct ExtractionCriticLogEntry {
    pub id: String,
    pub fact_id: String,
    pub verdict: String,
    pub reason: String,
    pub created_at: String,
}

pub struct ExtractionCriticLogRepo {
    pool: StoragePool,
}

impl ExtractionCriticLogRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    pub async fn insert(&self, fact_id: &str, verdict: &str, reason: &str) -> common::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO extraction_critic_log (id, fact_id, verdict, reason) VALUES (?1, ?2, ?3, ?4)",
            id, fact_id, verdict, reason
        )
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("critic_log insert: {e}")))?;
        Ok(())
    }

    pub async fn list_unreviewed(&self, limit: usize) -> common::Result<Vec<ExtractionCriticLogEntry>> {
        let lim = limit as i64;
        let rows = sqlx::query!(
            "SELECT id, fact_id, verdict, reason, created_at FROM extraction_critic_log WHERE reviewed_by_reforge_at IS NULL ORDER BY created_at DESC LIMIT ?1",
            lim
        )
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| ExtractionCriticLogEntry {
            id: r.id, fact_id: r.fact_id, verdict: r.verdict, reason: r.reason, created_at: r.created_at,
        }).collect())
    }

    pub async fn mark_reviewed(&self, ids: &[String]) -> common::Result<()> {
        for id in ids {
            sqlx::query!("UPDATE extraction_critic_log SET reviewed_by_reforge_at = datetime('now') WHERE id = ?1", id)
                .execute(self.pool.inner()).await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Register in `repos/mod.rs`.**

```rust
pub mod extraction_critic_log;
pub use extraction_critic_log::{ExtractionCriticLogRepo, ExtractionCriticLogEntry};
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/extraction_critic_log::/)'
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/cognitive/src/repos/extraction_critic_log.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): ExtractionCriticLogRepo (KCA Track 5)"
```

---

### Task B5.5: `LlmExtractionCriticHandler`

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn llm_extraction_critic_marks_hallucinated_facts() {
        use cognitive::services::extraction_critic_types::*;
        use cognitive::services::extraction_critic::ExtractionCriticHandler;

        let json = r#"{
            "verdicts": [
                {"fact_id": "f1", "verdict": "grounded", "reason": "directly stated"},
                {"fact_id": "f2", "verdict": "hallucinated", "reason": "not in turn text"}
            ],
            "missed_facts": []
        }"#;
        let provider = providers::test_helpers::FakeProvider::with_text(json);
        let handler = LlmExtractionCriticHandler::new(std::sync::Arc::new(provider), "m".into(), 1024);

        let input = ExtractionCriticInput {
            turn_text: "I love Rust.".into(),
            extracted_facts: vec![
                FactRef { fact_id: "f1".into(), subject: "user".into(), predicate: "loves".into(), object: "Rust".into() },
                FactRef { fact_id: "f2".into(), subject: "user".into(), predicate: "hates".into(), object: "Java".into() },
            ],
        };

        let out = handler.judge(input).await.unwrap();
        assert_eq!(out.verdicts.len(), 2);
        assert_eq!(out.verdicts[1].verdict, "hallucinated");
    }
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Prompt.**

```rust
pub(crate) const EXTRACTION_CRITIC_SYSTEM_PROMPT: &str = r#"You are a fact-extraction critic.

You receive a turn of conversation and a list of facts that another system extracted from it. For each fact, decide:
- "grounded": the fact is directly stated or strongly implied in the turn text.
- "hallucinated": the fact contains information not present in the turn text. Includes invented entity names, fabricated numbers, or claims the user did not make.
- "ambiguous": evidence is partial or could be read either way; do not penalize.

Also report any FACTS that the extractor MISSED — but only those clearly stated in the turn (≤3 missed facts).

Output JSON:
{"verdicts": [{"fact_id": "...", "verdict": "grounded|hallucinated|ambiguous", "reason": "..."}], "missed_facts": [{"subject": "...", "predicate": "...", "object": "...", "confidence": 0.0-1.0}]}

Be strict on hallucinated but conservative on missed_facts — only include the obvious ones."#;
```

- [ ] **Step 4: Handler.**

```rust
use cognitive::services::extraction_critic::ExtractionCriticHandler;
use cognitive::services::extraction_critic_types::*;

pub struct LlmExtractionCriticHandler {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
    max_tokens: u32,
}

impl LlmExtractionCriticHandler {
    pub fn new(provider: std::sync::Arc<dyn providers::DynProvider>, model: String, max_tokens: u32) -> Self {
        Self { provider, model, max_tokens }
    }
}

#[async_trait::async_trait]
impl ExtractionCriticHandler for LlmExtractionCriticHandler {
    async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
        if input.extracted_facts.is_empty() { return Ok(Default::default()); }
        let user = serde_json::to_string(&input)
            .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(EXTRACTION_CRITIC_SYSTEM_PROMPT.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(self.max_tokens),
            temperature: Some(0.0), // deterministic
            response_format: Some(providers::ResponseFormat::JsonObject),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("critic: {e}")))?;
        match serde_json::from_str::<ExtractionCriticOutput>(&resp.text()) {
            Ok(o) => Ok(o),
            Err(e) => {
                tracing::warn!(error = %e, "critic: parse failed");
                Ok(Default::default())
            }
        }
    }
}
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p agent -E 'test(llm_extraction_critic_marks_hallucinated_facts)'
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmExtractionCriticHandler + prompt (KCA Track 5)"
```

---

### Task B5.6: Wire critic into `BackgroundConsolidationService`

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn critic_demotes_stability_for_hallucinated_facts() {
        use cognitive::services::extraction_critic::ExtractionCriticHandler;
        use cognitive::services::extraction_critic_types::*;

        struct FakeCritic;
        #[async_trait::async_trait]
        impl ExtractionCriticHandler for FakeCritic {
            async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
                Ok(ExtractionCriticOutput {
                    verdicts: input.extracted_facts.iter().enumerate().map(|(i, f)| Verdict {
                        fact_id: f.fact_id.clone(),
                        verdict: if i % 2 == 0 { "grounded".into() } else { "hallucinated".into() },
                        reason: "test".into(),
                    }).collect(),
                    missed_facts: vec![],
                })
            }
        }

        let pool = StoragePool::connect_in_memory().await.unwrap();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let crit_log = cognitive::repos::extraction_critic_log::ExtractionCriticLogRepo::new(pool.clone());

        let f1 = cognitive::repos::semantic_fact::SemanticFact::new("A", "p", "B", 0.8, "t");
        let f2 = cognitive::repos::semantic_fact::SemanticFact::new("X", "p", "Y", 0.8, "t");
        fact_repo.upsert(&f1).await.unwrap();
        fact_repo.upsert(&f2).await.unwrap();

        run_critic_judgment(
            "User said A is B.",
            vec![f1.clone(), f2.clone()],
            std::sync::Arc::new(FakeCritic),
            &fact_repo,
            &crit_log,
        ).await;

        // f2 should have stability halved.
        let f2_after = fact_repo.find_by_id(&f2.id).await.unwrap().unwrap();
        let f1_after = fact_repo.find_by_id(&f1.id).await.unwrap().unwrap();
        assert!(f2_after.stability < f1_after.stability,
            "hallucinated fact stability {} should be < grounded {}", f2_after.stability, f1_after.stability);
        let log = crit_log.list_unreviewed(10).await.unwrap();
        assert_eq!(log.len(), 2);
    }
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Implement `run_critic_judgment` and integrate into `BackgroundConsolidationService`.**

```rust
pub(crate) async fn run_critic_judgment(
    turn_text: &str,
    facts: Vec<SemanticFact>,
    handler: std::sync::Arc<dyn crate::services::extraction_critic::ExtractionCriticHandler>,
    fact_repo: &SemanticFactRepo,
    log_repo: &crate::repos::extraction_critic_log::ExtractionCriticLogRepo,
) {
    use crate::services::extraction_critic_types::*;
    if facts.is_empty() { return; }

    let input = ExtractionCriticInput {
        turn_text: turn_text.to_string(),
        extracted_facts: facts.iter().map(|f| FactRef {
            fact_id: f.id.clone(),
            subject: f.subject.clone(),
            predicate: f.predicate.clone(),
            object: f.object.clone(),
        }).collect(),
    };
    let out = match handler.judge(input).await {
        Ok(o) => o,
        Err(e) => { tracing::warn!(error = %e, "critic call failed"); return; }
    };

    for v in &out.verdicts {
        if let Err(e) = log_repo.insert(&v.fact_id, &v.verdict, &v.reason).await {
            tracing::warn!(error = %e, "critic log insert failed");
        }
        if v.verdict == "hallucinated" {
            // Demote stability ×0.5 instead of supersede; nightly Reforge re-evaluates.
            if let Err(e) = fact_repo.scale_stability(&v.fact_id, 0.5).await {
                tracing::warn!(error = %e, "stability scale failed");
            }
        }
    }
}
```

If `SemanticFactRepo::scale_stability` doesn't exist, add it:

```rust
    pub async fn scale_stability(&self, id: &str, factor: f64) -> common::Result<()> {
        sqlx::query!(
            "UPDATE semantic_facts SET stability = stability * ?1 WHERE id = ?2",
            factor, id
        )
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
```

- [ ] **Step 4: Wire into the consolidation event loop.**

After extraction yields facts (in `BackgroundConsolidationService::process_extraction_result` or equivalent), spawn:

```rust
            if let Some(critic) = self.critic_handler.as_ref() {
                let critic = critic.clone();
                let facts_clone = added_facts.clone();
                let turn_text = source_text.clone();
                let fr = self.fact_repo.clone();
                let lg = self.critic_log_repo.clone();
                tokio::spawn(async move {
                    run_critic_judgment(&turn_text, facts_clone, critic, &fr, &lg).await;
                });
            }
```

Add `critic_handler: Option<Arc<dyn ExtractionCriticHandler>>` and `critic_log_repo: ExtractionCriticLogRepo` to the service.

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p cognitive -E 'test(critic_demotes_stability_for_hallucinated_facts)'
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/cognitive/src/services/background.rs crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): wire ExtractionCritic into background loop (KCA Track 5)"
```

---

### Task B5.7: Wire critic in `agent_loop::builder`

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Construct + pass.**

In the same block where `LlmConsolidationHandler` is built:

```rust
            let critic_handler: Option<std::sync::Arc<dyn cognitive::services::extraction_critic::ExtractionCriticHandler>> =
                cognitive_provider.as_ref().map(|p| {
                    std::sync::Arc::new(crate::adapters::cognitive_handlers::LlmExtractionCriticHandler::new(
                        p.clone(),
                        cognitive_cfg.critic_model.clone().unwrap_or_else(|| cognitive_cfg.model.clone()),
                        1024,
                    )) as std::sync::Arc<dyn cognitive::services::extraction_critic::ExtractionCriticHandler>
                });
```

Pass `critic_handler` into `BackgroundConsolidationService::new`.

- [ ] **Step 2: Add `critic_model` field to `CognitiveConfig`.**

In `crates/config/src/schema/cognitive.rs`:

```rust
    /// Model used for the extraction critic (KCA Track 5). Defaults to `model`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_model: Option<String>,
```

- [ ] **Step 3: Build + run full agent tests.**

```bash
cargo nextest run -p agent -p cognitive
```

Expected: green.

- [ ] **Step 4: Commit.**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/config/src/schema/cognitive.rs
git commit -m "feat(agent): wire ExtractionCritic in builder (KCA Track 5)"
```

---

# Track 11 — Online community membership

When a session ends, look at the entities touched in the session. For each entity not yet in a community, find the best-fit existing community via co_activation strength + shared neighbors, and a tiny LLM call confirms (or rejects) the assignment. New facts join clusters within minutes, not 24h.

### Task B11.1: Failing test — `find_best_community_for_entity`

**Files:**
- Test: `crates/cognitive/src/services/community_membership_online.rs`

- [ ] **Step 1: Create the file with the failing test.**

```rust
//! KCA Track 11 — online community membership.

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;
    use cognitive::repos::entity::EntityRepo;
    use cognitive::repos::community::CommunityRepo;

    #[tokio::test]
    async fn finds_best_community_via_co_activation() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let entity_repo = EntityRepo::new(pool.clone());
        let community_repo = CommunityRepo::new(pool.clone());

        // Seed: 3 entities, A & B in community "alpha", C uncategorized.
        let a = entity_repo.upsert_entity("A", "concept", None, "t", None).await.unwrap();
        let b = entity_repo.upsert_entity("B", "concept", None, "t", None).await.unwrap();
        let c = entity_repo.upsert_entity("C", "concept", None, "t", None).await.unwrap();
        let alpha = community_repo.create("alpha", "test community", None).await.unwrap();
        community_repo.add_member(&alpha.id, &a.id).await.unwrap();
        community_repo.add_member(&alpha.id, &b.id).await.unwrap();

        // Co-activate C with A (strength 0.9), C with B (0.7) — strong signal C ∈ alpha.
        cognitive::repos::co_activation::CoActivationRepo::new(pool.clone())
            .increment_pair(&c.id, &a.id, 0.9).await.unwrap();
        cognitive::repos::co_activation::CoActivationRepo::new(pool.clone())
            .increment_pair(&c.id, &b.id, 0.7).await.unwrap();

        let result = find_best_community_for_entity(&entity_repo, &community_repo, &c.id).await.unwrap();
        assert_eq!(result.as_ref().map(|c| c.community_id.clone()), Some(alpha.id));
        assert!(result.as_ref().unwrap().score > 0.5);
    }
}
```

- [ ] **Step 2: Add module placeholder + run, expect compile failure.**

```bash
cargo nextest run -p cognitive -E 'test(/community_membership_online::/)'
```

---

### Task B11.2: Implement `find_best_community_for_entity`

**Files:**
- Modify: `crates/cognitive/src/services/community_membership_online.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Implement.**

```rust
use cognitive::repos::entity::EntityRepo;
use cognitive::repos::community::CommunityRepo;

#[derive(Debug, Clone)]
pub struct CommunityMatch {
    pub community_id: String,
    pub community_name: String,
    pub score: f64,
}

pub async fn find_best_community_for_entity(
    entity_repo: &EntityRepo,
    community_repo: &CommunityRepo,
    entity_id: &str,
) -> common::Result<Option<CommunityMatch>> {
    use cognitive::repos::co_activation::CoActivationRepo;
    let coact = CoActivationRepo::new(entity_repo.pool().clone());

    // Get top-20 co-activated entities for this entity.
    let coact_pairs = coact.top_for_entity(entity_id, 20).await?;
    if coact_pairs.is_empty() {
        return Ok(None);
    }

    // For each co-activated entity, find which community it's in.
    let mut score_by_community: std::collections::HashMap<String, (String, f64)> = Default::default();
    for (other_id, strength) in coact_pairs {
        for community in community_repo.list_for_entity(&other_id).await.unwrap_or_default() {
            let entry = score_by_community.entry(community.id.clone())
                .or_insert((community.name.clone(), 0.0));
            entry.1 += strength;
        }
    }

    Ok(score_by_community
        .into_iter()
        .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, (name, score))| CommunityMatch { community_id: id, community_name: name, score }))
}
```

- [ ] **Step 2: Add helper repo methods if missing.**

For `CoActivationRepo::top_for_entity` and `CommunityRepo::list_for_entity`, search:

```bash
rg -n "fn top_for_entity\|fn list_for_entity" crates/cognitive/src/repos/ --type rust
```

If absent, add minimal SQL impls.

- [ ] **Step 3: Add `EntityRepo::pool()` accessor.**

```rust
    pub fn pool(&self) -> &storage::StoragePool { &self.pool }
```

- [ ] **Step 4: Register module.**

In `services/mod.rs`:

```rust
pub mod community_membership_online;
pub use community_membership_online::{find_best_community_for_entity, CommunityMatch};
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/community_membership_online::/)'
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/cognitive/src/services/community_membership_online.rs \
        crates/cognitive/src/services/mod.rs \
        crates/cognitive/src/repos/co_activation.rs \
        crates/cognitive/src/repos/community.rs \
        crates/cognitive/src/repos/entity.rs
git commit -m "feat(cognitive): online community membership via co-activation (KCA Track 11)"
```

---

### Task B11.3: LLM confirmation handler

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn llm_community_membership_confirms_match() {
        let json = r#"{"confirm": true, "reason": "C aligns with alpha by topic"}"#;
        let provider = providers::test_helpers::FakeProvider::with_text(json);
        let handler = LlmCommunityMembershipHandler::new(std::sync::Arc::new(provider), "m".into(), 256);

        let result = handler.confirm_membership(
            "C", "concept", "alpha", "topic about α-particles", 0.85
        ).await.unwrap();
        assert!(result.confirm);
    }
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Prompt + handler.**

```rust
pub(crate) const COMMUNITY_MEMBERSHIP_SYSTEM_PROMPT: &str = r#"You are a knowledge-graph cluster membership confirmer.

You are given a candidate entity, a community summary, and a co-activation score. Decide:
- confirm = true: this entity belongs in the community.
- confirm = false: it does not, the heuristic was misleading.

Output JSON: {"confirm": true|false, "reason": "short reason"}.

Be conservative: confirm only when the entity clearly fits the community theme."#;

pub struct CommunityMembershipDecision {
    pub confirm: bool,
    pub reason: String,
}

pub struct LlmCommunityMembershipHandler {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
    max_tokens: u32,
}

impl LlmCommunityMembershipHandler {
    pub fn new(provider: std::sync::Arc<dyn providers::DynProvider>, model: String, max_tokens: u32) -> Self {
        Self { provider, model, max_tokens }
    }

    pub async fn confirm_membership(
        &self,
        entity_name: &str,
        entity_type: &str,
        community_name: &str,
        community_summary: &str,
        score: f64,
    ) -> common::Result<CommunityMembershipDecision> {
        let user = format!(
            "Entity: {} (type: {})\nCommunity: {}\nSummary: {}\nCo-activation score: {:.2}\n\nConfirm membership?",
            entity_name, entity_type, community_name, community_summary, score
        );
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(COMMUNITY_MEMBERSHIP_SYSTEM_PROMPT.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(self.max_tokens),
            temperature: Some(0.0),
            response_format: Some(providers::ResponseFormat::JsonObject),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("community_membership: {e}")))?;

        #[derive(serde::Deserialize)]
        struct R { confirm: bool, reason: String }
        match serde_json::from_str::<R>(&resp.text()) {
            Ok(r) => Ok(CommunityMembershipDecision { confirm: r.confirm, reason: r.reason }),
            Err(_) => Ok(CommunityMembershipDecision { confirm: false, reason: "parse failed".into() }),
        }
    }
}
```

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p agent -E 'test(llm_community_membership_confirms_match)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmCommunityMembershipHandler (KCA Track 11)"
```

---

### Task B11.4: Wire into session-end pass

**Files:**
- Modify: `crates/coding-memory/src/reforge/session_end.rs`

- [ ] **Step 1: Failing test.**

In `crates/cognitive/tests/phase_b_continuous_learning.rs` (will be created in BIT.1):

```rust
#[tokio::test]
async fn session_end_assigns_uncategorized_entity_to_best_community() {
    use cognitive::repos::community::CommunityRepo;
    use cognitive::repos::entity::EntityRepo;
    use storage::StoragePool;

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let entity_repo = EntityRepo::new(pool.clone());
    let community_repo = CommunityRepo::new(pool.clone());
    let coact = cognitive::repos::co_activation::CoActivationRepo::new(pool.clone());

    let a = entity_repo.upsert_entity("A", "concept", None, "t", None).await.unwrap();
    let b = entity_repo.upsert_entity("B", "concept", None, "t", None).await.unwrap();
    let c = entity_repo.upsert_entity("C_new", "concept", None, "t", None).await.unwrap();
    let alpha = community_repo.create("alpha", "α-cluster", None).await.unwrap();
    community_repo.add_member(&alpha.id, &a.id).await.unwrap();
    community_repo.add_member(&alpha.id, &b.id).await.unwrap();

    coact.increment_pair(&c.id, &a.id, 0.95).await.unwrap();
    coact.increment_pair(&c.id, &b.id, 0.85).await.unwrap();

    // Auto-confirm handler (fake LLM).
    let handler = std::sync::Arc::new(crate::common_test::AlwaysConfirmHandler);

    cognitive::services::community_membership_online::run_for_session(
        &entity_repo, &community_repo, vec![c.id.clone()], handler,
    ).await;

    let cs = community_repo.list_for_entity(&c.id).await.unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].id, alpha.id);
}
```

- [ ] **Step 2: Implement `run_for_session`.**

In `community_membership_online.rs`:

```rust
pub async fn run_for_session(
    entity_repo: &EntityRepo,
    community_repo: &CommunityRepo,
    touched_entity_ids: Vec<String>,
    confirm_handler: std::sync::Arc<dyn AsyncConfirmFn>,
) {
    for eid in touched_entity_ids {
        if !community_repo.list_for_entity(&eid).await.unwrap_or_default().is_empty() {
            continue; // already in a community
        }
        let m = match find_best_community_for_entity(entity_repo, community_repo, &eid).await {
            Ok(Some(m)) => m,
            _ => continue,
        };
        if m.score < 1.0 {
            // Below threshold; skip until nightly Reforge has more data.
            continue;
        }
        let entity = match entity_repo.find_by_id(&eid).await.unwrap_or(None) {
            Some(e) => e, None => continue,
        };
        // Get community summary.
        let summary = community_repo.get_summary(&m.community_id).await.unwrap_or_default();
        let decision = confirm_handler.confirm(
            &entity.name, &entity.entity_type, &m.community_name, &summary, m.score
        ).await;
        if decision {
            let _ = community_repo.add_member(&m.community_id, &eid).await;
        }
    }
}

#[async_trait::async_trait]
pub trait AsyncConfirmFn: Send + Sync {
    async fn confirm(&self, entity_name: &str, entity_type: &str, community_name: &str, summary: &str, score: f64) -> bool;
}
```

Adapt `LlmCommunityMembershipHandler` to implement this trait.

- [ ] **Step 3: Wire from `session_end.rs`.**

After existing session-end work (Hebbian bumps, fix-attempt dedup), append:

```rust
    // KCA Track 11 — online community membership.
    if let Some(handler) = self.community_handler.as_ref() {
        let touched_ids = collect_touched_entity_ids(&self.coact_repo, session_id).await;
        cognitive::services::community_membership_online::run_for_session(
            &self.entity_repo, &self.community_repo, touched_ids, handler.clone(),
        ).await;
    }
```

`collect_touched_entity_ids` is a helper that pulls from `co_activation` rows updated this session.

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p cognitive --test phase_b_continuous_learning -E 'test(session_end_assigns_uncategorized_entity_to_best_community)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/cognitive/src/services/community_membership_online.rs \
        crates/coding-memory/src/reforge/session_end.rs \
        crates/cognitive/tests/phase_b_continuous_learning.rs
git commit -m "feat(cognitive): online community membership at session-end (KCA Track 11)"
```

---

# Phase B Integration Tests

### Task BIT.1: Multi-track session simulation

**Files:**
- Create/extend: `crates/cognitive/tests/phase_b_continuous_learning.rs`

- [ ] **Step 1: Add comprehensive integration test.**

```rust
#[tokio::test]
async fn phase_b_session_drives_micro_reforge_critic_and_community() {
    use cognitive::repos::*;
    use cognitive::services::micro_reforge::*;
    use cognitive::services::extraction_critic::*;
    use cognitive::services::community_membership_online as cmo;
    use storage::StoragePool;

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let fact_repo = semantic_fact::SemanticFactRepo::new(pool.clone());
    let entity_repo = entity::EntityRepo::new(pool.clone());
    let rule_repo = procedural_rule::ProceduralRuleRepo::new(pool.clone());
    let ep_repo = episodic::EpisodicMemoryRepo::new(pool.clone());
    let obs_repo = accumulated_observation::AccumulatedObservationRepo::new(pool.clone());
    let crit_log = extraction_critic_log::ExtractionCriticLogRepo::new(pool.clone());
    let community_repo = community::CommunityRepo::new(pool.clone());

    // Seed an existing rule so the dedup path is exercised.
    rule_repo.upsert(&procedural_rule::ProceduralRule {
        id: "r0".into(),
        domain: "coding".into(),
        rule_text: "When the user says 'lint', run cargo clippy".into(),
        confidence: 0.8,
        signal_count: 10,
        source: "reflected".into(),
        active: true,
        ..Default::default()
    }).await.unwrap();

    // Insert 10 episodics suggesting a new pattern.
    for i in 0..10 {
        ep_repo.insert(&episodic::EpisodicMemory {
            id: format!("ep{i}"),
            domain: "coding".into(),
            content: format!("user ran cargo nextest then asked for clippy in session {i}").into(),
            summary: Some("user runs nextest before clippy".into()),
            importance: 0.75,
            stability: 1.0,
            ..Default::default()
        }).await.unwrap();
    }

    // Run micro-Reforge with scripted handler that proposes the new pattern.
    let handler = std::sync::Arc::new(ScriptedMicroHandler(MicroReforgeOutput {
        proposed_rules: vec![ProposedRule {
            domain: "coding".into(),
            rule_text: "After cargo nextest passes, run clippy.".into(),
            confidence: 0.85,
            signal_count: 10,
            evidence_episodic_ids: (0..10).map(|i| format!("ep{i}")).collect(),
        }],
        notes: None,
    }));

    let cfg = MicroReforgeConfig::default();
    let svc = MicroReforgeService::new(pool.clone(), cfg);
    for _ in 0..15 { svc.note_turn().await.unwrap(); }
    let accepted = svc.run("turn_threshold", handler, &rule_repo, &ep_repo, &obs_repo).await.unwrap();

    assert_eq!(accepted, 1);
    let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
    assert_eq!(rules.len(), 2, "rules: {:?}", rules.iter().map(|r| &r.rule_text).collect::<Vec<_>>());

    // Critic round.
    let f1 = semantic_fact::SemanticFact::new("user", "uses", "cargo-nextest", 0.85, "t");
    let f2 = semantic_fact::SemanticFact::new("user", "loves", "Pluto", 0.6, "t"); // hallucination
    fact_repo.upsert(&f1).await.unwrap();
    fact_repo.upsert(&f2).await.unwrap();

    struct StrictCritic;
    #[async_trait::async_trait]
    impl ExtractionCriticHandler for StrictCritic {
        async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
            Ok(ExtractionCriticOutput {
                verdicts: input.extracted_facts.iter().map(|f| Verdict {
                    fact_id: f.fact_id.clone(),
                    verdict: if f.object == "Pluto" { "hallucinated".into() } else { "grounded".into() },
                    reason: "test".into(),
                }).collect(),
                missed_facts: vec![],
            })
        }
    }
    cognitive::services::background::run_critic_judgment(
        "user uses cargo-nextest for testing.",
        vec![f1.clone(), f2.clone()],
        std::sync::Arc::new(StrictCritic),
        &fact_repo, &crit_log,
    ).await;

    let f2_after = fact_repo.find_by_id(&f2.id).await.unwrap().unwrap();
    let f1_after = fact_repo.find_by_id(&f1.id).await.unwrap().unwrap();
    assert!(f2_after.stability < f1_after.stability);
    assert_eq!(crit_log.list_unreviewed(10).await.unwrap().len(), 2);

    // Community: assign new entity to existing community.
    let nextest = entity_repo.upsert_entity("cargo-nextest", "tool", None, "t", None).await.unwrap();
    let cargo = entity_repo.upsert_entity("cargo", "tool", None, "t", None).await.unwrap();
    let rust_tools = community_repo.create("rust_tools", "Rust toolchain", None).await.unwrap();
    community_repo.add_member(&rust_tools.id, &cargo.id).await.unwrap();

    co_activation::CoActivationRepo::new(pool.clone()).increment_pair(&nextest.id, &cargo.id, 1.5).await.unwrap();

    struct AlwaysConfirm;
    #[async_trait::async_trait]
    impl cmo::AsyncConfirmFn for AlwaysConfirm {
        async fn confirm(&self, _e: &str, _t: &str, _c: &str, _s: &str, _x: f64) -> bool { true }
    }
    cmo::run_for_session(&entity_repo, &community_repo, vec![nextest.id.clone()],
        std::sync::Arc::new(AlwaysConfirm)).await;

    let cs = community_repo.list_for_entity(&nextest.id).await.unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].id, rust_tools.id);
}

struct ScriptedMicroHandler(cognitive::services::micro_reforge_types::MicroReforgeOutput);
#[async_trait::async_trait]
impl cognitive::services::micro_reforge::MicroReforgeHandler for ScriptedMicroHandler {
    async fn synthesize(&self, _: cognitive::services::micro_reforge_types::MicroReforgeInput)
        -> common::Result<cognitive::services::micro_reforge_types::MicroReforgeOutput>
    { Ok(self.0.clone()) }
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p cognitive --test phase_b_continuous_learning
```

Expected: PASS.

- [ ] **Step 3: Commit.**

```bash
git add crates/cognitive/tests/phase_b_continuous_learning.rs
git commit -m "test(cognitive): Phase B end-to-end across Tracks 4/5/11 (KCA)"
```

---

### Task BIT.2: Critic property test — random hallucinations always demote stability

**Files:**
- Create: `crates/cognitive/tests/phase_b_critic_proptest.rs`

- [ ] **Step 1: Create.**

```rust
use proptest::prelude::*;
use cognitive::services::background::run_critic_judgment;
use cognitive::services::extraction_critic::ExtractionCriticHandler;
use cognitive::services::extraction_critic_types::*;
use cognitive::repos::{semantic_fact::*, extraction_critic_log::*};
use storage::StoragePool;

struct RandomCritic { hallucinate_rate: f64 }
#[async_trait::async_trait]
impl ExtractionCriticHandler for RandomCritic {
    async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
        let mut rng = oorandom::Rand64::new(0);
        Ok(ExtractionCriticOutput {
            verdicts: input.extracted_facts.iter().map(|f| {
                let halluc = (rng.rand_float() as f64) < self.hallucinate_rate;
                Verdict {
                    fact_id: f.fact_id.clone(),
                    verdict: if halluc { "hallucinated".into() } else { "grounded".into() },
                    reason: "rand".into(),
                }
            }).collect(),
            missed_facts: vec![],
        })
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn hallucinated_facts_always_have_lower_stability(
        n in 5usize..20,
        rate in 0.1f64..0.9
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let fr = SemanticFactRepo::new(pool.clone());
            let lr = ExtractionCriticLogRepo::new(pool.clone());

            let mut facts = Vec::with_capacity(n);
            for i in 0..n {
                let f = SemanticFact::new(&format!("S{i}"), "p", &format!("O{i}"), 0.8, "t");
                fr.upsert(&f).await.unwrap();
                facts.push(f);
            }

            run_critic_judgment(
                "test", facts.clone(),
                std::sync::Arc::new(RandomCritic { hallucinate_rate: rate }),
                &fr, &lr,
            ).await;

            for f in &facts {
                let after = fr.find_by_id(&f.id).await.unwrap().unwrap();
                let log = lr.list_unreviewed(100).await.unwrap();
                let log_for_fact = log.iter().find(|e| e.fact_id == f.id).unwrap();
                if log_for_fact.verdict == "hallucinated" {
                    prop_assert!(after.stability <= 0.5, "hallucinated should have stability ≤ 0.5, got {}", after.stability);
                } else {
                    prop_assert!(after.stability == 1.0, "grounded should have stability 1.0, got {}", after.stability);
                }
            }
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Add `oorandom` to dev-deps.**

In `crates/cognitive/Cargo.toml`:

```toml
[dev-dependencies]
oorandom = "11"
```

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p cognitive --test phase_b_critic_proptest
```

Expected: 16 cases PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/tests/phase_b_critic_proptest.rs crates/cognitive/Cargo.toml
git commit -m "test(cognitive): proptest — critic verdicts always demote correctly (KCA Track 5)"
```

---

### Task BIT.3: Workspace sweep + clippy

- [ ] **Step 1:**

```bash
cargo nextest run --workspace
```

- [ ] **Step 2:**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] **Step 3:**

```bash
cargo fmt --all --check
```

All green; if not, fix.

- [ ] **Step 4: Marker commit.**

```bash
git commit --allow-empty -m "test(workspace): KCA Phase B green — continuous learning live"
```

---

# Phase B Self-Review

1. **Spec coverage:** Tracks 4, 5, 11 — three independent tasks each with handler + service + integration. ✅
2. **No placeholders:** No TODOs, no "similar to". ✅
3. **Type consistency:** `MicroReforgeConfig`, `ExtractionCriticOutput`, `CommunityMatch` consistent across modules.
4. **Migrations registered:** 011 + 012 in `cognitive/src/lib.rs` migrations list.
5. **Tracing:** All new public services use tracing on warn/error paths. AppCore handlers (none added in Phase B — all wiring stays in cognitive) — N/A.
6. **No `#[allow(dead_code)]` introduced.**
7. **All new types serde + Debug + Clone**.
8. **Risk register:** Track 4 dedup gate exercised in BIT.1; Track 5 stability halving (not supersede) verified in proptest; Track 11 confidence threshold (`score < 1.0` skip) exercised.

---

**Phase B complete.** Continue to [`2026-04-29-kca-phase-c-retrieval-intelligence.md`](2026-04-29-kca-phase-c-retrieval-intelligence.md).
