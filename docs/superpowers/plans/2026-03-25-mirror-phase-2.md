# The Mirror Phase 2 — Meta-Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the MetaRuleDetector — the brain proposes rules about its own thinking, the user approves or dismisses them.

**Architecture:** A new `MetaRuleDetector` event subscriber listens for `UserCorrectedAI` and `SkillRouted` (low-confidence) events, detects correction streaks and low-confidence patterns, and proposes meta-rules via heuristic templates. Rules are stored in a new `mirror_meta_rules` table with pending/active/disabled status. The UI shows pending rules as actionable snippet cards with Approve/Dismiss buttons. A `MetaRuleProposer` trait enables LLM fallback for novel patterns.

**Tech Stack:** Rust (cognitive/agent/app-core/desktop crates), SQLite (mirror_meta_rules table), React + Tailwind v4 (desktop-ui)

**Spec:** `docs/superpowers/specs/2026-03-25-mirror-self-reflection-layer-design.md` — MetaRuleDetector section (lines 346-357) + MetaRule types (lines 204-229) + mirror_meta_rules table (lines 286-297)

**Depends on:** Phase 1 complete (all files in `crates/cognitive/src/mirror/` exist)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/mirror/subscribers/meta_rule.rs` | `MetaRuleDetector` — event subscriber detecting correction/confidence patterns, proposing meta-rules |
| *(no new migration file — `mirror_meta_rules` table appended to existing `003_mirror_tables.sql` in-place, pre-release convention)* | |
| `desktop-ui/src/features/mirror/components/MetaRulesSection.tsx` | Active + pending meta-rules display with Approve/Tweak/Dismiss |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/mirror/types.rs` | Add `MetaRule`, `MetaRuleAction`, `MetaRuleSource`, `MetaRuleStatus`, `MirrorAlert::MetaRuleProposed`, update `SuggestedAction` + `MirrorState` |
| `crates/cognitive/src/mirror/repo.rs` | Add meta-rule CRUD methods (insert, get_pending, get_active, approve, dismiss, update_effectiveness) |
| `crates/cognitive/src/mirror/narratives.rs` | Add `MetaRuleProposer` trait, handle `MirrorAlert::MetaRuleProposed` in `snippet_from_alert` |
| `crates/cognitive/src/mirror/facade.rs` | Add `approve_meta_rule`, `dismiss_meta_rule`, `get_meta_rules` methods; update `get_state` to include meta-rules |
| `crates/cognitive/src/mirror/engine.rs` | Start `MetaRuleDetector` subscriber alongside routing subscriber |
| `crates/cognitive/src/mirror/subscribers/mod.rs` | Export `MetaRuleDetector` |
| `crates/cognitive/src/mirror/mod.rs` | Re-export new types |
| `crates/cognitive/migrations/003_mirror_tables.sql` | Append `mirror_meta_rules` table (pre-release: update in-place) |
| `crates/cognitive/src/repos/mod.rs` | Bump `cognitive_mirror` migration version to force re-run |
| `crates/desktop/src/commands/mirror.rs` | Add `approve_meta_rule`, `dismiss_meta_rule`, `get_meta_rules` commands |
| `desktop-ui/src/features/mirror/MirrorPage.tsx` | Add `MetaRulesSection` component |
| `desktop-ui/src/features/mirror/components/SnippetFeed.tsx` | Add Approve/Dismiss buttons for `MetaRuleProposed` snippets |

---

## Task 1: Add Meta-Rule Types

**Files:**
- Modify: `crates/cognitive/src/mirror/types.rs`

- [ ] **Step 1: Add MetaRule and related enums**

In `crates/cognitive/src/mirror/types.rs`, add after the existing type definitions:

```rust
/// Meta-rule — stored in dedicated mirror_meta_rules table.
/// Represents a rule the brain proposes about its own thinking behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaRule {
    pub id: Uuid,
    pub trigger_condition: String,
    pub action: MetaRuleAction,
    pub source: MetaRuleSource,
    pub effectiveness_score: f64,
    pub status: MetaRuleStatus,
    pub signal_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetaRuleAction {
    AdjustRouting { skill: String, direction: String },
    ForceClarification,
    SwitchMode { mode: String },
    CreateExperiment { hypothesis: String },
    SurfaceInsight { message: String },
    Custom { payload: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetaRuleSource {
    UserCreated,
    ReflectionGenerated,
    CorrectionDerived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetaRuleStatus {
    Pending,
    Active,
    Disabled,
}
```

- [ ] **Step 2: Update `MirrorState` to include meta-rules**

```rust
pub struct MirrorState {
    pub last_routing_snapshot: Option<RoutingSnapshot>,
    pub latest_trend_narrative: Option<TrendNarrative>,
    pub pending_snippets: Vec<NarrativeSnippet>,
    pub active_meta_rules: Vec<MetaRule>,     // ADD
    pub pending_meta_rules: Vec<MetaRule>,    // ADD
}
```

- [ ] **Step 3: Add `ApproveMetaRule` and `DismissMetaRule` to `SuggestedAction`**

Update the `SuggestedAction` enum. Currently it has `BoostSkill`, `ViewDetails`, and `Unknown`. Add:

```rust
pub enum SuggestedAction {
    BoostSkill { skill: String },
    ApproveMetaRule { rule_id: Uuid },
    DismissMetaRule { rule_id: Uuid },
    ViewDetails,
    #[serde(other)]
    Unknown,
}
```

- [ ] **Step 4: Add `MetaRuleProposed` to `MirrorAlert`**

```rust
pub enum MirrorAlert {
    RoutingDrift {
        skill: String,
        delta: f64,
        suggestion: String,
    },
    MetaRuleProposed {
        rule_id: Uuid,
        rule_text: String,
        source: MetaRuleSource,
    },
}
```

- [ ] **Step 5: Fix compile breaks immediately**

Adding `MetaRuleProposed` to `MirrorAlert` will break `snippet_from_alert()` in `narratives.rs` (non-exhaustive match). Fix it NOW — don't wait for Task 4:

In `crates/cognitive/src/mirror/narratives.rs`, add the match arm to `snippet_from_alert`:

```rust
MirrorAlert::MetaRuleProposed { rule_id, rule_text, source: _ } => NarrativeSnippet {
    id: Uuid::new_v4(),
    created_at: Utc::now(),
    alert_type: MirrorAlertType::MetaRuleProposed,
    headline: "I learned something about how I think".to_string(),
    body: format!(
        "Based on recent patterns, I think I should: \"{}\". Does this sound right?",
        rule_text
    ),
    suggested_action: Some(SuggestedAction::ApproveMetaRule { rule_id: *rule_id }),
    user_feedback: None,
    dismissed_at: None,
},
```

Also temporarily fix `facade.rs` `get_state()` to compile (add the new fields with empty defaults):

```rust
active_meta_rules: vec![],
pending_meta_rules: vec![],
```

- [ ] **Step 6: Build to verify**

Run: `cargo build -p cognitive`
Expected: compiles with no errors

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/mirror/types.rs crates/cognitive/src/mirror/narratives.rs crates/cognitive/src/mirror/facade.rs
git commit -m "feat(mirror): add MetaRule types, actions, and status for Phase 2"
```

---

## Task 2: Add `mirror_meta_rules` Table

**Files:**
- Modify: `crates/cognitive/migrations/003_mirror_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs` (bump migration version)

- [ ] **Step 1: Add meta_rules table to existing migration**

Since we're pre-release, update `003_mirror_tables.sql` in-place. Append at the end:

```sql
CREATE TABLE IF NOT EXISTS mirror_meta_rules (
    id TEXT PRIMARY KEY,
    trigger_condition TEXT NOT NULL,
    action_json TEXT NOT NULL,
    source TEXT NOT NULL,
    effectiveness_score REAL NOT NULL DEFAULT 0.5,
    status TEXT NOT NULL DEFAULT 'pending',
    signal_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_meta_rules_status ON mirror_meta_rules(status);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, find the `cognitive_mirror` FeatureMigration entry and bump `version: 1` to `version: 2`.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/003_mirror_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(mirror): add mirror_meta_rules table to Phase 1 migration"
```

---

## Task 3: Meta-Rule Repo Methods

**Files:**
- Modify: `crates/cognitive/src/mirror/repo.rs`

- [ ] **Step 1: Write failing tests**

Add to the existing `#[cfg(test)] mod tests` in `repo.rs`:

```rust
#[tokio::test]
async fn test_insert_and_get_meta_rule() {
    let repo = setup().await;
    let rule = MetaRule {
        id: Uuid::new_v4(),
        trigger_condition: "When user corrects AI twice about finance routing".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();
    let pending = repo.get_meta_rules_by_status(MetaRuleStatus::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].trigger_condition, rule.trigger_condition);
    assert_eq!(pending[0].status, MetaRuleStatus::Pending);
}

#[tokio::test]
async fn test_approve_meta_rule() {
    let repo = setup().await;
    let id = Uuid::new_v4();
    let rule = MetaRule {
        id,
        trigger_condition: "Low confidence streak".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();
    repo.update_meta_rule_status(id, MetaRuleStatus::Active).await.unwrap();
    let active = repo.get_meta_rules_by_status(MetaRuleStatus::Active).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, MetaRuleStatus::Active);
    // Pending should now be empty
    let pending = repo.get_meta_rules_by_status(MetaRuleStatus::Pending).await.unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn test_dismiss_meta_rule() {
    let repo = setup().await;
    let id = Uuid::new_v4();
    let rule = MetaRule {
        id,
        trigger_condition: "test rule".to_string(),
        action: MetaRuleAction::SurfaceInsight { message: "test".to_string() },
        source: MetaRuleSource::ReflectionGenerated,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();
    repo.update_meta_rule_status(id, MetaRuleStatus::Disabled).await.unwrap();
    let disabled = repo.get_meta_rules_by_status(MetaRuleStatus::Disabled).await.unwrap();
    assert_eq!(disabled.len(), 1);
}

#[tokio::test]
async fn test_increment_signal_count() {
    let repo = setup().await;
    let id = Uuid::new_v4();
    let rule = MetaRule {
        id,
        trigger_condition: "test".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Active,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();
    repo.increment_meta_rule_signal(id).await.unwrap();
    repo.increment_meta_rule_signal(id).await.unwrap();
    let rules = repo.get_meta_rules_by_status(MetaRuleStatus::Active).await.unwrap();
    assert_eq!(rules[0].signal_count, 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(meta_rule)'`
Expected: FAIL — methods don't exist

- [ ] **Step 3: Implement meta-rule repo methods**

Add to `MirrorRepo` impl block:

```rust
pub async fn insert_meta_rule(&self, rule: &MetaRule) -> Result<()> {
    let id = rule.id.to_string();
    let action_json = serde_json::to_string(&rule.action)?;
    // Trim JSON quotes for enum variants: serde produces "\"Pending\"", we store "Pending"
    // This matches Phase 1's pattern for alert_type in snippets
    let source = serde_json::to_string(&rule.source)?.trim_matches('"').to_string();
    let status = serde_json::to_string(&rule.status)?.trim_matches('"').to_string();
    let created_at = rule.created_at.to_rfc3339();
    let updated_at = rule.updated_at.to_rfc3339();

    sqlx::query(
        "INSERT INTO mirror_meta_rules (id, trigger_condition, action_json, source, effectiveness_score, status, signal_count, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
    )
    .bind(&id)
    .bind(&rule.trigger_condition)
    .bind(&action_json)
    .bind(&source)
    .bind(rule.effectiveness_score)
    .bind(&status)
    .bind(rule.signal_count as i32)
    .bind(&created_at)
    .bind(&updated_at)
    .execute(self.pool.inner())
    .await?;
    Ok(())
}

pub async fn get_meta_rules_by_status(&self, status: MetaRuleStatus) -> Result<Vec<MetaRule>> {
    let status_str = serde_json::to_string(&status)?.trim_matches('"').to_string();
    let rows = sqlx::query_as::<_, MetaRuleRow>(
        "SELECT * FROM mirror_meta_rules WHERE status = ?1 ORDER BY created_at DESC"
    )
    .bind(&status_str)
    .fetch_all(self.pool.inner())
    .await?;
    rows.into_iter().map(|r| r.try_into()).collect()
}

pub async fn update_meta_rule_status(&self, id: Uuid, status: MetaRuleStatus) -> Result<()> {
    let id_str = id.to_string();
    let status_str = serde_json::to_string(&status)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE mirror_meta_rules SET status = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(&status_str)
        .bind(&now)
        .bind(&id_str)
        .execute(self.pool.inner())
        .await?;
    Ok(())
}

pub async fn increment_meta_rule_signal(&self, id: Uuid) -> Result<()> {
    let id_str = id.to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE mirror_meta_rules SET signal_count = signal_count + 1, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(&id_str)
        .execute(self.pool.inner())
        .await?;
    Ok(())
}

pub async fn decay_meta_rule_effectiveness(&self, id: Uuid, decay_factor: f64) -> Result<()> {
    let id_str = id.to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE mirror_meta_rules SET effectiveness_score = effectiveness_score * ?1, updated_at = ?2 WHERE id = ?3")
        .bind(decay_factor)
        .bind(&now)
        .bind(&id_str)
        .execute(self.pool.inner())
        .await?;
    Ok(())
}
```

Also add a `MetaRuleRow` struct with `#[derive(sqlx::FromRow)]` and a `TryFrom<MetaRuleRow> for MetaRule` impl. Follow the exact same pattern used for `RoutingSnapshotRow` and `SnippetRow` already in this file — parse `source` and `status` by stripping quotes and using `serde_json::from_str`, parse `action_json` directly.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(meta_rule)'`
Expected: all 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/repo.rs
git commit -m "feat(mirror): add MetaRule CRUD methods to MirrorRepo with tests"
```

---

## Task 4: MetaRuleProposer Trait + Snippet Test

**Files:**
- Modify: `crates/cognitive/src/mirror/narratives.rs`

NOTE: The `MetaRuleProposed` snippet arm was already added to `snippet_from_alert` in Task 1 (Step 5) to prevent a compile break. This task adds the `MetaRuleProposer` trait and a test for the snippet template.

- [ ] **Step 1: Add `MetaRuleProposer` trait**

In `narratives.rs`, add:

```rust
/// Trait for LLM-powered meta-rule proposal for novel patterns.
/// Defined in cognitive (L3-L4), implemented in agent (L5).
#[async_trait]
pub trait MetaRuleProposer: Send + Sync {
    async fn propose_meta_rule(&self, context: MetaRuleProposalContext) -> Result<Option<MetaRule>>;
}

#[derive(Debug, Clone)]
pub struct MetaRuleProposalContext {
    pub correction_history: Vec<String>,
    pub affected_skill: Option<String>,
    pub pattern_description: String,
}
```

- [ ] **Step 2: Write and run test for MetaRuleProposed snippet**

```rust
#[test]
fn test_snippet_from_meta_rule_proposed() {
    let rule_id = Uuid::new_v4();
    let alert = MirrorAlert::MetaRuleProposed {
        rule_id,
        rule_text: "When user corrects me twice about finance, ask for clarification".to_string(),
        source: MetaRuleSource::CorrectionDerived,
    };
    let snippet = snippet_from_alert(&alert);
    assert_eq!(snippet.alert_type, MirrorAlertType::MetaRuleProposed);
    assert!(snippet.headline.contains("learned something"));
    assert!(snippet.body.contains("finance"));
    match &snippet.suggested_action {
        Some(SuggestedAction::ApproveMetaRule { rule_id: id }) => assert_eq!(*id, rule_id),
        other => panic!("Expected ApproveMetaRule, got {:?}", other),
    }
}
```

Run: `cargo nextest run -p cognitive -E 'test(snippet_from)'`
Expected: all snippet tests PASS (the arm was added in Task 1)

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/mirror/narratives.rs
git commit -m "feat(mirror): add MetaRuleProposer trait and MetaRuleProposed snippet test"
```

---

## Task 5: Update MirrorFacade for Meta-Rules

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn test_approve_meta_rule() {
    let facade = setup().await;
    let id = Uuid::new_v4();
    let rule = MetaRule {
        id,
        trigger_condition: "test rule".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    facade.repo.insert_meta_rule(&rule).await.unwrap();
    facade.approve_meta_rule(id).await.unwrap();
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.active_meta_rules.len(), 1);
    assert!(state.pending_meta_rules.is_empty());
}

#[tokio::test]
async fn test_dismiss_meta_rule() {
    let facade = setup().await;
    let id = Uuid::new_v4();
    let rule = MetaRule {
        id,
        trigger_condition: "test".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    facade.repo.insert_meta_rule(&rule).await.unwrap();
    facade.dismiss_meta_rule(id).await.unwrap();
    let state = facade.get_state().await.unwrap();
    assert!(state.pending_meta_rules.is_empty());
    assert!(state.active_meta_rules.is_empty());
}

#[tokio::test]
async fn test_get_state_includes_meta_rules() {
    let facade = setup().await;
    let rule = MetaRule {
        id: Uuid::new_v4(),
        trigger_condition: "pending rule".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    facade.repo.insert_meta_rule(&rule).await.unwrap();
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.pending_meta_rules.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(approve_meta\|dismiss_meta\|includes_meta)'`
Expected: FAIL

- [ ] **Step 3: Implement facade methods**

Add to `MirrorFacade`:

```rust
pub async fn approve_meta_rule(&self, rule_id: Uuid) -> Result<()> {
    self.repo.update_meta_rule_status(rule_id, MetaRuleStatus::Active).await
}

pub async fn dismiss_meta_rule(&self, rule_id: Uuid) -> Result<()> {
    self.repo.update_meta_rule_status(rule_id, MetaRuleStatus::Disabled).await
}

pub async fn get_meta_rules(&self) -> Result<(Vec<MetaRule>, Vec<MetaRule>)> {
    let active = self.repo.get_meta_rules_by_status(MetaRuleStatus::Active).await?;
    let pending = self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending).await?;
    Ok((active, pending))
}
```

Update `get_state()` to include meta-rules:

```rust
pub async fn get_state(&self) -> Result<MirrorState> {
    let last_routing_snapshot = self.repo.get_latest_routing_snapshot().await?;
    let latest_trend_narrative = self.repo.get_latest_narrative().await?;
    let pending_snippets = self.repo.get_pending_snippets().await?;
    let active_meta_rules = self.repo.get_meta_rules_by_status(MetaRuleStatus::Active).await?;
    let pending_meta_rules = self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending).await?;

    Ok(MirrorState {
        last_routing_snapshot,
        latest_trend_narrative,
        pending_snippets,
        active_meta_rules,
        pending_meta_rules,
    })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/facade.rs
git commit -m "feat(mirror): add approve/dismiss/get meta-rule methods to MirrorFacade"
```

---

## Task 6: MetaRuleDetector Subscriber

**Files:**
- Create: `crates/cognitive/src/mirror/subscribers/meta_rule.rs`
- Modify: `crates/cognitive/src/mirror/subscribers/mod.rs`

This is the core of Phase 2 — the event subscriber that detects patterns and proposes meta-rules.

**PREREQUISITE — Enrich `UserCorrectedAI` event:**

Before implementing the detector, add `session_key: String` and `active_skill: Option<String>` fields to `DomainEvent::UserCorrectedAI` in `crates/bus/src/domain_events.rs`. Then update the emission site in `crates/agent/src/agent_loop/mod.rs` (where `UserCorrectedAI` is published) to include the current session key and the active skill profile name. Fix all match exhaustiveness across the workspace. Commit: `feat(mirror): enrich UserCorrectedAI with session_key and active_skill`.

This is essential — without these fields, the MetaRuleDetector cannot distinguish corrections per-session or per-skill, making the streak detection meaningless.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correction_streak_same_session() {
        let mut detector = MetaRuleDetector::new_for_test();
        // Two corrections in same session triggers proposal
        detector.record_correction("session-1", "finance-management");
        let result = detector.record_correction("session-1", "finance-management");
        assert!(result.is_some());
        let alert = result.unwrap();
        match alert {
            MirrorAlert::MetaRuleProposed { rule_text, .. } => {
                assert!(rule_text.contains("finance-management"));
            }
            _ => panic!("Expected MetaRuleProposed"),
        }
    }

    #[test]
    fn test_correction_single_no_trigger() {
        let mut detector = MetaRuleDetector::new_for_test();
        // Single correction doesn't trigger
        let result = detector.record_correction("session-1", "general");
        assert!(result.is_none());
    }

    #[test]
    fn test_low_confidence_streak() {
        let mut detector = MetaRuleDetector::new_for_test();
        // Three consecutive low-confidence routes triggers proposal
        detector.record_low_confidence("skill-1", 0.35);
        detector.record_low_confidence("skill-2", 0.30);
        let result = detector.record_low_confidence("skill-3", 0.38);
        assert!(result.is_some());
        match result.unwrap() {
            MirrorAlert::MetaRuleProposed { rule_text, .. } => {
                assert!(rule_text.contains("clarif"));
            }
            _ => panic!("Expected MetaRuleProposed"),
        }
    }

    #[test]
    fn test_low_confidence_resets_on_high() {
        let mut detector = MetaRuleDetector::new_for_test();
        detector.record_low_confidence("s1", 0.35);
        detector.record_low_confidence("s2", 0.30);
        // High confidence resets streak
        detector.record_high_confidence();
        let result = detector.record_low_confidence("s3", 0.38);
        assert!(result.is_none()); // streak was reset
    }

    #[test]
    fn test_cross_session_correction_streak() {
        let mut detector = MetaRuleDetector::new_for_test();
        // 3 corrections across different sessions for same skill
        detector.record_correction("session-1", "task-management");
        detector.record_correction("session-2", "task-management");
        let result = detector.record_correction("session-3", "task-management");
        assert!(result.is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(correction_streak\|low_confidence_streak\|cross_session)'`
Expected: FAIL

- [ ] **Step 3: Implement MetaRuleDetector**

```rust
use bus::DomainEvent;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::repo::MirrorRepo;
use super::super::types::*;
use super::super::narratives::snippet_from_alert;

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.4;
const SAME_SESSION_CORRECTION_THRESHOLD: u32 = 2;
const CROSS_SESSION_CORRECTION_THRESHOLD: u32 = 3;
const LOW_CONFIDENCE_STREAK_THRESHOLD: u32 = 3;

pub struct MetaRuleDetector {
    /// Per-session correction counts: session_key → count
    session_corrections: HashMap<String, u32>,
    /// Per-skill cross-session correction counts: skill_name → count
    skill_corrections: HashMap<String, u32>,
    /// Current low-confidence streak count
    low_confidence_streak: u32,
    /// Repo for persistence (None in tests)
    repo: Option<MirrorRepo>,
}

impl MetaRuleDetector {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            session_corrections: HashMap::new(),
            skill_corrections: HashMap::new(),
            low_confidence_streak: 0,
            repo: Some(repo),
        }
    }

    #[cfg(test)]
    fn new_for_test() -> Self {
        Self {
            session_corrections: HashMap::new(),
            skill_corrections: HashMap::new(),
            low_confidence_streak: 0,
            repo: None,
        }
    }

    /// Record a user correction. Returns a MirrorAlert if a pattern threshold is crossed.
    pub fn record_correction(&mut self, session_key: &str, skill_name: &str) -> Option<MirrorAlert> {
        // Track per-session
        let session_count = self.session_corrections
            .entry(session_key.to_string())
            .or_insert(0);
        *session_count += 1;

        // Track per-skill across sessions
        let skill_count = self.skill_corrections
            .entry(skill_name.to_string())
            .or_insert(0);
        *skill_count += 1;

        // Check same-session threshold
        if *session_count >= SAME_SESSION_CORRECTION_THRESHOLD {
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: format!(
                    "When user corrects me {} times in one session about {}, ask for clarification before acting",
                    SAME_SESSION_CORRECTION_THRESHOLD, skill_name
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        // Check cross-session threshold
        if *skill_count >= CROSS_SESSION_CORRECTION_THRESHOLD {
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: format!(
                    "User frequently corrects me on {} routing — consider strengthening {} skill detection",
                    skill_name, skill_name
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        None
    }

    /// Record a low-confidence routing event. Returns alert if streak threshold crossed.
    pub fn record_low_confidence(&mut self, _skill: &str, _confidence: f64) -> Option<MirrorAlert> {
        self.low_confidence_streak += 1;
        if self.low_confidence_streak >= LOW_CONFIDENCE_STREAK_THRESHOLD {
            self.low_confidence_streak = 0; // Reset after proposing
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: "When routing confidence is low for multiple messages in a row, switch to clarification mode before acting".to_string(),
                source: MetaRuleSource::CorrectionDerived,
            });
        }
        None
    }

    /// Reset low-confidence streak on high-confidence routing
    pub fn record_high_confidence(&mut self) {
        self.low_confidence_streak = 0;
    }

    /// Process a MirrorAlert and persist the proposed meta-rule + snippet
    async fn handle_alert(&self, alert: &MirrorAlert) {
        if let Some(repo) = &self.repo {
            // Create the meta-rule in pending state
            if let MirrorAlert::MetaRuleProposed { rule_id, rule_text, source } = alert {
                let meta_rule = MetaRule {
                    id: *rule_id,
                    trigger_condition: rule_text.clone(),
                    action: MetaRuleAction::ForceClarification, // Default action
                    source: source.clone(),
                    effectiveness_score: 0.5,
                    status: MetaRuleStatus::Pending,
                    signal_count: 0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                let _ = repo.insert_meta_rule(&meta_rule).await;

                // Also create a snippet for the UI feed
                let snippet = snippet_from_alert(alert);
                let _ = repo.insert_snippet(&snippet).await;
            }
        }
    }

    pub async fn run(
        mut self,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                event = rx.recv() => {
                    match event {
                        Ok(DomainEvent::UserCorrectedAI { session_key, active_skill, .. }) => {
                            // UserCorrectedAI now carries session_key and active_skill
                            // (enriched as part of this Phase 2 — see Task 6b below)
                            let skill = active_skill.as_deref().unwrap_or("unknown");
                            if let Some(alert) = self.record_correction(&session_key, skill) {
                                self.handle_alert(&alert).await;
                            }
                        }
                        Ok(DomainEvent::SkillRouted { confidence, skill_name, .. }) => {
                            if confidence < LOW_CONFIDENCE_THRESHOLD {
                                if let Some(alert) = self.record_low_confidence(&skill_name, confidence) {
                                    self.handle_alert(&alert).await;
                                }
                            } else {
                                self.record_high_confidence();
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("MetaRuleDetector lagged {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Export from subscribers/mod.rs**

Add to `crates/cognitive/src/mirror/subscribers/mod.rs`:

```rust
pub mod meta_rule;
pub use meta_rule::MetaRuleDetector;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(correction_streak\|low_confidence\|cross_session)'`
Expected: all 5 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/subscribers/
git commit -m "feat(mirror): add MetaRuleDetector subscriber with correction streak and low-confidence detection"
```

---

## Task 7: Wire MetaRuleDetector into MirrorEngine

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`

- [ ] **Step 1: Update MirrorEngine::start to spawn MetaRuleDetector**

In `engine.rs`, import `MetaRuleDetector` and update `MirrorEngine::start`:

1. Clone `repo` before it's moved into `MirrorFacade::new(repo)`:
```rust
let meta_rule_repo = repo.clone();
// ... existing routing_sub and facade creation ...
let meta_rule_detector = MetaRuleDetector::new(meta_rule_repo);
```

2. Add the detector to the spawned handles:
```rust
let handles = vec![
    tokio::spawn(routing_sub.run(bus.subscribe(), shutdown.clone())),
    tokio::spawn(meta_rule_detector.run(bus.subscribe(), shutdown.clone())),
];
```

3. **Update the existing subscriber count test** (`test_engine_subscriber_count`). Change the assertion from `assert_eq!(bus.subscriber_count(), 1)` to `assert_eq!(bus.subscriber_count(), 2)` — there are now two subscribers (routing + meta-rule).

- [ ] **Step 2: Build and verify**

Run: `cargo build --workspace`
Expected: compiles

- [ ] **Step 3: Run all mirror tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all tests PASS (including the updated subscriber count assertion)

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/mirror/engine.rs
git commit -m "feat(mirror): wire MetaRuleDetector into MirrorEngine startup"
```

---

## Task 8: Tauri Commands for Meta-Rules

**Files:**
- Modify: `crates/desktop/src/commands/mirror.rs`

- [ ] **Step 1: Add three new Tauri commands**

Follow the existing pattern in `mirror.rs`:

```rust
#[tauri::command]
pub async fn approve_meta_rule(
    state: State<'_, Arc<AppCore>>,
    rule_id: Uuid,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.approve_meta_rule(rule_id).await?)
}

#[tauri::command]
pub async fn dismiss_meta_rule(
    state: State<'_, Arc<AppCore>>,
    rule_id: Uuid,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.dismiss_meta_rule(rule_id).await?)
}

// NOTE: get_meta_rules is NOT needed as a separate command — MirrorState
// already includes activeMetaRules and pendingMetaRules via get_mirror_state.
// Skip this command to avoid tuple serialization issues and dead code.
```

- [ ] **Step 2: Update DEV_COMMANDS**

Add the three new command names to the `DEV_COMMANDS` array.

- [ ] **Step 3: Register commands in main.rs invoke_handler**

Add the three new functions to the Tauri builder's `invoke_handler` macro call.

- [ ] **Step 4: Add dev server dispatch**

In the `dispatch_dev` function (or `dev_server/dispatch.rs`), add branches for the three new commands following the existing pattern.

- [ ] **Step 5: Update dev_server coverage test**

Add the new commands to the dev server test list in `dev_server/mod.rs`.

- [ ] **Step 6: Build and verify**

Run: `cargo build -p desktop`
Expected: compiles, DEV_COMMANDS parity test passes

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/mirror.rs crates/desktop/src/main.rs crates/desktop/src/dev_server/
git commit -m "feat(mirror): add approve/dismiss/get meta-rules Tauri commands"
```

---

## Task 9: Frontend — MetaRulesSection + SnippetFeed Actions

**Files:**
- Create: `desktop-ui/src/features/mirror/components/MetaRulesSection.tsx`
- Modify: `desktop-ui/src/features/mirror/components/SnippetFeed.tsx`
- Modify: `desktop-ui/src/features/mirror/MirrorPage.tsx`

- [ ] **Step 1: Create MetaRulesSection component**

```tsx
import { useMutation } from "@shared/hooks/useMutation";
import { Check, X, Sparkles } from "lucide-react";

interface MetaRule {
  id: string;
  triggerCondition: string;
  action: Record<string, unknown>;
  source: string;
  effectivenessScore: number;
  status: string;
  signalCount: number;
}

interface MetaRulesSectionProps {
  activeRules: MetaRule[];
  pendingRules: MetaRule[];
}

export function MetaRulesSection({ activeRules, pendingRules }: MetaRulesSectionProps) {
  const { mutate: approve } = useMutation<void, { ruleId: string }>("approve_meta_rule");
  const { mutate: dismiss } = useMutation<void, { ruleId: string }>("dismiss_meta_rule");

  if (activeRules.length === 0 && pendingRules.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
        <Sparkles className="size-3.5" />
        Rules About How I Think
      </h2>

      {pendingRules.map((rule) => (
        <div key={rule.id} className="glass-card rounded-xl p-4 border border-accent/20">
          <p className="text-[12px] text-foreground mb-1">
            I think I should: &ldquo;{rule.triggerCondition}&rdquo;
          </p>
          <p className="text-[11px] text-muted-foreground mb-3">Sound good?</p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => approve({ ruleId: rule.id })}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-success bg-success/10 hover:bg-success/20 transition-colors"
            >
              <Check className="size-3" />
              Approve
            </button>
            <button
              type="button"
              onClick={() => dismiss({ ruleId: rule.id })}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors"
            >
              <X className="size-3" />
              Dismiss
            </button>
          </div>
        </div>
      ))}

      {activeRules.map((rule) => (
        <div key={rule.id} className="glass-card rounded-xl p-4 opacity-80">
          <div className="flex items-center justify-between">
            <p className="text-[11px] text-foreground">{rule.triggerCondition}</p>
            <span className="text-2xs text-success px-1.5 py-0.5 rounded bg-success/10">active</span>
          </div>
          {rule.signalCount > 0 && (
            <p className="text-2xs text-dim mt-1">Triggered {rule.signalCount} times</p>
          )}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Update MirrorPage to include MetaRulesSection**

In `MirrorPage.tsx`, add the import and render `MetaRulesSection` between SnippetFeed and RoutingDonut:

```tsx
import { MetaRulesSection } from "./components/MetaRulesSection";

// In the MirrorState interface, add:
// activeMetaRules: MetaRule[];
// pendingMetaRules: MetaRule[];

// In the render, between SnippetFeed and RoutingDonut:
<MetaRulesSection
  activeRules={mirrorState?.activeMetaRules ?? []}
  pendingRules={mirrorState?.pendingMetaRules ?? []}
/>
```

Also update the `DEFAULT_MIRROR_STATE` to include `activeMetaRules: []` and `pendingMetaRules: []`.

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/mirror/
git commit -m "feat(mirror): add MetaRulesSection UI with approve/dismiss actions"
```

---

## Task 10: Integration Test

**Files:**
- Modify: `tests/integration/mirror.rs`

- [ ] **Step 1: Add meta-rule lifecycle integration test**

```rust
#[tokio::test]
async fn test_mirror_meta_rule_lifecycle() {
    let pool = test_pool().await;
    StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations()).await.unwrap();

    let repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let facade = cognitive::mirror::MirrorFacade::new(repo.clone());

    // 1. Insert a pending meta-rule
    let rule_id = uuid::Uuid::new_v4();
    let rule = cognitive::mirror::MetaRule {
        id: rule_id,
        trigger_condition: "When corrected twice on finance, clarify first".to_string(),
        action: cognitive::mirror::MetaRuleAction::ForceClarification,
        source: cognitive::mirror::MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: cognitive::mirror::MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();

    // 2. Verify it appears in state as pending
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.pending_meta_rules.len(), 1);
    assert!(state.active_meta_rules.is_empty());

    // 3. Approve it
    facade.approve_meta_rule(rule_id).await.unwrap();

    // 4. Verify it moved to active
    let state = facade.get_state().await.unwrap();
    assert!(state.pending_meta_rules.is_empty());
    assert_eq!(state.active_meta_rules.len(), 1);
    assert_eq!(state.active_meta_rules[0].status, cognitive::mirror::MetaRuleStatus::Active);

    // 5. Signal count tracking
    repo.increment_meta_rule_signal(rule_id).await.unwrap();
    let (active, _) = facade.get_meta_rules().await.unwrap();
    assert_eq!(active[0].signal_count, 1);
}
```

- [ ] **Step 2: Run test**

Run: `cargo nextest run -E 'test(meta_rule_lifecycle)'`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/integration/mirror.rs
git commit -m "test(mirror): add meta-rule lifecycle integration test for Phase 2"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all mirror tests:** `cargo nextest run -p cognitive -E 'test(mirror)'`
- [ ] **Run integration tests:** `cargo nextest run -E 'test(mirror)'`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Run frontend lint:** `cd desktop-ui && bun run lint`
- [ ] **Run frontend build:** `cd desktop-ui && bun run build`
