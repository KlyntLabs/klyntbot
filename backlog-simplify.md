# Simplify Backlog — Klynt Workspace

> Reference: `crates/storage/` and `crates/app-core/` are **DONE**.
> Use them as the baseline for patterns (batch-repo macros, error mappers, `tokio::join!`, helper extraction).

---

## Legend

| Priority | Meaning |
|----------|---------|
| 🔴 P0 | Do next — high impact, clear wins, manageable risk |
| 🟡 P1 | Do soon — valuable but larger or needs design decision |
| 🟢 P2 | Backlog — nice to have, small crates, or requires migration |
| ⚪ Skip | Too small, stable, or blocked by external deps |

| Complexity | Meaning |
|------------|---------|
| Low | < 3k lines, focused domain, few cross-crate deps |
| Medium | 3–10k lines, some duplication, moderate cross-crate surface |
| High | > 10k lines, heavy duplication, architectural boundaries blurry |

---

## Phase 1 — Quick Clippy Wins (🔴 P0)

These crates have auto-fixable or near-auto-fixable clippy warnings. 
Low risk, immediate cleanliness improvement.

| # | Crate | Lines | Files | Warnings | Complexity | Action |
|---|-------|-------|-------|----------|------------|--------|
| 1 | `klynt-core` | ~5.9k | 36 | **10** | Medium | `cargo clippy --fix` + manual review of approval/matcher code |
| 2 | `agent` | ~33k | 120 | **8** | High | `cargo clippy --fix` (5 auto), then audit `cognitive_handlers.rs` god-file |
| 3 | `cognitive` | ~38k | 123 | **6** | High | `cargo clippy --fix` (5 auto), then tackle `freshness_label` / RRF dupes |
| 4 | `coding-ingest` | ~2.6k | 38 | **3** | Medium | `cargo clippy --fix` (1 auto), dedup adapter boilerplate |
| 5 | `klynt-hooks` | ~0.9k | 25 | **3** | Low | `cargo clippy --fix` (2 auto) |
| 6 | `klynt-execpolicy` | ~1.9k | 11 | **3** | Low | `cargo clippy --fix` (1 auto) |
| 7 | `desktop` | ~14k | 99 | **3** | Medium | `cargo clippy --fix` (2 auto) |
| 8 | `desktop-shared` | ~4.7k | 47 | **1** | Low | `cargo clippy --fix` (1 auto) |
| 9 | `feature-launcher` | ~8.5k | 51 | **1** | Medium | `cargo clippy --fix` (1 auto) |
| 10 | `coding-memory` | ~11.6k | 87 | **1** | High | `cargo clippy --fix` (1 auto) |

**Reference patterns from `app-core`:**
- Run `cargo clippy --workspace --fix` and review each crate separately.
- Apply `#[allow(clippy::too_many_arguments)]` only to genuinely complex dispatch functions (like `chat_send`).
- Replace `unwrap_or_else(Default::default)` with `unwrap_or_default()`.
- Remove needless borrows (`&path.join(...)` → `path.join(...)`).

---

## Phase 2 — Adapter & Handler Deduplication (🟡 P1)

These crates have **clear structural duplication** that can't be auto-fixed. 
Each needs 1–3 focused PRs.

### 2.1 `agent` — Adapter God-Files & Builder Bloat

| Issue | Location | Suggested Fix | Effort |
|-------|----------|---------------|--------|
| **Cognitive handlers god-file** | `adapters/cognitive_handlers.rs` (2,387 lines) | Split into `cognitive_handlers/{extraction, consolidation, coaching, conflict, graph, critic, predictor}.rs` | Medium |
| **Stop-word lists duplicated** | `adapters/query_rewriter.rs` vs `domain_searchers/mod.rs` | Extract shared `STOP_WORDS` to `agent::domain_searchers::stop_words` or config | Small |
| **Tree builder partial dedup** | `NoteTreeBuilder`, `TaskTreeBuilder` vs `TreeBuilderCore` | Wire Note/Task to use `TreeBuilderCore::persist_nodes` like Finance/Learning/Productivity | Medium |
| **Correction-prefix logic cloned** | `agent_loop/mod.rs` (`process_message` vs `process_direct_streaming`) | Extract `detect_correction_prefix()`, `emit_correction_signal()` helpers | Small |
| **Autotuner rollback duplicated** | `autotuner/mod.rs` (`handle_regression` vs cron callback) | Extract `perform_rollback(trial, champion) -> RollbackResult` | Small |
| **AgentLoopBuilder monolith** | `agent_loop/builder.rs` (2,127 lines) | Group tool registration, context sources, background services into private `build_*` methods | Medium |

**Reference patterns from `app-core`:**
- Extract shared helpers to dedicated modules (like `errors.rs` in app-core).
- Use `tokio::try_join!` for parallel independent awaits.
- For builder wiring, break sequential blocks into named private methods.

---

### 2.2 `cognitive` — Monolithic Services & Duplicated Helpers

| Issue | Location | Suggested Fix | Effort |
|-------|----------|---------------|--------|
| **`freshness_label` duplicated** | `services/memory_retriever.rs:31` vs `services/context_source.rs:22` | Move to `types.rs` or `utils.rs` | Small |
| **RRF merge duplicated** | `services/retrieval.rs:25` vs `services/memory_retriever.rs:770` | Extract parameterized `rrf_merge(a, b, k)` to `utils.rs` | Small |
| **`run_reforge` extreme params** | `services/reforge/service.rs` (~25 args) | Introduce `ReforgeContext` struct; group phase bridges | Medium |
| **Background loop god-block** | `services/background.rs` (1,753 lines, ~600-line loop body) | Extract one async fn per stage: `collect_batch`, `run_extraction`, `run_consolidation`, `run_dlq_retry`, `emit_metrics` | Medium |
| **IngestionConsumer spawn block** | `consumers/ingestion.rs:284` (~350-line closure) | Extract `process_signal_inner()` or move logic into `services/ingestion_pipeline.rs` | Medium |
| **Mirror row conversion boilerplate** | `mirror/repo.rs` (~230 lines of `TryFrom`) | Consider a derive macro or `sqlx::FromRow` where possible | Medium |
| **`SemanticFactRepo` query sprawl** | `repos/semantic_fact.rs` (1,738 lines, ~35 methods) | Group related methods into traits or split by concern (active vs archive vs scope) | Large |

**Reference patterns from `app-core`:**
- Extract large closures into named async functions (like `relay_insight_stream` extraction).
- Use `tokio::try_join!` for parallel DB ops.
- For repo sprawl, follow `storage/src/macros.rs` patterns (`crud_repo!`, `get_by_ids_impl!`).

---

### 2.3 `coding-memory` ↔ `cognitive` Overlap

These two crates have **parallel subsystems** for the same concerns.

| Overlap Area | Cognitive Location | Coding-Memory Location | Suggested Fix | Effort |
|--------------|-------------------|------------------------|---------------|--------|
| **Mirror / Meta-rules** | `cognitive::mirror` (~2.5k lines) | `coding-memory::mirror` (~1k lines) | Fold coding mirror into `cognitive::mirror` as a `CodingMirrorSource` plugin | Large |
| **Consolidation / Reconciliation** | `cognitive::services::consolidation` (Mem0 AUDD) | `coding-memory::distiller::phase_c` (Add/Supersede/Noop) | Unify: wire coding distiller Phase C into `cognitive::consolidation` handler | Large |
| **Retrieval scoring** | `cognitive::services::retrieval` (FSRS + BM25 + RRF) | `coding-memory::recall` (C3 probe + skill registry) | Delegate scoring to `cognitive::retrieval`; keep coding-specific probing/escalation only | Medium |
| **Reforge phases** | `cognitive::services::reforge` (traits) | `coding-memory::reforge` (impls) | Boundary is correct (trait/impl), but `cognitive` traits are too coding-specific; evaluate inversion | Medium |
| **Skill evolution** | `cognitive::services::reforge::skill_discovery` | `coding-memory::skill_evolver` | Decide single owner; likely move to `coding-memory` and thin the cognitive trait | Medium |

**Recommended order:**
1. Start with `freshness_label` + RRF merge in `cognitive` (small, safe).
2. Then tackle `coding-memory::distiller::phase_c` → `cognitive::consolidation` unification.
3. Mirror dedup is the hardest — defer until consolidation is unified.

---

## Phase 3 — Feature Crate / App-Core Boundary Cleanup (🟡 P1)

`app-core` has large handler subtrees that overlap with feature crates. 
The rule of thumb: **feature crate = domain logic + tools; app-core = orchestration + HTTP/Tauri surface.**

| Feature Crate | App-Core Handlers | Issue | Suggested Fix | Effort |
|---------------|-------------------|-------|---------------|--------|
| `feature-productivity` (~18k) | `handlers/productivity/` (calendar, focus, summaries, tracking) | **Worst boundary blur.** Productivity crate has 17 repos + intelligence layer; app-core re-implements orchestration. | Migrate app-core productivity handlers into `feature-productivity::handlers` or `feature-productivity::api`; app-core calls thin facade | Large |
| `feature-notes` (~3.5k) | `handlers/notes/` (CRUD, insight, language, practice, grading, graph) | App-core has ~10 note handler modules; feature-notes only has repo + embedding handler + tool. | Move note orchestration (insight, language, practice, grading) into `feature-notes::handlers`; app-core keeps routing only | Large |
| `feature-finance` (~9.4k) | `handlers/finance/` (accounts, budgets, investments, reports, transactions) | Finance crate has tool + handler + price service; app-core has CRUD handlers. | Move CRUD handlers into `feature-finance::handlers`; app-core wires Tauri commands only | Medium |
| `feature-tasks` (~5.7k) | `handlers/tasks/` (CRUD, queries, converters) | Task crate has tool/actions + types + recurrence; app-core has handlers. | Move task CRUD handlers into `feature-tasks::handlers` | Medium |
| `feature-launcher` (~8.5k) | `handlers/launcher/` (dashboard, search engine, calendar fetcher) | Launcher has 17 search submodules + services; app-core has thin handlers. | App-core handlers are already thin — **low priority**; focus on launcher search consolidation instead | Low |

**Reference patterns from `app-core`:**
- When migrating handlers out of app-core, extract shared error mappers to the destination crate's `errors.rs`.
- Use `HandlerResult<T>` pattern (return `(T, Updates)`) for Tauri command compatibility.

---

## Phase 4 — Misc Medium Crates (🟢 P2)

| Crate | Lines | Files | Warnings | Notes | Effort |
|-------|-------|-------|----------|-------|--------|
| `context_engine` | ~7.6k | 37 | 0 | Context assembly engine. Review for builder bloat similar to `agent`. | Medium |
| `config` | ~8.0k | 42 | 0 | Mostly data definitions + deserialization. Low ROI unless config validation is duplicated. | Low |
| `tools` | ~5.9k | 27 | 0 | Tool definitions. Review if tool schemas overlap with `desktop-shared::commands`. | Low |
| `voice-engine` | ~5.9k | 29 | 0 | Voice pipeline. Review for blocking I/O (like app-core round 1). | Medium |
| `analytics` | ~4.4k | 27 | 0 | Metrics + telemetry. Review for stringly-typed metric names. | Low |
| `feature-coaching` | ~3.0k | 16 | 0 | Signal + intervention pipeline. Well-structured. | Low |
| `feature-insights` | ~2.5k | 13 | 0 | Insight repo + service. Clean. | Low |
| `mcp` | ~1.8k | 12 | 0 | MCP server scaffolding. | Low |
| `skill-system` | ~1.8k | 9 | 0 | Skill loading + registry. | Low |
| `notifications` | ~1.6k | 17 | 0 | Notification dispatch. | Low |
| `klynt-skill-loader` | ~1.4k | 13 | 0 | Skill loading helpers. | Low |
| `ai-core` | ~1.2k | 18 | 0 | AI signal types + consumers. | Low |
| `klyntbot-server` | ~1.1k | 7 | 0 | HTTP server. | Low |
| `kca-e2e` | ~1.1k | 10 | 0 | E2E tests. | Low |
| `kca-bench` | ~1.3k | 11 | 0 | Benchmarks. | Low |

---

## Phase 5 — Tiny / Stable Crates (⚪ Skip)

| Crate | Lines | Files | Why Skip |
|-------|-------|-------|----------|
| `common` | ~3.1k | 22 | Shared types + constants; changes ripple across workspace |
| `bus` | ~2.6k | 8 | Event bus; stable, minimal surface |
| `providers` | ~5.5k | 11 | LLM provider adapters; stable interface |
| `session` | ~1.0k | 2 | Minimal session key types |
| `scheduling` | ~4.0k | 13 | Cron + scheduling; focused, no duplication |
| `channels` | ~5.8k | 13 | Channel routing; stable |
| `platform-*` | ~0.3–2.5k | 3–15 | Platform abstraction; thin by design |
| `plugin-runtime` / `plugin-sdk` | ~1.8k | 7 | Plugin system; low churn |
| `mcp-bridge` | ~0.7k | 9 | MCP bridge; thin |
| `desktop-macros` | ~0.4k | 16 | Proc macros; don't touch unless broken |
| `ai-core-macros` | ~2.0k | 16 | Proc macros; don't touch unless broken |
| `tools-core` / `tools-core-macros` | ~3.0k | 19 | Tool-core scaffolding |
| `autotuner` | ~2.5k | 8 | Autotuner types; logic lives in `agent::autotuner` |
| `activity-log` | ~4.7k | 17 | Activity ingestion; focused |
| `klynt-git-utils` | ~0.6k | 4 | Git helpers; tiny |
| `klynt-truncation` | ~0.3k | 2 | Truncation logic; tiny |
| `klynt-sandbox` / `klynt-sandbox-helper` | ~1.0k | 5 | Sandbox helpers; stable |
| `klynt-process-hardening` | ~0.2k | 1 | Process hardening; tiny |
| `lsp-client` | ~0.4k | 5 | LSP client stub; tiny |
| `coding-agents-md` | ~0.2k | 1 | AGENTS.md walker; already clean |
| `feature-alarms` | ~0.4k | 3 | Alarm tool; tiny |
| `feature-focus` | ~1.2k | 13 | Focus DND; small, overlaps with productivity but stable |
| `feature-learning` | ~0.6k | 7 | Flashcard gen; tiny |
| `feature-language-learning` | ~0.5k | 7 | Pronunciation; tiny |

---

## Recommended Execution Order

### Sprint A (next 1–2 sessions)
1. **Clippy sweep** — `klynt-core`, `agent`, `cognitive`, `coding-ingest`, `klynt-hooks`, `klynt-execpolicy`, `desktop`, `desktop-shared`, `feature-launcher`, `coding-memory`
2. **Small extractions** — `cognitive::freshness_label`, RRF merge, `agent::stop_words`

### Sprint B (next 2–3 sessions)
3. **`agent::adapters/cognitive_handlers.rs` split** into submodules
4. **`agent::autotuner` rollback dedup** + correction-prefix dedup
5. **`cognitive::services/background.rs` loop extraction** into stage functions

### Sprint C (next 3–4 sessions)
6. **`feature-finance` handler migration** from app-core → feature-finance (medium, clean boundary)
7. **`feature-tasks` handler migration** from app-core → feature-tasks (medium)
8. **Evaluate `feature-notes` handler migration** (large, many modules)

### Sprint D (deferred)
9. **`coding-memory::distiller::phase_c` → `cognitive::consolidation` unification**
10. **`feature-productivity` handler migration** (largest refactor)
11. **`coding-memory::mirror` → `cognitive::mirror` plugin migration**

---

## Metrics Dashboard

| Crate | Lines | Files | Clippy Warnings | Priority | Complexity |
|-------|-------|-------|-----------------|----------|------------|
| `app-core` | ~41.8k | 197 | **0** ✅ | Done | — |
| `storage` | ~21.4k | 126 | **3** ✅ | Done | — |
| `cognitive` | ~38.2k | 123 | 6 | 🟡 P1 | High |
| `agent` | ~33.5k | 120 | 8 | 🟡 P1 | High |
| `coding-memory` | ~11.6k | 87 | 1 | 🟡 P1 | High |
| `feature-productivity` | ~18.2k | 72 | 0 | 🟡 P1 | High |
| `feature-finance` | ~9.4k | 31 | 0 | 🟡 P1 | Medium |
| `feature-launcher` | ~8.5k | 51 | 1 | 🔴 P0 | Medium |
| `config` | ~8.0k | 42 | 0 | 🟢 P2 | Medium |
| `context_engine` | ~7.6k | 37 | 0 | 🟢 P2 | Medium |
| `feature-tasks` | ~5.7k | 29 | 0 | 🟡 P1 | Medium |
| `klynt-core` | ~5.9k | 36 | **10** | 🔴 P0 | Medium |
| `tools` | ~5.9k | 27 | 0 | 🟢 P2 | Low |
| `voice-engine` | ~5.9k | 29 | 0 | 🟢 P2 | Medium |
| `desktop` | ~14k | 99 | 3 | 🔴 P0 | Medium |
| `desktop-shared` | ~4.7k | 47 | 1 | 🔴 P0 | Low |
| `feature-notes` | ~3.5k | 16 | 0 | 🟡 P1 | Medium |
| `feature-coaching` | ~3.0k | 16 | 0 | 🟢 P2 | Low |
| `coding-ingest` | ~2.6k | 38 | 3 | 🔴 P0 | Medium |
| `feature-insights` | ~2.5k | 13 | 0 | 🟢 P2 | Low |
| `klynt-hooks` | ~0.9k | 25 | 3 | 🔴 P0 | Low |
| `klynt-execpolicy` | ~1.9k | 11 | 3 | 🔴 P0 | Low |
| *all others* | < 2k | — | 0 | ⚪ Skip | Low |

---

## Reference Checklist (from storage + app-core)

When simplifying a new crate, verify these patterns:

- [ ] **Blocking I/O** — Replace `std::fs` with `tokio::fs` in async paths.
- [ ] **Concurrency** — Use `tokio::join!` / `tokio::try_join!` for independent async ops.
- [ ] **Batch queries** — Replace N+1 loops with `IN (...)` batch queries (use `get_by_ids_impl!` macro from storage).
- [ ] **Error mappers** — Extract shared `map_*_err()` closures to a central `errors.rs`.
- [ ] **LLM boilerplate** — Replace `provider.as_ref().ok_or_else(...) + config.read() + params.build()` with `cognitive_chat_context(max_tokens).await?` (app-core pattern; adapt for non-AppCore crates).
- [ ] **Time helpers** — Use `today_range_utc()` instead of manual `jiff` arithmetic.
- [ ] **Stringly-typed** — Flag raw strings for `status` / `role` / `origin` / `action` / `grade` → require `sqlx::Type` enum + migration (invasive; own ticket).
- [ ] **God files** — Split files > 800 lines into modules by concern.
- [ ] **Clippy** — Run `cargo clippy --fix` before manual work.
