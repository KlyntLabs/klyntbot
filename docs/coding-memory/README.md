# Coding Memory

## Phase 3 — Distiller write path + Tier A/B activation (shipped 2026-04-25)

The Distiller turns persisted ingest events into cognitive memory rows.

Pipeline (per turn):

- **Phase A — Extractive.** `compute_turn_trace` builds a `TurnTrace` from ordered events and persists `episodic_memories { kind: 'turn_trace' }`. Always lands first — Phase B/C failures never block it.
- **Phase B — LLM synthesis.** Calls `ProviderManager` with the `record_observation` tool schema. Bounded by `DistillerConfig::timeout`. Transient failures enqueue into `ingest_distillation_retry`; malformed responses are dropped.
- **Phase C — Reconciliation.** Each `Observation` is mapped to a `Prepared::Fact` or `Prepared::Episode` by `FactBuilder`. Facts go through `reconcile()` → `NOOP` (bump access), `SUPERSEDE` (close `valid_until` + set `superseded_by`), or `ADD`.

**Tier B1 — counterfactual derivation.** When a `FixAttempt` observation reports `outcome ∈ {failure, abandoned}`, the Distiller derives a `DeadEndAttempt` via `counterfactual::derive_dead_end` and writes it as `SemanticFact { memory_type: 'counterfactual', source: 'distiller_counterfactual' }`. Reforge (Phase 5) consumes these to avoid repeating dead ends.

**Tier B3** — `CodeState` enum on `UserSituationSnapshot` for coding-vs-noncoding stratification.
**Tier B4** — `CodeDomainSearcher` registered with InsightForge.
**Tier B5** — `ShadowContext.session_type` for per-CLI autotuner trials.

**Sweepers.** `AppCore::init` spawns two periodic tickers tied to `shutdown_token`: `Distiller::sweep_idle` (clears stale `processing` claims) and `Distiller::sweep_retries` (drains backoff-eligible retry rows).

**Workbench panels.** Memory Browser, Activity Timeline, Cost Tracker, Sensitivity Inspector — backed by Tauri commands `coding_memory_browser` / `_activity` / `_cost` / `_sensitivity` (handlers in `app-core/src/coding_memory/handlers.rs`).

**Invariants under property test:**

1. Provenance always non-empty — `tests/prop_provenance_invariant.rs`
2. Bi-temporal monotone (`recorded_at >= valid_from`) — `tests/prop_bi_temporal.rs`
3. SUPERSEDE-chain equality — `tests/prop_supersede_chain.rs`
5. Distiller never deletes — `tests/prop_distiller_never_deletes.rs`

End-to-end: `tests/distiller_end_to_end.rs`. Tier A activation: `tests/tier_a_activation.rs`.

## Phase 2 — Ingestion transport + Claude Code E2E (shipped 2026-04-24)

Components newly live:

- `UnixIngestSocket` / `FileBufferFallback` — 200ms socket deadline; 50 MB rotate / 7 d TTL / 500 MB hard cap for the cold path.
- `HookClient` — socket-first-else-buffer dispatcher with rate-limited stderr warnings.
- `IngestDaemon` — binds `~/.klyntbot/ingest.sock`, decodes length-prefixed JSON, persists rows to `ingest_event_log`, drains any pre-existing buffer on startup, heartbeats `desktop.lock` every 30 s.
- Claude Code adapter — 7 hook events (`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact`). Bash + test-framework detection emits `TestRun`; file-ops emit `FileEdit`.
- `ClaudeCodeInstaller` — idempotent `~/.claude/settings.json` merge with a pre-install backup; the `klyntbot-managed` matcher tag lets users keep their own hooks alongside.
- Workbench: Coding CLI settings page (toggle + Diagnose), CLI Health panel, Session Replay panel.

Unchanged: Distiller / Recall / Reforge / Mirror coding behavior all remain Phase 1 stubs. No facts are written to `semantic_facts` or `episodic_memories` yet — only `ingest_event_log` rows accumulate.

Exit-gate evidence: `tests/integration/coding_memory_phase2_roundtrip.rs`, `tests/integration/coding_memory_phase2_desktop_off.rs`.
