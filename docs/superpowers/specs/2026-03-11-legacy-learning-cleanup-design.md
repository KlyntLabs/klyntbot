# Legacy Learning System Cleanup

> Design spec for recommendation #6 from system-architecture-analysis.md
> Date: 2026-03-11
> Scope: Phases 1+2 (delete zombie tables + migrate behavioral_patterns to L5)

## Problem

The L2 learning system (`storage/migrations/003_learning_system.sql`) creates four tables that overlap with or are superseded by the L5 cognitive system:

| L2 Table | Status | Overlap |
|----------|--------|---------|
| `user_profile` | Zombie — never written in production, read-only for transparency | Fully superseded by L5 `semantic_facts` |
| `agent_adaptations` | Zombie — never written in production, read-only for transparency | Per-agent prefs concept unique but unused |
| `behavioral_patterns` | Active — written by `PatternAnalyzer` every ~60s | Overlaps with L5 procedural rules |
| `interaction_log` | Active — written every message by `InteractionRecorder` | No L5 equivalent — keep as-is |

The dual authority creates confusion about which system is the source of truth for user understanding.

## Solution: Two-Phase Cleanup

### Phase 1: Delete Zombie Tables

Remove `user_profile` and `agent_adaptations` — dead code with no production writes.

**Delete files:**
- `crates/storage/src/repos/user_profile.rs` — `UserProfileRepo` (244 lines)
- `crates/storage/src/repos/agent_adaptation.rs` — `AgentAdaptationRepo` (118 lines)

**Modify files:**
- `crates/storage/src/repos/mod.rs` — remove `UserProfileRepo` and `AgentAdaptationRepo` from `Repos` struct and module declarations
- `crates/storage/src/rows/learning.rs` — remove `UserProfileRow` and `AgentAdaptationRow` structs
- `crates/agent/src/agent_runtime/runtime.rs`:
  - Remove `learning_user_profile` and `learning_adaptations` fields from `AgentRuntime` struct (lines 77, 79)
  - Remove their initialization in `new()` (lines 110, 112)
  - Remove `with_learning_repos` assignments for these two (lines 141, 143)
  - Remove `user_profile` and `adaptations` blocks from `emit_learning_summary()` (lines 510-516 destructure, 525-542, 561-575)
  - Simplify the destructure to only use `learning_patterns` (which gets removed in Phase 2)

**Add migration:**
```sql
-- Phase 1: Remove zombie L2 tables
DROP TABLE IF EXISTS user_profile;
DROP TABLE IF EXISTS agent_adaptations;
```

### Phase 2: Migrate behavioral_patterns to L5 Cognitive Pipeline

Redirect `PatternAnalyzer` to emit `DomainEvent` variants instead of writing to `behavioral_patterns`. The existing cognitive consolidation pipeline (background service → salience → extraction → consolidation) processes these into semantic facts and procedural rules automatically.

**New `DomainEvent` variant** in `crates/bus/src/domain_events.rs`:

```rust
// -- Learning patterns (migrated from L2) --
BehavioralPatternDetected {
    pattern_type: String,   // "day_of_week", "time_of_day", "agent_usage"
    pattern_key: String,    // e.g., "monday_task", "morning", "task"
    sample_count: i32,
    detail: String,         // human-readable description for extraction
},
```

**Salience mapping** in `crates/cognitive/src/salience.rs`:

```rust
DomainEvent::BehavioralPatternDetected { .. } => SalienceVerdict::Accumulate,
```

`Accumulate` is correct — patterns are routine observations that should be buffered and promoted after reaching the threshold (≥5 events across ≥3 days), then consolidated into procedural rules via the weekly reflection cycle.

**Rewrite `PatternAnalyzer`** in `crates/agent/src/learning/pattern_analyzer.rs`:

Replace `BehavioralPatternRepo` dependency with `Arc<DomainEventBus>`. Instead of `self.pattern_repo.upsert(...)`, call `self.event_bus.publish(DomainEvent::BehavioralPatternDetected { ... })`.

The `detail` field should contain a human-readable description for the LLM extraction step, e.g.:
- `"User uses task agent frequently on Mondays (15 interactions)"`
- `"User is most active in the morning (47 interactions)"`
- `"Task agent is the most-used agent (82 total uses)"`

**Update `LearningService`** in `crates/agent/src/learning/service.rs`:

- `PatternAnalyzer::new()` takes `InteractionLogRepo` + `Arc<DomainEventBus>` instead of `InteractionLogRepo` + `BehavioralPatternRepo`
- `LearningService` already has access to the event bus concept; wire it through

**Update transparency in `AgentRuntime`:**

Replace the `behavioral_patterns` transparency block (lines 544-558) with a read from L5 procedural rules. The `AgentRuntime` needs a `ProceduralRuleRepo` (from cognitive crate) to query active rules.

```rust
// Behavioral patterns (from L5 procedural rules)
if let Ok(rules) = procedural_rule_repo.list_active("productivity").await {
    if !rules.is_empty() {
        let previews: Vec<&str> = rules.iter().take(3).map(|r| r.rule_text.as_str()).collect();
        let _ = tx.send(AgentEvent::LearningEvent {
            event_type: "patterns".into(),
            detail: format!("{} learned rules ({})", rules.len(), previews.join(", ")),
        }).await;
    }
}
```

**Delete files:**
- `crates/storage/src/repos/behavioral_pattern.rs` — `BehavioralPatternRepo` (128 lines)

**Modify files:**
- `crates/storage/src/repos/mod.rs` — remove `BehavioralPatternRepo` from `Repos` struct
- `crates/storage/src/rows/learning.rs` — remove `BehavioralPatternRow` struct
- `crates/agent/src/agent_runtime/runtime.rs` — remove `learning_patterns` field entirely, replace transparency read with `ProceduralRuleRepo`
- `crates/agent/src/learning/pattern_analyzer.rs` — rewrite to use `DomainEventBus`
- `crates/agent/src/learning/service.rs` — update `PatternAnalyzer` construction
- `crates/agent/src/agent_loop/builder.rs` — update builder to pass event bus to `PatternAnalyzer`

**Add migration:**
```sql
-- Phase 2: Remove behavioral_patterns (migrated to L5 cognitive pipeline)
DROP TABLE IF EXISTS behavioral_patterns;
```

### What Stays Unchanged

- **`interaction_log` table** — actively used, no L5 equivalent, keeps its repo and `InteractionRecorder`
- **`003_learning_system.sql`** — the original migration file stays (SQLite migrations are append-only); new migrations add the DROP statements
- **`LearningService` core loop** — outcome analysis + adaptive thresholds unchanged; only pattern analyzer wiring changes
- **`InteractionRecorder`** — continues writing to `interaction_log` as before

## Testing Strategy

| Test | Location | Validates |
|------|----------|-----------|
| PatternAnalyzer emits events | `agent/src/learning/pattern_analyzer.rs` | Events published to bus instead of repo writes |
| Salience mapping | `cognitive/src/salience.rs` | `BehavioralPatternDetected` → `Accumulate` |
| Transparency reads procedural rules | `agent/src/agent_runtime/runtime.rs` | Learning summary uses L5 rules |
| LearningService lifecycle | `agent/src/learning/service.rs` | Service still starts/stops/triggers correctly |
| No compile errors from removed repos | `cargo build --workspace` | All references to deleted repos are cleaned up |

## Files Changed Summary

| Action | File |
|--------|------|
| Delete | `crates/storage/src/repos/user_profile.rs` |
| Delete | `crates/storage/src/repos/agent_adaptation.rs` |
| Delete | `crates/storage/src/repos/behavioral_pattern.rs` |
| Modify | `crates/storage/src/repos/mod.rs` — remove 3 repos from Repos struct |
| Modify | `crates/storage/src/rows/learning.rs` — remove 3 row structs |
| Modify | `crates/bus/src/domain_events.rs` — add `BehavioralPatternDetected` variant |
| Modify | `crates/cognitive/src/salience.rs` — add salience mapping |
| Modify | `crates/agent/src/learning/pattern_analyzer.rs` — rewrite to use event bus |
| Modify | `crates/agent/src/learning/service.rs` — update PatternAnalyzer construction |
| Modify | `crates/agent/src/agent_runtime/runtime.rs` — remove L2 fields, read from L5 |
| Modify | `crates/agent/src/agent_loop/builder.rs` — update wiring |
| Add | New migration — DROP 3 L2 tables |
