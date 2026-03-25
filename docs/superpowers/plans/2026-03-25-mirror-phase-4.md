# The Mirror Phase 4 — Trial Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the TrialPreviewSubscriber — "I can steer experiments in real-time" with 4-hour early evaluation and kill/continue buttons.

**Architecture:** A new `TrialActivated` domain event is emitted when the autotuner creates a trial. The `TrialPreviewSubscriber` listens for it, starts a 4-hour timer, then queries early metrics via an `EarlyTrialEvaluator` trait. Results are written to `mirror_trial_previews` and surfaced as snippet cards with Kill/Continue actions. Active timers are shared between the subscriber and `MirrorFacade` via `Arc<DashMap>` for cancellation.

**Tech Stack:** Rust (bus/autotuner/agent/cognitive/app-core/desktop crates), SQLite (mirror_trial_previews table), React + Tailwind v4 (desktop-ui)

**Spec:** `docs/superpowers/specs/2026-03-25-mirror-self-reflection-layer-design.md` — TrialPreviewSubscriber section (lines 331-344) + TrialPreview types (lines 137-152) + TrialActivated event (lines 44-56)

**Depends on:** Phase 1-3 complete (3 subscribers, MirrorEngine, AutotunerBridge)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/mirror/subscribers/trial.rs` | `TrialPreviewSubscriber` — listens for TrialActivated, 4h timer, queries metrics, writes preview |
| `desktop-ui/src/features/mirror/components/ExperimentWatchlist.tsx` | Active trial previews with Kill/Continue buttons |

### Modified files

| File | Change |
|------|--------|
| `crates/bus/src/domain_events.rs` | Add `TrialActivated` variant |
| `crates/agent/src/autotuner/mod.rs` | Emit `TrialActivated` after trial creation |
| `crates/cognitive/src/mirror/types.rs` | Add `TrialPreview`, `TrialEarlySignals`, `PreviewRecommendation`, `TrendDirection`, `EarlyTrialEvaluator` trait; update `MirrorState`, `SuggestedAction`, `MirrorAlert` |
| `crates/cognitive/migrations/003_mirror_tables.sql` | Append `mirror_trial_previews` table |
| `crates/cognitive/src/repos/mod.rs` | Bump migration version to 4 |
| `crates/cognitive/src/mirror/repo.rs` | Add trial preview CRUD methods |
| `crates/cognitive/src/mirror/narratives.rs` | Handle `MirrorAlert::TrialUnpromising` in `snippet_from_alert` |
| `crates/cognitive/src/mirror/facade.rs` | Add `kill_trial`, `continue_trial`; add `active_timers` field; update `get_state` |
| `crates/cognitive/src/mirror/engine.rs` | Start `TrialPreviewSubscriber` (4th subscriber); create shared `active_timers` DashMap |
| `crates/cognitive/src/mirror/subscribers/mod.rs` | Export `TrialPreviewSubscriber` |
| `crates/desktop/src/commands/mirror.rs` | Add `kill_trial`, `continue_trial` commands |
| `desktop-ui/src/features/mirror/MirrorPage.tsx` | Add `ExperimentWatchlist` component |

---

## Task 1: Add `TrialActivated` Domain Event + Emit

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/agent/src/autotuner/mod.rs`

- [ ] **Step 1: Add `TrialActivated` variant**

In `crates/bus/src/domain_events.rs`, add to the `DomainEvent` enum:

```rust
TrialActivated {
    trial_id: String,
    hypothesis: String,
    params_summary: String,
},
```

- [ ] **Step 2: Fix match exhaustiveness**

Run `cargo build --workspace` and fix all match arms. Key files:
- `crates/cognitive/src/services/salience.rs` — `TrialActivated { .. } => SalienceVerdict::Discard`
- `crates/cognitive/src/services/background.rs` — `TrialActivated { .. } => "TrialActivated"`
- `crates/desktop/src/app_core.rs` — `TrialActivated { .. } => "autotuner"`
- `crates/desktop/src/dev_server/streaming.rs` — `TrialActivated { .. } => "autotuner"`

- [ ] **Step 3: Emit `TrialActivated` in autotuner**

In `crates/agent/src/autotuner/mod.rs`, the `domain_event_bus` is captured in the `register_nightly_cycle` closure but NOT available inside the free functions `run_bootstrap_replay()` / `run_llm_generation()`. You must emit the event from within the closure body where the bus is in scope.

Find where `run_bootstrap_replay` and `run_llm_generation` are called from within `register_nightly_cycle`'s closure. After the function returns with the created trial(s), emit the event using the bus already captured in scope:

```rust
// Inside the nightly cycle closure, after trial creation:
if let Some(ref bus) = domain_event_bus {
    bus.publish(DomainEvent::TrialActivated {
        trial_id: trial.id.to_string(),
        hypothesis: trial.generation_reasoning.clone(),
        params_summary: format!("{:?}", trial.params),
    });
}
```

IMPORTANT: Do NOT try to access `self.domain_event_bus` inside the free functions — it won't compile. Emit from the closure that already captures the bus. Read how `AutotunerDecision` is emitted (around line 372-379) for the exact bus access pattern.

- [ ] **Step 4: Build and verify**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): add TrialActivated domain event and emit on trial creation"
```

---

## Task 2: Add Trial Preview Types

**Files:**
- Modify: `crates/cognitive/src/mirror/types.rs`

- [ ] **Step 1: Add types**

```rust
/// 4-hour early evaluation of an autotuner trial
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialPreview {
    pub id: Uuid,
    pub trial_id: String,
    pub started_at: DateTime<Utc>,
    pub preview_at: DateTime<Utc>,
    pub messages_scored: u32,
    pub early_signals: TrialEarlySignals,
    pub recommendation: PreviewRecommendation,
    pub narrative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialEarlySignals {
    pub correction_rate_delta: f64,
    pub confidence_trend: TrendDirection,
    pub dominant_skill_shift: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PreviewRecommendation {
    Continue,
    Kill,
    NeedMoreData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Rising,
    Falling,
    Stable,
}
```

- [ ] **Step 2: Add `EarlyTrialEvaluator` trait**

```rust
/// Trait for querying early trial metrics at the 4-hour mark.
/// Defined in cognitive (L3-L4), implemented in app-core (L7).
#[async_trait::async_trait]
pub trait EarlyTrialEvaluator: Send + Sync {
    async fn evaluate_trial_early(&self, trial_id: &str, since: DateTime<Utc>) -> common::Result<TrialEarlySignals>;
}
```

- [ ] **Step 3: Update `MirrorState`**

Add: `pub recent_trial_previews: Vec<TrialPreview>,`

- [ ] **Step 4: Update `SuggestedAction`**

Add before `RevertBrainVersion`:
```rust
KillTrial { trial_id: String },
ContinueTrial { trial_id: String },
```

- [ ] **Step 5: Update `MirrorAlert` AND fix narratives.rs atomically**

IMPORTANT: These must be done together — `snippet_from_alert` has an exhaustive match. Adding the variant without the match arm will break compilation.

Add variant to `MirrorAlert` in `types.rs`:
```rust
TrialUnpromising {
    trial_id: String,
    reason: String,
},
```

SIMULTANEOUSLY update `narratives.rs` `snippet_from_alert` — add the `TrialUnpromising` match arm. Also update `facade.rs` `get_state()` to add `recent_trial_previews: vec![]` temporarily:

```rust
MirrorAlert::TrialUnpromising { trial_id, reason } => NarrativeSnippet {
    id: Uuid::new_v4(),
    created_at: Utc::now(),
    alert_type: MirrorAlertType::TrialUnpromising,
    headline: "An experiment isn't looking great".to_string(),
    body: format!("After 4 hours, this experiment is {}. Want to kill it early or let it finish?", reason),
    suggested_action: Some(SuggestedAction::KillTrial { trial_id: trial_id.clone() }),
    user_feedback: None,
    dismissed_at: None,
},
```

- [ ] **Step 7: Re-export types from mod.rs**

- [ ] **Step 8: Build**

Run: `cargo build -p cognitive`

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(mirror): add TrialPreview types, EarlyTrialEvaluator trait, and TrialUnpromising alert"
```

---

## Task 3: Add `mirror_trial_previews` Table

**Files:**
- Modify: `crates/cognitive/migrations/003_mirror_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Append table**

```sql
CREATE TABLE IF NOT EXISTS mirror_trial_previews (
    id TEXT PRIMARY KEY,
    trial_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    preview_at TEXT NOT NULL,
    messages_scored INTEGER NOT NULL,
    early_signals_json TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    narrative TEXT NOT NULL
);
```

- [ ] **Step 2: Bump migration version to 4**

- [ ] **Step 3: Build and commit**

```bash
git commit -m "feat(mirror): add mirror_trial_previews table"
```

---

## Task 4: Trial Preview Repo Methods

**Files:**
- Modify: `crates/cognitive/src/mirror/repo.rs`

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn test_insert_and_get_trial_preview() {
    let repo = crate::mirror::test_mirror_repo().await;
    let preview = TrialPreview {
        id: Uuid::new_v4(),
        trial_id: "trial-abc".to_string(),
        started_at: Utc::now() - chrono::Duration::hours(4),
        preview_at: Utc::now(),
        messages_scored: 25,
        early_signals: TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: TrendDirection::Falling,
            dominant_skill_shift: Some("finance-management".to_string()),
        },
        recommendation: PreviewRecommendation::Kill,
        narrative: "Correction rate worsened 15% vs champion".to_string(),
    };
    repo.insert_trial_preview(&preview).await.unwrap();
    let previews = repo.get_recent_trial_previews().await.unwrap();
    assert_eq!(previews.len(), 1);
    assert_eq!(previews[0].trial_id, "trial-abc");
    assert_eq!(previews[0].recommendation, PreviewRecommendation::Kill);
}

#[tokio::test]
async fn test_get_trial_preview_by_trial_id() {
    let repo = crate::mirror::test_mirror_repo().await;
    let preview = TrialPreview {
        id: Uuid::new_v4(),
        trial_id: "trial-abc".to_string(),
        started_at: Utc::now() - chrono::Duration::hours(4),
        preview_at: Utc::now(),
        messages_scored: 25,
        early_signals: TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: TrendDirection::Falling,
            dominant_skill_shift: None,
        },
        recommendation: PreviewRecommendation::Kill,
        narrative: "Test narrative".to_string(),
    };
    repo.insert_trial_preview(&preview).await.unwrap();
    let found = repo.get_trial_preview_by_trial_id("trial-abc").await.unwrap();
    assert!(found.is_some());
}
```

- [ ] **Step 2: Implement**

Add `TrialPreviewRow` + `TryFrom` impl. Methods:
- `insert_trial_preview(&self, preview: &TrialPreview) -> Result<()>`
- `get_recent_trial_previews(&self) -> Result<Vec<TrialPreview>>` — last 10, ordered by preview_at DESC
- `get_trial_preview_by_trial_id(&self, trial_id: &str) -> Result<Option<TrialPreview>>`
- `cleanup_old_trial_previews(&self, max_age_days: u32) -> Result<u64>`

Use `enum_to_str` / `str_to_enum` helpers for `recommendation`. Use `serde_json` for `early_signals_json`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(trial_preview)'`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add TrialPreview CRUD methods to MirrorRepo with tests"
```

---

## Task 5: TrialPreviewSubscriber

**Files:**
- Create: `crates/cognitive/src/mirror/subscribers/trial.rs`
- Modify: `crates/cognitive/src/mirror/subscribers/mod.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_recommendation_kill() {
        let signals = TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: TrendDirection::Falling,
            dominant_skill_shift: None,
        };
        assert_eq!(compute_recommendation(&signals, 20), PreviewRecommendation::Kill);
    }

    #[test]
    fn test_compute_recommendation_continue() {
        let signals = TrialEarlySignals {
            correction_rate_delta: 0.05,
            confidence_trend: TrendDirection::Rising,
            dominant_skill_shift: None,
        };
        assert_eq!(compute_recommendation(&signals, 20), PreviewRecommendation::Continue);
    }

    #[test]
    fn test_compute_recommendation_need_more_data() {
        let signals = TrialEarlySignals {
            correction_rate_delta: 0.02,
            confidence_trend: TrendDirection::Stable,
            dominant_skill_shift: None,
        };
        assert_eq!(compute_recommendation(&signals, 3), PreviewRecommendation::NeedMoreData);
    }
}
```

- [ ] **Step 2: Implement subscriber**

```rust
use bus::DomainEvent;
use chrono::Utc;
use common::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::repo::MirrorRepo;
use super::super::types::*;
use super::super::narratives::snippet_from_alert;

const PREVIEW_DELAY_SECS: u64 = 4 * 60 * 60; // 4 hours
const MIN_MESSAGES_FOR_KILL: u32 = 5;

pub struct TrialPreviewSubscriber {
    repo: MirrorRepo,
    active_timers: Arc<DashMap<String, JoinHandle<()>>>,
    evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
}

impl TrialPreviewSubscriber {
    pub fn new(
        repo: MirrorRepo,
        active_timers: Arc<DashMap<String, JoinHandle<()>>>,
        evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
    ) -> Self {
        Self { repo, active_timers, evaluator }
    }

    // NOTE: Phase 4 ships with evaluator=None (stub). Without a real evaluator,
    // all trials produce correction_rate_delta=0.0 / Stable → NeedMoreData recommendation.
    // This is a known limitation. The EarlyTrialEvaluator implementation requires
    // MetricSource integration which is deferred to Phase 5. The UI, wiring, and
    // recommendation logic are fully functional — only the metric collection is stubbed.

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                event = rx.recv() => {
                    match event {
                        Ok(DomainEvent::TrialActivated { trial_id, hypothesis, .. }) => {
                            self.start_preview_timer(trial_id, hypothesis);
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("TrialPreviewSubscriber lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
        // Abort all active timer tasks on shutdown (clear() only drops handles, doesn't cancel tasks)
        for entry in self.active_timers.iter() {
            entry.value().abort();
        }
        self.active_timers.clear();
    }

    fn start_preview_timer(&self, trial_id: String, hypothesis: String) {
        let repo = self.repo.clone();
        let evaluator = self.evaluator.clone();
        let timers = self.active_timers.clone();
        let tid = trial_id.clone();

        let handle = tokio::spawn(async move {
            let started_at = Utc::now();
            tokio::time::sleep(std::time::Duration::from_secs(PREVIEW_DELAY_SECS)).await;

            // Query early metrics
            let signals = if let Some(eval) = &evaluator {
                eval.evaluate_trial_early(&trial_id, started_at)
                    .await
                    .unwrap_or(TrialEarlySignals {
                        correction_rate_delta: 0.0,
                        confidence_trend: TrendDirection::Stable,
                        dominant_skill_shift: None,
                    })
            } else {
                TrialEarlySignals {
                    correction_rate_delta: 0.0,
                    confidence_trend: TrendDirection::Stable,
                    dominant_skill_shift: None,
                }
            };

            let messages_scored = 0; // TODO: get from evaluator
            let recommendation = compute_recommendation(&signals, messages_scored);

            let narrative = format!(
                "After 4 hours ({} messages): correction rate {:.1}% vs champion. {}.",
                messages_scored,
                signals.correction_rate_delta * 100.0,
                match &recommendation {
                    PreviewRecommendation::Continue => "Looking good — keep going",
                    PreviewRecommendation::Kill => "Trending down — consider killing early",
                    PreviewRecommendation::NeedMoreData => "Not enough data yet — keep watching",
                }
            );

            let preview = TrialPreview {
                id: Uuid::new_v4(),
                trial_id: trial_id.clone(),
                started_at,
                preview_at: Utc::now(),
                messages_scored,
                early_signals: signals,
                recommendation: recommendation.clone(),
                narrative: narrative.clone(),
            };

            let _ = repo.insert_trial_preview(&preview).await;

            // If Kill, also create a snippet alert
            if recommendation == PreviewRecommendation::Kill {
                let alert = MirrorAlert::TrialUnpromising {
                    trial_id: trial_id.clone(),
                    reason: narrative,
                };
                let snippet = snippet_from_alert(&alert);
                let _ = repo.insert_snippet(&snippet).await;
            }

            // Remove self from active timers
            timers.remove(&trial_id);
        });

        self.active_timers.insert(tid, handle);
    }
}

/// Determine recommendation based on early signals
pub fn compute_recommendation(signals: &TrialEarlySignals, messages_scored: u32) -> PreviewRecommendation {
    // Kill: correction rate worsened >10% OR insufficient data trending badly
    if signals.correction_rate_delta < -0.10 {
        return PreviewRecommendation::Kill;
    }
    if messages_scored < MIN_MESSAGES_FOR_KILL && signals.confidence_trend == TrendDirection::Falling {
        return PreviewRecommendation::Kill;
    }
    // Continue: positive delta with rising/stable confidence
    if signals.correction_rate_delta > 0.0
        && (signals.confidence_trend == TrendDirection::Rising
            || signals.confidence_trend == TrendDirection::Stable)
    {
        return PreviewRecommendation::Continue;
    }
    // Everything else
    PreviewRecommendation::NeedMoreData
}
```

- [ ] **Step 3: Export from subscribers/mod.rs**

Add: `pub mod trial; pub use trial::TrialPreviewSubscriber;`

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(compute_recommendation)'`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): add TrialPreviewSubscriber with 4h timer and recommendation logic"
```

---

## Task 6: Update MirrorFacade for Trial Previews

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Add `active_timers` field**

Add to `MirrorFacade`:
```rust
pub active_timers: Option<Arc<DashMap<String, JoinHandle<()>>>>,
```

Add builder:
```rust
pub fn with_active_timers(mut self, timers: Arc<DashMap<String, JoinHandle<()>>>) -> Self {
    self.active_timers = Some(timers);
    self
}
```

- [ ] **Step 2: Add methods**

```rust
pub async fn kill_trial(&self, trial_id: &str) -> Result<()> {
    // Cancel the preview timer if running
    if let Some(timers) = &self.active_timers {
        if let Some((_, handle)) = timers.remove(trial_id) {
            handle.abort();
        }
    }
    // TODO: In the future, also tell the autotuner to stop the trial
    Ok(())
}

pub async fn continue_trial(&self, trial_id: &str) -> Result<()> {
    // Acknowledge the preview — just remove the timer, trial continues naturally
    if let Some(timers) = &self.active_timers {
        timers.remove(trial_id);
    }
    Ok(())
}

pub async fn get_trial_previews(&self) -> Result<Vec<TrialPreview>> {
    self.repo.get_recent_trial_previews().await
}
```

- [ ] **Step 3: Update `get_state()`**

Add `get_recent_trial_previews()` to the `tokio::try_join!` (7th query now):

```rust
let (..., recent_trial_previews) = tokio::try_join!(
    // ... existing 6 queries ...
    self.repo.get_recent_trial_previews(),
)?;
```

- [ ] **Step 4: Write tests**

```rust
#[tokio::test]
async fn test_get_state_includes_trial_previews() {
    let repo = crate::mirror::test_mirror_repo().await;
    let facade = MirrorFacade::new(repo.clone());
    // Insert a trial preview
    let preview = TrialPreview { /* ... */ };
    repo.insert_trial_preview(&preview).await.unwrap();
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.recent_trial_previews.len(), 1);
}
```

- [ ] **Step 5: Build and run tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(mirror): add kill_trial, continue_trial, and trial preview state to MirrorFacade"
```

---

## Task 7: Wire TrialPreviewSubscriber into MirrorEngine

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Update engine to create shared timers and 4th subscriber**

```rust
// Create shared active_timers
let active_timers: Arc<DashMap<String, JoinHandle<()>>> = Arc::new(DashMap::new());

// Create TrialPreviewSubscriber
let trial_sub = Arc::new(TrialPreviewSubscriber::new(
    trial_repo, active_timers.clone(), None, // evaluator — wired later
));

// Add to handles
tokio::spawn(trial_sub.run(bus.subscribe(), shutdown.clone())),

// Wire timers into facade
facade = facade.with_active_timers(active_timers);
```

- [ ] **Step 2: Update subscriber count test to 4**

- [ ] **Step 3: Update app-core init call site**

The `MirrorEngine::start` signature hasn't changed (it still takes the same 4 params), but if you added new params for the evaluator, update the call site.

- [ ] **Step 4: Build**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): wire TrialPreviewSubscriber into MirrorEngine (4 subscribers)"
```

---

## Task 8: Tauri Commands for Trial Actions

**Files:**
- Modify: `crates/desktop/src/commands/mirror.rs`

- [ ] **Step 1: Add commands**

```rust
#[tauri::command]
pub async fn kill_trial(
    state: State<'_, Arc<AppCore>>,
    trial_id: String,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.kill_trial(&trial_id).await?)
}

#[tauri::command]
pub async fn continue_trial(
    state: State<'_, Arc<AppCore>>,
    trial_id: String,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.continue_trial(&trial_id).await?)
}
```

- [ ] **Step 2: Update DEV_COMMANDS, invoke_handler, dispatch_dev**

In the hoisted dispatch_dev, add both commands. Accept both `trialId` (camelCase) and `trial_id` (snake_case) using the `dev::get(body, "trialId").or_else(...)` pattern.

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add kill_trial and continue_trial Tauri commands"
```

---

## Task 9: ExperimentWatchlist UI

**Files:**
- Create: `desktop-ui/src/features/mirror/components/ExperimentWatchlist.tsx`
- Modify: `desktop-ui/src/features/mirror/MirrorPage.tsx`

- [ ] **Step 1: Create ExperimentWatchlist**

```tsx
import { useMutation } from "@shared/hooks/useMutation";
import { FlaskConical, Play, X } from "lucide-react";

interface TrialPreview {
  id: string;
  trialId: string;
  startedAt: string;
  previewAt: string;
  messagesScored: number;
  earlySignals: {
    correctionRateDelta: number;
    confidenceTrend: string;
    dominantSkillShift: string | null;
  };
  recommendation: string;
  narrative: string;
}

interface ExperimentWatchlistProps {
  previews: TrialPreview[];
  onAction?: () => void;
}

export function ExperimentWatchlist({ previews, onAction }: ExperimentWatchlistProps) {
  const { mutate: kill } = useMutation<void, { trialId: string }>("kill_trial");
  const { mutate: cont } = useMutation<void, { trialId: string }>("continue_trial");

  if (previews.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
        <FlaskConical className="size-3.5" />
        Experiment Watchlist
      </h2>

      {previews.map((preview) => {
        const isKill = preview.recommendation === "Kill";
        const isContinue = preview.recommendation === "Continue";
        const deltaPct = (preview.earlySignals.correctionRateDelta * 100).toFixed(1);

        return (
          <div key={preview.id} className={`glass-panel rounded-xl p-4 ${isKill ? "border border-destructive/30" : ""}`}>
            <div className="flex items-center justify-between mb-1">
              <span className="text-[12px] font-medium text-foreground">
                Trial {preview.trialId.slice(0, 8)}
              </span>
              <span className={`text-2xs px-1.5 py-0.5 rounded ${
                isKill ? "text-destructive bg-destructive/10" :
                isContinue ? "text-success bg-success/10" :
                "text-muted-foreground bg-muted/10"
              }`}>
                {preview.recommendation}
              </span>
            </div>

            <p className="text-[11px] text-muted-foreground">{preview.narrative}</p>

            <div className="flex items-center gap-2 mt-3">
              <button
                type="button"
                onClick={async () => { await kill({ trialId: preview.trialId }); onAction?.(); }}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-destructive bg-destructive/10 hover:bg-destructive/20 transition-colors"
              >
                <X className="size-3" />
                Kill it
              </button>
              <button
                type="button"
                onClick={async () => { await cont({ trialId: preview.trialId }); onAction?.(); }}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-success bg-success/10 hover:bg-success/20 transition-colors"
              >
                <Play className="size-3" />
                Let it run
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Update MirrorPage**

Import and add to the render (between SnippetFeed and MetaRulesSection):

```tsx
import { ExperimentWatchlist } from "./components/ExperimentWatchlist";

// Add to MirrorState interface:
// recentTrialPreviews: TrialPreview[];

// Add to DEFAULT_MIRROR_STATE:
// recentTrialPreviews: [],

// In render:
<ExperimentWatchlist
  previews={mirrorState?.recentTrialPreviews ?? []}
  onAction={refetch}
/>
```

- [ ] **Step 3: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add ExperimentWatchlist UI with kill/continue actions"
```

---

## Task 10: Integration Test

**Files:**
- Modify: `tests/integration/mirror.rs`

- [ ] **Step 1: Add trial preview test**

```rust
#[tokio::test]
async fn test_mirror_trial_preview_lifecycle() {
    let pool = mirror_pool().await;
    let repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let facade = cognitive::mirror::MirrorFacade::new(repo.clone());

    // Insert a trial preview
    let preview = cognitive::mirror::TrialPreview {
        id: uuid::Uuid::new_v4(),
        trial_id: "trial-test-001".to_string(),
        started_at: chrono::Utc::now() - chrono::Duration::hours(4),
        preview_at: chrono::Utc::now(),
        messages_scored: 25,
        early_signals: cognitive::mirror::TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: cognitive::mirror::TrendDirection::Falling,
            dominant_skill_shift: None,
        },
        recommendation: cognitive::mirror::PreviewRecommendation::Kill,
        narrative: "Correction rate worsened".to_string(),
    };
    repo.insert_trial_preview(&preview).await.unwrap();

    // Verify in state
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.recent_trial_previews.len(), 1);
    assert_eq!(state.recent_trial_previews[0].recommendation, cognitive::mirror::PreviewRecommendation::Kill);
}
```

- [ ] **Step 2: Run test**

Run: `cargo nextest run -E 'test(trial_preview_lifecycle)'`

- [ ] **Step 3: Commit**

```bash
git commit -m "test(mirror): add trial preview lifecycle integration test for Phase 4"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all mirror tests:** `cargo nextest run -p cognitive -E 'test(mirror)'`
- [ ] **Run integration tests:** `cargo nextest run -E 'test(mirror)'`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Run frontend lint:** `cd desktop-ui && bun run lint`
- [ ] **Run frontend build:** `cd desktop-ui && bun run build`
