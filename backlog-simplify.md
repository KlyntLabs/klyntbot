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
| `coding-memory` | ~11.6k | 87 | 1 | 🟡 P1 | High |
| `feature-productivity` | ~18.2k | 72 | 0 | 🟡 P1 | High |
| `feature-finance` | ~9.4k | 31 | 0 | 🟡 P1 | Medium |
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
