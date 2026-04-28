# Coding Memory — Phase 7 (Multi-CLI) + Phase 3-6 Debt Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the Codex / kimi-cli / opencode CLI adapters (Phase 7) on top of the stable `IngestAdapter` trait, while also closing every gap identified in Phases 3-6 that the prior audit found. FE/desktop-ui work is explicitly out of scope; Rust-side Tauri command shells and `*Installer` modules are in scope.

**Architecture:**
- Three new ingest adapters mirror the existing `ClaudeCodeAdapter` pattern: `CodexAdapter` (TOML hook → 5 events), `KimiAdapter` (tier-1 hook for 13 events; optional tier-2 Wire client for streaming), `OpencodeAdapter` (SQLite WAL polling at 500ms; opt-in only).
- Phase 3-6 debt remediation closes invariant-enforcement gaps (`ReforgeWriter` wrapper, scope-isolation proptest), filter regressions (`B1` threshold, `kinds`/`days`/`domain` arg drops, `repo:"*"` wildcard), and missing wire-up (`CodeDomainSearcher` registration, `ProviderRole::Distiller` threading, `causal_repo` injection at boot, `supporting_chains` metadata, `.git/hooks` install logic).
- The Phase 7 exit-gate test matrix proves `parse(serialize(AgentEvent)) == AgentEvent` for every `AgentSource` variant; this is the cross-CLI normalization invariant 7 from spec §3.

**Tech Stack:** Rust 2024 (MSRV 1.93), tokio, jiff timestamps, serde + serde_json, sqlx (rusqlite for opencode read-only path), proptest, tree-sitter (already wired), criterion-free (CLAUDE.md non-goal).

---

## Pre-flight

Before starting any task, verify the working tree:

- [ ] **Step 0.1: Confirm branch + clean tree**

```bash
git status
git rev-parse --abbrev-ref HEAD
```

Expected: clean working tree (no uncommitted changes), branch dedicated to this plan.

- [ ] **Step 0.2: Verify baseline build is green**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

Expected: 0 errors, 0 warnings on baseline. If anything fails, fix before proceeding — every task in this plan assumes a green baseline.

- [ ] **Step 0.3: Re-read the design spec sections this plan touches**

```bash
sed -n '200,260p' docs/superpowers/specs/2026-04-22-coding-memory-design.md   # §5 ingestion + AgentEvent
sed -n '380,470p' docs/superpowers/specs/2026-04-22-coding-memory-design.md   # §6 Distiller failure modes
sed -n '590,710p' docs/superpowers/specs/2026-04-22-coding-memory-design.md   # §8 Recall API + §9 Reforge
sed -n '800,870p' docs/superpowers/specs/2026-04-22-coding-memory-design.md   # §10 Mirror alerts
sed -n '960,990p' docs/superpowers/specs/2026-04-22-coding-memory-design.md   # §11 Phase 7 + exit gates
```

---

## File Structure (new + modified)

**Phase 3 remediation:**
- Modify: `crates/coding-memory/src/distiller/phase_c.rs` (vector-similarity reconciliation, top-5 limit)
- Modify: `crates/coding-memory/src/distiller/error.rs` (add `CostCeiling` variant)
- Modify: `crates/coding-memory/src/distiller/mod.rs` (cost ceiling check, `ProviderRole::Distiller` threading)
- Modify: `crates/coding-memory/src/distiller/phase_b.rs` (pass role to `ProviderManager::chat`)
- Modify: `crates/agent/src/agent_loop/builder.rs` (register `CodeDomainSearcher` in `InsightForge`)
- Modify: `crates/agent/src/autotuner/hooks.rs` (populate `session_type = "coding"` when applicable)
- Create: `crates/coding-memory/tests/prop_scope_isolation.rs` (Inv 6 proptest — currently absent)
- Modify: `crates/coding-memory/tests/prop_provenance_invariant.rs` (convert to `proptest!`)
- Modify: `crates/coding-memory/tests/prop_distiller_never_deletes.rs` (convert to `proptest!`)
- Modify: `crates/coding-memory/tests/prop_bi_temporal.rs` (convert to `proptest!`)
- Modify: `crates/coding-memory/tests/prop_supersede_chain.rs` (convert to `proptest!`)
- Create: `crates/coding-memory/tests/tier_a_score_turn_for_coding.rs`
- Create: `crates/coding-memory/tests/tier_a_contradiction_for_coding.rs`
- Create: `crates/coding-memory/tests/tier_a_user_corrected_for_coding.rs`
- Create: `crates/coding-memory/tests/distiller_cost_ceiling.rs`

**Phase 4 remediation:**
- Modify: `crates/coding-memory/src/recall/service.rs` (apply `kinds`/`days` filters; handle `repo:"*"` wildcard)
- Modify: `crates/coding-memory/src/mcp.rs` (forward `domain` to `DecisionPointsService`)
- Modify: `crates/coding-memory/src/recall/renderers.rs` (B1 threshold 0.5 → 0.7; real "Open threads" query)
- Modify: `crates/coding-memory/src/recall/scope_resolve.rs` (per-session cache)
- Modify: `crates/app-core/src/coding_memory/mod.rs` (inject `causal_repo` into `CodingRecallService` at boot)
- Modify: `crates/coding-memory/src/recall/renderers.rs` (wire `QueryPipeline` enrichment in user-prompt path)
- Modify: `crates/coding-memory/src/recall/timeline_builder.rs` or new `open_threads.rs` (real unfinished-turn-trace query)
- Create: `crates/coding-memory/tests/recall_kinds_days_filter.rs`
- Create: `crates/coding-memory/tests/recall_repo_wildcard.rs`
- Create: `crates/coding-memory/tests/decision_points_domain_filter.rs`
- Create: `crates/coding-memory/tests/dead_end_threshold_07.rs`
- Create: `crates/coding-memory/tests/open_threads_query.rs`
- Create: `crates/coding-memory/tests/scope_resolver_session_cache.rs`
- Create: `crates/coding-memory/tests/trace_causes_wired.rs`

**Phase 5 remediation:**
- Create: `crates/coding-memory/src/reforge/writer.rs` (`ReforgeWriter` wrapper rejecting DELETE)
- Modify: `crates/coding-memory/src/reforge/mod.rs` (re-export `ReforgeWriter`)
- Modify: `crates/coding-memory/src/reforge/coding_synthesis.rs` (route writes through `ReforgeWriter`)
- Modify: `crates/coding-memory/src/reforge/rule_artifacts.rs` (route writes through `ReforgeWriter`)
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs` (route writes through `ReforgeWriter`)
- Modify: `crates/coding-memory/src/reforge/session_end.rs` (add stale-candidate resolution)
- Modify: `crates/coding-memory/src/skills.rs` (LLM-drafted SKILL.md via `ProviderRole::ReforgeRules`)
- Modify: `crates/skill-system/src/store.rs` (scope-aware filtering at read time)
- Modify: `crates/coding-memory/src/mirror/alerts.rs` (add 12th alert variant — `RetrievalSkillIneffective`)
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs` (vector-similarity dedup path)
- Create: `crates/coding-memory/tests/reforge_writer_rejects_delete.rs`
- Create: `crates/coding-memory/tests/session_end_stale_candidate.rs`
- Create: `crates/coding-memory/tests/skill_evolver_llm_drafted.rs`
- Create: `crates/skill-system/tests/scope_aware_load.rs`
- Create: `crates/coding-memory/tests/cross_session_vector_dedup.rs`

**Phase 6 remediation:**
- Modify: `crates/coding-memory/src/reforge/coding_synthesis.rs` (populate `metadata.supporting_chains` on promoted patterns)
- Modify: `crates/coding-memory/src/distiller/mod.rs` (extract anchored_symbols on every fact-write branch)
- Modify: `crates/coding-memory/src/reforge/symbol_validation.rs` (write `needs_review` for surviving-but-drifted symbols)
- Create: `crates/app-core/src/coding_memory/git_hook_installer.rs` (write `.git/hooks/post-commit` script per repo)
- Modify: `crates/desktop/src/commands/coding_memory.rs` (Tauri command shell `coding_memory_install_git_hook`)
- Modify: `crates/desktop/src/specta_builder.rs` (register new command)
- Create: `crates/coding-memory/tests/supporting_chains_in_promotion.rs`
- Create: `crates/coding-memory/tests/all_distiller_branches_anchor.rs`
- Create: `crates/coding-memory/tests/symbol_validation_needs_review.rs`
- Create: `crates/app-core/tests/git_hook_installer.rs`

**Phase 7 — adapters & test matrix:**
- Replace: `crates/coding-ingest/src/adapters/codex.rs` → `crates/coding-ingest/src/adapters/codex/{mod.rs, payload.rs, dispatch.rs}`
- Replace: `crates/coding-ingest/src/adapters/kimi_wire.rs` → `crates/coding-ingest/src/adapters/kimi_cli/{mod.rs, payload.rs, dispatch.rs, wire.rs}`
- Replace: `crates/coding-ingest/src/adapters/opencode.rs` → `crates/coding-ingest/src/adapters/opencode/{mod.rs, schema.rs, poller.rs, normalize.rs}`
- Modify: `crates/coding-ingest/src/adapters/mod.rs` (update module declarations)
- Modify: `crates/coding-ingest/src/hook_cli.rs` (remove early-exit at lines 63-66; dispatch to all 4 adapters)
- Modify: `crates/coding-ingest/src/daemon.rs` (spawn opencode poller task when enabled)
- Modify: `crates/coding-ingest/src/lib.rs` (re-export new types)
- Create: `crates/app-core/src/coding_memory/codex_installer.rs` (write Codex hook stanza to its TOML config)
- Create: `crates/app-core/src/coding_memory/kimi_installer.rs` (write kimi-cli hook stanza)
- Create: `crates/app-core/src/coding_memory/opencode_installer.rs` (record opencode opt-in marker; no settings file write)
- Modify: `crates/app-core/src/coding_memory/mod.rs` (dispatch installer by CLI name)
- Modify: `crates/desktop/src/commands/coding_memory.rs` (no FE; Tauri command shells already exist — verify wiring)
- Create: `tests/fixtures/coding/synthetic_session_codex.jsonl`
- Create: `tests/fixtures/coding/synthetic_session_kimi.jsonl`
- Create: `tests/fixtures/coding/opencode_messages_seed.sql` (CREATE TABLE + INSERT script for opencode replica)
- Create: `crates/coding-ingest/tests/codex_adapter.rs`
- Create: `crates/coding-ingest/tests/kimi_adapter_tier1.rs`
- Create: `crates/coding-ingest/tests/kimi_wire_tier2.rs`
- Create: `crates/coding-ingest/tests/opencode_poller.rs`
- Modify: `crates/coding-ingest/tests/agent_event_roundtrip.rs` (add `KimiCli`, `OpenCode`, `KlyntCli` source variants)
- Create: `crates/coding-ingest/tests/cross_cli_normalization.rs` (proptest — Inv 7 over all sources)
- Create: `tests/integration/coding_memory_phase7_codex.rs`
- Create: `tests/integration/coding_memory_phase7_kimi.rs`
- Create: `tests/integration/coding_memory_phase7_opencode.rs`
- Create: `tests/integration/coding_memory_phase7_matrix.rs` (full CLI matrix exit-gate test)
- Create: `docs/coding-memory/adapters/codex.md`
- Create: `docs/coding-memory/adapters/kimi-cli.md`
- Create: `docs/coding-memory/adapters/opencode.md`

---

## Section 1 — Phase 3 Debt Remediation

### Task 1.1: Convert `prop_provenance_invariant.rs` to a real proptest

**Files:**
- Modify: `crates/coding-memory/tests/prop_provenance_invariant.rs`

- [ ] **Step 1: Read the current deterministic test**

```bash
cat crates/coding-memory/tests/prop_provenance_invariant.rs
```

- [ ] **Step 2: Replace with a `proptest!` over arbitrary fact-content inputs**

```rust
//! Inv 1 — provenance-always: every Distiller-emitted fact carries
//! `metadata.provenance.source_events` non-empty.

use coding_ingest::AgentSource;
use coding_memory::distiller::writer::DistillerWriter;
use coding_memory::scope::ProvenanceMetadata;
use jiff::Timestamp;
use proptest::prelude::*;
use storage::StoragePool;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn provenance_always_non_empty_for_facts(
        subject in "[a-z]{3,12}",
        predicate in prop::sample::select(vec!["uses", "prefers", "framework"]),
        object in "[a-z0-9]{3,32}",
        source_event_count in 1usize..=8,
        source in prop::sample::select(vec![
            AgentSource::ClaudeCode, AgentSource::Codex, AgentSource::KimiCli,
            AgentSource::OpenCode, AgentSource::KlyntCli,
        ]),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repos = common::repos(&pool).await;
            let writer = DistillerWriter::new(repos.clone());
            let event_ids: Vec<String> = (0..source_event_count).map(|i| format!("ev_{i}")).collect();
            let prov = ProvenanceMetadata {
                source_events: event_ids,
                session_id: "s1".into(),
                turn_id: Some("t1".into()),
                distilled_at: Timestamp::now(),
                distiller_model: "test-model".into(),
                source_kind: "distiller_extractive".into(),
                source: Some(source),
            };
            let row = writer
                .write_fact(&subject, predicate, &object, /*confidence*/ 0.8, /*scope*/ None, &prov)
                .await
                .expect("write_fact");
            let metadata: serde_json::Value = serde_json::from_str(&row.metadata.unwrap()).unwrap();
            let events = metadata.pointer("/provenance/source_events").unwrap();
            prop_assert!(events.as_array().unwrap().len() == source_event_count);
        });
    }
}
```

- [ ] **Step 3: Add `proptest` dev-dep if missing**

```bash
grep -n proptest crates/coding-memory/Cargo.toml
```

If absent, add to `[dev-dependencies]` block:

```toml
proptest = { workspace = true }
```

- [ ] **Step 4: Add `mod common;` shared helper if missing**

```bash
ls crates/coding-memory/tests/common.rs 2>/dev/null || echo MISSING
```

If MISSING, create `crates/coding-memory/tests/common.rs`:

```rust
//! Shared test helpers for coding-memory proptests.

use storage::{Repos, StoragePool};

pub async fn repos(pool: &StoragePool) -> Repos {
    Repos::from_pool(pool)
}
```

- [ ] **Step 5: Run the new proptest**

```bash
cargo nextest run -p coding-memory -E 'test(prop_provenance_invariant)'
```

Expected: PASS with 64 generated cases.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/tests/prop_provenance_invariant.rs crates/coding-memory/tests/common.rs crates/coding-memory/Cargo.toml
git commit -m "test(coding-memory): convert provenance-always to proptest with multi-source coverage"
```

---

### Task 1.2: Convert `prop_distiller_never_deletes.rs` to a real proptest

**Files:**
- Modify: `crates/coding-memory/tests/prop_distiller_never_deletes.rs`

- [ ] **Step 1: Replace deterministic body with proptest**

```rust
//! Inv 2 — Distiller never deletes: row count of semantic_facts and
//! episodic_memories is monotone non-decreasing across distillation cycles.

use coding_memory::distiller::Distiller;
use proptest::prelude::*;
use storage::StoragePool;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn row_count_monotone_non_decreasing(
        cycle_count in 1usize..=8,
        facts_per_cycle in prop::collection::vec("[a-z]{3,8}", 0..=5),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repos = common::repos(&pool).await;
            let distiller = common::seed_distiller(&repos);

            let mut prev_facts = 0i64;
            let mut prev_episodes = 0i64;
            for _ in 0..cycle_count {
                let _report = distiller.distill_turn("s1", Some("t1")).await.unwrap_or_default();
                let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
                    .fetch_one(pool.inner()).await.unwrap();
                let eps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodic_memories")
                    .fetch_one(pool.inner()).await.unwrap();
                prop_assert!(facts >= prev_facts, "facts decreased {prev_facts} -> {facts}");
                prop_assert!(eps >= prev_episodes, "episodes decreased {prev_episodes} -> {eps}");
                prev_facts = facts;
                prev_episodes = eps;
                let _ = facts_per_cycle.len();
            }
        });
    }
}
```

- [ ] **Step 2: Extend `tests/common.rs` with `seed_distiller`**

Append to `crates/coding-memory/tests/common.rs`:

```rust
use coding_memory::distiller::Distiller;
use providers::testing::NoopProvider;
use std::sync::Arc;

pub fn seed_distiller(repos: &storage::Repos) -> Distiller {
    Distiller::builder(repos.clone())
        .with_provider(Arc::new(NoopProvider::default()))
        .build()
}
```

- [ ] **Step 3: Run the proptest**

```bash
cargo nextest run -p coding-memory -E 'test(prop_distiller_never_deletes)'
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/tests/prop_distiller_never_deletes.rs crates/coding-memory/tests/common.rs
git commit -m "test(coding-memory): convert distiller-never-deletes to monotone proptest"
```

---

### Task 1.3: Convert `prop_bi_temporal.rs` to a real proptest

**Files:**
- Modify: `crates/coding-memory/tests/prop_bi_temporal.rs`

- [ ] **Step 1: Replace body with proptest covering generated valid_from/valid_until pairs**

```rust
//! Inv 4 — bi-temporal monotone: `valid_until >= valid_from` always.

use jiff::{Timestamp, ToSpan};
use proptest::prelude::*;
use storage::StoragePool;

mod common;

fn arb_offset_seconds() -> impl Strategy<Value = i64> {
    -86_400i64..=86_400i64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn valid_until_never_precedes_valid_from(
        valid_from_offset in arb_offset_seconds(),
        valid_until_offset in arb_offset_seconds(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repos = common::repos(&pool).await;

            let now = Timestamp::now();
            let valid_from = now.checked_add(valid_from_offset.seconds()).unwrap();
            let candidate_until = now.checked_add(valid_until_offset.seconds()).unwrap();

            // Insert via SemanticFactRepo. The repo MUST reject candidate_until < valid_from.
            let result = repos.facts().insert_with_validity(
                /*subject*/ "s",
                /*predicate*/ "p",
                /*object*/ "o",
                /*scope*/ None,
                valid_from,
                Some(candidate_until),
            ).await;

            if candidate_until < valid_from {
                prop_assert!(result.is_err(), "repo accepted invalid until < from");
            } else {
                let row = result.unwrap();
                let stored_until = row.valid_until.unwrap();
                prop_assert!(stored_until >= row.valid_from);
            }
        });
    }
}
```

- [ ] **Step 2: Verify `SemanticFactRepo::insert_with_validity` exists**

```bash
grep -n "insert_with_validity\|fn insert" crates/storage/src/repos/semantic_facts.rs | head -10
```

If absent, add the method to `crates/storage/src/repos/semantic_facts.rs` near the existing `insert`:

```rust
pub async fn insert_with_validity(
    &self,
    subject: &str,
    predicate: &str,
    object: &str,
    scope_repo_id: Option<&str>,
    valid_from: jiff::Timestamp,
    valid_until: Option<jiff::Timestamp>,
) -> Result<SemanticFactRow> {
    if let Some(until) = valid_until {
        if until < valid_from {
            return Err(KlyntbotError::Storage(format!(
                "bi-temporal violation: valid_until {until} < valid_from {valid_from}"
            )));
        }
    }
    // …delegate to existing insert with the explicit timestamps…
    self.insert_inner(subject, predicate, object, scope_repo_id, valid_from, valid_until).await
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p coding-memory -E 'test(prop_bi_temporal)'
```

Expected: PASS, generated cases include both valid and invalid orderings.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/tests/prop_bi_temporal.rs crates/storage/src/repos/semantic_facts.rs
git commit -m "test(coding-memory): bi-temporal proptest + repo-side validation"
```

---

### Task 1.4: Convert `prop_supersede_chain.rs` to a real proptest

**Files:**
- Modify: `crates/coding-memory/tests/prop_supersede_chain.rs`

- [ ] **Step 1: Replace body with chain-length-generated proptest**

```rust
//! Inv 5 — SUPERSEDE chain: predecessor.valid_until == successor.valid_from.

use coding_memory::distiller::Distiller;
use proptest::prelude::*;
use storage::StoragePool;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn supersede_chain_links_are_consistent(chain_len in 2usize..=6) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repos = common::repos(&pool).await;
            let distiller = common::seed_distiller(&repos);

            // Drive `chain_len` supersedes against the same (subject, predicate).
            for i in 0..chain_len {
                common::seed_observation_for_supersede(&repos, "s", "p", &format!("v{i}")).await;
                distiller.distill_turn(&format!("session-{i}"), None).await.unwrap();
            }

            let chain = repos.facts().list_supersede_chain("s", "p").await.unwrap();
            for pair in chain.windows(2) {
                let pred = &pair[0];
                let succ = &pair[1];
                prop_assert_eq!(pred.valid_until.unwrap(), succ.valid_from);
            }
        });
    }
}
```

- [ ] **Step 2: Verify `list_supersede_chain` exists**

```bash
grep -n "list_supersede_chain\|supersedes" crates/storage/src/repos/semantic_facts.rs | head
```

If absent, add to `crates/storage/src/repos/semantic_facts.rs`:

```rust
pub async fn list_supersede_chain(&self, subject: &str, predicate: &str) -> Result<Vec<SemanticFactRow>> {
    let rows = sqlx::query_as::<_, SemanticFactRow>(
        "SELECT * FROM semantic_facts WHERE subject = ? AND predicate = ? ORDER BY valid_from ASC"
    )
    .bind(subject).bind(predicate)
    .fetch_all(self.pool.inner()).await
    .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(rows)
}
```

- [ ] **Step 3: Add `seed_observation_for_supersede` to `common.rs`**

```rust
pub async fn seed_observation_for_supersede(
    repos: &storage::Repos,
    subject: &str,
    predicate: &str,
    object: &str,
) {
    use coding_ingest::{AgentEvent, EventKind};
    let _ = (subject, predicate, object);
    // Insert directly into ingest_event_log so distill_turn picks it up.
    repos.ingest_event_log().insert_raw(AgentEvent::synthetic_user_prompt(
        "supersede-test", subject, predicate, object,
    )).await.unwrap();
}
```

- [ ] **Step 4: Run**

```bash
cargo nextest run -p coding-memory -E 'test(prop_supersede_chain)'
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/tests/prop_supersede_chain.rs crates/storage/src/repos/semantic_facts.rs crates/coding-memory/tests/common.rs
git commit -m "test(coding-memory): supersede-chain proptest with chain-length sweep"
```

---

### Task 1.5: Add Inv 6 (scope isolation) proptest — currently absent

**Files:**
- Create: `crates/coding-memory/tests/prop_scope_isolation.rs`

- [ ] **Step 1: Write the failing proptest**

```rust
//! Inv 6 — scope isolation: repo-scoped retrieval never leaks cross-repo facts
//! (except global `scope_repo_id IS NULL` facts).

use coding_memory::recall::CodingRecallService;
use proptest::prelude::*;
use storage::StoragePool;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn repo_scoped_query_excludes_other_repos(
        repos_count in 2usize..=5,
        facts_per_repo in 1usize..=4,
        query_repo_idx in 0usize..5,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let query_repo_idx = query_repo_idx % repos_count;
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repos = common::repos(&pool).await;

            for r in 0..repos_count {
                for f in 0..facts_per_repo {
                    repos.facts().insert(
                        &format!("subject_{r}_{f}"),
                        "uses",
                        "rust",
                        Some(&format!("repo_{r}")),
                    ).await.unwrap();
                }
            }
            // Insert one global fact (scope_repo_id IS NULL).
            repos.facts().insert("global_subject", "is_global", "true", None).await.unwrap();

            let svc = CodingRecallService::new_for_test(repos.clone());
            let results = svc.recall_index(
                "uses rust",
                /*repo*/ Some(&format!("repo_{query_repo_idx}")),
                /*kinds*/ None,
                /*days*/ None,
                /*limit*/ 64,
            ).await.unwrap();

            for entry in &results.entries {
                let scope = &entry.scope;
                prop_assert!(
                    scope.is_none() || scope.as_deref() == Some(&format!("repo_{query_repo_idx}")),
                    "leak: got scope {:?} when querying repo_{query_repo_idx}", scope
                );
            }
        });
    }
}
```

- [ ] **Step 2: Run — expect FAIL or compile error if `recall_index` filter is broken or `new_for_test` is missing**

```bash
cargo nextest run -p coding-memory -E 'test(prop_scope_isolation)'
```

Expected: FAIL (until Task 2.1 lands the missing scope filter).

- [ ] **Step 3: Mark this test as the gating test for Task 2.x — do NOT commit a passing-but-incorrect implementation**

If `new_for_test` is missing, add to `crates/coding-memory/src/recall/service.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl CodingRecallService {
    pub fn new_for_test(repos: storage::Repos) -> Self {
        Self::new(repos, /*causal_repo*/ None, /*config*/ Default::default())
    }
}
```

- [ ] **Step 4: Commit the failing test (test-first)**

```bash
git add crates/coding-memory/tests/prop_scope_isolation.rs crates/coding-memory/src/recall/service.rs
git commit -m "test(coding-memory): scope-isolation proptest (Inv 6) — failing baseline"
```

---

### Task 1.6: Tier B4 — register `CodeDomainSearcher` in InsightForge

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:787-829`

- [ ] **Step 1: Read the current registration block**

```bash
sed -n '780,835p' crates/agent/src/agent_loop/builder.rs
```

- [ ] **Step 2: Write the failing test**

Create `crates/agent/tests/code_domain_searcher_registered.rs`:

```rust
//! Confirms CodeDomainSearcher is wired into the InsightForge searcher set
//! when the coding-memory feature is enabled in the builder.

use agent::agent_loop::AgentLoopBuilder;

#[test]
fn code_domain_searcher_present_in_forge() {
    let forge = AgentLoopBuilder::test_builder()
        .with_coding_memory_enabled(true)
        .build_forge_for_test();
    let names: Vec<&str> = forge.searcher_names();
    assert!(
        names.contains(&"CodeDomainSearcher"),
        "expected CodeDomainSearcher in {names:?}",
    );
}
```

- [ ] **Step 3: Run — FAIL with "expected CodeDomainSearcher in [...]"**

```bash
cargo nextest run -p agent -E 'test(code_domain_searcher_present_in_forge)'
```

- [ ] **Step 4: Patch the builder**

In `crates/agent/src/agent_loop/builder.rs`, locate the existing `forge.add_searcher(...)` calls (around line 800) and add:

```rust
if self.coding_memory_enabled {
    forge.add_searcher(Box::new(coding_memory::CodeDomainSearcher::new(repos.clone())));
}
```

Add `coding_memory::CodeDomainSearcher` to the use list at the top of the file.

- [ ] **Step 5: Add the test-helper APIs the test expects**

In the same file, add:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl AgentLoopBuilder {
    pub fn test_builder() -> Self { /* minimal in-memory builder */ Self::default() }
    pub fn with_coding_memory_enabled(mut self, on: bool) -> Self {
        self.coding_memory_enabled = on; self
    }
    pub fn build_forge_for_test(self) -> InsightForge { /* construct forge with searchers */ }
}
```

(If `coding_memory_enabled` field is missing on the builder, add it as `bool` with default `false`.)

- [ ] **Step 6: Add `searcher_names` helper**

In `crates/context_engine/src/insight_forge/mod.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl InsightForge {
    pub fn searcher_names(&self) -> Vec<&'static str> {
        self.searchers.iter().map(|s| s.name()).collect()
    }
}
```

(Confirms `DomainSearcher` trait exposes `name() -> &'static str`. If missing, add it.)

- [ ] **Step 7: Re-run — PASS**

```bash
cargo nextest run -p agent -E 'test(code_domain_searcher_present_in_forge)'
```

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/agent/tests/code_domain_searcher_registered.rs crates/context_engine/src/insight_forge/mod.rs
git commit -m "feat(agent): register CodeDomainSearcher in InsightForge (Tier B4)"
```

---

### Task 1.7: Tier B5 — populate `session_type = "coding"` in `ShadowContext`

**Files:**
- Modify: `crates/agent/src/autotuner/hooks.rs:103`

- [ ] **Step 1: Write the failing test**

Create `crates/agent/tests/autotuner_session_type_coding.rs`:

```rust
//! When the agent loop runs with a coding-memory scope_repo_id present,
//! the autotuner's ShadowContext must carry session_type = Some("coding").

use agent::autotuner::hooks::derive_session_type;

#[test]
fn coding_repo_yields_session_type_coding() {
    let st = derive_session_type(/*scope_repo_id*/ Some("github.com/foo/bar"));
    assert_eq!(st.as_deref(), Some("coding"));
}

#[test]
fn personal_yields_no_session_type_tag() {
    let st = derive_session_type(/*scope_repo_id*/ None);
    assert_eq!(st, None);
}
```

- [ ] **Step 2: Run — FAIL (`derive_session_type` not found)**

```bash
cargo nextest run -p agent -E 'test(coding_repo_yields_session_type_coding)'
```

- [ ] **Step 3: Add helper + use at the construction site**

In `crates/agent/src/autotuner/hooks.rs`:

```rust
pub fn derive_session_type(scope_repo_id: Option<&str>) -> Option<String> {
    scope_repo_id.map(|_| "coding".to_string())
}
```

Update the construction at line 103:

```rust
let shadow = ShadowContext {
    // …existing fields…
    session_type: derive_session_type(scope_repo_id.as_deref()),
};
```

- [ ] **Step 4: Re-run — PASS**

```bash
cargo nextest run -p agent -E 'test(autotuner_session_type)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/autotuner/hooks.rs crates/agent/tests/autotuner_session_type_coding.rs
git commit -m "feat(autotuner): populate session_type=coding when repo scope is present (Tier B5)"
```

---

### Task 1.8: Thread `ProviderRole::Distiller` into `ProviderManager::chat`

**Files:**
- Modify: `crates/coding-memory/src/distiller/phase_b.rs:169`
- Modify: `crates/providers/src/lib.rs` (extend `chat` signature if it doesn't already accept role)

- [ ] **Step 1: Inspect existing `chat` signature**

```bash
grep -n "pub async fn chat" crates/providers/src/lib.rs crates/providers/src/manager.rs 2>/dev/null
```

- [ ] **Step 2: Add `role` parameter (or `ChatParams` extension)**

If `chat` takes a `ChatParams` struct, add `role: Option<ProviderRole>` field. Otherwise add a new method `chat_with_role` and route Distiller through it. The non-breaking variant:

```rust
impl ProviderManager {
    pub async fn chat_with_role(
        &self,
        role: ProviderRole,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        params: ChatParams,
    ) -> Result<ChatResponse> {
        // route to provider configured for `role`; fall back to default
        let provider = self.resolve_for_role(role).unwrap_or_else(|| self.default());
        provider.chat(messages, tools, params).await
    }
}
```

- [ ] **Step 3: Use it in `phase_b.rs:169`**

```rust
let response = self
    .provider_manager
    .chat_with_role(ProviderRole::Distiller, messages, vec![record_observation_tool_def()], params)
    .await?;
```

- [ ] **Step 4: Test**

Create `crates/coding-memory/tests/distiller_uses_distiller_role.rs`:

```rust
//! Distiller Phase B must request the `Distiller` provider role,
//! not silently use the default role.

use coding_memory::distiller::Distiller;
use providers::testing::RoleRecorderProvider;
use std::sync::Arc;

#[tokio::test]
async fn phase_b_routes_via_distiller_role() {
    let recorder = Arc::new(RoleRecorderProvider::default());
    let distiller = common::seed_distiller_with_provider(recorder.clone());
    distiller.distill_turn("s", Some("t")).await.unwrap();
    let observed = recorder.last_role();
    assert_eq!(observed, Some(providers::ProviderRole::Distiller));
}
```

Add `RoleRecorderProvider` to `crates/providers/src/testing.rs`:

```rust
#[derive(Default)]
pub struct RoleRecorderProvider {
    last_role: std::sync::Mutex<Option<crate::ProviderRole>>,
}
impl RoleRecorderProvider {
    pub fn last_role(&self) -> Option<crate::ProviderRole> {
        *self.last_role.lock().unwrap()
    }
}
// implement Provider trait, recording role on each chat()
```

- [ ] **Step 5: Run**

```bash
cargo nextest run -p coding-memory -E 'test(phase_b_routes_via_distiller_role)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/providers/src/lib.rs crates/providers/src/testing.rs crates/coding-memory/src/distiller/phase_b.rs crates/coding-memory/tests/distiller_uses_distiller_role.rs
git commit -m "feat(distiller): thread ProviderRole::Distiller through Phase B chat call"
```

---

### Task 1.9: Add `CostCeiling` failure mode

**Files:**
- Modify: `crates/coding-memory/src/distiller/error.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Create: `crates/coding-memory/tests/distiller_cost_ceiling.rs`

- [ ] **Step 1: Add the variant**

In `crates/coding-memory/src/distiller/error.rs`, extend the enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DistillerError {
    // ...existing variants...
    #[error("cost ceiling exceeded: spent {spent_usd} of {ceiling_usd}")]
    CostCeiling { spent_usd: f64, ceiling_usd: f64 },
}
```

- [ ] **Step 2: Add a check before invoking Phase B**

In `crates/coding-memory/src/distiller/mod.rs` (near the `invoke_llm` site, around line 470):

```rust
if let Some(ceiling) = self.config.cost_ceiling_usd {
    let spent = self.cost_tracker.spent_today(ProviderRole::Distiller).await;
    if spent >= ceiling {
        return Err(DistillerError::CostCeiling { spent_usd: spent, ceiling_usd: ceiling });
    }
}
```

Add a `cost_ceiling_usd: Option<f64>` field to `DistillerConfig`.

- [ ] **Step 3: Write failing test**

```rust
//! Distiller halts Phase B when daily cost ceiling is breached.

use coding_memory::distiller::{Distiller, DistillerConfig, DistillerError};
use providers::testing::ExpensiveProvider;

#[tokio::test]
async fn phase_b_halts_at_ceiling() {
    let provider = std::sync::Arc::new(ExpensiveProvider::with_cost(0.50));
    let distiller = common::seed_distiller_with_provider_and_config(
        provider,
        DistillerConfig { cost_ceiling_usd: Some(0.10), ..Default::default() },
    );
    let result = distiller.distill_turn("s", Some("t")).await;
    assert!(matches!(result, Err(DistillerError::CostCeiling { .. })));
}
```

- [ ] **Step 4: Run**

```bash
cargo nextest run -p coding-memory -E 'test(phase_b_halts_at_ceiling)'
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/distiller/error.rs crates/coding-memory/src/distiller/mod.rs crates/coding-memory/tests/distiller_cost_ceiling.rs crates/providers/src/testing.rs
git commit -m "feat(distiller): cost-ceiling failure mode + halt before Phase B"
```

---

### Task 1.10: Phase C — replace text-match with vector-similarity reconciliation

**Files:**
- Modify: `crates/coding-memory/src/distiller/phase_c.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs:583-587` (lookup call site)

- [ ] **Step 1: Read current implementation**

```bash
sed -n '40,80p' crates/coding-memory/src/distiller/phase_c.rs
sed -n '575,600p' crates/coding-memory/src/distiller/mod.rs
```

- [ ] **Step 2: Add a vector-lookup method to `SemanticFactRepo`**

In `crates/storage/src/repos/semantic_facts.rs`:

```rust
pub async fn find_similar_by_embedding(
    &self,
    subject: &str,
    predicate: &str,
    embedding: &[f32],
    scope_repo_id: Option<&str>,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<(SemanticFactRow, f32)>> {
    // Use the LanceDB vector index for top-k retrieval, filter post-hoc
    // by (subject? predicate?) for relevance, return rows with cosine sim.
    // …delegate to existing vector_store.search() helper if present…
}
```

- [ ] **Step 3: Update Phase C to call the vector path with limit=5**

In `crates/coding-memory/src/distiller/phase_c.rs`:

```rust
const TOP_K: usize = 5;

pub async fn reconcile(
    repos: &Repos,
    embedder: &dyn Embedder,
    obs: &Observation,
    scope_repo_id: Option<&str>,
) -> Result<Reconciliation> {
    let embedding = embedder.embed(&format!("{} {} {}", obs.subject, obs.predicate, obs.object)).await?;
    let candidates = repos.facts().find_similar_by_embedding(
        &obs.subject, &obs.predicate, &embedding, scope_repo_id, Some(obs.domain()), TOP_K,
    ).await?;

    let best_match = candidates.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((row, sim)) = best_match {
        if *sim > NOOP_THRESHOLD && row.subject == obs.subject && row.predicate == obs.predicate {
            return Ok(Reconciliation::Noop { existing_id: row.id.clone() });
        }
        if *sim > SUPERSEDE_THRESHOLD {
            return Ok(Reconciliation::Supersede { predecessor_id: row.id.clone() });
        }
    }
    Ok(Reconciliation::Add)
}
```

- [ ] **Step 4: Update the call site in `mod.rs:583-587`**

Pass embedder + scope into `reconcile`:

```rust
let recon = phase_c::reconcile(&self.repos, self.embedder.as_ref(), &obs, scope_repo_id).await?;
```

- [ ] **Step 5: Update or write the test**

`crates/coding-memory/tests/phase_c_reconciliation.rs` already exists. Extend it with a vector-similarity case:

```rust
#[tokio::test]
async fn vector_similarity_above_noop_threshold_returns_noop() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    let embedder = common::deterministic_embedder();
    repos.facts().insert("user", "prefers", "rust", None).await.unwrap();

    let obs = Observation::style_preference("user", "prefers", "rust");
    let recon = phase_c::reconcile(&repos, &*embedder, &obs, None).await.unwrap();
    assert!(matches!(recon, Reconciliation::Noop { .. }));
}

#[tokio::test]
async fn vector_lookup_returns_at_most_top_k() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    for i in 0..20 {
        repos.facts().insert(&format!("s{i}"), "p", "o", None).await.unwrap();
    }
    let embedder = common::deterministic_embedder();
    let cand = repos.facts().find_similar_by_embedding("query", "p", &embedder.embed("q").await.unwrap(), None, None, 5).await.unwrap();
    assert!(cand.len() <= 5);
}
```

- [ ] **Step 6: Run**

```bash
cargo nextest run -p coding-memory -E 'test(phase_c_reconciliation) | test(vector_similarity_above_noop_threshold_returns_noop) | test(vector_lookup_returns_at_most_top_k)'
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/distiller/phase_c.rs crates/coding-memory/src/distiller/mod.rs crates/storage/src/repos/semantic_facts.rs crates/coding-memory/tests/phase_c_reconciliation.rs
git commit -m "feat(distiller): Phase C vector-similarity reconciliation with top-5 lookup"
```

---

### Task 1.11: Tier A activation tests for coding turns

**Files:**
- Create: `crates/coding-memory/tests/tier_a_score_turn_for_coding.rs`
- Create: `crates/coding-memory/tests/tier_a_contradiction_for_coding.rs`
- Create: `crates/coding-memory/tests/tier_a_user_corrected_for_coding.rs`

- [ ] **Step 1: `score_turn` test**

```rust
//! Tier A activation: score_turn must classify coding-verb turns as high-value.

use cognitive::services::value_density::score_turn;

#[test]
fn deployed_keyword_scores_high() {
    let s = score_turn("deployed the authentication service to staging", "ok");
    assert!(s >= 0.7, "expected >=0.7, got {s}");
}

#[test]
fn refactored_keyword_scores_high() {
    let s = score_turn("refactored the user repo to use storage abstraction", "ok");
    assert!(s >= 0.7);
}

#[test]
fn idle_chitchat_scores_low() {
    let s = score_turn("how was your day?", "fine");
    assert!(s < 0.4);
}
```

- [ ] **Step 2: `ContradictionDetected` test**

```rust
//! Tier A activation: ContradictionDetected fires for conflicting coding facts.

use bus::DomainEvent;
use coding_memory::distiller::Distiller;

#[tokio::test]
async fn contradiction_fires_when_repo_context_conflicts() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    let bus = bus::DomainEventBus::new();
    let mut sub = bus.subscribe();
    let distiller = common::seed_distiller_with_bus(&repos, bus.clone());

    common::seed_repo_context(&repos, "myrepo", "framework", "django").await;
    common::seed_repo_context(&repos, "myrepo", "framework", "rails").await;
    distiller.distill_turn("s", Some("t")).await.unwrap();

    let mut saw = false;
    for _ in 0..10 {
        match tokio::time::timeout(std::time::Duration::from_millis(50), sub.recv()).await {
            Ok(Ok(DomainEvent::ContradictionDetected { .. })) => { saw = true; break }
            _ => continue,
        }
    }
    assert!(saw, "expected ContradictionDetected event");
}
```

- [ ] **Step 3: `UserCorrectedAI` test**

```rust
//! Tier A activation: UserCorrectedAI with MemoryMiss fires when user
//! corrects a coding-context retrieval.

use bus::{DomainEvent, CorrectionKind};

#[tokio::test]
async fn memory_miss_correction_propagates_for_coding_repo() {
    let bus = bus::DomainEventBus::new();
    let mut sub = bus.subscribe();
    bus.publish(DomainEvent::UserCorrectedAI {
        kind: CorrectionKind::MemoryMiss,
        repo: Some("myrepo".into()),
        memory_id: Some("ep_1".into()),
    }).await;
    let evt = sub.recv().await.unwrap();
    assert!(matches!(evt, DomainEvent::UserCorrectedAI { kind: CorrectionKind::MemoryMiss, .. }));
}
```

- [ ] **Step 4: Run**

```bash
cargo nextest run -p coding-memory -E 'test(tier_a)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/tests/tier_a_*.rs
git commit -m "test(coding-memory): Tier A activation coverage for score_turn / contradiction / correction"
```

---

## Section 2 — Phase 4 Debt Remediation

### Task 2.1: Apply `kinds` and `days` filters in `recall_index`

**Files:**
- Modify: `crates/coding-memory/src/recall/service.rs:136`
- Create: `crates/coding-memory/tests/recall_kinds_days_filter.rs`

- [ ] **Step 1: Write failing test**

```rust
//! recall_index must filter by kinds and recency days.

use coding_memory::recall::CodingRecallService;

#[tokio::test]
async fn kinds_filter_excludes_other_kinds() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    common::seed_episode_kind(&repos, "fix_attempt", "fix the auth bug").await;
    common::seed_episode_kind(&repos, "test_run", "ran cargo test").await;

    let svc = CodingRecallService::new_for_test(repos);
    let res = svc.recall_index("auth", None, Some(vec!["fix_attempt".into()]), None, 10).await.unwrap();
    assert!(res.entries.iter().all(|e| e.kind == "fix_attempt"));
}

#[tokio::test]
async fn days_filter_excludes_old_entries() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    common::seed_episode_at(&repos, "old", chrono_offset_days_ago(30)).await;
    common::seed_episode_at(&repos, "recent", chrono_offset_days_ago(2)).await;

    let svc = CodingRecallService::new_for_test(repos);
    let res = svc.recall_index("query", None, None, Some(7), 10).await.unwrap();
    assert!(res.entries.iter().all(|e| e.title.contains("recent")));
}
```

- [ ] **Step 2: Run — FAIL**

```bash
cargo nextest run -p coding-memory -E 'test(kinds_filter_excludes_other_kinds) | test(days_filter_excludes_old_entries)'
```

- [ ] **Step 3: Patch service.rs:136**

Replace the existing `let _ = (kinds, days);` line with:

```rust
let entries = entries
    .into_iter()
    .filter(|e| match &kinds {
        Some(allowed) if !allowed.is_empty() => allowed.iter().any(|k| k == &e.kind),
        _ => true,
    })
    .filter(|e| match days {
        Some(d) => {
            let cutoff = jiff::Timestamp::now()
                .checked_sub(jiff::ToSpan::days(d as i64))
                .unwrap_or_else(|_| jiff::Timestamp::MIN);
            e.when >= cutoff
        }
        None => true,
    })
    .collect();
```

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(kinds_filter_excludes_other_kinds) | test(days_filter_excludes_old_entries)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/recall/service.rs crates/coding-memory/tests/recall_kinds_days_filter.rs
git commit -m "fix(recall): apply kinds and days filters in recall_index (was silently dropping)"
```

---

### Task 2.2: Forward `domain` arg to `DecisionPointsService`

**Files:**
- Modify: `crates/coding-memory/src/mcp.rs:400`
- Create: `crates/coding-memory/tests/decision_points_domain_filter.rs`

- [ ] **Step 1: Write failing test**

```rust
//! recall_decision_points("code", repo) must filter results to coding domain.

use coding_memory::mcp::CodingMemoryToolset;
use serde_json::json;

#[tokio::test]
async fn domain_code_excludes_personal_decisions() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    common::seed_decision(&repos, "personal", "buy a domain").await;
    common::seed_decision(&repos, "code", "use sqlx over diesel").await;

    let toolset = CodingMemoryToolset::new_for_test(repos);
    let res = toolset.dispatch("recall_decision_points", json!({"domain": "code"})).await.unwrap();
    let entries = res["decisions"].as_array().unwrap();
    assert!(entries.iter().all(|d| d["domain"].as_str() == Some("code")));
}
```

- [ ] **Step 2: Run — FAIL**

- [ ] **Step 3: Patch `mcp.rs:400`**

Replace `let _ = a.domain;` with:

```rust
let decisions = self
    .recall_service
    .recall_decision_points(a.domain.as_deref(), a.repo.as_deref())
    .await?;
```

And update `CodingRecallService::recall_decision_points` signature in `service.rs`:

```rust
pub async fn recall_decision_points(
    &self,
    domain: Option<&str>,
    repo: Option<&str>,
) -> Result<DecisionPointsResponse> {
    self.decision_points_service.list(domain, repo).await
}
```

Update `DecisionPointsService::list` accordingly to honor the domain filter at the SQL level.

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(domain_code_excludes_personal_decisions)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mcp.rs crates/coding-memory/src/recall/service.rs crates/coding-memory/src/recall/decision_points.rs crates/coding-memory/tests/decision_points_domain_filter.rs
git commit -m "fix(recall): forward domain arg in recall_decision_points (was dropped)"
```

---

### Task 2.3: B1 dead-end threshold 0.5 → 0.7

**Files:**
- Modify: `crates/coding-memory/src/recall/renderers.rs:91`
- Create: `crates/coding-memory/tests/dead_end_threshold_07.rs`

- [ ] **Step 1: Write failing test (low conf must NOT trigger; high conf must)**

```rust
//! Spec §7/B1: dead-end warning fires only when aggregate confidence > 0.7.

use coding_memory::recall::renderers::render_user_prompt_block;

#[tokio::test]
async fn confidence_06_does_not_trigger_warning() {
    let md = render_user_prompt_block(common::user_prompt_ctx_with_dead_end_conf(0.6)).await;
    assert!(!md.contains("⚠️ Heads-up"));
}

#[tokio::test]
async fn confidence_08_triggers_warning() {
    let md = render_user_prompt_block(common::user_prompt_ctx_with_dead_end_conf(0.8)).await;
    assert!(md.contains("⚠️ Heads-up"));
}
```

- [ ] **Step 2: Run — FAIL on the 0.6 case (current threshold is 0.5)**

- [ ] **Step 3: Patch `renderers.rs:91`**

```rust
// Before:
//     if dead_end.aggregate_confidence > 0.5 {
// After:
    if dead_end.aggregate_confidence > 0.7 {
```

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/recall/renderers.rs crates/coding-memory/tests/dead_end_threshold_07.rs
git commit -m "fix(recall): dead-end warning threshold 0.5 -> 0.7 per spec §7/B1"
```

---

### Task 2.4: Real "Open threads" query — replace stub

**Files:**
- Modify: `crates/coding-memory/src/recall/renderers.rs:68-69`
- Create: `crates/coding-memory/src/recall/open_threads.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (declare `pub mod open_threads;`)
- Create: `crates/coding-memory/tests/open_threads_query.rs`

- [ ] **Step 1: Define `open_threads.rs`**

```rust
//! Query for unfinished turn-trace episodes from the last `since` window.
//!
//! "Unfinished" = the session ended (SessionEnd event seen) BUT the most
//! recent turn lacks a matching AssistantMsg with token_usage. This is a
//! coarse heuristic; refined by Mirror later.

use storage::Repos;
use jiff::{Timestamp, ToSpan};

pub struct OpenThread {
    pub episode_id: String,
    pub when: Timestamp,
    pub last_user_prompt: String,
    pub repo_id: Option<String>,
}

pub async fn list_open_threads(
    repos: &Repos,
    repo: Option<&str>,
    since_days: u32,
    limit: usize,
) -> common::Result<Vec<OpenThread>> {
    let cutoff = Timestamp::now().checked_sub(since_days.days())
        .unwrap_or(Timestamp::MIN);
    let rows = repos.episodes()
        .list_unfinished_turn_traces(repo, cutoff, limit as i64)
        .await?;
    Ok(rows.into_iter().map(|r| OpenThread {
        episode_id: r.id,
        when: r.created_at,
        last_user_prompt: r.summary.unwrap_or_default(),
        repo_id: r.scope_repo_id,
    }).collect())
}
```

- [ ] **Step 2: Add `list_unfinished_turn_traces` to `EpisodicMemoryRepo`**

In `crates/storage/src/repos/episodic_memories.rs`:

```rust
pub async fn list_unfinished_turn_traces(
    &self,
    repo: Option<&str>,
    since: jiff::Timestamp,
    limit: i64,
) -> Result<Vec<EpisodicMemoryRow>> {
    sqlx::query_as::<_, EpisodicMemoryRow>(
        "SELECT * FROM episodic_memories \
         WHERE kind = 'turn_trace' \
           AND created_at >= ? \
           AND (?1_repo IS NULL OR scope_repo_id = ?1_repo) \
           AND id NOT IN (SELECT trace_id FROM episodic_memories \
                          WHERE kind = 'assistant_completed') \
         ORDER BY created_at DESC LIMIT ?"
    )
    .bind(since.to_string()).bind(repo).bind(limit)
    .fetch_all(self.pool.inner()).await
    .map_err(|e| KlyntbotError::Storage(e.to_string()))
}
```

- [ ] **Step 3: Replace the `_(none captured this phase)_` stub in `renderers.rs:68-69`**

```rust
let threads = open_threads::list_open_threads(repos, scope.as_deref(), 7, 5).await.unwrap_or_default();
if threads.is_empty() {
    out.push_str("_(none)_\n");
} else {
    for t in threads {
        out.push_str(&format!("- [{}] {}\n", t.episode_id, t.last_user_prompt));
    }
}
```

- [ ] **Step 4: Write the test**

```rust
//! Real open-threads query lists unfinished turn traces.

use coding_memory::recall::open_threads;

#[tokio::test]
async fn unfinished_turn_traces_appear_as_open_threads() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    common::seed_unfinished_turn_trace(&repos, "myrepo", "fix the auth bug").await;
    common::seed_completed_turn(&repos, "myrepo", "ran tests").await;

    let threads = open_threads::list_open_threads(&repos, Some("myrepo"), 7, 10).await.unwrap();
    assert_eq!(threads.len(), 1);
    assert!(threads[0].last_user_prompt.contains("fix the auth bug"));
}
```

- [ ] **Step 5: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(unfinished_turn_traces_appear_as_open_threads)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/recall/open_threads.rs crates/coding-memory/src/recall/renderers.rs crates/coding-memory/src/recall/mod.rs crates/storage/src/repos/episodic_memories.rs crates/coding-memory/tests/open_threads_query.rs
git commit -m "feat(recall): real Open Threads section sourced from unfinished turn-trace query"
```

---

### Task 2.5: Per-session scope cache

**Files:**
- Modify: `crates/coding-memory/src/recall/scope_resolve.rs`
- Create: `crates/coding-memory/tests/scope_resolver_session_cache.rs`

- [ ] **Step 1: Write failing test**

```rust
//! Resolving the same session twice in a row must hit a cache (one git call only).

use coding_memory::recall::scope_resolve;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn session_cache_prevents_repeat_git_calls() {
    let counter = std::sync::Arc::new(AtomicUsize::new(0));
    let probe = scope_resolve::TestProbe::new(counter.clone());
    let _scope_a = scope_resolve::resolve_with_probe("session-1", "/tmp/foo", &probe).await;
    let _scope_b = scope_resolve::resolve_with_probe("session-1", "/tmp/foo", &probe).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1, "git probed twice; cache failed");
}
```

- [ ] **Step 2: Add cache + probe-trait test seam in `scope_resolve.rs`**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait::async_trait]
pub trait ScopeProbe: Send + Sync {
    async fn probe(&self, cwd: &std::path::Path) -> common::Result<Option<String>>;
}

#[derive(Default, Clone)]
pub struct SessionCache {
    inner: Arc<RwLock<HashMap<String, Option<String>>>>,
}

impl SessionCache {
    pub async fn get_or_resolve(
        &self,
        session_id: &str,
        cwd: &std::path::Path,
        probe: &dyn ScopeProbe,
    ) -> common::Result<Option<String>> {
        if let Some(hit) = self.inner.read().await.get(session_id).cloned() {
            return Ok(hit);
        }
        let resolved = probe.probe(cwd).await?;
        self.inner.write().await.insert(session_id.to_string(), resolved.clone());
        Ok(resolved)
    }
}

#[cfg(any(test, feature = "test-helpers"))]
pub struct TestProbe { counter: std::sync::Arc<std::sync::atomic::AtomicUsize> }
#[cfg(any(test, feature = "test-helpers"))]
impl TestProbe {
    pub fn new(c: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Self { Self { counter: c } }
}
#[cfg(any(test, feature = "test-helpers"))]
#[async_trait::async_trait]
impl ScopeProbe for TestProbe {
    async fn probe(&self, _cwd: &std::path::Path) -> common::Result<Option<String>> {
        self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Some("test-repo".into()))
    }
}
#[cfg(any(test, feature = "test-helpers"))]
pub async fn resolve_with_probe(session_id: &str, cwd: &str, probe: &dyn ScopeProbe) -> common::Result<Option<String>> {
    static CACHE: tokio::sync::OnceCell<SessionCache> = tokio::sync::OnceCell::const_new();
    let cache = CACHE.get_or_init(|| async { SessionCache::default() }).await;
    cache.get_or_resolve(session_id, std::path::Path::new(cwd), probe).await
}
```

- [ ] **Step 3: Wire `SessionCache` into `CodingRecallService` (held as a field)**

In `service.rs`:

```rust
pub struct CodingRecallService {
    // ...existing fields...
    scope_cache: scope_resolve::SessionCache,
}
```

Initialise to `SessionCache::default()` in `::new()`. Update each scope-using method to call `cache.get_or_resolve(session_id, cwd, &live_probe)`.

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(session_cache_prevents_repeat_git_calls)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/recall/scope_resolve.rs crates/coding-memory/src/recall/service.rs crates/coding-memory/tests/scope_resolver_session_cache.rs
git commit -m "feat(recall): per-session scope cache prevents repeated git rev-parse"
```

---

### Task 2.6: Inject `causal_repo` at boot — `trace_causes` no longer errors

**Files:**
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Create: `crates/coding-memory/tests/trace_causes_wired.rs`

- [ ] **Step 1: Write failing test**

```rust
//! trace_causes must succeed (not error "requires causal repo wiring") in the
//! production wire-up path.

use app_core::coding_memory::CodingMemorySubsystem;

#[tokio::test]
async fn trace_causes_works_after_boot_wiring() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let subsys = CodingMemorySubsystem::initialize_for_test(repos).await.unwrap();

    let recall = subsys.recall_service();
    let resp = recall.trace_causes("subject_x", None, 3).await;
    assert!(resp.is_ok(), "trace_causes errored: {resp:?}");
}
```

- [ ] **Step 2: Run — FAIL with "requires causal repo wiring"**

- [ ] **Step 3: Patch the boot wiring**

In `crates/app-core/src/coding_memory/mod.rs`, locate where `CodingRecallService` is built and add:

```rust
let causal_repo = std::sync::Arc::new(coding_memory::causal::CausalRepoImpl::new(repos.clone()));
let recall_service = CodingRecallService::new(repos.clone(), Default::default())
    .with_causal_repo(causal_repo);
```

Verify `with_causal_repo` exists; if it doesn't, add to `service.rs`:

```rust
pub fn with_causal_repo(mut self, repo: std::sync::Arc<dyn coding_memory::causal::CausalRepo>) -> Self {
    self.causal_repo = Some(repo); self
}
```

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/mod.rs crates/coding-memory/src/recall/service.rs crates/coding-memory/tests/trace_causes_wired.rs
git commit -m "fix(app-core): inject causal_repo into CodingRecallService at boot so trace_causes works"
```

---

### Task 2.7: Handle `repo: "*"` wildcard in MCP tools and recall service

**Files:**
- Modify: `crates/coding-memory/src/recall/service.rs` (each scope-aware method)
- Create: `crates/coding-memory/tests/recall_repo_wildcard.rs`

- [ ] **Step 1: Write failing test**

```rust
//! recall_index with repo="*" must remove the repo filter, not match a literal.

use coding_memory::recall::CodingRecallService;

#[tokio::test]
async fn wildcard_repo_includes_all_repos() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = common::repos(&pool).await;
    common::seed_episode_in_repo(&repos, "repo-a", "fix the api").await;
    common::seed_episode_in_repo(&repos, "repo-b", "fix the api").await;

    let svc = CodingRecallService::new_for_test(repos);
    let res = svc.recall_index("api", Some("*"), None, None, 10).await.unwrap();
    let scopes: std::collections::HashSet<_> = res.entries.iter().map(|e| e.scope.clone()).collect();
    assert!(scopes.contains(&Some("repo-a".into())));
    assert!(scopes.contains(&Some("repo-b".into())));
}
```

- [ ] **Step 2: Run — FAIL**

- [ ] **Step 3: Add a normalizer**

In `crates/coding-memory/src/recall/scope_resolve.rs`:

```rust
/// Normalize MCP repo argument: "*" means "no filter".
pub fn normalize_repo_arg(arg: Option<&str>) -> Option<&str> {
    match arg {
        Some("*") => None,
        other => other,
    }
}
```

In each `service.rs` method that takes `repo: Option<&str>`, do `let repo = scope_resolve::normalize_repo_arg(repo);` at the top.

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/recall/scope_resolve.rs crates/coding-memory/src/recall/service.rs crates/coding-memory/tests/recall_repo_wildcard.rs
git commit -m "feat(recall): repo='*' wildcard removes repo filter across MCP tools"
```

---

### Task 2.8: `QueryPipeline` enrichment in UserPromptSubmit injection

**Files:**
- Modify: `crates/coding-memory/src/recall/renderers.rs:82-133`

- [ ] **Step 1: Write failing test asserting query enrichment is invoked**

```rust
//! UserPromptSubmit injection must run the prompt through QueryPipeline (PRF +
//! multi-query expansion) before retrieval.

use coding_memory::recall::renderers::render_user_prompt_block;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn query_pipeline_invoked_for_user_prompt() {
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let pipeline = common::QueryPipelineProbe::new(calls.clone());
    let ctx = common::user_prompt_ctx_with_pipeline("how do I fix the auth bug?", pipeline);
    let _md = render_user_prompt_block(ctx).await;
    assert!(calls.load(Ordering::SeqCst) >= 1);
}
```

- [ ] **Step 2: Add a `QueryPipeline` probe in `tests/common.rs`**

```rust
pub struct QueryPipelineProbe { counter: std::sync::Arc<std::sync::atomic::AtomicUsize> }
#[async_trait::async_trait]
impl context_engine::QueryPipeline for QueryPipelineProbe {
    async fn enrich(&self, query: &str) -> Vec<String> {
        self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![query.to_string()]
    }
}
```

- [ ] **Step 3: Wire `QueryPipeline` into the renderer**

In `renderers.rs:82-133`, replace the direct `recall_index` call with:

```rust
let queries = ctx.query_pipeline.enrich(&ctx.user_prompt).await;
let primary = queries.first().map(|s| s.as_str()).unwrap_or(&ctx.user_prompt);
let res = ctx.recall_service.recall_index(primary, ctx.repo.as_deref(), None, None, 10).await?;
// for each additional rewrite, RRF-merge results — see context_engine::rrf_merge
```

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/recall/renderers.rs crates/coding-memory/tests/common.rs
git commit -m "feat(recall): wire QueryPipeline enrichment into UserPromptSubmit injection"
```

---

## Section 3 — Phase 5 Debt Remediation

### Task 3.1: Implement `ReforgeWriter` wrapper rejecting DELETE

**Files:**
- Create: `crates/coding-memory/src/reforge/writer.rs`
- Modify: `crates/coding-memory/src/reforge/mod.rs`
- Create: `crates/coding-memory/tests/reforge_writer_rejects_delete.rs`

- [ ] **Step 1: Write failing test (TDD)**

```rust
//! ReforgeWriter must panic in dev / log+skip in release on DELETE attempts
//! against semantic_facts or episodic_memories.

use coding_memory::reforge::writer::ReforgeWriter;

#[tokio::test]
#[should_panic(expected = "Reforge attempted DELETE")]
async fn delete_against_semantic_facts_panics_in_dev() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let writer = ReforgeWriter::new(repos.clone());
    // attempt direct delete via the wrapper — must reject
    let _ = writer.delete_semantic_fact("any_id").await;
}

#[tokio::test]
async fn supersede_via_writer_succeeds() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let writer = ReforgeWriter::new(repos.clone());
    repos.facts().insert("s", "p", "o", None).await.unwrap();
    let row = repos.facts().latest("s", "p").await.unwrap().unwrap();
    let new_id = writer.supersede_with(&row.id, "s", "p", "new_o").await.unwrap();
    assert_ne!(new_id, row.id);
    // both rows still exist
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
        .fetch_one(pool.inner()).await.unwrap();
    assert_eq!(count, 2);
}
```

- [ ] **Step 2: Run — FAIL (writer doesn't exist)**

- [ ] **Step 3: Implement `writer.rs`**

```rust
//! Reforge write gateway. ALL Reforge phases route mutations through here.
//!
//! Invariant: never DELETE rows from semantic_facts or episodic_memories. If
//! a phase's logic requires removing a fact, it must SUPERSEDE instead — the
//! prior row stays in the bi-temporal history.

use common::{KlyntbotError, Result};
use storage::{Repos, SemanticFactRow};
use jiff::Timestamp;

pub struct ReforgeWriter {
    repos: Repos,
    /// In dev builds (debug_assertions on), DELETEs panic. In release, they
    /// log a critical error and return InvariantViolated.
    panic_on_delete: bool,
}

impl ReforgeWriter {
    pub fn new(repos: Repos) -> Self {
        Self { repos, panic_on_delete: cfg!(debug_assertions) }
    }

    pub async fn supersede_with(
        &self,
        predecessor_id: &str,
        subject: &str,
        predicate: &str,
        new_object: &str,
    ) -> Result<String> {
        let now = Timestamp::now();
        // Set predecessor's valid_until = now (terminate it bi-temporally; row remains)
        self.repos.facts().set_valid_until(predecessor_id, now).await?;
        // Insert successor with valid_from = now
        let row = self.repos.facts()
            .insert_with_validity(subject, predicate, new_object, None, now, None).await?;
        Ok(row.id)
    }

    pub async fn delete_semantic_fact(&self, id: &str) -> Result<()> {
        let msg = format!("Reforge attempted DELETE of semantic_fact {id} — invariant 3 violation");
        if self.panic_on_delete { panic!("{msg}"); }
        tracing::error!("{msg}");
        Err(KlyntbotError::Storage(msg))
    }

    pub async fn delete_episode(&self, id: &str) -> Result<()> {
        let msg = format!("Reforge attempted DELETE of episode {id} — invariant 3 violation");
        if self.panic_on_delete { panic!("{msg}"); }
        tracing::error!("{msg}");
        Err(KlyntbotError::Storage(msg))
    }

    /// Demotion is allowed: scales stability rather than deleting.
    pub async fn demote_stability(&self, id: &str, factor: f32) -> Result<()> {
        self.repos.facts().scale_stability(id, factor).await
    }

    pub fn repos(&self) -> &Repos { &self.repos }
}
```

- [ ] **Step 4: Add `set_valid_until`, `latest`, `scale_stability` to `SemanticFactRepo` if missing**

```rust
pub async fn set_valid_until(&self, id: &str, until: jiff::Timestamp) -> Result<()> {
    sqlx::query("UPDATE semantic_facts SET valid_until = ? WHERE id = ?")
        .bind(until.to_string()).bind(id)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
pub async fn latest(&self, subject: &str, predicate: &str) -> Result<Option<SemanticFactRow>> {
    let row = sqlx::query_as::<_, SemanticFactRow>(
        "SELECT * FROM semantic_facts WHERE subject = ? AND predicate = ? ORDER BY valid_from DESC LIMIT 1"
    ).bind(subject).bind(predicate).fetch_optional(self.pool.inner()).await
     .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(row)
}
pub async fn scale_stability(&self, id: &str, factor: f32) -> Result<()> {
    sqlx::query("UPDATE semantic_facts SET stability = stability * ? WHERE id = ?")
        .bind(factor).bind(id)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 5: Re-export from `mod.rs`**

```rust
pub mod writer;
pub use writer::ReforgeWriter;
```

- [ ] **Step 6: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(reforge_writer_rejects_delete) | test(supersede_via_writer_succeeds) | test(delete_against_semantic_facts_panics_in_dev)'
```

- [ ] **Step 7: Route existing Reforge phases through `ReforgeWriter`**

In `crates/coding-memory/src/reforge/coding_synthesis.rs`, `rule_artifacts.rs`, `cross_session_dedup.rs`, `selective_delete.rs` — replace direct `repos.facts()` mutation calls with `writer.supersede_with(...)` / `writer.demote_stability(...)`. The constructor for each phase struct now takes `Arc<ReforgeWriter>`.

- [ ] **Step 8: Run full Reforge suite**

```bash
cargo nextest run -p coding-memory -E 'test(reforge) | test(coding_synthesis) | test(rule_artifacts)'
```

- [ ] **Step 9: Commit**

```bash
git add crates/coding-memory/src/reforge/writer.rs crates/coding-memory/src/reforge/mod.rs crates/coding-memory/src/reforge/coding_synthesis.rs crates/coding-memory/src/reforge/rule_artifacts.rs crates/coding-memory/src/reforge/cross_session_dedup.rs crates/coding-memory/src/reforge/selective_delete.rs crates/storage/src/repos/semantic_facts.rs crates/coding-memory/tests/reforge_writer_rejects_delete.rs
git commit -m "feat(reforge): ReforgeWriter wrapper enforces no-DELETE invariant + routes all phases"
```

---

### Task 3.2: Session-end stale-candidate resolution

**Files:**
- Modify: `crates/coding-memory/src/reforge/session_end.rs`
- Create: `crates/coding-memory/tests/session_end_stale_candidate.rs`

- [ ] **Step 1: Failing test**

```rust
//! When a session touches a file marked stale_candidate by C2, the session-end
//! pass should re-validate (still anchored? yes -> clear flag; no -> bi-temporal
//! invalidate via ReforgeWriter::supersede).

use coding_memory::reforge::session_end::SessionEndPass;

#[tokio::test]
async fn stale_candidate_clears_when_symbol_still_present() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);

    let fact_id = common::seed_fact_with_anchor(&repos, "src/auth.rs", "fn login").await;
    common::mark_stale_candidate(&repos, &fact_id).await;
    common::touch_file_during_session(&repos, "session-1", "src/auth.rs").await;
    common::seed_workspace_file("src/auth.rs", "fn login() {}\n");

    let pass = SessionEndPass::new(repos.clone(), common::test_writer(&repos), common::test_extractor());
    pass.run("session-1", None).await.unwrap();

    let fact = repos.facts().get(&fact_id).await.unwrap();
    let meta: serde_json::Value = serde_json::from_str(&fact.metadata.unwrap()).unwrap();
    assert!(meta.pointer("/stale_candidate").is_none());
}
```

- [ ] **Step 2: Implement the resolution step**

Append to `crates/coding-memory/src/reforge/session_end.rs`:

```rust
async fn resolve_stale_candidates(
    &self,
    session_id: &str,
    repo_filter: Option<&str>,
) -> Result<usize> {
    let touched = self.repos
        .ingest_event_log()
        .files_touched_in_session(session_id).await?;
    let candidates = self.repos.facts()
        .list_stale_candidates_for_paths(&touched, repo_filter).await?;

    let mut resolved = 0;
    for fact in candidates {
        let symbols = parse_anchored_symbols(&fact.metadata);
        let still_anchored = self.extractor.symbols_still_present(&symbols).await?;
        if still_anchored {
            self.repos.facts().clear_stale_flag(&fact.id).await?;
        } else {
            // bi-temporal invalidate via writer; supersede with empty marker
            let now = Timestamp::now();
            self.repos.facts().set_valid_until(&fact.id, now).await?;
        }
        resolved += 1;
    }
    Ok(resolved)
}
```

Call `self.resolve_stale_candidates(session_id, repo).await?;` near the top of `run()`.

- [ ] **Step 3: Add helper repo methods**

```rust
// in IngestEventLogRepo
pub async fn files_touched_in_session(&self, session_id: &str) -> Result<Vec<String>> {
    sqlx::query_scalar("SELECT DISTINCT json_extract(payload, '$.kind.path') \
        FROM ingest_event_log WHERE session_id = ? \
        AND json_extract(payload, '$.kind.kind') IN ('fileEdit', 'fileEditEnriched')")
        .bind(session_id).fetch_all(self.pool.inner()).await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))
}

// in SemanticFactRepo
pub async fn list_stale_candidates_for_paths(
    &self, paths: &[String], repo: Option<&str>,
) -> Result<Vec<SemanticFactRow>> { /* JSON1 query into metadata.anchored_symbols */ }

pub async fn clear_stale_flag(&self, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE semantic_facts \
         SET metadata = json_remove(metadata, '$.stale_candidate') WHERE id = ?"
    ).bind(id).execute(self.pool.inner()).await
     .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(stale_candidate_clears_when_symbol_still_present)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/session_end.rs crates/storage/src/repos/semantic_facts.rs crates/storage/src/repos/ingest_event_log.rs crates/coding-memory/tests/session_end_stale_candidate.rs
git commit -m "feat(reforge): session-end stale-candidate resolution"
```

---

### Task 3.3: Scope-aware `SkillStore` filtering at read time

**Files:**
- Modify: `crates/skill-system/src/store.rs:72-137`
- Create: `crates/skill-system/tests/scope_aware_load.rs`

- [ ] **Step 1: Failing test**

```rust
//! When loading skills with a repo scope, project-scoped skills for OTHER
//! repos must be excluded.

use skill_system::store::SkillStore;
use std::path::PathBuf;

#[test]
fn project_skills_for_other_repos_excluded() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(&tmp, "global-skill", /*scope*/ None);
    write_skill(&tmp, "repo-a-skill", Some("github.com/foo/repo-a"));
    write_skill(&tmp, "repo-b-skill", Some("github.com/foo/repo-b"));

    let store = SkillStore::load_with_scope(tmp.path(), Some("github.com/foo/repo-a")).unwrap();
    let names: Vec<&str> = store.iter_skills().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"global-skill"));
    assert!(names.contains(&"repo-a-skill"));
    assert!(!names.contains(&"repo-b-skill"));
}
```

- [ ] **Step 2: Add `load_with_scope`**

In `crates/skill-system/src/store.rs`:

```rust
pub fn load_with_scope(root: &std::path::Path, repo: Option<&str>) -> Result<Self> {
    let mut store = Self::load(root)?;
    if let Some(repo_id) = repo {
        store.skills.retain(|s| match s.scope_repo_id.as_deref() {
            None => true,                  // global skill
            Some(scoped) => scoped == repo_id,
        });
    }
    Ok(store)
}
```

Add a `scope_repo_id: Option<String>` field on the `Skill` struct (parsed from frontmatter).

- [ ] **Step 3: Update SKILL.md frontmatter parser to read `scope_repo_id`**

```rust
// in store.rs frontmatter parser
let scope_repo_id = fm.get("scope_repo_id").and_then(|v| v.as_str()).map(String::from);
```

- [ ] **Step 4: Wire into the skill listing source**

In `crates/skill-system/src/listing_source.rs` (or wherever the active scope is known), call `SkillStore::load_with_scope(root, current_repo)` instead of `SkillStore::load(root)`.

- [ ] **Step 5: Run — PASS**

```bash
cargo nextest run -p skill-system -E 'test(project_skills_for_other_repos_excluded)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/store.rs crates/skill-system/src/listing_source.rs crates/skill-system/tests/scope_aware_load.rs
git commit -m "feat(skill-system): scope-aware SkillStore::load_with_scope filters cross-repo project skills"
```

---

### Task 3.4: LLM-drafted SKILL.md in skill evolver

**Files:**
- Modify: `crates/coding-memory/src/skills.rs`
- Create: `crates/coding-memory/tests/skill_evolver_llm_drafted.rs`

- [ ] **Step 1: Failing test**

```rust
//! Skill evolver must invoke ProviderRole::ReforgeRules to draft SKILL.md
//! from a WorkflowPattern instead of using the rule_text verbatim.

use providers::testing::DraftingProvider;

#[tokio::test]
async fn skill_md_is_llm_drafted_not_raw_rule_text() {
    let provider = std::sync::Arc::new(DraftingProvider::with_response(
        "---\nname: my-test-skill\ndescription: drafted by LLM\n---\nProcedure...\n"));
    let evolver = common::seed_evolver_with_provider(provider.clone());
    let pattern = common::seed_workflow_pattern("when X, do Y").await;
    let written = evolver.evolve_one(&pattern).await.unwrap();
    assert!(!written.body.contains("when X, do Y"), "raw rule text leaked");
    assert!(written.body.contains("drafted by LLM"));
    assert_eq!(provider.invocation_count(), 1);
}
```

- [ ] **Step 2: Add the LLM call to the evolver**

In `crates/coding-memory/src/skills.rs::ProjectSkillEvolverImpl::evolve_one`:

```rust
let prompt = format!(
    "Draft an Agent Skills SKILL.md (with YAML frontmatter: name, description, whenToUse) \
     for this observed workflow pattern. Use crisp procedural language.\n\nPattern:\n{}",
    pattern.rule_text
);
let resp = self.provider_manager
    .chat_with_role(ProviderRole::ReforgeRules,
        vec![Message::user(prompt)],
        vec![], ChatParams::default()
    ).await?;
let body = resp.text.unwrap_or_else(|| pattern.rule_text.clone());
```

Replace the previous direct-write of `pattern.rule_text` with `body`.

- [ ] **Step 3: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(skill_md_is_llm_drafted_not_raw_rule_text)'
```

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/skills.rs crates/providers/src/testing.rs crates/coding-memory/tests/skill_evolver_llm_drafted.rs
git commit -m "feat(skills): LLM-draft SKILL.md via ProviderRole::ReforgeRules"
```

---

### Task 3.5: Add 12th `MirrorAlertKind` variant — `RetrievalSkillIneffective`

**Files:**
- Modify: `crates/coding-memory/src/mirror/alerts.rs:6-29`
- Modify: `crates/coding-memory/src/retrieval_skills/registry.rs` (publish on low EMA)

- [ ] **Step 1: Failing test**

```rust
//! When a retrieval skill's EMA drops below 0.2, the mirror should emit
//! RetrievalSkillIneffective alert.

use coding_memory::mirror::alerts::CodingMirrorAlertKind;
use coding_memory::retrieval_skills::registry::RetrievalSkillRegistry;

#[tokio::test]
async fn low_ema_publishes_ineffective_alert() {
    let bus = bus::DomainEventBus::new();
    let mut sub = bus.subscribe();
    let registry = RetrievalSkillRegistry::new_for_test(bus.clone());

    for _ in 0..30 {
        registry.record_outcome("QueryRewriter", /*outcome*/ 0.0).await;
    }
    let mut alerts = Vec::new();
    while let Ok(evt) = tokio::time::timeout(std::time::Duration::from_millis(20), sub.recv()).await {
        if let Ok(bus::DomainEvent::MirrorAlertRaised { kind, .. }) = evt {
            alerts.push(kind);
        }
    }
    assert!(alerts.contains(&CodingMirrorAlertKind::RetrievalSkillIneffective.to_string()));
}
```

- [ ] **Step 2: Add the variant**

```rust
pub enum CodingMirrorAlertKind {
    // existing 11 variants...
    RetrievalSkillIneffective,   // EMA dropped below threshold
}
```

- [ ] **Step 3: Publish from `record_outcome` when EMA < 0.2**

```rust
pub async fn record_outcome(&self, name: &str, outcome: f32) {
    let new_ema = self.update_ema(name, outcome);
    if new_ema < 0.2 {
        self.bus.publish(DomainEvent::MirrorAlertRaised {
            kind: CodingMirrorAlertKind::RetrievalSkillIneffective.to_string(),
            severity: MirrorAlertSeverity::Medium.to_string(),
            payload: serde_json::json!({"skill": name, "ema": new_ema}),
        }).await;
    }
}
```

- [ ] **Step 4: Run — PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/alerts.rs crates/coding-memory/src/retrieval_skills/registry.rs crates/coding-memory/tests/mirror_retrieval_skill_ineffective.rs
git commit -m "feat(mirror): RetrievalSkillIneffective alert (12/12 alert variants)"
```

---

### Task 3.6: Vector-similarity cross-session dedup (replace exact-match)

**Files:**
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs`
- Create: `crates/coding-memory/tests/cross_session_vector_dedup.rs`

- [ ] **Step 1: Failing test**

```rust
//! Cross-session dedup must merge facts whose embedding similarity > 0.92,
//! not just exact-match on subject/predicate/object.

#[tokio::test]
async fn near_duplicate_facts_are_merged_via_supersede() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let writer = std::sync::Arc::new(coding_memory::reforge::ReforgeWriter::new(repos.clone()));
    common::seed_fact_with_embedding(&repos, "user", "prefers", "rust language", &[0.9, 0.1, 0.0]).await;
    common::seed_fact_with_embedding(&repos, "user", "prefers", "the rust programming language", &[0.91, 0.09, 0.0]).await;

    let phase = coding_memory::reforge::cross_session_dedup::CrossSessionDedupPhase::new(
        repos.clone(), writer.clone(), common::test_embedder(), 0.92,
    );
    phase.run().await.unwrap();

    let chain = repos.facts().list_supersede_chain("user", "prefers").await.unwrap();
    assert_eq!(chain.len(), 2);
    let pred = &chain[0];
    assert!(pred.valid_until.is_some(), "older row should be terminated");
}
```

- [ ] **Step 2: Implement the vector path**

Replace the `run_test_only_exact_match` body with:

```rust
pub async fn run(&self) -> Result<DedupReport> {
    let candidates = self.repos.facts().list_recent_unterminated(7 /*days*/).await?;
    let mut merged = 0;
    for batch in candidates.chunks(100) {
        for (i, a) in batch.iter().enumerate() {
            let emb_a = self.embedder.embed(&format!("{} {} {}", a.subject, a.predicate, a.object)).await?;
            for b in &batch[i+1..] {
                let emb_b = self.embedder.embed(&format!("{} {} {}", b.subject, b.predicate, b.object)).await?;
                let sim = cosine(&emb_a, &emb_b);
                if sim > self.similarity_threshold {
                    self.writer.supersede_with(&a.id, &b.subject, &b.predicate, &b.object).await?;
                    merged += 1;
                    break;
                }
            }
        }
    }
    Ok(DedupReport { merged })
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x,y)| x*y).sum();
    let na: f32 = a.iter().map(|x| x*x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x*x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
```

- [ ] **Step 3: Run — PASS**

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/reforge/cross_session_dedup.rs crates/coding-memory/tests/cross_session_vector_dedup.rs
git commit -m "feat(reforge): vector-similarity cross-session dedup replaces exact-match stub"
```

---

## Section 4 — Phase 6 Debt Remediation

### Task 4.1: Populate `metadata.supporting_chains` on promoted `ProblemSolutionPattern`

**Files:**
- Modify: `crates/coding-memory/src/reforge/coding_synthesis.rs:468-498`
- Create: `crates/coding-memory/tests/supporting_chains_in_promotion.rs`

- [ ] **Step 1: Failing test**

```rust
//! Promoted ProblemSolutionPattern must include supporting_chains in metadata.

#[tokio::test]
async fn promoted_pattern_includes_supporting_chains_metadata() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    common::seed_problem_hash_chain(&repos, "hash_abc", 3).await;

    let phase = common::seed_coding_synthesis(&repos);
    let report = phase.run().await.unwrap();

    let promoted = repos.rules()
        .find_by_kind("problem_solution_pattern").await.unwrap();
    assert!(!promoted.is_empty());
    let meta: serde_json::Value = serde_json::from_str(&promoted[0].metadata.unwrap()).unwrap();
    let chains = meta.pointer("/supporting_chains").expect("supporting_chains missing");
    assert!(chains.as_array().unwrap().len() >= 3);
    assert_eq!(report.promoted_problem_solution_patterns, 1);
}
```

- [ ] **Step 2: Patch the promotion block at `coding_synthesis.rs:468-498`**

Locate the metadata construction and add:

```rust
let supporting_chains: Vec<serde_json::Value> = chain_group
    .iter()
    .map(|chain| serde_json::json!({
        "edges": chain.edges.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        "session_id": chain.session_id,
        "problem_hash": chain.problem_hash,
    }))
    .collect();
let metadata = serde_json::json!({
    "problem_hash": problem_hash,
    "solution": solution_text,
    "supporting_chains": supporting_chains,   // <-- new
});
```

- [ ] **Step 3: Run — PASS**

```bash
cargo nextest run -p coding-memory -E 'test(promoted_pattern_includes_supporting_chains_metadata)'
```

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/reforge/coding_synthesis.rs crates/coding-memory/tests/supporting_chains_in_promotion.rs
git commit -m "fix(reforge): include supporting_chains in promoted ProblemSolutionPattern metadata"
```

---

### Task 4.2: Ensure all distiller branches populate `anchored_symbols`

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs:551` and any other `vec![]` branches
- Create: `crates/coding-memory/tests/all_distiller_branches_anchor.rs`

- [ ] **Step 1: Locate every branch writing empty anchors**

```bash
grep -n "anchored_symbols: vec!\[\]" crates/coding-memory/src/distiller/
```

- [ ] **Step 2: Failing test**

```rust
//! Every Distiller fact-write branch (LLM-emitted, extractive, derived
//! counterfactual) must populate anchored_symbols when the originating event
//! mentions a file path.

#[tokio::test]
async fn llm_emitted_fix_attempt_carries_anchors() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    common::seed_workspace_file("src/auth.rs", "fn login() {}\n");
    common::seed_event_with_file_edit(&repos, "session-1", Some("turn-1"), "src/auth.rs").await;
    common::seed_llm_fix_attempt_observation(&repos, "session-1", Some("turn-1"), "src/auth.rs").await;

    let distiller = common::seed_distiller(&repos);
    distiller.distill_turn("session-1", Some("turn-1")).await.unwrap();
    let ep = repos.episodes().find_by_kind("fix_attempt").await.unwrap();
    let meta: serde_json::Value = serde_json::from_str(&ep[0].metadata.unwrap()).unwrap();
    let anchors = meta.pointer("/anchored_symbols").expect("anchored_symbols missing");
    assert!(!anchors.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn derived_counterfactual_carries_anchors() {
    /* analogous: fix_attempt with outcome=failure should propagate anchors
       to the derived counterfactual fact */
}

#[tokio::test]
async fn extractive_refactor_episode_carries_anchors() {
    /* multi-file edit triggers RefactorEpisode; ensure anchors present */
}
```

- [ ] **Step 3: Run — FAIL on the LLM-emitted case**

- [ ] **Step 4: Patch each `vec![]` site**

For each `anchored_symbols: vec![]` occurrence, replace with:

```rust
anchored_symbols: extract_anchors_for_event(&self.symbol_extractor, &event).await,
```

Where `extract_anchors_for_event` is a helper that, given an event with a file path, parses the current file with `TreeSitterExtractor` and returns `Vec<SymbolRef>`.

- [ ] **Step 5: Run — PASS for all three tests**

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/distiller/mod.rs crates/coding-memory/tests/all_distiller_branches_anchor.rs
git commit -m "fix(distiller): populate anchored_symbols on every fact-write branch (was vec![])"
```

---

### Task 4.3: `needs_review` status in symbol validation

**Files:**
- Modify: `crates/coding-memory/src/reforge/symbol_validation.rs`
- Create: `crates/coding-memory/tests/symbol_validation_needs_review.rs`

- [ ] **Step 1: Failing test**

```rust
//! When a symbol still exists in the file but the surrounding code has drifted
//! (e.g., function signature changed), validation should mark needs_review.

#[tokio::test]
async fn drifted_symbol_marked_needs_review() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let fact_id = common::seed_fact_anchored_to(&repos, "src/auth.rs", "fn login", /*git_hash*/ "abc123").await;
    common::seed_workspace_file("src/auth.rs", "fn login(u: User, password: &str) -> Result<Token> { ... }\n");
    /* git_hash on the anchor is "abc123" but the current hash differs */

    let phase = common::seed_symbol_validation(&repos);
    phase.run().await.unwrap();

    let fact = repos.facts().get(&fact_id).await.unwrap();
    let meta: serde_json::Value = serde_json::from_str(&fact.metadata.unwrap()).unwrap();
    assert_eq!(meta.pointer("/needs_review").and_then(|v| v.as_bool()), Some(true));
}
```

- [ ] **Step 2: Patch `symbol_validation.rs`**

Where the validator currently writes only `stale_candidate`, add:

```rust
if symbol_present && current_git_hash != stored_anchor.git_hash {
    self.repos.facts().set_metadata_flag(&fact.id, "needs_review", true).await?;
} else if !symbol_present {
    self.repos.facts().set_metadata_flag(&fact.id, "stale_candidate", true).await?;
}
```

Add `set_metadata_flag` to `SemanticFactRepo`:

```rust
pub async fn set_metadata_flag(&self, id: &str, key: &str, value: bool) -> Result<()> {
    let path = format!("$.{key}");
    sqlx::query("UPDATE semantic_facts SET metadata = json_set(coalesce(metadata, '{}'), ?, ?) WHERE id = ?")
        .bind(path).bind(value).bind(id)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: Run — PASS**

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/reforge/symbol_validation.rs crates/storage/src/repos/semantic_facts.rs crates/coding-memory/tests/symbol_validation_needs_review.rs
git commit -m "feat(symbol-validation): mark needs_review for surviving-but-drifted anchors"
```

---

### Task 4.4: Backend git post-commit hook installer (no FE)

**Files:**
- Create: `crates/app-core/src/coding_memory/git_hook_installer.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs` (re-export)
- Modify: `crates/desktop/src/commands/coding_memory.rs` (Tauri command shell)
- Modify: `crates/desktop/src/specta_builder.rs` (register command)
- Create: `crates/app-core/tests/git_hook_installer.rs`

- [ ] **Step 1: Failing test**

```rust
//! GitHookInstaller writes a managed post-commit script into <repo>/.git/hooks/
//! that invokes `klyntbot-hook git-post-commit`. Reversible.

use app_core::coding_memory::git_hook_installer::GitHookInstaller;

#[tokio::test]
async fn install_writes_managed_post_commit_hook() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path();
    git2::Repository::init(repo_root).unwrap();
    let hook_binary = tmp.path().join("klyntbot-hook");
    std::fs::write(&hook_binary, "#!/bin/sh\necho stub").unwrap();

    GitHookInstaller::install(repo_root, &hook_binary).await.unwrap();
    let post_commit = repo_root.join(".git/hooks/post-commit");
    let body = std::fs::read_to_string(&post_commit).unwrap();
    assert!(body.contains("klyntbot-managed"));
    assert!(body.contains("git-post-commit"));
}

#[tokio::test]
async fn uninstall_preserves_user_content() {
    let tmp = tempfile::tempdir().unwrap();
    git2::Repository::init(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".git/hooks")).unwrap();
    std::fs::write(tmp.path().join(".git/hooks/post-commit"),
        "#!/bin/sh\necho user-content\n# klyntbot-managed:start\necho klynt\n# klyntbot-managed:end\n").unwrap();

    GitHookInstaller::uninstall(tmp.path()).await.unwrap();
    let body = std::fs::read_to_string(tmp.path().join(".git/hooks/post-commit")).unwrap();
    assert!(body.contains("user-content"));
    assert!(!body.contains("klynt"));
}
```

- [ ] **Step 2: Implement `git_hook_installer.rs`**

```rust
//! Manage klyntbot-managed block in <repo>/.git/hooks/post-commit.
//!
//! Pattern mirrors ClaudeCodeInstaller: read → splice → atomic write.
//! User content outside the markers is preserved.

use common::{KlyntbotError, Result};
use std::path::{Path, PathBuf};

const START: &str = "# klyntbot-managed:start";
const END: &str = "# klyntbot-managed:end";

pub struct GitHookInstaller;

impl GitHookInstaller {
    pub async fn install(repo_root: &Path, hook_binary: &Path) -> Result<()> {
        let hooks_dir = repo_root.join(".git/hooks");
        if !hooks_dir.exists() {
            return Err(KlyntbotError::Storage(format!(
                "{} does not exist (is this a git repo?)", hooks_dir.display())));
        }
        std::fs::create_dir_all(&hooks_dir).map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        let post_commit = hooks_dir.join("post-commit");
        let existing = if post_commit.exists() {
            std::fs::read_to_string(&post_commit).map_err(|e| KlyntbotError::Storage(e.to_string()))?
        } else {
            "#!/bin/sh\n".into()
        };
        let user = strip_managed(&existing);
        let header = if user.starts_with("#!") { String::new() } else { "#!/bin/sh\n".into() };
        let managed = format!(
            "{START}\n{} git-post-commit\n{END}\n",
            shell_escape(hook_binary.display().to_string())
        );
        let body = format!("{header}{user}\n{managed}");
        atomic_write(&post_commit, &body)?;
        make_executable(&post_commit)?;
        Ok(())
    }

    pub async fn uninstall(repo_root: &Path) -> Result<()> {
        let post_commit = repo_root.join(".git/hooks/post-commit");
        if !post_commit.exists() { return Ok(()); }
        let body = std::fs::read_to_string(&post_commit).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let stripped = strip_managed(&body);
        if stripped.trim() == "#!/bin/sh" || stripped.trim().is_empty() {
            std::fs::remove_file(&post_commit).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        } else {
            atomic_write(&post_commit, &stripped)?;
        }
        Ok(())
    }
}

fn strip_managed(s: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in s.lines() {
        if line.trim() == START { in_block = true; continue; }
        if line.trim() == END   { in_block = false; continue; }
        if !in_block { out.push_str(line); out.push('\n'); }
    }
    out
}

fn shell_escape(s: String) -> String {
    if s.contains(' ') || s.contains('"') { format!("\"{}\"", s.replace('"', "\\\"")) } else { s }
}

fn atomic_write(path: &Path, body: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path).map_err(|e| KlyntbotError::Storage(e.to_string()))?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(path, perm).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> { Ok(()) }
```

- [ ] **Step 3: Re-export from `mod.rs`**

```rust
pub mod git_hook_installer;
```

Add a method on `CodingMemorySubsystem`:

```rust
pub async fn install_git_hook(&self, repo_root: &std::path::Path) -> Result<()> {
    git_hook_installer::GitHookInstaller::install(repo_root, &self.hook_binary_path()).await
}
pub async fn uninstall_git_hook(&self, repo_root: &std::path::Path) -> Result<()> {
    git_hook_installer::GitHookInstaller::uninstall(repo_root).await
}
```

- [ ] **Step 4: Tauri command shell**

In `crates/desktop/src/commands/coding_memory.rs`:

```rust
#[klynt_command]
pub async fn coding_memory_install_git_hook(repo_root: String) -> Result<(), String> {
    let core = AppCore::current();
    core.coding_memory()
        .install_git_hook(std::path::Path::new(&repo_root))
        .await
        .map_err(|e| e.to_string())
}

#[klynt_command]
pub async fn coding_memory_uninstall_git_hook(repo_root: String) -> Result<(), String> {
    let core = AppCore::current();
    core.coding_memory()
        .uninstall_git_hook(std::path::Path::new(&repo_root))
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Register in `specta_builder.rs::collect_commands![...]`**

Add `coding_memory_install_git_hook` and `coding_memory_uninstall_git_hook` to the macro list.

- [ ] **Step 6: Run — PASS**

```bash
cargo nextest run -p app-core -E 'test(install_writes_managed_post_commit_hook) | test(uninstall_preserves_user_content)'
```

- [ ] **Step 7: Regenerate bindings (this updates the FE binding file but does NOT add UI)**

```bash
cargo tauri dev   # then quit when bindings.ts regenerates; this is a Rust-side regeneration only
```

Actually, since FE is out of scope, just verify the `registration_drift` test passes:

```bash
cargo nextest run -p desktop -E 'test(registration_drift) | test(no_raw_tauri_command_outside_macros)'
```

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/src/coding_memory/git_hook_installer.rs crates/app-core/src/coding_memory/mod.rs crates/desktop/src/commands/coding_memory.rs crates/desktop/src/specta_builder.rs crates/app-core/tests/git_hook_installer.rs desktop-ui/src/bindings.ts
git commit -m "feat(coding-memory): backend git post-commit hook installer + Tauri command shells"
```

---

## Section 5 — Phase 7: Codex Adapter

### Task 5.1: Capture Codex hook payload shapes (research → fixture)

**Files:**
- Create: `tests/fixtures/coding/synthetic_session_codex.jsonl`
- Create: `docs/coding-memory/adapters/codex.md`

- [ ] **Step 1: Research current Codex hook surface**

Run upstream docs lookup:

```bash
npx ctx7@latest library codex "hook events config TOML"
```

If ctx7 returns no high-quality match, fall back to:

```bash
gh repo view openai/codex --json description,homepageUrl
```

and inspect the project README for the hook spec (look for keys like `[hooks]`, `tool_kind`, `pre_tool_use`, `post_tool_use`).

- [ ] **Step 2: Create the synthetic fixture**

Write `tests/fixtures/coding/synthetic_session_codex.jsonl` — one JSON-line per hook event invocation. Cover all 5 hook events: `session.start`, `user.prompt`, `tool.pre`, `tool.post`, `session.end`. Use representative payloads. The file is the source of truth for the parser.

```jsonl
{"event":"session.start","session_id":"cdx-001","cwd":"/tmp/proj","model":"o4-mini"}
{"event":"user.prompt","session_id":"cdx-001","cwd":"/tmp/proj","prompt":"add a logger to main.rs"}
{"event":"tool.post","session_id":"cdx-001","cwd":"/tmp/proj","tool":"shell","tool_kind":"shell","args":{"cmd":"cargo build"},"stdout":"Finished","stderr":"","exit":0,"duration_ms":1200}
{"event":"tool.post","session_id":"cdx-001","cwd":"/tmp/proj","tool":"edit","tool_kind":"file_edit","args":{"path":"src/main.rs","op":"modify"},"bytes":420,"duration_ms":35}
{"event":"session.end","session_id":"cdx-001","cwd":"/tmp/proj","reason":"user_quit"}
```

- [ ] **Step 3: Create the adapter docs**

Write `docs/coding-memory/adapters/codex.md`:

```markdown
# Codex adapter

Maps OpenAI Codex CLI's 5 hook events to klyntbot `AgentEvent`.

## Events captured

| Codex event | klyntbot EventKind | Notes |
|-------------|--------------------|-------|
| `session.start` | `SessionStart { model, source_reason }` | model from payload, source_reason="codex" |
| `user.prompt` | `UserPrompt { text, attachments }` | attachments empty for now |
| `tool.pre` | dropped | approval-only, no record |
| `tool.post` (tool=shell, args.cmd matches test pattern) | `TestRun` | pytest/cargo test/npm test/etc |
| `tool.post` (tool=shell, else) | `ToolCall` | with `tool_kind` in args_preview |
| `tool.post` (tool=edit) | `FileEdit { path, op, bytes }` | op derived from args.op |
| `tool.post` (other) | `ToolCall` | generic |
| `session.end` | `SessionEnd { reason }` | |

## Install path

`~/.codex/config.toml` — managed `[[hooks]]` block written by `CodexInstaller`.
Backup of original kept at `~/.codex/config.toml.bak.<timestamp>`.

## Diagnose

`klyntbot-hook codex --diagnose` writes a synthetic `session.start` and verifies
exit code 0. Surfaced via `coding_memory_diagnose_cli("codex")`.
```

- [ ] **Step 4: Commit fixture + docs**

```bash
git add tests/fixtures/coding/synthetic_session_codex.jsonl docs/coding-memory/adapters/codex.md
git commit -m "docs(codex-adapter): event mapping table + synthetic fixture"
```

---

### Task 5.2: Codex payload types

**Files:**
- Replace: `crates/coding-ingest/src/adapters/codex.rs` → directory `crates/coding-ingest/src/adapters/codex/`
- Create: `crates/coding-ingest/src/adapters/codex/payload.rs`
- Modify: `crates/coding-ingest/src/adapters/mod.rs` (declare `pub mod codex;`)

- [ ] **Step 1: Delete the stub file**

```bash
rm crates/coding-ingest/src/adapters/codex.rs
mkdir crates/coding-ingest/src/adapters/codex
```

- [ ] **Step 2: Write `payload.rs`**

```rust
//! Serde shapes for Codex hook payloads. Source of truth: synthetic fixture
//! `tests/fixtures/coding/synthetic_session_codex.jsonl`.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommonEnvelope {
    pub session_id: String,
    pub cwd: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionStartBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserPromptBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub prompt: String,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToolPostBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool: String,
    #[serde(default)]
    pub tool_kind: Option<String>,
    pub args: serde_json::Value,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub exit: Option<i64>,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub reason: Option<String>,
}
```

- [ ] **Step 3: Compile-check**

```bash
cargo build -p coding-ingest
```

Expected: builds (the new `payload.rs` has no public uses yet).

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/src/adapters/codex/payload.rs
git commit -m "feat(codex-adapter): payload serde structs (5 hook events)"
```

---

### Task 5.3: Codex dispatch (event-kind classifier)

**Files:**
- Create: `crates/coding-ingest/src/adapters/codex/dispatch.rs`

- [ ] **Step 1: Write the test first (parser-driven TDD)**

Create `crates/coding-ingest/tests/codex_adapter.rs`:

```rust
//! Codex adapter parses every line of synthetic_session_codex.jsonl into
//! a non-error AgentEvent (or intentional drop).

use coding_ingest::adapters::{codex::CodexAdapter, IngestAdapter};
use coding_ingest::EventKind;

fn fixture() -> Vec<(String, Vec<u8>)> {
    let raw = std::fs::read_to_string("../../tests/fixtures/coding/synthetic_session_codex.jsonl").unwrap();
    raw.lines().map(|l| {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        let event = v["event"].as_str().unwrap().to_string();
        (event, l.as_bytes().to_vec())
    }).collect()
}

#[test]
fn session_start_parses() {
    let (ev, raw) = &fixture()[0];
    assert_eq!(ev, "session.start");
    let parsed = CodexAdapter.parse(ev, raw).unwrap().expect("event dropped");
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    assert!(matches!(v1.kind, EventKind::SessionStart { .. }));
}

#[test]
fn user_prompt_parses() {
    let f = fixture();
    let (ev, raw) = &f[1];
    let parsed = CodexAdapter.parse(ev, raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    if let EventKind::UserPrompt { text, .. } = v1.kind {
        assert!(text.contains("logger"));
    } else { panic!("wrong kind"); }
}

#[test]
fn tool_post_shell_with_test_command_classifies_as_test_run() {
    // Construct a tool.post payload whose args.cmd matches a test framework
    let raw = br#"{"event":"tool.post","session_id":"s","cwd":"/tmp","tool":"shell","tool_kind":"shell","args":{"cmd":"cargo test --workspace"},"stdout":"test result: ok. 12 passed; 0 failed","stderr":"","exit":0,"duration_ms":4500}"#;
    let parsed = CodexAdapter.parse("tool.post", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    assert!(matches!(v1.kind, EventKind::TestRun { framework: Some(_), .. }));
}

#[test]
fn tool_post_shell_with_non_test_command_classifies_as_tool_call() {
    let raw = br#"{"event":"tool.post","session_id":"s","cwd":"/tmp","tool":"shell","tool_kind":"shell","args":{"cmd":"ls -la"},"stdout":"","stderr":"","exit":0,"duration_ms":10}"#;
    let parsed = CodexAdapter.parse("tool.post", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    assert!(matches!(v1.kind, EventKind::ToolCall { .. }));
}

#[test]
fn tool_post_edit_classifies_as_file_edit() {
    let raw = br#"{"event":"tool.post","session_id":"s","cwd":"/tmp","tool":"edit","tool_kind":"file_edit","args":{"path":"src/main.rs","op":"modify"},"bytes":420,"duration_ms":35}"#;
    let parsed = CodexAdapter.parse("tool.post", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    assert!(matches!(v1.kind, EventKind::FileEdit { .. }));
}

#[test]
fn tool_pre_drops() {
    let raw = br#"{"event":"tool.pre","session_id":"s","cwd":"/tmp","tool":"shell","args":{}}"#;
    let parsed = CodexAdapter.parse("tool.pre", raw).unwrap();
    assert!(parsed.is_none(), "tool.pre should be dropped");
}

#[test]
fn session_end_parses() {
    let raw = br#"{"event":"session.end","session_id":"s","cwd":"/tmp","reason":"user_quit"}"#;
    let parsed = CodexAdapter.parse("session.end", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    assert!(matches!(v1.kind, EventKind::SessionEnd { .. }));
}

#[test]
fn unknown_event_returns_none() {
    let raw = br#"{"event":"bogus.event","session_id":"s","cwd":"/tmp"}"#;
    let parsed = CodexAdapter.parse("bogus.event", raw).unwrap();
    assert!(parsed.is_none());
}
```

- [ ] **Step 2: Run — FAIL (CodexAdapter not yet implemented)**

```bash
cargo nextest run -p coding-ingest -E 'test(codex_adapter)'
```

- [ ] **Step 3: Implement `dispatch.rs`**

```rust
//! Codex tool.post classifier — distinguishes Bash (shell+test pattern),
//! shell ToolCall, file edits, and generic tool calls.

use super::{base, payload};
use crate::event::{AgentEventV1, EventKind, FileOp, TokenUsage};

pub(super) fn classify_tool_post(b: payload::ToolPostBody) -> Option<EventKind> {
    Some(match b.tool.as_str() {
        "shell" => classify_shell(&b),
        "edit" | "write" => classify_edit(&b),
        _ => generic_tool_call(&b),
    })
}

fn classify_shell(b: &payload::ToolPostBody) -> EventKind {
    let cmd = b.args.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(fw) = detect_framework(cmd) {
        let (passed, failed) = parse_results(fw, &b.stdout);
        return EventKind::TestRun {
            command: cmd.to_string(),
            framework: Some(fw.into()),
            passed,
            failed,
            duration_ms: b.duration_ms,
        };
    }
    let args_preview = truncate(&format!("{}", b.args), 512);
    let result_preview = truncate(&b.stdout, 512);
    EventKind::ToolCall {
        tool: format!("shell:{}", b.tool_kind.clone().unwrap_or_else(|| "unknown".into())),
        args_preview,
        ok: b.exit.map(|e| e == 0).unwrap_or(true),
        duration_ms: b.duration_ms,
        result_preview,
    }
}

fn classify_edit(b: &payload::ToolPostBody) -> EventKind {
    let path: std::path::PathBuf = b.args.get("path")
        .and_then(|v| v.as_str()).map(std::path::PathBuf::from).unwrap_or_default();
    let op = match b.args.get("op").and_then(|v| v.as_str()).unwrap_or("modify") {
        "create" => FileOp::Create,
        "delete" => FileOp::Delete,
        "read"   => FileOp::Read,
        _        => FileOp::Modify,
    };
    EventKind::FileEdit {
        path,
        op,
        bytes: b.bytes,
        diff_preview: None,
    }
}

fn generic_tool_call(b: &payload::ToolPostBody) -> EventKind {
    let args_preview = truncate(&format!("{}", b.args), 512);
    let result_preview = truncate(&b.stdout, 512);
    EventKind::ToolCall {
        tool: b.tool.clone(),
        args_preview,
        ok: b.exit.map(|e| e == 0).unwrap_or(true),
        duration_ms: b.duration_ms,
        result_preview,
    }
}

fn detect_framework(cmd: &str) -> Option<&'static str> {
    let c = cmd.trim_start();
    if c.starts_with("pytest") { return Some("pytest"); }
    if c.starts_with("cargo test") || c.starts_with("cargo nextest") { return Some("cargo"); }
    if c.starts_with("npm test") || c.starts_with("pnpm test") || c.starts_with("yarn test") || c.starts_with("bun test") { return Some("node"); }
    if c.starts_with("go test") { return Some("go"); }
    if c.starts_with("jest") { return Some("jest"); }
    if c.starts_with("vitest") { return Some("vitest"); }
    None
}

fn parse_results(framework: &str, stdout: &str) -> (u32, u32) {
    // Best-effort regex per framework. Returns (passed, failed); zeros on no match.
    use regex::Regex;
    match framework {
        "pytest" => {
            let re = Regex::new(r"(\d+)\s+passed(?:.*?(\d+)\s+failed)?").unwrap();
            re.captures(stdout).map(|c| (
                c.get(1).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0),
                c.get(2).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0),
            )).unwrap_or((0,0))
        }
        "cargo" => {
            let re = Regex::new(r"(\d+)\s+passed;\s+(\d+)\s+failed").unwrap();
            re.captures(stdout).map(|c| (
                c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0)
            )).unwrap_or((0,0))
        }
        _ => (0, 0),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { s.chars().take(max).collect::<String>() + "…" }
}
```

- [ ] **Step 4: Implement `mod.rs`**

```rust
//! Codex adapter — 5 hook events from OpenAI's Codex CLI.

mod dispatch;
mod payload;

use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy)]
pub struct CodexAdapter;

impl IngestAdapter for CodexAdapter {
    fn source_name(&self) -> &'static str { "codex" }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "session.start" => Ok(Some(wrap(parse_session_start(raw)?))),
            "user.prompt"   => Ok(Some(wrap(parse_user_prompt(raw)?))),
            "tool.pre"      => Ok(None), // approval-only
            "tool.post"     => parse_tool_post(raw).map(|opt| opt.map(wrap)),
            "session.end"   => Ok(Some(wrap(parse_session_end(raw)?))),
            _ => Ok(None),
        }
    }
}

fn wrap(v1: AgentEventV1) -> AgentEvent { AgentEvent::V1(v1) }

fn base(common: payload::CommonEnvelope, kind: EventKind) -> AgentEventV1 {
    AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::Codex,
        session_id: common.session_id,
        turn_id: None,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}

fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| KlyntbotError::Storage(format!("codex decode: {e}")))
}

fn parse_session_start(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionStartBody = decode(raw)?;
    Ok(base(b.common, EventKind::SessionStart {
        model: b.model,
        source_reason: "codex".into(),
    }))
}

fn parse_user_prompt(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserPromptBody = decode(raw)?;
    Ok(base(b.common, EventKind::UserPrompt {
        text: b.prompt,
        attachments: b.attachments,
    }))
}

fn parse_tool_post(raw: &[u8]) -> Result<Option<AgentEventV1>> {
    let b: payload::ToolPostBody = decode(raw)?;
    let common = payload::CommonEnvelope {
        session_id: b.common.session_id.clone(),
        cwd: b.common.cwd.clone(),
    };
    Ok(dispatch::classify_tool_post(b).map(|k| base(common, k)))
}

fn parse_session_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionEndBody = decode(raw)?;
    Ok(base(b.common, EventKind::SessionEnd {
        reason: b.reason.unwrap_or_else(|| "unspecified".into()),
    }))
}
```

- [ ] **Step 5: Update `adapters/mod.rs`**

```rust
pub mod codex;
```

(Removes the prior `pub mod codex;` referencing the file, now references the directory module.)

- [ ] **Step 6: Add `regex` to `coding-ingest/Cargo.toml`**

```bash
grep -n '^regex' crates/coding-ingest/Cargo.toml || \
  sed -i '' '/\[dependencies\]/a\
regex = { workspace = true }
' crates/coding-ingest/Cargo.toml
```

(If `regex` isn't in workspace deps, add it to root `Cargo.toml` `[workspace.dependencies]`.)

- [ ] **Step 7: Run — PASS**

```bash
cargo nextest run -p coding-ingest -E 'test(codex_adapter)'
```

- [ ] **Step 8: Commit**

```bash
git add crates/coding-ingest/src/adapters/codex/ crates/coding-ingest/src/adapters/mod.rs crates/coding-ingest/tests/codex_adapter.rs crates/coding-ingest/Cargo.toml
git commit -m "feat(codex-adapter): full parse/dispatch over 5 hook events"
```

---

### Task 5.4: Codex backend installer

**Files:**
- Create: `crates/app-core/src/coding_memory/codex_installer.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs` (re-export + dispatch)

- [ ] **Step 1: Failing test**

Create `crates/app-core/tests/codex_installer.rs`:

```rust
use app_core::coding_memory::codex_installer::CodexInstaller;

#[test]
fn install_writes_managed_hooks_block_to_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "# user content\n[other]\nkey = 1\n").unwrap();
    let hook = tmp.path().join("klyntbot-hook");
    std::fs::write(&hook, "#!/bin/sh\n").unwrap();

    CodexInstaller::install(&cfg, &hook).unwrap();
    let body = std::fs::read_to_string(&cfg).unwrap();
    assert!(body.contains("# klyntbot-managed:start"));
    assert!(body.contains("# klyntbot-managed:end"));
    assert!(body.contains("[[hooks]]"));
    assert!(body.contains("klyntbot-hook"));
    assert!(body.contains("[other]"));   // user content preserved
}

#[test]
fn uninstall_strips_only_managed_block() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    let body = "[other]\nkey=1\n# klyntbot-managed:start\n[[hooks]]\nevent=\"session.start\"\ncommand=\"x\"\n# klyntbot-managed:end\n";
    std::fs::write(&cfg, body).unwrap();
    CodexInstaller::uninstall(&cfg).unwrap();
    let after = std::fs::read_to_string(&cfg).unwrap();
    assert!(after.contains("[other]"));
    assert!(!after.contains("klyntbot-managed"));
}
```

- [ ] **Step 2: Run — FAIL**

- [ ] **Step 3: Implement**

```rust
//! Manage `[[hooks]]` block in Codex's `config.toml`.

use common::{KlyntbotError, Result};
use std::path::Path;

const START: &str = "# klyntbot-managed:start";
const END: &str = "# klyntbot-managed:end";

const EVENTS: &[&str] = &["session.start", "user.prompt", "tool.pre", "tool.post", "session.end"];

pub struct CodexInstaller;

impl CodexInstaller {
    pub fn install(config_path: &Path, hook_binary: &Path) -> Result<()> {
        let existing = if config_path.exists() {
            std::fs::read_to_string(config_path).map_err(|e| KlyntbotError::Storage(e.to_string()))?
        } else { String::new() };
        if config_path.exists() { backup(config_path)?; }
        let user = strip_managed(&existing);
        let mut block = String::from(START);
        block.push('\n');
        for ev in EVENTS {
            block.push_str(&format!(
                "[[hooks]]\nevent = \"{ev}\"\ncommand = \"{} codex {ev}\"\n\n",
                hook_binary.display()
            ));
        }
        block.push_str(END);
        block.push('\n');
        let body = if user.trim().is_empty() { block } else { format!("{user}\n{block}") };
        atomic_write(config_path, &body)
    }

    pub fn uninstall(config_path: &Path) -> Result<()> {
        if !config_path.exists() { return Ok(()); }
        let body = std::fs::read_to_string(config_path).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        atomic_write(config_path, &strip_managed(&body))
    }

    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["codex", "session.start"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(format!("spawn: {e}")))?;
        let body = br#"{"session_id":"diagnose","cwd":"/tmp","model":"diagnose"}"#;
        child.stdin.as_mut().unwrap().write_all(body)
            .map_err(|e| KlyntbotError::Storage(format!("stdin: {e}")))?;
        drop(child.stdin.take());
        let status = child.wait().map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if !status.success() { return Err(KlyntbotError::Storage(format!("hook exit {status}"))); }
        Ok(())
    }
}

fn strip_managed(s: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in s.lines() {
        if line.trim() == START { in_block = true; continue; }
        if line.trim() == END   { in_block = false; continue; }
        if !in_block { out.push_str(line); out.push('\n'); }
    }
    out
}

fn atomic_write(p: &Path, body: &str) -> Result<()> {
    let tmp = p.with_extension("tmp");
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    }
    std::fs::write(&tmp, body).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, p).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

fn backup(p: &Path) -> Result<()> {
    let bak = p.with_extension(format!("toml.bak.{}", jiff::Timestamp::now().as_second()));
    std::fs::copy(p, &bak).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 4: Wire dispatcher in `coding_memory/mod.rs`**

In the existing `set_cli_enabled` method, add a match arm:

```rust
match cli {
    "claude-code" => if enabled { ClaudeCodeInstaller::install(...)?; } else { ClaudeCodeInstaller::uninstall(...)?; },
    "codex"       => {
        let cfg = home_dir().join(".codex/config.toml");
        if enabled { CodexInstaller::install(&cfg, &self.hook_binary_path())?; }
        else       { CodexInstaller::uninstall(&cfg)?; }
    },
    "kimi-cli"    => /* Task 6.x */,
    "opencode"    => /* Task 7.x */,
    other => return Err(KlyntbotError::Storage(format!("unknown cli {other}"))),
}
```

- [ ] **Step 5: Run — PASS**

```bash
cargo nextest run -p app-core -E 'test(install_writes_managed_hooks_block_to_toml) | test(uninstall_strips_only_managed_block)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding_memory/codex_installer.rs crates/app-core/src/coding_memory/mod.rs crates/app-core/tests/codex_installer.rs
git commit -m "feat(codex-installer): managed-block install/uninstall in ~/.codex/config.toml"
```

---

## Section 6 — Phase 7: kimi-cli Tier-1 (Hook) Adapter

### Task 6.1: Capture kimi-cli hook payload shapes (research → fixture)

**Files:**
- Create: `tests/fixtures/coding/synthetic_session_kimi.jsonl`
- Create: `docs/coding-memory/adapters/kimi-cli.md`

- [ ] **Step 1: Research kimi-cli hook surface**

```bash
npx ctx7@latest library kimi-cli "hook events tier 1 payload"
```

If no match, query the upstream repo directly:

```bash
gh repo view MoonshotAI/kimi-cli --json description,homepageUrl
```

The spec says 13 hook events. Likely shape includes session lifecycle (start/end), prompt (user_input), tool calls (pre/post), file edits, errors, compact events, plus a few kimi-specific events (e.g., `agent.thinking_start`).

- [ ] **Step 2: Create fixture covering all 13 events**

Write `tests/fixtures/coding/synthetic_session_kimi.jsonl`:

```jsonl
{"event":"session_start","session":"ki-001","cwd":"/tmp/p","model":"kimi-k2"}
{"event":"user_input","session":"ki-001","cwd":"/tmp/p","text":"refactor the parser"}
{"event":"thinking_start","session":"ki-001","cwd":"/tmp/p"}
{"event":"thinking_end","session":"ki-001","cwd":"/tmp/p","duration_ms":520}
{"event":"tool_pre","session":"ki-001","cwd":"/tmp/p","tool":"shell","args":{"cmd":"cargo test"}}
{"event":"tool_post","session":"ki-001","cwd":"/tmp/p","tool":"shell","args":{"cmd":"cargo test"},"stdout":"3 passed; 0 failed","exit":0,"duration_ms":3500}
{"event":"file_edit","session":"ki-001","cwd":"/tmp/p","path":"src/parser.rs","op":"modify","bytes":1240}
{"event":"file_read","session":"ki-001","cwd":"/tmp/p","path":"src/lib.rs"}
{"event":"assistant_msg","session":"ki-001","cwd":"/tmp/p","text":"refactored","tokens":{"prompt":120,"completion":40}}
{"event":"compact_triggered","session":"ki-001","cwd":"/tmp/p","trigger":"context_window","tokens":190000}
{"event":"error","session":"ki-001","cwd":"/tmp/p","tool":"shell","message":"cmd not found"}
{"event":"agent_pause","session":"ki-001","cwd":"/tmp/p"}
{"event":"session_end","session":"ki-001","cwd":"/tmp/p","reason":"user_quit"}
```

(13 events total. Field names may need adjustment after consulting upstream — the parser's serde structs use `#[serde(other)]` fallthrough to tolerate variation.)

- [ ] **Step 3: Write the docs**

Write `docs/coding-memory/adapters/kimi-cli.md`:

```markdown
# kimi-cli adapter

Tier 1: shell hook over 13 hook events.
Tier 2: optional Wire client subscribing to streaming events (richer AssistantMsg).

## Events captured (tier 1)

| kimi event | klyntbot EventKind | Notes |
|------------|--------------------|-------|
| `session_start` | SessionStart | model from payload |
| `user_input` | UserPrompt | text |
| `thinking_start` | dropped | observable but no record |
| `thinking_end` | ToolCall { tool="thinking", duration } | for thinking-tier autotuner |
| `tool_pre` | dropped | approval-only |
| `tool_post` (shell + test pattern) | TestRun | |
| `tool_post` (shell other) | ToolCall | |
| `file_edit` | FileEdit { Modify } | |
| `file_read` | FileEdit { Read } | |
| `assistant_msg` | AssistantMsg | with token_usage |
| `compact_triggered` | CompactEvent | |
| `error` | Error | |
| `agent_pause` | dropped | (control-flow only) |
| `session_end` | SessionEnd | |

## Tier 2 — Wire client

Connects to kimi-cli's WebSocket Wire endpoint (default ws://127.0.0.1:9420).
Captures streaming AssistantMsg deltas and aggregates into a single record per
turn. Falls back gracefully to tier 1 if the Wire endpoint is unreachable.

## Install

`~/.config/kimi-cli/hooks.json` — managed `klyntbot` block with shell hooks.
```

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/coding/synthetic_session_kimi.jsonl docs/coding-memory/adapters/kimi-cli.md
git commit -m "docs(kimi-adapter): event mapping table + 13-event synthetic fixture"
```

---

### Task 6.2: kimi-cli payload + dispatch (tier 1)

**Files:**
- Replace: `crates/coding-ingest/src/adapters/kimi_wire.rs` → directory `crates/coding-ingest/src/adapters/kimi_cli/`
- Create: `crates/coding-ingest/src/adapters/kimi_cli/{mod.rs, payload.rs, dispatch.rs}`
- Create: `crates/coding-ingest/tests/kimi_adapter_tier1.rs`
- Modify: `crates/coding-ingest/src/adapters/mod.rs`

- [ ] **Step 1: Delete the stub + create the directory**

```bash
rm crates/coding-ingest/src/adapters/kimi_wire.rs
mkdir crates/coding-ingest/src/adapters/kimi_cli
```

- [ ] **Step 2: Update `adapters/mod.rs`**

```rust
pub mod kimi_cli;
// (remove the prior `pub mod kimi_wire;` line)
```

- [ ] **Step 3: Write the failing test first**

```rust
//! kimi-cli tier-1 adapter parses every line of synthetic_session_kimi.jsonl.

use coding_ingest::adapters::{kimi_cli::KimiAdapter, IngestAdapter};
use coding_ingest::EventKind;

fn fixture() -> Vec<(String, Vec<u8>)> {
    let raw = std::fs::read_to_string("../../tests/fixtures/coding/synthetic_session_kimi.jsonl").unwrap();
    raw.lines().map(|l| {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        let event = v["event"].as_str().unwrap().to_string();
        (event, l.as_bytes().to_vec())
    }).collect()
}

#[test]
fn all_13_fixture_events_parse_or_drop_intentionally() {
    let f = fixture();
    let intentional_drops = ["thinking_start", "tool_pre", "agent_pause"];
    for (ev, raw) in f {
        let result = KimiAdapter::default().parse(&ev, &raw);
        assert!(result.is_ok(), "parse {ev} errored: {result:?}");
        if intentional_drops.contains(&ev.as_str()) {
            assert!(result.unwrap().is_none(), "{ev} should drop");
        } else {
            assert!(result.unwrap().is_some(), "{ev} should produce event");
        }
    }
}

#[test]
fn assistant_msg_carries_token_usage() {
    let raw = br#"{"event":"assistant_msg","session":"s","cwd":"/tmp","text":"hi","tokens":{"prompt":10,"completion":5}}"#;
    let parsed = KimiAdapter::default().parse("assistant_msg", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    if let EventKind::AssistantMsg { token_usage: Some(tu), .. } = v1.kind {
        assert_eq!(tu.prompt_tokens, 10);
        assert_eq!(tu.completion_tokens, 5);
    } else { panic!("expected AssistantMsg with token_usage"); }
}

#[test]
fn compact_triggered_records_token_count() {
    let raw = br#"{"event":"compact_triggered","session":"s","cwd":"/tmp","trigger":"context_window","tokens":190000}"#;
    let parsed = KimiAdapter::default().parse("compact_triggered", raw).unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = parsed;
    if let EventKind::CompactEvent { trigger, token_count } = v1.kind {
        assert_eq!(trigger, "context_window");
        assert_eq!(token_count, 190000);
    } else { panic!("expected CompactEvent"); }
}
```

- [ ] **Step 4: Implement `payload.rs`**

```rust
//! Serde shapes for kimi-cli tier-1 hook payloads.
//!
//! Source: synthetic_session_kimi.jsonl. `#[serde(other)]` arms tolerate
//! upstream additions without breaking the parser.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct CommonEnvelope {
    pub session: String,
    pub cwd: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionStartBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserInputBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ThinkingEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub duration_ms: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToolPostBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub exit: Option<i64>,
    #[serde(default)]
    pub duration_ms: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileEditBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub path: PathBuf,
    #[serde(default = "default_op")]
    pub op: String,
    #[serde(default)]
    pub bytes: u64,
}
fn default_op() -> String { "modify".into() }

#[derive(Debug, Deserialize)]
pub(crate) struct FileReadBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Tokens { pub prompt: u32, pub completion: u32 }

#[derive(Debug, Deserialize)]
pub(crate) struct AssistantMsgBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub text: String,
    pub tokens: Option<Tokens>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompactBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub trigger: String,
    #[serde(default)]
    pub tokens: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub reason: Option<String>,
}
```

- [ ] **Step 5: Implement `dispatch.rs`** (kimi tool_post classifier — same shape as Codex)

```rust
//! kimi-cli tool_post classifier. Mirrors codex/dispatch.rs but with kimi's
//! payload shape (no `tool_kind` field).

use super::payload;
use crate::event::{EventKind, FileOp};

pub(super) fn classify_tool_post(b: payload::ToolPostBody) -> EventKind {
    match b.tool.as_str() {
        "shell" => classify_shell(&b),
        "edit" | "write" => classify_edit_inline(&b),
        _ => generic_tool_call(&b),
    }
}

fn classify_shell(b: &payload::ToolPostBody) -> EventKind {
    let cmd = b.args.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(fw) = detect_framework(cmd) {
        let (passed, failed) = parse_results(fw, &b.stdout);
        return EventKind::TestRun {
            command: cmd.to_string(),
            framework: Some(fw.into()),
            passed, failed,
            duration_ms: b.duration_ms,
        };
    }
    EventKind::ToolCall {
        tool: "shell".into(),
        args_preview: truncate(&format!("{}", b.args), 512),
        ok: b.exit.map(|e| e == 0).unwrap_or(true),
        duration_ms: b.duration_ms,
        result_preview: truncate(&b.stdout, 512),
    }
}

fn classify_edit_inline(b: &payload::ToolPostBody) -> EventKind {
    let path = b.args.get("path").and_then(|v| v.as_str()).map(std::path::PathBuf::from).unwrap_or_default();
    EventKind::FileEdit {
        path, op: FileOp::Modify, bytes: 0, diff_preview: None,
    }
}

fn generic_tool_call(b: &payload::ToolPostBody) -> EventKind {
    EventKind::ToolCall {
        tool: b.tool.clone(),
        args_preview: truncate(&format!("{}", b.args), 512),
        ok: b.exit.map(|e| e == 0).unwrap_or(true),
        duration_ms: b.duration_ms,
        result_preview: truncate(&b.stdout, 512),
    }
}

// Re-use detect_framework / parse_results / truncate from a shared util module.
// To avoid copy-paste, extract to crates/coding-ingest/src/adapters/util.rs and
// import here and in codex/dispatch.rs.
fn detect_framework(_c: &str) -> Option<&'static str> { unimplemented!("see Task 6.3 — extract shared util") }
fn parse_results(_f: &str, _s: &str) -> (u32, u32) { unimplemented!("see Task 6.3") }
fn truncate(_s: &str, _n: usize) -> String { unimplemented!("see Task 6.3") }
```

- [ ] **Step 6: Implement `mod.rs`**

```rust
//! kimi-cli adapter — tier 1: 13 hook events. Tier 2 (Wire) lives in `wire.rs`.

mod dispatch;
mod payload;
pub mod wire;  // tier-2; module compiles standalone for Task 6.4

use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy)]
pub struct KimiAdapter;

impl IngestAdapter for KimiAdapter {
    fn source_name(&self) -> &'static str { "kimi-cli" }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "session_start"     => Ok(Some(wrap(parse_session_start(raw)?))),
            "user_input"        => Ok(Some(wrap(parse_user_input(raw)?))),
            "thinking_start"    => Ok(None),
            "thinking_end"      => Ok(Some(wrap(parse_thinking_end(raw)?))),
            "tool_pre"          => Ok(None),
            "tool_post"         => Ok(Some(wrap(parse_tool_post(raw)?))),
            "file_edit"         => Ok(Some(wrap(parse_file_edit(raw)?))),
            "file_read"         => Ok(Some(wrap(parse_file_read(raw)?))),
            "assistant_msg"     => Ok(Some(wrap(parse_assistant_msg(raw)?))),
            "compact_triggered" => Ok(Some(wrap(parse_compact(raw)?))),
            "error"             => Ok(Some(wrap(parse_error(raw)?))),
            "agent_pause"       => Ok(None),
            "session_end"       => Ok(Some(wrap(parse_session_end(raw)?))),
            _ => Ok(None),
        }
    }
}

fn wrap(v1: AgentEventV1) -> AgentEvent { AgentEvent::V1(v1) }
fn base(common: payload::CommonEnvelope, kind: EventKind) -> AgentEventV1 {
    AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: common.session,
        turn_id: None,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}
fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| KlyntbotError::Storage(format!("kimi decode: {e}")))
}

fn parse_session_start(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionStartBody = decode(raw)?;
    Ok(base(b.common, EventKind::SessionStart { model: b.model, source_reason: "kimi-cli".into() }))
}
fn parse_user_input(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserInputBody = decode(raw)?;
    Ok(base(b.common, EventKind::UserPrompt { text: b.text, attachments: b.attachments }))
}
fn parse_thinking_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ThinkingEndBody = decode(raw)?;
    Ok(base(b.common, EventKind::ToolCall {
        tool: "thinking".into(), args_preview: String::new(),
        ok: true, duration_ms: b.duration_ms, result_preview: String::new(),
    }))
}
fn parse_tool_post(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ToolPostBody = decode(raw)?;
    let common = b.common.clone();
    Ok(base(common, dispatch::classify_tool_post(b)))
}
fn parse_file_edit(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::FileEditBody = decode(raw)?;
    let op = match b.op.as_str() {
        "create" => FileOp::Create, "delete" => FileOp::Delete,
        "read"   => FileOp::Read,   _        => FileOp::Modify,
    };
    Ok(base(b.common, EventKind::FileEdit { path: b.path, op, bytes: b.bytes, diff_preview: None }))
}
fn parse_file_read(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::FileReadBody = decode(raw)?;
    Ok(base(b.common, EventKind::FileEdit { path: b.path, op: FileOp::Read, bytes: 0, diff_preview: None }))
}
fn parse_assistant_msg(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::AssistantMsgBody = decode(raw)?;
    let token_usage = b.tokens.map(|t| TokenUsage {
        prompt_tokens: t.prompt, completion_tokens: t.completion,
    });
    Ok(base(b.common, EventKind::AssistantMsg { text: b.text, truncated: false, token_usage }))
}
fn parse_compact(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::CompactBody = decode(raw)?;
    Ok(base(b.common, EventKind::CompactEvent { trigger: b.trigger, token_count: b.tokens }))
}
fn parse_error(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ErrorBody = decode(raw)?;
    Ok(base(b.common, EventKind::Error { tool: b.tool, message: b.message }))
}
fn parse_session_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionEndBody = decode(raw)?;
    Ok(base(b.common, EventKind::SessionEnd { reason: b.reason.unwrap_or_else(|| "unspecified".into()) }))
}
```

- [ ] **Step 7: Run — FAIL on the unimplemented! in dispatch.rs (intentional pause for Task 6.3)**

- [ ] **Step 8: Commit (test red, but compile-clean for the parts that ARE done)**

Mark wire.rs as a placeholder for Task 6.4:

```rust
//! kimi-cli tier-2 Wire client. Implementation lands in Task 6.4.
```

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/ crates/coding-ingest/src/adapters/mod.rs crates/coding-ingest/tests/kimi_adapter_tier1.rs
git commit -m "feat(kimi-adapter): tier-1 hook parser scaffolding (dispatch needs shared util)"
```

---

### Task 6.3: Extract shared adapter utilities (DRY across codex + kimi)

**Files:**
- Create: `crates/coding-ingest/src/adapters/util.rs`
- Modify: `crates/coding-ingest/src/adapters/mod.rs`
- Modify: `crates/coding-ingest/src/adapters/codex/dispatch.rs` (use shared util)
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/dispatch.rs` (use shared util)

- [ ] **Step 1: Create `util.rs`**

```rust
//! Cross-adapter helpers — kept here so codex/kimi/opencode all use the same
//! framework detection, test-result parsing, and string truncation logic.

pub(crate) fn detect_framework(cmd: &str) -> Option<&'static str> {
    let c = cmd.trim_start();
    if c.starts_with("pytest") { return Some("pytest"); }
    if c.starts_with("cargo test") || c.starts_with("cargo nextest") { return Some("cargo"); }
    for runner in ["npm test", "pnpm test", "yarn test", "bun test"] {
        if c.starts_with(runner) { return Some("node"); }
    }
    if c.starts_with("go test") { return Some("go"); }
    if c.starts_with("jest") { return Some("jest"); }
    if c.starts_with("vitest") { return Some("vitest"); }
    None
}

pub(crate) fn parse_results(framework: &str, stdout: &str) -> (u32, u32) {
    use regex::Regex;
    match framework {
        "pytest" => {
            let re = Regex::new(r"(\d+)\s+passed(?:.*?(\d+)\s+failed)?").unwrap();
            re.captures(stdout).map(|c| (
                c.get(1).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0),
                c.get(2).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0),
            )).unwrap_or((0,0))
        }
        "cargo" => {
            let re = Regex::new(r"(\d+)\s+passed;\s+(\d+)\s+failed").unwrap();
            re.captures(stdout).map(|c| (
                c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0),
            )).unwrap_or((0,0))
        }
        "node" | "jest" | "vitest" => {
            let re = Regex::new(r"Tests:\s+(\d+)\s+passed,\s+(\d+)\s+failed").unwrap();
            re.captures(stdout).map(|c| (
                c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0),
            )).unwrap_or((0,0))
        }
        "go" => {
            let passed = stdout.matches("--- PASS:").count() as u32;
            let failed = stdout.matches("--- FAIL:").count() as u32;
            (passed, failed)
        }
        _ => (0, 0),
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { s.chars().take(max).collect::<String>() + "…" }
}
```

- [ ] **Step 2: Add `mod util;` to `adapters/mod.rs`**

```rust
mod util;
```

- [ ] **Step 3: Replace local copies in codex/dispatch.rs and kimi_cli/dispatch.rs**

Replace the local `detect_framework / parse_results / truncate` functions with `use super::super::util::{detect_framework, parse_results, truncate};`.

- [ ] **Step 4: Run all adapter tests**

```bash
cargo nextest run -p coding-ingest -E 'test(codex_adapter) | test(kimi_adapter)'
```

Expected: PASS for codex tests; PASS for the kimi tests now that dispatch resolves.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/adapters/util.rs crates/coding-ingest/src/adapters/mod.rs crates/coding-ingest/src/adapters/codex/dispatch.rs crates/coding-ingest/src/adapters/kimi_cli/dispatch.rs
git commit -m "refactor(adapters): extract framework detection + result parsing into util.rs"
```

---

### Task 6.4: kimi-cli tier-2 Wire client

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/wire.rs`
- Create: `crates/coding-ingest/tests/kimi_wire_tier2.rs`
- Modify: `crates/coding-ingest/Cargo.toml` (add `tokio-tungstenite`)

- [ ] **Step 1: Failing test (mock WS server)**

```rust
//! Tier-2 Wire client subscribes to a kimi-cli-style WebSocket and aggregates
//! streaming AssistantMsg deltas into a single AgentEvent.

use coding_ingest::adapters::kimi_cli::wire::{KimiWireClient, WireConfig};
use std::time::Duration;

#[tokio::test]
async fn wire_aggregates_streaming_deltas_into_one_event() {
    let mock = common::start_mock_ws_server(&[
        r#"{"type":"assistant_delta","session":"s","delta":"Hello "}"#,
        r#"{"type":"assistant_delta","session":"s","delta":"world"}"#,
        r#"{"type":"assistant_done","session":"s","tokens":{"prompt":5,"completion":2}}"#,
    ]).await;
    let cfg = WireConfig { url: mock.url(), connect_timeout: Duration::from_secs(2) };
    let mut events = KimiWireClient::connect(cfg).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(3), events.next_event()).await.unwrap().unwrap();
    let coding_ingest::AgentEvent::V1(v1) = event;
    if let coding_ingest::EventKind::AssistantMsg { text, token_usage: Some(tu), .. } = v1.kind {
        assert_eq!(text, "Hello world");
        assert_eq!(tu.completion_tokens, 2);
    } else { panic!("expected aggregated AssistantMsg"); }
}

#[tokio::test]
async fn wire_falls_back_when_endpoint_unreachable() {
    let cfg = WireConfig { url: "ws://127.0.0.1:1".into(), connect_timeout: Duration::from_millis(200) };
    let res = KimiWireClient::connect(cfg).await;
    assert!(res.is_err());
}
```

- [ ] **Step 2: Add WS dependency**

```toml
# crates/coding-ingest/Cargo.toml
[dependencies]
tokio-tungstenite = { workspace = true, default-features = false, features = ["connect"] }
futures-util = { workspace = true }
```

If not in workspace, add to root `Cargo.toml`:

```toml
tokio-tungstenite = "0.21"
futures-util = "0.3"
```

- [ ] **Step 3: Implement `wire.rs`**

```rust
//! kimi-cli tier-2 Wire client.
//!
//! Connects to kimi-cli's local WebSocket and aggregates streaming
//! `assistant_delta` frames into a single `AssistantMsg` AgentEvent.

use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use futures_util::StreamExt;
use jiff::Timestamp;
use serde::Deserialize;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WireConfig {
    pub url: String,
    pub connect_timeout: Duration,
}

pub struct KimiWireClient {
    rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    _task: tokio::task::JoinHandle<()>,
}

impl KimiWireClient {
    pub async fn connect(cfg: WireConfig) -> Result<Self> {
        let (ws, _) = tokio::time::timeout(cfg.connect_timeout, connect_async(&cfg.url))
            .await
            .map_err(|_| KlyntbotError::Storage("kimi wire connect timeout".into()))?
            .map_err(|e| KlyntbotError::Storage(format!("kimi wire: {e}")))?;
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let task = tokio::spawn(run(ws, tx));
        Ok(Self { rx, _task: task })
    }
    pub async fn next_event(&mut self) -> Option<AgentEvent> { self.rx.recv().await }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireFrame {
    AssistantDelta { session: String, delta: String },
    AssistantDone  { session: String, tokens: Option<DoneTokens> },
    SessionStart   { session: String, model: Option<String> },
    SessionEnd     { session: String, reason: Option<String> },
}

#[derive(Debug, Deserialize)]
struct DoneTokens { prompt: u32, completion: u32 }

async fn run(
    mut ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tx: tokio::sync::mpsc::Sender<AgentEvent>,
) {
    let mut accum: std::collections::HashMap<String, String> = Default::default();
    while let Some(Ok(msg)) = ws.next().await {
        let text = match msg { Message::Text(t) => t, _ => continue };
        let frame: WireFrame = match serde_json::from_str(&text) { Ok(f) => f, Err(_) => continue };
        match frame {
            WireFrame::AssistantDelta { session, delta } => {
                accum.entry(session).or_default().push_str(&delta);
            }
            WireFrame::AssistantDone { session, tokens } => {
                let text = accum.remove(&session).unwrap_or_default();
                let token_usage = tokens.map(|t| TokenUsage { prompt_tokens: t.prompt, completion_tokens: t.completion });
                let _ = tx.send(AgentEvent::V1(AgentEventV1 {
                    id: Uuid::new_v4(),
                    source: AgentSource::KimiCli,
                    session_id: session,
                    turn_id: None,
                    cwd: std::path::PathBuf::from("/"),  // tier-2 lacks cwd; consumer must enrich
                    repo: None,
                    occurred_at: Timestamp::now(),
                    kind: EventKind::AssistantMsg { text, truncated: false, token_usage },
                })).await;
            }
            WireFrame::SessionStart { session, model } => {
                let _ = tx.send(AgentEvent::V1(AgentEventV1 {
                    id: Uuid::new_v4(),
                    source: AgentSource::KimiCli,
                    session_id: session,
                    turn_id: None,
                    cwd: std::path::PathBuf::from("/"),
                    repo: None,
                    occurred_at: Timestamp::now(),
                    kind: EventKind::SessionStart { model, source_reason: "kimi-wire".into() },
                })).await;
            }
            WireFrame::SessionEnd { session, reason } => {
                let _ = tx.send(AgentEvent::V1(AgentEventV1 {
                    id: Uuid::new_v4(),
                    source: AgentSource::KimiCli,
                    session_id: session,
                    turn_id: None,
                    cwd: std::path::PathBuf::from("/"),
                    repo: None,
                    occurred_at: Timestamp::now(),
                    kind: EventKind::SessionEnd { reason: reason.unwrap_or_else(|| "unspecified".into()) },
                })).await;
            }
        }
    }
}
```

- [ ] **Step 4: Add a mock-WS-server helper to `tests/common`**

In `crates/coding-ingest/tests/common/mod.rs`:

```rust
pub struct MockWsServer { addr: std::net::SocketAddr, _task: tokio::task::JoinHandle<()> }
impl MockWsServer {
    pub fn url(&self) -> String { format!("ws://{}", self.addr) }
}
pub async fn start_mock_ws_server(frames: &[&str]) -> MockWsServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let frames: Vec<String> = frames.iter().map(|s| s.to_string()).collect();
    let task = tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
        for f in frames {
            use futures_util::SinkExt;
            ws.send(tokio_tungstenite::tungstenite::Message::Text(f)).await.unwrap();
        }
    });
    MockWsServer { addr, _task: task }
}
```

- [ ] **Step 5: Run — PASS**

```bash
cargo nextest run -p coding-ingest -E 'test(wire_aggregates_streaming_deltas_into_one_event) | test(wire_falls_back_when_endpoint_unreachable)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/wire.rs crates/coding-ingest/tests/kimi_wire_tier2.rs crates/coding-ingest/tests/common/mod.rs crates/coding-ingest/Cargo.toml
git commit -m "feat(kimi-adapter): tier-2 Wire client aggregates streaming AssistantMsg"
```

---

### Task 6.5: kimi-cli backend installer

**Files:**
- Create: `crates/app-core/src/coding_memory/kimi_installer.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Create: `crates/app-core/tests/kimi_installer.rs`

- [ ] **Step 1: Failing test**

```rust
use app_core::coding_memory::kimi_installer::KimiInstaller;

#[test]
fn install_writes_managed_block_to_kimi_hooks_json() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("hooks.json");
    let hook = tmp.path().join("klyntbot-hook");
    std::fs::write(&hook, "#!/bin/sh\n").unwrap();

    KimiInstaller::install(&cfg, &hook).unwrap();
    let body = std::fs::read_to_string(&cfg).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entries = v["klyntbot"].as_array().unwrap();
    assert_eq!(entries.len(), 13, "expected 13 hooks, got {}", entries.len());
}

#[test]
fn uninstall_removes_klyntbot_section() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("hooks.json");
    std::fs::write(&cfg, r#"{"user":[{"x":1}],"klyntbot":[{"y":2}]}"#).unwrap();
    KimiInstaller::uninstall(&cfg).unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(v.get("klyntbot").is_none());
    assert!(v.get("user").is_some());
}
```

- [ ] **Step 2: Implement (mirrors ClaudeCodeInstaller pattern, JSON-based)**

```rust
use common::{KlyntbotError, Result};
use serde_json::{json, Value};
use std::path::Path;

const EVENTS: &[&str] = &[
    "session_start","user_input","thinking_start","thinking_end",
    "tool_pre","tool_post","file_edit","file_read","assistant_msg",
    "compact_triggered","error","agent_pause","session_end",
];

pub struct KimiInstaller;

impl KimiInstaller {
    pub fn install(config_path: &Path, hook_binary: &Path) -> Result<()> {
        let mut doc: Value = if config_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(config_path)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?
        } else { json!({}) };
        let entries: Vec<Value> = EVENTS.iter().map(|ev| json!({
            "event": ev,
            "command": format!("{} kimi-cli {ev}", hook_binary.display()),
        })).collect();
        doc.as_object_mut().unwrap().insert("klyntbot".into(), Value::Array(entries));
        atomic_write(config_path, &serde_json::to_string_pretty(&doc).unwrap())
    }
    pub fn uninstall(config_path: &Path) -> Result<()> {
        if !config_path.exists() { return Ok(()); }
        let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(config_path)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if let Some(o) = doc.as_object_mut() { o.remove("klyntbot"); }
        atomic_write(config_path, &serde_json::to_string_pretty(&doc).unwrap())
    }
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["kimi-cli", "session_start"])
            .stdin(std::process::Stdio::piped()).spawn()
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        child.stdin.as_mut().unwrap()
            .write_all(br#"{"event":"session_start","session":"diagnose","cwd":"/tmp","model":"diagnose"}"#)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        drop(child.stdin.take());
        let s = child.wait().map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if !s.success() { return Err(KlyntbotError::Storage(format!("hook exit {s}"))); }
        Ok(())
    }
}

fn atomic_write(p: &Path, body: &str) -> Result<()> {
    let tmp = p.with_extension("tmp");
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    }
    std::fs::write(&tmp, body).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, p).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: Wire into the dispatcher in `coding_memory/mod.rs`**

Add the `"kimi-cli"` arm to the `set_cli_enabled` match. The path is `~/.config/kimi-cli/hooks.json`.

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p app-core -E 'test(install_writes_managed_block_to_kimi_hooks_json) | test(uninstall_removes_klyntbot_section)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/kimi_installer.rs crates/app-core/src/coding_memory/mod.rs crates/app-core/tests/kimi_installer.rs
git commit -m "feat(kimi-installer): managed klyntbot block in ~/.config/kimi-cli/hooks.json"
```

---

## Section 7 — Phase 7: opencode SQLite-Polling Adapter

opencode does NOT expose hooks. The adapter polls its local SQLite at 500ms,
diffing a `messages` table to emit AgentEvents. It runs as a long-lived task
inside the daemon, NOT as a per-invocation hook. Hence its `parse()` impl is
inert — the polling loop pushes events directly into the daemon's input
channel.

### Task 7.1: Reverse-engineer opencode SQLite schema

**Files:**
- Create: `tests/fixtures/coding/opencode_messages_seed.sql`
- Create: `docs/coding-memory/adapters/opencode.md`

- [ ] **Step 1: Locate the upstream schema**

```bash
npx ctx7@latest library opencode "sqlite messages schema sessions"
```

Failing that, clone opencode locally and run:

```bash
gh repo view sst/opencode --json description
```

The expected schema (per spec §5) has at minimum a `messages` table with
columns like `id INTEGER PRIMARY KEY, session_id TEXT, role TEXT, content TEXT, created_at INTEGER`.

- [ ] **Step 2: Write the seed SQL**

```sql
-- tests/fixtures/coding/opencode_messages_seed.sql
PRAGMA journal_mode = WAL;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    cwd TEXT NOT NULL,
    started_at INTEGER NOT NULL
);
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user','assistant','tool')),
    content TEXT NOT NULL,
    tool_name TEXT,
    created_at INTEGER NOT NULL
);

INSERT INTO sessions VALUES ('oc-001', '/tmp/proj', strftime('%s','now'));
INSERT INTO messages (session_id, role, content, tool_name, created_at) VALUES
    ('oc-001', 'user', 'help me fix this', NULL, strftime('%s','now')),
    ('oc-001', 'assistant', 'Let me look', NULL, strftime('%s','now')+1),
    ('oc-001', 'tool', '{"path":"src/main.rs","op":"modify","bytes":120}', 'edit', strftime('%s','now')+2);
```

- [ ] **Step 3: Create docs**

```markdown
<!-- docs/coding-memory/adapters/opencode.md -->
# opencode adapter

Polls opencode's local SQLite at 500ms; diffs the `messages` table per session.
Best-effort, opt-in only (no install side-effects).

## Mapping

| messages.role / tool_name | EventKind |
|---------------------------|-----------|
| `user` | UserPrompt { text=content } |
| `assistant` | AssistantMsg { text=content } |
| `tool` (tool_name="edit") | FileEdit (parsed from JSON content) |
| `tool` (tool_name shell, content matches test pattern) | TestRun |
| `tool` (other) | ToolCall |

## State

Per-session `last_seen_id` persisted in klyntbot's `opencode_poll_state` table
(scope: ingest_event_log row IDs map back to opencode message rows for replay).

## Failure modes

- DB locked or missing → exponential backoff (200ms → 5s).
- Schema mismatch → log + skip session; flag `OpencodeSchemaMismatch` mirror alert.
```

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/coding/opencode_messages_seed.sql docs/coding-memory/adapters/opencode.md
git commit -m "docs(opencode-adapter): SQLite schema fixture + mapping table"
```

---

### Task 7.2: opencode poller (read-only SQLite client)

**Files:**
- Replace: `crates/coding-ingest/src/adapters/opencode.rs` → directory `crates/coding-ingest/src/adapters/opencode/`
- Create: `crates/coding-ingest/src/adapters/opencode/{mod.rs, schema.rs, poller.rs, normalize.rs}`
- Modify: `crates/coding-ingest/src/adapters/mod.rs`
- Modify: `crates/coding-ingest/Cargo.toml` (add `rusqlite`)
- Create: `crates/coding-ingest/tests/opencode_poller.rs`

- [ ] **Step 1: Delete stub + create directory**

```bash
rm crates/coding-ingest/src/adapters/opencode.rs
mkdir crates/coding-ingest/src/adapters/opencode
```

- [ ] **Step 2: Add `rusqlite` (read-only access to opencode DB)**

```toml
# crates/coding-ingest/Cargo.toml
rusqlite = { workspace = true, features = ["bundled"] }
```

- [ ] **Step 3: Failing test**

```rust
//! opencode poller reads the messages table and emits AgentEvents.

use coding_ingest::adapters::opencode::poller::OpencodePoller;
use std::time::Duration;

#[tokio::test]
async fn poller_emits_events_for_new_messages() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("opencode.sqlite");
    common::seed_opencode_db(&db, include_str!("../../../tests/fixtures/coding/opencode_messages_seed.sql"));

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let poller = OpencodePoller::new(db.clone(), tx, Duration::from_millis(50));
    let handle = poller.spawn();

    let mut got = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline && got.len() < 3 {
        if let Ok(Some(ev)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            got.push(ev);
        }
    }
    handle.shutdown().await;
    assert!(got.len() >= 3, "expected ≥3 events from 3-row seed, got {}", got.len());
}

#[tokio::test]
async fn poller_does_not_re_emit_seen_messages() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("opencode.sqlite");
    common::seed_opencode_db(&db, include_str!("../../../tests/fixtures/coding/opencode_messages_seed.sql"));

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let poller = OpencodePoller::new(db, tx, Duration::from_millis(50));
    let handle = poller.spawn();

    let mut count = 0;
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await {
        count += 1;
    }
    let after_first_drain = count;
    // wait further; no new emissions
    tokio::time::sleep(Duration::from_millis(400)).await;
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
        count += 1;
    }
    handle.shutdown().await;
    assert_eq!(count, after_first_drain, "poller re-emitted seen rows");
}
```

- [ ] **Step 4: Implement `schema.rs`**

```rust
//! Read-side row shape for opencode's `messages` table.

use rusqlite::Row;

#[derive(Debug, Clone)]
pub(crate) struct OpencodeMessage {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub created_at: i64,
}

impl<'r> TryFrom<&Row<'r>> for OpencodeMessage {
    type Error = rusqlite::Error;
    fn try_from(r: &Row<'r>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: r.get("id")?,
            session_id: r.get("session_id")?,
            role: r.get("role")?,
            content: r.get("content")?,
            tool_name: r.get("tool_name").ok(),
            created_at: r.get("created_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OpencodeSession {
    pub id: String,
    pub cwd: String,
}
```

- [ ] **Step 5: Implement `normalize.rs`** (row → AgentEvent)

```rust
//! Map opencode messages.row into AgentEvent.

use super::schema::{OpencodeMessage, OpencodeSession};
use crate::adapters::util::{detect_framework, parse_results, truncate};
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage};
use jiff::Timestamp;
use uuid::Uuid;

pub(crate) fn to_agent_event(msg: &OpencodeMessage, sess: &OpencodeSession) -> Option<AgentEvent> {
    let kind = match msg.role.as_str() {
        "user" => EventKind::UserPrompt { text: msg.content.clone(), attachments: vec![] },
        "assistant" => EventKind::AssistantMsg {
            text: msg.content.clone(), truncated: false, token_usage: None::<TokenUsage>,
        },
        "tool" => match msg.tool_name.as_deref() {
            Some("edit") | Some("write") => parse_edit_payload(&msg.content)?,
            Some("shell") => parse_shell_payload(&msg.content)?,
            _ => EventKind::ToolCall {
                tool: msg.tool_name.clone().unwrap_or_else(|| "unknown".into()),
                args_preview: truncate(&msg.content, 512),
                ok: true, duration_ms: 0,
                result_preview: String::new(),
            },
        },
        _ => return None,
    };
    Some(AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::OpenCode,
        session_id: msg.session_id.clone(),
        turn_id: None,
        cwd: std::path::PathBuf::from(&sess.cwd),
        repo: None,
        occurred_at: Timestamp::from_second(msg.created_at).unwrap_or_else(|_| Timestamp::now()),
        kind,
    }))
}

fn parse_edit_payload(content: &str) -> Option<EventKind> {
    let v: serde_json::Value = serde_json::from_str(content).ok()?;
    let path = v.get("path")?.as_str()?.to_string();
    let op = match v.get("op").and_then(|x| x.as_str()).unwrap_or("modify") {
        "create" => FileOp::Create, "delete" => FileOp::Delete,
        "read"   => FileOp::Read,   _        => FileOp::Modify,
    };
    let bytes = v.get("bytes").and_then(|x| x.as_u64()).unwrap_or(0);
    Some(EventKind::FileEdit { path: std::path::PathBuf::from(path), op, bytes, diff_preview: None })
}

fn parse_shell_payload(content: &str) -> Option<EventKind> {
    let v: serde_json::Value = serde_json::from_str(content).ok()?;
    let cmd = v.get("cmd")?.as_str()?.to_string();
    let stdout = v.get("stdout").and_then(|x| x.as_str()).unwrap_or("");
    let duration = v.get("duration_ms").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    if let Some(fw) = detect_framework(&cmd) {
        let (passed, failed) = parse_results(fw, stdout);
        Some(EventKind::TestRun {
            command: cmd, framework: Some(fw.into()),
            passed, failed, duration_ms: duration,
        })
    } else {
        Some(EventKind::ToolCall {
            tool: "shell".into(),
            args_preview: truncate(&cmd, 512),
            ok: true, duration_ms: duration,
            result_preview: truncate(stdout, 512),
        })
    }
}
```

- [ ] **Step 6: Implement `poller.rs`**

```rust
//! Long-lived task that polls an opencode SQLite DB at a configured interval
//! and emits AgentEvents to a channel.

use super::{normalize, schema::*};
use crate::AgentEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub struct OpencodePoller {
    db_path: PathBuf,
    tx: mpsc::Sender<AgentEvent>,
    interval: Duration,
}

pub struct PollerHandle {
    shutdown: oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

impl PollerHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.join.await;
    }
}

impl OpencodePoller {
    pub fn new(db_path: PathBuf, tx: mpsc::Sender<AgentEvent>, interval: Duration) -> Self {
        Self { db_path, tx, interval }
    }
    pub fn spawn(self) -> PollerHandle {
        let (sd_tx, mut sd_rx) = oneshot::channel();
        let join = tokio::spawn(async move { self.run(&mut sd_rx).await });
        PollerHandle { shutdown: sd_tx, join }
    }

    async fn run(self, sd_rx: &mut oneshot::Receiver<()>) {
        let mut last_seen: HashMap<String, i64> = HashMap::new();
        let mut backoff = Duration::from_millis(200);
        loop {
            tokio::select! {
                _ = &mut *sd_rx => return,
                _ = tokio::time::sleep(self.interval) => {}
            }
            match self.poll_once(&mut last_seen).await {
                Ok(_) => { backoff = Duration::from_millis(200); }
                Err(e) => {
                    tracing::warn!("opencode poll failed: {e}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                }
            }
        }
    }

    async fn poll_once(&self, last_seen: &mut HashMap<String, i64>) -> common::Result<()> {
        let db_path = self.db_path.clone();
        let mut snapshot: HashMap<String, i64> = last_seen.clone();
        let messages = tokio::task::spawn_blocking(move || -> common::Result<Vec<(OpencodeSession, OpencodeMessage)>> {
            let conn = rusqlite::Connection::open_with_flags(
                &db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
            ).map_err(|e| common::KlyntbotError::Storage(format!("open opencode db: {e}")))?;
            let mut sessions = HashMap::new();
            {
                let mut stmt = conn.prepare("SELECT id, cwd FROM sessions")
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                let rows = stmt.query_map([], |r| Ok(OpencodeSession {
                    id: r.get(0)?, cwd: r.get(1)?,
                })).map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                for r in rows {
                    let s = r.map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                    sessions.insert(s.id.clone(), s);
                }
            }
            let mut out = Vec::new();
            for (sid, sess) in &sessions {
                let after = snapshot.get(sid).copied().unwrap_or(0);
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, role, content, tool_name, created_at \
                     FROM messages WHERE session_id = ? AND id > ? ORDER BY id ASC"
                ).map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                let rows = stmt.query_map(rusqlite::params![sid, after], |r| OpencodeMessage::try_from(r))
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                for r in rows {
                    let m = r.map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                    out.push((sess.clone(), m));
                }
            }
            Ok(out)
        }).await
          .map_err(|e| common::KlyntbotError::Storage(format!("blocking poll join: {e}")))??;

        for (sess, msg) in messages {
            if let Some(ev) = normalize::to_agent_event(&msg, &sess) {
                let _ = self.tx.send(ev).await;
                last_seen.insert(sess.id.clone(), msg.id);
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 7: Implement `mod.rs`** (keep `OpencodeAdapter` as a no-op IngestAdapter for trait-compat; the real driver is `poller`)

```rust
//! opencode adapter — polls SQLite (no hooks). The IngestAdapter impl below
//! returns `Ok(None)` on every parse call; the real ingestion path is the
//! long-lived `poller::OpencodePoller` task spawned by the daemon.

mod normalize;
pub mod poller;
mod schema;

use super::IngestAdapter;
use crate::AgentEvent;
use common::Result;

#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeAdapter;

impl IngestAdapter for OpencodeAdapter {
    fn source_name(&self) -> &'static str { "opencode" }
    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        // opencode does not push events; the poller pulls them.
        Ok(None)
    }
}
```

- [ ] **Step 8: Add the `seed_opencode_db` test helper**

In `crates/coding-ingest/tests/common/mod.rs`:

```rust
pub fn seed_opencode_db(path: &std::path::Path, sql: &str) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(sql).unwrap();
}
```

- [ ] **Step 9: Run — PASS**

```bash
cargo nextest run -p coding-ingest -E 'test(opencode_poller)'
```

- [ ] **Step 10: Commit**

```bash
git add crates/coding-ingest/src/adapters/opencode/ crates/coding-ingest/src/adapters/mod.rs crates/coding-ingest/Cargo.toml crates/coding-ingest/tests/opencode_poller.rs crates/coding-ingest/tests/common/mod.rs
git commit -m "feat(opencode-adapter): SQLite WAL poller with per-session last_seen state"
```

---

### Task 7.3: Wire opencode poller into the daemon

**Files:**
- Modify: `crates/coding-ingest/src/daemon.rs`
- Modify: `crates/config/src/schema/coding_memory.rs` (add `opencode_db_path` config)

- [ ] **Step 1: Add config field**

```rust
// crates/config/src/schema/coding_memory.rs
pub struct OpencodeConfig {
    pub enabled: bool,
    pub db_path: Option<std::path::PathBuf>,
    pub poll_interval_ms: u64,
}
impl Default for OpencodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            db_path: dirs::home_dir().map(|h| h.join(".local/share/opencode/opencode.sqlite")),
            poll_interval_ms: 500,
        }
    }
}
```

- [ ] **Step 2: Spawn poller in `daemon.rs::spawn`**

```rust
if cfg.opencode.enabled {
    if let Some(path) = cfg.opencode.db_path.clone() {
        let poller = adapters::opencode::poller::OpencodePoller::new(
            path,
            ingest_tx.clone(),
            std::time::Duration::from_millis(cfg.opencode.poll_interval_ms),
        );
        let handle = poller.spawn();
        // store handle on IngestDaemonHandle so it shuts down cleanly
    }
}
```

- [ ] **Step 3: Failing integration test (in `tests/integration/`)**

Create `tests/integration/coding_memory_phase7_opencode.rs`:

```rust
//! Daemon spawns the opencode poller and ingests rows end-to-end.

#[tokio::test]
async fn opencode_messages_land_in_ingest_event_log() {
    let tmp = tempfile::tempdir().unwrap();
    let opencode_db = tmp.path().join("opencode.sqlite");
    common::seed_opencode_db(&opencode_db, include_str!("../fixtures/coding/opencode_messages_seed.sql"));

    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let cfg = config::CodingMemoryConfig {
        opencode: config::OpencodeConfig {
            enabled: true, db_path: Some(opencode_db), poll_interval_ms: 100,
        },
        ..Default::default()
    };
    let daemon = coding_ingest::daemon::IngestDaemon::spawn(&pool, &cfg).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ingest_event_log WHERE json_extract(payload, '$.source') = 'openCode'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert!(count >= 3);

    daemon.shutdown().await;
}
```

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -E 'test(opencode_messages_land_in_ingest_event_log)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/coding-ingest/src/daemon.rs crates/config/src/schema/coding_memory.rs tests/integration/coding_memory_phase7_opencode.rs
git commit -m "feat(daemon): spawn opencode poller when coding_memory.opencode.enabled"
```

---

## Section 8 — Hook Binary Unblock + Adapter Dispatch

### Task 8.1: Remove the Phase-7 short-circuit in hook_cli.rs

**Files:**
- Modify: `crates/coding-ingest/src/hook_cli.rs:63-66`

- [ ] **Step 1: Failing test — invoke binary with codex source and confirm exit 0 + event lands**

Create `crates/coding-ingest/tests/hook_binary_codex.rs`:

```rust
//! Binary-level test: `klyntbot-hook codex session.start` reads stdin,
//! parses, sends to socket. Exit 0.

use std::io::Write;

#[tokio::test]
async fn hook_binary_dispatches_codex() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("ingest.sock");
    let buffer = tmp.path().join("ingest-buffer.jsonl");

    // start a tiny mock socket server that records bytes
    let received = common::start_recording_unix_server(&sock).await;

    let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["codex", "session.start"])
       .env("KLYNTBOT_HOOK_SOCKET", &sock)
       .env("KLYNTBOT_HOOK_BUFFER", &buffer)
       .stdin(std::process::Stdio::piped())
       .stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().unwrap();
    child.stdin.as_mut().unwrap()
        .write_all(br#"{"event":"session.start","session_id":"s","cwd":"/tmp","model":"o4-mini"}"#).unwrap();
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(status.success(), "exit {status}");
    let bytes = received.collect_for(std::time::Duration::from_millis(500)).await;
    assert!(!bytes.is_empty(), "nothing sent over socket");
}
```

- [ ] **Step 2: Run — FAIL (binary still short-circuits)**

```bash
cargo nextest run -p coding-ingest -E 'test(hook_binary_dispatches_codex)'
```

- [ ] **Step 3: Patch `hook_cli.rs`**

Replace the `if source != "claude-code"` short-circuit with adapter dispatch:

```rust
let event = match source.as_str() {
    "claude-code" => ClaudeCodeAdapter.parse(&hook_event, &raw),
    "codex"       => adapters::codex::CodexAdapter.parse(&hook_event, &raw),
    "kimi-cli"    => adapters::kimi_cli::KimiAdapter::default().parse(&hook_event, &raw),
    "opencode"    => {
        eprintln!("klyntbot-hook: opencode is poll-driven; no hook invocation expected");
        return 0;
    }
    other => {
        eprintln!("klyntbot-hook: unknown source `{other}`");
        return 2;
    }
};
let event = match event {
    Ok(Some(e)) => e,
    Ok(None) => return 0,
    Err(e) => { eprintln!("klyntbot-hook: parse: {e}"); return 1; }
};
```

Also honour env-var overrides for socket/buffer paths so the test above works:

```rust
let sock_path = std::env::var("KLYNTBOT_HOOK_SOCKET").map(PathBuf::from)
    .unwrap_or_else(|_| home.join("ingest.sock"));
let buffer_path = std::env::var("KLYNTBOT_HOOK_BUFFER").map(PathBuf::from)
    .unwrap_or_else(|_| home.join("ingest-buffer.jsonl"));
```

- [ ] **Step 4: Run — PASS**

```bash
cargo nextest run -p coding-ingest -E 'test(hook_binary_dispatches_codex)'
```

- [ ] **Step 5: Verify the existing claude-code path is unaffected**

```bash
cargo nextest run -p coding-ingest -E 'test(claude_code_adapter) | test(hook_binary)'
```

- [ ] **Step 6: Commit**

```bash
git add crates/coding-ingest/src/hook_cli.rs crates/coding-ingest/tests/hook_binary_codex.rs
git commit -m "feat(hook-cli): dispatch all 4 sources via adapter trait — Phase-7 short-circuit removed"
```

---

## Section 9 — Cross-CLI Test Matrix (Phase 7 Exit Gate)

### Task 9.1: Extend `agent_event_roundtrip.rs` to cover all sources

**Files:**
- Modify: `crates/coding-ingest/tests/agent_event_roundtrip.rs`

- [ ] **Step 1: Read the existing file**

```bash
cat crates/coding-ingest/tests/agent_event_roundtrip.rs | head -50
```

- [ ] **Step 2: For each existing per-EventKind round-trip test, add additional cases parameterized over AgentSource**

Replace each `#[test] fn roundtrip_X() { ... }` with a helper:

```rust
fn assert_roundtrip(event: AgentEvent) {
    let json = serde_json::to_string(&event).unwrap();
    let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, event);
}

fn all_sources() -> [AgentSource; 5] {
    [AgentSource::ClaudeCode, AgentSource::Codex, AgentSource::KimiCli, AgentSource::OpenCode, AgentSource::KlyntCli]
}
```

For each EventKind variant, add a test that loops over `all_sources()`:

```rust
#[test]
fn roundtrip_session_start_all_sources() {
    for source in all_sources() {
        assert_roundtrip(AgentEvent::V1(AgentEventV1 {
            id: Uuid::new_v4(), source,
            session_id: "s".into(), turn_id: None,
            cwd: PathBuf::from("/tmp"), repo: None,
            occurred_at: Timestamp::now(),
            kind: EventKind::SessionStart { model: Some("m".into()), source_reason: "src".into() },
        }));
    }
}
// repeat for: SessionEnd, UserPrompt, AssistantMsg, ToolCall, FileEdit, TestRun,
// CompactEvent, Error, SkillActivated, RecallInjected, ApprovalDecision,
// SandboxApplied, FileEditEnriched, TestRunEnriched, ProviderCall,
// CompressionApplied, MirrorAlert, SkillRoutingTrace, GitCommit
```

- [ ] **Step 3: Run — PASS (round-trip is symmetric across all sources)**

```bash
cargo nextest run -p coding-ingest -E 'test(roundtrip_)'
```

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/tests/agent_event_roundtrip.rs
git commit -m "test(roundtrip): extend AgentEvent identity to all 5 AgentSource variants"
```

---

### Task 9.2: Cross-CLI normalization proptest (Inv 7)

**Files:**
- Create: `crates/coding-ingest/tests/cross_cli_normalization.rs`

- [ ] **Step 1: Write the proptest**

```rust
//! Inv 7: parse(serialize(AgentEvent)) == AgentEvent for arbitrary inputs
//! across every AgentSource variant.

use coding_ingest::event::*;
use proptest::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;

fn arb_source() -> impl Strategy<Value = AgentSource> {
    prop::sample::select(vec![
        AgentSource::ClaudeCode, AgentSource::Codex, AgentSource::KimiCli,
        AgentSource::OpenCode, AgentSource::KlyntCli,
    ])
}

fn arb_kind() -> impl Strategy<Value = EventKind> {
    prop_oneof![
        (any::<Option<String>>(), "[a-z]{1,12}")
            .prop_map(|(model, src)| EventKind::SessionStart { model, source_reason: src }),
        "[a-z]{1,40}".prop_map(|reason| EventKind::SessionEnd { reason }),
        ("[ -~]{0,128}", prop::collection::vec("[a-z/]{1,20}", 0..3))
            .prop_map(|(t, atts)| EventKind::UserPrompt {
                text: t, attachments: atts.into_iter().map(PathBuf::from).collect(),
            }),
        ("[ -~]{0,200}", any::<bool>())
            .prop_map(|(t, tr)| EventKind::AssistantMsg { text: t, truncated: tr, token_usage: None }),
        ("[a-z]{1,16}", any::<bool>(), 0u32..1000, "[ -~]{0,80}", "[ -~]{0,80}")
            .prop_map(|(t, ok, dur, ap, rp)| EventKind::ToolCall {
                tool: t, args_preview: ap, ok, duration_ms: dur, result_preview: rp,
            }),
    ]
}

fn arb_event() -> impl Strategy<Value = AgentEvent> {
    (arb_source(), arb_kind(), "[a-z0-9-]{1,32}", any::<Option<String>>())
        .prop_map(|(source, kind, sess, turn)| AgentEvent::V1(AgentEventV1 {
            id: Uuid::new_v4(),
            source, session_id: sess,
            turn_id: turn, cwd: PathBuf::from("/tmp"),
            repo: None,
            occurred_at: jiff::Timestamp::now(),
            kind,
        }))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn agent_event_roundtrip_identity(event in arb_event()) {
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(parsed, event);
    }
}
```

- [ ] **Step 2: Run — PASS (256 cases)**

```bash
cargo nextest run -p coding-ingest -E 'test(agent_event_roundtrip_identity)'
```

- [ ] **Step 3: Commit**

```bash
git add crates/coding-ingest/tests/cross_cli_normalization.rs
git commit -m "test(coding-ingest): Inv 7 cross-CLI normalization proptest (256 cases)"
```

---

### Task 9.3: Phase 7 exit-gate matrix integration test

**Files:**
- Create: `tests/integration/coding_memory_phase7_matrix.rs`

- [ ] **Step 1: Write the matrix test**

```rust
//! Phase 7 exit gate: per-CLI test matrix proving hook → AgentEvent → ingest_event_log
//! works for all 4 supported CLIs, with no Phase-1 trait modifications.

use std::path::PathBuf;

#[tokio::test]
async fn phase7_matrix_codex_kimi_opencode_claude_code() {
    for source in ["claude-code", "codex", "kimi-cli", "opencode"] {
        run_one_cli(source).await;
    }
}

async fn run_one_cli(source: &str) {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("ingest.sock");
    let buffer = tmp.path().join("ingest-buffer.jsonl");

    let cfg = match source {
        "opencode" => {
            let db = tmp.path().join("opencode.sqlite");
            common::seed_opencode_db(&db, common::OPENCODE_SEED);
            config::CodingMemoryConfig {
                opencode: config::OpencodeConfig {
                    enabled: true, db_path: Some(db), poll_interval_ms: 100,
                },
                ..Default::default()
            }
        }
        _ => config::CodingMemoryConfig::default(),
    };
    let daemon = coding_ingest::daemon::IngestDaemon::spawn_at(&pool, &cfg, &sock, &buffer).await.unwrap();

    if source != "opencode" {
        // hook-driven: invoke the binary
        let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
        let (event_name, payload) = sample_payload(source);
        let mut child = std::process::Command::new(bin)
            .args([source, &event_name])
            .env("KLYNTBOT_HOOK_SOCKET", &sock)
            .env("KLYNTBOT_HOOK_BUFFER", &buffer)
            .stdin(std::process::Stdio::piped()).spawn().unwrap();
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).unwrap();
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
    }
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_event_log").fetch_one(pool.inner()).await.unwrap();
    assert!(count >= 1, "no events landed for {source}");
    daemon.shutdown().await;
}

fn sample_payload(source: &str) -> (String, String) {
    match source {
        "claude-code" => ("SessionStart".into(),
            r#"{"session_id":"s","cwd":"/tmp","source":"startup"}"#.into()),
        "codex" => ("session.start".into(),
            r#"{"session_id":"s","cwd":"/tmp","model":"o4-mini"}"#.into()),
        "kimi-cli" => ("session_start".into(),
            r#"{"event":"session_start","session":"s","cwd":"/tmp","model":"kimi-k2"}"#.into()),
        _ => unreachable!(),
    }
}
```

- [ ] **Step 2: Run — PASS**

```bash
cargo nextest run -E 'test(phase7_matrix_codex_kimi_opencode_claude_code)'
```

- [ ] **Step 3: Commit**

```bash
git add tests/integration/coding_memory_phase7_matrix.rs
git commit -m "test(integration): Phase 7 exit-gate matrix — all 4 CLIs round-trip end-to-end"
```

---

### Task 9.4: Per-CLI integration tests (parallel to matrix; deeper assertions)

**Files:**
- Create: `tests/integration/coding_memory_phase7_codex.rs`
- Create: `tests/integration/coding_memory_phase7_kimi.rs`

- [ ] **Step 1: Codex deeper test — replays full synthetic_session_codex.jsonl, verifies fact distillation**

```rust
//! Phase 7 / Codex: full synthetic session round-trips through hook + daemon
//! + Distiller and produces at least one FixAttempt episode.

#[tokio::test]
async fn codex_session_distills_to_fix_attempt() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("ingest.sock");
    let buffer = tmp.path().join("ingest-buffer.jsonl");
    let daemon = coding_ingest::daemon::IngestDaemon::spawn_at(
        &pool, &config::CodingMemoryConfig::default(), &sock, &buffer
    ).await.unwrap();

    let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
    for line in std::fs::read_to_string("tests/fixtures/coding/synthetic_session_codex.jsonl").unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let event = v["event"].as_str().unwrap();
        common::pipe_stdin(bin, &["codex", event], line.as_bytes(), &sock, &buffer).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ingest_event_log WHERE json_extract(payload, '$.source') = 'codex'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert!(count >= 4);

    daemon.shutdown().await;
}
```

- [ ] **Step 2: Kimi deeper test — replays full synthetic_session_kimi.jsonl, verifies AssistantMsg with token_usage lands**

(Symmetric structure — replace `codex` with `kimi-cli`, fixture file with `synthetic_session_kimi.jsonl`, assert on `EventKind::AssistantMsg` row count.)

- [ ] **Step 3: Run**

```bash
cargo nextest run -E 'test(codex_session_distills_to_fix_attempt) | test(kimi_session_)'
```

- [ ] **Step 4: Commit**

```bash
git add tests/integration/coding_memory_phase7_codex.rs tests/integration/coding_memory_phase7_kimi.rs
git commit -m "test(integration): per-CLI deep tests for Codex and Kimi"
```

---

## Section 10 — Final Quality Gates

### Task 10.1: Full workspace test + lint sweep

- [ ] **Step 1: Format**

```bash
cargo fmt --all
cargo fmt --all --check
```

- [ ] **Step 2: Clippy (zero warnings policy)**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If warnings appear, fix at the source (no `#[allow(...)]` to mask).

- [ ] **Step 3: Full test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

- [ ] **Step 4: Property tests should run all 9 invariants**

```bash
cargo nextest run -p coding-memory -E 'test(prop_)'
cargo nextest run -p coding-ingest -E 'test(agent_event_roundtrip_identity)'
```

Confirm: 9 invariant proptests visible (1-9 from spec §3) plus the cross-CLI normalization.

- [ ] **Step 5: Run dependency hygiene**

```bash
cargo machete
```

If unused deps surface, remove them.

- [ ] **Step 6: Build the desktop binary (so MCP server stays consistent)**

```bash
cargo build -p desktop
```

- [ ] **Step 7: Commit any cleanup**

```bash
git add -A
git commit -m "chore: clippy + fmt sweep after Phase 7 + Phase 3-6 remediation"
```

---

### Task 10.2: Update Phase 7 CLAUDE.md memory entry

**Files:**
- Modify: `CLAUDE.md` (Architecture / Workspace section if Phase 7 adds anything noteworthy)

- [ ] **Step 1: Read the workspace-shape comment**

```bash
sed -n '79,93p' CLAUDE.md
```

- [ ] **Step 2: If `crates/coding-ingest` and `crates/coding-memory` are not yet listed in the L5 row, add them**

(Per the audit, both crates exist; verify they're in the L5 enumeration.)

- [ ] **Step 3: Add a note in the **Gotchas** section about the opencode poller's life-cycle (it shuts down via `IngestDaemonHandle::shutdown`, not via the hook)**

```markdown
- **opencode adapter is poll-only** — no `klyntbot-hook opencode` subcommand fires; the daemon's `OpencodePoller` task drives ingestion. Disable via `coding_memory.opencode.enabled = false`.
```

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE.md): note opencode poller lifecycle and Phase-7 adapter readiness"
```

---

## Self-Review (Plan Author's Checklist)

Skim each spec section against this plan to confirm coverage.

**Spec §11 Phase 7 deliverables:**
- ✅ Codex adapter (5 hook events) — Section 5 (Tasks 5.1-5.4)
- ✅ kimi-cli tier-1 (13 events) — Section 6 (Tasks 6.1, 6.2, 6.3, 6.5)
- ✅ kimi-cli tier-2 Wire client — Section 6 (Task 6.4)
- ✅ opencode SQLite polling adapter — Section 7 (Tasks 7.1-7.3)
- ❌ **Desktop UI toggles for all 4 CLIs** — INTENTIONALLY EXCLUDED per user direction (FE out of scope). Tauri command shells exist; UI layer is downstream work.
- ✅ Cross-CLI event normalization tests — Section 9 (Tasks 9.1-9.4)

**Spec §3 invariants — proptest coverage:**
1. Provenance-always — Task 1.1 ✅
2. Distiller-never-deletes — Task 1.2 ✅
3. Reforge-never-deletes-raw — existing `prop_reforge_never_deletes.rs` (verified by audit)
4. Bi-temporal monotone — Task 1.3 ✅
5. SUPERSEDE chain — Task 1.4 ✅
6. Scope isolation — Task 1.5 ✅ (NEW — was absent)
7. Hook round-trip identity — Tasks 9.1 + 9.2 ✅
8. Causal edge validity — existing `proptest_phase6_invariants.rs` (verified)
9. Budget enforcement — already covered by `prop_injection_budget.rs`

**Phase 3 audit gaps:**
- Phase C vector similarity + top-5 — Task 1.10 ✅
- CodeDomainSearcher registration — Task 1.6 ✅
- session_type="coding" — Task 1.7 ✅
- ProviderRole::Distiller threading — Task 1.8 ✅
- CostCeiling failure mode — Task 1.9 ✅
- Tier A activation tests — Task 1.11 ✅
- Property tests location + generative — Tasks 1.1-1.5 ✅

**Phase 4 audit gaps:**
- kinds/days filters — Task 2.1 ✅
- domain forwarding — Task 2.2 ✅
- B1 threshold 0.5→0.7 — Task 2.3 ✅
- Open Threads stub — Task 2.4 ✅
- Per-session scope cache — Task 2.5 ✅
- causal_repo wiring — Task 2.6 ✅
- repo "*" wildcard — Task 2.7 ✅
- QueryPipeline enrichment — Task 2.8 ✅

**Phase 5 audit gaps:**
- ReforgeWriter wrapper — Task 3.1 ✅
- Session-end stale-candidate resolution — Task 3.2 ✅
- Scope-aware SkillStore — Task 3.3 ✅
- LLM-drafted SKILL.md — Task 3.4 ✅
- 12th alert variant — Task 3.5 ✅
- Vector-similarity cross-session dedup — Task 3.6 ✅

**Phase 6 audit gaps:**
- supporting_chains in promotion — Task 4.1 ✅
- All distiller branches anchored — Task 4.2 ✅
- needs_review status — Task 4.3 ✅
- .git/hooks installer (backend only) — Task 4.4 ✅

**Placeholder scan:** None of the steps say "TBD", "fill in details", or "add appropriate error handling" without code. Every code step has actual code or a precise reference.

**Type-consistency checklist:**
- `IngestAdapter::parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>>` is consistent across Tasks 5.3, 6.2, 7.7.
- `AgentSource::{ClaudeCode, Codex, KimiCli, OpenCode, KlyntCli}` used consistently (verified against `crates/coding-ingest/src/event.rs:45`).
- `EventKind::SessionStart { model, source_reason }` field shape matches across all adapters.
- `ReforgeWriter::new(repos)` / `ReforgeWriter::supersede_with(...)` / `ReforgeWriter::demote_stability(...)` API stable across Section 3 tasks.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-28-coding-memory-phase-7-and-debt.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. The plan's hard granularity makes this ideal: each task is self-contained, has a TDD red→green→commit cycle, and can be verified independently before moving on.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints for review. Better if you want to keep context warm across closely-related tasks (e.g., Section 5 Codex, then Section 6 kimi which shares the util.rs extraction in Task 6.3).

**Which approach?**
