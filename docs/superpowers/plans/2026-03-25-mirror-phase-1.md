# The Mirror — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Routing Mirror + weekly narrative + conversational Mirror input — the "my brain watches how it thinks and I can ask it why" moment.

**Architecture:** Event-driven subscriber in `crates/cognitive/src/mirror/` accumulates `SkillRouted` events into hourly routing snapshots. A `NarrativeHandler` trait (implemented in agent) generates first-person weekly narratives via LLM. A thin `MirrorFacade` serves the UI. Frontend gets a new top-level Mirror tab with routing donut, narrative card, snippet feed, and conversational input.

**Tech Stack:** Rust (cognitive/bus/agent/app-core/desktop crates), SQLite (mirror tables), React + Tailwind v4 (desktop-ui), Tauri 2 commands

**Spec:** `docs/superpowers/specs/2026-03-25-mirror-self-reflection-layer-design.md`

---

## File Structure

### New files (Rust)

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/mirror/mod.rs` | Module exports |
| `crates/cognitive/src/mirror/types.rs` | `RoutingSnapshot`, `SkillRouteStats`, `TrendNarrative`, `NarrativeSnippet`, `MirrorState`, `MirrorResponse`, `UserFeedback`, `FeedbackTarget`, `SuggestedAction`, `MirrorAlertType` |
| `crates/cognitive/src/mirror/repo.rs` | `MirrorRepo` — CRUD for Phase 1 tables (routing_snapshots, trend_narratives, snippets) |
| `crates/cognitive/src/mirror/subscribers/mod.rs` | Subscriber module exports |
| `crates/cognitive/src/mirror/subscribers/routing.rs` | `RoutingMirrorSubscriber` — accumulates `SkillRouted`, hourly flush, drift detection |
| `crates/cognitive/src/mirror/narratives.rs` | `NarrativeHandler` trait, `snippet_from_alert()` template function, `NarrativeContext` |
| `crates/cognitive/src/mirror/facade.rs` | `MirrorFacade` — public API (Phase 1 subset) |
| `crates/cognitive/src/mirror/engine.rs` | `MirrorEngine` — starts subscribers, produces facade |
| `crates/cognitive/migrations/003_mirror_tables.sql` | Phase 1 mirror tables |
| `crates/desktop/src/commands/mirror.rs` | Tauri command adapters for Mirror |

### New files (Frontend)

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/mirror/MirrorPage.tsx` | Top-level Mirror page layout |
| `desktop-ui/src/features/mirror/components/RoutingDonut.tsx` | Skill distribution donut chart |
| `desktop-ui/src/features/mirror/components/NarrativeCard.tsx` | Weekly narrative display + feedback |
| `desktop-ui/src/features/mirror/components/SnippetFeed.tsx` | Live alert card feed |
| `desktop-ui/src/features/mirror/components/MirrorInput.tsx` | Conversational "Ask the Mirror" input |

### Modified files

| File | Change |
|------|--------|
| `crates/bus/src/domain_events.rs` | Add `SkillRouted` variant |
| `crates/desktop-shared/src/types.rs` | Add `MirrorSnippet` to `EntityKind` |
| `crates/app-core/src/events.rs` | Map `MirrorSnippetCreated` → entity update |
| `crates/agent/src/agent_runtime/runtime.rs` | Emit `SkillRouted` after skill selection |
| `crates/cognitive/src/repos/mod.rs` | Add mirror migration to `cognitive_migrations()` |
| `crates/cognitive/src/lib.rs` | Re-export mirror module |
| `crates/app-core/src/init/cron.rs` | Register `JOB_MIRROR_WEEKLY_NARRATIVE` and `JOB_MIRROR_CLEANUP` |
| `crates/desktop/src/commands/mod.rs` | Add mirror command module |
| `desktop-ui/src/app/layouts/Sidebar.tsx` | Add Mirror nav item |
| `desktop-ui/src/app/App.tsx` (or router) | Add Mirror route |

---

## Task 1: Add `SkillRouted` Domain Event

**Files:**
- Modify: `crates/bus/src/domain_events.rs:13-392`
- Modify: `crates/agent/src/agent_runtime/runtime.rs:304-310`

- [ ] **Step 1: Add `SkillRouted` variant to `DomainEvent`**

In `crates/bus/src/domain_events.rs`, add to the enum (near the existing transparency events like `AgentSelected`):

```rust
SkillRouted {
    skill_name: String,
    confidence: f64,
    source: String,
    trigger_phrases: Vec<String>,
    session_key: String,
},
```

- [ ] **Step 2: Build to verify compilation**

Run: `cargo build -p bus`
Expected: compiles successfully. Any `match` on `DomainEvent` will warn about non-exhaustive — fix each match arm with `SkillRouted { .. } => {}` or by adding to existing wildcard arms.

- [ ] **Step 3: Fix all match exhaustiveness warnings**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep -i "SkillRouted\|non-exhaustive"`
For each match that needs updating (likely in `crates/cognitive/src/services/salience.rs` and `crates/cognitive/src/services/background.rs`), add `SkillRouted { .. } => SalienceVerdict::Discard` or the appropriate wildcard handling.

- [ ] **Step 4: Emit `SkillRouted` in AgentRuntime**

In `crates/agent/src/agent_runtime/runtime.rs`, after the `AgentSelected` event emission (around line 310), add:

```rust
if let Some(bus) = &self.domain_event_bus {
    let _ = bus.send(DomainEvent::SkillRouted {
        skill_name: profile.name.clone(),
        confidence: classification.confidence,
        source: format!("{:?}", classification.source),
        trigger_phrases: vec![], // populated from router match data if available
        session_key: ctx.session_key.clone(),
    });
}
```

Note: Check the exact variable names by reading the surrounding code. The `classification` comes from the intent analysis result. The `profile` comes from the skill router. Adapt field names to match what's in scope.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p agent`
Expected: compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/agent/src/agent_runtime/runtime.rs
# Also add any files where match arms were fixed
git commit -m "feat(mirror): add SkillRouted domain event and emit from AgentRuntime"
```

---

## Task 2: Add `MirrorSnippet` EntityKind + SSE Wiring

**Files:**
- Modify: `crates/desktop-shared/src/types.rs:48-89`
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add `MirrorSnippetCreated` to `DomainEvent`**

In `crates/bus/src/domain_events.rs`:

```rust
MirrorSnippetCreated {
    snippet_id: String,
    headline: String,
},
```

- [ ] **Step 2: Add `MirrorSnippet` to `EntityKind`**

In `crates/desktop-shared/src/types.rs`, add to the `EntityKind` enum:

```rust
MirrorSnippet,
```

And in the `from_str` / parse method, add:

```rust
"mirrorsnippet" | "mirror_snippet" => Some(EntityKind::MirrorSnippet),
```

- [ ] **Step 3: Add SSE bridge in app-core events**

In `crates/app-core/src/events.rs`, add handling for the new event. Follow the existing pattern for how other `DomainEvent` variants are mapped to `emit_entity_updated` calls. Add:

```rust
DomainEvent::MirrorSnippetCreated { snippet_id, .. } => {
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &snippet_id);
}
```

This bridges the `DomainEvent` to the frontend SSE stream so `SnippetFeed` receives live updates.

- [ ] **Step 4: Fix all match exhaustiveness**

Run: `cargo build --workspace`
Fix any match arms that need the new variants.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/types.rs crates/bus/src/domain_events.rs crates/app-core/src/events.rs
git commit -m "feat(mirror): add MirrorSnippet entity kind, event, and SSE bridge"
```

---

## Task 3: Mirror Types

**Files:**
- Create: `crates/cognitive/src/mirror/mod.rs`
- Create: `crates/cognitive/src/mirror/types.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Create mirror module structure**

Create `crates/cognitive/src/mirror/mod.rs`:

```rust
pub mod types;

pub use types::*;
```

- [ ] **Step 2: Create types**

Create `crates/cognitive/src/mirror/types.rs` with all Phase 1 types:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorState {
    pub last_routing_snapshot: Option<RoutingSnapshot>,
    pub latest_trend_narrative: Option<TrendNarrative>,
    pub pending_snippets: Vec<NarrativeSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSnapshot {
    pub id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub window_hours: u8,
    pub total_messages: u32,
    pub distribution: HashMap<String, SkillRouteStats>,
    pub fallback_rate: f64,
    pub avg_routing_confidence: f64,
    pub low_confidence_count: u32,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRouteStats {
    pub count: u32,
    pub percentage: f64,
    pub avg_confidence: f64,
    pub top_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendNarrative {
    pub id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
    pub experiment_summary: String,
    pub meta_rule_updates: Vec<String>,
    pub full_narrative: String,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSnippet {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub alert_type: MirrorAlertType,
    pub headline: String,
    pub body: String,
    pub suggested_action: Option<SuggestedAction>,
    pub user_feedback: Option<UserFeedback>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorResponse {
    pub answer: String,
    pub data_sources_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserFeedback {
    Helpful,
    NotHelpful,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackTarget {
    Narrative,
    Snippet,
    Routing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MirrorAlertType {
    RoutingDrift,
    TrialUnpromising,
    MetaRuleProposed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestedAction {
    BoostSkill { skill: String },
    ViewDetails,
    #[serde(other)]
    Unknown,  // Forward compatibility: Phase 2-5 may add variants
}

/// Context assembled for the NarrativeHandler LLM call
#[derive(Debug, Clone, Serialize)]
pub struct NarrativeContext {
    pub period: (DateTime<Utc>, DateTime<Utc>),
    pub routing_snapshots: Vec<RoutingSnapshot>,
    pub correction_count: u32,
    pub top_skills_by_usage: Vec<(String, f64)>,
    pub past_narrative_feedback: Vec<UserFeedback>,
}

/// Output from the NarrativeHandler LLM call
#[derive(Debug, Clone, Deserialize)]
pub struct GeneratedNarrative {
    pub full_narrative: String,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
}

/// Alert emitted by subscribers when patterns are detected
#[derive(Debug, Clone)]
pub enum MirrorAlert {
    RoutingDrift {
        skill: String,
        delta: f64,
        suggestion: String,
    },
}
```

- [ ] **Step 3: Register mirror module in cognitive lib**

In `crates/cognitive/src/lib.rs`, add:

```rust
pub mod mirror;
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p cognitive`
Expected: compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/ crates/cognitive/src/lib.rs
git commit -m "feat(mirror): add Phase 1 mirror types"
```

---

## Task 4: Mirror Migration

**Files:**
- Create: `crates/cognitive/migrations/003_mirror_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs:81-97`

- [ ] **Step 1: Write the migration SQL**

Create `crates/cognitive/migrations/003_mirror_tables.sql`:

```sql
-- Mirror Phase 1 tables

CREATE TABLE IF NOT EXISTS mirror_routing_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    total_messages INTEGER NOT NULL,
    distribution_json TEXT NOT NULL,
    fallback_rate REAL NOT NULL,
    avg_routing_confidence REAL NOT NULL,
    low_confidence_count INTEGER NOT NULL DEFAULT 0,
    user_feedback TEXT
);
CREATE INDEX IF NOT EXISTS idx_routing_snapshots_time ON mirror_routing_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS mirror_trend_narratives (
    id TEXT PRIMARY KEY,
    generated_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    routing_summary TEXT NOT NULL,
    improvement_highlights_json TEXT NOT NULL,
    experiment_summary TEXT NOT NULL,
    meta_rule_updates_json TEXT NOT NULL,
    full_narrative TEXT NOT NULL,
    user_feedback TEXT
);
CREATE INDEX IF NOT EXISTS idx_trend_narratives_time ON mirror_trend_narratives(generated_at);

CREATE TABLE IF NOT EXISTS mirror_snippets (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    alert_type TEXT NOT NULL,
    headline TEXT NOT NULL,
    body TEXT NOT NULL,
    action_json TEXT,
    user_feedback TEXT,
    dismissed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_snippets_created ON mirror_snippets(created_at);
```

- [ ] **Step 2: Register migration**

In `crates/cognitive/src/repos/mod.rs`, add a new entry to the `cognitive_migrations()` function:

```rust
FeatureMigration {
    feature_name: "cognitive_mirror".to_string(),
    version: 1,
    description: "Mirror Phase 1 tables (routing snapshots, trend narratives, snippets)".to_string(),
    sql: include_str!("../../migrations/003_mirror_tables.sql").to_string(),
},
```

Note: Use a separate feature_name (`cognitive_mirror`) to avoid bumping the existing `cognitive` migration version. This keeps Phase 1 tables independent.

- [ ] **Step 3: Build and test migration runs**

Run: `cargo nextest run -p cognitive -E 'test(mirror)' -- --no-capture`
This may not find tests yet, but verify compilation: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/003_mirror_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(mirror): add Phase 1 database migration"
```

---

## Task 5: MirrorRepo

**Files:**
- Create: `crates/cognitive/src/mirror/repo.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Write the repo tests first**

At the bottom of `crates/cognitive/src/mirror/repo.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> MirrorRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Run cognitive migrations including mirror
        storage::StoragePool::run_feature_migrations(pool.inner(), &crate::repos::cognitive_migrations()).await.unwrap();
        MirrorRepo::new(pool.clone())
    }

    #[tokio::test]
    async fn test_insert_and_get_routing_snapshot() {
        let repo = setup().await;
        let snap = RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            window_hours: 1,
            total_messages: 42,
            distribution: HashMap::from([(
                "general".to_string(),
                SkillRouteStats {
                    count: 30,
                    percentage: 71.4,
                    avg_confidence: 0.82,
                    top_triggers: vec!["hello".to_string()],
                },
            )]),
            fallback_rate: 0.714,
            avg_routing_confidence: 0.82,
            low_confidence_count: 3,
            user_feedback: None,
        };
        repo.insert_routing_snapshot(&snap).await.unwrap();
        let loaded = repo.get_latest_routing_snapshot().await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().total_messages, 42);
    }

    #[tokio::test]
    async fn test_insert_and_get_snippet() {
        let repo = setup().await;
        let snippet = NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            alert_type: MirrorAlertType::RoutingDrift,
            headline: "Test".to_string(),
            body: "Test body".to_string(),
            suggested_action: None,
            user_feedback: None,
            dismissed_at: None,
        };
        repo.insert_snippet(&snippet).await.unwrap();
        let pending = repo.get_pending_snippets().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].headline, "Test");
    }

    #[tokio::test]
    async fn test_retention_cleanup() {
        let repo = setup().await;
        // Insert a snapshot older than 30 days
        let mut old = RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now() - chrono::Duration::days(31),
            window_hours: 1,
            total_messages: 10,
            distribution: HashMap::new(),
            fallback_rate: 0.5,
            avg_routing_confidence: 0.5,
            low_confidence_count: 0,
            user_feedback: None,
        };
        repo.insert_routing_snapshot(&old).await.unwrap();

        // Insert a recent one
        old.id = Uuid::new_v4();
        old.captured_at = Utc::now();
        repo.insert_routing_snapshot(&old).await.unwrap();

        let deleted = repo.cleanup_old_snapshots(30).await.unwrap();
        assert_eq!(deleted, 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: FAIL — `MirrorRepo` not defined yet

- [ ] **Step 3: Implement MirrorRepo**

Write the implementation above the test module in `crates/cognitive/src/mirror/repo.rs`:

```rust
use chrono::{DateTime, Utc};
use common::Result;
use sqlx::SqlitePool;
use uuid::Uuid;
use std::collections::HashMap;

use super::types::*;

#[derive(Clone)]
pub struct MirrorRepo {
    pool: storage::StoragePool,
}

impl MirrorRepo {
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    pub async fn insert_routing_snapshot(&self, snap: &RoutingSnapshot) -> Result<()> {
        let id = snap.id.to_string();
        let captured_at = snap.captured_at.to_rfc3339();
        let distribution_json = serde_json::to_string(&snap.distribution)?;
        let feedback = snap.user_feedback.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default());

        sqlx::query(
            "INSERT INTO mirror_routing_snapshots (id, captured_at, window_hours, total_messages, distribution_json, fallback_rate, avg_routing_confidence, low_confidence_count, user_feedback) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&captured_at)
        .bind(snap.window_hours as i32)
        .bind(snap.total_messages as i32)
        .bind(&distribution_json)
        .bind(snap.fallback_rate)
        .bind(snap.avg_routing_confidence)
        .bind(snap.low_confidence_count as i32)
        .bind(&feedback)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn get_latest_routing_snapshot(&self) -> Result<Option<RoutingSnapshot>> {
        let row = sqlx::query_as::<_, RoutingSnapshotRow>(
            "SELECT * FROM mirror_routing_snapshots ORDER BY captured_at DESC LIMIT 1"
        )
        .fetch_optional(self.pool.inner())
        .await?;
        Ok(row.map(|r| r.into()))
    }

    pub async fn get_routing_history(&self, days: u32) -> Result<Vec<RoutingSnapshot>> {
        let cutoff = (Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        let rows = sqlx::query_as::<_, RoutingSnapshotRow>(
            "SELECT * FROM mirror_routing_snapshots WHERE captured_at > ? ORDER BY captured_at DESC"
        )
        .bind(&cutoff)
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn insert_snippet(&self, snippet: &NarrativeSnippet) -> Result<()> {
        let id = snippet.id.to_string();
        let created_at = snippet.created_at.to_rfc3339();
        let alert_type = serde_json::to_string(&snippet.alert_type)?;
        let action_json = snippet.suggested_action.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default());
        let feedback = snippet.user_feedback.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default());
        let dismissed = snippet.dismissed_at.map(|d| d.to_rfc3339());

        sqlx::query(
            "INSERT INTO mirror_snippets (id, created_at, alert_type, headline, body, action_json, user_feedback, dismissed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&created_at)
        .bind(&alert_type)
        .bind(&snippet.headline)
        .bind(&snippet.body)
        .bind(&action_json)
        .bind(&feedback)
        .bind(&dismissed)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn get_pending_snippets(&self) -> Result<Vec<NarrativeSnippet>> {
        let rows = sqlx::query_as::<_, SnippetRow>(
            "SELECT * FROM mirror_snippets WHERE dismissed_at IS NULL ORDER BY created_at DESC LIMIT 20"
        )
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn insert_trend_narrative(&self, narrative: &TrendNarrative) -> Result<()> {
        let id = narrative.id.to_string();
        let generated_at = narrative.generated_at.to_rfc3339();
        let period_start = narrative.period_start.to_rfc3339();
        let period_end = narrative.period_end.to_rfc3339();
        let highlights = serde_json::to_string(&narrative.improvement_highlights)?;
        let meta_rules = serde_json::to_string(&narrative.meta_rule_updates)?;
        let feedback = narrative.user_feedback.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default());

        sqlx::query(
            "INSERT INTO mirror_trend_narratives (id, generated_at, period_start, period_end, routing_summary, improvement_highlights_json, experiment_summary, meta_rule_updates_json, full_narrative, user_feedback) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&generated_at)
        .bind(&period_start)
        .bind(&period_end)
        .bind(&narrative.routing_summary)
        .bind(&highlights)
        .bind(&narrative.experiment_summary)
        .bind(&meta_rules)
        .bind(&narrative.full_narrative)
        .bind(&feedback)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn get_latest_narrative(&self) -> Result<Option<TrendNarrative>> {
        let row = sqlx::query_as::<_, TrendNarrativeRow>(
            "SELECT * FROM mirror_trend_narratives ORDER BY generated_at DESC LIMIT 1"
        )
        .fetch_optional(self.pool.inner())
        .await?;
        Ok(row.map(|r| r.into()))
    }

    pub async fn get_narratives(&self, limit: u32) -> Result<Vec<TrendNarrative>> {
        let rows = sqlx::query_as::<_, TrendNarrativeRow>(
            "SELECT * FROM mirror_trend_narratives ORDER BY generated_at DESC LIMIT ?"
        )
        .bind(limit as i32)
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn update_feedback(&self, target: &FeedbackTarget, item_id: Uuid, feedback: &UserFeedback) -> Result<()> {
        let id = item_id.to_string();
        let fb = serde_json::to_string(feedback)?;
        let table = match target {
            FeedbackTarget::Narrative => "mirror_trend_narratives",
            FeedbackTarget::Snippet => "mirror_snippets",
            FeedbackTarget::Routing => "mirror_routing_snapshots",
        };
        let sql = format!("UPDATE {} SET user_feedback = ? WHERE id = ?", table);
        sqlx::query(&sql).bind(&fb).bind(&id).execute(self.pool.inner()).await?;
        Ok(())
    }

    pub async fn cleanup_old_snapshots(&self, max_age_days: u32) -> Result<u64> {
        let cutoff = (Utc::now() - chrono::Duration::days(max_age_days as i64)).to_rfc3339();
        let result = sqlx::query(
            "DELETE FROM mirror_routing_snapshots WHERE captured_at < ? AND window_hours = 1"
        )
        .bind(&cutoff)
        .execute(self.pool.inner())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn cleanup_old_snippets(&self, max_age_days: u32) -> Result<u64> {
        let cutoff = (Utc::now() - chrono::Duration::days(max_age_days as i64)).to_rfc3339();
        let result = sqlx::query(
            "DELETE FROM mirror_snippets WHERE created_at < ?"
        )
        .bind(&cutoff)
        .execute(self.pool.inner())
        .await?;
        Ok(result.rows_affected())
    }
}
```

You'll also need `RoutingSnapshotRow`, `SnippetRow`, and `TrendNarrativeRow` structs with `#[derive(sqlx::FromRow)]` and `From<Row> for Type` impls. Follow the pattern in existing repos like `crates/cognitive/src/repos/semantic_fact.rs` for the row-to-domain conversion approach.

- [ ] **Step 4: Update mirror/mod.rs**

```rust
pub mod types;
pub mod repo;

pub use types::*;
pub use repo::MirrorRepo;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all 3 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/
git commit -m "feat(mirror): add MirrorRepo with tests for routing snapshots, snippets, narratives"
```

---

## Task 6: NarrativeHandler Trait + Alert Snippets

**Files:**
- Create: `crates/cognitive/src/mirror/narratives.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Write test for snippet_from_alert**

At the bottom of `crates/cognitive/src/mirror/narratives.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_from_routing_drift() {
        let alert = MirrorAlert::RoutingDrift {
            skill: "finance-management".to_string(),
            delta: 18.5,
            suggestion: "strengthen finance routing".to_string(),
        };
        let snippet = snippet_from_alert(&alert);
        assert!(snippet.headline.contains("finance-management"));
        assert_eq!(snippet.alert_type, MirrorAlertType::RoutingDrift);
        assert!(snippet.suggested_action.is_some());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(snippet_from)'`
Expected: FAIL — function not defined

- [ ] **Step 3: Implement NarrativeHandler trait + snippet_from_alert**

```rust
use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use uuid::Uuid;

use super::types::*;

/// Trait for LLM-powered narrative generation.
/// Defined in cognitive (L3-L4), implemented in agent (L5).
#[async_trait]
pub trait NarrativeHandler: Send + Sync {
    async fn generate_narrative(&self, ctx: NarrativeContext) -> Result<GeneratedNarrative>;
    async fn generate_mirror_response(&self, query: &str, ctx: NarrativeContext) -> Result<String>;
}

/// Convert a MirrorAlert into a user-facing NarrativeSnippet (no LLM needed).
pub fn snippet_from_alert(alert: &MirrorAlert) -> NarrativeSnippet {
    match alert {
        MirrorAlert::RoutingDrift { skill, delta, suggestion } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            alert_type: MirrorAlertType::RoutingDrift,
            headline: format!("I'm routing more to {} lately", skill),
            body: format!(
                "Your {} usage shifted {:.0}% this week. \
                 This might mean your focus is changing — or I might be \
                 misreading some messages. Want me to lean into this?",
                skill, delta
            ),
            suggested_action: Some(SuggestedAction::BoostSkill {
                skill: skill.clone(),
            }),
            user_feedback: None,
            dismissed_at: None,
        },
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(snippet_from)'`
Expected: PASS

- [ ] **Step 5: Update mirror/mod.rs**

Add `pub mod narratives;` and `pub use narratives::{NarrativeHandler, snippet_from_alert};`

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/narratives.rs crates/cognitive/src/mirror/mod.rs
git commit -m "feat(mirror): add NarrativeHandler trait and snippet_from_alert templates"
```

---

## Task 7: RoutingMirrorSubscriber

**Files:**
- Create: `crates/cognitive/src/mirror/subscribers/mod.rs`
- Create: `crates/cognitive/src/mirror/subscribers/routing.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Write tests**

In `crates/cognitive/src/mirror/subscribers/routing.rs`, write test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulate_skill_routed() {
        let mut subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.85, vec!["hello".to_string()]);
        subscriber.accumulate("general", 0.90, vec![]);
        subscriber.accumulate("finance-management", 0.75, vec!["budget".to_string()]);

        let snapshot = subscriber.build_snapshot();
        assert_eq!(snapshot.total_messages, 3);
        assert_eq!(snapshot.distribution.len(), 2);
        assert_eq!(snapshot.distribution["general"].count, 2);
        assert!((snapshot.distribution["general"].avg_confidence - 0.875).abs() < 0.01);
        assert_eq!(snapshot.distribution["finance-management"].count, 1);
    }

    #[test]
    fn test_drift_detection_no_baseline() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        let snapshot = RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            window_hours: 1,
            total_messages: 10,
            distribution: HashMap::new(),
            fallback_rate: 0.5,
            avg_routing_confidence: 0.8,
            low_confidence_count: 0,
            user_feedback: None,
        };
        // No baseline history → no drift alert
        let alert = subscriber.detect_drift(&snapshot, &[]);
        assert!(alert.is_none());
    }

    #[test]
    fn test_drift_detection_high_fallback() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        let snapshot = RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            window_hours: 1,
            total_messages: 10,
            distribution: HashMap::from([(
                "general".to_string(),
                SkillRouteStats { count: 8, percentage: 80.0, avg_confidence: 0.5, top_triggers: vec![] },
            )]),
            fallback_rate: 0.80,
            avg_routing_confidence: 0.5,
            low_confidence_count: 3,
            user_feedback: None,
        };
        let alert = subscriber.detect_drift(&snapshot, &[]);
        assert!(alert.is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(accumulate_skill\|drift_detection)'`
Expected: FAIL

- [ ] **Step 3: Implement RoutingMirrorSubscriber**

```rust
use bus::DomainEvent;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::repo::MirrorRepo;
use super::super::types::*;
use super::super::narratives::snippet_from_alert;

pub struct RoutingMirrorSubscriber {
    accumulator: DashMap<String, SkillRouteAccum>,
    total_count: AtomicU32,
    low_confidence_count: AtomicU32,
    repo: Option<MirrorRepo>,
}

struct SkillRouteAccum {
    count: u32,
    confidence_sum: f64,
    trigger_hits: HashMap<String, u32>,
}

impl RoutingMirrorSubscriber {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo: Some(repo),
        }
    }

    #[cfg(test)]
    fn new_for_test() -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo: None,
        }
    }

    pub fn accumulate(&self, skill_name: &str, confidence: f64, triggers: Vec<String>) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        if confidence < 0.4 {
            self.low_confidence_count.fetch_add(1, Ordering::Relaxed);
        }
        let mut entry = self.accumulator
            .entry(skill_name.to_string())
            .or_insert_with(|| SkillRouteAccum {
                count: 0,
                confidence_sum: 0.0,
                trigger_hits: HashMap::new(),
            });
        entry.count += 1;
        entry.confidence_sum += confidence;
        for trigger in triggers {
            *entry.trigger_hits.entry(trigger).or_insert(0) += 1;
        }
    }

    pub fn build_snapshot(&self) -> RoutingSnapshot {
        let total = self.total_count.load(Ordering::Relaxed);
        let low_conf = self.low_confidence_count.load(Ordering::Relaxed);
        let mut distribution = HashMap::new();
        let mut total_confidence = 0.0;
        let mut general_count = 0u32;

        for entry in self.accumulator.iter() {
            let name = entry.key().clone();
            let accum = entry.value();
            let pct = if total > 0 { (accum.count as f64 / total as f64) * 100.0 } else { 0.0 };
            let avg_conf = if accum.count > 0 { accum.confidence_sum / accum.count as f64 } else { 0.0 };
            total_confidence += accum.confidence_sum;

            let mut top_triggers: Vec<(String, u32)> = accum.trigger_hits.iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            top_triggers.sort_by(|a, b| b.1.cmp(&a.1));
            let top_3: Vec<String> = top_triggers.into_iter().take(3).map(|(k, _)| k).collect();

            if name == "general" {
                general_count = accum.count;
            }

            distribution.insert(name, SkillRouteStats {
                count: accum.count,
                percentage: pct,
                avg_confidence: avg_conf,
                top_triggers: top_3,
            });
        }

        let fallback_rate = if total > 0 { general_count as f64 / total as f64 } else { 0.0 };
        let avg_confidence = if total > 0 { total_confidence / total as f64 } else { 0.0 };

        RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            window_hours: 1,
            total_messages: total,
            distribution,
            fallback_rate,
            avg_routing_confidence: avg_confidence,
            low_confidence_count: low_conf,
            user_feedback: None,
        }
    }

    pub fn detect_drift(&self, snapshot: &RoutingSnapshot, history: &[RoutingSnapshot]) -> Option<MirrorAlert> {
        // High fallback rate always triggers
        if snapshot.fallback_rate > 0.70 {
            return Some(MirrorAlert::RoutingDrift {
                skill: "general".to_string(),
                delta: snapshot.fallback_rate * 100.0,
                suggestion: "too many messages falling back to general".to_string(),
            });
        }

        // Need at least 7 daily snapshots for trend comparison
        if history.len() < 7 {
            return None;
        }

        // Compare each skill's share to 7-day average
        let mut avg_distribution: HashMap<String, f64> = HashMap::new();
        for snap in history {
            for (skill, stats) in &snap.distribution {
                *avg_distribution.entry(skill.clone()).or_insert(0.0) += stats.percentage;
            }
        }
        for val in avg_distribution.values_mut() {
            *val /= history.len() as f64;
        }

        for (skill, stats) in &snapshot.distribution {
            let avg_pct = avg_distribution.get(skill).copied().unwrap_or(0.0);
            let delta = stats.percentage - avg_pct;
            if delta.abs() > 15.0 {
                return Some(MirrorAlert::RoutingDrift {
                    skill: skill.clone(),
                    delta,
                    suggestion: if delta > 0.0 {
                        format!("usage of {} is increasing", skill)
                    } else {
                        format!("usage of {} is decreasing", skill)
                    },
                });
            }
        }

        None
    }

    fn reset(&self) {
        self.accumulator.clear();
        self.total_count.store(0, Ordering::Relaxed);
        self.low_confidence_count.store(0, Ordering::Relaxed);
    }

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        let mut flush_interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        flush_interval.tick().await; // skip first immediate tick

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = flush_interval.tick() => {
                    if self.total_count.load(Ordering::Relaxed) > 0 {
                        self.flush().await;
                    }
                }
                event = rx.recv() => {
                    if let Ok(DomainEvent::SkillRouted { skill_name, confidence, trigger_phrases, .. }) = event {
                        self.accumulate(&skill_name, confidence, trigger_phrases);
                    }
                }
            }
        }
    }

    async fn flush(&self) {
        let snapshot = self.build_snapshot();
        if let Some(repo) = &self.repo {
            // Get recent daily snapshots for drift detection
            let history = repo.get_routing_history(7).await.unwrap_or_default();
            let daily_snapshots: Vec<_> = history.iter().filter(|s| s.window_hours == 24).cloned().collect();

            if let Some(alert) = self.detect_drift(&snapshot, &daily_snapshots) {
                let snippet = snippet_from_alert(&alert);
                let _ = repo.insert_snippet(&snippet).await;
            }

            let _ = repo.insert_routing_snapshot(&snapshot).await;
        }
        self.reset();
    }
}
```

- [ ] **Step 4: Create subscribers/mod.rs**

```rust
pub mod routing;
pub use routing::RoutingMirrorSubscriber;
```

- [ ] **Step 5: Update mirror/mod.rs**

Add `pub mod subscribers;`

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/mirror/subscribers/
git commit -m "feat(mirror): add RoutingMirrorSubscriber with accumulation, flush, and drift detection"
```

---

## Task 8: MirrorFacade + MirrorEngine

**Files:**
- Create: `crates/cognitive/src/mirror/facade.rs`
- Create: `crates/cognitive/src/mirror/engine.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Write facade test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_state_empty() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        crate::repos::run_cognitive_migrations(&pool).await.unwrap();
        let repo = MirrorRepo::new(pool);
        let facade = MirrorFacade::new(repo);
        let state = facade.get_state().await.unwrap();
        assert!(state.last_routing_snapshot.is_none());
        assert!(state.latest_trend_narrative.is_none());
        assert!(state.pending_snippets.is_empty());
    }
}
```

- [ ] **Step 2: Implement MirrorFacade (Phase 1 subset)**

```rust
use chrono::{DateTime, Utc};
use common::Result;
use uuid::Uuid;

use super::repo::MirrorRepo;
use super::types::*;
use super::narratives::NarrativeHandler;
use std::sync::Arc;

pub struct MirrorFacade {
    repo: MirrorRepo,
    narrative_handler: Option<Arc<dyn NarrativeHandler>>,
}

impl MirrorFacade {
    pub fn new(repo: MirrorRepo) -> Self {
        Self { repo, narrative_handler: None }
    }

    pub fn with_narrative_handler(mut self, handler: Arc<dyn NarrativeHandler>) -> Self {
        self.narrative_handler = Some(handler);
        self
    }

    pub async fn get_state(&self) -> Result<MirrorState> {
        let last_routing_snapshot = self.repo.get_latest_routing_snapshot().await?;
        let latest_trend_narrative = self.repo.get_latest_narrative().await?;
        let pending_snippets = self.repo.get_pending_snippets().await?;

        Ok(MirrorState {
            last_routing_snapshot,
            latest_trend_narrative,
            pending_snippets,
        })
    }

    pub async fn get_routing_history(&self, days: u32) -> Result<Vec<RoutingSnapshot>> {
        self.repo.get_routing_history(days).await
    }

    pub async fn get_narratives(&self, limit: u32) -> Result<Vec<TrendNarrative>> {
        self.repo.get_narratives(limit).await
    }

    pub async fn get_pending_snippets(&self) -> Result<Vec<NarrativeSnippet>> {
        self.repo.get_pending_snippets().await
    }

    pub async fn submit_feedback(&self, item_id: Uuid, target: FeedbackTarget, feedback: UserFeedback) -> Result<()> {
        self.repo.update_feedback(&target, item_id, &feedback).await
    }

    /// Generate and persist a weekly narrative — called by cron job
    pub async fn generate_weekly_narrative(&self) -> Result<TrendNarrative> {
        let handler = self.narrative_handler.as_ref()
            .ok_or_else(|| common::KlyntbotError::internal("NarrativeHandler not configured"))?;

        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        let snapshots = self.repo.get_routing_history(7).await?;

        let ctx = NarrativeContext {
            period: (week_ago, now),
            routing_snapshots: snapshots,
            correction_count: 0, // TODO: populate from episodic memory in Phase 5
            top_skills_by_usage: vec![], // TODO: compute from snapshots
            past_narrative_feedback: vec![], // TODO: load last 3 feedbacks
        };

        let generated = handler.generate_narrative(ctx).await?;

        let narrative = TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: now,
            period_start: week_ago,
            period_end: now,
            routing_summary: generated.routing_summary,
            improvement_highlights: generated.improvement_highlights,
            experiment_summary: String::new(), // Phase 4
            meta_rule_updates: vec![], // Phase 2
            full_narrative: generated.full_narrative,
            user_feedback: None,
        };

        self.repo.insert_trend_narrative(&narrative).await?;
        Ok(narrative)
    }

    pub async fn generate_mirror_response(
        &self,
        query: String,
        period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    ) -> Result<MirrorResponse> {
        let handler = self.narrative_handler.as_ref()
            .ok_or_else(|| common::KlyntbotError::internal("NarrativeHandler not configured"))?;

        let (start, end) = period.unwrap_or_else(|| {
            let now = Utc::now();
            (now - chrono::Duration::days(14), now)
        });

        let snapshots = self.repo.get_routing_history(14).await?;
        let ctx = NarrativeContext {
            period: (start, end),
            routing_snapshots: snapshots,
            correction_count: 0,
            top_skills_by_usage: vec![],
            past_narrative_feedback: vec![],
        };

        let answer = handler.generate_mirror_response(&query, ctx).await?;
        Ok(MirrorResponse {
            answer,
            data_sources_used: vec!["routing_snapshots".to_string()],
        })
    }
}
```

- [ ] **Step 3: Implement MirrorEngine**

```rust
use bus::DomainEventBus;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::facade::MirrorFacade;
use super::repo::MirrorRepo;
use super::subscribers::RoutingMirrorSubscriber;
use super::narratives::NarrativeHandler;
use std::sync::Arc;

pub struct MirrorEngine;

impl MirrorEngine {
    pub fn start(
        repo: MirrorRepo,
        bus: &DomainEventBus,
        narrative_handler: Option<Arc<dyn NarrativeHandler>>,
    ) -> (MirrorFacade, Vec<JoinHandle<()>>, CancellationToken) {
        let shutdown = CancellationToken::new();

        let routing_sub = Arc::new(RoutingMirrorSubscriber::new(repo.clone()));
        let handles = vec![
            tokio::spawn(routing_sub.run(bus.subscribe(), shutdown.clone())),
        ];

        let mut facade = MirrorFacade::new(repo);
        if let Some(handler) = narrative_handler {
            facade = facade.with_narrative_handler(handler);
        }

        (facade, handles, shutdown)
    }
}
```

- [ ] **Step 4: Update mirror/mod.rs with all exports**

```rust
pub mod types;
pub mod repo;
pub mod narratives;
pub mod subscribers;
pub mod facade;
pub mod engine;

pub use types::*;
pub use repo::MirrorRepo;
pub use narratives::{NarrativeHandler, snippet_from_alert};
pub use facade::MirrorFacade;
pub use engine::MirrorEngine;
```

- [ ] **Step 5: Run all mirror tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/
git commit -m "feat(mirror): add MirrorFacade and MirrorEngine for Phase 1"
```

---

## Task 9: NarrativeHandler Implementation in Agent

**Files:**
- Create: `crates/agent/src/adapters/mirror_handlers.rs` (or add to existing `cognitive_handlers.rs`)
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Implement NarrativeHandler**

This follows the same pattern as `LlmExtractionHandler` and `LlmReflectionHandler` in `crates/agent/src/adapters/cognitive_handlers.rs`. Create the implementation:

```rust
use async_trait::async_trait;
use cognitive::mirror::{NarrativeHandler, NarrativeContext, GeneratedNarrative};
use common::Result;
use providers::LlmProvider;
use std::sync::Arc;

pub struct LlmNarrativeHandler {
    provider: Arc<dyn LlmProvider>,
    model: String,
}

impl LlmNarrativeHandler {
    pub fn new(provider: Arc<dyn LlmProvider>, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl NarrativeHandler for LlmNarrativeHandler {
    async fn generate_narrative(&self, ctx: NarrativeContext) -> Result<GeneratedNarrative> {
        let system_prompt = build_narrative_system_prompt();
        let user_prompt = build_narrative_user_prompt(&ctx);

        let messages = vec![
            providers::Message::System { content: system_prompt },
            providers::Message::User { content: providers::UserContent::Text(user_prompt) },
        ];

        let params = providers::ChatParams::new(&self.model)
            .with_temperature(0.7);

        let response = self.provider.chat(&messages, None, &params).await?;
        let text = response.content.unwrap_or_default();

        // Parse structured output or use full text as narrative
        Ok(GeneratedNarrative {
            full_narrative: text.clone(),
            routing_summary: extract_section(&text, "routing").unwrap_or_default(),
            improvement_highlights: vec![],
        })
    }

    async fn generate_mirror_response(&self, query: &str, ctx: NarrativeContext) -> Result<String> {
        let system_prompt = build_mirror_response_system_prompt();
        let user_prompt = format!(
            "User's question: {}\n\nContext:\n- {} routing snapshots available\n- Top skills: {:?}",
            query, ctx.routing_snapshots.len(), ctx.top_skills_by_usage
        );

        let messages = vec![
            providers::Message::System { content: system_prompt },
            providers::Message::User { content: providers::UserContent::Text(user_prompt) },
        ];

        let params = providers::ChatParams::new(&self.model)
            .with_temperature(0.7);

        let response = self.provider.chat(&messages, None, &params).await?;
        Ok(response.content.unwrap_or_default())
    }
}

fn build_narrative_system_prompt() -> String {
    r#"You are writing a weekly reflection for a personal AI assistant's "Mirror" — a self-awareness journal that helps the user understand how their AI brain is evolving.

Write in first person ("I noticed...", "I improved..."). Be warm, specific, and honest.
When things got worse, own it: "I struggled with X this week."
When things improved, celebrate briefly: "That routing fix from Tuesday is paying off."

Do NOT use jargon (no "correction_rate", "semantic weight"). Translate everything into human terms ("I misunderstood you less often", "I got better at knowing when you're asking about money vs tasks").

Structure:
1. Opening: One sentence capturing the week's vibe
2. Routing: How message understanding changed
3. Experiments: What was tested, what worked (if anything)
4. Meta-rules: New self-awareness insights
5. Looking ahead: One concrete thing to watch next week"#.to_string()
}

fn build_mirror_response_system_prompt() -> String {
    r#"You are the Mirror — the self-aware part of a personal AI assistant. The user is asking you about how you think and work.

Answer in first person. Be warm, honest, and specific. Reference actual data from the context provided. If you notice a pattern worth watching, mention it.

Keep your answer concise (2-4 paragraphs). Speak like a thoughtful companion reflecting on your own behavior, not like a dashboard reading numbers."#.to_string()
}

fn extract_section(text: &str, _section: &str) -> Option<String> {
    // Simple extraction — can be improved later
    Some(text.lines().take(3).collect::<Vec<_>>().join(" "))
}
```

- [ ] **Step 2: Register in agent adapters**

Add to `crates/agent/src/adapters/mod.rs`:
```rust
pub mod mirror_handlers;
pub use mirror_handlers::LlmNarrativeHandler;
```

- [ ] **Step 3: Build**

Run: `cargo build -p agent`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/adapters/mirror_handlers.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(mirror): implement LlmNarrativeHandler for weekly narrative and mirror response"
```

---

## Task 10: App-Core Wiring + Cron Jobs

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: App-core initialization (wherever `BackgroundConsolidationService` is started — check `crates/app-core/src/init/` or `crates/app-core/src/state.rs`)

- [ ] **Step 1: Add cron job constants**

In `crates/app-core/src/init/cron.rs`, add near the other JOB_ constants:

```rust
pub const JOB_MIRROR_WEEKLY_NARRATIVE: &str = "mirror_weekly_narrative";
pub const JOB_MIRROR_CLEANUP: &str = "mirror_cleanup";
```

- [ ] **Step 2: Register cron handlers**

Follow the pattern of existing cron registrations (e.g., `JOB_WEEKLY_REFLECTION`). Add two new handlers:

```rust
// Follow the exact registration pattern used by JOB_WEEKLY_REFLECTION in the same file.
// The actual CronService API uses register_handler(name, callback) where callback
// matches the JobCallback type. Check the existing JOB_WEEKLY_REFLECTION handler
// registration and replicate that exact pattern, calling:
//
// For JOB_MIRROR_WEEKLY_NARRATIVE (Sunday 7pm local, cron: "0 19 * * 0"):
//   facade.generate_weekly_narrative().await
//
// For JOB_MIRROR_CLEANUP (daily midnight, cron: "0 0 * * *"):
//   repo.cleanup_old_snapshots(30).await
//   repo.cleanup_old_snippets(90).await
//
// Also add entries to ensure_cron_jobs() for both jobs with the correct
// CronSchedule representations.
```

Note: The exact registration pattern may differ — check how `JOB_WEEKLY_REFLECTION` is registered and follow that pattern exactly.

- [ ] **Step 3: Wire MirrorEngine startup**

In the app initialization code (where `BackgroundConsolidationService` is started), add:

```rust
let mirror_repo = MirrorRepo::new(pool.clone());
let narrative_handler = Arc::new(LlmNarrativeHandler::new(cognitive_provider.clone()));
let (mirror_facade, mirror_handles, mirror_shutdown) = MirrorEngine::start(
    mirror_repo.clone(),
    &domain_event_bus,
    Some(narrative_handler),
);
// Store facade in AppState for Tauri commands
```

- [ ] **Step 4: Build**

Run: `cargo build -p app-core`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/
git commit -m "feat(mirror): wire MirrorEngine startup and register cron jobs"
```

---

## Task 11: Tauri Commands

**Files:**
- Create: `crates/desktop/src/commands/mirror.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

- [ ] **Step 1: Create mirror commands**

First, add `pub mirror_facade: Option<Arc<MirrorFacade>>` to `AppCore` in `crates/app-core/src/state.rs`, and add a `pub fn mirror_facade(&self) -> Option<&MirrorFacade>` accessor method. Populate it during `init_with_sender` after `MirrorEngine::start()` returns the facade. Then create commands following the pattern in `crates/desktop/src/commands/areas.rs`:

```rust
use tauri::State;
use std::sync::Arc;
use app_core::AppCore;

#[tauri::command]
pub async fn get_mirror_state(state: State<'_, Arc<AppCore>>) -> Result<serde_json::Value, String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let result = facade.get_state().await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_routing_history(
    days: u32,
    state: State<'_, Arc<AppCore>>,
) -> Result<serde_json::Value, String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let result = facade.get_routing_history(days).await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_mirror_narratives(
    limit: u32,
    state: State<'_, Arc<AppCore>>,
) -> Result<serde_json::Value, String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let result = facade.get_narratives(limit).await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pending_snippets(
    state: State<'_, Arc<AppCore>>,
) -> Result<serde_json::Value, String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let result = facade.get_pending_snippets().await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_mirror_feedback(
    item_id: String,
    target: String,
    feedback: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<(), String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let id = uuid::Uuid::parse_str(&item_id).map_err(|e| e.to_string())?;
    let target: cognitive::mirror::FeedbackTarget = serde_json::from_str(&format!("\"{}\"", target))
        .map_err(|e| e.to_string())?;
    let fb: cognitive::mirror::UserFeedback = serde_json::from_str(&format!("\"{}\"", feedback))
        .map_err(|e| e.to_string())?;
    facade.submit_feedback(id, target, fb).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_mirror_response(
    query: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<serde_json::Value, String> {
    let facade = state.mirror_facade()
        .ok_or("Mirror not initialized")?;
    let result = facade.generate_mirror_response(query, None).await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[cfg(test)]
pub const DEV_COMMANDS: &[&str] = &[
    "get_mirror_state",
    "get_routing_history",
    "get_mirror_narratives",
    "get_pending_snippets",
    "submit_mirror_feedback",
    "generate_mirror_response",
];
```

- [ ] **Step 2: Register in commands/mod.rs**

Add `pub mod mirror;` and register the commands in the Tauri builder. Also add `mirror::DEV_COMMANDS` to the dev server coverage list.

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/commands/mirror.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(mirror): add Tauri commands for Mirror tab"
```

---

## Task 12: Frontend — Mirror Page + Routing Donut

**Files:**
- Create: `desktop-ui/src/features/mirror/MirrorPage.tsx`
- Create: `desktop-ui/src/features/mirror/components/RoutingDonut.tsx`
- Create: `desktop-ui/src/features/mirror/components/NarrativeCard.tsx`
- Create: `desktop-ui/src/features/mirror/components/SnippetFeed.tsx`
- Create: `desktop-ui/src/features/mirror/components/MirrorInput.tsx`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`
- Modify: App router file

- [ ] **Step 1: Create MirrorPage**

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { RoutingDonut } from "./components/RoutingDonut";
import { NarrativeCard } from "./components/NarrativeCard";
import { SnippetFeed } from "./components/SnippetFeed";
import { MirrorInput } from "./components/MirrorInput";

export function MirrorPage() {
  const { data: state, isLoading } = useQuery("get_mirror_state");

  if (isLoading) {
    return <div className="flex-1 flex items-center justify-center text-muted">Loading Mirror...</div>;
  }

  return (
    <div className="flex-1 flex flex-col gap-6 p-6 overflow-y-auto">
      <header className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-full bg-accent/20 flex items-center justify-center">
          <span className="text-accent text-sm">◉</span>
        </div>
        <h1 className="text-xl font-semibold text-primary">The Mirror</h1>
      </header>

      <NarrativeCard narrative={state?.latest_trend_narrative} />
      <SnippetFeed snippets={state?.pending_snippets ?? []} />
      <RoutingDonut snapshot={state?.last_routing_snapshot} />
      <MirrorInput />
    </div>
  );
}
```

- [ ] **Step 2: Create RoutingDonut**

A simple SVG donut chart showing skill distribution. Use CSS variables for skill colors (hash-derived). Keep implementation minimal — a functional donut, not a charting library.

```tsx
interface RoutingDonutProps {
  snapshot: RoutingSnapshot | null | undefined;
}

export function RoutingDonut({ snapshot }: RoutingDonutProps) {
  if (!snapshot || Object.keys(snapshot.distribution).length === 0) {
    return (
      <div className="glass-panel p-4 text-muted text-sm">
        No routing data yet. Keep chatting and the Mirror will learn your patterns.
      </div>
    );
  }

  const entries = Object.entries(snapshot.distribution)
    .sort(([, a], [, b]) => b.count - a.count);

  return (
    <div className="glass-panel p-4">
      <h3 className="text-sm font-medium text-muted mb-3">How I Route Your Messages</h3>
      <div className="flex gap-4 items-center">
        {/* Simple bar chart instead of SVG donut for Phase 1 */}
        <div className="flex-1 space-y-2">
          {entries.map(([skill, stats]) => (
            <div key={skill} className="flex items-center gap-2">
              <div className="w-24 text-xs text-muted truncate">{skill}</div>
              <div className="flex-1 bg-surface-raised rounded-full h-2">
                <div
                  className="bg-accent rounded-full h-2 transition-all"
                  style={{ width: `${stats.percentage}%`, opacity: 0.5 + stats.avg_confidence * 0.5 }}
                />
              </div>
              <div className="w-12 text-xs text-muted text-right">{stats.percentage.toFixed(0)}%</div>
            </div>
          ))}
        </div>
      </div>
      <p className="text-xs text-muted mt-2">
        {snapshot.total_messages} messages · avg confidence {(snapshot.avg_routing_confidence * 100).toFixed(0)}%
      </p>
    </div>
  );
}
```

- [ ] **Step 3: Create NarrativeCard**

```tsx
import { useMutation } from "@shared/hooks/useMutation";

export function NarrativeCard({ narrative }: { narrative: TrendNarrative | null | undefined }) {
  const submitFeedback = useMutation("submit_mirror_feedback");

  if (!narrative) {
    return (
      <div className="glass-panel p-6 text-center">
        <p className="text-muted text-sm">Your first weekly reflection will appear after 7 days of use.</p>
        <p className="text-xs text-muted mt-1">The Mirror is watching and learning.</p>
      </div>
    );
  }

  return (
    <div className="glass-panel p-6">
      <div className="prose prose-sm text-primary whitespace-pre-line">
        {narrative.full_narrative}
      </div>
      <div className="flex gap-2 mt-4">
        <button
          className="text-xs text-muted hover:text-primary transition-colors"
          onClick={() => submitFeedback.mutate({
            item_id: narrative.id,
            target: "Narrative",
            feedback: "Helpful",
          })}
        >
          Helpful
        </button>
        <button
          className="text-xs text-muted hover:text-primary transition-colors"
          onClick={() => submitFeedback.mutate({
            item_id: narrative.id,
            target: "Narrative",
            feedback: "NotHelpful",
          })}
        >
          Not helpful
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create SnippetFeed**

```tsx
export function SnippetFeed({ snippets }: { snippets: NarrativeSnippet[] }) {
  if (snippets.length === 0) return null;

  return (
    <div className="space-y-2">
      {snippets.map((snippet) => (
        <div key={snippet.id} className="glass-panel p-4">
          <h4 className="text-sm font-medium text-primary">{snippet.headline}</h4>
          <p className="text-xs text-muted mt-1">{snippet.body}</p>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 5: Create MirrorInput**

```tsx
import { useState } from "react";
import { useMutation } from "@shared/hooks/useMutation";

export function MirrorInput() {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState<string | null>(null);
  const { mutate, isLoading: loading } = useMutation<{ answer: string }, { query: string }>("generate_mirror_response");

  const handleSubmit = () => {
    if (!query.trim() || loading) return;
    mutate({ query }, {
      onSuccess: (result) => setResponse(result.answer),
      onError: () => setResponse("I couldn't reflect on that right now. Try again?"),
    });
  };

  return (
    <div className="glass-panel p-4">
      <div className="flex gap-2">
        <input
          className="flex-1 bg-transparent text-sm text-primary placeholder:text-muted outline-none"
          placeholder="Ask me about how I think..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
        />
        <button
          className="text-xs text-accent hover:text-accent/80 transition-colors disabled:opacity-50"
          onClick={handleSubmit}
          disabled={loading || !query.trim()}
        >
          {loading ? "Thinking..." : "Ask"}
        </button>
      </div>
      {response && (
        <div className="mt-3 text-sm text-primary whitespace-pre-line border-t border-border pt-3">
          {response}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 6: Add Mirror to sidebar**

In `desktop-ui/src/app/layouts/Sidebar.tsx`, add a new nav item. Find the existing items array (around line 27) and add:

```tsx
{ key: "Mirror", icon: Eye, label: "Mirror" },
```

Import `Eye` from `lucide-react` (or use a suitable icon like `Sparkles`).

- [ ] **Step 7: Add Mirror route**

In the router file (check `desktop-ui/src/app/App.tsx` or `routes.tsx`), add:

```tsx
import { MirrorPage } from "@features/mirror/MirrorPage";
// In the route config:
{ path: "/mirror", element: <MirrorPage /> }
```

- [ ] **Step 8: Verify dev server**

Run: `cd desktop-ui && bun run dev`
Navigate to the Mirror page and verify it renders without errors.

- [ ] **Step 9: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/features/mirror/ desktop-ui/src/app/
git commit -m "feat(mirror): add Mirror tab UI with routing donut, narrative card, snippet feed, and conversational input"
```

---

## Task 13: Integration Test

**Files:**
- Create: `tests/integration/mirror.rs` (or add to existing integration test file)

- [ ] **Step 1: Write integration test**

```rust
#[tokio::test]
async fn test_mirror_routing_accumulation_and_facade() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Run all migrations
    run_all_migrations(&pool).await.unwrap();

    let repo = MirrorRepo::new(pool.clone());

    // Manually insert a routing snapshot (simulating subscriber flush)
    let snapshot = RoutingSnapshot {
        id: Uuid::new_v4(),
        captured_at: Utc::now(),
        window_hours: 1,
        total_messages: 100,
        distribution: HashMap::from([
            ("general".to_string(), SkillRouteStats {
                count: 60, percentage: 60.0, avg_confidence: 0.75, top_triggers: vec![]
            }),
            ("finance-management".to_string(), SkillRouteStats {
                count: 40, percentage: 40.0, avg_confidence: 0.85, top_triggers: vec!["budget".into()]
            }),
        ]),
        fallback_rate: 0.60,
        avg_routing_confidence: 0.79,
        low_confidence_count: 5,
        user_feedback: None,
    };
    repo.insert_routing_snapshot(&snapshot).await.unwrap();

    // Query via facade
    let facade = MirrorFacade::new(repo);
    let state = facade.get_state().await.unwrap();

    assert!(state.last_routing_snapshot.is_some());
    let snap = state.last_routing_snapshot.unwrap();
    assert_eq!(snap.total_messages, 100);
    assert_eq!(snap.distribution.len(), 2);
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo nextest run -E 'test(mirror_routing_accumulation)'`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test(mirror): add integration test for routing accumulation and facade query"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all tests:** `cargo nextest run --workspace`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Run frontend lint:** `cd desktop-ui && bun run lint`
- [ ] **Run frontend tests:** `cd desktop-ui && bun run test`
