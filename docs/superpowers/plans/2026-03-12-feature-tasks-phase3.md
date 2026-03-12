# Feature-Tasks Phase 3: Proactive Ecosystem Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the learning loop (cognitive integration), proactive suggestion engine, and estimation forecasting system — in that order — so each layer benefits from the data foundation below it.

**Architecture:** Phase 3 builds on Phase 1's schema/types and Phase 2's handler traits. Implementation order is 3.3 → 3.1 → 3.2 (cognitive first). Cognitive Integration wires existing domain events into the observation pipeline for learning. ProactiveHandler generates suggestions from event triggers + periodic scans. ForecastHandler uses pure computation (L4) + LLM risk narratives (L5). All traits are already defined in `feature-tasks` (L4); this plan implements them in `agent` (L5) and wires everything together.

**Tech Stack:** Rust, async-trait, serde_json, tokio, chrono, `providers::DynProvider` (LLM), `bus::DomainEventBus`, `storage::TaskRepo`, `cognitive::types::SemanticFact`

**Spec:** `docs/superpowers/specs/2026-03-11-feature-tasks-phase2-3-design.md` (sections 3.1–3.3)

---

## File Structure

### Files to create:
| File | Responsibility |
|------|---------------|
| `crates/feature-tasks/src/cognitive_bridge.rs` | Pure L4 helpers: extract energy profile, estimation bias, velocity, deferral patterns, agentic success rate from semantic facts |
| `crates/feature-tasks/src/forecast.rs` | Pure L4 computation: similarity matching, deviation correction, velocity calculation, accuracy stats |
| `crates/feature-tasks/src/tool/actions/suggest.rs` | Tool action: trigger proactive suggestion scan |
| `crates/feature-tasks/src/tool/actions/forecast.rs` | Tool action: generate task/project forecasts and accuracy reports |
| `crates/agent/src/handlers/proactive.rs` | `LlmProactiveHandler` — event-driven + periodic suggestion generation |
| `crates/agent/src/handlers/suggestion_applier.rs` | `TaskSuggestionApplier` — executes accepted suggestion actions |
| `crates/agent/src/handlers/forecast.rs` | `LlmForecastHandler` — wraps L4 computation + LLM risk narratives |
| `crates/agent/src/templates/proactive_suggestions.md` | Prompt template for suggestion generation |
| `crates/agent/src/templates/forecast_risk.md` | Prompt template for forecast risk analysis |

### Files to modify:
| File | Changes |
|------|---------|
| `crates/cognitive/src/background.rs` | Add observation mappings for Phase 3 task events (TaskFocusStarted/Ended, EstimationRecorded, TaskDecomposed, DayPlanGenerated, ProactiveSuggestionCreated) with spec-defined importance scoring |
| `crates/cognitive/src/salience.rs` | Upgrade salience for TaskCompleted with large deviation (→ Extract), EstimationRecorded with large deviation (→ Extract), repeated TaskDeferred (→ Extract) |
| `crates/feature-tasks/src/lib.rs` | Export `cognitive_bridge` and `forecast` modules |
| `crates/feature-tasks/src/handlers/mod.rs` | Already exports Phase 3 traits (no changes needed) |
| `crates/feature-tasks/src/tool/mod.rs` | Add `proactive_handler`, `suggestion_applier`, `forecast_handler` fields + builder methods + action routing |
| `crates/feature-tasks/src/tool/actions/mod.rs` | Add `suggest` and `forecast` modules |
| `crates/agent/src/handlers/mod.rs` | Add Phase 3 handler modules + re-exports |
| `crates/agent/src/agent_loop/builder.rs` | Construct and inject Phase 3 handlers into TaskTool |

---

## Chunk 1: Cognitive Integration (3.3)

Wire existing Phase 2 domain events into the cognitive observation pipeline so the system starts learning immediately.

### Task 1: Enhanced salience classification

**Files:**
- Modify: `crates/cognitive/src/salience.rs`
- Test: inline `#[cfg(test)] mod tests`

The spec defines conditional promotion to Extract for certain task events. Currently all task events are Accumulate. We need to upgrade events with high-signal conditions.

- [ ] **Step 1: Write failing tests for upgraded salience**

Add to `crates/cognitive/src/salience.rs` test module:

```rust
#[test]
fn test_task_completed_large_deviation_is_extract() {
    let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(90),
        estimated_duration_mins: Some(30),
        deviation_pct: Some(200.0), // > 50% deviation
    });
    assert_eq!(verdict, SalienceVerdict::Extract);
}

#[test]
fn test_task_completed_small_deviation_is_accumulate() {
    let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(35),
        estimated_duration_mins: Some(30),
        deviation_pct: Some(16.7), // < 50% deviation
    });
    assert_eq!(verdict, SalienceVerdict::Accumulate);
}

#[test]
fn test_estimation_recorded_large_deviation_is_extract() {
    let verdict = evaluate_salience(&DomainEvent::EstimationRecorded {
        task_id: "t1".into(),
        estimated_mins: 30,
        actual_mins: 90,
        deviation_pct: 200.0,
    });
    assert_eq!(verdict, SalienceVerdict::Extract);
}

#[test]
fn test_estimation_recorded_small_deviation_is_accumulate() {
    let verdict = evaluate_salience(&DomainEvent::EstimationRecorded {
        task_id: "t1".into(),
        estimated_mins: 30,
        actual_mins: 35,
        deviation_pct: 16.7,
    });
    assert_eq!(verdict, SalienceVerdict::Accumulate);
}

#[test]
fn test_task_execution_completed_is_extract() {
    // Already Extract — verify it stays that way
    let verdict = evaluate_salience(&DomainEvent::TaskExecutionCompleted {
        task_id: "t1".into(),
        execution_id: "e1".into(),
        tokens_used: 1000,
        cost_usd: Some(0.05),
        artifacts_count: 2,
    });
    assert_eq!(verdict, SalienceVerdict::Extract);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(/salience/)' 2>&1`
Expected: `test_task_completed_large_deviation_is_extract` and `test_estimation_recorded_large_deviation_is_extract` FAIL (both currently return Accumulate)

- [ ] **Step 3: Implement conditional salience upgrades**

In `crates/cognitive/src/salience.rs`, replace the catch-all patterns with conditional logic:

```rust
// Replace:
//   DomainEvent::TaskCompleted { .. } => SalienceVerdict::Accumulate,
// With:
DomainEvent::TaskCompleted { deviation_pct, .. } => {
    if deviation_pct.map_or(false, |d| d.abs() > 50.0) {
        SalienceVerdict::Extract
    } else {
        SalienceVerdict::Accumulate
    }
}

// Replace:
//   DomainEvent::EstimationRecorded { .. } => SalienceVerdict::Accumulate,
// With:
DomainEvent::EstimationRecorded { deviation_pct, .. } => {
    if deviation_pct.abs() > 50.0 {
        SalienceVerdict::Extract
    } else {
        SalienceVerdict::Accumulate
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(/salience/)' 2>&1`
Expected: All salience tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/salience.rs
git commit -m "feat(cognitive): conditional salience upgrade for task deviation events"
```

---

### Task 2: Enhanced observation mapping for task events

**Files:**
- Modify: `crates/cognitive/src/background.rs`
- Test: inline `#[cfg(test)] mod tests`

Currently `event_to_observation()` uses a catch-all `_ =>` for many task events (TaskDecomposed, DayPlanGenerated, TaskFocusStarted/Ended, EstimationRecorded, ProactiveSuggestionCreated). These get generic `Debug` formatting with importance 0.3. The spec requires domain-specific observations with proper importance scores.

- [ ] **Step 1: Write failing tests for new observation mappings**

Add to `crates/cognitive/src/background.rs` test module:

```rust
#[test]
fn test_event_to_observation_task_focus_started() {
    let event = DomainEvent::TaskFocusStarted {
        task_id: "t1".into(),
        energy_level: Some("high".into()),
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("t1"));
    assert!(obs.content.contains("high"));
    assert_eq!(obs.source_event, "TaskFocusStarted");
}

#[test]
fn test_event_to_observation_task_focus_ended() {
    let event = DomainEvent::TaskFocusEnded {
        task_id: "t1".into(),
        duration_secs: 2700,
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("45min")); // 2700 / 60
    assert_eq!(obs.source_event, "TaskFocusEnded");
}

#[test]
fn test_event_to_observation_estimation_recorded() {
    let event = DomainEvent::EstimationRecorded {
        task_id: "t1".into(),
        estimated_mins: 30,
        actual_mins: 75,
        deviation_pct: 150.0,
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("estimated 30min"));
    assert!(obs.content.contains("actual 75min"));
    assert!(obs.content.contains("150.0%"));
    assert_eq!(obs.source_event, "EstimationRecorded");
}

#[test]
fn test_event_to_observation_estimation_recorded_importance() {
    let large_dev = DomainEvent::EstimationRecorded {
        task_id: "t1".into(),
        estimated_mins: 30,
        actual_mins: 75,
        deviation_pct: 150.0,
    };
    let obs = event_to_observation(&large_dev).unwrap();
    assert!(obs.importance >= 0.6, "Large deviation should have high importance");

    let small_dev = DomainEvent::EstimationRecorded {
        task_id: "t2".into(),
        estimated_mins: 30,
        actual_mins: 35,
        deviation_pct: 16.7,
    };
    let obs2 = event_to_observation(&small_dev).unwrap();
    assert!(obs2.importance <= 0.4, "Small deviation should have low importance");
}

#[test]
fn test_event_to_observation_task_decomposed() {
    let event = DomainEvent::TaskDecomposed {
        source_task_id: "t1".into(),
        subtask_ids: vec!["s1".into(), "s2".into(), "s3".into()],
        total_estimated_mins: Some(120),
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("3 subtasks"));
    assert!(obs.content.contains("120min"));
    assert_eq!(obs.source_event, "TaskDecomposed");
}

#[test]
fn test_event_to_observation_day_plan_generated() {
    let event = DomainEvent::DayPlanGenerated {
        task_count: 5,
        total_minutes: 360,
        utilization_pct: Some(85.0),
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("5 tasks"));
    assert!(obs.content.contains("360min"));
    assert_eq!(obs.source_event, "DayPlanGenerated");
}

#[test]
fn test_event_to_observation_proactive_suggestion() {
    let event = DomainEvent::ProactiveSuggestionCreated {
        suggestion_id: "sug-1".into(),
        suggestion_type: "Decompose".into(),
        task_id: Some("t1".into()),
        confidence: 0.85,
    };
    let obs = event_to_observation(&event).unwrap();
    assert_eq!(obs.domain, "tasks");
    assert!(obs.content.contains("Decompose"));
    assert!(obs.content.contains("85%"));
    assert_eq!(obs.source_event, "ProactiveSuggestionCreated");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(/event_to_observation/)' 2>&1`
Expected: New tests FAIL — these events currently hit the `_ =>` catch-all with domain "general" and source_event "Other"

- [ ] **Step 3: Implement specific observation mappings**

In `crates/cognitive/src/background.rs` `event_to_observation()`, add explicit match arms BEFORE the `_ =>` catch-all:

```rust
DomainEvent::TaskFocusStarted {
    task_id,
    energy_level,
} => {
    let energy = energy_level
        .as_deref()
        .unwrap_or("unknown");
    Some(Observation {
        domain: "tasks".into(),
        content: format!("Focus started on task {task_id} at energy level {energy}"),
        importance: 0.3,
        source_event: "TaskFocusStarted".into(),
        timestamp: now,
    })
}
DomainEvent::TaskFocusEnded {
    task_id,
    duration_secs,
} => Some(Observation {
    domain: "tasks".into(),
    content: format!(
        "Focus ended on task {task_id} after {}min",
        duration_secs / 60
    ),
    importance: 0.3,
    source_event: "TaskFocusEnded".into(),
    timestamp: now,
}),
DomainEvent::EstimationRecorded {
    task_id,
    estimated_mins,
    actual_mins,
    deviation_pct,
} => {
    let base_importance = 0.3;
    let importance = if deviation_pct.abs() > 50.0 {
        (base_importance + 0.3).min(1.0)
    } else {
        base_importance
    };
    Some(Observation {
        domain: "tasks".into(),
        content: format!(
            "Estimation recorded for task {task_id}: estimated {estimated_mins}min, actual {actual_mins}min, deviation {deviation_pct:.1}%"
        ),
        importance,
        source_event: "EstimationRecorded".into(),
        timestamp: now,
    })
},
DomainEvent::TaskDecomposed {
    source_task_id,
    subtask_ids,
    total_estimated_mins,
} => {
    let est = total_estimated_mins
        .map(|m| format!(", total {m}min"))
        .unwrap_or_default();
    Some(Observation {
        domain: "tasks".into(),
        content: format!(
            "Task {source_task_id} decomposed into {} subtasks{est}",
            subtask_ids.len()
        ),
        importance: 0.4,
        source_event: "TaskDecomposed".into(),
        timestamp: now,
    })
},
DomainEvent::DayPlanGenerated {
    task_count,
    total_minutes,
    utilization_pct,
} => {
    let util = utilization_pct
        .map(|u| format!(", {u:.0}% utilization"))
        .unwrap_or_default();
    Some(Observation {
        domain: "tasks".into(),
        content: format!(
            "Day plan generated: {task_count} tasks, {total_minutes}min scheduled{util}"
        ),
        importance: 0.4,
        source_event: "DayPlanGenerated".into(),
        timestamp: now,
    })
},
DomainEvent::ProactiveSuggestionCreated {
    suggestion_id: _,
    suggestion_type,
    task_id,
    confidence,
} => {
    let target = task_id
        .as_deref()
        .map(|id| format!(" for task {id}"))
        .unwrap_or_default();
    Some(Observation {
        domain: "tasks".into(),
        content: format!(
            "Proactive suggestion: {suggestion_type}{target} (confidence {:.0}%)",
            confidence * 100.0
        ),
        importance: 0.3,
        source_event: "ProactiveSuggestionCreated".into(),
        timestamp: now,
    })
},
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(/event_to_observation/)' 2>&1`
Expected: All observation mapping tests PASS

- [ ] **Step 5: Run full cognitive test suite**

Run: `cargo nextest run -p cognitive 2>&1`
Expected: All tests PASS, no regressions

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/background.rs
git commit -m "feat(cognitive): add observation mappings for task focus, estimation, decomposition, planning events"
```

---

### Task 3: Cognitive bridge helpers (L4)

**Files:**
- Create: `crates/feature-tasks/src/cognitive_bridge.rs`
- Modify: `crates/feature-tasks/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests`

Pure functions that parse cognitive `SemanticFact` structs into task-domain structures. No LLM calls. These will be consumed by Phase 3 handlers (ProactiveHandler, ForecastHandler, DayPlanningHandler).

- [ ] **Step 1: Write the cognitive_bridge module with tests**

Create `crates/feature-tasks/src/cognitive_bridge.rs`:

```rust
//! Typed helpers for parsing cognitive facts into task-domain structures.
//!
//! These functions bridge the cognitive memory system (SemanticFact) with
//! task-domain types (EnergyProfile, estimation bias, velocity, etc.).
//! Pure computation — no LLM calls, no I/O.

/// A parsed energy profile from cognitive facts.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyProfile {
    pub peak_hours: Option<String>,
    pub preferred_energy_by_period: Vec<(String, String)>, // (period, energy_level)
}

/// Extract an energy profile from cognitive facts.
///
/// Looks for facts with predicates: `peak_focus_hours`, `preferred_energy_*`.
pub fn extract_energy_profile(facts: &[SemanticFact]) -> Option<EnergyProfile> {
    let peak = facts
        .iter()
        .find(|f| f.predicate == "peak_focus_hours")
        .map(|f| f.object.clone());

    let prefs: Vec<(String, String)> = facts
        .iter()
        .filter(|f| f.predicate.starts_with("preferred_energy_"))
        .map(|f| {
            let period = f.predicate.strip_prefix("preferred_energy_").unwrap_or("").to_string();
            (period, f.object.clone())
        })
        .collect();

    if peak.is_some() || !prefs.is_empty() {
        Some(EnergyProfile {
            peak_hours: peak,
            preferred_energy_by_period: prefs,
        })
    } else {
        None
    }
}

/// Extract estimation bias from cognitive facts.
///
/// Looks for `estimation_bias` (general) and `estimation_bias_{tag}` (per-tag).
/// If tags are provided, returns the most specific match; otherwise returns
/// the general bias. Returns the bias as a fraction (e.g., 0.38 = +38% underestimation).
pub fn extract_estimation_bias(facts: &[SemanticFact], tags: &[String]) -> Option<f64> {
    // Try tag-specific bias first
    for tag in tags {
        let predicate = format!("estimation_bias_{tag}");
        if let Some(fact) = facts.iter().find(|f| f.predicate == predicate) {
            if let Some(bias) = parse_bias_value(&fact.object) {
                return Some(bias);
            }
        }
    }

    // Fall back to general bias
    facts
        .iter()
        .find(|f| f.predicate == "estimation_bias")
        .and_then(|f| parse_bias_value(&f.object))
}

/// Extract task completion velocity from cognitive facts.
///
/// Looks for `completion_pace` (per-project) or `tasks_completed_per_week` (global).
pub fn extract_velocity(facts: &[SemanticFact], project_id: Option<&str>) -> Option<f64> {
    // Try project-specific velocity first
    if let Some(pid) = project_id {
        let subject = format!("project:{pid}");
        if let Some(fact) = facts
            .iter()
            .find(|f| f.subject == subject && f.predicate == "completion_pace")
        {
            if let Some(v) = parse_numeric_value(&fact.object) {
                return Some(v);
            }
        }
    }

    // Fall back to global velocity
    facts
        .iter()
        .find(|f| f.predicate == "tasks_completed_per_week")
        .and_then(|f| parse_numeric_value(&f.object))
}

/// Extract deferral patterns from cognitive facts.
///
/// Looks for facts with predicate `deferral_pattern`.
pub fn extract_deferral_patterns(facts: &[SemanticFact]) -> Vec<String> {
    facts
        .iter()
        .filter(|f| f.predicate == "deferral_pattern")
        .map(|f| f.object.clone())
        .collect()
}

/// Extract agentic task success rate from cognitive facts.
///
/// Looks for `agentic_success_rate` predicate. Returns as fraction (e.g., 0.78 = 78%).
pub fn extract_agentic_success_rate(facts: &[SemanticFact]) -> Option<f64> {
    facts
        .iter()
        .find(|f| f.predicate == "agentic_success_rate")
        .and_then(|f| parse_percentage_value(&f.object))
}

/// Parse a bias string like "+38% underestimation" or "-10%" into a fraction.
fn parse_bias_value(s: &str) -> Option<f64> {
    let cleaned = s
        .trim()
        .replace('%', "")
        .split_whitespace()
        .next()?
        .to_string();
    cleaned.parse::<f64>().ok().map(|v| v / 100.0)
}

/// Parse a numeric value from strings like "12.5 average" or "3.2 tasks/week".
fn parse_numeric_value(s: &str) -> Option<f64> {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
}

/// Parse a percentage string like "78% (7/9)" or "0.78" into a fraction.
fn parse_percentage_value(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    // Try "78% ..." format
    if let Some(pct_str) = trimmed.split('%').next() {
        if let Ok(v) = pct_str.trim().parse::<f64>() {
            return Some(v / 100.0);
        }
    }
    // Try raw fraction "0.78"
    trimmed
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v >= 0.0 && *v <= 1.0)
}

// Use the cognitive SemanticFact type
use cognitive::types::SemanticFact;

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(subject: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: uuid::Uuid::new_v4().to_string(),
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source_observations: vec![],
            first_observed: chrono::Utc::now().to_rfc3339(),
            last_reinforced: chrono::Utc::now().to_rfc3339(),
            reinforcement_count: 1,
            decay_rate: None,
            embedding: None,
        }
    }

    #[test]
    fn test_extract_energy_profile_with_peak() {
        let facts = vec![
            fact("user", "peak_focus_hours", "9:00-11:30"),
            fact("user", "preferred_energy_morning", "deep"),
            fact("user", "preferred_energy_afternoon", "medium"),
        ];
        let profile = extract_energy_profile(&facts).unwrap();
        assert_eq!(profile.peak_hours, Some("9:00-11:30".into()));
        assert_eq!(profile.preferred_energy_by_period.len(), 2);
    }

    #[test]
    fn test_extract_energy_profile_none() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_energy_profile(&facts).is_none());
    }

    #[test]
    fn test_extract_estimation_bias_general() {
        let facts = vec![fact("user", "estimation_bias", "+38% underestimation")];
        let bias = extract_estimation_bias(&facts, &[]).unwrap();
        assert!((bias - 0.38).abs() < 0.01);
    }

    #[test]
    fn test_extract_estimation_bias_tag_specific() {
        let facts = vec![
            fact("user", "estimation_bias", "+38% underestimation"),
            fact("user", "estimation_bias_rust", "+55% for rust tasks"),
        ];
        let bias = extract_estimation_bias(&facts, &["rust".into()]).unwrap();
        assert!((bias - 0.55).abs() < 0.01);
    }

    #[test]
    fn test_extract_estimation_bias_none() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_estimation_bias(&facts, &[]).is_none());
    }

    #[test]
    fn test_extract_velocity_project() {
        let facts = vec![
            fact("project:p1", "completion_pace", "3.2 tasks/week"),
            fact("user", "tasks_completed_per_week", "12.5 average"),
        ];
        let v = extract_velocity(&facts, Some("p1")).unwrap();
        assert!((v - 3.2).abs() < 0.01);
    }

    #[test]
    fn test_extract_velocity_global_fallback() {
        let facts = vec![fact("user", "tasks_completed_per_week", "12.5 average")];
        let v = extract_velocity(&facts, Some("p999")).unwrap();
        assert!((v - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_extract_deferral_patterns() {
        let facts = vec![
            fact("user", "deferral_pattern", "defers planning tasks to someday"),
            fact("user", "deferral_pattern", "defers research tasks when busy"),
        ];
        let patterns = extract_deferral_patterns(&facts);
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_extract_agentic_success_rate() {
        let facts = vec![fact("user", "agentic_success_rate", "78% (7/9)")];
        let rate = extract_agentic_success_rate(&facts).unwrap();
        assert!((rate - 0.78).abs() < 0.01);
    }

    #[test]
    fn test_parse_bias_negative() {
        assert!((parse_bias_value("-10%").unwrap() - (-0.10)).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Add module to feature-tasks lib.rs**

In `crates/feature-tasks/src/lib.rs`, add:
```rust
pub mod cognitive_bridge;
```

- [ ] **Step 3: Check that cognitive crate is a dependency of feature-tasks**

Run: `grep 'cognitive' crates/feature-tasks/Cargo.toml`

If not present, add to `[dependencies]`:
```toml
cognitive = { path = "../cognitive" }
```

Note: If this creates a circular dependency (cognitive depends on feature-tasks), then move the `SemanticFact` type reference to a shared location or use a generic struct instead. The cognitive_bridge should only depend on the `SemanticFact` struct definition, not the full cognitive crate. If blocked, define a local `CognitiveFact` struct matching the same fields and convert at the call site.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-tasks -E 'test(/cognitive_bridge/)' 2>&1`
Expected: All cognitive bridge tests PASS

- [ ] **Step 5: Run workspace build to verify no circular deps**

Run: `cargo build --workspace 2>&1`
Expected: Successful build

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/src/cognitive_bridge.rs crates/feature-tasks/src/lib.rs crates/feature-tasks/Cargo.toml
git commit -m "feat(tasks): add cognitive bridge helpers for parsing semantic facts into task-domain types"
```

---

## Chunk 2: Forecast Pure Computation (3.2 — L4 only)

Build the pure computation layer for forecasting before the LLM wrapper. This is stateless math that can be thoroughly unit tested.

### Task 4: Similarity matching and deviation correction

**Files:**
- Create: `crates/feature-tasks/src/forecast.rs`
- Modify: `crates/feature-tasks/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests`

Implements the spec's similarity matching (tags overlap, energy level, complexity score, same project, recency) and deviation correction formulas.

- [ ] **Step 1: Write forecast.rs with similarity and deviation functions + tests**

Create `crates/feature-tasks/src/forecast.rs`:

```rust
//! Pure computation for task estimation forecasting.
//!
//! No LLM calls, no I/O. Stateless functions for:
//! - Similarity matching between tasks
//! - Deviation correction (adjust estimates based on historical bias)
//! - Velocity calculation
//! - Accuracy statistics

use chrono::{DateTime, Utc};

/// A completed task's estimation record for similarity matching.
#[derive(Debug, Clone)]
pub struct EstimationRecord {
    pub task_id: String,
    pub tags: Vec<String>,
    pub energy_level: Option<String>,
    pub complexity_score: Option<i32>,
    pub project_id: Option<String>,
    pub estimated_minutes: i32,
    pub actual_minutes: i32,
    pub completed_at: DateTime<Utc>,
}

/// Compute similarity score between a target task and a historical record.
///
/// Weights from spec:
/// - Tags overlap (Jaccard): 0.35
/// - Energy level: 0.20
/// - Complexity score: 0.20
/// - Same project: 0.15
/// - Recency: 0.10
pub fn similarity(
    target_tags: &[String],
    target_energy: Option<&str>,
    target_complexity: Option<i32>,
    target_project: Option<&str>,
    record: &EstimationRecord,
    now: DateTime<Utc>,
) -> f64 {
    let tags_sim = jaccard_similarity(target_tags, &record.tags);
    let energy_sim = energy_similarity(target_energy, record.energy_level.as_deref());
    let complexity_sim = complexity_similarity(target_complexity, record.complexity_score);
    let project_sim = if target_project == record.project_id.as_deref() && target_project.is_some() {
        1.0
    } else {
        0.0
    };
    let recency = recency_score(record.completed_at, now);

    tags_sim * 0.35 + energy_sim * 0.20 + complexity_sim * 0.20 + project_sim * 0.15 + recency * 0.10
}

/// Jaccard similarity: |A ∩ B| / |A ∪ B|
fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let set_b: std::collections::HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 { 0.0 } else { intersection / union }
}

/// Energy similarity: exact=1.0, adjacent=0.5, else 0.0
/// Order: low < medium < high < deep
fn energy_similarity(a: Option<&str>, b: Option<&str>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) if a == b => 1.0,
        (Some(a), Some(b)) => {
            let order = ["low", "medium", "high", "deep"];
            let pos_a = order.iter().position(|&x| x == a);
            let pos_b = order.iter().position(|&x| x == b);
            match (pos_a, pos_b) {
                (Some(pa), Some(pb)) if pa.abs_diff(pb) == 1 => 0.5,
                _ => 0.0,
            }
        }
        _ => 0.0,
    }
}

/// Complexity similarity: max(0.0, 1.0 - |a-b| / 10.0)
fn complexity_similarity(a: Option<i32>, b: Option<i32>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => (1.0 - (a - b).unsigned_abs() as f64 / 10.0).max(0.0),
        _ => 0.0,
    }
}

/// Recency score: e^(-days_ago / 30)
fn recency_score(completed_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let days_ago = (now - completed_at).num_days().max(0) as f64;
    (-days_ago / 30.0).exp()
}

/// Deviation correction result.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationCorrection {
    pub adjusted_estimate: f64,
    pub optimistic: f64,
    pub pessimistic: f64,
    pub mean_deviation: f64,
    pub std_deviation: f64,
    pub sample_size: usize,
}

/// Apply deviation correction to an estimate using historical records.
///
/// Only considers records with similarity >= `threshold`.
/// Formula from spec:
///   adjusted = original × (1.0 + mean_deviation)
///   optimistic = original × (1.0 + mean_deviation - std_deviation)
///   pessimistic = original × (1.0 + mean_deviation + std_deviation)
pub fn deviation_correction(
    original_estimate: i32,
    records: &[(f64, &EstimationRecord)], // (similarity, record)
    threshold: f64,
) -> Option<DeviationCorrection> {
    let eligible: Vec<f64> = records
        .iter()
        .filter(|(sim, _)| *sim >= threshold)
        .map(|(_, r)| {
            (r.actual_minutes as f64 - r.estimated_minutes as f64) / r.estimated_minutes as f64
        })
        .collect();

    if eligible.is_empty() {
        return None;
    }

    let n = eligible.len();
    let mean = eligible.iter().sum::<f64>() / n as f64;
    let variance = eligible.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    let orig = original_estimate as f64;

    Some(DeviationCorrection {
        adjusted_estimate: orig * (1.0 + mean),
        optimistic: (orig * (1.0 + mean - std_dev)).max(1.0),
        pessimistic: orig * (1.0 + mean + std_dev),
        mean_deviation: mean,
        std_deviation: std_dev,
        sample_size: n,
    })
}

/// Data quality tier based on sample size.
#[derive(Debug, Clone, PartialEq)]
pub enum DataQualityTier {
    Strong,       // 20+
    Moderate,     // 10-19
    Weak,         // 5-9
    Insufficient, // <5
}

impl DataQualityTier {
    pub fn from_sample_size(n: usize) -> Self {
        match n {
            20.. => Self::Strong,
            10..=19 => Self::Moderate,
            5..=9 => Self::Weak,
            _ => Self::Insufficient,
        }
    }
}

/// Compute project velocity: completed minutes in last N weeks / N.
pub fn project_velocity(
    completed_records: &[EstimationRecord],
    now: DateTime<Utc>,
    weeks: u32,
) -> Option<f64> {
    let cutoff = now - chrono::Duration::weeks(weeks as i64);
    let recent: Vec<&EstimationRecord> = completed_records
        .iter()
        .filter(|r| r.completed_at >= cutoff)
        .collect();

    if recent.is_empty() {
        return None;
    }

    let total_mins: i32 = recent.iter().map(|r| r.actual_minutes).sum();
    Some(total_mins as f64 / weeks as f64)
}

/// Accuracy statistics for a set of estimation records.
#[derive(Debug, Clone)]
pub struct AccuracyStats {
    pub count: usize,
    pub mean_deviation_pct: f64,
    pub median_deviation_pct: f64,
    pub p90_deviation_pct: f64,
    pub std_deviation_pct: f64,
}

/// Compute accuracy statistics from estimation records.
pub fn accuracy_stats(records: &[EstimationRecord]) -> Option<AccuracyStats> {
    if records.is_empty() {
        return None;
    }

    let mut deviations: Vec<f64> = records
        .iter()
        .map(|r| {
            ((r.actual_minutes as f64 - r.estimated_minutes as f64) / r.estimated_minutes as f64)
                * 100.0
        })
        .collect();
    deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = deviations.len();
    let mean = deviations.iter().sum::<f64>() / n as f64;
    let variance = deviations.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n as f64;
    let median = if n % 2 == 0 {
        (deviations[n / 2 - 1] + deviations[n / 2]) / 2.0
    } else {
        deviations[n / 2]
    };
    let p90_idx = ((n as f64 * 0.9).ceil() as usize).min(n) - 1;

    Some(AccuracyStats {
        count: n,
        mean_deviation_pct: mean,
        median_deviation_pct: median,
        p90_deviation_pct: deviations[p90_idx],
        std_deviation_pct: variance.sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn record(
        tags: &[&str],
        energy: &str,
        complexity: i32,
        project: &str,
        est: i32,
        actual: i32,
        days_ago: i64,
    ) -> EstimationRecord {
        EstimationRecord {
            task_id: uuid::Uuid::new_v4().to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            energy_level: Some(energy.into()),
            complexity_score: Some(complexity),
            project_id: Some(project.into()),
            estimated_minutes: est,
            actual_minutes: actual,
            completed_at: Utc::now() - Duration::days(days_ago),
        }
    }

    #[test]
    fn test_jaccard_identical() {
        let a = vec!["rust".into(), "backend".into()];
        assert!((jaccard_similarity(&a, &a) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a = vec!["rust".into()];
        let b = vec!["python".into()];
        assert!((jaccard_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_partial() {
        let a = vec!["rust".into(), "backend".into()];
        let b = vec!["rust".into(), "frontend".into()];
        assert!((jaccard_similarity(&a, &b) - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_energy_exact_match() {
        assert!((energy_similarity(Some("high"), Some("high")) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_energy_adjacent() {
        assert!((energy_similarity(Some("high"), Some("deep")) - 0.5).abs() < 0.001);
        assert!((energy_similarity(Some("low"), Some("medium")) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_energy_distant() {
        assert!((energy_similarity(Some("low"), Some("deep")) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_complexity_same() {
        assert!((complexity_similarity(Some(5), Some(5)) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_complexity_far() {
        assert!((complexity_similarity(Some(1), Some(10)) - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_deviation_correction_underestimate() {
        let records = vec![
            record(&["rust"], "high", 5, "p1", 30, 60, 5), // +100% deviation
            record(&["rust"], "high", 5, "p1", 45, 60, 10), // +33% deviation
        ];
        let scored: Vec<(f64, &EstimationRecord)> = records.iter().map(|r| (0.8, r)).collect();
        let result = deviation_correction(30, &scored, 0.3).unwrap();

        // mean deviation = (1.0 + 0.333) / 2 = 0.667
        assert!(result.adjusted_estimate > 30.0);
        assert!(result.pessimistic > result.adjusted_estimate);
        assert!(result.optimistic < result.adjusted_estimate);
        assert_eq!(result.sample_size, 2);
    }

    #[test]
    fn test_deviation_correction_threshold_filters() {
        let records = vec![
            record(&["rust"], "high", 5, "p1", 30, 60, 5),
        ];
        let scored: Vec<(f64, &EstimationRecord)> = records.iter().map(|r| (0.1, r)).collect();
        let result = deviation_correction(30, &scored, 0.3);
        assert!(result.is_none(), "Below threshold should return None");
    }

    #[test]
    fn test_data_quality_tiers() {
        assert_eq!(DataQualityTier::from_sample_size(25), DataQualityTier::Strong);
        assert_eq!(DataQualityTier::from_sample_size(15), DataQualityTier::Moderate);
        assert_eq!(DataQualityTier::from_sample_size(7), DataQualityTier::Weak);
        assert_eq!(DataQualityTier::from_sample_size(3), DataQualityTier::Insufficient);
    }

    #[test]
    fn test_project_velocity() {
        let now = Utc::now();
        let records = vec![
            record(&[], "medium", 3, "p1", 30, 45, 3),
            record(&[], "medium", 3, "p1", 60, 55, 10),
            record(&[], "medium", 3, "p1", 90, 100, 50), // > 4 weeks ago
        ];
        let v = project_velocity(&records, now, 4).unwrap();
        // 45 + 55 = 100 mins in 4 weeks = 25 mins/week
        assert!((v - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_project_velocity_none_when_empty() {
        assert!(project_velocity(&[], Utc::now(), 4).is_none());
    }

    #[test]
    fn test_accuracy_stats_basic() {
        let records = vec![
            record(&[], "medium", 3, "p1", 30, 45, 5),  // +50%
            record(&[], "medium", 3, "p1", 60, 90, 10),  // +50%
            record(&[], "medium", 3, "p1", 40, 40, 15),  // 0%
        ];
        let stats = accuracy_stats(&records).unwrap();
        assert_eq!(stats.count, 3);
        // mean: (50 + 50 + 0) / 3 = 33.3%
        assert!((stats.mean_deviation_pct - 33.33).abs() < 0.1);
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

In `crates/feature-tasks/src/lib.rs`, add:
```rust
pub mod forecast;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-tasks -E 'test(/forecast/)' 2>&1`
Expected: All forecast tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/forecast.rs crates/feature-tasks/src/lib.rs
git commit -m "feat(tasks): add pure forecast computation — similarity matching, deviation correction, velocity, accuracy stats"
```

---

## Chunk 3: ProactiveHandler Implementation (3.1)

### Task 5: Proactive suggestion prompt template

**Files:**
- Create: `crates/agent/src/templates/proactive_suggestions.md`

- [ ] **Step 1: Write the prompt template**

Create `crates/agent/src/templates/proactive_suggestions.md`:

```markdown
You are analyzing a task to generate actionable suggestions. You will receive the task details and a trigger reason.

## Trigger: {{ trigger }}

## Task
- ID: {{ task_id }}
- Title: {{ title }}
- Status: {{ status }}
- Priority: {{ priority }}
- Energy Level: {{ energy_level }}
- Estimated Minutes: {{ estimated_minutes }}
- Due Date: {{ due_date }}
- Tags: {{ tags }}
- Created: {{ created_at }}
- Description: {{ description }}

## Context
{{ context }}

## Instructions

Based on the trigger and task details, generate 0-3 suggestion candidates. Each suggestion should be:
1. **Actionable** — maps to a concrete SuggestionAction
2. **Confident** — include a confidence score (0.0-1.0) reflecting how sure you are this suggestion is helpful
3. **Reasoned** — brief explanation of why this suggestion is appropriate

Available suggestion types and their actions:
- Reprioritize → SetPriority { priority: i16 }
- Reschedule → SetDueDate { due_date: "YYYY-MM-DD" }
- Decompose → TriggerDecomposition
- AdjustEstimation → UpdateEstimationBaseline { minutes: i32 }
- AdjustEnergy → SetEnergyLevel { level: "low"|"medium"|"high"|"deep" }
- Abandon → Archive
- WorkflowInsight → Informational

Respond in JSON:
```json
{
  "suggestions": [
    {
      "suggestion_type": "Reprioritize",
      "title": "Brief title",
      "description": "Why this suggestion",
      "confidence": 0.85,
      "action": { "SetPriority": { "priority": 1 } }
    }
  ]
}
```

If no suggestions are warranted, return `{"suggestions": []}`.
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent/src/templates/proactive_suggestions.md
git commit -m "feat(agent): add proactive suggestions prompt template"
```

---

### Task 6: LlmProactiveHandler implementation

**Files:**
- Create: `crates/agent/src/handlers/proactive.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the handler implementation**

Create `crates/agent/src/handlers/proactive.rs`:

```rust
//! LLM-powered proactive suggestion handler.
//!
//! Evaluates tasks against triggers (overdue, stale, WIP exceeded, etc.)
//! and generates suggestion candidates using LLM reasoning.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use tracing::{debug, warn};

use feature_tasks::types::{
    SuggestionCandidate, SuggestionScope, SuggestionTrigger, Task,
    SuggestionType, SuggestionAction,
};
use feature_tasks::ProactiveHandler;
use providers::DynProvider;
use storage::TaskRepo;

static PROMPT_TEMPLATE: &str = include_str!("../templates/proactive_suggestions.md");

pub struct LlmProactiveHandler {
    provider: DynProvider,
    model: String,
    repo: TaskRepo,
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl LlmProactiveHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        repo: TaskRepo,
        domain_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self { provider, model, repo, domain_bus }
    }

    fn build_prompt(&self, task: &Task, trigger: &SuggestionTrigger, context: &str) -> String {
        PROMPT_TEMPLATE
            .replace("{{ trigger }}", &format!("{trigger:?}"))
            .replace("{{ task_id }}", &task.id)
            .replace("{{ title }}", &task.title)
            .replace("{{ status }}", &task.status)
            .replace("{{ priority }}", &task.priority.map_or("none".into(), |p| p.to_string()))
            .replace("{{ energy_level }}", task.energy_level.as_deref().unwrap_or("none"))
            .replace("{{ estimated_minutes }}", &task.estimated_minutes.map_or("none".into(), |m| m.to_string()))
            .replace("{{ due_date }}", &task.due_date.map_or("none".into(), |d| d.to_rfc3339()))
            .replace("{{ tags }}", &task.tags.join(", "))
            .replace("{{ created_at }}", &task.created_at.to_rfc3339())
            .replace("{{ description }}", task.description.as_deref().unwrap_or(""))
            .replace("{{ context }}", context)
    }

    async fn parse_suggestions(
        &self,
        response: &str,
        task_id: &str,
        trigger: &SuggestionTrigger,
    ) -> Vec<SuggestionCandidate> {
        // Extract JSON from response (may be wrapped in markdown code blocks)
        let json_str = response
            .find('{')
            .and_then(|start| response.rfind('}').map(|end| &response[start..=end]))
            .unwrap_or(response);

        let parsed: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse proactive suggestions JSON: {e}");
                return vec![];
            }
        };

        let suggestions = parsed
            .get("suggestions")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        suggestions
            .into_iter()
            .filter_map(|s| {
                let suggestion_type_str = s.get("suggestion_type")?.as_str()?;
                let suggestion_type: SuggestionType =
                    serde_json::from_value(serde_json::Value::String(suggestion_type_str.into()))
                        .ok()?;
                let action: SuggestionAction = serde_json::from_value(s.get("action")?.clone()).ok()?;

                Some(SuggestionCandidate {
                    task_id: Some(task_id.to_string()),
                    suggestion_type,
                    title: s.get("title")?.as_str()?.to_string(),
                    description: s.get("description").and_then(|d| d.as_str()).map(String::from),
                    confidence: s.get("confidence")?.as_f64()?,
                    action,
                    trigger: trigger.clone(),
                })
            })
            .collect()
    }
}

#[async_trait]
impl ProactiveHandler for LlmProactiveHandler {
    async fn suggest(&self, scope: &SuggestionScope) -> Result<Vec<SuggestionCandidate>> {
        // Load tasks matching scope
        let filter = storage::TaskFilter {
            status: Some("todo".into()),
            ..Default::default()
        };
        let tasks = self.repo.list(&filter).await?;

        let mut all_suggestions = Vec::new();
        let now = Utc::now();

        for row in &tasks {
            let task: Task = row.into();

            // Check each applicable trigger
            if let Some(due) = &task.due_date {
                if *due < now {
                    let candidates = self.evaluate_task(&task, &SuggestionTrigger::TaskOverdue).await?;
                    all_suggestions.extend(candidates);
                }
            }
        }

        // Deduplicate: keep highest confidence per (task_id, suggestion_type)
        all_suggestions.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        all_suggestions.retain(|s| {
            let key = (s.task_id.clone(), format!("{:?}", s.suggestion_type));
            seen.insert(key)
        });

        Ok(all_suggestions)
    }

    async fn evaluate_task(
        &self,
        task: &Task,
        trigger: &SuggestionTrigger,
    ) -> Result<Vec<SuggestionCandidate>> {
        let context = format!("Trigger reason: {trigger:?}");
        let prompt = self.build_prompt(task, trigger, &context);

        debug!(task_id = %task.id, trigger = ?trigger, "Evaluating task for proactive suggestions");

        let response = self
            .provider
            .chat(&self.model, &[providers::ChatMessage::user(&prompt)], None)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(format!("LLM call failed: {e}")))?;

        let text = response.text().unwrap_or_default();
        let candidates = self.parse_suggestions(&text, &task.id, trigger).await;

        // Emit events for created suggestions
        if let Some(ref bus) = self.domain_bus {
            for c in &candidates {
                let _ = bus.publish(bus::DomainEvent::ProactiveSuggestionCreated {
                    suggestion_id: uuid::Uuid::new_v4().to_string(),
                    suggestion_type: format!("{:?}", c.suggestion_type),
                    task_id: c.task_id.clone(),
                    confidence: c.confidence,
                });
            }
        }

        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_includes_task_details() {
        let handler = LlmProactiveHandler::new(
            providers::test_provider(),
            "test-model".into(),
            TaskRepo::new_dummy(),
            None,
        );

        let task = Task {
            id: "t1".into(),
            title: "Fix login bug".into(),
            status: "todo".into(),
            tags: vec!["rust".into(), "auth".into()],
            ..Task::default_instance()
        };

        let prompt = handler.build_prompt(
            &task,
            &SuggestionTrigger::TaskOverdue,
            "overdue by 3 days",
        );

        assert!(prompt.contains("Fix login bug"));
        assert!(prompt.contains("TaskOverdue"));
        assert!(prompt.contains("rust, auth"));
    }

    #[tokio::test]
    async fn test_parse_suggestions_valid_json() {
        let handler = LlmProactiveHandler::new(
            providers::test_provider(),
            "test-model".into(),
            TaskRepo::new_dummy(),
            None,
        );

        let response = r#"{"suggestions": [{"suggestion_type": "Reprioritize", "title": "Increase priority", "description": "Task is overdue", "confidence": 0.85, "action": {"SetPriority": {"priority": 1}}}]}"#;

        let candidates = handler
            .parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue)
            .await;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Increase priority");
        assert!((candidates[0].confidence - 0.85).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_parse_suggestions_empty() {
        let handler = LlmProactiveHandler::new(
            providers::test_provider(),
            "test-model".into(),
            TaskRepo::new_dummy(),
            None,
        );

        let response = r#"{"suggestions": []}"#;
        let candidates = handler
            .parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue)
            .await;

        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn test_parse_suggestions_invalid_json() {
        let handler = LlmProactiveHandler::new(
            providers::test_provider(),
            "test-model".into(),
            TaskRepo::new_dummy(),
            None,
        );

        let response = "I'm sorry, I can't help with that.";
        let candidates = handler
            .parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue)
            .await;

        assert!(candidates.is_empty());
    }
}
```

Note: The `providers::test_provider()` and `TaskRepo::new_dummy()` are test helpers. If they don't exist, create minimal stubs or use the existing test patterns from Phase 2 handlers. Check `crates/agent/src/handlers/decomposition.rs` tests for the established pattern.

- [ ] **Step 2: Update handlers/mod.rs**

In `crates/agent/src/handlers/mod.rs`:

```rust
//! Phase 2-3 handler implementations (L5).

mod decomposition;
mod execution;
mod planning;
mod proactive;

pub use decomposition::LlmDecompositionHandler;
pub use execution::LlmTaskExecutionHandler;
pub use planning::LlmDayPlanningHandler;
pub use proactive::LlmProactiveHandler;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(/proactive/)' 2>&1`
Expected: All proactive handler tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/handlers/proactive.rs crates/agent/src/handlers/mod.rs crates/agent/src/templates/proactive_suggestions.md
git commit -m "feat(agent): implement LlmProactiveHandler with event-driven suggestion generation"
```

---

### Task 7: SuggestionApplier implementation

**Files:**
- Create: `crates/agent/src/handlers/suggestion_applier.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the SuggestionApplier implementation**

Create `crates/agent/src/handlers/suggestion_applier.rs`:

```rust
//! Applies accepted suggestion actions to tasks.
//!
//! Each SuggestionAction variant maps to a specific repo operation.
//! This handler is intentionally simple — it delegates to TaskRepo
//! and returns a human-readable summary.

use async_trait::async_trait;
use common::Result;
use tracing::info;

use feature_tasks::types::SuggestionAction;
use feature_tasks::SuggestionApplier;
use storage::TaskRepo;

pub struct TaskSuggestionApplier {
    repo: TaskRepo,
}

impl TaskSuggestionApplier {
    pub fn new(repo: TaskRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl SuggestionApplier for TaskSuggestionApplier {
    async fn apply(
        &self,
        suggestion_id: &str,
        task_id: Option<&str>,
        action: &SuggestionAction,
    ) -> Result<String> {
        info!(suggestion_id, ?task_id, ?action, "Applying suggestion");

        let summary = match action {
            SuggestionAction::SetPriority { priority } => {
                let tid = require_task_id(task_id)?;
                self.repo.update_field(tid, "priority", &priority.to_string()).await?;
                format!("Set priority to {priority}")
            }
            SuggestionAction::SetDueDate { due_date } => {
                let tid = require_task_id(task_id)?;
                self.repo.update_field(tid, "due_date", due_date).await?;
                format!("Set due date to {due_date}")
            }
            SuggestionAction::TriggerDecomposition => {
                let tid = require_task_id(task_id)?;
                // Mark task for decomposition — the actual decomposition
                // is triggered by the caller (tool action or cron)
                format!("Marked task {tid} for decomposition")
            }
            SuggestionAction::ConvertToAgentic => {
                let tid = require_task_id(task_id)?;
                self.repo.update_field(tid, "task_type", "agentic").await?;
                format!("Converted task {tid} to agentic type")
            }
            SuggestionAction::Archive => {
                let tid = require_task_id(task_id)?;
                self.repo.update_field(tid, "status", "archived").await?;
                format!("Archived task {tid}")
            }
            SuggestionAction::MergeInto { target_task_id } => {
                let tid = require_task_id(task_id)?;
                // Mark source as archived, note the merge target
                self.repo.update_field(tid, "status", "archived").await?;
                format!("Merged task {tid} into {target_task_id}")
            }
            SuggestionAction::RemoveBlocker { blocker_id } => {
                let tid = require_task_id(task_id)?;
                self.repo.remove_dependency(tid, blocker_id).await?;
                format!("Removed blocker {blocker_id} from task {tid}")
            }
            SuggestionAction::UpdateEstimationBaseline { minutes } => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update_field(tid, "estimated_minutes", &minutes.to_string())
                    .await?;
                format!("Updated estimation to {minutes}min")
            }
            SuggestionAction::SetEnergyLevel { level } => {
                let tid = require_task_id(task_id)?;
                self.repo.update_field(tid, "energy_level", &format!("{level:?}")).await?;
                format!("Set energy level to {level:?}")
            }
            SuggestionAction::Informational => {
                "Informational suggestion — no action taken".into()
            }
        };

        // Mark suggestion as applied
        self.repo
            .resolve_suggestion(suggestion_id, "applied")
            .await?;

        Ok(summary)
    }
}

fn require_task_id(task_id: Option<&str>) -> Result<&str> {
    task_id.ok_or_else(|| {
        common::ToolError::ExecutionFailed("task_id required for this action".into()).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_task_id_some() {
        assert_eq!(require_task_id(Some("t1")).unwrap(), "t1");
    }

    #[test]
    fn test_require_task_id_none() {
        assert!(require_task_id(None).is_err());
    }
}
```

Note: The `repo.update_field()` and `repo.remove_dependency()` methods may not exist yet. Check the TaskRepo API. If missing, the implementation should call the appropriate existing methods (e.g., `repo.update()` with a partial update struct). Adapt to the actual repo interface.

- [ ] **Step 2: Update handlers/mod.rs**

Add to `crates/agent/src/handlers/mod.rs`:

```rust
mod suggestion_applier;
pub use suggestion_applier::TaskSuggestionApplier;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(/suggestion/)' 2>&1`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/handlers/suggestion_applier.rs crates/agent/src/handlers/mod.rs
git commit -m "feat(agent): implement TaskSuggestionApplier for executing accepted suggestion actions"
```

---

### Task 8: Proactive tool actions + TaskTool wiring

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/suggest.rs`
- Modify: `crates/feature-tasks/src/tool/actions/mod.rs`
- Modify: `crates/feature-tasks/src/tool/mod.rs`

- [ ] **Step 1: Create suggest tool action**

Create `crates/feature-tasks/src/tool/actions/suggest.rs`:

```rust
//! Suggest action: generate or apply proactive suggestions.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;

impl TaskTool {
    pub(crate) async fn handle_suggest(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.proactive_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Proactive handler not available".into())
        })?;

        let scope = crate::types::SuggestionScope {
            project_id: p.optional_str("project_id")?.map(String::from),
            area_id: p.optional_str("area_id")?.map(String::from),
            tags: vec![],
        };

        info!(?scope, "Generating proactive suggestions");
        let candidates = handler.suggest(&scope).await?;

        if candidates.is_empty() {
            return Ok("No suggestions at this time.".into());
        }

        // Persist suggestions
        for c in &candidates {
            let row = storage::TaskSuggestionRow {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: c.task_id.clone(),
                suggestion_type: format!("{:?}", c.suggestion_type),
                title: c.title.clone(),
                description: c.description.clone(),
                confidence: c.confidence,
                action_payload: serde_json::to_string(&c.action).ok(),
                status: "pending".into(),
                trigger: Some(format!("{:?}", c.trigger)),
                created_at: chrono::Utc::now(),
                resolved_at: None,
            };
            self.repo.create_suggestion(&row).await?;
        }

        // Format response
        let mut output = format!("Generated {} suggestions:\n\n", candidates.len());
        for (i, c) in candidates.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** (confidence: {:.0}%)\n   {}\n",
                i + 1,
                c.title,
                c.confidence * 100.0,
                c.description.as_deref().unwrap_or(""),
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_apply_suggestion(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let applier = self.suggestion_applier.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Suggestion applier not available".into())
        })?;

        let suggestion_id = p.required_str("suggestion_id")?;

        // Load the suggestion
        let suggestions = self.repo.list_pending_suggestions(None).await?;
        let suggestion = suggestions
            .iter()
            .find(|s| s.id == suggestion_id)
            .ok_or_else(|| {
                common::ToolError::ExecutionFailed(
                    format!("Suggestion {suggestion_id} not found or not pending")
                )
            })?;

        let action: crate::types::SuggestionAction = suggestion
            .action_payload
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| common::ToolError::ExecutionFailed(format!("Invalid action: {e}")))?
            .ok_or_else(|| common::ToolError::ExecutionFailed("No action payload".into()))?;

        let summary = applier
            .apply(suggestion_id, suggestion.task_id.as_deref(), &action)
            .await?;

        Ok(format!("Applied suggestion: {summary}"))
    }

    pub(crate) async fn handle_dismiss_suggestion(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let suggestion_id = p.required_str("suggestion_id")?;
        self.repo.resolve_suggestion(suggestion_id, "dismissed").await?;
        Ok(format!("Dismissed suggestion {suggestion_id}"))
    }

    pub(crate) async fn handle_list_suggestions(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let suggestions = self.repo.list_pending_suggestions(None).await?;
        if suggestions.is_empty() {
            return Ok("No pending suggestions.".into());
        }

        let mut output = format!("{} pending suggestions:\n\n", suggestions.len());
        for s in &suggestions {
            output.push_str(&format!(
                "- [{}] **{}** (confidence: {:.0}%)\n  {}\n",
                s.id,
                s.title,
                s.confidence * 100.0,
                s.description.as_deref().unwrap_or(""),
            ));
        }
        Ok(output)
    }
}
```

- [ ] **Step 2: Register module in actions/mod.rs**

Add to `crates/feature-tasks/src/tool/actions/mod.rs`:
```rust
pub mod suggest;
```

- [ ] **Step 3: Add handler fields and builder methods to TaskTool**

In `crates/feature-tasks/src/tool/mod.rs`, add fields to the `TaskTool` struct:
```rust
proactive_handler: Option<Arc<dyn ProactiveHandler>>,
suggestion_applier: Option<Arc<dyn SuggestionApplier>>,
forecast_handler: Option<Arc<dyn ForecastHandler>>,
```

Add builder methods:
```rust
pub fn with_proactive_handler(mut self, handler: Arc<dyn ProactiveHandler>) -> Self {
    self.proactive_handler = Some(handler);
    self
}

pub fn with_suggestion_applier(mut self, applier: Arc<dyn SuggestionApplier>) -> Self {
    self.suggestion_applier = Some(applier);
    self
}

pub fn with_forecast_handler(mut self, handler: Arc<dyn ForecastHandler>) -> Self {
    self.forecast_handler = Some(handler);
    self
}
```

Add action routing in the `execute` match:
```rust
"suggest" => self.handle_suggest(&p).await,
"apply_suggestion" => self.handle_apply_suggestion(&p).await,
"dismiss_suggestion" => self.handle_dismiss_suggestion(&p).await,
"list_suggestions" => self.handle_list_suggestions(&p).await,
```

Add action descriptions to the tool schema's `action` enum and parameter definitions for suggest/apply_suggestion/dismiss_suggestion/list_suggestions.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-tasks 2>&1`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/suggest.rs crates/feature-tasks/src/tool/actions/mod.rs crates/feature-tasks/src/tool/mod.rs
git commit -m "feat(tasks): add suggest/apply/dismiss/list_suggestions tool actions with ProactiveHandler wiring"
```

---

## Chunk 4: ForecastHandler Implementation (3.2)

### Task 9: Forecast prompt template

**Files:**
- Create: `crates/agent/src/templates/forecast_risk.md`

- [ ] **Step 1: Write the forecast risk analysis prompt**

Create `crates/agent/src/templates/forecast_risk.md`:

```markdown
You are analyzing estimation accuracy data and historical patterns to identify risks for a task or project forecast.

## Task/Project
{{ subject }}

## Historical Data
- Sample size: {{ sample_size }} completed tasks
- Data quality: {{ data_quality }}
- Mean deviation: {{ mean_deviation }}%
- Adjusted estimate: {{ adjusted_estimate }} minutes
- Optimistic: {{ optimistic }} minutes
- Pessimistic: {{ pessimistic }} minutes

## Additional Context
{{ context }}

## Instructions

Analyze the data and identify 0-3 risks. For each risk:
1. Classify the kind: HistoricalUnderestimation, DependencyChain, UnknownComplexity, ResourceContention, ExternalDependency
2. Rate severity (0.0-1.0)
3. Provide a brief narrative explaining the risk
4. Suggest mitigation if applicable

Respond in JSON:
```json
{
  "risks": [
    {
      "kind": "HistoricalUnderestimation",
      "severity": 0.7,
      "narrative": "Based on 15 similar tasks, estimates are typically 38% too optimistic.",
      "mitigation": "Add 40% buffer to estimates for this type of work."
    }
  ]
}
```

If no risks, return `{"risks": []}`.
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent/src/templates/forecast_risk.md
git commit -m "feat(agent): add forecast risk analysis prompt template"
```

---

### Task 10: LlmForecastHandler implementation

**Files:**
- Create: `crates/agent/src/handlers/forecast.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Test: inline `#[cfg(test)] mod tests`

The handler wraps the pure L4 `forecast` module for computation and uses LLM for risk narrative generation.

- [ ] **Step 1: Write the handler**

Create `crates/agent/src/handlers/forecast.rs`:

```rust
//! LLM-enhanced forecast handler.
//!
//! Wraps feature_tasks::forecast (pure computation) with LLM-powered
//! risk narrative generation. The L4 module handles similarity matching,
//! deviation correction, and accuracy stats. This L5 handler adds
//! contextual risk analysis.

use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use tracing::debug;

use feature_tasks::forecast::{self as fc, DataQualityTier, EstimationRecord};
use feature_tasks::types::{
    AccuracyReport, AccuracyScope, ForecastContext, ProjectForecast, Task, TaskForecast,
    DataQuality, ForecastRisk, RiskKind,
};
use feature_tasks::ForecastHandler;
use providers::DynProvider;
use storage::TaskRepo;

static RISK_PROMPT: &str = include_str!("../templates/forecast_risk.md");

pub struct LlmForecastHandler {
    provider: DynProvider,
    model: String,
    repo: TaskRepo,
    proactive: Option<Arc<dyn feature_tasks::ProactiveHandler>>,
}

impl LlmForecastHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        repo: TaskRepo,
        proactive: Option<Arc<dyn feature_tasks::ProactiveHandler>>,
    ) -> Self {
        Self { provider, model, repo, proactive }
    }

    /// Load estimation records from storage within lookback window.
    async fn load_records(&self, lookback_days: u32) -> Result<Vec<EstimationRecord>> {
        let rows = self.repo.estimation_stats_raw(lookback_days).await?;
        Ok(rows
            .into_iter()
            .map(|r| EstimationRecord {
                task_id: r.task_id,
                tags: serde_json::from_str(&r.tags).unwrap_or_default(),
                energy_level: r.energy_level,
                complexity_score: r.complexity_score,
                project_id: r.project_id,
                estimated_minutes: r.estimated_minutes,
                actual_minutes: r.actual_minutes,
                completed_at: r.completed_at,
            })
            .collect())
    }

    fn to_data_quality(tier: &DataQualityTier) -> DataQuality {
        match tier {
            DataQualityTier::Strong => DataQuality::High,
            DataQualityTier::Moderate => DataQuality::Medium,
            DataQualityTier::Weak => DataQuality::Low,
            DataQualityTier::Insufficient => DataQuality::Insufficient,
        }
    }

    /// Call LLM for risk analysis (optional — returns empty vec on failure).
    async fn analyze_risks(
        &self,
        subject: &str,
        sample_size: usize,
        data_quality: &str,
        mean_deviation: f64,
        adjusted: f64,
        optimistic: f64,
        pessimistic: f64,
        context: &str,
    ) -> Vec<ForecastRisk> {
        let prompt = RISK_PROMPT
            .replace("{{ subject }}", subject)
            .replace("{{ sample_size }}", &sample_size.to_string())
            .replace("{{ data_quality }}", data_quality)
            .replace("{{ mean_deviation }}", &format!("{mean_deviation:.1}"))
            .replace("{{ adjusted_estimate }}", &format!("{adjusted:.0}"))
            .replace("{{ optimistic }}", &format!("{optimistic:.0}"))
            .replace("{{ pessimistic }}", &format!("{pessimistic:.0}"))
            .replace("{{ context }}", context);

        let response = match self
            .provider
            .chat(&self.model, &[providers::ChatMessage::user(&prompt)], None)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("Risk analysis LLM call failed (non-fatal): {e}");
                return vec![];
            }
        };

        let text = response.text().unwrap_or_default();
        parse_risks(&text)
    }
}

fn parse_risks(response: &str) -> Vec<ForecastRisk> {
    let json_str = response
        .find('{')
        .and_then(|start| response.rfind('}').map(|end| &response[start..=end]))
        .unwrap_or(response);

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    parsed
        .get("risks")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    Some(ForecastRisk {
                        kind: serde_json::from_value(
                            serde_json::Value::String(r.get("kind")?.as_str()?.into()),
                        )
                        .ok()?,
                        severity: r.get("severity")?.as_f64()?,
                        narrative: r.get("narrative")?.as_str()?.to_string(),
                        mitigation: r.get("mitigation").and_then(|m| m.as_str()).map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl ForecastHandler for LlmForecastHandler {
    async fn forecast_task(&self, task: &Task, context: &ForecastContext) -> Result<TaskForecast> {
        let records = self.load_records(context.lookback_days.unwrap_or(90)).await?;
        let now = chrono::Utc::now();

        // Score each record for similarity
        let scored: Vec<(f64, &EstimationRecord)> = records
            .iter()
            .map(|r| {
                let sim = fc::similarity(
                    &task.tags,
                    task.energy_level.as_deref(),
                    task.complexity_score,
                    task.project_id.as_deref(),
                    r,
                    now,
                );
                (sim, r)
            })
            .collect();

        let min_sample = context.min_sample_size.unwrap_or(5) as usize;
        let threshold = if scored.iter().filter(|(s, _)| *s >= 0.3).count() >= min_sample {
            0.3
        } else {
            0.1 // Relaxed threshold if insufficient sample
        };

        let original_est = task.estimated_minutes.unwrap_or(60);
        let correction = fc::deviation_correction(original_est, &scored, threshold);
        let sample_size = correction.as_ref().map_or(0, |c| c.sample_size);
        let quality_tier = DataQualityTier::from_sample_size(sample_size);

        let (adjusted, optimistic, pessimistic, mean_dev, std_dev) = match &correction {
            Some(c) => (
                c.adjusted_estimate,
                c.optimistic,
                c.pessimistic,
                c.mean_deviation,
                c.std_deviation,
            ),
            None => (original_est as f64, original_est as f64, original_est as f64, 0.0, 0.0),
        };

        // LLM risk analysis
        let risks = self
            .analyze_risks(
                &task.title,
                sample_size,
                &format!("{quality_tier:?}"),
                mean_dev * 100.0,
                adjusted,
                optimistic,
                pessimistic,
                "",
            )
            .await;

        // Trigger proactive handler if high-severity risk found
        if let Some(ref proactive) = self.proactive {
            for risk in &risks {
                if risk.severity >= 0.7 {
                    let _ = proactive
                        .evaluate_task(task, &feature_tasks::types::SuggestionTrigger::EstimationDeviation)
                        .await;
                    break;
                }
            }
        }

        Ok(TaskForecast {
            task_id: task.id.clone(),
            original_estimate_mins: Some(original_est),
            adjusted_estimate_mins: adjusted as i32,
            optimistic_mins: optimistic as i32,
            pessimistic_mins: pessimistic as i32,
            confidence_interval: std_dev,
            methodology: feature_tasks::types::ForecastMethodology::HistoricalAverage,
            data_quality: Self::to_data_quality(&quality_tier),
            sample_size: sample_size as u32,
            risks,
        })
    }

    async fn forecast_project(
        &self,
        project_id: &str,
        context: &ForecastContext,
    ) -> Result<ProjectForecast> {
        let records = self.load_records(context.lookback_days.unwrap_or(90)).await?;

        let velocity = fc::project_velocity(&records, chrono::Utc::now(), 4);

        // Load incomplete tasks for project
        let filter = storage::TaskFilter {
            project_id: Some(project_id.into()),
            status: Some("todo".into()),
            ..Default::default()
        };
        let remaining_tasks = self.repo.list(&filter).await?;
        let remaining_mins: i32 = remaining_tasks
            .iter()
            .map(|t| t.estimated_minutes.unwrap_or(60))
            .sum();

        let projected_weeks = velocity.map(|v| {
            if v > 0.0 {
                remaining_mins as f64 / v
            } else {
                0.0
            }
        });

        Ok(ProjectForecast {
            project_id: project_id.to_string(),
            total_tasks: remaining_tasks.len() as u32,
            completed_tasks: records.iter().filter(|r| r.project_id.as_deref() == Some(project_id)).count() as u32,
            velocity_mins_per_week: velocity,
            remaining_mins,
            projected_weeks,
            projected_completion: None, // Could compute from projected_weeks + now
            risks: vec![],
            data_quality: DataQualityTier::from_sample_size(records.len())
                .pipe(|t| Self::to_data_quality(&t)),
        })
    }

    async fn accuracy_stats(&self, scope: &AccuracyScope) -> Result<AccuracyReport> {
        let records = self.load_records(scope.lookback_days.unwrap_or(90)).await?;

        let stats = fc::accuracy_stats(&records);

        Ok(match stats {
            Some(s) => AccuracyReport {
                scope: scope.clone(),
                sample_size: s.count as u32,
                mean_deviation_pct: s.mean_deviation_pct,
                median_deviation_pct: s.median_deviation_pct,
                p90_deviation_pct: s.p90_deviation_pct,
                std_deviation_pct: s.std_deviation_pct,
                trend: feature_tasks::types::AccuracyTrend::Insufficient,
                data_quality: DataQualityTier::from_sample_size(s.count)
                    .pipe(|t| Self::to_data_quality(&t)),
            },
            None => AccuracyReport {
                scope: scope.clone(),
                sample_size: 0,
                mean_deviation_pct: 0.0,
                median_deviation_pct: 0.0,
                p90_deviation_pct: 0.0,
                std_deviation_pct: 0.0,
                trend: feature_tasks::types::AccuracyTrend::Insufficient,
                data_quality: DataQuality::Insufficient,
            },
        })
    }
}

/// Pipe trait for inline transformations.
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_risks_valid() {
        let json = r#"{"risks": [{"kind": "HistoricalUnderestimation", "severity": 0.7, "narrative": "You underestimate", "mitigation": "Add buffer"}]}"#;
        let risks = parse_risks(json);
        assert_eq!(risks.len(), 1);
        assert!((risks[0].severity - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_parse_risks_empty() {
        let json = r#"{"risks": []}"#;
        let risks = parse_risks(json);
        assert!(risks.is_empty());
    }

    #[test]
    fn test_parse_risks_invalid() {
        let risks = parse_risks("not json");
        assert!(risks.is_empty());
    }
}
```

Note: The `estimation_stats_raw()` method on `TaskRepo` may not exist — it should return raw `TaskEstimationRow` records within the lookback window. Check the existing repo interface. The `ProjectForecast` struct fields must match `crates/feature-tasks/src/types.rs`. Adapt field names and types to the actual type definitions.

- [ ] **Step 2: Update handlers/mod.rs**

Add to `crates/agent/src/handlers/mod.rs`:
```rust
mod forecast;
pub use forecast::LlmForecastHandler;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(/forecast/)' 2>&1`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/handlers/forecast.rs crates/agent/src/handlers/mod.rs crates/agent/src/templates/forecast_risk.md
git commit -m "feat(agent): implement LlmForecastHandler wrapping L4 computation with LLM risk narratives"
```

---

### Task 11: Forecast tool actions

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/forecast.rs`
- Modify: `crates/feature-tasks/src/tool/actions/mod.rs`
- Modify: `crates/feature-tasks/src/tool/mod.rs`

- [ ] **Step 1: Create forecast tool action**

Create `crates/feature-tasks/src/tool/actions/forecast.rs`:

```rust
//! Forecast action: generate task/project forecasts and accuracy reports.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::ForecastContext;

impl TaskTool {
    pub(crate) async fn handle_forecast_task(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let id = p.required_str("id")?;
        let task = self.require_task(id).await?;

        let context = ForecastContext {
            min_sample_size: Some(self.config.forecast_min_sample_size),
            lookback_days: Some(self.config.forecast_lookback_days),
            include_subtasks: false,
        };

        info!(task_id = %id, "Forecasting task");
        let forecast = handler.forecast_task(&task, &context).await?;

        Ok(format!(
            "Forecast for '{}' (based on {} similar tasks, quality: {:?}):\n\
             - Original estimate: {}min\n\
             - Adjusted estimate: {}min\n\
             - Optimistic: {}min\n\
             - Pessimistic: {}min\n\
             {}",
            task.title,
            forecast.sample_size,
            forecast.data_quality,
            forecast.original_estimate_mins.unwrap_or(0),
            forecast.adjusted_estimate_mins,
            forecast.optimistic_mins,
            forecast.pessimistic_mins,
            if forecast.risks.is_empty() {
                "No risks identified.".to_string()
            } else {
                format!(
                    "Risks:\n{}",
                    forecast
                        .risks
                        .iter()
                        .map(|r| format!("  - [{:.0}%] {}", r.severity * 100.0, r.narrative))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        ))
    }

    pub(crate) async fn handle_forecast_project(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let project_id = p.required_str("project_id")?;

        let context = ForecastContext {
            min_sample_size: Some(self.config.forecast_min_sample_size),
            lookback_days: Some(self.config.forecast_lookback_days),
            include_subtasks: true,
        };

        info!(project_id, "Forecasting project");
        let forecast = handler.forecast_project(project_id, &context).await?;

        let velocity_str = forecast
            .velocity_mins_per_week
            .map(|v| format!("{v:.0}min/week"))
            .unwrap_or_else(|| "insufficient data".into());

        let projection_str = forecast
            .projected_weeks
            .map(|w| format!("{w:.1} weeks"))
            .unwrap_or_else(|| "cannot project".into());

        Ok(format!(
            "Project forecast:\n\
             - Tasks: {} remaining, {} completed\n\
             - Velocity: {}\n\
             - Remaining: {}min\n\
             - Projected completion: {}",
            forecast.total_tasks,
            forecast.completed_tasks,
            velocity_str,
            forecast.remaining_mins,
            projection_str,
        ))
    }

    pub(crate) async fn handle_accuracy_report(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let scope = crate::types::AccuracyScope {
            project_id: p.optional_str("project_id")?.map(String::from),
            area_id: p.optional_str("area_id")?.map(String::from),
            tags: vec![],
            lookback_days: Some(self.config.forecast_lookback_days),
        };

        let report = handler.accuracy_stats(&scope).await?;

        if report.sample_size == 0 {
            return Ok("No estimation data available yet. Complete some tasks with time tracking to see accuracy stats.".into());
        }

        Ok(format!(
            "Estimation accuracy ({} tasks, {:?}):\n\
             - Mean deviation: {:.1}%\n\
             - Median deviation: {:.1}%\n\
             - P90 deviation: {:.1}%\n\
             - Trend: {:?}",
            report.sample_size,
            report.data_quality,
            report.mean_deviation_pct,
            report.median_deviation_pct,
            report.p90_deviation_pct,
            report.trend,
        ))
    }
}
```

- [ ] **Step 2: Register module and routing**

Add to `crates/feature-tasks/src/tool/actions/mod.rs`:
```rust
pub mod forecast;
```

Add routing in `crates/feature-tasks/src/tool/mod.rs` execute match:
```rust
"forecast_task" => self.handle_forecast_task(&p).await,
"forecast_project" => self.handle_forecast_project(&p).await,
"accuracy_report" => self.handle_accuracy_report(&p).await,
```

Add these actions to the tool schema description.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-tasks 2>&1`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/forecast.rs crates/feature-tasks/src/tool/actions/mod.rs crates/feature-tasks/src/tool/mod.rs
git commit -m "feat(tasks): add forecast_task, forecast_project, accuracy_report tool actions"
```

---

## Chunk 5: Builder Wiring & Final Integration

### Task 12: Wire Phase 3 handlers in AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/src/handlers/mod.rs`

- [ ] **Step 1: Add handler construction in builder**

In `crates/agent/src/agent_loop/builder.rs`, after the Phase 2 handler wiring block (around line 673), add:

```rust
// Phase 3 handlers
let proactive_handler = Arc::new(crate::handlers::LlmProactiveHandler::new(
    provider.clone(),
    config.agents.defaults.model.clone(),
    task_repo_for_handlers.clone(),
    self.domain_event_bus.clone(),
));
task_tool = task_tool.with_proactive_handler(
    proactive_handler.clone() as Arc<dyn feature_tasks::ProactiveHandler>,
);

let suggestion_applier = Arc::new(crate::handlers::TaskSuggestionApplier::new(
    task_repo_for_handlers.clone(),
));
task_tool = task_tool.with_suggestion_applier(
    suggestion_applier as Arc<dyn feature_tasks::SuggestionApplier>,
);

let forecast_handler = Arc::new(crate::handlers::LlmForecastHandler::new(
    provider.clone(),
    config.agents.defaults.model.clone(),
    task_repo_for_handlers.clone(),
    Some(proactive_handler as Arc<dyn feature_tasks::ProactiveHandler>),
));
task_tool = task_tool.with_forecast_handler(
    forecast_handler as Arc<dyn feature_tasks::ForecastHandler>,
);
```

- [ ] **Step 2: Verify final mod.rs exports**

`crates/agent/src/handlers/mod.rs` should export all 6 handlers:

```rust
//! Phase 2-3 handler implementations (L5).

mod decomposition;
mod execution;
mod forecast;
mod planning;
mod proactive;
mod suggestion_applier;

pub use decomposition::LlmDecompositionHandler;
pub use execution::LlmTaskExecutionHandler;
pub use forecast::LlmForecastHandler;
pub use planning::LlmDayPlanningHandler;
pub use proactive::LlmProactiveHandler;
pub use suggestion_applier::TaskSuggestionApplier;
```

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace 2>&1`
Expected: Successful build, no errors

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace 2>&1`
Expected: All tests PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1`
Expected: No new warnings from Phase 3 code

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/agent/src/handlers/mod.rs
git commit -m "feat(agent): wire Phase 3 handlers (Proactive, SuggestionApplier, Forecast) into AgentLoopBuilder"
```

---

### Task 13: Update task agent profile

**Files:**
- Modify: `agents/task/AGENT.md`

- [ ] **Step 1: Add Phase 3 actions to agent profile**

Ensure the task agent's tool descriptions include the new actions. The agent must know these actions exist to call them. Update the `AGENT.md` to document:
- `suggest` — generate proactive suggestions
- `apply_suggestion` — apply a pending suggestion
- `dismiss_suggestion` — dismiss a suggestion
- `list_suggestions` — list pending suggestions
- `forecast_task` — forecast task completion
- `forecast_project` — forecast project timeline
- `accuracy_report` — estimation accuracy stats

- [ ] **Step 2: Commit**

```bash
git add agents/task/AGENT.md
git commit -m "feat(task-agent): document Phase 3 tool actions in agent profile"
```

---

## Implementation Notes

### Critical corrections from review (MUST apply during implementation)

**1. Provider API (affects Tasks 6, 10) — code snippets use wrong API:**
```rust
// CORRECT pattern (from crates/agent/src/handlers/decomposition.rs:79-91):
let params = ChatParams::new(&self.model)
    .with_temperature(0.3)
    .with_max_tokens(4096)
    .with_response_format(ResponseFormat::JsonObject);
let messages = vec![
    Message::system(SYSTEM_PROMPT.to_string()),
    Message::user(prompt),
];
let response = self.provider.chat(&messages, None, &params).await?;
let content = response.content.unwrap_or_default();
let json_str = common::utils::strip_llm_fences(&content);
```
Do NOT use `providers::ChatMessage`, `.text()`, or pass `None` as params.

**2. Type field mismatches (affects Tasks 10, 11) — actual types in `types.rs`:**
- `TaskForecast`: `{ task_id, estimated_minutes: i32, confidence_low: i32, confidence_high: i32, methodology: ForecastMethodology, risks: Vec<ForecastRisk>, data_quality }` — NO `sample_size`, `adjusted_estimate_mins`, `optimistic_mins`, `pessimistic_mins`
- `ProjectForecast`: `{ project_id, total_estimated_minutes: i32, confidence_low: i32, confidence_high: i32, remaining_tasks: u32, completed_tasks: u32, risks, data_quality }` — NO `velocity_mins_per_week`, `projected_weeks`
- `ForecastRisk`: `{ kind: RiskKind, description: String, impact_minutes: Option<i32> }` — NOT `severity`/`narrative`/`mitigation`
- `ForecastMethodology`: struct `{ name: String, sample_size: u32, lookback_days: u32, adjustments: Vec<String> }` — NOT an enum
- `AccuracyReport`: has `by_energy_level: HashMap<String, f64>`, `by_complexity: HashMap<String, f64>`, `trend: AccuracyTrend` — NO `std_deviation_pct`
- `ForecastContext`: `min_sample_size: u32`, `lookback_days: u32` — NOT `Option<u32>`
- `EnergyProfile`: `{ peak_hours: Vec<u32>, low_energy_hours: Vec<u32>, avg_focus_duration_mins: Option<u32>, preferred_task_size_mins: Option<u32> }` — NOT `Option<String>` peak
- `EstimationRecord` already exists in `types.rs:1399` with `From<TaskEstimationRow>` impl — reuse it, don't redefine in `forecast.rs`

**3. Cognitive bridge layer violation (affects Task 3):**
`feature-tasks` is L4, `cognitive` is L5. Cannot import `cognitive::types::SemanticFact`. Define a minimal local `CognitiveFact` struct in `cognitive_bridge.rs`:
```rust
pub struct CognitiveFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}
```
Convert from `SemanticFact` at the L5 call site where both types are available.

**4. Missing repo methods (affects Tasks 7, 10):**
- NO `update_field()` — use `repo.update(&TaskRow)` or specific update methods
- NO `estimation_stats_raw()` — must add a method returning `Vec<TaskEstimationRow>` within lookback window
- NO `remove_dependency()` — check actual dependency management API in `task_repo.rs`

**5. Missing spec requirements (add as implementation steps):**
- **Suggestion auto-expiration subscriber** (spec 3.1): DomainEventBus subscriber watches TaskCompleted/TaskUpdated/deleted → calls `repo.expire_suggestions_for_task()`. Add to Chunk 5.
- **Full trigger coverage in `suggest()`** (spec 3.1): Must scan for stale tasks, WIP limit exceeded, blocked chains — not just overdue.
- **TaskExecutionFailed conditional salience** (spec 3.3): 1st failure = Accumulate, 3+ = Extract (requires counting failures per task_id).

**6. Minor corrections:**
- `Task::from(row.clone())` not `row.into()` when iterating `&tasks`
- Use `const` not `static` for prompt templates (Phase 2 pattern)
- `task.energy_level` is `Option<EnergyLevel>` (enum), not `Option<String>` — use `.as_ref().map(|e| e.to_string())`

### Testing strategy
- **L4 modules** (cognitive_bridge, forecast): Pure unit tests, no mocks needed
- **L5 handlers** (proactive, forecast, suggestion_applier): Test prompt building and JSON parsing. Follow patterns from `crates/agent/src/handlers/decomposition.rs`
- **Tool actions**: Tested through existing `TaskTool` test harness

### Order of execution
1. Chunk 1 (Cognitive) — can be built immediately, no dependencies
2. Chunk 2 (Forecast L4) — can be built in parallel with Chunk 1 (both modify `lib.rs` — needs merge if parallel)
3. Chunk 3 (Proactive) — needs prompt template only
4. Chunk 4 (Forecast L5) — needs Chunk 2 complete
5. Chunk 5 (Wiring) — needs all chunks complete
