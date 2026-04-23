# v3 Audit Decisions (Tasks 58–61)

## UserSituation — KEEP

**Spec asked:** Delete if not wired.
**Audit found:**
- `crates/cognitive/src/services/situation.rs:12` — defines `UserSituation`, `SituationInputs`, `compute_situation`.
- `crates/cognitive/src/services/memory_retriever.rs:51,101` — `MemoryRetriever` holds `Option<Arc<Mutex<UserSituation>>>` and exposes `with_situation()`.
- `crates/feature-coaching/src/signal_accumulator/mod.rs:12,59,86` — evaluates heuristic triggers against `UserSituation`.
- `crates/feature-coaching/src/service.rs` — coaching service holds and updates the situation.

**Decision:** Keep. The type is actively used by the coaching and memory-retrieval subsystems.

---

## MetaRule — KEEP

**Spec asked:** Either implement or purge references.
**Audit found:**
- `crates/cognitive/src/mirror/types.rs` — defines `MetaRule`, `MetaRuleAction`, `MetaRuleSource`, `MetaRuleStatus`.
- `crates/cognitive/src/mirror/facade.rs` — uses it in `get_meta_rules`, `create_meta_rule_from_text`, `approve`, `dismiss`.
- `crates/cognitive/src/services/reforge/collector.rs:198` — `pending_meta_rules()` queries it.
- `crates/cognitive/src/mirror/narratives.rs` — `MetaRuleProposer` trait and proposal context.

**Decision:** Keep. The type is implemented and integrated into the Mirror/Reforge flow.

---

## Squad System — KEEP (clarified scope)

**Spec asked:** Squad system exists only at repo level with zero agent integration — decide: integrate into insight generation or delete.
**Audit found:**
- `crates/cognitive/src/repos/squad.rs` — defines `SquadRepo`.
- `crates/app-core/src/state.rs:125` — `AppState::squad_repo: Option<SquadRepo>` carried throughout.
- `crates/app-core/src/handlers/squads.rs` — full HTTP handler set for squad CRUD.
- `crates/app-core/src/handlers/chat/sessions.rs:105` — enriches squad data in sessions.
- `crates/app-core/src/handlers/chat/streaming.rs:279` — used in chat streaming.

**Decision:** Keep. The squad system is wired into chat handling via `app-core`, which is the layer that mediates agent ↔ data access. Integration is sufficient at this layer.

---

## Orphan ContextSource — NONE FOUND

**Spec asked:** Audit for orphan `ContextSource` implementations.
**Audit found:** All 16 implementations (across `cognitive`, `agent/context_sources`, `activity-log`, `skill-system`) are registered and active. No deletion needed.
