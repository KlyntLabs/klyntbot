# Cognitive Debug Dashboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a 5-tab developer debug dashboard (`/debug`) that exposes the full cognitive architecture (memory, coaching, events, pipeline, system) from the desktop UI.

**Architecture:** DTOs in `desktop-shared` → Tauri commands in `desktop/commands/cognitive.rs` → register in `main.rs` → mirror in `dev-api` → React tabs with `useQuery`/`useMutation`/`useEvent` hooks. Coaching engine state added to `AppCore` via `Option<Arc<Mutex<>>>`. Live domain events forwarded to frontend via Tauri emit.

**Tech Stack:** Rust (Tauri 2, sqlx, tokio), React 19, TypeScript, Tailwind v4, lucide-react icons

**Design doc:** `docs/plans/2026-03-07-cognitive-debug-dashboard-design.md`

---

## Phase 1: Backend DTOs (desktop-shared)

### Task 1: Cognitive Memory DTOs

**Files:**
- Create: `crates/desktop-shared/src/cognitive_commands.rs`
- Modify: `crates/desktop-shared/src/lib.rs`

**Step 1: Create the DTO file with memory response types**

```rust
// crates/desktop-shared/src/cognitive_commands.rs

use serde::{Deserialize, Serialize};

// ── Memory DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticFactResponse {
    pub id: String,
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub stability: f64,
    pub retrievability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
    pub status: String, // "active" | "superseded" | "archived"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodicMemoryResponse {
    pub id: String,
    pub domain: String,
    pub content: String,
    pub summary: Option<String>,
    pub importance: f64,
    pub occurred_at: String,
    pub recorded_at: String,
    pub stability: f64,
    pub access_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProceduralRuleResponse {
    pub id: String,
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub source: String,
    pub signal_count: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserModelSummaryResponse {
    pub identity_count: usize,
    pub energy_count: usize,
    pub work_count: usize,
    pub finance_count: usize,
    pub learning_count: usize,
    pub preferences_count: usize,
    pub identity_preview: Vec<String>,
    pub energy_preview: Vec<String>,
    pub work_preview: Vec<String>,
    pub finance_preview: Vec<String>,
    pub learning_preview: Vec<String>,
    pub preferences_preview: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatsResponse {
    pub active_facts: usize,
    pub archived_facts: u64,
    pub episodic_count: usize,
    pub rules_count: usize,
    pub last_compaction: Option<String>,
}

// ── Coaching DTOs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSituationResponse {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
    pub coaching_receptivity: f64,
    pub task_avoidance_detected: bool,
    pub hours_active_today: f64,
    pub mins_since_break: f64,
    pub hour_of_day: u32,
    pub recent_context_switches: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalResponse {
    pub event_type: String,
    pub timestamp: String,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerConditionResponse {
    pub name: String,
    pub cooldown_remaining_secs: i64,
    pub last_fired: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalWindowResponse {
    pub window_size: usize,
    pub signals: Vec<SignalResponse>,
    pub triggers: Vec<TriggerConditionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPatternResponse {
    pub name: String,
    pub confidence: f64,
    pub signal_count: usize,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveredInterventionResponse {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFeedbackResponse {
    pub trigger_name: String,
    pub intervention_type: String,
    pub times_delivered: u32,
    pub acceptance_rate: f64,
    pub effectiveness: f64,
    pub behavioral_positive: u32,
    pub behavioral_negative: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterStatusResponse {
    pub hourly_count: usize,
    pub hourly_limit: usize,
    pub daily_count: usize,
    pub daily_limit: usize,
}

// ── Events DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEventPayload {
    pub event_type: String,
    pub salience: String, // "extract" | "accumulate" | "discard"
    pub domain: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

// ── System DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatusResponse {
    pub name: String,
    pub status: String, // "wired" | "built" | "stub"
    pub handler_type: String, // "heuristic" | "llm" | "n/a"
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusResponse {
    pub domain_bus_subscribers: usize,
    pub domain_bus_published: u64,
    pub background_service_running: bool,
    pub background_events_processed: u64,
    pub active_facts: usize,
    pub episodic_count: usize,
    pub rules_count: usize,
    pub components: Vec<ComponentStatusResponse>,
}

// ── Mutation Params ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactCreateParams {
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactUpdateParams {
    pub object: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCreateParams {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResultResponse {
    pub archived_count: u64,
    pub deleted_episodic: u64,
}
```

**Step 2: Register the module in `desktop-shared/src/lib.rs`**

Add `pub mod cognitive_commands;` to the module declarations in `crates/desktop-shared/src/lib.rs`.

**Step 3: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add crates/desktop-shared/src/cognitive_commands.rs crates/desktop-shared/src/lib.rs
git commit -m "feat(desktop-shared): add cognitive debug dashboard DTOs"
```

---

## Phase 2: Coaching Engine on AppCore

### Task 2: Add CoachingEngine wrapper struct

The coaching engine components (SignalAccumulator, PatternDetector, InterventionRouter, FeedbackTracker) are currently in-memory only in the agent's background service. We need to expose them via AppCore so Tauri commands can read/mutate their state.

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

**Step 1: Add coaching fields to AppCore struct**

In `crates/desktop/src/app_core.rs`, add these imports at the top:

```rust
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use cognitive::situation::UserSituation;
```

Add these fields to the `AppCore` struct (after `distraction_interceptor`):

```rust
    /// Cognitive domain event bus.
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    /// Coaching signal accumulator.
    signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    /// Coaching pattern detector.
    pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    /// Coaching intervention router.
    intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    /// Coaching feedback tracker.
    feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    /// Cached user situation.
    user_situation: Option<Arc<Mutex<UserSituation>>>,
```

**Step 2: Add accessor methods**

Add these accessor methods to the `impl AppCore` block (follow the existing `focus_manager()` pattern):

```rust
    pub fn signal_accumulator(&self) -> Result<&Arc<Mutex<SignalAccumulator>>, ApiError> {
        self.signal_accumulator.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "coaching engine is not available")
        })
    }

    pub fn pattern_detector(&self) -> Result<&Arc<Mutex<PatternDetector>>, ApiError> {
        self.pattern_detector.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "coaching engine is not available")
        })
    }

    pub fn intervention_router(&self) -> Result<&Arc<Mutex<InterventionRouter>>, ApiError> {
        self.intervention_router.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "coaching engine is not available")
        })
    }

    pub fn feedback_tracker(&self) -> Result<&Arc<Mutex<FeedbackTracker>>, ApiError> {
        self.feedback_tracker.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "coaching engine is not available")
        })
    }

    pub fn user_situation(&self) -> Result<&Arc<Mutex<UserSituation>>, ApiError> {
        self.user_situation.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "coaching engine is not available")
        })
    }

    pub fn domain_event_bus(&self) -> Result<&Arc<bus::DomainEventBus>, ApiError> {
        self.domain_event_bus.as_ref().ok_or_else(|| {
            ApiError::new("FEATURE_DISABLED", "domain event bus is not available")
        })
    }
```

**Step 3: Initialize coaching fields in `AppCore::init()`**

In the `init()` method, after the agent and bus are wired, create and store coaching state. The existing code creates a `DomainEventBus` at line ~138. After that, add:

```rust
    let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
    let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
    let intervention_router = Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
    let feedback_tracker = Arc::new(Mutex::new(FeedbackTracker::new()));
    let user_situation = Arc::new(Mutex::new(UserSituation::default()));
```

And in the `AppCore { ... }` struct initialization, add:

```rust
    domain_event_bus: Some(Arc::clone(&domain_bus)),
    signal_accumulator: Some(signal_accumulator),
    pattern_detector: Some(pattern_detector),
    intervention_router: Some(intervention_router),
    feedback_tracker: Some(feedback_tracker),
    user_situation: Some(user_situation),
```

**Step 4: Verify it compiles**

Run: `cargo build -p desktop`
Expected: compiles (may need to add `feature-coaching` to desktop's `Cargo.toml` dependencies)

**Step 5: Commit**

```bash
git add crates/desktop/src/app_core.rs crates/desktop/Cargo.toml
git commit -m "feat(desktop): add coaching engine state to AppCore"
```

---

## Phase 3: Cognitive Tauri Commands

### Task 3: Read commands for Memory tab

**Files:**
- Create: `crates/desktop/src/commands/cognitive.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

**Step 1: Create cognitive commands file with memory reads**

```rust
// crates/desktop/src/commands/cognitive.rs

use std::sync::Arc;

use cognitive::decay::retrievability;
use cognitive::repos::{load_user_model, SemanticFactRepo};
use cognitive::types::SemanticFact;
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

fn fact_to_response(f: &SemanticFact) -> SemanticFactResponse {
    let elapsed_days = chrono::Utc::now()
        .signed_duration_since(
            f.last_accessed
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|| {
                    chrono::DateTime::parse_from_rfc3339(&f.recorded_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now())
                }),
        )
        .num_seconds() as f64
        / 86400.0;
    let r = retrievability(elapsed_days, f.stability);

    let status = if f.superseded_at.is_some() {
        "superseded"
    } else {
        "active"
    };

    SemanticFactResponse {
        id: f.id.clone(),
        domain: f.domain.clone(),
        subject: f.subject.clone(),
        predicate: f.predicate.clone(),
        object: f.object.clone(),
        confidence: f.confidence,
        source: f.source.clone(),
        valid_from: f.valid_from.clone(),
        valid_until: f.valid_until.clone(),
        stability: f.stability,
        retrievability: r,
        last_accessed: f.last_accessed.clone(),
        access_count: f.access_count,
        status: status.to_string(),
    }
}

fn fact_preview(fact: &SemanticFact) -> String {
    format!("{} = {}", fact.predicate, fact.object)
}

// ── Memory Reads ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_user_model(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserModelSummaryResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let model = load_user_model(&fact_repo).await;

    Ok(UserModelSummaryResponse {
        identity_count: model.identity.len(),
        energy_count: model.energy.len(),
        work_count: model.work.len(),
        finance_count: model.finance.len(),
        learning_count: model.learning.len(),
        preferences_count: model.preferences.len(),
        identity_preview: model.identity.iter().take(3).map(fact_preview).collect(),
        energy_preview: model.energy.iter().take(3).map(fact_preview).collect(),
        work_preview: model.work.iter().take(3).map(fact_preview).collect(),
        finance_preview: model.finance.iter().take(3).map(fact_preview).collect(),
        learning_preview: model.learning.iter().take(3).map(fact_preview).collect(),
        preferences_preview: model.preferences.iter().take(3).map(fact_preview).collect(),
    })
}

#[tauri::command]
pub async fn cognitive_facts_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    let domains = match domain {
        Some(d) => vec![d],
        None => vec![
            "identity".to_string(),
            "energy".to_string(),
            "work".to_string(),
            "finance".to_string(),
            "learning".to_string(),
            "preferences".to_string(),
        ],
    };

    let mut all_facts = Vec::new();
    for d in &domains {
        let facts = fact_repo.list_active(d).await.map_err(|e| {
            ApiError::new("STORAGE_ERROR", e.to_string())
        })?;
        all_facts.extend(facts);
    }

    Ok(all_facts.iter().map(fact_to_response).collect())
}

#[tauri::command]
pub async fn cognitive_episodic_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EpisodicMemoryResponse>, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
    let limit = limit.unwrap_or(50);

    let memories = match domain {
        Some(d) => repo.list_by_domain(&d, limit).await,
        None => {
            // List from all domains
            let mut all = Vec::new();
            for d in &["identity", "energy", "work", "finance", "learning", "preferences"] {
                let mems = repo.list_by_domain(d, limit).await.map_err(|e| {
                    ApiError::new("STORAGE_ERROR", e.to_string())
                })?;
                all.extend(mems);
            }
            Ok(all)
        }
    }
    .map_err(|e: sqlx::Error| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(memories
        .iter()
        .map(|m| EpisodicMemoryResponse {
            id: m.id.clone(),
            domain: m.domain.clone(),
            content: m.content.clone(),
            summary: m.summary.clone(),
            importance: m.importance,
            occurred_at: m.occurred_at.clone(),
            recorded_at: m.recorded_at.clone(),
            stability: m.stability,
            access_count: m.access_count,
        })
        .collect())
}

#[tauri::command]
pub async fn cognitive_rules_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<ProceduralRuleResponse>, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    let domains = match domain {
        Some(d) => vec![d],
        None => vec![
            "identity".to_string(),
            "energy".to_string(),
            "work".to_string(),
            "finance".to_string(),
            "learning".to_string(),
            "preferences".to_string(),
        ],
    };

    let mut all_rules = Vec::new();
    for d in &domains {
        let rules = repo.list_active(d).await.map_err(|e| {
            ApiError::new("STORAGE_ERROR", e.to_string())
        })?;
        all_rules.extend(rules);
    }

    Ok(all_rules
        .iter()
        .map(|r| ProceduralRuleResponse {
            id: r.id.clone(),
            domain: r.domain.clone(),
            rule_text: r.rule_text.clone(),
            confidence: r.confidence,
            source: r.source.clone(),
            signal_count: r.signal_count,
            active: r.active,
            created_at: r.created_at.clone(),
            updated_at: r.updated_at.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn cognitive_memory_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<MemoryStatsResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
    let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    let model = load_user_model(&fact_repo).await;
    let active_facts = model.identity.len()
        + model.energy.len()
        + model.work.len()
        + model.finance.len()
        + model.learning.len()
        + model.preferences.len();

    let archived = fact_repo.archive_superseded(0).await.unwrap_or(0);

    let mut episodic_count = 0;
    for d in &["identity", "energy", "work", "finance", "learning", "preferences"] {
        episodic_count += episodic_repo
            .list_by_domain(d, 10000)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
    }

    let mut rules_count = 0;
    for d in &["identity", "energy", "work", "finance", "learning", "preferences"] {
        rules_count += rule_repo
            .list_active(d)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
    }

    Ok(MemoryStatsResponse {
        active_facts,
        archived_facts: archived,
        episodic_count,
        rules_count,
        last_compaction: None, // TODO: track last compaction time
    })
}
```

**Step 2: Register the module**

Add `pub mod cognitive;` to `crates/desktop/src/commands/mod.rs`.

**Step 3: Verify it compiles**

Run: `cargo build -p desktop`
Expected: compiles (may need to add `cognitive` to `desktop/Cargo.toml` dependencies)

**Step 4: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs crates/desktop/src/commands/mod.rs crates/desktop/Cargo.toml
git commit -m "feat(desktop): cognitive memory read commands"
```

---

### Task 4: Coaching read commands

**Files:**
- Modify: `crates/desktop/src/commands/cognitive.rs`

**Step 1: Add coaching read commands**

Append to `crates/desktop/src/commands/cognitive.rs`:

```rust
// ── Coaching Reads ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_situation(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserSituationResponse, ApiError> {
    let sit = state.user_situation()?.lock().await;
    Ok(UserSituationResponse {
        energy_level: sit.energy_level,
        focus_state: sit.focus_state,
        deadline_pressure: sit.deadline_pressure,
        distraction_risk: sit.distraction_risk,
        coaching_receptivity: sit.coaching_receptivity,
        task_avoidance_detected: sit.task_avoidance_detected,
        hours_active_today: sit.hours_active_today,
        mins_since_break: sit.mins_since_break,
        hour_of_day: sit.hour_of_day,
        recent_context_switches: sit.recent_context_switches,
    })
}

#[tauri::command]
pub async fn coaching_signals(
    state: State<'_, Arc<AppCore>>,
) -> Result<SignalWindowResponse, ApiError> {
    let acc = state.signal_accumulator()?.lock().await;

    Ok(SignalWindowResponse {
        window_size: acc.window_size(),
        signals: vec![], // SignalAccumulator's signals are private; return window_size for now
        triggers: vec![], // TriggerConditions are private; expose via accessors in Phase 2+
    })
}

#[tauri::command]
pub async fn coaching_patterns(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<DetectedPatternResponse>, ApiError> {
    let detector = state.pattern_detector()?.lock().await;
    let patterns = detector.detect_patterns();

    Ok(patterns
        .iter()
        .map(|p| DetectedPatternResponse {
            name: p.name.clone(),
            confidence: p.confidence,
            signal_count: p.signal_count,
            description: p.description.clone(),
            domain: p.domain.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn coaching_feedback_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StrategyFeedbackResponse>, ApiError> {
    let tracker = state.feedback_tracker()?.lock().await;

    Ok(tracker
        .all_strategies()
        .iter()
        .map(|s| StrategyFeedbackResponse {
            trigger_name: s.trigger_name.clone(),
            intervention_type: s.intervention_type.clone(),
            times_delivered: s.times_delivered,
            acceptance_rate: s.acceptance_rate(),
            effectiveness: s.effectiveness(),
            behavioral_positive: s.behavioral_positive,
            behavioral_negative: s.behavioral_negative,
        })
        .collect())
}

#[tauri::command]
pub async fn coaching_router_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<RouterStatusResponse, ApiError> {
    let router = state.intervention_router()?.lock().await;

    Ok(RouterStatusResponse {
        hourly_count: router.hourly_count(),
        hourly_limit: 3,
        daily_count: 0, // TODO: expose daily_count on InterventionRouter
        daily_limit: 10,
    })
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop`

**Step 3: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs
git commit -m "feat(desktop): coaching read commands"
```

---

### Task 5: System status command

**Files:**
- Modify: `crates/desktop/src/commands/cognitive.rs`

**Step 1: Add system status command**

Append to `crates/desktop/src/commands/cognitive.rs`:

```rust
// ── System Status ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_system_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<SystemStatusResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let model = load_user_model(&fact_repo).await;

    let active_facts = model.identity.len()
        + model.energy.len()
        + model.work.len()
        + model.finance.len()
        + model.learning.len()
        + model.preferences.len();

    let components = vec![
        ComponentStatusResponse {
            name: "DomainEvent Bus".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "tokio::broadcast, 256 capacity".into(),
        },
        ComponentStatusResponse {
            name: "Salience Filter".into(),
            status: "wired".into(),
            handler_type: "heuristic".into(),
            notes: "Rule-based classification".into(),
        },
        ComponentStatusResponse {
            name: "Extraction".into(),
            status: "wired".into(),
            handler_type: "heuristic".into(),
            notes: "HeuristicExtractionHandler in agent crate".into(),
        },
        ComponentStatusResponse {
            name: "Consolidation".into(),
            status: "wired".into(),
            handler_type: "heuristic".into(),
            notes: "HeuristicConsolidationHandler in agent crate".into(),
        },
        ComponentStatusResponse {
            name: "FSRS Decay".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "R = exp(ln(0.9) * elapsed / stability)".into(),
        },
        ComponentStatusResponse {
            name: "Compaction".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "Archive superseded, prune episodic, size budget".into(),
        },
        ComponentStatusResponse {
            name: "Reflection".into(),
            status: "built".into(),
            handler_type: "heuristic".into(),
            notes: "ReflectionHandler trait defined, needs scheduling".into(),
        },
        ComponentStatusResponse {
            name: "Context Source".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "CognitiveContextSource at priority 60, 60s cache".into(),
        },
        ComponentStatusResponse {
            name: "UserSituation".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "compute_situation() from SituationInputs".into(),
        },
        ComponentStatusResponse {
            name: "Signal Accumulator".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "30min rolling window, 7 trigger conditions".into(),
        },
        ComponentStatusResponse {
            name: "Pattern Detector".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "5 pattern types detected".into(),
        },
        ComponentStatusResponse {
            name: "Intervention Router".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "Rate limits: 3/hr, 10/day, exponential backoff".into(),
        },
        ComponentStatusResponse {
            name: "Feedback Tracker".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "3 channels: explicit, behavioral, outcome".into(),
        },
        ComponentStatusResponse {
            name: "LLM Extraction".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "Trait defined, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Consolidation".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "Trait defined, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Reflection".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "ReflectionHandler trait, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Coaching Reasoner".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "CoachingReasonerHandler trait, heuristic fallback exists".into(),
        },
        ComponentStatusResponse {
            name: "Background Consolidation".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "DomainEventBus subscriber, accumulation buffers".into(),
        },
    ];

    Ok(SystemStatusResponse {
        domain_bus_subscribers: 0, // TODO: expose subscriber_count on DomainEventBus
        domain_bus_published: 0,
        background_service_running: true, // assume running if AppCore is up
        background_events_processed: 0,
        active_facts,
        episodic_count: 0,
        rules_count: 0,
        components,
    })
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop`

**Step 3: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs
git commit -m "feat(desktop): system status and completeness matrix command"
```

---

### Task 6: Write/mutation commands

**Files:**
- Modify: `crates/desktop/src/commands/cognitive.rs`

**Step 1: Add mutation commands**

Append to `crates/desktop/src/commands/cognitive.rs`:

```rust
// ── Mutations ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_fact_create(
    state: State<'_, Arc<AppCore>>,
    params: FactCreateParams,
) -> Result<SemanticFactResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    let now = chrono::Utc::now().to_rfc3339();
    let fact = SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: params.domain,
        subject: params.subject,
        predicate: params.predicate,
        object: params.object,
        confidence: params.confidence,
        source: "debug_dashboard".to_string(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now,
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
    };

    fact_repo
        .upsert(&fact)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(fact_to_response(&fact))
}

#[tauri::command]
pub async fn cognitive_fact_update(
    state: State<'_, Arc<AppCore>>,
    id: String,
    params: FactUpdateParams,
) -> Result<SemanticFactResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    let mut fact = fact_repo
        .get(&id)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("fact {id} not found")))?;

    if let Some(obj) = params.object {
        fact.object = obj;
    }
    if let Some(conf) = params.confidence {
        fact.confidence = conf;
    }

    fact_repo
        .upsert(&fact)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(fact_to_response(&fact))
}

#[tauri::command]
pub async fn cognitive_fact_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    // Supersede the fact (mark as deleted, don't hard-delete)
    fact_repo
        .supersede(&id, "deleted_by_debug_dashboard")
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(true)
}

#[tauri::command]
pub async fn cognitive_rule_create(
    state: State<'_, Arc<AppCore>>,
    params: RuleCreateParams,
) -> Result<ProceduralRuleResponse, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
    let now = chrono::Utc::now().to_rfc3339();

    let rule = cognitive::types::ProceduralRule {
        id: uuid::Uuid::new_v4().to_string(),
        domain: params.domain,
        rule_text: params.rule_text,
        confidence: params.confidence,
        source: "debug_dashboard".to_string(),
        signal_count: 0,
        created_at: now.clone(),
        updated_at: now.clone(),
        active: true,
    };

    repo.upsert(&rule)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(ProceduralRuleResponse {
        id: rule.id,
        domain: rule.domain,
        rule_text: rule.rule_text,
        confidence: rule.confidence,
        source: rule.source,
        signal_count: rule.signal_count,
        active: rule.active,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    })
}

#[tauri::command]
pub async fn cognitive_rule_toggle(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    repo.deactivate(&id)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(true)
}

#[tauri::command]
pub async fn cognitive_run_compaction(
    state: State<'_, Arc<AppCore>>,
) -> Result<CompactionResultResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());

    let archived = fact_repo
        .archive_superseded(90)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    let deleted_episodic = episodic_repo
        .delete_old(90, 2)
        .await
        .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

    Ok(CompactionResultResponse {
        archived_count: archived,
        deleted_episodic,
    })
}

// ── Coaching Mutations ──────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_reset_dismissals(
    state: State<'_, Arc<AppCore>>,
    trigger_name: Option<String>,
) -> Result<bool, ApiError> {
    let mut router = state.intervention_router()?.lock().await;

    if let Some(name) = trigger_name {
        router.reset_dismissals(&name);
    }
    // TODO: reset all dismissals if no trigger_name provided

    Ok(true)
}

#[tauri::command]
pub async fn coaching_clear_signals(
    state: State<'_, Arc<AppCore>>,
) -> Result<bool, ApiError> {
    // Replace the accumulator with a fresh one
    let mut acc = state.signal_accumulator()?.lock().await;
    *acc = feature_coaching::SignalAccumulator::new();
    Ok(true)
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop`

**Step 3: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs
git commit -m "feat(desktop): cognitive mutation commands (facts, rules, compaction, coaching)"
```

---

### Task 7: Register commands in main.rs

**Files:**
- Modify: `crates/desktop/src/main.rs`

**Step 1: Add cognitive commands to invoke_handler**

In `crates/desktop/src/main.rs`, inside the `tauri::generate_handler![]` macro (after the `// Status` section, around line 237), add:

```rust
            // Cognitive Debug
            commands::cognitive::cognitive_user_model,
            commands::cognitive::cognitive_facts_list,
            commands::cognitive::cognitive_episodic_list,
            commands::cognitive::cognitive_rules_list,
            commands::cognitive::cognitive_memory_stats,
            commands::cognitive::coaching_situation,
            commands::cognitive::coaching_signals,
            commands::cognitive::coaching_patterns,
            commands::cognitive::coaching_feedback_stats,
            commands::cognitive::coaching_router_status,
            commands::cognitive::cognitive_system_status,
            commands::cognitive::cognitive_fact_create,
            commands::cognitive::cognitive_fact_update,
            commands::cognitive::cognitive_fact_delete,
            commands::cognitive::cognitive_rule_create,
            commands::cognitive::cognitive_rule_toggle,
            commands::cognitive::cognitive_run_compaction,
            commands::cognitive::coaching_reset_dismissals,
            commands::cognitive::coaching_clear_signals,
```

**Step 2: Verify full build**

Run: `cargo build -p desktop`

**Step 3: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): register cognitive debug commands in Tauri handler"
```

---

## Phase 4: Dev-API Mirror Routes

### Task 8: Add cognitive routes to dev-api dispatch

**Files:**
- Modify: `crates/dev-api/src/main.rs`

**Step 1: Add cognitive routes to the dispatch function**

In the `dispatch()` function's `match cmd.as_str()` block, add a new section for cognitive debug commands:

```rust
        // ── Cognitive Debug ─────────────────────────────────────
        "cognitive_user_model" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let model = cognitive::repos::load_user_model(&fact_repo).await;
            let resp = UserModelSummaryResponse {
                identity_count: model.identity.len(),
                energy_count: model.energy.len(),
                work_count: model.work.len(),
                finance_count: model.finance.len(),
                learning_count: model.learning.len(),
                preferences_count: model.preferences.len(),
                identity_preview: model.identity.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
                energy_preview: model.energy.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
                work_preview: model.work.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
                finance_preview: model.finance.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
                learning_preview: model.learning.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
                preferences_preview: model.preferences.iter().take(3).map(|f| format!("{} = {}", f.predicate, f.object)).collect(),
            };
            ok(resp)
        }
        "cognitive_facts_list" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let domain: Option<String> = get(&body, "domain");
            let domains = match domain {
                Some(d) => vec![d],
                None => vec!["identity", "energy", "work", "finance", "learning", "preferences"]
                    .into_iter().map(String::from).collect(),
            };
            let mut all = Vec::new();
            for d in &domains {
                match fact_repo.list_active(d).await {
                    Ok(facts) => all.extend(facts),
                    Err(e) => return err(ApiError::new("STORAGE_ERROR", e.to_string())),
                }
            }
            // Convert to response (simplified — skip retrievability in dev-api for now)
            let resp: Vec<serde_json::Value> = all.iter().map(|f| {
                serde_json::json!({
                    "id": f.id, "domain": f.domain, "subject": f.subject,
                    "predicate": f.predicate, "object": f.object,
                    "confidence": f.confidence, "source": f.source,
                    "stability": f.stability, "accessCount": f.access_count,
                    "validFrom": f.valid_from, "retrievability": 0.9,
                    "status": if f.superseded_at.is_some() { "superseded" } else { "active" },
                })
            }).collect();
            ok(resp)
        }
        "cognitive_memory_stats" => {
            ok(serde_json::json!({
                "activeFacts": 0, "archivedFacts": 0,
                "episodicCount": 0, "rulesCount": 0,
                "lastCompaction": null
            }))
        }
        "cognitive_system_status" => {
            ok(serde_json::json!({
                "domainBusSubscribers": 0, "domainBusPublished": 0,
                "backgroundServiceRunning": false, "backgroundEventsProcessed": 0,
                "activeFacts": 0, "episodicCount": 0, "rulesCount": 0,
                "components": []
            }))
        }
        "coaching_situation" => {
            ok(serde_json::json!({
                "energyLevel": 0.5, "focusState": 0.5,
                "deadlinePressure": 0.0, "distractionRisk": 0.0,
                "coachingReceptivity": 0.5, "taskAvoidanceDetected": false,
                "hoursActiveToday": 0.0, "minsSinceBreak": 0.0,
                "hourOfDay": 12, "recentContextSwitches": 0
            }))
        }
        "coaching_signals" => {
            ok(serde_json::json!({ "windowSize": 0, "signals": [], "triggers": [] }))
        }
        "coaching_patterns" => { ok(Vec::<()>::new()) }
        "coaching_feedback_stats" => { ok(Vec::<()>::new()) }
        "coaching_router_status" => {
            ok(serde_json::json!({
                "hourlyCount": 0, "hourlyLimit": 3,
                "dailyCount": 0, "dailyLimit": 10
            }))
        }
        "cognitive_episodic_list" => { ok(Vec::<()>::new()) }
        "cognitive_rules_list" => { ok(Vec::<()>::new()) }
        "cognitive_fact_create" | "cognitive_fact_update" | "cognitive_fact_delete"
        | "cognitive_rule_create" | "cognitive_rule_toggle"
        | "cognitive_run_compaction" | "coaching_reset_dismissals"
        | "coaching_clear_signals" => {
            ok(serde_json::json!({ "ok": true }))
        }
```

Also add `use desktop_shared::cognitive_commands::*;` to the imports at the top if not already covered by the existing `use desktop_shared::commands::*;`.

**Step 2: Add `cognitive` dependency to dev-api's Cargo.toml**

Add `cognitive = { path = "../cognitive" }` to `[dependencies]` in `crates/dev-api/Cargo.toml`.

**Step 3: Verify it compiles**

Run: `cargo build -p dev-api`

**Step 4: Commit**

```bash
git add crates/dev-api/src/main.rs crates/dev-api/Cargo.toml
git commit -m "feat(dev-api): mirror cognitive debug routes in dev API"
```

---

## Phase 5: Frontend — Route & Navigation

### Task 9: Add `/debug` route and sidebar entry

**Files:**
- Modify: `desktop-ui/src/App.tsx`
- Modify: `desktop-ui/src/lib/types.ts`
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx`

**Step 1: Add "Debug" to SidebarItem type**

In `desktop-ui/src/lib/types.ts`, add `"Debug"` to the `SidebarItem` union type:

```typescript
export type SidebarItem =
  | "Chat"
  | "Tasks"
  | "OKR"
  | "Calendar"
  | "Notes"
  | "Finance"
  | "Productivity"
  | "Debug"
  | "Settings";
```

**Step 2: Add Debug entry to Sidebar**

In `desktop-ui/src/components/layout/Sidebar.tsx`:

1. Add import: `import { Activity, Bug, CheckSquare, FileText, MessageSquare, Settings, Wallet } from "lucide-react";`
2. Add entry to `items` array (before Settings, with `bottom: true`):

```typescript
const items: { key: SidebarItem; icon: typeof MessageSquare; path?: string; bottom?: boolean }[] = [
  { key: "Tasks", icon: CheckSquare, path: "/" },
  { key: "Notes", icon: FileText, path: "/notes" },
  { key: "Finance", icon: Wallet, path: "/finance" },
  { key: "Productivity", icon: Activity, path: "/productivity" },
  { key: "Debug", icon: Bug, path: "/debug", bottom: true },
  { key: "Settings", icon: Settings, path: "/settings", bottom: true },
];
```

**Step 3: Add lazy import and route in App.tsx**

In `desktop-ui/src/App.tsx`:

Add lazy import (after the other lazy imports):
```typescript
const DebugDashboard = lazy(() =>
  import("./components/debug/DebugDashboard").then((m) => ({ default: m.DebugDashboard })),
);
```

Add route (inside the `children` array of the AppShell route, before the settings routes):
```typescript
      { path: "/debug", element: <DebugDashboard /> },
```

**Step 4: Verify dev server starts (no runtime errors)**

Run: `cd desktop-ui && bun run dev`
Expected: Vite compiles (the DebugDashboard component doesn't exist yet, but lazy loading means it won't fail until navigated to)

**Step 5: Commit**

```bash
git add desktop-ui/src/App.tsx desktop-ui/src/lib/types.ts desktop-ui/src/components/layout/Sidebar.tsx
git commit -m "feat(ui): add /debug route and sidebar entry"
```

---

## Phase 6: Frontend — Debug Dashboard Shell & Tabs

### Task 10: Create DebugDashboard shell with tab navigation

**Files:**
- Create: `desktop-ui/src/components/debug/DebugDashboard.tsx`

**Step 1: Create the dashboard shell component**

```tsx
// desktop-ui/src/components/debug/DebugDashboard.tsx

import { useState } from "react";
import { Brain, Radio, GitBranch, Cpu, Activity } from "lucide-react";
import { MemoryTab } from "./tabs/MemoryTab";
import { CoachingTab } from "./tabs/CoachingTab";
import { EventsTab } from "./tabs/EventsTab";
import { PipelineTab } from "./tabs/PipelineTab";
import { SystemTab } from "./tabs/SystemTab";

type DebugTab = "memory" | "coaching" | "events" | "pipeline" | "system";

const tabs: { id: DebugTab; label: string; icon: typeof Brain }[] = [
  { id: "memory", label: "Memory", icon: Brain },
  { id: "coaching", label: "Coaching", icon: Activity },
  { id: "events", label: "Events", icon: Radio },
  { id: "pipeline", label: "Pipeline", icon: GitBranch },
  { id: "system", label: "System", icon: Cpu },
];

export function DebugDashboard() {
  const [activeTab, setActiveTab] = useState<DebugTab>("memory");

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 px-6 py-4 border-b border-border">
        <h1 className="text-lg font-medium text-primary">Cognitive Debug</h1>
        <span className="text-xs text-muted bg-white/[0.06] px-2 py-0.5 rounded">Developer</span>
      </div>

      {/* Tab Bar */}
      <div className="flex gap-1 px-6 pt-3 pb-0">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[13px] transition-all ${
                isActive
                  ? "bg-white/[0.08] text-primary font-medium"
                  : "text-muted hover:text-secondary hover:bg-white/[0.04]"
              }`}
            >
              <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        {activeTab === "memory" && <MemoryTab />}
        {activeTab === "coaching" && <CoachingTab />}
        {activeTab === "events" && <EventsTab />}
        {activeTab === "pipeline" && <PipelineTab />}
        {activeTab === "system" && <SystemTab />}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/DebugDashboard.tsx
git commit -m "feat(ui): debug dashboard shell with tab navigation"
```

---

### Task 11: Memory Tab

**Files:**
- Create: `desktop-ui/src/components/debug/tabs/MemoryTab.tsx`

**Step 1: Create MemoryTab component**

```tsx
// desktop-ui/src/components/debug/tabs/MemoryTab.tsx

import { useState } from "react";
import { useQuery } from "../../../hooks/useQuery";
import { useMutation } from "../../../hooks/useMutation";
import { invalidateQueries } from "../../../hooks/useQuery";
import { Play, Plus, Trash2 } from "lucide-react";

interface UserModelSummary {
  identityCount: number;
  energyCount: number;
  workCount: number;
  financeCount: number;
  learningCount: number;
  preferencesCount: number;
  identityPreview: string[];
  energyPreview: string[];
  workPreview: string[];
  financePreview: string[];
  learningPreview: string[];
  preferencesPreview: string[];
}

interface SemanticFact {
  id: string;
  domain: string;
  subject: string;
  predicate: string;
  object: string;
  confidence: number;
  source: string;
  validFrom: string;
  validUntil: string | null;
  stability: number;
  retrievability: number;
  lastAccessed: string | null;
  accessCount: number;
  status: string;
}

interface EpisodicMemory {
  id: string;
  domain: string;
  content: string;
  summary: string | null;
  importance: number;
  occurredAt: string;
  recordedAt: string;
  stability: number;
  accessCount: number;
}

interface ProceduralRule {
  id: string;
  domain: string;
  ruleText: string;
  confidence: number;
  source: string;
  signalCount: number;
  active: boolean;
  createdAt: string;
  updatedAt: string;
}

interface MemoryStats {
  activeFacts: number;
  archivedFacts: number;
  episodicCount: number;
  rulesCount: number;
  lastCompaction: string | null;
}

interface CompactionResult {
  archivedCount: number;
  deletedEpisodic: number;
}

const DOMAINS = ["identity", "energy", "work", "finance", "learning", "preferences"] as const;

const domainColors: Record<string, string> = {
  identity: "bg-purple-500/20 text-purple-300",
  energy: "bg-yellow-500/20 text-yellow-300",
  work: "bg-blue-500/20 text-blue-300",
  finance: "bg-green-500/20 text-green-300",
  learning: "bg-orange-500/20 text-orange-300",
  preferences: "bg-pink-500/20 text-pink-300",
};

export function MemoryTab() {
  const [domainFilter, setDomainFilter] = useState<string | null>(null);

  const { data: model, refetch: refetchModel } = useQuery<UserModelSummary>(
    "cognitive_user_model",
    undefined,
    {
      identityCount: 0, energyCount: 0, workCount: 0,
      financeCount: 0, learningCount: 0, preferencesCount: 0,
      identityPreview: [], energyPreview: [], workPreview: [],
      financePreview: [], learningPreview: [], preferencesPreview: [],
    },
  );

  const { data: facts, refetch: refetchFacts } = useQuery<SemanticFact[]>(
    "cognitive_facts_list",
    domainFilter ? { domain: domainFilter } : {},
    [],
  );

  const { data: episodic } = useQuery<EpisodicMemory[]>(
    "cognitive_episodic_list",
    domainFilter ? { domain: domainFilter, limit: 20 } : { limit: 20 },
    [],
  );

  const { data: rules } = useQuery<ProceduralRule[]>(
    "cognitive_rules_list",
    domainFilter ? { domain: domainFilter } : {},
    [],
  );

  const { data: stats } = useQuery<MemoryStats>(
    "cognitive_memory_stats",
    undefined,
    { activeFacts: 0, archivedFacts: 0, episodicCount: 0, rulesCount: 0, lastCompaction: null },
  );

  const { mutate: runCompaction, loading: compacting } = useMutation<CompactionResult>("cognitive_run_compaction");
  const { mutate: deleteFact } = useMutation<boolean>("cognitive_fact_delete");

  const handleCompact = async () => {
    await runCompaction({} as never);
    invalidateQueries("cognitive_");
    refetchModel();
    refetchFacts();
  };

  const handleDeleteFact = async (id: string) => {
    await deleteFact({ id } as never);
    invalidateQueries("cognitive_");
    refetchFacts();
  };

  const domainCards = DOMAINS.map((d) => {
    const count = model[`${d}Count` as keyof UserModelSummary] as number;
    const preview = model[`${d}Preview` as keyof UserModelSummary] as string[];
    return { domain: d, count, preview };
  });

  return (
    <div className="space-y-6">
      {/* UserModel Summary Cards */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">User Model</h2>
        <div className="grid grid-cols-3 gap-3">
          {domainCards.map(({ domain, count, preview }) => (
            <button
              key={domain}
              type="button"
              onClick={() => setDomainFilter(domainFilter === domain ? null : domain)}
              className={`text-left p-3 rounded-lg border transition-all ${
                domainFilter === domain
                  ? "border-brand/50 bg-brand/10"
                  : "border-white/[0.08] bg-white/[0.04] hover:bg-white/[0.06]"
              }`}
            >
              <div className="flex items-center justify-between mb-1">
                <span className={`text-[11px] px-1.5 py-0.5 rounded ${domainColors[domain]}`}>
                  {domain}
                </span>
                <span className="text-[11px] text-muted">{count} facts</span>
              </div>
              <div className="space-y-0.5 mt-2">
                {preview.slice(0, 2).map((p, i) => (
                  <p key={i} className="text-[11px] text-muted truncate">{p}</p>
                ))}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Semantic Facts Table */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-[13px] font-medium text-secondary">
            Semantic Facts {domainFilter && <span className="text-muted">({domainFilter})</span>}
          </h2>
          <button
            type="button"
            className="flex items-center gap-1 text-[11px] text-muted hover:text-secondary"
          >
            <Plus className="w-3 h-3" /> Add Fact
          </button>
        </div>
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-white/[0.06]">
                <th className="text-left p-2 text-muted font-normal">Domain</th>
                <th className="text-left p-2 text-muted font-normal">Subject</th>
                <th className="text-left p-2 text-muted font-normal">Predicate</th>
                <th className="text-left p-2 text-muted font-normal">Object</th>
                <th className="text-left p-2 text-muted font-normal">Conf</th>
                <th className="text-left p-2 text-muted font-normal">Stab</th>
                <th className="text-left p-2 text-muted font-normal">Retr</th>
                <th className="text-left p-2 text-muted font-normal">Accessed</th>
                <th className="text-left p-2 text-muted font-normal" />
              </tr>
            </thead>
            <tbody>
              {facts.map((f) => (
                <tr
                  key={f.id}
                  className={`border-b border-white/[0.04] hover:bg-white/[0.02] ${
                    f.retrievability < 0.3 ? "opacity-40" : ""
                  }`}
                >
                  <td className="p-2">
                    <span className={`text-[10px] px-1 py-0.5 rounded ${domainColors[f.domain] ?? "text-muted"}`}>
                      {f.domain}
                    </span>
                  </td>
                  <td className="p-2 text-secondary">{f.subject}</td>
                  <td className="p-2 text-secondary">{f.predicate}</td>
                  <td className="p-2 text-primary">{f.object}</td>
                  <td className="p-2">
                    <div className="w-12 bg-white/[0.1] rounded-full h-1.5">
                      <div
                        className="bg-brand h-1.5 rounded-full"
                        style={{ width: `${f.confidence * 100}%` }}
                      />
                    </div>
                  </td>
                  <td className="p-2 text-muted">{f.stability.toFixed(1)}</td>
                  <td className="p-2 text-muted">{(f.retrievability * 100).toFixed(0)}%</td>
                  <td className="p-2 text-muted">{f.accessCount}x</td>
                  <td className="p-2">
                    <button
                      type="button"
                      onClick={() => handleDeleteFact(f.id)}
                      className="text-muted hover:text-red-400"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </td>
                </tr>
              ))}
              {facts.length === 0 && (
                <tr>
                  <td colSpan={9} className="p-4 text-center text-muted">
                    No facts found
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Episodic Memories */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Episodic Memories</h2>
        <div className="space-y-2">
          {episodic.map((m) => (
            <div
              key={m.id}
              className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
            >
              <div className="flex items-center gap-2 mb-1">
                <span className={`text-[10px] px-1 py-0.5 rounded ${domainColors[m.domain] ?? "text-muted"}`}>
                  {m.domain}
                </span>
                <span className="text-[10px] text-muted">{m.occurredAt}</span>
                <span className="text-[10px] text-muted">imp: {m.importance.toFixed(2)}</span>
              </div>
              <p className="text-[12px] text-secondary">{m.content}</p>
              {m.summary && (
                <p className="text-[11px] text-muted mt-1">{m.summary}</p>
              )}
            </div>
          ))}
          {episodic.length === 0 && (
            <p className="text-[12px] text-muted">No episodic memories</p>
          )}
        </div>
      </div>

      {/* Procedural Rules */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Procedural Rules</h2>
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-white/[0.06]">
                <th className="text-left p-2 text-muted font-normal">Domain</th>
                <th className="text-left p-2 text-muted font-normal">Rule</th>
                <th className="text-left p-2 text-muted font-normal">Conf</th>
                <th className="text-left p-2 text-muted font-normal">Signals</th>
                <th className="text-left p-2 text-muted font-normal">Active</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((r) => (
                <tr key={r.id} className="border-b border-white/[0.04]">
                  <td className="p-2">
                    <span className={`text-[10px] px-1 py-0.5 rounded ${domainColors[r.domain] ?? "text-muted"}`}>
                      {r.domain}
                    </span>
                  </td>
                  <td className="p-2 text-secondary">{r.ruleText}</td>
                  <td className="p-2 text-muted">{r.confidence.toFixed(2)}</td>
                  <td className="p-2 text-muted">{r.signalCount}</td>
                  <td className="p-2">
                    <span className={`text-[10px] ${r.active ? "text-green-400" : "text-red-400"}`}>
                      {r.active ? "ON" : "OFF"}
                    </span>
                  </td>
                </tr>
              ))}
              {rules.length === 0 && (
                <tr>
                  <td colSpan={5} className="p-4 text-center text-muted">No rules</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="flex items-center gap-4 p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
        <span className="text-[11px] text-muted">
          Active: <span className="text-secondary">{stats.activeFacts}</span>
        </span>
        <span className="text-[11px] text-muted">
          Archived: <span className="text-secondary">{stats.archivedFacts}</span>
        </span>
        <span className="text-[11px] text-muted">
          Episodic: <span className="text-secondary">{stats.episodicCount}</span>
        </span>
        <span className="text-[11px] text-muted">
          Rules: <span className="text-secondary">{stats.rulesCount}</span>
        </span>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleCompact}
          disabled={compacting}
          className="flex items-center gap-1 text-[11px] text-brand hover:text-brand/80 disabled:opacity-50"
        >
          <Play className="w-3 h-3" />
          {compacting ? "Running..." : "Run Compaction"}
        </button>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/tabs/MemoryTab.tsx
git commit -m "feat(ui): Memory tab with facts table, episodic list, rules, compaction"
```

---

### Task 12: Coaching Tab

**Files:**
- Create: `desktop-ui/src/components/debug/tabs/CoachingTab.tsx`

**Step 1: Create CoachingTab component**

```tsx
// desktop-ui/src/components/debug/tabs/CoachingTab.tsx

import { useQuery } from "../../../hooks/useQuery";
import { useMutation } from "../../../hooks/useMutation";
import { invalidateQueries } from "../../../hooks/useQuery";
import { RefreshCw, Trash2 } from "lucide-react";

interface UserSituation {
  energyLevel: number;
  focusState: number;
  deadlinePressure: number;
  distractionRisk: number;
  coachingReceptivity: number;
  taskAvoidanceDetected: boolean;
  hoursActiveToday: number;
  minsSinceBreak: number;
  hourOfDay: number;
  recentContextSwitches: number;
}

interface SignalWindow {
  windowSize: number;
  signals: { eventType: string; timestamp: string; metadata: string }[];
  triggers: { name: string; cooldownRemainingSecs: number; lastFired: string | null }[];
}

interface DetectedPattern {
  name: string;
  confidence: number;
  signalCount: number;
  description: string;
  domain: string;
}

interface StrategyFeedback {
  triggerName: string;
  interventionType: string;
  timesDelivered: number;
  acceptanceRate: number;
  effectiveness: number;
  behavioralPositive: number;
  behavioralNegative: number;
}

interface RouterStatus {
  hourlyCount: number;
  hourlyLimit: number;
  dailyCount: number;
  dailyLimit: number;
}

function Gauge({ label, value, color = "bg-brand" }: { label: string; value: number; color?: string }) {
  const pct = Math.round(value * 100);
  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative w-16 h-16">
        <svg className="w-16 h-16 -rotate-90" viewBox="0 0 36 36">
          <circle
            className="text-white/[0.08]"
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
          />
          <circle
            className={color.replace("bg-", "text-")}
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
            strokeDasharray={`${pct} 100`}
            strokeLinecap="round"
          />
        </svg>
        <span className="absolute inset-0 flex items-center justify-center text-[11px] text-secondary font-mono">
          {pct}%
        </span>
      </div>
      <span className="text-[10px] text-muted text-center leading-tight">{label}</span>
    </div>
  );
}

export function CoachingTab() {
  const { data: situation } = useQuery<UserSituation>(
    "coaching_situation",
    undefined,
    {
      energyLevel: 0, focusState: 0, deadlinePressure: 0,
      distractionRisk: 0, coachingReceptivity: 0, taskAvoidanceDetected: false,
      hoursActiveToday: 0, minsSinceBreak: 0, hourOfDay: 0, recentContextSwitches: 0,
    },
  );

  const { data: signals } = useQuery<SignalWindow>(
    "coaching_signals",
    undefined,
    { windowSize: 0, signals: [], triggers: [] },
  );

  const { data: patterns } = useQuery<DetectedPattern[]>("coaching_patterns", undefined, []);

  const { data: feedback } = useQuery<StrategyFeedback[]>("coaching_feedback_stats", undefined, []);

  const { data: router } = useQuery<RouterStatus>(
    "coaching_router_status",
    undefined,
    { hourlyCount: 0, hourlyLimit: 3, dailyCount: 0, dailyLimit: 10 },
  );

  const { mutate: clearSignals } = useMutation("coaching_clear_signals");
  const { mutate: resetDismissals } = useMutation("coaching_reset_dismissals");

  const handleClearSignals = async () => {
    await clearSignals({} as never);
    invalidateQueries("coaching_");
  };

  const handleResetDismissals = async (trigger?: string) => {
    await resetDismissals((trigger ? { triggerName: trigger } : {}) as never);
    invalidateQueries("coaching_");
  };

  return (
    <div className="space-y-6">
      {/* Situation Gauges */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">User Situation</h2>
        <div className="flex gap-4 items-start p-4 bg-white/[0.04] rounded-lg border border-white/[0.08]">
          <Gauge label="Energy" value={situation.energyLevel} />
          <Gauge label="Focus" value={situation.focusState} />
          <Gauge label="Deadline" value={situation.deadlinePressure} color="bg-red-500" />
          <Gauge label="Distraction" value={situation.distractionRisk} color="bg-orange-500" />
          <Gauge label="Receptivity" value={situation.coachingReceptivity} color="bg-green-500" />
          <div className="flex flex-col gap-1 ml-4 text-[11px]">
            <span className="text-muted">
              Hours active: <span className="text-secondary">{situation.hoursActiveToday.toFixed(1)}h</span>
            </span>
            <span className="text-muted">
              Since break: <span className="text-secondary">{situation.minsSinceBreak.toFixed(0)}min</span>
            </span>
            <span className="text-muted">
              Context switches: <span className="text-secondary">{situation.recentContextSwitches}</span>
            </span>
            {situation.taskAvoidanceDetected && (
              <span className="text-orange-400 font-medium">Task avoidance detected</span>
            )}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        {/* Left: Signals & Patterns */}
        <div className="space-y-4">
          <div>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-[13px] font-medium text-secondary">Signal Accumulator</h3>
              <button
                type="button"
                onClick={handleClearSignals}
                className="text-[10px] text-muted hover:text-secondary flex items-center gap-1"
              >
                <Trash2 className="w-3 h-3" /> Clear
              </button>
            </div>
            <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
              <p className="text-[12px] text-muted">
                {signals.windowSize} signals in 30min window
              </p>
              {signals.triggers.length > 0 && (
                <div className="mt-2 space-y-1">
                  {signals.triggers.map((t) => (
                    <div key={t.name} className="flex items-center justify-between text-[11px]">
                      <span className="text-secondary">{t.name}</span>
                      <span className="text-muted">
                        {t.cooldownRemainingSecs > 0 ? `${t.cooldownRemainingSecs}s cooldown` : "ready"}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div>
            <h3 className="text-[13px] font-medium text-secondary mb-2">Detected Patterns</h3>
            <div className="space-y-2">
              {patterns.map((p) => (
                <div key={p.name} className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-[12px] text-secondary font-medium">{p.name}</span>
                    <span className="text-[10px] text-muted">{p.signalCount} signals</span>
                  </div>
                  <div className="w-full bg-white/[0.1] rounded-full h-1 mb-1">
                    <div className="bg-brand h-1 rounded-full" style={{ width: `${p.confidence * 100}%` }} />
                  </div>
                  <p className="text-[11px] text-muted">{p.description}</p>
                </div>
              ))}
              {patterns.length === 0 && <p className="text-[12px] text-muted">No patterns detected</p>}
            </div>
          </div>
        </div>

        {/* Right: Router & Feedback */}
        <div className="space-y-4">
          <div>
            <h3 className="text-[13px] font-medium text-secondary mb-2">Intervention Router</h3>
            <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
              <div className="flex gap-4 text-[12px]">
                <span className="text-muted">
                  Hourly: <span className="text-secondary">{router.hourlyCount}/{router.hourlyLimit}</span>
                </span>
                <span className="text-muted">
                  Daily: <span className="text-secondary">{router.dailyCount}/{router.dailyLimit}</span>
                </span>
              </div>
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-[13px] font-medium text-secondary">Strategy Feedback</h3>
              <button
                type="button"
                onClick={() => handleResetDismissals()}
                className="text-[10px] text-muted hover:text-secondary flex items-center gap-1"
              >
                <RefreshCw className="w-3 h-3" /> Reset All
              </button>
            </div>
            <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
              <table className="w-full text-[12px]">
                <thead>
                  <tr className="border-b border-white/[0.06]">
                    <th className="text-left p-2 text-muted font-normal">Trigger</th>
                    <th className="text-left p-2 text-muted font-normal">Type</th>
                    <th className="text-left p-2 text-muted font-normal">Used</th>
                    <th className="text-left p-2 text-muted font-normal">Accept</th>
                    <th className="text-left p-2 text-muted font-normal">Effect</th>
                  </tr>
                </thead>
                <tbody>
                  {feedback.map((s) => (
                    <tr key={s.triggerName} className="border-b border-white/[0.04]">
                      <td className="p-2 text-secondary">{s.triggerName}</td>
                      <td className="p-2 text-muted">{s.interventionType}</td>
                      <td className="p-2 text-muted">{s.timesDelivered}</td>
                      <td className="p-2 text-muted">{(s.acceptanceRate * 100).toFixed(0)}%</td>
                      <td className="p-2 text-muted">{(s.effectiveness * 100).toFixed(0)}%</td>
                    </tr>
                  ))}
                  {feedback.length === 0 && (
                    <tr>
                      <td colSpan={5} className="p-4 text-center text-muted">No feedback data</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/tabs/CoachingTab.tsx
git commit -m "feat(ui): Coaching tab with situation gauges, signals, patterns, feedback"
```

---

### Task 13: Events Tab (live stream)

**Files:**
- Create: `desktop-ui/src/components/debug/tabs/EventsTab.tsx`

**Step 1: Create EventsTab with live event stream**

```tsx
// desktop-ui/src/components/debug/tabs/EventsTab.tsx

import { useCallback, useRef, useState } from "react";
import { useEvent } from "../../../hooks/useEvent";
import { Pause, Play, Trash2 } from "lucide-react";

interface DomainEventPayload {
  eventType: string;
  salience: string;
  domain: string;
  timestamp: string;
  payload: unknown;
}

const salienceColors: Record<string, string> = {
  extract: "bg-green-500/20 text-green-300 border-l-green-500",
  accumulate: "bg-yellow-500/20 text-yellow-300 border-l-yellow-500",
  discard: "bg-white/[0.06] text-muted border-l-white/20",
};

const MAX_EVENTS = 200;

export function EventsTab() {
  const [events, setEvents] = useState<DomainEventPayload[]>([]);
  const [paused, setPaused] = useState(false);
  const [filters, setFilters] = useState({ extract: true, accumulate: true, discard: false });
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  const filtersRef = useRef(filters);
  filtersRef.current = filters;

  useEvent<DomainEventPayload>("cognitive:domain_event", useCallback((event: DomainEventPayload) => {
    if (pausedRef.current) return;
    if (!filtersRef.current[event.salience as keyof typeof filters]) return;
    setEvents((prev) => [event, ...prev].slice(0, MAX_EVENTS));
  }, []));

  const visibleEvents = events.filter(
    (e) => filters[e.salience as keyof typeof filters],
  );

  return (
    <div className="space-y-4">
      {/* Filter Bar */}
      <div className="flex items-center gap-3">
        {(["extract", "accumulate", "discard"] as const).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setFilters((f) => ({ ...f, [s]: !f[s] }))}
            className={`text-[11px] px-2 py-1 rounded transition-all ${
              filters[s]
                ? salienceColors[s]
                : "bg-white/[0.04] text-muted"
            }`}
          >
            {s.charAt(0).toUpperCase() + s.slice(1)}
          </button>
        ))}
        <div className="flex-1" />
        <button
          type="button"
          onClick={() => setPaused(!paused)}
          className="flex items-center gap-1 text-[11px] text-muted hover:text-secondary"
        >
          {paused ? <Play className="w-3 h-3" /> : <Pause className="w-3 h-3" />}
          {paused ? "Resume" : "Pause"}
        </button>
        <button
          type="button"
          onClick={() => setEvents([])}
          className="flex items-center gap-1 text-[11px] text-muted hover:text-secondary"
        >
          <Trash2 className="w-3 h-3" /> Clear
        </button>
        <span className="text-[11px] text-muted">{events.length} events</span>
      </div>

      {/* Event Stream */}
      <div className="space-y-1">
        {visibleEvents.map((e, i) => {
          const color = salienceColors[e.salience] ?? salienceColors.discard;
          const isExpanded = expandedId === i;
          return (
            <button
              key={i}
              type="button"
              onClick={() => setExpandedId(isExpanded ? null : i)}
              className={`w-full text-left p-2 rounded border-l-2 transition-all ${color} ${
                e.salience === "extract" ? "border-l-green-500" : ""
              }`}
            >
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted font-mono w-20">{e.timestamp}</span>
                <span className="text-[11px] text-secondary">{e.eventType}</span>
                <span className={`text-[9px] px-1 py-0.5 rounded ${color}`}>{e.salience}</span>
                <span className="text-[10px] text-muted">{e.domain}</span>
              </div>
              {isExpanded && (
                <pre className="mt-2 text-[10px] text-muted font-mono whitespace-pre-wrap">
                  {JSON.stringify(e.payload, null, 2)}
                </pre>
              )}
            </button>
          );
        })}
        {visibleEvents.length === 0 && (
          <p className="text-[12px] text-muted text-center py-8">
            {paused ? "Stream paused" : "Waiting for domain events..."}
          </p>
        )}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/tabs/EventsTab.tsx
git commit -m "feat(ui): Events tab with live domain event stream"
```

---

### Task 14: Pipeline Tab

**Files:**
- Create: `desktop-ui/src/components/debug/tabs/PipelineTab.tsx`

**Step 1: Create PipelineTab component**

```tsx
// desktop-ui/src/components/debug/tabs/PipelineTab.tsx

import { useCallback, useState } from "react";
import { useEvent } from "../../../hooks/useEvent";
import { GitBranch } from "lucide-react";

interface ExtractionEvent {
  observation: string;
  factsExtracted: number;
}

interface ConsolidationEvent {
  operation: string;
  fact: string;
}

export function PipelineTab() {
  const [extractions, setExtractions] = useState<(ExtractionEvent & { ts: string })[]>([]);
  const [consolidations, setConsolidations] = useState<(ConsolidationEvent & { ts: string })[]>([]);

  useEvent<ExtractionEvent>("cognitive:extraction", useCallback((e: ExtractionEvent) => {
    setExtractions((prev) => [{ ...e, ts: new Date().toISOString() }, ...prev].slice(0, 50));
  }, []));

  useEvent<ConsolidationEvent>("cognitive:consolidation", useCallback((e: ConsolidationEvent) => {
    setConsolidations((prev) => [{ ...e, ts: new Date().toISOString() }, ...prev].slice(0, 50));
  }, []));

  const opColors: Record<string, string> = {
    ADD: "bg-green-500/20 text-green-300",
    UPDATE: "bg-blue-500/20 text-blue-300",
    DELETE: "bg-red-500/20 text-red-300",
    NOOP: "bg-white/[0.06] text-muted",
  };

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-6">
        {/* Extraction Log */}
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3 flex items-center gap-1.5">
            <GitBranch className="w-3.5 h-3.5" /> Extraction Log
          </h2>
          <div className="space-y-2">
            {extractions.map((e, i) => (
              <div
                key={i}
                className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">{e.ts}</span>
                  <span className="text-[10px] bg-brand/20 text-brand px-1 py-0.5 rounded">
                    {e.factsExtracted} facts
                  </span>
                </div>
                <p className="text-[11px] text-secondary">{e.observation}</p>
              </div>
            ))}
            {extractions.length === 0 && (
              <p className="text-[12px] text-muted text-center py-4">
                Waiting for extraction events...
              </p>
            )}
          </div>
        </div>

        {/* Consolidation Log */}
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3">Consolidation Log</h2>
          <div className="space-y-2">
            {consolidations.map((c, i) => (
              <div
                key={i}
                className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">{c.ts}</span>
                  <span className={`text-[10px] px-1 py-0.5 rounded ${opColors[c.operation] ?? opColors.NOOP}`}>
                    {c.operation}
                  </span>
                </div>
                <p className="text-[11px] text-secondary">{c.fact}</p>
              </div>
            ))}
            {consolidations.length === 0 && (
              <p className="text-[12px] text-muted text-center py-4">
                Waiting for consolidation events...
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Pipeline stats summary */}
      <div className="flex items-center gap-4 p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
        <span className="text-[11px] text-muted">
          Extractions: <span className="text-secondary">{extractions.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          Consolidations: <span className="text-secondary">{consolidations.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          ADDs: <span className="text-green-400">
            {consolidations.filter((c) => c.operation === "ADD").length}
          </span>
        </span>
        <span className="text-[11px] text-muted">
          UPDATEs: <span className="text-blue-400">
            {consolidations.filter((c) => c.operation === "UPDATE").length}
          </span>
        </span>
        <span className="text-[11px] text-muted">
          DELETEs: <span className="text-red-400">
            {consolidations.filter((c) => c.operation === "DELETE").length}
          </span>
        </span>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/tabs/PipelineTab.tsx
git commit -m "feat(ui): Pipeline tab with extraction and consolidation logs"
```

---

### Task 15: System Tab

**Files:**
- Create: `desktop-ui/src/components/debug/tabs/SystemTab.tsx`

**Step 1: Create SystemTab component**

```tsx
// desktop-ui/src/components/debug/tabs/SystemTab.tsx

import { useQuery } from "../../../hooks/useQuery";
import { CheckCircle2, Circle, AlertCircle } from "lucide-react";

interface ComponentStatus {
  name: string;
  status: string;
  handlerType: string;
  notes: string;
}

interface SystemStatus {
  domainBusSubscribers: number;
  domainBusPublished: number;
  backgroundServiceRunning: boolean;
  backgroundEventsProcessed: number;
  activeFacts: number;
  episodicCount: number;
  rulesCount: number;
  components: ComponentStatus[];
}

const statusConfig: Record<string, { icon: typeof CheckCircle2; color: string; bg: string }> = {
  wired: { icon: CheckCircle2, color: "text-green-400", bg: "bg-green-500/20" },
  built: { icon: AlertCircle, color: "text-yellow-400", bg: "bg-yellow-500/20" },
  stub: { icon: Circle, color: "text-muted", bg: "bg-white/[0.06]" },
};

export function SystemTab() {
  const { data: system } = useQuery<SystemStatus>(
    "cognitive_system_status",
    undefined,
    {
      domainBusSubscribers: 0,
      domainBusPublished: 0,
      backgroundServiceRunning: false,
      backgroundEventsProcessed: 0,
      activeFacts: 0,
      episodicCount: 0,
      rulesCount: 0,
      components: [],
    },
  );

  const wiredCount = system.components.filter((c) => c.status === "wired").length;
  const builtCount = system.components.filter((c) => c.status === "built").length;
  const stubCount = system.components.filter((c) => c.status === "stub").length;

  return (
    <div className="space-y-6">
      {/* Service Health Cards */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Service Health</h2>
        <div className="grid grid-cols-4 gap-3">
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Domain Event Bus</h3>
            <p className="text-[13px] text-secondary">{system.domainBusSubscribers} subscribers</p>
            <p className="text-[11px] text-muted">{system.domainBusPublished} published</p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Background Consolidation</h3>
            <p className="text-[13px] text-secondary">
              {system.backgroundServiceRunning ? (
                <span className="text-green-400">Running</span>
              ) : (
                <span className="text-red-400">Stopped</span>
              )}
            </p>
            <p className="text-[11px] text-muted">{system.backgroundEventsProcessed} processed</p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Memory System</h3>
            <p className="text-[13px] text-secondary">{system.activeFacts} active facts</p>
            <p className="text-[11px] text-muted">
              {system.episodicCount} episodic / {system.rulesCount} rules
            </p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Implementation</h3>
            <p className="text-[13px] text-secondary">
              <span className="text-green-400">{wiredCount}</span> /{" "}
              <span className="text-yellow-400">{builtCount}</span> /{" "}
              <span className="text-muted">{stubCount}</span>
            </p>
            <p className="text-[11px] text-muted">wired / built / stub</p>
          </div>
        </div>
      </div>

      {/* Implementation Completeness Matrix */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Implementation Completeness</h2>
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-white/[0.06]">
                <th className="text-left p-2 text-muted font-normal">Component</th>
                <th className="text-left p-2 text-muted font-normal">Status</th>
                <th className="text-left p-2 text-muted font-normal">Handler</th>
                <th className="text-left p-2 text-muted font-normal">Notes</th>
              </tr>
            </thead>
            <tbody>
              {system.components.map((c) => {
                const cfg = statusConfig[c.status] ?? statusConfig.stub;
                const Icon = cfg.icon;
                return (
                  <tr key={c.name} className="border-b border-white/[0.04]">
                    <td className="p-2 text-secondary">{c.name}</td>
                    <td className="p-2">
                      <span className={`inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded ${cfg.bg} ${cfg.color}`}>
                        <Icon className="w-3 h-3" />
                        {c.status}
                      </span>
                    </td>
                    <td className="p-2">
                      <span className="text-[10px] text-muted bg-white/[0.06] px-1.5 py-0.5 rounded">
                        {c.handlerType}
                      </span>
                    </td>
                    <td className="p-2 text-muted">{c.notes}</td>
                  </tr>
                );
              })}
              {system.components.length === 0 && (
                <tr>
                  <td colSpan={4} className="p-4 text-center text-muted">No component data</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Legend */}
      <div className="flex gap-4 text-[11px]">
        <span className="flex items-center gap-1 text-green-400">
          <CheckCircle2 className="w-3 h-3" /> Wired — fully integrated and running
        </span>
        <span className="flex items-center gap-1 text-yellow-400">
          <AlertCircle className="w-3 h-3" /> Built — code exists but not connected
        </span>
        <span className="flex items-center gap-1 text-muted">
          <Circle className="w-3 h-3" /> Stub — trait defined, implementation pending
        </span>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/debug/tabs/SystemTab.tsx
git commit -m "feat(ui): System tab with health cards and completeness matrix"
```

---

## Phase 7: Live Event Forwarding

### Task 16: Forward DomainEvents to Tauri frontend

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

**Step 1: Add a Tauri event forwarder for DomainEvents**

In `AppCore::init()`, after the `BackgroundConsolidationService::start()` call and before the `Ok(AppCore { ... })` return, add a second subscriber to the DomainEventBus that forwards events to the Tauri frontend:

```rust
    // Forward domain events to frontend for debug dashboard
    {
        let mut event_rx = domain_bus.subscribe();
        let app_handle = handle.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let salience = cognitive::salience::evaluate_salience(&event);
                        let domain = match &event {
                            bus::DomainEvent::TaskCompleted { .. }
                            | bus::DomainEvent::TaskCreated { .. }
                            | bus::DomainEvent::TaskDeferred { .. }
                            | bus::DomainEvent::TaskBlocked { .. }
                            | bus::DomainEvent::TaskPriorityChanged { .. } => "work",
                            bus::DomainEvent::FocusSessionEnded { .. }
                            | bus::DomainEvent::ContextSwitch { .. }
                            | bus::DomainEvent::DistractionBlocked { .. } => "energy",
                            bus::DomainEvent::BudgetThresholdReached { .. }
                            | bus::DomainEvent::InvestmentChanged { .. }
                            | bus::DomainEvent::RecurringExpenseDetected { .. } => "finance",
                            bus::DomainEvent::GoalMilestoneReached { .. }
                            | bus::DomainEvent::WeeklyReviewDue { .. } => "learning",
                            bus::DomainEvent::CoachingFeedback { .. } => "coaching",
                        };
                        let salience_str = match salience {
                            cognitive::types::SalienceVerdict::Extract => "extract",
                            cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                            cognitive::types::SalienceVerdict::Discard => "discard",
                        };
                        let payload = desktop_shared::cognitive_commands::DomainEventPayload {
                            event_type: format!("{:?}", event).split('{').next().unwrap_or("Unknown").trim().to_string(),
                            salience: salience_str.to_string(),
                            domain: domain.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            payload: serde_json::to_value(&event).unwrap_or_default(),
                        };
                        let _ = app_handle.emit("cognitive:domain_event", &payload);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("debug event forwarder lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop`

**Step 3: Commit**

```bash
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): forward DomainEvents to frontend for live debug stream"
```

---

## Phase 8: Verification

### Task 17: Full build verification

**Step 1: Build all Rust crates**

Run: `cargo build --workspace`
Expected: no errors

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings (existing desktop exceptions allowed)

**Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: all existing tests pass

**Step 4: Build frontend**

Run: `cd desktop-ui && bun run build`
Expected: builds successfully

**Step 5: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: passes

**Step 6: Manual smoke test**

Run: `cargo run -p dev-api` in one terminal, `cd desktop-ui && bun run dev` in another.
Navigate to `http://localhost:1420/#/debug`.
Expected: Debug dashboard loads with 5 tabs. Memory tab shows empty tables. System tab shows completeness matrix. Coaching tab shows zero gauges.

**Step 7: Final commit**

```bash
git add -A
git commit -m "feat: cognitive debug dashboard — complete implementation"
```

---

## Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1 | 1 | DTOs in `desktop-shared` |
| 2 | 2 | Coaching engine state on AppCore |
| 3 | 3-7 | Tauri commands (read + write + register) |
| 4 | 8 | Dev-API mirror routes |
| 5 | 9 | Route, sidebar, App.tsx |
| 6 | 10-15 | React tabs (Dashboard, Memory, Coaching, Events, Pipeline, System) |
| 7 | 16 | Live event forwarding |
| 8 | 17 | Full build + lint + test verification |

**Total: 17 tasks, ~50 files touched**
