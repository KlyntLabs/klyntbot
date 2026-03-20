# Autotuner Phase 1 Remaining — Complete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete every remaining Phase 1 item from the autoresearch spec — bootstrap replay, `memory_relevance` metric, per-trial correction rates, focus banner integration, and promotion toast persistence.

**Architecture:** Five independent tasks. Bootstrap replay runs once at startup to seed informed first experiments from historical session data. `memory_relevance` threads retrieved memory IDs from context_engine through to the metric collector. Per-trial correction rates use the existing `user_corrected` flag on `autotuner_shadow_log`. Focus banner adds a subtle line in the focus timer UI. Toast counter persists via `LearningStateRepo`.

**Tech Stack:** Rust (SQLite/sqlx, tokio, serde), React 19, TypeScript, Tailwind v4

**Spec:** `docs/superpowers/specs/2026-03-19-autoresearch-design.md` (Phase 0 lines 270-275, Focus Banner lines 461-466, Toast Counter line 620)

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/storage/src/repos/session.rs` | Add `list_sessions_since` query | 1 |
| `crates/agent/src/autotuner/mod.rs` | Bootstrap replay function + session_repo field | 1 |
| `crates/app-core/src/init/cron.rs` | Trigger bootstrap on first startup | 1 |
| `crates/context_engine/src/assembler/mod.rs` | Thread retrieved memory IDs through assembly | 2 |
| `crates/context_engine/src/assembler/types.rs` | Add `retrieved_memory_ids` to `AssembledContext` | 2 |
| `crates/agent/src/agent_runtime/runtime.rs` | Capture memory IDs, store for post-response comparison | 2 |
| `crates/agent/src/autotuner/metric_collector.rs` | Wire `memory_relevance` + per-trial correction rate | 2, 3 |
| `crates/storage/src/repos/trial_repo.rs` | Add `correction_rate_for_trial` query | 3 |
| `desktop-ui/src/features/tray/components/FocusControl.tsx` | Add autotuner learning banner during focus | 4 |
| `desktop-ui/src/features/autotuner/hooks/usePromotionListener.ts` | Persist toast count via backend | 5 |
| `crates/app-core/src/handlers/autotuner.rs` | Add toast count get/increment handlers | 5 |
| `crates/desktop/src/commands/autotuner.rs` | Add toast count Tauri commands | 5 |

---

### Task 1: Bootstrap / Historical Replay

**Files:**
- Modify: `crates/storage/src/repos/session.rs`
- Modify: `crates/agent/src/autotuner/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`

Seeds the first experiment from real historical data instead of blind LLM guesses.

- [ ] **Step 1: Add `list_sessions_since` to `SessionRepo`**

In `crates/storage/src/repos/session.rs`, add:

```rust
pub async fn list_sessions_since(
    &self,
    since: DateTime<Utc>,
) -> Result<Vec<SessionListRow>, StorageError> {
    Ok(sqlx::query_as::<_, SessionListRow>(
        "SELECT * FROM sessions WHERE updated_at >= ?1 ORDER BY updated_at DESC",
    )
    .bind(since.to_rfc3339())
    .fetch_all(&self.pool)
    .await?)
}
```

Add test:
```rust
#[tokio::test]
async fn list_sessions_since_filters_by_date() {
    // Setup pool, insert sessions, verify filtering
}
```

- [ ] **Step 2: Run test to verify it fails, implement, verify pass**

Run: `cargo nextest run -p storage -E 'test(list_sessions_since)' --no-fail-fast`

- [ ] **Step 3: Add `session_repo` to `AutoTunerOrchestrator`**

In `crates/agent/src/autotuner/mod.rs`, add to the struct:
```rust
session_repo: Option<storage::SessionRepo>,
```

Add builder method `with_session_repo(self, repo: storage::SessionRepo) -> Self`.

- [ ] **Step 4: Implement `run_bootstrap_replay` function**

Add to `crates/agent/src/autotuner/mod.rs`:

```rust
/// Runs historical replay to seed the first experiment with informed variants.
/// Only runs if no experiments exist yet (first startup).
async fn run_bootstrap_replay(orch: &AutoTunerOrchestrator) -> common::Result<()> {
    // 1. Check if experiments already exist — skip if so
    let existing = orch.trial_repo.get_experiments(1).await?;
    if !existing.is_empty() {
        tracing::info!("Bootstrap skipped — experiments already exist");
        return Ok(());
    }

    let (session_repo, strategy_repo) = match (&orch.session_repo, &orch.strategy_repo) {
        (Some(sr), Some(str)) => (sr, str),
        _ => {
            tracing::info!("Bootstrap skipped — session_repo or strategy_repo not available");
            return Ok(());
        }
    };

    // 2. Load sessions from last 7 days
    let cutoff = Utc::now() - chrono::Duration::days(7);
    let sessions = session_repo.list_sessions_since(cutoff).await?;
    if sessions.is_empty() {
        tracing::info!("Bootstrap skipped — no recent sessions");
        return Ok(());
    }

    // 3. Load strategy records for ground truth matching
    let strategy_records = strategy_repo
        .list_by_date_range(cutoff, Utc::now())
        .await?;

    // 4. For each session, get user messages and replay with random perturbations
    let mut replay_results: Vec<(TrialParams, f64)> = Vec::new(); // (params, accuracy)
    let rng_perturbations = generate_random_perturbations(5);

    for session_row in sessions.iter().take(20) {
        let messages = session_repo.get_messages(&session_row.key).await?;
        let user_messages: Vec<&storage::SessionMessageRow> = messages
            .iter()
            .filter(|m| m.role == "user")
            .collect();

        for params in &rng_perturbations {
            let mut correct = 0u32;
            let mut total = 0u32;

            for msg in &user_messages {
                // Find closest strategy_record within 30s window
                let ground_truth = strategy_records.iter().find(|sr| {
                    sr.chat_id.as_deref() == Some(session_row.key.as_str())
                        && (sr.timestamp - msg.timestamp).abs() < chrono::Duration::seconds(30)
                });

                if let Some(gt) = ground_truth {
                    // Shadow classify with these params
                    let prediction = orch.shadow_classifier
                        .classify_shadow(&msg.content, &ShadowContext {
                            chat_id: session_row.key.clone(),
                            session_key: session_row.key.clone(),
                        }, params)
                        .await;

                    if let Ok(pred) = prediction {
                        total += 1;
                        if pred.predicted_mode == gt.actual_strategy {
                            correct += 1;
                        }
                    }
                }
            }

            if total > 0 {
                replay_results.push((params.clone(), correct as f64 / total as f64));
            }
        }
    }

    if replay_results.is_empty() {
        tracing::info!("Bootstrap: no replay results — not enough matchable data");
        return Ok(());
    }

    // 5. Sort by accuracy, take top 3 as seed for LLM generation
    replay_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_results: Vec<_> = replay_results.into_iter().take(3).collect();

    tracing::info!(
        results = top_results.len(),
        best_accuracy = %format!("{:.1}%", top_results[0].1 * 100.0),
        "Bootstrap replay completed — seeding first experiment"
    );

    // 6. Use these as the initial active trials
    let exp_id = uuid::Uuid::new_v4().to_string();
    orch.trial_repo.create_experiment(&storage::rows::trial::ExperimentRow {
        id: exp_id.clone(),
        hypothesis: "Bootstrap: seeded from historical replay of past 7 days".into(),
        trend_analysis: format!("Replayed {} sessions, best accuracy {:.1}%",
            sessions.len().min(20), top_results[0].1 * 100.0),
        recommendation_for_next: "Evaluate bootstrap seeds before generating LLM variants".into(),
        created_at: String::new(),
    }).await?;

    for (i, (params, accuracy)) in top_results.iter().enumerate() {
        orch.trial_repo.create_trial(&storage::rows::trial::TrialRow {
            id: uuid::Uuid::new_v4().to_string(),
            experiment_id: exp_id.clone(),
            params: serde_json::to_string(params)?,
            generation_reasoning: format!(
                "Bootstrap variant {} — {:.1}% accuracy on historical replay",
                i + 1, accuracy * 100.0
            ),
            status: "active".into(),
            created_at: String::new(),
            completed_at: None,
            result: None,
        }).await?;
    }

    Ok(())
}

/// Generate N random TrialParams perturbations within spec bounds.
fn generate_random_perturbations(count: usize) -> Vec<TrialParams> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..count).map(|_| TrialParams {
        skill_keyword_weight: Some(rng.gen_range(0.30..=0.90)),
        skill_semantic_weight: Some(rng.gen_range(0.10..=0.70)),
        skill_activation_threshold: Some(rng.gen_range(0.20..=0.70)),
        heuristic_confidence_threshold: Some(rng.gen_range(0.60..=0.95)),
        llm_classifier_timeout_ms: Some(rng.gen_range(500..=5000)),
        relevance_weight_semantic: Some(rng.gen_range(0.10..=0.60)),
        relevance_weight_retrievability: Some(rng.gen_range(0.05..=0.50)),
        relevance_weight_situation: Some(rng.gen_range(0.05..=0.50)),
    }).collect()
}
```

Note: Check if `rand` is already a dependency of the `agent` crate. If not, add `rand = "0.8"` to `Cargo.toml`. Also verify the exact field names on `SessionMessageRow` and `StrategyRecordRow` — the code above uses field names from the research; verify by reading the actual structs.

The `shadow_classifier` field needs to be accessible on the orchestrator. Currently `AgentShadowClassifier` is on `AutoTunerHookImpl`, not on the orchestrator. Either pass it to the bootstrap function or construct a new one. Check the constructor — it needs a `provider` and `model` which the orchestrator already holds.

- [ ] **Step 5: Wire bootstrap in `init/cron.rs`**

In `crates/app-core/src/init/cron.rs`, after the orchestrator is built (around line 142) and before `ensure_nightly_job`:

```rust
// Run bootstrap replay on first startup (no-op if experiments already exist)
let orch_clone = Arc::clone(&orchestrator);
tokio::spawn(async move {
    if let Err(e) = run_bootstrap_replay(&orch_clone).await {
        tracing::warn!(error = %e, "Bootstrap replay failed");
    }
});
```

Also wire `session_repo`:
```rust
orchestrator = orchestrator.with_session_repo(storage::SessionRepo::new(repos.pool().clone()));
```

- [ ] **Step 6: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p agent -E 'test(bootstrap)' --no-fail-fast`

- [ ] **Step 7: Commit**

```bash
git add crates/storage/src/repos/session.rs crates/agent/src/autotuner/mod.rs crates/app-core/src/init/cron.rs
git commit -m "feat(autotuner): bootstrap replay — seed first experiment from historical sessions"
```

---

### Task 2: Wire `memory_relevance` metric

**Files:**
- Modify: `crates/context_engine/src/assembler/types.rs`
- Modify: `crates/context_engine/src/assembler/mod.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/agent/src/autotuner/metric_collector.rs`

- [ ] **Step 1: Add `retrieved_memory_ids` to `AssembledContext`**

In `crates/context_engine/src/assembler/types.rs`, add to the `AssembledContext` struct:

```rust
/// IDs of memories retrieved during assembly (for relevance scoring).
#[serde(default)]
pub retrieved_memory_ids: Vec<String>,
```

- [ ] **Step 2: Thread memory IDs through assembly**

In `crates/context_engine/src/assembler/mod.rs`, find `assemble_uncached()` where `retrieve_memory()` is called. Currently it returns `Option<String>`. Change to also capture the raw `MemoryEntry` IDs before rendering to text.

Find the `retrieve_memory` method — it likely calls a `MemoryRetriever::retrieve(query, limit)` that returns `Vec<MemoryEntry>`. Capture the `id` fields before converting to a formatted string:

```rust
let (memory_text, memory_ids) = if let Some(retriever) = &self.memory_retriever {
    let entries = retriever.retrieve(&query, limit).await?;
    let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    let text = format_memory_entries(&entries); // existing formatting logic
    (Some(text), ids)
} else {
    (None, vec![])
};
```

Set `assembled.retrieved_memory_ids = memory_ids;`

Note: The exact structure of `retrieve_memory` may differ — read the file carefully. The key principle is: intercept the `Vec<MemoryEntry>` before it's reduced to a `String`, extract IDs, pass them through.

- [ ] **Step 3: Store memory IDs on the runtime for post-response comparison**

In `crates/agent/src/agent_runtime/runtime.rs`, after `context_engine.assemble()` returns the `AssembledContext`, capture `assembled.retrieved_memory_ids`. After the LLM response is received, compute relevance:

```rust
let memory_relevance = if !assembled.retrieved_memory_ids.is_empty() {
    // Simple heuristic: check how many retrieved memory contents appear in the response
    // This requires access to the actual memory content, not just IDs
    // For now, use a simpler proxy: retrieved_count / max_expected
    // Full implementation needs the content stored alongside IDs
    1.0 // placeholder until content comparison is wired
} else {
    1.0 // no memories retrieved = fully relevant (no waste)
};
```

The full content-comparison approach requires storing `Vec<(String, String)>` (id + content) instead of just IDs. This is heavier. For Phase 1, store the count and compute `memory_relevance = retrieved_count as f64 / max_retrievable as f64` as a proxy — it measures "did we retrieve memories at all?" rather than "were they used in the response."

A better Phase 1 approach: store `retrieved_memory_count` as a field on `StrategyRecordRow` alongside the existing metrics. Then `MetricCollector` can query it.

- [ ] **Step 4: Add `avg_memory_retrieval_rate` to metric collector**

In `crates/agent/src/autotuner/metric_collector.rs`, replace `memory_relevance = 1.0` with a query that checks how many messages had memories retrieved vs total messages. This is a simpler proxy that doesn't require response content comparison.

For Phase 1, if `AssembledContext.retrieved_memory_ids` is threaded through and stored somewhere queryable, use it. Otherwise, leave at `1.0` with a comment explaining the Phase 2 plan.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p context_engine -p agent --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/ crates/agent/
git commit -m "feat(autotuner): thread memory retrieval IDs for memory_relevance metric"
```

---

### Task 3: Per-trial correction rates

**Files:**
- Modify: `crates/storage/src/repos/trial_repo.rs`
- Modify: `crates/agent/src/autotuner/metric_collector.rs`

- [ ] **Step 1: Add `correction_rate_for_trial` to TrialRepo**

```rust
pub async fn correction_rate_for_trial(
    &self,
    trial_id: &str,
    since: DateTime<Utc>,
) -> Result<(i64, i64), StorageError> {
    // Returns (total_messages, corrected_messages) for this trial
    let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*) AS total,
                COALESCE(SUM(CASE WHEN user_corrected = 1 THEN 1 ELSE 0 END), 0) AS corrected
         FROM autotuner_shadow_log
         WHERE trial_id = ?1 AND created_at >= ?2 AND control_mode != 'pending'",
    )
    .bind(trial_id)
    .bind(&since_str)
    .fetch_one(&self.pool)
    .await
    .map_err(Into::into)
}
```

- [ ] **Step 2: Add test**

```rust
#[tokio::test]
async fn correction_rate_for_trial_computes_correctly() {
    // Setup: create experiment + trial, insert shadow log rows with some user_corrected = 1
    // Verify (total, corrected) counts match
}
```

- [ ] **Step 3: Run test, implement, verify**

Run: `cargo nextest run -p storage -E 'test(correction_rate_for)' --no-fail-fast`

- [ ] **Step 4: Use per-trial rate in metric collector**

In `crates/agent/src/autotuner/metric_collector.rs`, update the correction_rate computation:

```rust
// Per-trial correction rate when trial_id is available
let correction_rate = if let Some(tid) = trial_id_str.as_deref() {
    let (total, corrected) = self.trial_repo
        .correction_rate_for_trial(tid, since)
        .await
        .unwrap_or((0, 0));
    if total == 0 { 0.0 } else { (corrected as f64 / total as f64).min(1.0) }
} else {
    // System-wide fallback (existing logic)
    let correction_count = correction_count.unwrap_or(0);
    let total = stats.total_records.max(1);
    (correction_count as f64 / total as f64).min(1.0)
};
```

This requires restructuring the `tokio::join!` block slightly — `correction_count` from `event_log_repo` is still needed for the `None` branch.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p agent -E 'test(metric)' --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/repos/trial_repo.rs crates/agent/src/autotuner/metric_collector.rs
git commit -m "feat(autotuner): per-trial correction rates from shadow_log.user_corrected"
```

---

### Task 4: Focus banner integration

**Files:**
- Modify: `desktop-ui/src/features/tray/components/FocusControl.tsx`

- [ ] **Step 1: Read `FocusControl.tsx`**

Understand the rendering structure. Find where `phase === "focus"` content is rendered. Find the `timer.actionTitle` display area.

- [ ] **Step 2: Add autotuner learning line**

Import `useAutoTunerStatus` from `@features/autotuner`. Add a subtle line during active focus:

```tsx
import { useAutoTunerStatus } from "@features/autotuner"

// Inside FocusControl component:
const { data: autotunerStatus } = useAutoTunerStatus()
const showLearningBanner = autotunerStatus?.enabled
  && (autotunerStatus.champion?.days_active ?? 0) > 3
  && phase === "focus"

// Track "shown once per session" with a ref
const learningBannerShown = useRef(false)
useEffect(() => {
  if (phase === "focus") learningBannerShown.current = false
}, [phase])
```

In the JSX, near the timer display during focus phase, add:

```tsx
{showLearningBanner && !learningBannerShown.current && (
  <p className="text-[11px] text-muted/60 mt-2 animate-[fadeIn_0.5s_ease-out]"
    onAnimationEnd={() => { learningBannerShown.current = true }}
  >
    Learning how you focus best...
  </p>
)}
```

Note: The `onAnimationEnd` trick sets the ref after the first render, ensuring it shows exactly once. If `fadeIn` keyframe doesn't exist, add a simple one or use `animate-in` from existing CSS.

- [ ] **Step 3: Verify**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "feat(desktop): show 'Learning how you focus best...' during focus sessions"
```

---

### Task 5: Promotion toast counter persistence

**Files:**
- Modify: `crates/app-core/src/handlers/autotuner.rs`
- Modify: `crates/desktop/src/commands/autotuner.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `desktop-ui/src/features/autotuner/hooks/usePromotionListener.ts`

- [ ] **Step 1: Add toast count handlers to AppCore**

In `crates/app-core/src/handlers/autotuner.rs`:

```rust
const TOAST_COUNT_KEY: &str = "autotuner_promotion_toast_count";

pub async fn autotuner_get_toast_count(&self) -> Result<i64, ApiError> {
    let orch = self.autotuner_orchestrator()
        .ok_or_else(|| ApiError::not_found("autotuner not enabled"))?;
    let count = orch.learning_state_repo()
        .get_value(TOAST_COUNT_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    Ok(count)
}

pub async fn autotuner_increment_toast_count(&self) -> Result<i64, ApiError> {
    let orch = self.autotuner_orchestrator()
        .ok_or_else(|| ApiError::not_found("autotuner not enabled"))?;
    let current = self.autotuner_get_toast_count().await.unwrap_or(0);
    let new_count = current + 1;
    orch.learning_state_repo()
        .set(TOAST_COUNT_KEY, &serde_json::Value::Number(new_count.into()))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(new_count)
}
```

- [ ] **Step 2: Add Tauri commands**

In `crates/desktop/src/commands/autotuner.rs`:

```rust
#[tauri::command]
pub async fn autotuner_get_toast_count(
    state: State<'_, Arc<AppCore>>,
) -> Result<i64, ApiError> {
    state.autotuner_get_toast_count().await
}

#[tauri::command]
pub async fn autotuner_increment_toast_count(
    state: State<'_, Arc<AppCore>>,
) -> Result<i64, ApiError> {
    state.autotuner_increment_toast_count().await
}
```

Add both to `DEV_COMMANDS` and `dispatch_dev`. Register in `main.rs`.

- [ ] **Step 3: Update `usePromotionListener.ts`**

Replace the `useRef` counter with persistent backend calls:

```tsx
import { useQuery } from "@shared/hooks/useQuery"
import { useMutation } from "@shared/hooks/useMutation"

const MAX_PROMOTIONS = 3

export function usePromotionListener(onPromotion: (impact: string) => void) {
  const { data: status } = useAutoTunerStatus()
  const { data: toastCount } = useQuery<number>("autotuner_get_toast_count", undefined, 0)
  const { mutate: incrementCount } = useMutation<number>("autotuner_increment_toast_count")
  const prevTrialId = useRef<string | null>(null)
  const onPromotionRef = useRef(onPromotion)
  onPromotionRef.current = onPromotion

  useEffect(() => {
    if (!status?.enabled || !status.champion) return
    const currentTrialId = status.champion.trial_id
    if (
      currentTrialId
      && prevTrialId.current !== null
      && currentTrialId !== prevTrialId.current
      && (toastCount ?? 0) < MAX_PROMOTIONS
    ) {
      incrementCount({})
      onPromotionRef.current(status.champion.impact || "response quality improved")
    }
    prevTrialId.current = currentTrialId ?? null
  }, [status?.champion?.trial_id, toastCount, incrementCount])
}
```

- [ ] **Step 4: Verify**

Run: `cargo check --workspace && cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/autotuner.rs crates/desktop/src/commands/autotuner.rs crates/desktop/src/main.rs desktop-ui/src/features/autotuner/hooks/usePromotionListener.ts
git commit -m "feat(autotuner): persist promotion toast counter in LearningStateRepo"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run full backend checks**

Run: `cargo check --workspace && cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 2: Run backend tests**

Run: `cargo nextest run -p storage -p agent -p context_engine -p app-core --no-fail-fast`

- [ ] **Step 3: Run frontend build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 4: Commit if fixes needed**

```bash
git add -A && git commit -m "chore: fix lint/fmt from phase 1 remaining"
```

---

## Dependency Graph

```
Task 1 (Bootstrap replay) — independent, largest
Task 2 (memory_relevance) — independent
Task 3 (Per-trial correction) — independent
Task 4 (Focus banner) — independent, frontend only
Task 5 (Toast persistence) — independent, frontend + small backend

Tasks 1, 2, 3, 4, 5 can all run in parallel.
Task 6 runs last.
```
