# Reforge Pipeline Reliability & Testability — Design

**Date:** 2026-05-27
**Candidate:** Architecture-review candidate 4 — "make the nightly reforge path reliable + testable"
**Crate:** `crates/cognitive-reforge` (+ one call-site migration in `crates/app-core`)
**Type:** Behavior-preserving — adds a production builder + test coverage; does **not** change reforge logic.

## Problem

The reforge nightly cycle (`run_reforge`) mutates the database (facts, rules, skills,
trials, communities, reforge_state) on a cron schedule. Two gaps make it unreliable:

1. **Construction is error-prone.** `ReforgeContext` has 22 fields — 8 required deps and 14
   `Option<&dyn …>` extension hooks. It is built as a single inline struct literal at the one
   production call site (`crates/app-core/src/init/cron.rs:440`). Whether a phase runs is encoded
   only as `Some(&x)` vs `None` inside a 22-line literal — easy to misread, nothing enforces that
   the required deps are present.
2. **Zero real coverage.** `pipeline.rs` (the 11 `Phase` impls, the `run_reforge` driver, the
   `None`-guard skips, and the DB-mutating `record_run` path) has **no** `#[cfg(test)]` module.
   The only "integration" test, `crates/cognitive/tests/phase_d_moat.rs`, is a phantom: it scripts
   two fakes, calls them directly, and asserts `2 == 2` / `1 == 1` — it never invokes `run_reforge`
   or any `Phase`.

> Note: the structural refactor the review proposed (collapse 24 params → `ReforgeContext`; express
> phases as an ordered `Vec<Box<dyn Phase>>`) **is already done and merged**. `ReforgeContext`,
> the `Phase` trait, `reforge_phases()`, and `run_reforge(ctx, run)` already exist in `pipeline.rs`.
> This work completes the **remaining half**: ergonomic, enforced construction + real coverage.

## Goals

- Make required deps a compile-time guarantee and optional phases self-documenting at the call site.
- Cover every phase's behavior **and** its `None`-guard skip (locality).
- Cover the full `run_reforge` path end-to-end, including DB mutations and the "no new data → `None`"
  short-circuit (the nightly path, finally verified).
- Retire the phantom test.

## Non-goals

- No change to reforge phase logic, ordering, or the `ReforgeContext`/`ReforgeRun`/`Phase` shapes
  (the struct literal must keep compiling; the builder is purely additive).
- No new observability/metrics.
- No real-LLM tests — the `ReforgeHandler` seam is faked.

## Part A — Production change: `ReforgeContext` builder

A new `ReforgeContextBuilder<'a>` in `crates/cognitive-reforge/src/pipeline.rs` (same crate, same
lifetimes as `ReforgeContext<'a>`).

- **Constructor** takes the 8 required deps:
  `ReforgeContext::builder(reforge_state_repo, skill_version_repo, session_memory_repo, fact_repo,
  episodic_repo, rule_repo, handler, skill_mgr) -> ReforgeContextBuilder<'a>`.
- **One chained setter per optional hook (14):** e.g. `.mirror_repo(&r)`, `.feedback_repo(&r)`,
  `.density_repo(&r)`, `.entity_repo(&r)`, `.snapshot_repo(&r)`, `.community_repo(&r)`,
  `.co_activation_repo_for_split(&r)`, `.domain_event_bus(arc)`, `.cross_cli_runner(&r)`,
  `.skill_discovery_runner(&r)`.
- **Pass-through for already-`Option` hooks at the call site** (`autotuner_bridge`,
  `graph_enrichment_handler`, `community_intelligence_handler`): the setter accepts the `Option`
  directly (e.g. `.autotuner_bridge(opt)`), so `cron.rs` does not need to unwrap/re-wrap.
- **`.build() -> ReforgeContext<'a>`** — all unset hooks default to `None`.
- **Migrate the single prod call site** (`cron.rs:440`) from the struct literal to the builder.
  Behavior is identical (same field values); only the construction syntax changes.

`ReforgeContext` keeps its public fields and `run_reforge`/`reforge_phases` are unchanged.

## Part B — Test architecture (inside `crates/cognitive-reforge`)

All new tests live in-crate because the phase structs (`CollectPhase`, …) are private. They go in a
dedicated `src/pipeline_tests.rs` file wired into `pipeline.rs` via `#[cfg(test)] mod pipeline_tests;`
— a separate file keeps the already-892-line `pipeline.rs` from ballooning while retaining in-crate
access to the private phase structs. This module holds the harness, fakes, and both test layers.

### Test harness

`ReforgeTestHarness` **owns** the dependencies and lends a borrowed context — this resolves the
lifetime problem (a test cannot return a `ReforgeContext` that borrows function-local repos).

- Owns: an in-memory `StoragePool` (via `cognitive_schema::cognitive_test_pool`, already a dev-dep),
  the 8 required repos constructed from that pool, and a `FakeReforgeHandler`.
- `fn ctx(&self) -> ReforgeContext<'_>` — builds a context via the Part-A builder with all 14 hooks
  defaulting to `None`.
- Builder-style toggles to install fake optional hooks for the phases under test
  (e.g. `with_cross_cli(&self, runner)`), returning a context that borrows `self`.

### Fakes

- `FakeReforgeHandler` — implements `ReforgeHandler` (`synthesize`/`review`/`narrate`) returning
  scripted `SynthesizeOutput`/`ReviewOutput`/narrative strings; supports configuring outputs and
  an error mode (to exercise per-phase error isolation).
- Minimal fakes for the optional trait hooks we assert on: `CrossCliPhaseRunner`,
  `SkillDiscoveryRunner`, `AutotunerBridge`, `GraphEnrichmentHandler`,
  `CommunityIntelligenceHandler` — each scripted to return a fixed count/result.

### Layer 1 — per-phase unit tests

For each of the 11 phases, drive `phase.run(&ctx, &mut run)` in isolation:

- **Happy path:** with required deps (and any relevant fake hook) present, assert the expected
  `ReforgeRun` mutation (e.g. `SynthesizePhase` populates `run.synthesize_output` from the fake
  handler; `ApplyPhase` writes facts/rules and bumps `run.result` counters).
- **`None`-guard skip:** for phases gated on an optional hook (CrossCli, SkillDiscovery, Optimize,
  GraphConsolidation, CommunityIntelligence), assert that with the hook absent the phase is a no-op
  — no panic, no `ReforgeRun` mutation, no DB write.
- **Error isolation (representative):** a phase whose dependency errors records into
  `run.result.phase_errors` and does not panic.

### Layer 2 — full-pipeline tests

Drive `run_reforge(ctx, run)` end-to-end through the harness:

- **Full run:** seed sessions/episodics so Collect yields data; assert the returned `ReforgeResult`
  counters and that DB state changed (e.g. `reforge_state.record_run` persisted a stats row;
  proposals/trials written where applicable).
- **No-new-data short-circuit:** with Collect yielding nothing, assert `run_reforge` returns `None`
  and records no run.
- **Optional phases off:** a run with all 14 hooks `None` completes and only the
  always-on phases take effect (proves graceful degradation through the real driver).

### Retire the phantom

Delete `crates/cognitive/tests/phase_d_moat.rs`. Its `CrossCli`/`SkillDiscovery` assertions are
subsumed by real `CrossCliPhase`/`SkillDiscoveryPhase` coverage in Layer 1. Remove now-unused
facade dev-deps only if this deletion orphans them (verify with `cargo machete`).

## Verification

- `cargo nextest run -p cognitive-reforge` — new harness + both layers green.
- `cargo build -p app-core` — builder migration of `cron.rs` compiles.
- `cargo build --workspace` + `cargo clippy -p cognitive-reforge -p app-core` — clean.
- `cargo machete` — no new unused deps (and confirm phantom removal didn't orphan facade dev-deps).

## Risks

- **Builder lifetime ergonomics:** `ReforgeContext<'a>` borrows everything; the builder must thread
  `'a` through without forcing `'static`. Mitigated by keeping the builder in the same module/crate
  with identical lifetime params.
- **Collect seeding:** Layer 2 needs realistic enough session/episodic seed data for `CollectPhase`
  to produce a non-empty `ReforgeCollected`. Mitigated by reading `CollectPhase`'s queries first and
  seeding the minimum rows; if seeding proves heavy, Layer 2 leans on a harness helper
  `seed_collectable()`.
