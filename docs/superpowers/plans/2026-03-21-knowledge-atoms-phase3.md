# Knowledge Atoms Phase 3: "The Mentor" Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the knowledge system proactive — 5 coaching pattern detectors that celebrate streaks and nudge on decay, a morning briefing Knowledge Health section, focus session micro-review injection, autotuner retention feedback, and cross-feature atom surfacing.

**Architecture:** Extend the existing coaching pipeline (SignalAccumulator → PatternDetector → CoachingService) with 5 learning-specific patterns that read from `review_log` and `knowledge_atoms`. A new `LearningSignalConverter` maps learning DomainEvents into coaching signals/triggers. Morning briefing is a new daily cron that compiles a Knowledge Health digest and delivers via coaching intervention. Focus session micro-review hooks into `FocusSessionStarted` to offer a 45-second atom review before deep work. Autotuner gains a `knowledge_retention_score` long-term metric.

**Tech Stack:** Rust (SQLite via sqlx, tokio broadcast, chrono), React (TypeScript, Tailwind CSS), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-03-21-unified-learning-system-design.md` (Section 4: Coaching Integration + Decay System, Phase 3 deliverables)

**Depends on:** Phase 1 + Phase 2 complete (knowledge_atoms, extraction, decay, Knowledge Health page, domain events)

---

## Already covered by Phase 1/2

These Phase 3 spec items are already implemented:
- **DomainEvents for learning** — all 11 variants emitted (Phase 1)
- **ActivityLogSubscriber handles learning events** — normalizers exist (Phase 1)
- **ContextInferenceEngine: "learning" work context** — added in Phase 2
- **Decay cron + auto-archive** — daily 3 AM cron runs FSRS retention + tiered salience (Phase 2)
- **CoachingLearningDigest event** — emitted by decay cron (Phase 2)
- **Knowledge Health page** — topic heatmap with retention bars (Phase 2)
- **Atom restore (undo archive)** — `atom_restore` handler + IPC already exists (Phase 1)

## File Map

### New files
| File | Responsibility |
|---|---|
| `crates/feature-coaching/src/learning_patterns.rs` | 5 learning pattern detectors: streak, decay, momentum, domain gap, knowledge transfer |
| `crates/feature-coaching/src/learning_signals.rs` | Convert learning DomainEvents into coaching signals + trigger conditions |
| `crates/feature-coaching/src/learning_templates.rs` | Message templates for learning coaching (70% celebration tone) |
| `crates/cognitive/src/repos/review_stats.rs` | Review log queries: streak calculation, daily review counts, per-domain stats |
| `crates/app-core/src/handlers/morning_briefing.rs` | AppCore handler that compiles Knowledge Health digest data |
| `crates/desktop-shared/src/commands/morning_briefing.rs` | IPC types for morning briefing |
| `crates/desktop/src/commands/morning_briefing.rs` | Tauri command + DEV_COMMANDS |
| `desktop-ui/src/features/coaching/components/MorningBriefing.tsx` | Morning briefing Knowledge Health section |
| `desktop-ui/src/features/coaching/components/MicroReviewPrompt.tsx` | Focus session pre-review prompt (45s) |
| `desktop-ui/src/features/learn/components/FocusedReview.tsx` | Focused review session page (topic-scoped) |

### Modified files
| File | Changes |
|---|---|
| `crates/feature-coaching/src/signal_accumulator/conversion.rs` | Map learning events to coaching signals |
| `crates/feature-coaching/src/signal_accumulator/types.rs` | Add learning trigger conditions (streak_review, retention_drop, etc.) |
| `crates/feature-coaching/src/pattern_detector.rs` | Add 5 learning pattern detection blocks |
| `crates/feature-coaching/src/service.rs` | Wire learning signals, inject micro-review on FocusSessionStarted |
| `crates/cognitive/src/repos/mod.rs` | Export review_stats module |
| `crates/cognitive/src/lib.rs` | Re-export ReviewStatsRepo |
| `crates/app-core/src/init/cron.rs` | Register morning briefing daily cron (9 AM local) |
| `crates/app-core/src/handlers/mod.rs` | Add morning_briefing module |
| `crates/app-core/src/state.rs` | Add review_stats_repo accessor |
| `crates/autotuner/src/traits.rs` | Add `knowledge_retention_score: f64` to MetricSnapshot |
| `crates/agent/src/autotuner/metric_collector.rs` | Compute knowledge_retention_score from review_log + atoms |
| `crates/desktop-shared/src/commands/mod.rs` | Add morning_briefing types module |
| `crates/desktop/src/commands/mod.rs` | Add morning_briefing command module |
| `crates/desktop/src/main.rs` | Register morning_briefing commands |
| `crates/desktop/src/dev_server/dispatch.rs` | Add morning_briefing dispatch |
| `crates/desktop/src/dev_server/mod.rs` | Add morning_briefing DEV_COMMANDS |
| `desktop-ui/src/features/coaching/pages/OverviewPage.tsx` | Add Morning Briefing section with Knowledge Health |
| `desktop-ui/src/app/router.tsx` | Add focused-review route |
| `desktop-ui/src/features/learn/components/DashboardHome.tsx` | Add "Start focused review" action |

---

### Task 1: ReviewStatsRepo — streak + daily review queries

**Files:**
- Create: `crates/cognitive/src/repos/review_stats.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Create the review stats repo**

```rust
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReviewStatsRepo {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct DailyReviewStat {
    pub date: String,
    pub review_count: i64,
    pub avg_rating: f64,
}

#[derive(Debug, Clone)]
pub struct DomainRetentionStat {
    pub domain: String,
    pub atom_count: i64,
    pub avg_retention: f64,
    pub reviews_last_7d: i64,
}

impl ReviewStatsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Calculate current study streak (consecutive days with at least 1 review).
    pub async fn current_streak(&self) -> Result<usize, sqlx::Error> {
        // Query distinct dates from review_log, ordered DESC
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT DATE(reviewed_at) as d FROM review_log ORDER BY d DESC LIMIT 60",
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let today = chrono::Local::now().date_naive();
        let mut streak = 0usize;
        let mut expected = today;

        for (date_str,) in &rows {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if date == expected {
                    streak += 1;
                    expected = expected.pred_opt().unwrap_or(expected);
                } else if date == expected.pred_opt().unwrap_or(expected) {
                    // Allow gap of exactly 1 day (yesterday counts if today hasn't reviewed yet)
                    if streak == 0 {
                        streak = 1;
                        expected = date.pred_opt().unwrap_or(date);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        Ok(streak)
    }

    /// Daily review counts for the last N days.
    pub async fn daily_reviews(&self, days: i64) -> Result<Vec<DailyReviewStat>, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        sqlx::query_as::<_, (String, i64, f64)>(
            r#"
            SELECT DATE(reviewed_at) as d, COUNT(*) as cnt, AVG(rating) as avg_r
            FROM review_log
            WHERE reviewed_at > ?1
            GROUP BY d
            ORDER BY d DESC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(date, count, avg)| DailyReviewStat {
                    date,
                    review_count: count,
                    avg_rating: avg,
                })
                .collect()
        })
    }

    /// Per-domain retention stats (from knowledge_atoms + review_log).
    pub async fn domain_retention_stats(&self) -> Result<Vec<DomainRetentionStat>, sqlx::Error> {
        let cutoff_7d = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        sqlx::query_as::<_, (String, i64, f64, i64)>(
            r#"
            SELECT
                ka.domain,
                COUNT(DISTINCT ka.id) as atom_count,
                AVG(ka.retention_pct) as avg_ret,
                (SELECT COUNT(*) FROM review_log rl
                 JOIN flashcards fc ON fc.id = rl.card_id
                 WHERE fc.atom_id IN (SELECT id FROM knowledge_atoms WHERE domain = ka.domain AND status = 'active')
                   AND rl.reviewed_at > ?1) as reviews_7d
            FROM knowledge_atoms ka
            WHERE ka.status = 'active'
            GROUP BY ka.domain
            "#,
        )
        .bind(&cutoff_7d)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(domain, count, avg, reviews)| DomainRetentionStat {
                    domain,
                    atom_count: count,
                    avg_retention: avg,
                    reviews_last_7d: reviews,
                })
                .collect()
        })
    }

    /// Weighted knowledge retention score for autotuner (importance-weighted avg retention).
    pub async fn knowledge_retention_score(&self) -> Result<f64, sqlx::Error> {
        let row: Option<(f64,)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(
                    SUM(retention_pct * personal_importance) / NULLIF(SUM(personal_importance), 0),
                    1.0
                )
            FROM knowledge_atoms
            WHERE status = 'active'
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(s,)| s).unwrap_or(1.0))
    }
}
```

- [ ] **Step 2: Export from repos/mod.rs and lib.rs**

Add `pub mod review_stats;` and `pub use review_stats::ReviewStatsRepo;` to repos/mod.rs. Add `pub use repos::ReviewStatsRepo;` to lib.rs.

- [ ] **Step 3: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_current_streak_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);
        assert_eq!(repo.current_streak().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_knowledge_retention_score_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);
        let score = repo.knowledge_retention_score().await.unwrap();
        assert!((score - 1.0).abs() < 0.01); // default when no atoms
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(review_stats)'`
Expected: all pass

- [ ] **Step 5: Commit**

```
feat(cognitive): add ReviewStatsRepo — streak, daily reviews, domain retention
```

---

### Task 2: Learning signal conversion — map DomainEvents to coaching signals

**Files:**
- Create: `crates/feature-coaching/src/learning_signals.rs`
- Modify: `crates/feature-coaching/src/signal_accumulator/conversion.rs`
- Modify: `crates/feature-coaching/src/signal_accumulator/types.rs`

- [ ] **Step 1: Add learning trigger conditions**

In `crates/feature-coaching/src/signal_accumulator/types.rs`, add learning trigger conditions to the `default_conditions()` function. Follow the existing pattern (each condition has a `name`, `threshold`, `cooldown_secs`):

```rust
// Learning conditions (TriggerCondition::new takes name + cooldown_secs only)
TriggerCondition::new("flashcard_reviewed", 0),
TriggerCondition::new("retention_drop_important", 3600),
TriggerCondition::new("learning_streak_milestone", 86400),
TriggerCondition::new("learning_momentum_shift", 3600),
TriggerCondition::new("domain_retention_decline", 86400),
TriggerCondition::new("knowledge_transfer", 3600),
TriggerCondition::new("atom_created", 0),
TriggerCondition::new("coaching_learning_digest", 86400),
```

- [ ] **Step 2: Map learning events to signals**

In `crates/feature-coaching/src/signal_accumulator/conversion.rs`, add match arms for learning events in `event_to_signal()`:

```rust
// SignalMetadata fields: app: Option<String>, task_id: Option<String>,
//                        category: Option<String>, amount: Option<f64>
DomainEvent::AtomFlashcardReviewed { quality, .. } => (
    "AtomFlashcardReviewed",
    SignalMetadata { amount: Some(*quality as f64), ..Default::default() },
),
DomainEvent::KnowledgeAtomCreated { domain, .. } => (
    "KnowledgeAtomCreated",
    SignalMetadata { category: Some(domain.clone()), ..Default::default() },
),
DomainEvent::KnowledgeAtomArchived { reason, .. } => (
    "KnowledgeAtomArchived",
    SignalMetadata { category: Some(reason.clone()), ..Default::default() },
),
DomainEvent::RetentionMilestoneReached { new_retention_pct, .. } => (
    "RetentionMilestoneReached",
    SignalMetadata { amount: Some(*new_retention_pct), ..Default::default() },
),
DomainEvent::KnowledgeTransferDetected { from_domain, to_domain, confidence, .. } => (
    "KnowledgeTransferDetected",
    SignalMetadata { category: Some(format!("{from_domain}->{to_domain}")), amount: Some(*confidence), ..Default::default() },
),
DomainEvent::CoachingLearningDigest { fading_count, .. } => (
    "CoachingLearningDigest",
    SignalMetadata { amount: Some(*fading_count as f64), ..Default::default() },
),
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p feature-coaching 2>&1 | tail -10`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(feature-coaching): map learning DomainEvents into coaching signals
```

---

### Task 3: Learning pattern detectors — streak, decay, momentum, domain gap, transfer

**Files:**
- Create: `crates/feature-coaching/src/learning_patterns.rs`
- Modify: `crates/feature-coaching/src/pattern_detector.rs`

- [ ] **Step 1: Create learning patterns module**

Create `crates/feature-coaching/src/learning_patterns.rs` with 5 pattern detection functions. Each takes the history map and returns `Option<DetectedPattern>`:

```rust
use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use crate::pattern_detector::DetectedPattern;

/// StudyStreakPattern: consecutive days with AtomFlashcardReviewed.
/// Celebrate at 3/7/14/30 days; nudge within 24h on break.
pub fn detect_study_streak(
    reviews: &VecDeque<(DateTime<Utc>, String)>,
) -> Option<DetectedPattern> {
    if reviews.is_empty() { return None; }

    // Count consecutive distinct days from most recent
    let mut days: Vec<chrono::NaiveDate> = reviews.iter()
        .map(|(ts, _)| ts.date_naive())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .rev()
        .collect();

    let today = Utc::now().date_naive();
    let mut streak = 0;
    let mut expected = today;
    for day in &days {
        if *day == expected || *day == expected.pred_opt().unwrap_or(expected) {
            streak += 1;
            expected = day.pred_opt().unwrap_or(*day);
        } else {
            break;
        }
    }

    // Celebrate milestones
    let milestones = [3, 7, 14, 30, 60, 100];
    if milestones.contains(&streak) {
        return Some(DetectedPattern {
            name: format!("study_streak_{streak}_days"),
            confidence: 0.95,
            signal_count: streak as i32,
            description: format!("{streak}-day study streak! Keep it up!"),
            domain: "learning".into(),
        });
    }

    // Nudge on streak break: last review was yesterday but not today
    let last_review_date = reviews.back().map(|(ts, _)| ts.date_naive());
    if let Some(last) = last_review_date {
        let gap = (today - last).num_days();
        if gap == 1 && streak >= 3 {
            return Some(DetectedPattern {
                name: "study_streak_at_risk".into(),
                confidence: 0.8,
                signal_count: streak as i32,
                description: format!("{streak}-day streak at risk — one review keeps it alive"),
                domain: "learning".into(),
            });
        }
    }

    None
}

/// RetentionDecayPattern: high-importance atom drops below 60%.
pub fn detect_retention_decay(
    digest_signals: &VecDeque<(DateTime<Utc>, String)>,
) -> Option<DetectedPattern> {
    // Triggered by CoachingLearningDigest with fading_count > 0
    let recent_fading: Vec<_> = digest_signals.iter()
        .filter(|(ts, _)| (Utc::now() - *ts).num_hours() < 24)
        .collect();

    if recent_fading.is_empty() { return None; }

    // Parse fading count from context (stored as value in signal metadata)
    Some(DetectedPattern {
        name: "high_importance_retention_decay".into(),
        confidence: 0.85,
        signal_count: recent_fading.len() as i32,
        description: "High-importance atoms are fading — a quick review would help".into(),
        domain: "learning".into(),
    })
}

/// LearningMomentumPattern: creating >> reviewing (or vice versa) over 7 days.
pub fn detect_learning_momentum(
    created: &VecDeque<(DateTime<Utc>, String)>,
    reviewed: &VecDeque<(DateTime<Utc>, String)>,
) -> Option<DetectedPattern> {
    let week_ago = Utc::now() - chrono::Duration::days(7);
    let created_7d = created.iter().filter(|(ts, _)| *ts > week_ago).count();
    let reviewed_7d = reviewed.iter().filter(|(ts, _)| *ts > week_ago).count();

    if created_7d == 0 && reviewed_7d == 0 { return None; }

    let ratio = if reviewed_7d > 0 { created_7d as f64 / reviewed_7d as f64 } else { 10.0 };

    if ratio > 3.0 && created_7d >= 5 {
        // Creating much more than reviewing
        Some(DetectedPattern {
            name: "learning_momentum_create_heavy".into(),
            confidence: (ratio / 5.0).min(0.9),
            signal_count: created_7d as i32,
            description: format!("Created {created_7d} atoms but only reviewed {reviewed_7d} — want a catch-up session?"),
            domain: "learning".into(),
        })
    } else if ratio < 0.3 && reviewed_7d >= 10 {
        // Reviewing much more than creating — strong retention week
        Some(DetectedPattern {
            name: "learning_momentum_review_strong".into(),
            confidence: 0.85,
            signal_count: reviewed_7d as i32,
            description: format!("Strong retention week — {reviewed_7d} reviews!"),
            domain: "learning".into(),
        })
    } else {
        None
    }
}

/// DomainGapPattern: retention declining across all atoms in a domain.
/// Triggered by CoachingLearningDigest with weakest_topic data.
pub fn detect_domain_gap(
    digest_signals: &VecDeque<(DateTime<Utc>, String)>,
) -> Option<DetectedPattern> {
    // Look for repeated digest signals indicating a weak domain
    let recent: Vec<_> = digest_signals.iter()
        .filter(|(ts, _)| (Utc::now() - *ts).num_days() < 7)
        .collect();

    if recent.len() >= 2 {
        return Some(DetectedPattern {
            name: "domain_retention_gap".into(),
            confidence: (recent.len() as f64 / 5.0).min(0.9),
            signal_count: recent.len() as i32,
            description: "Retention declining in a topic — a 15-min session would fix it".into(),
            domain: "learning".into(),
        });
    }
    None
}

/// KnowledgeTransferPattern: atom from one domain referenced in another.
pub fn detect_knowledge_transfer(
    transfer_signals: &VecDeque<(DateTime<Utc>, String)>,
) -> Option<DetectedPattern> {
    let recent: Vec<_> = transfer_signals.iter()
        .filter(|(ts, _)| (Utc::now() - *ts).num_hours() < 24)
        .collect();

    if !recent.is_empty() {
        Some(DetectedPattern {
            name: "knowledge_transfer_detected".into(),
            confidence: 0.9,
            signal_count: recent.len() as i32,
            description: "Your knowledge is connecting across domains — second brain working!".into(),
            domain: "learning".into(),
        })
    } else {
        None
    }
}
```

- [ ] **Step 2: Wire into PatternDetector**

In `crates/feature-coaching/src/pattern_detector.rs`, add calls to the learning pattern functions in `detect_patterns()`:

```rust
// At the end of detect_patterns(), after existing patterns:

// IMPORTANT: history keys MUST match condition names from default_conditions()
// in signal_accumulator/types.rs. The PatternDetector indexes by condition_name
// from TriggerFired, which is set by SignalAccumulator::evaluate_triggers().

// Learning: study streak (key = condition name from default_conditions)
if let Some(reviews) = self.history.get("flashcard_reviewed") {
    if let Some(p) = learning_patterns::detect_study_streak(reviews) {
        patterns.push(p);
    }
}

// Learning: retention decay
if let Some(digest) = self.history.get("coaching_learning_digest") {
    if let Some(p) = learning_patterns::detect_retention_decay(digest) {
        patterns.push(p);
    }
}

// Learning: momentum
if let (Some(created), Some(reviewed)) = (
    self.history.get("atom_created"),
    self.history.get("flashcard_reviewed"),
) {
    if let Some(p) = learning_patterns::detect_learning_momentum(created, reviewed) {
        patterns.push(p);
    }
}

// Learning: domain gap
if let Some(digest) = self.history.get("coaching_learning_digest") {
    if let Some(p) = learning_patterns::detect_domain_gap(digest) {
        patterns.push(p);
    }
}

// Learning: knowledge transfer
if let Some(transfers) = self.history.get("knowledge_transfer") {
    if let Some(p) = learning_patterns::detect_knowledge_transfer(transfers) {
        patterns.push(p);
    }
}
```

Add `mod learning_patterns;` at the top of `crates/feature-coaching/src/pattern_detector.rs` (NOT in `lib.rs` — this is an internal module consumed only by the pattern detector, not part of the crate's public API). Then add `use learning_patterns;` or reference as `learning_patterns::detect_*`.

- [ ] **Step 3: Write tests for each pattern**

Add tests in `learning_patterns.rs` with concrete scenarios for streak milestones, streak-at-risk, momentum imbalances, etc.

- [ ] **Step 4: Build and run tests**

Run: `cargo build -p feature-coaching 2>&1 | tail -10`
Run: `cargo nextest run -p feature-coaching 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```
feat(feature-coaching): add 5 learning pattern detectors — streak, decay, momentum, gap, transfer
```

---

### Task 4: Learning coaching message templates

**Files:**
- Create: `crates/feature-coaching/src/learning_templates.rs`
- Modify: `crates/feature-coaching/src/lib.rs`

- [ ] **Step 1: Create message templates**

70% celebration, 30% gentle correction. Each template returns (message, intervention_type).

```rust
use crate::reasoner::InterventionType;

pub struct LearningMessage {
    pub message: String,
    pub intervention_type: InterventionType,
}

/// Generate a coaching message for a learning pattern.
pub fn learning_message(pattern_name: &str, signal_count: i32, description: &str) -> LearningMessage {
    match pattern_name {
        // Celebrations (70%)
        p if p.starts_with("study_streak_") && !p.contains("risk") => LearningMessage {
            message: format!("🔥 {description} You're building real knowledge depth."),
            intervention_type: InterventionType::DashboardCard,
        },
        "learning_momentum_review_strong" => LearningMessage {
            message: format!("💪 {description} Your retention is rock-solid."),
            intervention_type: InterventionType::DashboardCard,
        },
        "knowledge_transfer_detected" => LearningMessage {
            message: format!("🧠 {description}"),
            intervention_type: InterventionType::ChatMessage,
        },

        // Gentle nudges (30%)
        "study_streak_at_risk" => LearningMessage {
            message: format!("📚 {description}"),
            intervention_type: InterventionType::ChatMessage,
        },
        "high_importance_retention_decay" => LearningMessage {
            message: format!("🔔 {description}"),
            intervention_type: InterventionType::ChatMessage,
        },
        "learning_momentum_create_heavy" => LearningMessage {
            message: format!("📖 {description}"),
            intervention_type: InterventionType::DashboardCard,
        },
        "domain_retention_gap" => LearningMessage {
            message: format!("📉 {description}"),
            intervention_type: InterventionType::DashboardCard,
        },

        // Fallback
        _ => LearningMessage {
            message: description.to_string(),
            intervention_type: InterventionType::DashboardCard,
        },
    }
}
```

- [ ] **Step 2: Export and build**

Add `pub mod learning_templates;` to lib.rs. Build: `cargo build -p feature-coaching`

- [ ] **Step 3: Commit**

```
feat(feature-coaching): add learning coaching message templates (70% celebration)
```

---

### Task 5: Morning briefing — Knowledge Health section

**Files:**
- Create: `crates/app-core/src/handlers/morning_briefing.rs`
- Create: `crates/desktop-shared/src/commands/morning_briefing.rs`
- Create: `crates/desktop/src/commands/morning_briefing.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create IPC types**

```rust
// crates/desktop-shared/src/commands/morning_briefing.rs
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MorningBriefingResponse {
    pub streak_days: usize,
    pub due_cards: i64,
    pub fading_atoms: Vec<FadingAtomSummary>,
    pub strongest_topic: Option<TopicSummary>,
    pub weakest_topic: Option<TopicSummary>,
    pub atoms_reviewed_this_week: i64,
    pub atoms_created_this_week: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FadingAtomSummary {
    pub id: String,
    pub subject: String,
    pub retention_pct: f64,
    pub domain: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicSummary {
    pub name: String,
    pub avg_retention: f64,
    pub atom_count: i64,
}
```

- [ ] **Step 1b: Register in desktop-shared/commands/mod.rs**

Add `pub mod morning_briefing;` and re-export all types via `pub use morning_briefing::*;` in `crates/desktop-shared/src/commands/mod.rs`. Follow the same pattern as `knowledge_health` exports.

- [ ] **Step 2: Create AppCore handler**

```rust
// crates/app-core/src/handlers/morning_briefing.rs
use super::atoms::map_db;
use crate::state::AppCore;
use desktop_shared::commands::{MorningBriefingResponse, FadingAtomSummary, TopicSummary};
use desktop_shared::errors::ApiError;

impl AppCore {
    pub async fn morning_briefing(&self) -> Result<MorningBriefingResponse, ApiError> {
        let atom_repo = self.knowledge_atom_repo()?;
        let fc_repo = self.flashcard_repo()?;

        // Streak
        let review_stats = cognitive::ReviewStatsRepo::new(atom_repo.pool().clone());
        let streak = review_stats.current_streak().await.unwrap_or(0);

        // Due cards
        let due = fc_repo.total_due_count().await.map_err(map_db)?;

        // Fading atoms (retention < 0.6, importance > 0.7, active)
        let stale = atom_repo.list_stale_active(1).await.map_err(map_db)?;
        let fading: Vec<FadingAtomSummary> = stale.iter()
            .filter(|a| a.retention_pct < 0.6 && a.personal_importance > 0.7)
            .take(5)
            .map(|a| FadingAtomSummary {
                id: a.id.clone(),
                subject: a.subject.clone(),
                retention_pct: a.retention_pct,
                domain: a.domain.clone(),
            })
            .collect();

        // Topics
        let topics = atom_repo.list_topics_with_atoms().await.unwrap_or_default();
        let weakest = topics.first().map(|t| TopicSummary {
            name: t.name.clone(), avg_retention: t.avg_retention, atom_count: t.atom_count,
        });
        let strongest = topics.last().map(|t| TopicSummary {
            name: t.name.clone(), avg_retention: t.avg_retention, atom_count: t.atom_count,
        });

        // Weekly stats
        let daily = review_stats.daily_reviews(7).await.unwrap_or_default();
        let reviews_week: i64 = daily.iter().map(|d| d.review_count).sum();

        // Count atoms created this week
        let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let created_week: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM knowledge_atoms WHERE created_at > ?1",
        )
        .bind(&week_ago)
        .fetch_one(atom_repo.pool())
        .await
        .map(|(c,)| c)
        .unwrap_or(0);

        Ok(MorningBriefingResponse {
            streak_days: streak,
            due_cards: due,
            fading_atoms: fading,
            strongest_topic: strongest,
            weakest_topic: weakest,
            atoms_reviewed_this_week: reviews_week,
            atoms_created_this_week: created_week,
        })
    }
}
```

- [ ] **Step 3: Create Tauri command + DEV_COMMANDS**

Follow the pattern from `commands/knowledge_health.rs`. Single command: `morning_briefing_summary`.

- [ ] **Step 4: Register cron (9 AM daily)**

In `crates/app-core/src/init/cron.rs`, add `JOB_MORNING_BRIEFING` constant, handler, and ensure_job (9 AM local). The handler calls `morning_briefing()` and delivers via the coaching intervention channel.

- [ ] **Step 5: Build and run dev_server parity test**

Run: `cargo build -p desktop 2>&1 | tail -10`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)' 2>&1 | tail -5`

- [ ] **Step 6: Commit**

```
feat(app-core,desktop): add morning briefing with Knowledge Health data + 9 AM cron
```

---

### Task 6: Morning briefing frontend — coaching overview integration

**Files:**
- Create: `desktop-ui/src/features/coaching/components/MorningBriefing.tsx`
- Modify: `desktop-ui/src/features/coaching/pages/OverviewPage.tsx`

- [ ] **Step 1: Create MorningBriefing component**

```tsx
import { useQuery } from "@shared/hooks/useQuery";

interface MorningBriefingData {
  streakDays: number;
  dueCards: number;
  fadingAtoms: { id: string; subject: string; retentionPct: number; domain: string }[];
  strongestTopic: { name: string; avgRetention: number; atomCount: number } | null;
  weakestTopic: { name: string; avgRetention: number; atomCount: number } | null;
  atomsReviewedThisWeek: number;
  atomsCreatedThisWeek: number;
}

export function MorningBriefing() {
  const { data } = useQuery<MorningBriefingData>("morning_briefing_summary", undefined, {
    streakDays: 0, dueCards: 0, fadingAtoms: [], strongestTopic: null,
    weakestTopic: null, atomsReviewedThisWeek: 0, atomsCreatedThisWeek: 0,
  });

  // Render: streak flame, due cards count, fading atoms list,
  // strongest/weakest topic bars, "Start 5-min review" + "See details" buttons
}
```

Design: glass-card with stats row (streak, due, reviewed this week), fading atoms list with retention bars, action buttons linking to `/learn` and `/learn/knowledge`.

- [ ] **Step 2: Add to coaching OverviewPage**

Insert `<MorningBriefing />` at the top of the coaching overview page, before existing coaching cards.

- [ ] **Step 3: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```
feat(desktop-ui): add Morning Briefing component with Knowledge Health section
```

---

### Task 7: Focus session micro-review prompt

**Files:**
- Create: `desktop-ui/src/features/coaching/components/MicroReviewPrompt.tsx`
- Modify: `crates/feature-coaching/src/service.rs`

- [ ] **Step 1: Add pre-focus micro-review check to CoachingService**

In the `FocusSessionStarted` match arm in `service.rs`, before setting `focus_active = true`, check if there are fading atoms in the same domain. If found, emit a special intervention:

```rust
DomainEvent::FocusSessionStarted { .. } => {
    // Check for fading high-importance atoms — offer micro-review
    // Query is async, use the review_stats_repo if available
    // For simplicity, emit a "micro_review_available" intervention
    // The frontend decides whether to show it
    debug!("Focus session started — coaching delivery paused");
    focus_active = true;
}
```

The micro-review injection is better handled frontend-side: when a focus session starts, the productivity timer UI checks for due atoms and shows the prompt. This avoids complex async in the coaching pipeline.

- [ ] **Step 2: Create MicroReviewPrompt component**

```tsx
import { useState } from "react";
import { useQuery } from "@shared/hooks/useQuery";

interface MicroReviewPromptProps {
  onAccept: () => void;
  onSkip: () => void;
}

export function MicroReviewPrompt({ onAccept, onSkip }: MicroReviewPromptProps) {
  // Query due atoms count
  // Show: "Before you dive in — 45s review to keep your streak alive?"
  // Buttons: "Quick Review (45s)" / "Skip"
}
```

This component is shown inline in the focus timer start screen when due atoms exist.

- [ ] **Step 3: Wire into focus timer UI**

In the productivity focus timer component, before the timer starts, check if `dueCards > 0` and show `<MicroReviewPrompt>`. On accept, navigate to a quick review flow; on skip, start the timer normally.

- [ ] **Step 4: Commit**

```
feat(desktop-ui): add focus session micro-review prompt (45s pre-session)
```

---

### Task 8: Autotuner — knowledge_retention_score metric

**Files:**
- Modify: `crates/autotuner/src/traits.rs`
- Modify: `crates/agent/src/autotuner/metric_collector.rs`

- [ ] **Step 1: Add field to MetricSnapshot**

In `crates/autotuner/src/traits.rs`, add to `MetricSnapshot`:

```rust
/// Phase 3: importance-weighted avg retention across active knowledge atoms.
/// Long-term metric — changes over weeks, not per-trial.
pub knowledge_retention_score: f64,
```

- [ ] **Step 2: Compute in metric collector**

In `crates/agent/src/autotuner/metric_collector.rs`:

1. Add a `review_stats: cognitive::ReviewStatsRepo` field to `AgentMetricCollector`
2. Initialize it in the constructor from an existing repo's pool: `review_stats: cognitive::ReviewStatsRepo::new(fact_repo.pool().clone())` (or pass the pool explicitly — check what repos are already available in `new()`)
3. In `collect_metrics()`, add:

```rust
let knowledge_retention_score = self.review_stats
    .knowledge_retention_score().await.unwrap_or(1.0);
```

Set `snapshot.knowledge_retention_score = knowledge_retention_score;`

Note: `AgentMetricCollector::new()` already receives typed repos (strategy_repo, event_log_repo, usage_repo, trial_repo, fact_repo). Derive the pool from `fact_repo` which accesses the cognitive database.

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace 2>&1 | tail -10`
Fix any compilation errors (other crates that construct `MetricSnapshot` will need the new field with a default).

- [ ] **Step 4: Commit**

```
feat(autotuner): add knowledge_retention_score metric from atom data
```

---

### Task 9: Focused review page + "Start focused review" action

**Files:**
- Create: `desktop-ui/src/features/learn/components/FocusedReview.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Modify: `desktop-ui/src/features/learn/components/DashboardHome.tsx`
- Modify: `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx`

- [ ] **Step 1: Create FocusedReview page**

A topic-scoped review page that fetches due cards for a specific topic/deck and presents them in sequence. Reuses the existing flashcard review UI pattern from the Learn feature but filtered to a topic.

- [ ] **Step 2: Add route**

In `router.tsx`, add lazy route: `/learn/review/:topicId?` (optional topicId for all-topics review).

- [ ] **Step 3: Add "Start focused review" action**

In `DashboardHome.tsx`, add a quick action button linking to `/learn/review`.
In `KnowledgeHealth.tsx`, add per-topic "Review" buttons linking to `/learn/review/:topicId`.
In `MorningBriefing.tsx`, "Start 5-min review" links to `/learn/review`.

- [ ] **Step 4: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```
feat(desktop-ui): add focused review page with topic-scoped card review
```

---

### Task 10: Fading atom badge on atom cards

**Files:**
- Modify: `desktop-ui/src/features/notes/components/AtomCard.tsx`

**Scope note:** The spec mentions "Inline editor retention badge (fading atom margin pill)". This task implements a simplified version — a pulsing indicator on the atom card in the Knowledge Atoms panel, not a margin annotation in the note editor body. A full editor margin integration would require TipTap extension work and is deferred.

- [ ] **Step 1: Add fading indicator to atom cards**

When `retentionPct < 0.5`, show a small pulsing dot or badge on the atom card to draw attention:

```tsx
{atom.retentionPct < 0.5 && (
  <span className="h-1.5 w-1.5 rounded-full bg-red-500 animate-pulse shrink-0" />
)}
```

Add this inside the atom card, next to the retention percentage.

- [ ] **Step 2: Lint and commit**

```
feat(desktop-ui): add fading atom indicator badge in note editor
```

---

### Task 11: Final integration — workspace build + test

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 2: Run all related tests**

Run: `cargo nextest run --workspace -E 'test(review_stats) | test(learning_pattern) | test(knowledge_atom) | test(dev_server_covers)'`
Expected: all pass

- [ ] **Step 3: Clippy + fmt**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep -E "^error|warning:.*learning|warning:.*streak|warning:.*briefing" | head -10`
Run: `cargo fmt --all --check`

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```
feat: Knowledge Atoms Phase 3 complete — coaching patterns, morning briefing, micro-review
```

---

## Verification Checklist

After all tasks complete:

- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] `cd desktop-ui && bun run lint` — no new errors
- [ ] Dev server parity test passes
- [ ] Manual test: review 3 days in a row → streak pattern fires → celebration coaching message
- [ ] Manual test: morning briefing command returns streak + due cards + fading atoms
- [ ] Manual test: Knowledge Health section visible on coaching overview page
- [ ] Manual test: focused review page loads with topic-scoped cards
- [ ] Manual test: fading atom badge shows on low-retention atom cards
