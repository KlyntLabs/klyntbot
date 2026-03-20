# Autotuner Feedback Loop — Phase 1 Design

> **Date:** 2026-03-20
> **Scope:** Close the autotuner's broken feedback loop (gaps 1–5). Phase 2 (generation context enrichment) is deliberately deferred.
> **Goal:** Get to the first "Trial X promoted" log line — proof that the self-tuning flywheel is alive.

---

## Problem Statement

The autotuner has a complete architecture — shadow classification, nightly evaluation cycles, LLM-generated variant experiments, promotion/rollback logic — but its feedback loop is dead:

- 4 of 8 metrics in `MetricSnapshot` are hardcoded placeholders (`correction_rate: 0.0`, `avg_tokens_per_message: 0.0`, `routing_stability: 1.0`, `memory_relevance: 1.0`)
- `on_message_completed()` is a no-op — ground truth is never written to `autotuner_shadow_log`
- `UserCorrectedAI` is defined as a domain event but never emitted from live interactions
- The `ConstraintEvaluator` checks constraints against fake data, so no trial can meaningfully pass or fail

The evaluator can't promote because it has no real signal. The generator can't learn because it has no real history. The system is architecturally complete but operationally inert.

## Out of Scope

- **Gap 6: Generation context enrichment** — `trend_summary`, `behavioral_context`, `memory_snapshot` stay as placeholder strings. These become high-leverage *after* we see which failure modes cost most.
- **`memory_relevance` metric** — stays `1.0`. Requires context engine retrieval scoring integration (Phase 2).
- **Per-trial correction rates** — Phase 1 uses system-wide correction rate. Per-trial requires joining shadow_log ↔ domain_event_log by chat_id + time.
- **Full correction UX** (mini-form, "What was wrong?" dialog) — the one-line acknowledgment ("Noted — adjusting for next time.") is in-scope; richer correction capture UX is Phase 2.

---

## Architecture: Approach A (Bus-Mediated)

Correction signals are emitted at the `AgentLoop` level → cognitive pipeline persists them to `domain_event_log` → `AgentMetricCollector` queries `domain_event_log` for correction counts. Ground truth for shadow_log is written separately via `on_message_completed`.

```
Reaction (👎)  ──→  AgentLoop::handle_reaction()
                         │
                         ├─→ set_satisfaction (existing)
                         ├─→ bus.publish(UserCorrectedAI { kind: Reaction, ... })
                         └─→ trial_repo.mark_recent_messages_corrected(chat_id, 15min)
                                    │
                                    ▼
                         cognitive background service
                         persists to domain_event_log
                                    │
                                    ▼ (nightly cycle reads)
                         EventLogRepo::count_by_event_type("UserCorrectedAI", since)
                                    │
                                    ▼
                         AgentMetricCollector → correction_rate

User text "no, wrong" ──→ AgentLoop::process_message()
                         │ (before pipeline)
                         ├─→ detect keyword correction
                         ├─→ bus.publish(UserCorrectedAI { kind: KeywordPrefix, ... })
                         └─→ trial_repo.mark_recent_messages_corrected(chat_id, 15min)

Pipeline completes ──→ AgentRuntime (Step 11)
                         └─→ autotuner_hook.on_message_completed(
                               chat_id, orchestrator_name, execution_mode,
                               tokens, elapsed_ms)
                               └─→ trial_repo.update_shadow_log_ground_truth(
                                     chat_id, orchestrator, mode)
```

### Why Bus-Mediated?

- Follows existing patterns (cognitive already subscribes to all bus events)
- No new dependencies on the autotuner hook — it only needs `TrialRepo` (already has it)
- Clean separation: emission ≠ consumption
- System-wide correction rate is more stable with small sample sizes (< 300 messages)

---

## Section 1: Correction Signal Emission

### 1a. Extend `UserCorrectedAI` domain event

**File:** `crates/bus/src/domain_events.rs`

Current:
```rust
UserCorrectedAI {
    original: String,
    correction: String,
}
```

New:
```rust
UserCorrectedAI {
    original: String,
    correction: String,
    kind: CorrectionKind,
    strength: f64,
}
```

New enum (same file):
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionKind {
    Reaction,
    KeywordPrefix,
}
```

### 1b. Reaction-based correction

**File:** `crates/agent/src/agent_loop/mod.rs` — `handle_reaction()`

When `reaction_to_satisfaction()` returns `Some(score)` and score == `0.0` (negative):

1. Read last assistant message from `session_manager.get_or_create(session_key)` → find most recent `role == "assistant"`
2. Emit:
   ```rust
   bus.publish(DomainEvent::UserCorrectedAI {
       original: last_assistant_msg.unwrap_or("(unavailable)".into()),
       correction: format!("[reaction:{}]", emoji),
       kind: CorrectionKind::Reaction,
       strength: 1.0,
   });
   ```
3. Fire-and-forget: `trial_repo.mark_recent_messages_corrected(chat_id, 15).await`

### 1c. Keyword-based correction

**File:** `crates/agent/src/agent_loop/mod.rs` — `process_message()`, before pipeline

After session load, before `run_pipeline`:

1. Check if session's last message was `role == "assistant"`
2. Check if current user message matches a correction prefix (case-insensitive, first 80 chars)

**Keyword list (tiered by strength):**

| Strength | Phrases |
|----------|---------|
| 1.0 | `"no,"`, `"no "`, `"wrong"`, `"that's not"`, `"incorrect"` |
| 0.8 | `"i meant"`, `"try again"`, `"redo"`, `"not quite"`, `"never mind"` |

**Excluded from v1:** `"not "`, `"hold on"`, `"actually"`, `"wait"` — too noisy in normal conversation. Re-evaluate after gathering real usage data.

3. If matched:
   ```rust
   bus.publish(DomainEvent::UserCorrectedAI {
       original: last_assistant_msg,
       correction: user_message.to_string(),
       kind: CorrectionKind::KeywordPrefix,
       strength,
   });
   trial_repo.mark_recent_messages_corrected(chat_id, 15).await;
   ```

**Performance:** String prefix check on first 80 chars. Negligible cost.

**Rate limiting:** Max 1 keyword correction per 3 messages per session. Track with a simple counter on the session (reset after 3 non-correction messages). This prevents spam in rapid-fire chats while still capturing genuine corrections. Reaction-based corrections are NOT rate-limited (explicit user action = always valid).

### 1e. Correction acknowledgment (micro-interaction)

After emitting `UserCorrectedAI` (either reaction or keyword), set a flag on the session: `pending_correction_ack = true`. On the next assistant reply, if the flag is set, prepend a single warm line:

> "Noted — adjusting for next time."

Then clear the flag. This is presentation-layer polish — 5 lines of code, zero new UI. The exact phrasing can be made configurable later via user profile.

For reaction-based corrections where no immediate reply follows, the ack is skipped (the user isn't in a conversation flow). The flag expires after 5 minutes.

### 1d. `TrialRepo` access in `AgentLoop`

`AgentLoop` doesn't currently hold a `TrialRepo`. Two options:
- Inject `TrialRepo` into `AgentLoop` via the builder
- Access via `AppCore` if available

The builder already injects `strategy_repo: Option<StrategyRepo>`. Adding `trial_repo: Option<TrialRepo>` follows the same pattern. When `None` (autotuner disabled), the `mark_recent_messages_corrected` call is skipped.

---

## Section 2: Ground Truth Writing

### 2a. Signature change

**File:** `crates/agent/src/autotuner/hooks.rs`

```rust
// Old
async fn on_message_completed(
    &self, chat_id: &str, user_corrected: bool,
    tokens_used: u32, response_time_ms: u64,
);

// New
async fn on_message_completed(
    &self, chat_id: &str,
    orchestrator_name: &str, execution_mode: &str,
    tokens_used: u32, response_time_ms: u64,
);
```

Drop the `user_corrected: bool` parameter — it was always `false` and correction is now handled asynchronously via `mark_recent_messages_corrected`.

### 2b. Runtime call site

**File:** `crates/agent/src/agent_runtime/runtime.rs` — Step 11

The `IntentAnalysis` struct does not have `orchestrator_name` or `execution_mode` fields directly. The actual values are:
- **orchestrator name** → `agent_name` (a `String` local variable captured at Step 1 via `profile.name.clone()`)
- **execution mode** → `analysis.mode.short_name()` (returns `"direct"` or `"reactive"`)

```rust
if let Some(ref hook) = self.autotuner_hook {
    let tokens = router_result.usage.prompt_tokens + router_result.usage.completion_tokens;
    hook.on_message_completed(
        ctx.chat_id.as_str(),
        &agent_name,                    // NEW — skill/orchestrator name
        analysis.mode.short_name(),     // NEW — "direct" or "reactive"
        tokens,
        pipeline_elapsed_ms,
    ).await;
}
```

### 2c. Implementation

**File:** `crates/agent/src/autotuner/hooks.rs` — `AutoTunerHookImpl`

```rust
async fn on_message_completed(
    &self, chat_id: &str,
    orchestrator_name: &str, execution_mode: &str,
    tokens_used: u32, response_time_ms: u64,
) {
    if !self.orchestrator.is_active() { return; }

    if let Err(e) = self.trial_repo.update_shadow_log_ground_truth(
        chat_id, orchestrator_name, execution_mode,
    ).await {
        warn!(error = %e, "Failed to update shadow log ground truth");
    }
}
```

### 2d. New `TrialRepo` methods

**File:** `crates/storage/src/repos/trial_repo.rs`

```rust
/// Update the most recent shadow_log rows for a chat_id (within 60s)
/// where ground truth is still 'pending'.
pub async fn update_shadow_log_ground_truth(
    &self, chat_id: &str,
    control_orchestrator: &str, control_mode: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE autotuner_shadow_log
         SET control_orchestrator = ?1, control_mode = ?2
         WHERE chat_id = ?3
           AND control_orchestrator = 'pending'
           AND created_at >= datetime('now', '-60 seconds')"
    )
    .bind(control_orchestrator).bind(control_mode).bind(chat_id)
    .execute(&self.pool).await?;
    Ok(())
}

/// Mark recent shadow_log rows as user-corrected within a time window.
/// Uses subquery because SQLite does not support ORDER BY/LIMIT on UPDATE
/// without the SQLITE_ENABLE_UPDATE_DELETE_LIMIT compile flag.
pub async fn mark_recent_messages_corrected(
    &self, chat_id: &str, window_minutes: i32,
) -> Result<()> {
    sqlx::query(
        "UPDATE autotuner_shadow_log
         SET user_corrected = 1
         WHERE id IN (
             SELECT id FROM autotuner_shadow_log
             WHERE chat_id = ?1
               AND created_at >= datetime('now', ?2)
             ORDER BY created_at DESC LIMIT 2
         )"
    )
    .bind(chat_id)
    .bind(format!("-{} minutes", window_minutes))
    .execute(&self.pool).await?;
    Ok(())
}
```

---

## Section 3: Metric Collector — Replacing Placeholders

### 3a. New `AgentMetricCollector` dependencies

**File:** `crates/agent/src/autotuner/metric_collector.rs`

```rust
pub struct AgentMetricCollector {
    strategy_repo: StrategyRepo,     // existing
    event_log_repo: EventLogRepo,    // NEW
    usage_repo: UsageRepo,           // NEW
    trial_repo: TrialRepo,           // NEW
}
```

### 3b. Metric implementations

```rust
// 1. correction_rate — from domain_event_log
let correction_count = self.event_log_repo
    .count_by_event_type("UserCorrectedAI", since).await?;
let total = stats.total_records.max(1);
let correction_rate = (correction_count as f64 / total as f64).min(1.0);

// 2. avg_tokens_per_message — from usage_records
let total_tokens = self.usage_repo.total_tokens_since(since).await?;
let (total_requests, _) = self.usage_repo.totals_since(since).await?;
let avg_tokens = total_tokens as f64 / total_requests.max(1) as f64;

// 3. routing_stability — from shadow_log agreement rate
let routing_stability = self.trial_repo
    .shadow_log_agreement_rate(trial_id, since).await?;

// 4. memory_relevance — stays 1.0 (Phase 2)
let memory_relevance = 1.0;
```

### 3c. New repo methods

**`EventLogRepo::count_by_event_type`** (`crates/cognitive/src/repos/event_log.rs`):
```rust
pub async fn count_by_event_type(
    &self, event_type: &str, since: DateTime<Utc>,
) -> Result<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM domain_event_log
         WHERE event_type = ?1 AND timestamp >= ?2"
    )
    .bind(event_type).bind(since.to_rfc3339())
    .fetch_one(&self.pool).await?;
    Ok(row)
}
```

**`UsageRepo::total_tokens_since`** (`crates/storage/src/repos/usage.rs`):
```rust
pub async fn total_tokens_since(&self, since: DateTime<Utc>) -> Result<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)
         FROM usage_records WHERE timestamp >= ?1"
    )
    .bind(since.to_rfc3339()).fetch_one(&self.pool).await?;
    Ok(row)
}
```

**`TrialRepo::shadow_log_agreement_rate`** (`crates/storage/src/repos/trial_repo.rs`):

Uses a single conditional-aggregate query instead of two round-trips:

```rust
pub async fn shadow_log_agreement_rate(
    &self, trial_id: Option<&str>, since: DateTime<Utc>,
) -> Result<f64> {
    let since_str = since.to_rfc3339();

    let (total, agreed): (i64, i64) = if let Some(tid) = trial_id {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
             FROM autotuner_shadow_log
             WHERE trial_id = ?1 AND control_mode != 'pending'
               AND created_at >= ?2"
        ).bind(tid).bind(&since_str).fetch_one(&self.pool).await?
    } else {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
             FROM autotuner_shadow_log
             WHERE control_mode != 'pending' AND created_at >= ?1"
        ).bind(&since_str).fetch_one(&self.pool).await?
    };

    Ok(if total == 0 { 1.0 } else { agreed as f64 / total as f64 })
}
```

**`TrialRepo::count_trials_since`** and **`TrialRepo::count_promoted_since`** (for `learning_health`):

```rust
pub async fn count_trials_since(&self, since: DateTime<Utc>) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM autotuner_trials
         WHERE status IN ('completed', 'promoted', 'reverted')
           AND completed_at >= ?1"
    ).bind(since.to_rfc3339()).fetch_one(&self.pool).await
}

pub async fn count_promoted_since(&self, since: DateTime<Utc>) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM autotuner_trials
         WHERE status = 'promoted' AND completed_at >= ?1"
    ).bind(since.to_rfc3339()).fetch_one(&self.pool).await
}
```

### 3d. Wiring

**File:** `crates/app-core/src/init/cron.rs`

The `AgentMetricCollector` constructor gains three new args. All repos are available at the init site:
- `repos.strategies` — existing
- `repos.event_log` — from cognitive repos (need to verify accessor name)
- `repos.usage` — from storage repos
- `trial_repo` — already constructed at this point

The `MetricSource` trait signature stays unchanged (`collect_metrics(since, trial_id) -> MetricSnapshot`). Only the implementation changes.

---

## Section 4: Observability

### 4a. Structured logging at decision points

**File:** `crates/agent/src/autotuner/mod.rs` (nightly cycle callback, which invokes `NightlyCycle` from the pure `autotuner` crate)

Three log lines:

```rust
// After evaluating each trial
info!(
    trial_id = %trial_id,
    correction_rate = %format!("{:.3}", result.correction_rate),
    accuracy = %format!("{:.3}", result.classification_accuracy),
    messages = result.messages_scored,
    verdict = if verdict.passes_all() { "PASS" } else { "FAIL" },
    "Autotuner: trial evaluated"
);

// On promotion
info!(
    trial_id = %winner_id,
    improvement_pct = %format!("{:.1}", correction_improvement_pct),
    "Autotuner: trial PROMOTED to champion"
);

// On regression
warn!(
    days = champion.consecutive_regression_days,
    threshold = config.rollback_after_days,
    "Autotuner: champion regression detected"
);
```

### 4b. `AutotunerDecision` domain event

**File:** `crates/bus/src/domain_events.rs`

New variant:
```rust
AutotunerDecision {
    trial_id: String,
    verdict: String,          // "promoted" | "rolled_back" | "unchanged"
    improvement_pct: f64,
    affected_params: Vec<String>,  // which TrialParams fields were changed
}
```

Emitted in the nightly cycle callback after promotion or rollback. The cognitive pipeline will:
- Persist to `domain_event_log` (automatic, via existing subscriber)
- Extract as a memory fact via salience rules (needs one new rule: `AutotunerDecision` → `SalienceVerdict::Extract`)

**Important:** Both `event_to_observation()` and `event_type_key()` in `crates/cognitive/src/services/background.rs` are exhaustive `match` blocks on `DomainEvent`. Adding `AutotunerDecision` (and extending `UserCorrectedAI` with new fields) requires updating these match arms. For `AutotunerDecision`, map to an `Observation` with domain `"meta"`, importance `0.8`, and a descriptive text from the verdict + affected_params. For the extended `UserCorrectedAI`, update the existing destructuring pattern to include `kind` and `strength` fields (or use `..` to ignore them if not needed in the observation).

### 4c. `brain_growth` in status response (user-facing)

**File:** `crates/app-core/src/handlers/autotuner.rs` — `autotuner_status`

Named `brain_growth` (not `learning_health`) — clinical language kills the second-brain magic. This is the field that Insights, Coaching, and a future "Brain Health" glassmorphism widget will reference.

```rust
brain_growth: BrainGrowth {
    corrections_captured_7d: i64,    // from EventLogRepo
    trials_evaluated_7d: i32,        // from TrialRepo
    promoted_this_week: i32,         // from TrialRepo
    status: String,                  // "growing" | "needs_feedback" | "adapting"
}
```

Status derivation:
- `"needs_feedback"` — if `corrections_captured_7d == 0` or total messages < 50
- `"adapting"` — if corrections exist but no promotions yet
- `"growing"` — if at least one promotion has occurred

### 4d. `metrics_health` in status response (debug/internal)

```rust
metrics_health: MetricsHealth {
    correction_rate_available: bool,   // corrections_captured_7d > 0
    token_rate_available: bool,        // usage_records exist
    stability_available: bool,         // shadow_log has non-pending rows
}
```

### 4e. Cognitive memory extraction on adaptation

The `AutotunerDecision` domain event (Section 4b) flows through the existing cognitive pipeline. In `event_to_observation()`, map it to a descriptive observation text that the Reflection/InsightForge pipeline can surface:

- On promotion: `"I refined my {affected_params} after {correction_count} corrections — routing improved {improvement_pct}%"`
- On rollback: `"I reverted a recent change to {affected_params} — it wasn't working well"`

This lets Insights and Coaching naturally reference real progress: "Based on your feedback, I've gotten better at task routing." The existing pipeline does all the heavy lifting — we just need to feed it a well-structured observation.

---

## Files Modified (Complete List)

| File | Change Type | Description |
|------|-------------|-------------|
| `crates/bus/src/domain_events.rs` | Modify | Extend `UserCorrectedAI` (add `kind`, `strength`), add `CorrectionKind` enum, add `AutotunerDecision` variant |
| `crates/agent/src/agent_loop/mod.rs` | Modify | Emit corrections in `handle_reaction()` + keyword detection in `process_message()`. Rename `_domain_event_bus` → `domain_event_bus` (no longer unused) |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Inject `TrialRepo` into `AgentLoop`, update bus field name |
| `crates/agent/src/autotuner/hooks.rs` | Modify | Implement `on_message_completed`, update trait signature (drop `user_corrected`, add `orchestrator_name` + `execution_mode`) |
| `crates/agent/src/autotuner/metric_collector.rs` | Modify | Replace 4 placeholders, add 3 new repo dependencies |
| `crates/agent/src/autotuner/mod.rs` | Modify | Add logging, emit `AutotunerDecision` in nightly cycle callback |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Pass `agent_name` + `analysis.mode.short_name()` to hook |
| `crates/storage/src/repos/trial_repo.rs` | Modify | Add `update_shadow_log_ground_truth`, `mark_recent_messages_corrected`, `shadow_log_agreement_rate`, `count_trials_since`, `count_promoted_since` |
| `crates/storage/src/repos/usage.rs` | Modify | Add `total_tokens_since` |
| `crates/cognitive/src/repos/event_log.rs` | Modify | Add `count_by_event_type` |
| `crates/cognitive/src/services/salience.rs` | Modify | Add salience rule for `AutotunerDecision`, update `UserCorrectedAI` test construction to include new fields |
| `crates/cognitive/src/services/background.rs` | Modify | Update `event_type_key()` + `event_to_observation()` exhaustive matches for `AutotunerDecision` variant + extended `UserCorrectedAI` fields |
| `crates/app-core/src/init/cron.rs` | Modify | Wire new repos into `AgentMetricCollector` |
| `crates/app-core/src/handlers/autotuner.rs` | Modify | Add `learning_health` + `metrics_health` to status |

**No new files.** All changes modify existing code.

### Implementation notes

- **`_domain_event_bus` rename (B5):** The field `AgentLoop._domain_event_bus` has a leading underscore to suppress unused-field warnings. Since we now actively use it in `handle_reaction()` and `process_message()`, rename to `domain_event_bus`. Update all builder/initialization sites.
- **`correction_rate` ceiling (N1):** The correction rate denominator uses `strategy_records` count, but corrections fire before the pipeline writes a strategy record. In pathological scenarios (e.g., rapid-fire corrections), `correction_rate` could briefly exceed 1.0. The metric collector should clamp: `correction_rate.min(1.0)`.
- **Timestamp binding consistency (W4):** All new repo methods should use `.to_rfc3339()` for timestamp binding, since `domain_event_log.timestamp` and `autotuner_shadow_log.created_at` are `TEXT` columns. The `usage_records.timestamp` column is also stored as text in practice — use `.to_rfc3339()` consistently.
- **SQLite write serialization (N4):** Concurrent `mark_recent_messages_corrected` calls are safe because SQLite serializes writes at the connection level. No additional locking is needed.

---

## Testing Strategy

### Unit tests (per-crate)

| Test | Crate | What it validates |
|------|-------|-------------------|
| `keyword_correction_detection` | `agent` | Prefix matching with tiered strength |
| `reaction_emits_correction_event` | `agent` | Negative reaction → bus publish |
| `ground_truth_updates_shadow_log` | `agent` | `on_message_completed` writes orchestrator + mode |
| `mark_corrected_within_window` | `storage` | `mark_recent_messages_corrected` respects time window |
| `count_by_event_type_filters_correctly` | `cognitive` | EventLogRepo query filters by type + time |
| `total_tokens_since` | `storage` | UsageRepo aggregation |
| `shadow_log_agreement_rate` | `storage` | Agreement ratio with pending exclusion, zero-row default |
| `metric_collector_computes_real_values` | `agent` | All 4 metrics are non-placeholder when data exists |
| `metric_collector_handles_empty_data` | `agent` | Graceful defaults when no data (correction_rate=0, stability=1.0) |

### Integration tests (facade crate)

| Test | What it validates |
|------|-------------------|
| `autotuner_feedback_loop_e2e` | Emit correction → nightly cycle → evaluator sees real correction_rate |
| `negative_reaction_marks_shadow_log` | Reaction → mark_recent_messages_corrected → row updated |

---

## Success Criteria

1. **`correction_rate` in MetricSnapshot is non-zero** after a user sends a negative reaction or correction keyword
2. **`avg_tokens_per_message` in MetricSnapshot is non-zero** after any LLM interaction
3. **`routing_stability` in MetricSnapshot is < 1.0** when shadow predictions disagree with actual routing
4. **Shadow log rows have real `control_orchestrator` and `control_mode`** instead of `"pending"`
5. **First "Trial promoted" or "Trial evaluated: FAIL" log line** appears after the nightly cycle runs with real data
6. **`autotuner_status` command returns `learning_health.status != "needs_data"`** after sufficient message volume

---

## What This Does NOT Do (Phase 2)

- Replace generation context placeholders (`trend_summary`, `behavioral_context`, `memory_snapshot`)
- Compute `memory_relevance` from context engine retrieval scoring
- Per-trial correction rates (requires chat_id + time join between shadow_log and domain_event_log)
- UX acknowledgment messages on correction ("Got it — thanks for the correction")
- Desktop UI for learning health visualization (widget / insights tab integration)
