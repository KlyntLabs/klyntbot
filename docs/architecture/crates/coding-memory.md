# Crate: `coding-memory`

> **Status:** 🟡 In Progress (Distiller live; 4 Reforge phases stubbed at `required_phase: 5`)
> **Subsystem:** [09 — Coding Mode](../subsystems/09-coding-mode.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The Distiller pipeline + 8 MCP recall tools + `ReforgeWriter` (only sanctioned removal path)

---

## TL;DR

The coding-mode memory layer. Consumes `AgentEvent` streams from `coding-ingest` and produces structured `episodic_memories` + `semantic_facts` via a **3-phase Distiller** (Phase A extractive → Phase B LLM synthesis → Phase C reconciliation). Owns the **`ReforgeWriter` — the only sanctioned removal path** (`reject_delete()` always errors; mutations are via `valid_until + superseded_by`). Exposes **8 MCP recall tools** to external clients. Performs tree-sitter symbol extraction for Phase A.5 refactor episodes.

Four Reforge phases owned by this crate (`CodingSynthesisPhase` 2.5, `RuleArtifactGenerationPhase` 3.5, `SessionEndPass`, `CrossSessionDedup`) all return `NotImplementedInPhase { required_phase: 5 }` — significant gap between Distiller (wired and active) and cross-session Reforge work.

---

## Module map

```
crates/coding-memory/src/
├── lib.rs                  ← Re-exports + crate doc
├── error.rs                ← NotImplementedInPhase + other errors
│
├── distiller/
│   ├── mod.rs              ← Distiller — fire-and-forget per-turn pipeline
│   ├── turn_buffer.rs      ← TurnBuffer (event accumulator)
│   ├── boundary.rs         ← TurnBoundary detection
│   ├── phase_a.rs          ← Phase A: extractive (always runs)
│   ├── phase_a5.rs         ← Phase A.5: tree-sitter refactor episodes
│   ├── phase_b.rs          ← Phase B: LLM synthesis (record_observation tool)
│   ├── phase_c.rs          ← Phase C: reconciliation (Add / Supersede / Noop)
│   └── writer.rs           ← DistillerWriter (logical-time supersede)
│
├── reforge/
│   ├── mod.rs              ← Re-exports
│   ├── writer.rs           ← ReforgeWriter (bi-temporal supersede; NO raw DELETE)
│   ├── coding_synthesis.rs ← CodingSynthesisPhase (2.5) — STUB
│   ├── rule_artifacts.rs   ← RuleArtifactGenerationPhase (3.5) — STUB
│   ├── session_end_pass.rs ← SessionEndPass — STUB
│   └── cross_session_dedup.rs ← CrossSessionDedup — STUB
│
├── recall/
│   ├── mod.rs              ← CodingRecallService
│   └── …                   ← Query implementations for each recall tool
│
├── mcp.rs                  ← CodingMemoryToolset + 8 MCP tool impls
│
├── symbols/
│   ├── mod.rs              ← SymbolExtractor trait
│   ├── tree_sitter.rs      ← TreeSitterExtractor (Rust/TS/JS/Python/Go grammars)
│   └── anchors.rs          ← Symbol anchor types
│
├── observation/
│   ├── mod.rs              ← Observation types
│   └── reconcile.rs        ← Phase C decision logic
│
└── retry/
    └── mod.rs              ← DistillationRetryRepo accessor
```

---

## Public API surface

### `Distiller`

```rust
pub struct Distiller {
    cognitive_provider: DynProvider,
    symbol_extractor: Arc<dyn SymbolExtractor>,
    semantic_repo: SemanticFactRepo,
    episodic_repo: EpisodicMemoryRepo,
    distiller_writer: DistillerWriter,
    retry_repo: DistillationRetryRepo,
    cost_ceiling_usd: f64,
    // ...
}

impl Distiller {
    pub fn new(
        cognitive_provider: DynProvider,
        symbol_extractor: Arc<dyn SymbolExtractor>,
        repos: &Repos,
        config: DistillerConfig,
    ) -> Self;

    /// Fire-and-forget — failures never propagate to the caller.
    pub fn accept_event(self: &Arc<Self>, event: AgentEvent);
}

pub struct DistillerConfig {
    pub cost_ceiling_usd: f64,
    pub phase_b_timeout: Duration,        // default: 30s
    pub phase_b_model: String,            // default: "claude-haiku-4-5-20251001"
    pub min_turn_tokens: u32,
    // ...
}
```

### Distiller pipeline

```
Distiller::accept_event(event)
   ↓
TurnBuffer.push(event)
   ↓
on TurnBoundary::Fire:
   tokio::spawn(distill_turn())     ← fire-and-forget; failures logged as warnings
       ↓
   Phase A — phase_a::compute_turn_trace(events) → TurnTrace
       Always runs (extractive)
       Persists EpisodicMemory of kind "turn_trace"

   Phase A.5 — tree-sitter anchored refactor episode
       Persists EpisodicMemory of kind "refactor_episode"

   Phase B — phase_b::invoke_llm(events, cognitive_provider)
       Default model: claude-haiku-4-5-20251001
       Timeout: 30s
       Cost-ceiling guard: blocks Phase B if exceeded
       Returns Vec<Observation> via record_observation tool
       Transient failures → enqueue to DistillationRetryRepo

   Phase C — phase_c::reconcile(observation) per observation
       Returns one of:
         Add                       → write fresh row
         Supersede { predecessor } → DistillerWriter::complete_supersede (logical-time)
         Noop                      → skip
       Auto-derive: failed FixAttempt → DeadEndAttempt counterfactual fact
```

### `ReforgeWriter` — the only sanctioned removal path

```rust
pub struct ReforgeWriter {
    semantic_repo: SemanticFactRepo,
    episodic_repo: EpisodicMemoryRepo,
}

impl ReforgeWriter {
    pub fn new(semantic_repo: SemanticFactRepo, episodic_repo: EpisodicMemoryRepo) -> Self;

    /// ALWAYS returns an error — no raw DELETE is allowed.
    pub async fn reject_delete(&self, _id: i64) -> Result<()> {
        Err(KlyntbotError::PermissionDenied("raw DELETE rejected".into()))
    }

    /// Demote stability — sets convergence_score → 0.01.
    pub async fn demote_stability(&self, id: i64) -> Result<()>;

    /// Bi-temporal supersede: sets valid_until + superseded_by.
    /// BOTH rows remain on disk permanently.
    pub async fn set_superseded_by(
        &self,
        predecessor_id: i64,
        successor_id: i64,
        valid_until: Timestamp,
    ) -> Result<()>;
}
```

**No physical DELETE ever runs through this writer.** Two distinct "supersede" paths exist:

| Path | Where | Semantics | When |
|---|---|---|---|
| `DistillerWriter::complete_supersede` | Distiller Phase C | Logical-time: sets `superseded_at` + `superseded_by` | Within-session, per-turn reconciliation |
| `ReforgeWriter::set_superseded_by` | Reforge phases | Bi-temporal: sets `valid_until` + `superseded_by` | Cross-session, batch dedup |

Both keep all rows on disk.

### `DistillerWriter` (within-session)

```rust
pub struct DistillerWriter { /* opaque */ }

impl DistillerWriter {
    pub async fn add(&self, fact: SemanticFact) -> Result<i64>;

    /// Logical-time supersede: sets superseded_at + superseded_by.
    /// Different from ReforgeWriter::set_superseded_by (bi-temporal).
    pub async fn complete_supersede(
        &self,
        predecessor_id: i64,
        successor: SemanticFact,
    ) -> Result<i64>;
}
```

### Reforge phases (all 🔴 stubbed)

```rust
pub struct CodingSynthesisPhase { /* deps */ }

#[async_trait]
impl ReforgePhase for CodingSynthesisPhase {
    async fn run(&self, ctx: PhaseContext) -> Result<PhaseResult> {
        Err(KlyntbotError::NotImplemented(
            format!("NotImplementedInPhase {{ required_phase: 5 }}")
        ))
    }
}

// Same shape for:
//   - RuleArtifactGenerationPhase (3.5)
//   - SessionEndPass
//   - CrossSessionDedup
```

**`RuleArtifactGenerationPhase` future behavior** (per code comments): when implemented, will read patterns and preferences with `confidence ≥ 0.7` AND `stability ≥ 0.5`, then write managed blocks into:
- `CLAUDE.md`
- `AGENTS.md`
- `.cursorrules`
- `.continue/rules/klyntbot.md`

### `CodingRecallService` + MCP toolset

```rust
pub struct CodingRecallService {
    semantic_repo: SemanticFactRepo,
    episodic_repo: EpisodicMemoryRepo,
    fact_changelog_repo: FactChangelogRepo,
    // ...
}

impl CodingRecallService {
    pub fn new(repos: &Repos) -> Self;

    pub async fn recall_index(&self, args: Value) -> Result<String>;
    pub async fn recall_timeline(&self, args: Value) -> Result<String>;
    pub async fn recall_fetch(&self, args: Value) -> Result<String>;
    pub async fn trace_causes(&self, args: Value) -> Result<String>;
    pub async fn check_dead_ends(&self, args: Value) -> Result<String>;
    pub async fn recall_facts_as_of(&self, args: Value) -> Result<String>;
    pub async fn recall_change_history(&self, args: Value) -> Result<String>;
    pub async fn recall_decision_points(&self, args: Value) -> Result<String>;
}

pub struct CodingMemoryToolset {
    service: Arc<CodingRecallService>,
}

impl CodingMemoryToolset {
    pub fn new(service: Arc<CodingRecallService>) -> Self;

    pub async fn dispatch(&self, tool_name: &str, args: Value) -> Result<String>;
}

pub const CODING_MEMORY_MCP_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];
```

The 8 MCP recall tools auto-added to MCP exposure via `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs`.

### `SymbolExtractor`

```rust
#[async_trait]
pub trait SymbolExtractor: Send + Sync {
    async fn extract_symbols(&self, file_path: &Path, source: &str) -> Result<Vec<SymbolAnchor>>;
}

pub struct SymbolAnchor {
    pub kind: SymbolKind,                 // Function | Class | Module | …
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub enum SymbolKind {
    Function, Method, Class, Struct, Trait, Enum, Module, Constant, Variable,
}

pub struct TreeSitterExtractor { /* tree-sitter parsers */ }

impl TreeSitterExtractor {
    pub fn new() -> Self;
    // Supports: Rust, TypeScript, JavaScript, Python, Go
}

#[async_trait]
impl SymbolExtractor for TreeSitterExtractor {
    async fn extract_symbols(&self, file_path: &Path, source: &str) -> Result<Vec<SymbolAnchor>>;
}
```

Used by Phase A.5 (refactor episode anchors) + Phase C (FixAttempt counterfactual derivation).

### Observation types

```rust
pub struct Observation {
    pub kind: ObservationKind,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source_message_ids: Vec<String>,
    pub anchors: Vec<SymbolAnchor>,
}

pub enum ObservationKind {
    Pattern,              // recurring code pattern
    Preference,           // user preference
    FixAttempt,           // attempted fix (may succeed or fail)
    DeadEndAttempt,       // auto-derived from failed FixAttempt
    ProblemClass,         // class of problem encountered
    Convention,           // codebase convention
    AntiPattern,          // identified anti-pattern
}

pub enum ReconciliationDecision {
    Add,
    Supersede { predecessor_id: i64 },
    Noop,
}
```

---

## Internals

### Fire-and-forget Distiller

```rust
// crates/coding-memory/src/distiller/mod.rs
impl Distiller {
    pub fn accept_event(self: &Arc<Self>, event: AgentEvent) {
        let this = self.clone();
        if let TurnBoundary::Fire = this.turn_buffer.push(event) {
            tokio::spawn(async move {
                if let Err(e) = this.distill_turn().await {
                    tracing::warn!(error = ?e, "distill_turn failed");
                    // Failures never propagate to the caller.
                }
            });
        }
    }
}
```

The Distiller cannot block the ingestion path or the user-facing coding session. **Failure isolation is load-bearing.**

### Phase A vs Phase A.5

| | Phase A | Phase A.5 |
|---|---|---|
| Always runs | ✅ | If file edits in turn |
| Cost | Free (extractive) | Free (tree-sitter) |
| Output | `TurnTrace` (files read/modified, commands, test outcomes, errors, token usage) | Symbol anchors for edited regions |
| Persisted as | `EpisodicMemory { kind: "turn_trace" }` | `EpisodicMemory { kind: "refactor_episode" }` |

### Phase B cost-ceiling guard

```rust
// Inside phase_b::invoke_llm
if current_cycle_cost_usd >= self.config.cost_ceiling_usd {
    tracing::warn!(cost = current_cycle_cost_usd, "Phase B skipped — cost ceiling");
    return Ok(Vec::new());
}

// Otherwise proceed with the LLM call:
let response = self.provider.chat_completion(
    messages,
    ChatParams {
        model: self.config.phase_b_model.clone(),
        temperature: 0.2,
        max_tokens: 2048,
        tools: Some(vec![record_observation_tool()]),
        ..Default::default()
    }
).await?;
```

### Phase C reconciliation

```rust
// Per observation:
match phase_c::reconcile(&observation, &existing_facts).await? {
    ReconciliationDecision::Add => {
        distiller_writer.add(fact).await?;
    }
    ReconciliationDecision::Supersede { predecessor_id } => {
        distiller_writer.complete_supersede(predecessor_id, successor).await?;
    }
    ReconciliationDecision::Noop => { /* skip */ }
}

// Auto-derive: failed FixAttempt → DeadEndAttempt
if observation.kind == ObservationKind::FixAttempt && observation.failed() {
    let dead_end = derive_dead_end(observation);
    distiller_writer.add(dead_end).await?;
}
```

### `DistillationRetryRepo` for transient failures

When Phase B fails with a transient error (network timeout, provider 5xx), the events are enqueued to `DistillationRetryRepo` rather than dropped. A background sweep retries them later. Permanent failures (e.g., schema mismatch) are not retried.

### Tree-sitter grammars

| Language | Grammar |
|---|---|
| Rust | `tree-sitter-rust` |
| TypeScript | `tree-sitter-typescript` |
| JavaScript | `tree-sitter-javascript` |
| Python | `tree-sitter-python` |
| Go | `tree-sitter-go` |

Adding a language: add the grammar dep + the parser-init code in `TreeSitterExtractor::new()`.

### Why two supersede paths (DistillerWriter vs ReforgeWriter)

The Distiller operates **within a session** — it sees a stream of events from one user's one coding session and reconciles them against the existing fact store. Logical-time supersede (`superseded_at` + `superseded_by`) is sufficient — the timestamps record the order of within-session events.

Reforge operates **across sessions** — it consolidates the entire fact store on a nightly batch. Bi-temporal supersede (`valid_until` + `superseded_by`) captures both:
- *When the fact stopped being valid* (`valid_until`)
- *What replaced it* (`superseded_by`)

Bi-temporal is more expressive but slower per write — appropriate for batch.

---

## Workflows

### A coding turn drives the Distiller

```
1. User sends coding message; agent runs tools
2. coding-ingest emits AgentEvent (UserPrompt, ToolCall, FileEdit, AssistantMsg, …)
3. Each event → distiller.accept_event(event)
4. distiller.turn_buffer accumulates
5. On TurnBoundary::Fire (e.g., SessionEnd or new UserPrompt):
   tokio::spawn(distill_turn()):
      a. Phase A: compute_turn_trace → write "turn_trace" EpisodicMemory
      b. Phase A.5 (if file edits): tree-sitter anchors → write "refactor_episode"
      c. Phase B (if under cost ceiling): LLM synthesis → Vec<Observation>
      d. Phase C: per observation, reconcile + write or supersede
      e. (If transient Phase B failure): enqueue to DistillationRetryRepo
6. Ingestion path continues unblocked
```

### MCP client invokes `recall_timeline`

```
1. Claude Code MCP client calls tools/call { name: "recall_timeline", arguments: {...} }
2. KlyntbotServerHandler routes to ToolRegistryBridge
3. ToolRegistryBridge dispatches to CodingMemoryToolset
4. CodingMemoryToolset.dispatch("recall_timeline", args)
5. CodingRecallService.recall_timeline(args)
   → queries episodic_memories within time window
   → groups by session_id
   → formats as markdown timeline
6. Result returned as CallToolResult::success(vec![Content::text(result)])
```

### Reforge phase invocation (when not stubbed)

```
// In services/reforge/service.rs::run_reforge (cognitive)
if let Some(runner) = coding_phase_runner {
    runner.run_synthesis(/* args */).await?;
}

// In app-core::adapters::CodingPhaseRunnerImpl
async fn run_synthesis(&self, args: ...) -> Result<()> {
    self.coding_memory_phase_runner.synthesis_phase.run(ctx).await?;
}

// In coding-memory::reforge::coding_synthesis::CodingSynthesisPhase
async fn run(&self, ctx: PhaseContext) -> Result<PhaseResult> {
    Err(KlyntbotError::NotImplemented(
        "NotImplementedInPhase { required_phase: 5 }".into()
    ))
}
// ↑ Currently a stub. When implemented, will consume sessions + FixAttempts
//   + causal edges and emit ExtractPattern / PromoteToProblemClass.
```

---

## Testing approach

### In-memory pool + cognitive migrations

```rust
let pool = StoragePool::connect_in_memory().await.unwrap();
// Apply cognitive migrations (includes semantic_facts, episodic_memories, etc.)
cognitive::repos::cognitive_migrations()
    .into_iter()
    .for_each(|m| sqlx::query(&m.sql).execute(pool.inner()).await.unwrap());
```

### Test Distiller with a mock provider

```rust
let provider: DynProvider = Box::new(
    NoopProvider::new().with_response(LlmResponse {
        tool_calls: vec![ToolCall {
            id: "1".into(),
            name: "record_observation".into(),
            arguments: serde_json::json!({
                "observations": [{"kind": "Pattern", "subject": "...", ...}]
            }),
        }],
        ..Default::default()
    })
);

let distiller = Arc::new(Distiller::new(
    provider,
    Arc::new(NoopSymbolExtractor),
    &repos,
    DistillerConfig::default(),
));

distiller.accept_event(AgentEvent::SessionStart { ... });
distiller.accept_event(AgentEvent::UserPrompt { ... });
distiller.accept_event(AgentEvent::SessionEnd { ... });

// Wait for fire-and-forget tasks
tokio::time::sleep(Duration::from_millis(100)).await;

let stored = repos.semantic_fact.count_active().await.unwrap();
assert!(stored > 0);
```

### Test `ReforgeWriter::reject_delete`

```rust
let writer = ReforgeWriter::new(semantic_repo, episodic_repo);
let result = writer.reject_delete(123).await;
assert!(matches!(result, Err(KlyntbotError::PermissionDenied(_))));
```

### Test bi-temporal supersede

```rust
let predecessor_id = semantic_repo.upsert(fact_v1).await.unwrap();
let successor_id = semantic_repo.upsert(fact_v2).await.unwrap();

writer.set_superseded_by(
    predecessor_id, successor_id, Timestamp::now(),
).await.unwrap();

// Predecessor still on disk
assert!(semantic_repo.find(predecessor_id).await.unwrap().is_some());
// But valid_until is set
let fetched = semantic_repo.find(predecessor_id).await.unwrap().unwrap();
assert!(fetched.valid_until.is_some());
assert_eq!(fetched.superseded_by, Some(successor_id));
```

### Test stub phases

```rust
let phase = CodingSynthesisPhase::new(/* deps */);
let result = phase.run(ctx).await;
assert!(matches!(result, Err(KlyntbotError::NotImplemented(_))));
```

---

## Extension points

### Add a recall tool

1. Add a method to `CodingRecallService::my_new_tool(args: Value) -> Result<String>`.
2. Add the tool name to `CODING_MEMORY_MCP_TOOLS` constant.
3. Add dispatch arm in `CodingMemoryToolset::dispatch`.
4. Add to `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs` if it should be MCP-exposed.

### Add an observation kind

1. Add variant to `ObservationKind` enum.
2. Update `phase_b` LLM prompt to include the new kind in `record_observation` tool schema.
3. Update `phase_c::reconcile` if the new kind has special reconciliation semantics.
4. Update MCP recall tools that filter by kind.

### Add a `SymbolExtractor` impl

```rust
struct MyExtractor;

#[async_trait]
impl SymbolExtractor for MyExtractor {
    async fn extract_symbols(&self, file_path: &Path, source: &str) -> Result<Vec<SymbolAnchor>> {
        // Use any parser
        Ok(vec![])
    }
}
```

Inject into `Distiller::new(.., Arc::new(MyExtractor), ..)`.

### Implement a stubbed Reforge phase

1. Remove the `Err(NotImplemented(...))` return in `coding-memory/src/reforge/<phase>.rs`.
2. Implement the actual logic.
3. Update `app-core::adapters::CodingPhaseRunnerImpl` to wire it.
4. Verify via reforge integration test in `kca-e2e`.

### Add a tree-sitter grammar

```toml
# crates/coding-memory/Cargo.toml
tree-sitter-ruby = "0.20"
```

```rust
// crates/coding-memory/src/symbols/tree_sitter.rs
let mut ruby_parser = Parser::new();
ruby_parser.set_language(tree_sitter_ruby::language()).unwrap();
self.parsers.insert("rb".to_string(), ruby_parser);
```

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| Default Distiller `phase_b_model` | `"claude-haiku-4-5-20251001"` | `distiller/mod.rs` |
| Default Distiller `phase_b_timeout` | `30s` | `distiller/mod.rs` |
| Phase B temperature | `0.2` | `phase_b.rs` |
| Phase B max_tokens | `2048` | `phase_b.rs` |
| Reforge phase `required_phase` | `5` | `reforge/{coding_synthesis,rule_artifacts,session_end_pass,cross_session_dedup}.rs` |
| `CODING_MEMORY_MCP_TOOLS` count | `8` | `mcp.rs` |
| `RuleArtifactGenerationPhase` thresholds (future) | `confidence ≥ 0.7`, `stability ≥ 0.5` | `reforge/rule_artifacts.rs` |

---

## Open questions

- **4 Reforge phases all stubbed** at `required_phase: 5`. Implement or document a release plan.
- **Distiller default model is hardcoded** (`claude-haiku-4-5-20251001`). Should be `config.coding.distiller_provider`.
- **`NotImplementedInPhase` is a magic-string error** — codify as a dedicated error variant.
- **Two supersede paths exist** (`DistillerWriter` logical-time vs `ReforgeWriter` bi-temporal). Rename methods to make the distinction obvious (`complete_supersede` vs `set_superseded_by` — both reference "supersede").
- **`RuleArtifactGenerationPhase` will write managed blocks** into `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.continue/rules/klyntbot.md` — needs idempotency story (don't duplicate blocks on re-run).
- **Distiller failure isolation is good but observability is low** — `tracing::warn!` is the only signal. Add metric (e.g., `distillation_failures_total`).
- **Tree-sitter grammar version pins** are spread across `Cargo.toml`. Consolidate into workspace deps.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 + #8 for specifics.

---

## Cross-references

- [Subsystem 09 — Coding Mode](../subsystems/09-coding-mode.md) (parent)
- [`crates/coding-ingest.md`](./coding-ingest.md) — produces the `AgentEvent` stream
- [`crates/cognitive.md`](./cognitive.md) — Reforge phases plug into hooks defined there; `SemanticFactRepo`/`EpisodicMemoryRepo` live there
- [`crates/storage.md`](./storage.md) — repo definitions
- [`crates/providers.md`](./providers.md) — `cognitive_provider` for Phase B
- [`crates/app-core.md`](./app-core.md) — `init/coding_recall.rs` wires the recall service
- [`crates/mcp.md`](./mcp.md) *(planned)* — MCP server exposes the 8 recall tools
