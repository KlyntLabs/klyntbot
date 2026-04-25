# Coding Memory — Phase 3 Implementation Summary

## Overview

Phase 3 wires the **Distiller** end-to-end: events flow from CLI hooks → ingestion daemon → real-time Distiller → semantic facts + episodic memories. Phase A (extractive) always runs; Phase B (LLM synthesis) and Phase C (reconciliation) enrich the memory graph. Transient failures are retried; the system never deletes facts.

## Architecture

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────┐
│ klynt-cli   │────▶│ IngestDaemon    │────▶│ Distiller   │
│ (hook)      │     │ (Unix socket)   │     │             │
└─────────────┘     └─────────────────┘     └──────┬──────┘
                                                   │
                         ┌─────────────────────────┼─────────────────────────┐
                         │                         │                         │
                         ▼                         ▼                         ▼
                   ┌──────────┐            ┌──────────┐            ┌──────────┐
                   │ Phase A  │            │ Phase B  │            │ Phase C  │
                   │ TurnTrace│            │ LLM obs  │            │ Reconcile│
                   │(episodic)│            │(semantic)│            │(semantic)│
                   └──────────┘            └──────────┘            └──────────┘
```

## Key Components

### Distiller (`crates/coding-memory/src/distiller/`)

| Module | Purpose |
|--------|---------|
| `mod.rs` | Handle — `accept_event`, `sweep_idle`, `sweep_retries`, `distill_turn` |
| `turn_buffer.rs` | Detects boundaries on `AssistantMsg` + `token_usage`, `SessionEnd`, idle timeout |
| `phase_a.rs` | Extractive pass — builds `TurnTrace` from events, persists to `episodic_memories` |
| `phase_b.rs` | Prompt construction + LLM invocation via `ProviderManager` with timeout |
| `phase_c.rs` | Mem0-style reconciliation: NOOP (>0.9 + exact), SUPERSEDE (>0.75), ADD |
| `writer.rs` | Single write chokepoint — enforces provenance-always invariant |
| `retry_queue.rs` | `ingest_distillation_retry` table — backoff: 1m → 5m → 30m |
| `fact_builder.rs` | Converts `record_observation` tool calls to `SemanticFact` / `EpisodicMemory` |
| `record_observation.rs` | LLM tool schema + JSON decoder |

### IngestDaemon (`crates/coding-ingest/src/daemon.rs`)

- Accepts `AgentEvent` via Unix socket
- Persists to `ingest_event_log` (SQLite)
- **New in Phase 3**: Optional real-time forwarding via `event_tx` → Distiller
- Buffer drain on startup for desktop-off resilience

### AppCore Wiring (`crates/app-core/src/init/mod.rs`)

1. Constructs `Distiller` with:
   - `IngestEventLogRepo` (shared with daemon)
   - `DistillerWriter` (`SemanticFactRepo` + `EpisodicMemoryRepo`)
   - `ProviderManager` (from app config or failover)
   - `UnifiedMemoryService` retriever
   - `DistillationRetryRepo`
2. Passes `event_tx` to `IngestDaemonConfig`
3. Spawns:
   - Event receiver task (`accept_event` loop)
   - Idle sweep ticker (60s)
   - Retry sweep ticker (60s)

## Invariants

1. **Provenance-always**: Every Distiller-authored row has non-empty `source_events`. Enforced by `debug_assert!` (dev panic) + `Err(ProvenanceMissing)` in all builds.
2. **Bi-temporal monotone**: `valid_from` only moves forward; `valid_until` and `superseded_by` are set once.
3. **SUPERSEDE chain equality**: Predecessor's `valid_until` + `superseded_by` exactly match successor's `valid_from` + `id`.
4. **Distiller never deletes**: Reconciliation only NOOPs, SUPERSEDEs, or ADDs. No `DELETE`.

## Failure Handling

| Failure | Behavior |
|---------|----------|
| LLM timeout | Log warning, enqueue retry (`RetryReason::LlmTimeout`) |
| LLM provider error | Log warning, enqueue retry (`RetryReason::LlmProvider`) |
| Transient storage error | Log warning, enqueue retry (`RetryReason::Transient`) |
| Malformed LLM output | Log warning, **drop** (no retry — bad data) |
| Turn already in flight | Return `DistillationReport::default()` (idempotent) |

## Retry Backoff

```
Attempt 0 → +1 minute
Attempt 1 → +5 minutes
Attempt 2+ → +30 minutes
```

Permanent failure after max attempts (implicit — rows stay in queue with 30m backoff).

## Frontend Panels (`desktop-ui/src/features/coding-memory/`)

| Panel | Hook | Route |
|-------|------|-------|
| Memory Browser | `useMemoryBrowser` | `/coding-memory/memory` |
| Activity Timeline | `useActivityTimeline` | `/coding-memory/activity` |
| Cost Tracker | `useCostRollup` | `/coding-memory/cost` |
| Sensitivity Inspector | `useSensitivityInspector` | `/coding-memory/sensitivity` |

All panels are co-located with tests in `__tests__/`.

## Tier B Fields

| Tier | Field | Location |
|------|-------|----------|
| B3 | `code_state: Option<String>` | `UserSituationSnapshot` |
| B4 | `CodeDomainSearcher` | `coding_memory::code_domain_searcher` |
| B5 | `session_type: Option<String>` | `ShadowContext` |

## Testing

- **57 backend tests** across `crates/coding-memory/tests/` (property tests + integration tests)
- **83 frontend tests** in `desktop-ui/` (20 test files)
- Key property tests:
  - `prop_provenance_invariant`
  - `prop_bi_temporal`
  - `prop_supersede_chain`
  - `prop_distiller_never_deletes`

## Migration

Phase 3 adds migration `002_retry_queue.sql`:

```sql
CREATE TABLE ingest_distillation_retry (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_id TEXT,
    reason TEXT NOT NULL,
    attempt_count INTEGER DEFAULT 0,
    next_due_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_ingest_distillation_retry_due
    ON ingest_distillation_retry(next_due_at);
```

Run via `StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())`.
