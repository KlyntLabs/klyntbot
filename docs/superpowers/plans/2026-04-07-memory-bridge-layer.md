# Memory Bridge Layer (SP1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect Klynt's 11 isolated memory subsystems through a unified 3-stage cognitive pipeline (Collectors → Consolidator → Writers) with configurable Standard/Deep intelligence modes.

**Architecture:** Five event-driven collectors normalize signals from chat, sessions, atoms, coaching, and conversation recalls into a unified `CognitiveSignal` type, batched into a consolidator that groups by subject overlap, computes multi-source convergence, and promotes to facts/rules/episodes. A `convergence_score` column on semantic facts enables the `cross_note_boost` retrieval scoring factor. The LLM generates `[@type:id]` markers that the frontend renders as hoverable inline references.

**Tech Stack:** Rust, tokio (mpsc channels), SQLite, LanceDB, React (remark plugin), Tauri commands

**Spec:** `docs/superpowers/specs/2026-04-07-memory-unification-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/config/src/schema/cognitive.rs` | Modify | Add `IntelligenceMode` enum and field |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Modify | Add `convergence_score` column to `semantic_facts` |
| `crates/cognitive/src/types.rs` | Modify | Add `convergence_score` field to `SemanticFact` |
| `crates/cognitive/src/repos/semantic_fact.rs` | Modify | Handle `convergence_score` in queries |
| `crates/cognitive/src/pipeline/mod.rs` | Create | Pipeline module root |
| `crates/cognitive/src/pipeline/signal.rs` | Create | `CognitiveSignal`, `SignalSource`, `SignalContext` types |
| `crates/cognitive/src/pipeline/collector.rs` | Create | Collector trait + `SignalSender`/`SignalReceiver` |
| `crates/cognitive/src/pipeline/session_collector.rs` | Create | `SessionCollector` |
| `crates/cognitive/src/pipeline/atom_collector.rs` | Create | `AtomCollector` |
| `crates/cognitive/src/pipeline/coaching_collector.rs` | Create | `CoachingCollector` |
| `crates/cognitive/src/pipeline/consolidator.rs` | Create | Grouping, convergence scoring, promotion decisions |
| `crates/cognitive/src/pipeline/writer.rs` | Create | Execute `PromotionOp`s against repos |
| `crates/cognitive/src/services/background.rs` | Modify | Wire collectors and consolidator into event loop |
| `crates/cognitive/src/services/context_source.rs` | Modify | Add freshness labels to user model and rules |
| `crates/cognitive/src/services/memory_retriever.rs` | Modify | Add `[@type:id]` markers and freshness to formatted output |
| `crates/bus/src/domain_events.rs` | Modify | Add fields to `AtomReinforced` variant |
| `crates/cognitive/src/services/atom_extraction.rs` | Modify | Publish enriched `AtomReinforced` with subject/domain |
| `crates/desktop-shared/src/commands/memory.rs` | Create | `MemoryReferenceDetail` response type |
| `crates/desktop/src/commands/memory.rs` | Create | `memory_reference_detail` Tauri command |
| `crates/app-core/src/handlers/memory.rs` | Create | `AppCore::memory_reference_detail()` handler |
| `desktop-ui/src/shared/ui/MemoryReference.tsx` | Create | Inline reference component with tooltip |
| `desktop-ui/src/features/chat/plugins/memoryRefPlugin.ts` | Create | Remark plugin to parse `[@type:id]` markers |
| `desktop-ui/src/features/chat/components/MarkdownContent.tsx` | Modify | Register memory reference remark plugin |
| `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx` | Modify | Add IntelligenceMode toggle |

---

### Task 1: IntelligenceMode Config + convergence_score Schema

Add the intelligence mode configuration and the convergence_score column to semantic_facts.

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/types.rs`
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`

- [ ] **Step 1: Add `IntelligenceMode` enum to cognitive config**

In `crates/config/src/schema/cognitive.rs`, add the enum before `CognitiveConfig`:

```rust
/// Intelligence processing depth for the cognitive pipeline.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum IntelligenceMode {
    #[default]
    Standard,
    Deep,
}
```

Add the field to `CognitiveConfig` struct:

```rust
    /// Intelligence mode: "standard" (heuristic-first) or "deep" (full LLM).
    #[serde(default)]
    pub intelligence_mode: IntelligenceMode,
```

Add to the `Default` impl:

```rust
    intelligence_mode: IntelligenceMode::Standard,
```

- [ ] **Step 2: Add `convergence_score` column to semantic_facts table**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, find the `semantic_facts` CREATE TABLE and add after `access_count`:

```sql
    convergence_score REAL NOT NULL DEFAULT 0.0,
```

- [ ] **Step 3: Add `convergence_score` field to `SemanticFact` struct**

In `crates/cognitive/src/types.rs`, add to `SemanticFact` after `access_count`:

```rust
    pub convergence_score: f64,
```

- [ ] **Step 4: Update `SemanticFactRepo` queries to include convergence_score**

In `crates/cognitive/src/repos/semantic_fact.rs`, update the `upsert()` INSERT query to include `convergence_score` in the column list and add `.bind(fact.convergence_score)`. Add `convergence_score = excluded.convergence_score` to the ON CONFLICT UPDATE clause.

Add two new methods:

```rust
    pub async fn update_convergence(&self, id: &str, convergence: f64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE semantic_facts SET convergence_score = ?1 WHERE id = ?2")
            .bind(convergence)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_confidence(&self, id: &str, confidence: f64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE semantic_facts SET confidence = ?1 WHERE id = ?2")
            .bind(confidence)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
```

- [ ] **Step 5: Fix all test SemanticFact struct literals**

Search and add `convergence_score: 0.0` to every `SemanticFact { ... }` test literal:

```bash
grep -rn "SemanticFact {" crates/ tests/
```

- [ ] **Step 6: Build and test**

```bash
cargo build -p config -p cognitive 2>&1 | tail -20
cargo nextest run -p cognitive -p config --no-fail-fast 2>&1 | tail -20
```

- [ ] **Step 7: Commit**

```bash
git add crates/config/src/schema/cognitive.rs crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/types.rs crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): add IntelligenceMode config and convergence_score column

Adds Standard/Deep intelligence mode to CognitiveConfig. Adds
convergence_score column to semantic_facts for multi-source convergence
tracking in retrieval scoring."
```

---

### Task 2: CognitiveSignal Type + Pipeline Module

Create the pipeline module with the common signal type that all collectors produce.

**Files:**
- Create: `crates/cognitive/src/pipeline/mod.rs`
- Create: `crates/cognitive/src/pipeline/signal.rs`
- Create: `crates/cognitive/src/pipeline/collector.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Create `crates/cognitive/src/pipeline/signal.rs`**

```rust
//! Common signal type produced by all collectors and consumed by the consolidator.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CognitiveSignal {
    pub source: SignalSource,
    pub content: String,
    pub domain: String,
    pub confidence: f64,
    pub context: SignalContext,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalSource {
    ChatTurn,
    SessionEnd,
    AtomReinforcement,
    CoachingPattern,
    ConversationRecall,
    UserStatedFact,
}

#[derive(Debug, Clone, Default)]
pub struct SignalContext {
    pub session_key: Option<String>,
    pub related_fact_ids: Vec<String>,
    pub related_atom_ids: Vec<String>,
    pub source_count: u32,
    pub raw_observations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_source_equality() {
        assert_eq!(SignalSource::ChatTurn, SignalSource::ChatTurn);
        assert_ne!(SignalSource::ChatTurn, SignalSource::SessionEnd);
    }

    #[test]
    fn test_signal_construction() {
        let signal = CognitiveSignal {
            source: SignalSource::ChatTurn,
            content: "User is a software engineer".into(),
            domain: "identity".into(),
            confidence: 0.8,
            context: SignalContext {
                session_key: Some("sess_1".into()),
                source_count: 1,
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        assert_eq!(signal.confidence, 0.8);
        assert_eq!(signal.context.source_count, 1);
    }
}
```

- [ ] **Step 2: Create `crates/cognitive/src/pipeline/collector.rs`**

```rust
//! Signal queue types for the unified pipeline.

use tokio::sync::mpsc;
use super::signal::CognitiveSignal;

pub type SignalSender = mpsc::Sender<CognitiveSignal>;
pub type SignalReceiver = mpsc::Receiver<CognitiveSignal>;

pub fn signal_queue(capacity: usize) -> (SignalSender, SignalReceiver) {
    mpsc::channel(capacity)
}
```

- [ ] **Step 3: Create `crates/cognitive/src/pipeline/mod.rs`**

```rust
//! Unified cognitive pipeline: Collectors -> Consolidator -> Writers.

pub mod collector;
pub mod signal;

pub use collector::{signal_queue, SignalReceiver, SignalSender};
pub use signal::{CognitiveSignal, SignalContext, SignalSource};
```

- [ ] **Step 4: Register in `crates/cognitive/src/lib.rs`**

Add `pub mod pipeline;`

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(signal)' --no-fail-fast 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/pipeline/ crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add CognitiveSignal type and pipeline module

Common signal type for all collectors. Includes SignalSource enum and
SignalSender/SignalReceiver queue types."
```

---

### Task 3: Enrich AtomReinforced Event

Add subject, domain, and reinforcement_count to the existing `AtomReinforced` domain event.

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/cognitive/src/services/atom_extraction.rs`

- [ ] **Step 1: Add fields to `AtomReinforced` variant**

In `crates/bus/src/domain_events.rs`, find the `AtomReinforced` variant and add fields:

```rust
    AtomReinforced {
        atom_id: String,
        referencing_note_id: String,
        new_salience: f64,
        subject: String,
        domain: String,
        reinforcement_count: i64,
    },
```

- [ ] **Step 2: Fix all match sites**

```bash
grep -rn "AtomReinforced" crates/
```

Update every destructure to include the new fields (add `subject, domain, reinforcement_count` or use `..`).

- [ ] **Step 3: Update atom extraction to publish enriched event**

In `crates/cognitive/src/services/atom_extraction.rs`, update the `AtomReinforced` publish to include the new fields:

```rust
bus.publish(DomainEvent::AtomReinforced {
    atom_id: target.id.clone(),
    referencing_note_id: note_id.to_string(),
    new_salience,
    subject: atom.subject.clone(),
    domain: atom.domain.clone().unwrap_or_else(|| "general".into()),
    reinforcement_count: 1, // increment from current; read from atom if available
});
```

- [ ] **Step 4: Build and test**

```bash
cargo build --workspace 2>&1 | tail -20
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 5: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/cognitive/src/services/atom_extraction.rs
git commit -m "feat(bus): enrich AtomReinforced with subject, domain, reinforcement_count

AtomReinforced now carries the atom's subject, domain, and reinforcement
count so the AtomCollector can make promotion decisions without querying
the atom repo."
```

---

### Task 4: SessionCollector

Subscribe to `SessionEnded` events and extract insights from the session memory scratchpad.

**Files:**
- Create: `crates/cognitive/src/pipeline/session_collector.rs`
- Modify: `crates/cognitive/src/pipeline/mod.rs`

- [ ] **Step 1: Create `crates/cognitive/src/pipeline/session_collector.rs`**

```rust
//! Collects knowledge signals from ended sessions by reading session scratchpads.

use chrono::Utc;
use storage::SessionMemoryRepo;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const INSIGHT_KEYWORDS: &[&str] = &[
    "learned", "decided", "error", "fixed", "important", "remember",
    "preference", "realized", "discovered", "pattern", "issue", "solution",
    "goal", "plan", "need", "want", "struggle", "improve",
];
const MIN_SCRATCHPAD_LEN: usize = 50;

pub struct SessionCollector;

impl SessionCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        memory_repo: SessionMemoryRepo,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::SessionEnded { session_id, .. }) => {
                                Self::handle(&session_id, &memory_repo, &signal_tx).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("SessionCollector lagged by {n} events");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("SessionCollector stopped");
        })
    }

    async fn handle(session_key: &str, repo: &SessionMemoryRepo, tx: &SignalSender) {
        let scratchpad = match repo.get(session_key).await {
            Ok(Some(c)) => c,
            _ => return,
        };
        if scratchpad.len() < MIN_SCRATCHPAD_LEN {
            return;
        }
        let insights = extract_insight_sentences(&scratchpad);
        if insights.is_empty() {
            return;
        }
        let confidence = keyword_confidence(&insights);
        let signal = CognitiveSignal {
            source: SignalSource::SessionEnd,
            content: insights.join(" "),
            domain: "general".into(),
            confidence,
            context: SignalContext {
                session_key: Some(session_key.to_string()),
                raw_observations: vec![scratchpad],
                source_count: 1,
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        let _ = tx.send(signal).await;
    }
}

fn extract_insight_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '\n'])
        .filter(|s| {
            let sl = s.to_lowercase();
            INSIGHT_KEYWORDS.iter().any(|kw| sl.contains(kw))
        })
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() > 10)
        .collect()
}

fn keyword_confidence(insights: &[String]) -> f64 {
    let unique: std::collections::HashSet<&&str> = insights
        .iter()
        .flat_map(|s| {
            let lower = s.to_lowercase();
            INSIGHT_KEYWORDS.iter().filter(move |kw| lower.contains(**kw))
        })
        .collect();
    (0.5 + unique.len() as f64 / INSIGHT_KEYWORDS.len() as f64 * 0.3).min(0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_insight_sentences() {
        let text = "User asked about Rust. Learned that async requires tokio. Fixed the build error. Had lunch.";
        let insights = extract_insight_sentences(text);
        assert_eq!(insights.len(), 2);
    }

    #[test]
    fn test_no_insights_from_mundane_text() {
        let insights = extract_insight_sentences("The weather is nice today. Goodbye.");
        assert!(insights.is_empty());
    }

    #[test]
    fn test_keyword_confidence_range() {
        let insights = vec!["Learned something".into(), "Fixed a bug".into()];
        let conf = keyword_confidence(&insights);
        assert!(conf >= 0.5 && conf <= 0.8);
    }
}
```

- [ ] **Step 2: Register in pipeline module**

Add to `crates/cognitive/src/pipeline/mod.rs`:

```rust
pub mod session_collector;
pub use session_collector::SessionCollector;
```

- [ ] **Step 3: Build and test**

```bash
cargo nextest run -p cognitive -E 'test(session_collector)' --no-fail-fast 2>&1
```

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/pipeline/session_collector.rs crates/cognitive/src/pipeline/mod.rs
git commit -m "feat(cognitive): add SessionCollector for session->memory bridge

Subscribes to SessionEnded events, reads session scratchpad, extracts
insight sentences via keyword heuristic, sends CognitiveSignals to the
unified pipeline queue."
```

---

### Task 5: AtomCollector + CoachingCollector

Two small collectors that subscribe to existing events.

**Files:**
- Create: `crates/cognitive/src/pipeline/atom_collector.rs`
- Create: `crates/cognitive/src/pipeline/coaching_collector.rs`
- Modify: `crates/cognitive/src/pipeline/mod.rs`

- [ ] **Step 1: Create `crates/cognitive/src/pipeline/atom_collector.rs`**

```rust
//! Collects signals from cross-note atom reinforcement.

use chrono::Utc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const MIN_REINFORCEMENT: i64 = 2;

pub struct AtomCollector;

impl AtomCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::AtomReinforced {
                                atom_id, subject, domain, reinforcement_count, ..
                            }) if reinforcement_count >= MIN_REINFORCEMENT => {
                                let confidence = (0.5 + reinforcement_count as f64 * 0.15).min(0.95);
                                let signal = CognitiveSignal {
                                    source: SignalSource::AtomReinforcement,
                                    content: subject,
                                    domain,
                                    confidence,
                                    context: SignalContext {
                                        related_atom_ids: vec![atom_id],
                                        source_count: reinforcement_count as u32,
                                        ..Default::default()
                                    },
                                    timestamp: Utc::now(),
                                };
                                let _ = signal_tx.send(signal).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("AtomCollector lagged {n}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("AtomCollector stopped");
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_confidence_scaling() {
        let conf = |n: i64| (0.5 + n as f64 * 0.15).min(0.95);
        assert!((conf(2) - 0.80).abs() < 0.01);
        assert!((conf(3) - 0.95).abs() < 0.01);
        assert!((conf(10) - 0.95).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Create `crates/cognitive/src/pipeline/coaching_collector.rs`**

```rust
//! Collects signals from coaching pattern detection.

use chrono::Utc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

pub struct CoachingCollector;

impl CoachingCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::CoachingPatternDetected {
                                pattern_name, confidence, description, domain, signal_count, ..
                            }) => {
                                let rule_text = pattern_to_rule(&pattern_name, &description);
                                let signal = CognitiveSignal {
                                    source: SignalSource::CoachingPattern,
                                    content: rule_text,
                                    domain,
                                    confidence,
                                    context: SignalContext {
                                        source_count: signal_count as u32,
                                        ..Default::default()
                                    },
                                    timestamp: Utc::now(),
                                };
                                let _ = signal_tx.send(signal).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("CoachingCollector lagged {n}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("CoachingCollector stopped");
        })
    }
}

fn pattern_to_rule(name: &str, description: &str) -> String {
    match name {
        "afternoon_energy_drop" => "Schedule demanding tasks in the morning; take breaks in the afternoon when energy drops".into(),
        "chronic_task_avoidance" => "Break avoided tasks into smaller steps to overcome procrastination".into(),
        "habitual_context_switching" => "Batch similar tasks together to reduce context switching overhead".into(),
        "declining_focus_quality" => "Take a break when focus quality starts declining".into(),
        "recurring_budget_pressure" => "Review spending patterns when budget pressure is detected".into(),
        "study_streak_at_risk" => "Complete at least one review session to maintain the study streak".into(),
        "retention_decay_detected" => "Schedule review sessions for domains with declining retention".into(),
        "learning_momentum_create_heavy" => "Balance content creation with review sessions to avoid review backlog".into(),
        _ => description.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_pattern() {
        let text = pattern_to_rule("afternoon_energy_drop", "");
        assert!(text.contains("morning"));
    }

    #[test]
    fn test_unknown_pattern_uses_description() {
        assert_eq!(pattern_to_rule("unknown", "Custom description"), "Custom description");
    }
}
```

- [ ] **Step 3: Check `CoachingPatternDetected` event variant exists**

```bash
grep -rn "CoachingPatternDetected" crates/bus/src/domain_events.rs
```

If the variant has different field names, update the match arm to match. If the variant doesn't exist, add it to the `DomainEvent` enum with fields: `pattern_name: String, confidence: f64, description: String, domain: String, signal_count: i32`.

- [ ] **Step 4: Register both in pipeline module**

Add to `crates/cognitive/src/pipeline/mod.rs`:

```rust
pub mod atom_collector;
pub mod coaching_collector;
pub use atom_collector::AtomCollector;
pub use coaching_collector::CoachingCollector;
```

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(atom_co) or test(coaching_co) or test(confidence_scaling) or test(known_pattern) or test(unknown_pattern)' --no-fail-fast 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/pipeline/atom_collector.rs crates/cognitive/src/pipeline/coaching_collector.rs crates/cognitive/src/pipeline/mod.rs
git commit -m "feat(cognitive): add AtomCollector and CoachingCollector

AtomCollector promotes atoms reinforced 2+ times across notes.
CoachingCollector converts detected patterns into rule-candidate signals."
```

---

### Task 6: Consolidator (Grouping + Heuristic Promotion)

Groups signals by subject overlap, computes convergence, and decides promotions.

**Files:**
- Create: `crates/cognitive/src/pipeline/consolidator.rs`
- Modify: `crates/cognitive/src/repos/procedural_rule.rs`
- Modify: `crates/cognitive/src/pipeline/mod.rs`

- [ ] **Step 1: Make `word_overlap_ratio` public**

In `crates/cognitive/src/repos/procedural_rule.rs`, change `fn word_overlap_ratio` to `pub fn word_overlap_ratio`.

- [ ] **Step 2: Create `crates/cognitive/src/pipeline/consolidator.rs`**

```rust
//! Stage 2: groups signals, computes convergence, decides promotions.

use std::collections::HashSet;
use tracing::{debug, info};

use crate::repos::procedural_rule::word_overlap_ratio;
use super::signal::{CognitiveSignal, SignalSource};

const GROUPING_THRESHOLD: f64 = 0.4;

#[derive(Debug, Clone)]
pub struct KnowledgeCluster {
    pub signals: Vec<CognitiveSignal>,
    pub merged_subject: String,
    pub domain: String,
    pub source_diversity: u32,
    pub convergence_score: f64,
    pub max_confidence: f64,
    pub combined_observations: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PromotionOp {
    CreateFact {
        subject: String,
        predicate: String,
        object: String,
        domain: String,
        confidence: f64,
        convergence: f64,
        source: String,
    },
    CreateRule {
        rule_text: String,
        domain: String,
        confidence: f64,
    },
    CreateEpisode {
        content: String,
        summary: String,
        domain: String,
        importance: f64,
    },
}

pub fn group_signals(signals: Vec<CognitiveSignal>) -> Vec<KnowledgeCluster> {
    let mut clusters: Vec<KnowledgeCluster> = Vec::new();
    for signal in signals {
        let mut merged = false;
        for cluster in &mut clusters {
            if cluster.domain == signal.domain
                && word_overlap_ratio(&cluster.merged_subject, &signal.content) > GROUPING_THRESHOLD
            {
                if signal.confidence > cluster.max_confidence {
                    cluster.merged_subject = signal.content.clone();
                    cluster.max_confidence = signal.confidence;
                }
                cluster.combined_observations.extend(signal.context.raw_observations.clone());
                cluster.signals.push(signal);
                let sources: HashSet<SignalSource> = cluster.signals.iter().map(|s| s.source).collect();
                cluster.source_diversity = sources.len() as u32;
                cluster.convergence_score = cluster.source_diversity as f64 / 5.0;
                merged = true;
                break;
            }
        }
        if !merged {
            clusters.push(KnowledgeCluster {
                merged_subject: signal.content.clone(),
                domain: signal.domain.clone(),
                source_diversity: 1,
                convergence_score: 0.2,
                max_confidence: signal.confidence,
                combined_observations: signal.context.raw_observations.clone(),
                signals: vec![signal],
            });
        }
    }
    clusters
}

pub fn heuristic_promote(clusters: &[KnowledgeCluster]) -> Vec<PromotionOp> {
    let mut ops = Vec::new();
    for cluster in clusters {
        let has_coaching = cluster.signals.iter().any(|s| s.source == SignalSource::CoachingPattern);

        if has_coaching && cluster.max_confidence >= 0.7 {
            ops.push(PromotionOp::CreateRule {
                rule_text: cluster.merged_subject.clone(),
                domain: cluster.domain.clone(),
                confidence: cluster.max_confidence,
            });
        } else if cluster.max_confidence >= 0.6 || cluster.convergence_score >= 0.4 {
            let (subject, predicate, object) = extract_spo(&cluster.merged_subject);
            ops.push(PromotionOp::CreateFact {
                subject, predicate, object,
                domain: cluster.domain.clone(),
                confidence: cluster.max_confidence,
                convergence: cluster.convergence_score,
                source: promotion_source(&cluster.signals),
            });
        } else if cluster.max_confidence >= 0.5 {
            let summary = if cluster.merged_subject.len() > 120 {
                format!("{}...", &cluster.merged_subject[..117])
            } else {
                cluster.merged_subject.clone()
            };
            ops.push(PromotionOp::CreateEpisode {
                content: cluster.merged_subject.clone(),
                summary, domain: cluster.domain.clone(),
                importance: cluster.max_confidence,
            });
        }
    }
    info!("Consolidator: {} ops from {} clusters", ops.len(), clusters.len());
    ops
}

fn extract_spo(text: &str) -> (String, String, String) {
    for pred in ["is a", "is", "has", "prefers", "uses", "works", "likes", "wants", "needs"] {
        if let Some(idx) = text.to_lowercase().find(pred) {
            let subject = text[..idx].trim().to_string();
            let object = text[idx + pred.len()..].trim().to_string();
            if !subject.is_empty() && !object.is_empty() {
                return (subject, pred.to_string(), object);
            }
        }
    }
    ("user".into(), "noted".into(), text.to_string())
}

fn promotion_source(signals: &[CognitiveSignal]) -> String {
    let mut sources: Vec<&str> = signals.iter().map(|s| match s.source {
        SignalSource::ChatTurn => "chat",
        SignalSource::SessionEnd => "session",
        SignalSource::AtomReinforcement => "notes",
        SignalSource::CoachingPattern => "coaching",
        SignalSource::ConversationRecall => "recall",
        SignalSource::UserStatedFact => "user_stated",
    }).collect::<HashSet<_>>().into_iter().collect();
    sources.sort();
    sources.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::pipeline::signal::SignalContext;

    fn sig(source: SignalSource, content: &str, domain: &str, confidence: f64) -> CognitiveSignal {
        CognitiveSignal {
            source, content: content.into(), domain: domain.into(), confidence,
            context: SignalContext::default(), timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_group_similar() {
        let signals = vec![
            sig(SignalSource::ChatTurn, "User is learning Rust programming", "learning", 0.7),
            sig(SignalSource::AtomReinforcement, "Rust programming language concepts", "learning", 0.8),
        ];
        let clusters = group_signals(signals);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].source_diversity, 2);
    }

    #[test]
    fn test_group_dissimilar() {
        let clusters = group_signals(vec![
            sig(SignalSource::ChatTurn, "User is learning Rust", "learning", 0.7),
            sig(SignalSource::CoachingPattern, "Take breaks in the afternoon", "productivity", 0.8),
        ]);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn test_promote_fact() {
        let clusters = group_signals(vec![
            sig(SignalSource::ChatTurn, "Jayden is a software engineer", "identity", 0.8),
        ]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateFact { subject, .. } if subject == "Jayden"));
    }

    #[test]
    fn test_promote_rule() {
        let clusters = group_signals(vec![
            sig(SignalSource::CoachingPattern, "Schedule tasks in the morning", "productivity", 0.8),
        ]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateRule { .. }));
    }

    #[test]
    fn test_promote_episode() {
        let clusters = group_signals(vec![
            sig(SignalSource::SessionEnd, "Fixed a tricky async bug in middleware", "general", 0.55),
        ]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateEpisode { .. }));
    }

    #[test]
    fn test_extract_spo() {
        let (s, p, o) = extract_spo("Jayden is a software engineer");
        assert_eq!(s, "Jayden");
        assert_eq!(p, "is a");
        assert_eq!(o, "software engineer");
    }

    #[test]
    fn test_convergence_multi_source() {
        let clusters = group_signals(vec![
            sig(SignalSource::ChatTurn, "User is learning Rust", "learning", 0.6),
            sig(SignalSource::AtomReinforcement, "Learning Rust programming", "learning", 0.7),
            sig(SignalSource::CoachingPattern, "Learning momentum Rust is strong", "learning", 0.8),
        ]);
        assert_eq!(clusters.len(), 1);
        assert!((clusters[0].convergence_score - 0.6).abs() < 0.01);
    }
}
```

- [ ] **Step 3: Register in pipeline module**

Add to `crates/cognitive/src/pipeline/mod.rs`:

```rust
pub mod consolidator;
pub use consolidator::{group_signals, heuristic_promote, KnowledgeCluster, PromotionOp};
```

- [ ] **Step 4: Build and test**

```bash
cargo nextest run -p cognitive -E 'test(group_) or test(promote_) or test(extract_spo) or test(convergence_)' --no-fail-fast 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/pipeline/consolidator.rs crates/cognitive/src/pipeline/mod.rs crates/cognitive/src/repos/procedural_rule.rs
git commit -m "feat(cognitive): add consolidator with grouping and heuristic promotion

Groups signals by word overlap (Jaccard > 0.4), computes convergence
(source_diversity / 5), and promotes: coaching->rules, high-confidence
->facts, moderate->episodes."
```

---

### Task 7: Pipeline Writer

Executes PromotionOps against the repos with deduplication.

**Files:**
- Create: `crates/cognitive/src/pipeline/writer.rs`
- Modify: `crates/cognitive/src/pipeline/mod.rs`

- [ ] **Step 1: Create `crates/cognitive/src/pipeline/writer.rs`**

```rust
//! Stage 3: executes PromotionOps against repos.

use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::embedder::SemanticFactEmbedder;
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact};
use super::consolidator::PromotionOp;

pub async fn execute_promotions(
    ops: &[PromotionOp],
    fact_repo: &SemanticFactRepo,
    rule_repo: &ProceduralRuleRepo,
    episodic_repo: &Option<EpisodicMemoryRepo>,
    embedder: Option<&dyn SemanticFactEmbedder>,
) {
    let (mut facts, mut rules, mut episodes) = (0u32, 0u32, 0u32);

    for op in ops {
        match op {
            PromotionOp::CreateFact { subject, predicate, object, domain, confidence, convergence, source } => {
                match fact_repo.find_similar(subject, predicate).await {
                    Ok(existing) if !existing.is_empty() => {
                        let best = &existing[0];
                        let _ = fact_repo.update_confidence(&best.id, best.confidence.max(*confidence)).await;
                        let _ = fact_repo.update_convergence(&best.id, *convergence).await;
                        debug!("Writer: reinforced fact '{}'", best.id);
                    }
                    _ => {
                        let now = Utc::now().to_rfc3339();
                        let fact = SemanticFact {
                            id: Uuid::new_v4().to_string(),
                            domain: domain.clone(), subject: subject.clone(),
                            predicate: predicate.clone(), object: object.clone(),
                            confidence: *confidence, source: source.clone(),
                            valid_from: now.clone(), valid_until: None,
                            recorded_at: now.clone(), superseded_at: None,
                            superseded_by: None, stability: 1.0,
                            last_accessed: Some(now), access_count: 0,
                            convergence_score: *convergence,
                            project_id: None, memory_type: "fact".into(),
                            scope_type: "system".into(), scope_id: None,
                        };
                        if let Err(e) = fact_repo.upsert(&fact).await {
                            warn!("Writer: failed to create fact: {e}");
                        } else {
                            if let Some(emb) = embedder {
                                let text = format!("{}: {} = {}", subject, predicate, object);
                                let _ = emb.embed_fact(&fact.id, &text, domain, *confidence, 1.0).await;
                            }
                            facts += 1;
                        }
                    }
                }
            }
            PromotionOp::CreateRule { rule_text, domain, confidence } => {
                match rule_repo.find_similar(rule_text, domain).await {
                    Ok(Some(existing)) => {
                        let _ = rule_repo.increment_signal_count(&existing.id).await;
                        debug!("Writer: reinforced rule '{}'", existing.id);
                    }
                    _ => {
                        let now = Utc::now().to_rfc3339();
                        let rule = ProceduralRule {
                            id: Uuid::new_v4().to_string(), domain: domain.clone(),
                            rule_text: rule_text.clone(), confidence: *confidence,
                            source: "pipeline".into(), signal_count: 1,
                            created_at: now.clone(), updated_at: now,
                            active: true, project_id: None,
                            scope_type: "system".into(), scope_id: None,
                        };
                        if let Err(e) = rule_repo.upsert(&rule).await {
                            warn!("Writer: failed to create rule: {e}");
                        } else { rules += 1; }
                    }
                }
            }
            PromotionOp::CreateEpisode { content, summary, domain, importance } => {
                if let Some(ep_repo) = episodic_repo {
                    let now = Utc::now().to_rfc3339();
                    let memory = EpisodicMemory {
                        id: Uuid::new_v4().to_string(), domain: domain.clone(),
                        content: content.clone(), summary: Some(summary.clone()),
                        importance: *importance, occurred_at: now.clone(),
                        recorded_at: now, stability: 1.0,
                        last_accessed: None, access_count: 0,
                        project_id: None, scope_type: "system".into(), scope_id: None,
                    };
                    if let Err(e) = ep_repo.insert(&memory).await {
                        warn!("Writer: failed to create episode: {e}");
                    } else { episodes += 1; }
                }
            }
        }
    }
    info!("Writer: {facts} facts, {rules} rules, {episodes} episodes");
}
```

- [ ] **Step 2: Register in pipeline module**

Add to `crates/cognitive/src/pipeline/mod.rs`:

```rust
pub mod writer;
pub use writer::execute_promotions;
```

- [ ] **Step 3: Build**

```bash
cargo build -p cognitive 2>&1 | tail -20
```

Fix any mismatches with the `embed_fact` method signature — check `SemanticFactEmbedder` trait and adapt the call parameters.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/pipeline/writer.rs crates/cognitive/src/pipeline/mod.rs
git commit -m "feat(cognitive): add pipeline writer for PromotionOp execution

Deduplicates against existing facts/rules before creating. Reinforces
existing matches (confidence + convergence for facts, signal_count for
rules). New facts are embedded in LanceDB."
```

---

### Task 8: Wire Pipeline into BackgroundConsolidationService

Connect collectors and consolidator into the existing event loop.

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Add signal queue fields to `BackgroundServiceConfig`**

Add to the config struct:

```rust
    pub signal_tx: Option<crate::pipeline::SignalSender>,
    pub signal_rx: Option<crate::pipeline::SignalReceiver>,
```

- [ ] **Step 2: Spawn collectors in `start()`**

Inside the `start()` method, after existing setup but before the main loop, spawn collectors:

```rust
        // Spawn unified pipeline collectors
        if let Some(ref signal_tx) = config.signal_tx {
            if let Some(ref bus) = config.domain_bus {
                // AtomCollector
                let _atom_handle = crate::pipeline::AtomCollector::start(
                    bus.subscribe(), signal_tx.clone(), config.cancel.clone(),
                );
                // CoachingCollector
                let _coaching_handle = crate::pipeline::CoachingCollector::start(
                    bus.subscribe(), signal_tx.clone(), config.cancel.clone(),
                );
                // SessionCollector requires SessionMemoryRepo — wire if available
            }
        }
```

- [ ] **Step 3: Add signal drain at end of each batch cycle**

Find the end of the batch processing block (after extraction and consolidation). Add signal drain:

```rust
            // Drain unified pipeline signals
            if let Some(ref mut signal_rx) = signal_rx {
                let mut signals = Vec::new();
                while let Ok(signal) = signal_rx.try_recv() {
                    signals.push(signal);
                }
                if !signals.is_empty() {
                    let clusters = crate::pipeline::group_signals(signals);
                    let ops = crate::pipeline::heuristic_promote(&clusters);
                    if !ops.is_empty() {
                        crate::pipeline::execute_promotions(
                            &ops, &repo,
                            rule_repo.as_ref().expect("rule_repo"),
                            &episodic_repo, embedder_ref,
                        ).await;
                    }
                }
            }
```

Ensure the `signal_rx` is moved into the async block. Since `SignalReceiver` is not `Clone`, it needs to be taken from the config: `let mut signal_rx = config.signal_rx.take();`

- [ ] **Step 4: Update construction sites**

Find where `BackgroundServiceConfig` is constructed (in `app-core` init). Add:

```rust
let (signal_tx, signal_rx) = cognitive::pipeline::signal_queue(256);
// Add to config:
signal_tx: Some(signal_tx),
signal_rx: Some(signal_rx),
```

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive -p app-core 2>&1 | tail -30
cargo nextest run -p cognitive --no-fail-fast 2>&1 | tail -20
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): wire unified pipeline into background service

Spawns AtomCollector and CoachingCollector on startup. Drains signal
queue at end of each batch cycle, runs consolidator grouping and
heuristic promotion, executes promotion ops via pipeline writer."
```

---

### Task 9: CognitiveContextSource Freshness Labels

**Files:**
- Modify: `crates/cognitive/src/services/context_source.rs`

- [ ] **Step 1: Add freshness label function**

```rust
fn freshness_label(fact: &crate::types::SemanticFact) -> &'static str {
    let days_old = fact.last_accessed.as_ref()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days())
        .unwrap_or(90);
    if fact.convergence_score >= 0.4 || (fact.confidence >= 0.8 && days_old <= 7) {
        "strong"
    } else if fact.confidence >= 0.5 && days_old <= 30 {
        "moderate"
    } else {
        "weak -- verify"
    }
}
```

- [ ] **Step 2: Update fact formatting**

Change the fact format line from `format!("- {}: {} = {}", ...)` to:

```rust
format!("- {}: {} = {} [{}]", f.subject, f.predicate, f.object, freshness_label(f))
```

- [ ] **Step 3: Update rule formatting to include signal count**

Update rule formatting to show signal counts:

```rust
format!("- {} ({} signals)", rule.rule_text, rule.signal_count)
```

- [ ] **Step 4: Build, fix tests, commit**

```bash
cargo build -p cognitive && cargo nextest run -p cognitive -E 'test(context_source)' --no-fail-fast
git add crates/cognitive/src/services/context_source.rs
git commit -m "feat(cognitive): add freshness labels to context source

Facts show [strong/moderate/weak--verify] based on convergence,
confidence, and age. Rules show signal counts."
```

---

### Task 10: Retrieved Memory Format with Markers

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add freshness helper and update fact formatting**

Add the same `freshness_label` function (or extract it into a shared module). Update the fact formatting line:

```rust
let label = freshness_label(&f.fact);
let content = format!(
    "[@fact:{}] {}: {} = {} [{}]",
    f.fact.id, f.fact.subject, f.fact.predicate, f.fact.object, label
);
```

- [ ] **Step 2: Update episodic memory formatting**

```rust
format!("[@episode:{}] {}", ep.id, ep.summary.as_deref().unwrap_or(&ep.content))
```

- [ ] **Step 3: Add marker instruction to memory header**

Find where the memory section header is built and add:

```rust
let header = "## Relevant Memories\nWhen your response draws on memories below, reference them inline using [@type:id] markers. The UI renders these as hoverable details. Only reference memories you actually use.\n";
```

- [ ] **Step 4: Build, fix tests, commit**

```bash
cargo build -p cognitive && cargo nextest run -p cognitive -E 'test(memory_retriever) or test(unified)' --no-fail-fast
git add crates/cognitive/src/services/memory_retriever.rs
git commit -m "feat(cognitive): add [@type:id] markers and freshness to retrieved memory

Facts include [@fact:id] markers and freshness labels. Episodes include
[@episode:id] markers. Header instructs LLM to emit markers inline."
```

---

### Task 11: Frontend Inline References + Settings Toggle

**Files:**
- Create: `desktop-ui/src/shared/ui/MemoryReference.tsx`
- Create: `desktop-ui/src/features/chat/plugins/memoryRefPlugin.ts`
- Modify: `desktop-ui/src/features/chat/components/MarkdownContent.tsx`
- Create: `crates/desktop-shared/src/commands/memory.rs`
- Create: `crates/desktop/src/commands/memory.rs`
- Create: `crates/app-core/src/handlers/memory.rs`
- Modify: `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx`

- [ ] **Step 1: Create MemoryReference tooltip component**

Create `desktop-ui/src/shared/ui/MemoryReference.tsx` — a span that on hover calls `ipc("memory_reference_detail", { refType, refId })` and shows a glass-panel tooltip with the response (title, subtitle, key-value details).

- [ ] **Step 2: Create remark plugin to parse `[@type:id]` markers**

Create `desktop-ui/src/features/chat/plugins/memoryRefPlugin.ts` — visits text nodes, matches `\[@(\w+):([a-f0-9-]+)\]` pattern, replaces with custom `memoryRef` MDAST nodes.

- [ ] **Step 3: Install unist-util-visit**

```bash
cd desktop-ui && bun add unist-util-visit
```

- [ ] **Step 4: Register plugin in MarkdownContent**

Add `memoryRefPlugin` to `remarkPlugins` array and add `"memory-ref"` component renderer that renders `<MemoryReference>`.

- [ ] **Step 5: Create backend command (desktop-shared + desktop + app-core)**

Create `MemoryReferenceDetail` response type in `desktop-shared`. Create `memory_reference_detail` Tauri command in desktop. Create `AppCore::memory_reference_detail()` handler that loads fact/rule/episode by ID and returns formatted tooltip data.

- [ ] **Step 6: Register command in mod.rs and dev server**

Add `pub mod memory;` to desktop commands, register the command, add to `DEV_COMMANDS`.

- [ ] **Step 7: Add IntelligenceMode toggle to Settings**

In `PersonalizationSettings.tsx`, add a Toggle for "Deep Intelligence Mode" that reads/writes `cognitive.intelligenceMode` config.

- [ ] **Step 8: Build and lint**

```bash
cd desktop-ui && bun run build && bun run lint:fix
cargo build -p desktop -p app-core -p desktop-shared 2>&1 | tail -20
```

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src/ crates/desktop-shared/src/commands/memory.rs crates/desktop/src/commands/memory.rs crates/app-core/src/handlers/memory.rs
git commit -m "feat(ui): inline memory references with hover tooltips + Deep Mode toggle

[@type:id] markers in LLM responses render as hoverable (...) references.
Tooltips show fact details, rule signals, episode summaries via
memory_reference_detail command. Deep Mode toggle in Settings."
```

---

### Task 12: Full Validation

- [ ] **Step 1: Build workspace**

```bash
cargo build --workspace 2>&1 | tail -10
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -10
```

- [ ] **Step 3: Format**

```bash
cargo fmt --all --check
```

- [ ] **Step 4: Rust tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 5: Frontend build + lint + test**

```bash
cd desktop-ui && bun run build && bun run lint && bun run test
```

- [ ] **Step 6: Format if needed**

```bash
cargo fmt --all && cd desktop-ui && bun run lint:fix
git add -A && git diff --cached --stat
```

If changes: `git commit -m "style: format after memory bridge layer implementation"`

---

## Summary

| Task | What It Builds |
|------|---------------|
| 1 | IntelligenceMode config + convergence_score schema |
| 2 | CognitiveSignal type + pipeline module |
| 3 | Enrich AtomReinforced event |
| 4 | SessionCollector (session->memory bridge) |
| 5 | AtomCollector + CoachingCollector |
| 6 | Consolidator (grouping + heuristic promotion) |
| 7 | Pipeline Writer (execute PromotionOps) |
| 8 | Wire pipeline into BackgroundConsolidationService |
| 9 | CognitiveContextSource freshness labels |
| 10 | Retrieved memory format with [@type:id] markers |
| 11 | Frontend inline references + Settings toggle |
| 12 | Full validation |
