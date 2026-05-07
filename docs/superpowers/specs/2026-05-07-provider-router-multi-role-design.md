# ProviderRouter — Role-Keyed Multi-Provider Routing

- **Status:** Draft (approved in brainstorm 2026-05-07)
- **Author:** Jayden + Claude (Opus 4.7, explanatory mode)
- **Type:** Design / Spec
- **Scope:** `crates/providers`, `crates/config`, `crates/app-core`, `crates/agent`, `crates/cognitive`, `crates/coding-memory`
- **Estimated effort:** 2–3 days, ~900 LOC net change including tests + ~50 call-site updates
- **Implementation handoff:** writing-plans skill

---

## 1. Motivation

Klynt today resolves LLM providers through two parallel paths that don't share routing infrastructure:

1. The **agent path** — `create_provider_with_failover_full` returns a `DynProvider` that *is* a `ProviderManager` underneath when a fallback is configured. It carries primary + fallback + classifier slots, retry/backoff, and a circuit breaker.
2. The **cognitive path** — `create_cognitive_provider` returns a **bare** `DynProvider`. No manager, no failover, no circuit breaker.

A scan of the workspace surfaced ~50 LLM call sites. ~40 of them hold the bare cognitive provider. **None of those 40 sites have failover protection**, even though the infrastructure for it exists in the agent path. If the cognitive provider's API hiccups, the entire memory pipeline (extraction, consolidation, graph linking, reforge, mirror, notes generation, practice, grading, language learning, auto-titles) fails open.

The system has the seams of a role-aware router but none of the wiring:

- A `ProviderRole` enum exists with three variants (`Distiller`, `ReforgeSynth`, `ReforgeRules`).
- `ChatParams.role: Option<ProviderRole>` is plumbed through every adapter.
- `ProviderManager::chat_with_role` exists but is **a stub** — it stamps `params.role` and routes to the same primary→fallback chain.
- `chat_with_role` is an **inherent method** on `ProviderManager`, not on the `LlmProvider` trait, so any call site holding a `DynProvider` cannot reach it without downcasting.

Of the 50 sites scanned, only 2 stamp a role (`Distiller` at `coding-memory/src/distiller/phase_b.rs:174`, `ReforgeRules` at `coding-memory/src/skills.rs:212`). Both stampings are accepted but ignored by the manager.

This spec unifies both paths behind a single `ProviderRouter` abstraction with role-keyed dispatch, gives every call site failover for free, and sets up the structural seams needed for the Phase 2 multi-model composition work (planner+coder+critic, cascade-on-failure, etc.) without specifying that work here.

We are pre-release (per `CLAUDE.md`) — there is no user data or external config to migrate. We can hard-cut the schema.

## 2. Goals and non-goals

### Goals

1. **Single injected handle.** Every LLM-calling struct holds `Arc<dyn ProviderRouter>` instead of a bare `DynProvider` or a separate cognitive provider.
2. **Role-keyed dispatch.** Each call site declares the *kind of work* it's doing via a `ProviderRole`. The router picks the chain.
3. **Failover for the cognitive pipeline.** All ~40 cognitive sites get retry, fallback, and circuit-breaker protection through the same `ProviderManager` machinery the agent path already uses.
4. **Minimum-viable config.** The smallest sensible config is two chains (`agent_default` + `cognitive_default`) plus the existing `providers.*` API keys. Per-role overrides are entirely optional.
5. **Compile-time exhaustiveness.** Closed enum, not open strings. Refactors are caught by the compiler; telemetry buckets are stable.
6. **No change to `LlmProvider`.** Every existing adapter (Anthropic native, OpenAI-compatible, Noop, future ones) keeps working without modification.
7. **Phase 2 seams.** The router exposes `chain_for(role)` returning a single `LlmProvider`-shaped chain so future composition workflows can call multiple chains without depending on router internals.

### Non-goals

1. **Phase 2 composition primitives** (cascade, race, pipeline, voting, planner+coder+critic). Sketched in §10; designed in a follow-up spec.
2. **Backwards-compatibility shims** for the legacy `provider_manager.fallback` / `provider_manager.classifier_model` config keys. Pre-release: hard cutover.
3. **Per-message provider selection by user.** Routing is config-driven, not message-driven.
4. **Dynamic role registration.** Closed enum, compile-time only.
5. **Per-role cost budgets, rate limits, or per-call price ceilings.** Future work; the design doesn't preclude them but doesn't ship them.
6. **OpenTelemetry / Prometheus metrics per role.** Per `CLAUDE.md` non-goals; we tag existing `tracing` events with role and stop there.

## 3. Decisions locked in (from brainstorm 2026-05-07)

| Concern | Decision |
|---|---|
| Sequencing | Phase 1 = unify the two lineages. Phase 2 (multi-model composition) is future work — sketched but not designed here. |
| Role model | **Closed enum**, not open strings. Compile-time exhaustive, IDE-rename-safe, matches the codebase's domain-enum convention. |
| Unification shape | **Single `ProviderRouter`** replacing both `provider` and `cognitive_provider`. Internally a `HashMap<ProviderRole, Arc<ProviderManager>>` plus two named default chains. |
| Default policy | Two named default chains: `agent_default` and `cognitive_default`. Each role declares which it falls back to via a `default_chain()` method. Per-role override is optional. |
| Tier abstraction | **Removed.** With only one Light role (`Cognitive`), tiering collapses to a single match arm in `default_chain()`. No `Tier` enum. |
| Role count | 9 variants total (8 Heavy + 1 Cognitive catch-all) — significantly smaller than the 22-role first draft, motivated by config UX. |
| `ProviderManager` | Stays as the **per-chain** implementation. Its existing failover, retry, circuit-breaker, and persistence logic are reused unchanged. |
| Trait surface | New `ProviderRouter` trait above `LlmProvider`. `LlmProvider` is unchanged. |
| Legacy config keys | `provider_manager.fallback` and `provider_manager.classifier_model` are **removed**. Pre-release; no migration shim. |
| Autotuner provider | **Bug fix in flight.** Currently uses the expensive primary; switches to `Cognitive` (light) tier. |
| Dead code | `app-core/init/coaching.rs:34`'s unused `_cognitive_provider` parameter is removed. `LlmCommunityMembershipHandler` (no production caller found) is verified and either deleted or wired during migration. |

## 4. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                         AppCore                              │
│                                                              │
│   router: Arc<dyn ProviderRouter>   ◄── ONE handle           │
│                                                              │
└────────────────┬─────────────────────────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────────────────────────┐
│              DefaultProviderRouter                           │
│                                                              │
│   chains: HashMap<ProviderRole, Arc<ProviderManager>>        │
│   agent_default:     Arc<ProviderManager>                    │
│   cognitive_default: Arc<ProviderManager>                    │
│                                                              │
│   chat(role, …) {                                            │
│     let chain = self.chains.get(&role)                       │
│       .cloned()                                              │
│       .unwrap_or_else(|| self.named(role.default_chain()));  │
│     chain.chat(messages, tools, params, breakpoints).await   │
│   }                                                          │
└────────────────┬─────────────────────────────────────────────┘
                 │
                 ▼
       ┌─────────────────────────────────────────────────┐
       │  ProviderManager (existing — one per chain)     │
       │                                                 │
       │  primary:   DynProvider                         │
       │  fallback:  Option<DynProvider>                 │
       │  retry + backoff + circuit breaker              │
       │  on_circuit_open / on_provider_degraded         │
       └─────────────────────────────────────────────────┘
```

`DynProvider` and `LlmProvider` are unchanged. Each chain is a `ProviderManager` with its own circuit breaker. Failure of one chain does not trip another (e.g., a `ReforgeSynth` Anthropic outage does not open the `Cognitive` Groq breaker).

## 5. `ProviderRole` enum

```rust
// crates/providers/src/lib.rs (replaces existing 3-variant enum)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    // -- Heavy: quality-critical; named for individual customization --

    /// Main agent loop; fallback for any unrolled site.
    Default,

    /// Reforge Phase 2 — synthesize new rules / observations.
    ReforgeSynth,

    /// Reforge Phase 3 — generate rule artifacts (CLAUDE.md / AGENTS.md / SKILL.md).
    ReforgeRules,

    /// Reforge Phase 3 — review/critique synthesized rules.
    ReforgeReview,

    /// Notes/insight generation: streaming generation, regenerate, scenario challenges,
    /// changes-summary between insight versions.
    NotesGen,

    /// Coaching reasoner — generates coaching interventions.
    Coach,

    /// Memory consolidation: per-turn consolidation, deep cross-session consolidation,
    /// micro-reforge.
    Consolidate,

    /// Knowledge-graph operations: graph link, graph enrichment, community intelligence,
    /// community membership inference.
    GraphLink,

    // -- Light: single catch-all for cheap, frequent, latency-sensitive calls --

    /// Cheap/fast tier. Includes:
    ///   - distiller (per-turn, high-frequency)
    ///   - session title autogen
    ///   - productivity activity classifier, complexity classifier
    ///   - LLM rerank, query rewriter, multi-query expansion
    ///   - batch summary, hierarchical summarizer, reforge narrate (prose)
    ///   - episodic/semantic extraction, atom extraction, session memory
    ///   - extraction critic
    ///   - query predictor (predictive cache)
    ///   - temporal pruner
    ///   - mirror narrative
    ///   - autotuner trial generation (bug-fix migration from primary)
    ///   - notes practice/distractor/language gen, notes grading
    ///   - cross-CLI synthesis, skill discovery
    Cognitive,
}

impl ProviderRole {
    /// Name of the default chain to use when this role is not explicitly overridden.
    pub fn default_chain(self) -> &'static str {
        match self {
            Self::Cognitive => "cognitive_default",
            _ => "agent_default",
        }
    }

    /// All variants — used by config validation and telemetry.
    pub const ALL: &'static [Self] = &[
        Self::Default,
        Self::ReforgeSynth,
        Self::ReforgeRules,
        Self::ReforgeReview,
        Self::NotesGen,
        Self::Coach,
        Self::Consolidate,
        Self::GraphLink,
        Self::Cognitive,
    ];
}
```

**Why these 9 (and not 22):** The earlier draft enumerated ~22 roles, one per call-site purpose. User feedback (correct, in our judgment) was that the Light-tier customization granularity is fictional surface area — nobody will realistically point `Rerank`, `Rewrite`, `Predictor`, and `Pruner` at four different cheap models. Collapsing all Light-tier work into a single `Cognitive` role keeps the *common-case config* trivial (two chains, no role overrides) while preserving per-role customization for the 8 Heavy roles where it matters. If a future power user wants to pin one specific Light operation to its own model, splitting one variant out of `Cognitive` is a one-call-site, one-enum-variant edit.

## 6. Trait & module surface

```rust
// crates/providers/src/router.rs (new)

use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;

use crate::manager::ProviderManager;
use crate::types::{
    CacheBreakpoint, ChatParams, LlmResponse, LlmStream, Message,
};
use crate::ProviderRole;
use common::Result;

/// Role-keyed dispatcher above `LlmProvider`. Resolves a chain per role
/// and delegates to the underlying `ProviderManager`.
#[async_trait]
pub trait ProviderRouter: Send + Sync {
    async fn chat(
        &self,
        role: ProviderRole,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse>;

    async fn chat_stream(
        &self,
        role: ProviderRole,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream>;

    /// Resolve the chain for a role. Used by:
    ///   - Phase 2 composition workflows that hold a chain across multiple calls
    ///   - Telemetry / cost-accounting
    ///   - Tests that need to assert a specific chain is hit
    fn chain_for(&self, role: ProviderRole) -> Arc<ProviderManager>;

    /// Default model name for a role; stamped into `ChatParams.model`
    /// at the call site when the call site doesn't override it.
    fn default_model(&self, role: ProviderRole) -> String;
}

pub struct DefaultProviderRouter {
    chains: HashMap<ProviderRole, Arc<ProviderManager>>,
    agent_default: Arc<ProviderManager>,
    cognitive_default: Arc<ProviderManager>,
}

impl DefaultProviderRouter {
    pub fn new(
        agent_default: Arc<ProviderManager>,
        cognitive_default: Arc<ProviderManager>,
        overrides: HashMap<ProviderRole, Arc<ProviderManager>>,
    ) -> Self {
        Self {
            chains: overrides,
            agent_default,
            cognitive_default,
        }
    }

    fn resolve(&self, role: ProviderRole) -> Arc<ProviderManager> {
        if let Some(chain) = self.chains.get(&role) {
            return chain.clone();
        }
        match role.default_chain() {
            "cognitive_default" => self.cognitive_default.clone(),
            _ => self.agent_default.clone(),
        }
    }
}

#[async_trait]
impl ProviderRouter for DefaultProviderRouter {
    async fn chat(
        &self, role: ProviderRole, messages: &[Message],
        tools: Option<&[Value]>, params: &ChatParams, cb: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        let chain = self.resolve(role);
        let mut params = params.clone();
        params.role = Some(role);
        chain.chat(messages, tools, &params, cb).await
    }

    async fn chat_stream(
        &self, role: ProviderRole, messages: &[Message],
        tools: Option<&[Value]>, params: &ChatParams, cb: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        let chain = self.resolve(role);
        let mut params = params.clone();
        params.role = Some(role);
        chain.chat_stream(messages, tools, &params, cb).await
    }

    fn chain_for(&self, role: ProviderRole) -> Arc<ProviderManager> {
        self.resolve(role)
    }

    fn default_model(&self, role: ProviderRole) -> String {
        self.resolve(role).default_model().to_string()
    }
}
```

**`LlmProvider` is unchanged.** `ProviderManager` keeps its existing inherent `chat_with_role` method (now redundant but harmless; deleted in a small follow-up cleanup once no caller remains).

The factory layer (`crates/providers/src/factory.rs`) gains a new entry point:

```rust
pub fn create_router(config: &Config) -> Result<Arc<dyn ProviderRouter>>;
```

`create_provider_with_failover_full` and `create_cognitive_provider` are **deleted** in the same PR — no caller remains after the migration in §8. `create_provider` (single-provider, no manager) is retained for the `NoopProvider` setup-wizard path.

## 7. Configuration schema

New top-level block `router` replaces `provider_manager`:

```jsonc
{
  "router": {
    "agent_default": {
      "primary": "anthropic",
      "fallbacks": ["openai"]
    },
    "cognitive_default": {
      "primary": "groq",
      "fallbacks": ["deepseek"]
    },

    // Optional. Most users will leave this empty.
    "roles": {
      "reforge_synth": { "primary": "anthropic" },
      "notes_gen":     { "primary": "anthropic", "fallbacks": ["openai"] }
    },

    "circuit_breaker": {
      "failure_threshold": 5,
      "reset_timeout_secs": 60
    }
  }
}
```

### Type schema

```rust
// crates/config/src/schema/router.rs (new; replaces provider_manager.rs)

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouterConfig {
    pub agent_default:     ChainConfig,
    pub cognitive_default: ChainConfig,

    #[serde(default)]
    pub roles: HashMap<ProviderRole, ChainConfig>,

    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChainConfig {
    /// Provider name from `providers.*` (e.g., "anthropic", "openai", "groq").
    pub primary: String,
    /// Optional ordered list of fallback provider names.
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Optional model override; defaults to the provider's registry default.
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub reset_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self { failure_threshold: 5, reset_timeout_secs: 60 }
    }
}
```

`ChainConfig` references the existing `providers.*` block by name — no duplication of API keys or base URLs. `ProviderRole` is the same enum as in §5; serde uses `snake_case` keys so config reads naturally.

### Validation

- `agent_default.primary` and `cognitive_default.primary` must reference providers with non-empty API keys, **or** the `NoopProvider` is substituted with a single startup warning (matches existing `create_provider_with_failover_full` graceful behavior for setup-wizard mode).
- Fallback names that don't have an API key are dropped at startup with `tracing::warn!` and the chain continues without that fallback.
- `roles` keys are deserialized into `ProviderRole` via `#[serde(rename_all = "snake_case")]`; unknown keys fail loudly via `deny_unknown_fields`.

### Removed config

- `config.provider_manager` — entire block deleted.
- `config.cognitive.provider` — folded into `router.cognitive_default.primary`.
- `config.cognitive.model` — folded into `router.cognitive_default.model`.
- All per-handler model keys (`graph_linker_model`, `critic_model`, `temporal_prune_model`, `predictive_cache.model`, `micro_reforge.model`, `hierarchical.model`, `coding_memory.reforge.synth_model`, `coding_memory.reforge.rules_model`) — folded into the corresponding `roles.<name>.model` overrides where relevant. Most of these collapse into `roles.consolidate.model`, `roles.graph_link.model`, `roles.cognitive.model`. Several disappear entirely because they were never user-set in practice.

This is a hard schema cut. Existing dev `config.json` files need a manual migration entry (called out in the implementation plan).

## 8. Migration plan

### Order of work

1. **Build the router skeleton** in `crates/providers`:
   - `ProviderRole` (9 variants) + `default_chain()` + `ALL`.
   - `ProviderRouter` trait + `DefaultProviderRouter` impl.
   - `create_router(config)` factory.
   - Unit tests (§9).

2. **Add config schema** in `crates/config`:
   - `RouterConfig`, `ChainConfig` types.
   - Delete `ProviderManagerConfig`.
   - Update `Config::default()`.

3. **Wire `AppCore.router`** alongside existing fields in a single PR step:
   - `AppCore` gains `router: Arc<dyn ProviderRouter>`.
   - Existing `provider`, `cognitive_provider` fields are kept temporarily so we can migrate call sites in batches without a giant single commit.

4. **Migrate call sites in batches**, by crate, leaves first:
   - **Batch A** — `crates/coding-memory`: 2 sites already use roles; retarget from `Arc<ProviderManager>` to `Arc<dyn ProviderRouter>`. Remove `Distiller` variant (replaced with `Cognitive`).
   - **Batch B** — `crates/cognitive`: `services/session_memory.rs`, `services/atom_extraction.rs`. Both stamp `Cognitive`.
   - **Batch C** — `crates/agent/src/adapters/*`: ~12 files. Most stamp `Cognitive`; reforge/cognitive handlers pick specific Heavy roles per §8 mapping.
   - **Batch D** — `crates/agent/src/handlers/*`, `autotuner/`, `subagent.rs`, `execution/core.rs`: stamp `Default`, `ReforgeSynth`, `ReforgeRules`, `Cognitive` (autotuner bugfix).
   - **Batch E** — `crates/app-core/handlers/notes/*`, `coding/*`: stamp `NotesGen`, `Cognitive`, `Title`-equivalent (Cognitive).
   - **Batch F** — `crates/app-core/init/*`: collapse construction; remove `cognitive_provider` parameter from `init_coaching` and similar (the dead `_cognitive_provider` is one of these).

5. **Delete legacy fields and factories:**
   - Remove `AppCore.provider` and `AppCore.cognitive_provider`.
   - Remove `create_provider_with_failover_full`, `create_cognitive_provider`.
   - Remove `ProviderManager::chat_with_role` inherent method.
   - Remove `crates/config/src/schema/providers.rs::ProviderManagerConfig`.

### Per-site role mapping (the ~50 sites)

**`Default`** (1 file, 2 entry points): `agent/src/execution/core.rs:289` (streaming) and `:588` (non-streaming fallback).

**`ReforgeSynth`** (2 sites): `agent/src/handlers/coding_synthesis.rs:52`, `agent/src/adapters/reforge_handlers.rs:275`.

**`ReforgeRules`** (2 sites): `agent/src/handlers/rule_artifacts.rs:49`, `coding-memory/src/skills.rs:212` (already stamped — keeps name).

**`ReforgeReview`** (1 site): `agent/src/adapters/reforge_handlers.rs:293`.

**`NotesGen`** (7 chat sites): `app-core/handlers/notes/insight.rs` — `:203` (changes-summary), `:365` (changes-summary chat), `:575` (scenario challenge), `:680` (regenerate tab), `:1197` (insight pipeline streaming), `:1259` (insight pipeline non-streaming); plus `app-core/handlers/notes/insight_chat.rs:37` (streaming chat over insight). The wiring point at `insight.rs:121` is not a chat call but a provider selection — folded into the constructor that now takes `Arc<dyn ProviderRouter>`. (Lines may shift; named by file + intent.)

**`Coach`** (1 site): `agent/src/adapters/cognitive_handlers.rs:1266`.

**`Consolidate`** (3 sites): `agent/src/adapters/cognitive_handlers.rs:806` (consolidation), `:1081` (deep consolidation), `:1504` (micro-reforge).

**`GraphLink`** (4 sites): `agent/src/adapters/cognitive_handlers.rs:915` (graph link), `:1414` (community membership — verify-then-route), `agent/src/adapters/reforge_handlers.rs:417` (graph enrichment), `:537` (community intelligence).

**`Cognitive`** (~30+ sites): everything else. Notable:
- `agent/src/adapters/cognitive_handlers.rs:254` (conflict resolver), `:552` (extraction), `:1343` (critic), `:1554` (predictor), `:1646` (hierarchical), `:1694` (pruner).
- `agent/src/adapters/{llm_summary, llm_rerank, multi_query, query_rewriter, productivity, mirror_handlers}.rs`.
- `agent/src/adapters/reforge_handlers.rs:320` (narrate prose), `:642` (cross-CLI), `:745` (skill discovery).
- `agent/src/autotuner/mod.rs:771` — **bug fix**: was using primary; now `Cognitive`.
- `app-core/handlers/notes/{card_generation, practice, grading, distractors, language}.rs`.
- `app-core/coding/title_service.rs`.
- `cognitive/src/services/{session_memory, atom_extraction}.rs`.
- `coding-memory/src/distiller/phase_b.rs:174` — was `Distiller`; now `Cognitive`. Variant deleted from enum.

### Dead code removed during migration

1. **`app-core/src/init/coaching.rs:34`** — `_cognitive_provider` parameter (underscore-prefixed, unused). Removed from signature.
2. **`agent/src/adapters/cognitive_handlers.rs:1414` `LlmCommunityMembershipHandler`** — first-pass scan found no production wiring, only test construction. Verification step: grep for non-test callers. If none, the handler is deleted; if some, route to `GraphLink`.
3. **`ProviderManager::chat_with_role`** — inherent method, redundant after migration. Removed in step 5.

### Verification gates

Per `CLAUDE.md`:

- `cargo build --workspace` clean.
- `cargo nextest run --workspace` green.
- `cargo clippy --workspace --all-targets --all-features` zero warnings.
- `cargo fmt --all --check` clean.
- `./scripts/run_kca_validation.sh` passes (cognitive pipeline gets failover, which is strictly an improvement; no regression expected).

## 9. Testing

### Unit tests in `crates/providers`

1. **Role → chain resolution** — explicit override beats default; unrolled role hits its tier default; `Cognitive` hits `cognitive_default`; everything else hits `agent_default`.
2. **Default-chain fallback** — every variant of `ProviderRole::ALL` resolves to a non-null chain when only the two defaults are configured.
3. **Per-chain circuit breaker isolation** — opening the breaker on chain A does not affect chain B. (Reuse existing `ProviderManager` test patterns; add a test that constructs two managers behind a router and trips one.)
4. **Per-chain `chat_stream` parity** — streaming routes to the same chain as `chat`.
5. **`chain_for` returns the same chain instance as `chat` would resolve** — by `Arc::ptr_eq`.

### Integration tests in `tests/integration/`

1. **Two-fake-provider router test** — wire a `FakeCheap` and `FakeExpensive` adapter as the two defaults, fire a known sequence of roles, assert each call hits the expected fake. Fakes count invocations.
2. **Override priority test** — a `roles.cognitive` override is honored over `cognitive_default`.

### Migration regression

`cargo nextest run --workspace` must remain green throughout. The two existing role-stamped tests (`coding-memory/tests/skill_evolver_llm_drafted.rs` checking for `ReforgeRules`) continue to pass; the test asserting `ProviderRole::Distiller` (same file referencing `phase_b.rs`) is updated to `Cognitive` *or* removed if its assertion was about role-based routing rather than role-name stability.

### KCA gates

`./scripts/run_kca_validation.sh` should pass with no changes. The cognitive pipeline gains failover; this is structurally a quality improvement, not a regression risk. If a gate fails, root-cause before merge.

## 10. Future work — Phase 2 composition (sketch only)

This spec deliberately does *not* design composition workflows. The Phase 1 router preserves the seams Phase 2 will need:

- **`ProviderRouter::chain_for(role)` returns `Arc<ProviderManager>`** — itself a `LlmProvider`. A workflow node can hold one chain across multiple calls without round-tripping through the router.
- **Roles are addressable.** Phase 2 can introduce new `ProviderRole` variants (e.g. `PlannerLight`, `CoderHeavy`, `CriticFast`) without touching the router or any existing call site.
- **`ChatParams.role` is plumbed end-to-end** — adapters that grow Phase-2-specific behavior (e.g. self-consistency sampling) can branch on it.

Compositional patterns Phase 2 may explore:

| Pattern | Use case |
|---|---|
| Cascade | Cheap-first; escalate to expensive on low-confidence output. |
| Pipeline | Distinct-role stages: `PlannerLight` → `CoderHeavy` → `CriticFast`. |
| Race | Parallel calls to N chains; take fastest or merge. |
| Voting / N-best | Sample from multiple providers; pick by self-consistency. |

Phase 2 will get its own design pass with its own decisions table.

## 11. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Hard schema cut breaks dev `config.json` files in active worktrees. | Implementation plan ships with a one-page migration cheat-sheet; pre-release means no users are affected. |
| Migration touches ~50 files; merge conflicts likely if landed slowly. | Land in batches A→F (§8) over a small number of PRs, ideally same week. Keep `AppCore.provider` and `AppCore.cognitive_provider` alive until the last batch so partial-state builds compile. |
| `LlmCommunityMembershipHandler` may be live in some path the scan missed. | Verification grep before deleting; if alive, route to `GraphLink` instead. |
| `ReforgeNarrate` tagged `Cognitive` may produce lower-quality prose than today's expensive default. | Acceptable: narration is summary prose, not synthesis. If quality regresses noticeably in QA, we can introduce a `ReforgeNarrate` Heavy role in a small follow-up. |
| Autotuner moving to `Cognitive` may reduce trial generation quality. | Autotuner is structurally similar to the cognitive classification jobs already on `Cognitive`; migration is a cost reduction. If quality regresses, override via `roles.cognitive.model` config override or split out an `Autotuner` Heavy role later. |

## 12. Appendix A — Discovery summary

Four parallel scanning agents covered the workspace on 2026-05-07. Inventory totals:

| Domain | Sites | Source |
|---|---|---|
| `agent` crate (runtime, adapters, handlers) | 32 | adapters/cognitive_handlers (12), adapters/reforge_handlers (8), execution/core (2), adapters/{summary, rerank, multi_query, rewriter, productivity, mirror_handlers} (6), handlers/{coding_synthesis, rule_artifacts} (2), autotuner (1), agent_loop/builder decomposer (1) |
| `cognitive` crate | 2 | services/session_memory, services/atom_extraction |
| `coding-memory` crate | 2 | distiller/phase_b, skills |
| `app-core` handlers/notes | 13 | insight (7), insight_chat (1), card_generation (1), practice (1), grading (2), distractors (1), language (4) |
| `app-core` handlers/cognitive (constructors) | 5 | mod.rs build_*_handler |
| `app-core` coding | 1 | coding/title_service |
| `app-core` init / cron | 2 | init/cron nightly polish, init/productivity |
| **Total** | **~57 LLM-touching sites** | |

All sites except 2 (`coding-memory/src/distiller/phase_b.rs:174` stamping `Distiller`, `coding-memory/src/skills.rs:212` stamping `ReforgeRules`) currently call `.chat()` with no role tag. Both stampings are accepted but ignored by `ProviderManager::chat_with_role` (the existing stub).

The cognitive provider (used by ~40 of these sites) is bare — no `ProviderManager` wrap — so none of those 40 sites have failover today.

## 13. Appendix B — Mapping to legacy config keys (for the implementation plan)

| Legacy key | New location |
|---|---|
| `provider_manager.fallback` | `router.agent_default.fallbacks[0]` |
| `provider_manager.classifier_model` | `router.cognitive_default.model` (classifier collapsed into Cognitive) |
| `cognitive.provider` | `router.cognitive_default.primary` |
| `cognitive.model` | `router.cognitive_default.model` |
| `cognitive.temperature` | unchanged (lives in `cognitive_chat_params`, still used per-call) |
| `cognitive.max_tokens` | unchanged |
| `coding_memory.reforge.synth_model` | `router.roles.reforge_synth.model` |
| `coding_memory.reforge.rules_model` | `router.roles.reforge_rules.model` |
| `graph_linker_model` (and per-handler peers) | `router.roles.graph_link.model` (or `router.roles.consolidate.model` per intent) |

The implementation plan owns the precise key-by-key migration of the (unreleased) developer config and any `KLYNTBOT_*` env-override paths.
