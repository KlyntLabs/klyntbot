# Phase 3 Architecture Doc Verification — Agent 9

**Crates:** `cognitive`, `coding-ingest`, `coding-memory`
**Docs:** `docs/architecture/crates/cognitive.md`, `coding-ingest.md`, `coding-memory.md`
**Verified:** 2026-05-16
**Method:** Source-file reading, signature comparison, module-tree diff, TODO/FIXME/unimplemented catalog.

---

## Summary

| Crate | Accurate | Drift | Wrong | Missing |
|-------|---------:|------:|------:|--------:|
| `cognitive` | ~60 % | ~35 % | ~5 % | Low |
| `coding-ingest` | ~55 % | ~40 % | ~0 % | Moderate |
| `coding-memory` | ~45 % | ~35 % | ~20 % | High |

**Overall assessment:** The `cognitive` doc captures the *intent* and *high-level design* accurately (common-confusion points, two `retrievability` functions, two `AutotunerBridge` traits, Mirror wiring, 16-phase cycle structure). However, **concrete signatures, struct fields, and method names drifted heavily** across all three crates. The `coding-memory` doc is the most stale: it describes an older module tree, older `Distiller`/`ReforgeWriter` APIs, and incorrectly claims that four Reforge phases are stubbed when they are in fact fully implemented and wired in `app-core`.

---

## Per-Crate Findings

### `cognitive`

#### ✅ Accurate
- **Module tree** — `services/reforge/`, `services/community_intelligence/`, `mirror/sources/`, `repos/` (25+ files), `pipeline/`, `search/bm25.rs` all exist as documented.
- **`UnifiedMemoryService`** struct exists and implements `MemoryRetriever`; PPR boost and BM25 merge are real.
- **`ConversationRecallService`**, **`SessionMemoryService`**, **`CognitiveContextSource`** structs exist. `CognitiveContextSource::priority()` is `60`.
- **`compute_situation`** function exists.
- **`ExtractionHandler`** trait exists; `BatchExtraction`/`BatchExtractionResult` exist.
- **`run_reforge`** entry point exists with ~25 `Option<&dyn Trait>` extension hooks.
- **6 Reforge hook traits** exist (`ReforgeHandler`, `AutotunerBridge`, `GraphEnrichmentHandler`, `CommunityIntelligenceHandler`, `CodingPhaseRunner`, `CrossCliPhaseRunner`, `SkillDiscoveryRunner`).
- **16-phase cycle markers** exist in `services/reforge/service.rs` (actually 17+ distinct log markers, but the doc's table rows are all present).
- **`MirrorEngine`/`StartedMirror`** exist; `StartedMirror` fields match doc.
- **`NarrativeHandler`** and **`EarlyTrialEvaluator`** traits exist.
- **Two `retrievability` functions** — `services/fsrs5.rs` uses power-law `1/(1+t/(9S))`; `services/decay.rs` uses exponential `exp(ln(0.9)*t/s)`. Verified correct.
- **Two `AutotunerBridge` traits** — `services/reforge/mod.rs` has `run_evaluation`/`create_trials`; `mirror/types.rs` has `apply_champion`/`current_champion_params`. Verified correct.
- **`SkillEffectivenessSource`** is a stub (no-op `accumulate`/`flush` with `TODO(T7)`) and is **NOT registered** by `MirrorEngine::start` (confirmed by `engine.rs` imports and the `start_produces_eight_consumers` test).
- **`strategy_records` raw-SQL access** confirmed at `services/reforge/feedback.rs:173`.
- **KCA env flags** (`KCA_COMMUNITY_SUMMARIES`, `KCA_REFORGE_COMPRESS`, etc.) all appear in code.
- **`ProceduralRuleRepo`** is live (full CRUD), not a stub.
- **`DEFAULT_WEIGHTS`** is `[f64; 19]` in `services/fsrs5.rs`.
- **`RelevanceWeights`** has 12 factors in `services/decay.rs`.
- **Louvain** is first-party (~394 LOC) using `petgraph::UnGraph`.
- **PPR** is first-party (~404 LOC) using `petgraph::DiGraph<String, f32, u32>`.
- **`cognitive_migrations()`** exported from `repos/mod.rs`.

#### ⚠️ Drift
- **`UnifiedMemoryService` public API** — Doc claims `search(&self, query, session_key, top_k)` and `search_with_ppr_boost`. Actual public API is `MemoryRetriever::retrieve(&self, query, limit)`, plus `retrieve_scoped` and `retrieve_with_overrides`. The documented methods do not exist.
- **`RecallConfig`** — Doc fields: `top_k`, `include_episodic`, `include_semantic`, `time_range`. Actual: `decay_half_life_days`, `default_threshold`, `default_limit`.
- **`RecallResult`/`RecallMetadata`** — Doc describes a nested metadata struct with `query_expansion`, `embedding_ms`, etc. Actual `RecallResult` is a flat struct (`id`, `session_key`, `role`, `content`, `score`, `created_at`). No `RecallMetadata` struct exists.
- **`SessionMemoryConfig`** — Doc fields: `max_session_facts`, `decay_half_life`. Actual: `event_rx`, `session_repo`, `memory_repo`, `provider`, `cancel`.
- **`SituationInputs`/`UserSituation`** — Doc fields (`active_tasks`, `recent_messages`, `focus_state`, `time_of_day`, `current_focus`, `active_topics`, `estimated_load`) do not match actual fields (`hours_active_today`, `mins_since_break`, `productive_ratio_today`, `energy_level`, `focus_state`, `deadline_pressure`, `distraction_risk`, etc.).
- **`ExtractionHandler`** — Doc method name: `extract`. Actual: `extract_facts_batch`.
- **`ExtractedFact`** — Doc field `source_message_id` does not exist. Actual has `domain`, `source`, `speaker`, `valid_until`, `valid_from`.
- **`ExtractedEntity`** — Doc fields: `kind`, `aliases`. Actual: `entity_type`, `description`.
- **`ExtractedRelationship`** — Doc fields: `source_entity`, `target_entity`, `kind`. Actual: `source_name`, `target_name`, `relationship_type`.
- **`run_reforge` parameter count** — Doc claims 26 parameters. Actual signature has 25 parameters (counted individually).
- **Reforge `AutotunerBridge`** — Doc says `run_evaluation(&self, ctx: AutotunerContext) -> Result<CycleResult>` and `create_trials(&self, ctx: AutotunerContext) -> Result<Vec<Trial>>`. Actual: `run_evaluation(&self) -> Result<Phase6Result>` and `create_trials(&self, Vec<ValidatedTrial>) -> Result<u32>`. Also has `champion_params_map`, `active_trial_count`, `expire_stale_trials` not in doc.
- **`CodingPhaseRunner`** — Doc shows 4 methods. Actual has 5: `run_synthesis`, `run_rule_artifacts`, `run_cross_session_dedup`, `run_selective_delete`, `run_symbol_validation`.
- **`MirrorEngine::start`** — Doc shows params as non-Option (`repo`, `narrative_handler: Arc<dyn ...>`, etc.). Actual signature wraps almost everything in `Option` (`narrative_handler: Option<Arc<dyn ...>>`, `autotuner_bridge: Option<Arc<dyn ...>>`, etc.).
- **`MirrorFacade` method names** — Doc: `brain_state`, `recent_narratives`, `record_feedback`. Actual: `get_state`, `get_narratives`, `submit_feedback`. `list_brain_versions` matches.
- **Louvain return type** — Doc: `Vec<HashSet<String>>`. Actual: `CommunityAssignment` (struct with `assignments: HashMap<String, usize>`, `modularity`, `community_count`).
- **PPR signature** — Doc: `personalized_pagerank(graph, seeds, teleport_prob: f32, iterations: u32)`. Actual: `personalized_pagerank(graph, seeds, cfg: &PprConfig)` where `PprConfig` has `alpha`, `max_iterations`, `tolerance`.
- **`SemanticFactRepo`** — Doc methods: `find`, `find_active(subject, predicate)`, `search_by_subject`, `search_full_text`, `invalidate(id, valid_until)`. Actual: `get(id)`, `list_active(domain)`, `find_by_subject_predicate`, `search_fts`, `invalidate_batch(ids)`. Signature shapes differ.
- **`FlashcardRepo`** — Doc: `create`, `record_review(card_id, rating, reviewed_at)`, `due_cards`. Actual: `create_single`, `record_review(id, quality, recall_speed_ms)`, `get_all_due_cards`/`get_due_cards`.
- **`FsrsParamsRepo`** — Doc: `get_for_domain(domain) -> Result<[f64;19]>`, `update(domain, weights)`. Actual: `get_weights() -> Result<[f64;19]>`, `update_weights(weights)`, `update_desired_retention`, `seconds_since_trained`.
- **`ProceduralRuleRepo`** — Doc: `search_text`, `find_active_for_context(ctx_key)`. Actual: `search_fts`, `list_active(domain: &str)`.

#### ❌ Wrong
- **`services/reforge/service.rs:1` doc comment** still says "drives all 8 phases". The doc correctly flags this as stale, but the source comment itself is wrong (actual code has 17+ phase markers).

#### 🔍 Missing
- None critical; the doc covers the crate well at the conceptual level.

#### 📋 Tech Debt
- `mirror/sources/skill_effectiveness.rs:77` — `TODO(T7): Extract tool_name and success from coding-memory queries`
- `mirror/sources/skill_effectiveness.rs:84` — `TODO(T7): Query coding_memory for recent tool executions`
- No `unimplemented!()`, `todo!()`, or `FIXME` found elsewhere in the crate.

---

### `coding-ingest`

#### ✅ Accurate
- **Five adapters exist** as claimed: `claude_code`, `codex`, `kimi_cli`, `opencode`, `git_post_commit`.
- **Hook vs poll classification** is correct: `claude_code` and `codex` are hook-driven; `kimi_cli` and `opencode` are poll-only with short-circuit messages; `git_post_commit` is hook-driven.
- **`AgentEvent::V1`** wrapper struct exists with fields `id`, `source`, `session_id`, `turn_id`, `cwd`, `repo`, `occurred_at`, `kind`.
- **`AgentSource`** enum has exactly 5 variants (`ClaudeCode`, `Codex`, `KimiCli`, `OpenCode`, `KlyntCli`).
- **`RepoScope`** struct exists.
- **`hook_cli::run` dispatch behavior** matches doc: `status`, `context`, `git-post-commit`, `claude-code`, `codex`, `kimi-cli` short-circuit, `opencode` short-circuit.
- **`HookClient`** implements socket-first-then-buffer fallback.
- **`OpencodePoller`** polls SQLite `message`/`part` tables by `time_created`.
- **`IngestEventLogRepo`** exists for persistence.
- **`IngestDaemon`** spawns socket listener, buffer drainer, and poller tasks.
- **Cross-CLI normalization proptest** exists at `tests/cross_cli_normalization.rs` with exactly 64 cases (`ProptestConfig::with_cases(64)`).
- **`kimi_cli::mapper.rs:265`** TODO confirmed: `// TODO(distiller): attach token usage to the prior AssistantMsg row.`
- **Codex legacy dead code** confirmed: `codex/mod.rs:8` notes "legacy `dispatch` and `payload` modules below are retained as dead code".

#### ⚠️ Drift
- **Module tree** — Doc lists `exclude_set.rs`, `repo_scope.rs`, `repos.rs`. Actual files: `excludes.rs`, `scope.rs` + `scope_resolver.rs`, `store.rs`. Doc omits `coverage/`, `transport/`, `desktop_lock.rs`, `git_invalidation.rs`, `pending_invalidations.rs`, `warn.rs`.
- **`IngestAdapter` trait** — Doc shows only `parse`. Actual trait also requires `source_name(&self) -> &'static str`.
- **`EventKind` variant count** — Doc claims 21. Actual has **22** (includes `GitCommit` which the doc omits from the count).
- **`EventKind` field names** — Many variants have different fields than documented:
  - `SessionStart`: doc `{ profile, agent_id }` → actual `{ model, source_reason }`
  - `SessionEnd`: doc `{ summary }` → actual `{ reason }`
  - `UserPrompt`: doc `{ content }` → actual `{ text, attachments }`
  - `AssistantMsg`: doc `{ content, model }` → actual `{ text, truncated, token_usage }`
  - `ToolCall`: doc `{ tool_name, args_preview, result_preview, duration_ms }` → actual `{ tool, args_preview, ok, duration_ms, result_preview }` (adds `ok: bool`)
  - `FileEdit`: doc `{ path, edit_kind }` → actual `{ path, op, bytes, diff_preview }`
  - `TestRun`: doc `{ kind, passed, failed, duration_ms }` → actual `{ command, framework, passed, failed, duration_ms }`
  - `CompactEvent`: doc `{ tokens_before, tokens_after }` → actual `{ trigger, token_count }`
  - `Error`: doc `{ message }` → actual `{ tool, message }`
  - `SkillActivated`: doc `{ skill_name, reason }` → actual `{ skill_id, source_path, trigger }`
  - `ApprovalDecision`: doc `{ tool, class, decision, decided_by }` → actual `{ tool, decision, layer }`
  - `SandboxApplied`: doc `{ policy, allowed_paths, network }` → actual `{ tool, policy_summary, fallback_unsandboxed }`
  - And several others.
- **`hook_cli::run` signature** — Doc: `pub fn run(args: &[String]) -> Result<()>`. Actual: `pub fn run(args: Vec<String>) -> i32` (returns exit code, not `Result`).
- **`HookClient::new`** — Doc: `new() -> Self`. Actual: `new(socket_path: PathBuf, buffer_path: PathBuf, warn_stamp: PathBuf) -> Self`.
- **`OpencodePoller`** — Doc constructor order: `(db_path, poll_interval, tx, repo)`. Actual: `(db_path, event_tx, repo, interval)`. Doc method: `start(self) -> JoinHandle<()>`. Actual: `spawn(self) -> JoinHandle<()>`.
- **`IngestEventLogRepo`** — Doc methods: `write_event`, `list_recent`, `find_by_session`, `find_by_repo`. Actual methods: `insert`, `list_unprocessed`, `count_by_session`, `mark_processed`, `fetch_turn`, `last_distilled_at`, etc.
- **`IngestDaemon`** — Doc shape: `new(repo, distiller, config) -> Self` then `start(self) -> JoinHandle`. Actual: free function `spawn(cfg: IngestDaemonConfig) -> Result<IngestDaemonHandle>`.
- **`RepoScope` enrichment** — Doc claims `enrich_with_repo_scope(event: &mut AgentEvent, cwd: &Path) -> Result<()>`. Actual: `enrich_with_scope(event: AgentEvent) -> AgentEvent` (takes ownership, returns new event, no `cwd` param).

#### ❌ Wrong
- None found at the behavioral level; the drift is in signatures and module names, not in the overall architecture description.

#### 🔍 Missing
- `ExcludeSet` struct exists in `excludes.rs` (doc module name `exclude_set.rs` is wrong).
- `RepoScope` enrichment function name and signature differ (see Drift).

#### 📋 Tech Debt
- `adapters/kimi_cli/mapper.rs:265` — `TODO(distiller): attach token usage to the prior AssistantMsg row.`
- No `FIXME`, `unimplemented!()`, `todo!()`, or `NotImplementedInPhase` found.

---

### `coding-memory`

#### ✅ Accurate
- **`Distiller`** struct exists; `distiller/mod.rs` contains Phase A/B/C pipeline.
- **`DistillerWriter`** exists; enforces provenance invariant.
- **`ReforgeWriter`** exists and rejects raw DELETE (`reject_delete` returns `Err`).
- **`CodingRecallService`** exists.
- **`CodingMemoryToolset`** exists with `dispatch` method.
- **`CODING_MEMORY_MCP_TOOLS`** constant has exactly 8 tool names, matching doc.
- **`SymbolExtractor`** trait exists.
- **`TreeSitterExtractor`** exists and supports Rust, TypeScript, JavaScript, Python, Go.
- **`NotImplementedInPhase`** type exists in `error.rs`.
- **`ReforgeWriter::set_superseded_by`** performs bi-temporal supersede (`valid_until` + `superseded_by`).
- **`ReforgeWriter::demote_stability`** sets convergence to `0.01`.
- **`SessionEndPass`** struct exists and is wired in `app-core`.
- **`CrossSessionDedup`** struct exists and is wired in `app-core`.

#### ⚠️ Drift
- **Module tree** — Major drift. Doc claims files that do not exist:
  - `distiller/boundary.rs` — **missing**
  - `distiller/phase_a5.rs` — **missing**
  - `reforge/session_end_pass.rs` — actual is `reforge/session_end.rs`
  - `symbols/tree_sitter.rs` — **missing**
  - `symbols/anchors.rs` — **missing**
  - `observation/mod.rs`, `observation/reconcile.rs` — **missing**
  - `retry/mod.rs` — **missing** (actual: `distiller/retry_queue.rs`)
  Many extra modules exist and are not in the doc: `causal/`, `code_domain_searcher.rs`, `code_state.rs`, `counterfactual.rs`, `facts.rs`, `git_invalidation.rs`, `mirror/`, `problem_hash.rs`, `retrieval_skills/`, `scope.rs`, `sink/`, `skill_evolver/`, `skills.rs`, `reforge/cross_cli_synthesis.rs`, `reforge/managed_block.rs`, `reforge/selective_delete.rs`, `reforge/sensitivity_filter.rs`, `reforge/session_summary_repo.rs`, `reforge/symbol_validation.rs`, `reforge/synth_handler.rs`, `reforge/types.rs`.
- **`Distiller::new` signature** — Doc: `new(cognitive_provider, symbol_extractor, repos, config)`. Actual: `new(config, ingest_repo, writer, provider, retriever)`. No `symbol_extractor` param; writer is injected, not constructed internally.
- **`Distiller::accept_event` signature** — Doc: `pub fn accept_event(self: &Arc<Self>, event: AgentEvent)` (fire-and-forget, no return). Actual: `pub async fn accept_event(&self, event: AgentEvent) -> Result<()>` (async, returns Result, clones self into spawned task).
- **`DistillerConfig` fields** — Doc: `cost_ceiling_usd`, `phase_b_timeout`, `phase_b_model`, `min_turn_tokens`. Actual: `model`, `max_input_tokens`, `timeout`, `idle_timeout`, `cost_ceiling_usd: Option<f64>`.
- **`DistillerWriter` API** — Doc: `add(fact: SemanticFact) -> Result<i64>` and `complete_supersede(predecessor_id: i64, successor: SemanticFact) -> Result<i64>`. Actual: `write_fact(prepared: PreparedFact) -> Result<(), DistillerError>` and `complete_supersede(predecessor_id: &str, successor_id: &str, successor_valid_from: &str) -> Result<(), DistillerError>`. Completely different interface (uses `PreparedFact`/`PreparedEpisode`, no `i64` ids).
- **`ReforgeWriter` API** — Doc signatures:
  - `reject_delete(&self, _id: i64) -> Result<()>`
  - `demote_stability(&self, id: i64) -> Result<()>`
  - `set_superseded_by(&self, predecessor_id: i64, successor_id: i64, valid_until: Timestamp) -> Result<()>`
  Actual signatures:
  - `reject_delete(&self, _table: &str, _reason: &str) -> Result<()>`
  - `demote_stability(&self, repo: &SemanticFactRepo, id: &str) -> Result<()>`
  - `set_superseded_by(&self, repo: &SemanticFactRepo, older_id: &str, newer_id: &str, at: Timestamp) -> Result<()>`
  All require a `repo` parameter and use `&str` ids instead of `i64`.
- **`SymbolExtractor` trait** — Doc: `#[async_trait] trait SymbolExtractor { async fn extract_symbols(&self, file_path: &Path, source: &str) -> Result<Vec<SymbolAnchor>>; }`. Actual: `trait SymbolExtractor: Send + Sync { fn extract(&self, path: &Path, source: &str, git_hash: &str) -> Vec<AnchoredSymbol>; }`. Not async, method name `extract`, extra `git_hash` param, no `Result` wrapper, return type `Vec<AnchoredSymbol>`.
- **`CodingRecallService` methods** — Doc shows all methods taking `args: Value`. Actual methods are strongly typed: e.g., `recall_index(&self, query: &str, repo: Option<&str>)`, `recall_timeline(&self, ids_or_query: RecallQuery, repo: Option<&str>)`, etc.
- **`RuleArtifactGenerationPhase` thresholds** — Doc says future behavior will use `confidence ≥ 0.7` and `stability ≥ 0.5`. Actual code in `reforge/rule_artifacts.rs` already queries `confidence >= 0.7` and `COALESCE(effectiveness_score, 0.5) >= 0.5`. The behavior is already implemented, not future.

#### ❌ Wrong
- **"Four Reforge phases all return `NotImplementedInPhase { required_phase: 5 }`"** — This is **false**. The actual wired implementations in `reforge/coding_synthesis.rs`, `reforge/rule_artifacts.rs`, `reforge/session_end.rs`, and `reforge/cross_session_dedup.rs` are **fully implemented** and actively used by `app-core::coding_memory::reforge::CodingPhaseRunnerImpl`. The old stubs in `reforge_phase.rs` are dead code, not the wired paths.
- **Distiller Phase A.5** — Doc claims a dedicated `phase_a5.rs` for tree-sitter refactor episodes. No such file exists; tree-sitter extraction is handled inside `symbols/extractor.rs` and used by the Distiller, but there is no distinct Phase A.5 file.
- **Observation types / `observation/` module** — Doc claims `observation/mod.rs` and `observation/reconcile.rs`. These do not exist. Reconciliation logic is in `distiller/phase_c.rs`.

#### 🔍 Missing
- The doc omits ~15 modules/files that exist in the actual crate (see Drift section above).
- The `symbols/` directory structure is completely mis-documented.

#### 📋 Tech Debt
- `reforge_phase.rs` contains dead-code stubs (`CodingSynthesisPhase`, `RuleArtifactGenerationPhase`) returning `Err(phase(5))`. These are **not** the wired implementations.
- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` found in active code.

---

## Cross-Reference Check

### `cognitive.md` ↔ `coding-ingest.md`
- `coding-ingest.md` correctly states it produces the `AgentEvent` stream consumed by `coding-memory::Distiller`.
- `cognitive.md` correctly references `coding-memory.md` for Reforge phases.

### `cognitive.md` ↔ `coding-memory.md`
- `cognitive.md` claims `CodingPhaseRunner::run_synthesis` and other hooks plug into `coding-memory`. Verified true in `app-core::coding_memory::reforge::CodingPhaseRunnerImpl`.
- `coding-memory.md` claims `ReforgeWriter` is the only sanctioned removal path. Verified true — `ReforgeWriter::reject_delete` errors, and bi-temporal supersede is the only removal path used.

### `coding-ingest.md` ↔ `coding-memory.md`
- `coding-ingest.md` says events flow to `distiller.accept_event(event)`. Verified in `app-core` daemon wiring.
- `coding-memory.md` describes the 3-phase Distiller consuming `AgentEvent`. Verified in `distiller/mod.rs`.

### Consistency issues across docs
- `coding-memory.md` describes stubbed Reforge phases, but `cognitive.md` describes them as wired hooks. The `cognitive.md` description is the accurate one; `coding-memory.md` is stale.
- `coding-ingest.md` claims 21 `EventKind` variants, but `coding-memory.md` does not mention the `GitCommit` variant. Neither doc is fully consistent with the 22-variant reality.
