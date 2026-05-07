# Provider-Router Multi-Role Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the dual `provider` / `cognitive_provider` lineages with a single `Arc<dyn ProviderRouter>` that does role-keyed dispatch over per-chain `ProviderManager` instances. Migrate ~50 LLM call sites to declare their role; cognitive pipeline gains failover for free; lay the groundwork for Phase 2 multi-model composition.

**Architecture:** New `ProviderRouter` trait above the existing `LlmProvider` trait. `DefaultProviderRouter` holds `HashMap<ProviderRole, Arc<ProviderManager>>` plus two named default chains (`agent_default`, `cognitive_default`). Each `ProviderRole` declares its own default chain; per-role overrides come from config. `ProviderManager` is unchanged structurally but now there is one *per chain* instead of one global.

**Tech Stack:** Rust stable 1.93, `async-trait`, `tokio`, `serde`, `cargo-nextest`. Config in JSON (camelCase). Tests use ephemeral SQLite (`StoragePool::connect_in_memory()`).

**Spec reference:** `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`.

**Pre-flight:** Per `CLAUDE.md`, this is a pre-release codebase — no user data migration needed. Hard schema cuts are acceptable. Per the same doc, MSRV is 1.93; clippy must run with **zero warnings**; tracing on every public AppCore handler method.

---

## Phase 0 — Branch + worktree setup

### Task 0: Create a worktree for the migration

**Files:** none — git only.

- [ ] **Step 1: Create the worktree branch**

```bash
git worktree add -b feat/provider-router ../bot-provider-router main
cd ../bot-provider-router
```

Expected: a parallel checkout at `../bot-provider-router` on a new branch `feat/provider-router`.

- [ ] **Step 2: Verify clean build before changes**

```bash
cargo build --workspace
```

Expected: clean build with whatever warnings exist on `main`. Capture the warning count to compare against later.

- [ ] **Step 3: Confirm test suite green before changes**

```bash
cargo nextest run --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit a baseline marker**

```bash
git commit --allow-empty -m "chore: baseline before provider-router migration"
```

This anchors the work and lets the engineer `git diff` the entire migration later.

---

## Phase 1 — New router infrastructure (TDD)

This phase builds the new abstraction and its tests in isolation. After Phase 1, nothing else in the codebase has changed yet — the router is dead code waiting to be wired.

### Task 1: Replace the `ProviderRole` enum with the 9-variant version

**Files:**
- Modify: `crates/providers/src/lib.rs:7-17`

The existing enum has 3 variants (`Distiller`, `ReforgeSynth`, `ReforgeRules`). Replace with the 9-variant version. **Note:** removing `Distiller` is a breaking change to two existing tagged call sites — they get fixed in Tasks 13–14. Until those tasks complete, the workspace will not build. We accept that intermediate red state and unblock by leaving `Distiller` as a deprecated alias for one phase.

- [ ] **Step 1: Write failing test for `default_chain()`**

Append to `crates/providers/src/lib.rs` inside a new `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn default_chain_for_cognitive_is_cognitive_default() {
        assert_eq!(ProviderRole::Cognitive.default_chain(), "cognitive_default");
    }

    #[test]
    fn default_chain_for_default_role_is_agent_default() {
        assert_eq!(ProviderRole::Default.default_chain(), "agent_default");
    }

    #[test]
    fn default_chain_for_all_heavy_roles_is_agent_default() {
        for role in ProviderRole::ALL.iter().copied() {
            if role == ProviderRole::Cognitive {
                continue;
            }
            assert_eq!(role.default_chain(), "agent_default", "role: {role:?}");
        }
    }

    #[test]
    fn all_contains_every_variant() {
        // If a new variant is added without updating ALL, this test still
        // compiles; a clippy::missing_match_arms would warn on default_chain().
        // The exhaustiveness check lives there.
        assert!(ProviderRole::ALL.contains(&ProviderRole::Default));
        assert!(ProviderRole::ALL.contains(&ProviderRole::Cognitive));
        assert!(ProviderRole::ALL.contains(&ProviderRole::ReforgeSynth));
    }

    #[test]
    fn snake_case_serialization() {
        let json = serde_json::to_string(&ProviderRole::ReforgeSynth).unwrap();
        assert_eq!(json, "\"reforge_synth\"");
        let json = serde_json::to_string(&ProviderRole::NotesGen).unwrap();
        assert_eq!(json, "\"notes_gen\"");
        let json = serde_json::to_string(&ProviderRole::GraphLink).unwrap();
        assert_eq!(json, "\"graph_link\"");
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo nextest run -p providers role_tests
```

Expected: compile error — `ProviderRole::Default`, `Cognitive`, `NotesGen`, `GraphLink`, `ALL`, `default_chain` do not exist yet.

- [ ] **Step 3: Replace the enum and add the impl**

Replace lines 7-17 of `crates/providers/src/lib.rs` with:

```rust
/// Identifies the role a provider invocation serves.
///
/// Each variant has a default chain — the named chain in `RouterConfig` it
/// resolves to when `roles.<variant>` is not explicitly overridden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    // -- Heavy: quality-critical, named for individual customization --
    /// Main agent loop; fallback for any unrolled site.
    Default,
    /// Reforge Phase 2 — synthesize new rules / observations.
    ReforgeSynth,
    /// Reforge Phase 3 — generate rule artifacts (CLAUDE.md / AGENTS.md / SKILL.md).
    ReforgeRules,
    /// Reforge Phase 3 — review/critique synthesized rules.
    ReforgeReview,
    /// Notes/insight generation: streaming generation, regenerate, scenario
    /// challenges, changes-summary between insight versions.
    NotesGen,
    /// Coaching reasoner — generates coaching interventions.
    Coach,
    /// Memory consolidation: per-turn consolidation, deep cross-session
    /// consolidation, micro-reforge.
    Consolidate,
    /// Knowledge-graph operations: graph link, graph enrichment, community
    /// intelligence, community membership inference.
    GraphLink,
    // -- Light: catch-all for cheap, frequent, latency-sensitive calls --
    /// Cheap/fast tier — distiller, title, classify, rerank, rewrite,
    /// multi-query, summary, extract, critic, predictor, pruner, mirror,
    /// autotuner, notes-practice, notes-grade.
    Cognitive,
}

impl ProviderRole {
    /// Name of the default chain in `RouterConfig` to use when this role
    /// is not explicitly overridden.
    pub fn default_chain(self) -> &'static str {
        match self {
            Self::Cognitive => "cognitive_default",
            Self::Default
            | Self::ReforgeSynth
            | Self::ReforgeRules
            | Self::ReforgeReview
            | Self::NotesGen
            | Self::Coach
            | Self::Consolidate
            | Self::GraphLink => "agent_default",
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

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo nextest run -p providers role_tests
```

Expected: all 4 tests pass. Workspace compile errors are expected at the two ex-`Distiller` call sites — those are fixed in Tasks 13 and 14.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/lib.rs
git commit -m "feat(providers): expand ProviderRole to 9 variants with default_chain()"
```

---

### Task 2: Add the `ProviderRouter` trait

**Files:**
- Create: `crates/providers/src/router.rs`
- Modify: `crates/providers/src/lib.rs` (add `pub mod router;` and re-exports)

- [ ] **Step 1: Write failing trait-existence test**

Add to a new file `crates/providers/src/router.rs`:

```rust
//! Role-keyed dispatcher above `LlmProvider`. See spec
//! `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::manager::ProviderManager;
use crate::types::{CacheBreakpoint, ChatParams, LlmResponse, LlmStream, Message};
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

    /// Resolve the chain for a role. Used by Phase 2 composition workflows,
    /// telemetry / cost-accounting, and tests.
    fn chain_for(&self, role: ProviderRole) -> Arc<ProviderManager>;

    /// Default model name for a role; stamped into `ChatParams.model`
    /// when a call site doesn't override it.
    fn default_model(&self, role: ProviderRole) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check: a `Box<dyn ProviderRouter>` is `Send + Sync`.
    #[test]
    fn router_is_object_safe_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn ProviderRouter>>();
    }
}
```

- [ ] **Step 2: Wire the module into `crates/providers/src/lib.rs`**

After the `pub mod` block (around line 22), add:

```rust
pub mod router;
```

After the `pub use manager::{...}` block (around line 35), add:

```rust
// -- Router --
pub use router::{DefaultProviderRouter, ProviderRouter};
```

(`DefaultProviderRouter` will be added in Task 3 — declaring it in the re-export now is intentional; the export will fail until Task 3 lands. This forces Tasks 2 + 3 to be committed together if they're not done in the same session.)

If you prefer to land them as separate commits, omit the `DefaultProviderRouter` re-export here and add it in Task 3 instead.

- [ ] **Step 3: Run tests to confirm trait compiles and is Send+Sync**

```bash
cargo nextest run -p providers router::tests
```

Expected: `router_is_object_safe_and_sync` passes.

- [ ] **Step 4: Commit**

```bash
git add crates/providers/src/router.rs crates/providers/src/lib.rs
git commit -m "feat(providers): add ProviderRouter trait"
```

---

### Task 3: Add `DefaultProviderRouter` implementation + tests

**Files:**
- Modify: `crates/providers/src/router.rs`
- Modify: `crates/providers/src/lib.rs` (already added `DefaultProviderRouter` to re-exports in Task 2)

- [ ] **Step 1: Write failing tests for `DefaultProviderRouter`**

Append to `crates/providers/src/router.rs` (replace the `mod tests` block from Task 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::NoopProvider;
    use crate::manager::ProviderManager;
    use crate::types::DynProvider;
    use std::sync::Arc;

    fn manager_named(_name: &'static str) -> Arc<ProviderManager> {
        let p: DynProvider = Arc::new(NoopProvider);
        Arc::new(ProviderManager::new(p, None, None))
    }

    fn router_with_no_overrides() -> DefaultProviderRouter {
        DefaultProviderRouter::new(
            manager_named("agent"),
            manager_named("cognitive"),
            HashMap::new(),
        )
    }

    #[test]
    fn router_is_object_safe_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn ProviderRouter>>();
    }

    #[test]
    fn cognitive_role_resolves_to_cognitive_default() {
        let router = router_with_no_overrides();
        let agent = manager_named("agent_for_compare");
        let cognitive = router.chain_for(ProviderRole::Cognitive);
        // chain_for(Cognitive) must return the cognitive_default Arc, not agent_default.
        // Compare by Arc::ptr_eq to a fresh handle the test holds.
        assert!(!Arc::ptr_eq(&cognitive, &agent));
    }

    #[test]
    fn heavy_role_with_no_override_resolves_to_agent_default() {
        let agent = manager_named("agent");
        let cognitive = manager_named("cognitive");
        let router =
            DefaultProviderRouter::new(agent.clone(), cognitive.clone(), HashMap::new());

        for role in [
            ProviderRole::Default,
            ProviderRole::ReforgeSynth,
            ProviderRole::ReforgeRules,
            ProviderRole::ReforgeReview,
            ProviderRole::NotesGen,
            ProviderRole::Coach,
            ProviderRole::Consolidate,
            ProviderRole::GraphLink,
        ] {
            let resolved = router.chain_for(role);
            assert!(
                Arc::ptr_eq(&resolved, &agent),
                "role {role:?} did not resolve to agent_default"
            );
        }
    }

    #[test]
    fn role_override_beats_default() {
        let agent = manager_named("agent");
        let cognitive = manager_named("cognitive");
        let override_chain = manager_named("override");

        let mut overrides = HashMap::new();
        overrides.insert(ProviderRole::ReforgeSynth, override_chain.clone());

        let router = DefaultProviderRouter::new(agent, cognitive, overrides);
        let resolved = router.chain_for(ProviderRole::ReforgeSynth);
        assert!(Arc::ptr_eq(&resolved, &override_chain));
    }

    #[test]
    fn cognitive_override_beats_cognitive_default() {
        let agent = manager_named("agent");
        let cognitive_default = manager_named("cognitive_default");
        let cognitive_override = manager_named("cognitive_override");

        let mut overrides = HashMap::new();
        overrides.insert(ProviderRole::Cognitive, cognitive_override.clone());

        let router = DefaultProviderRouter::new(agent, cognitive_default, overrides);
        let resolved = router.chain_for(ProviderRole::Cognitive);
        assert!(Arc::ptr_eq(&resolved, &cognitive_override));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo nextest run -p providers router::tests
```

Expected: compile error — `DefaultProviderRouter::new` and the struct itself don't exist yet.

- [ ] **Step 3: Add the `DefaultProviderRouter` struct and impl**

Insert into `crates/providers/src/router.rs` between the trait definition and the `#[cfg(test)]` block:

```rust
/// Default `ProviderRouter` impl backed by per-chain `ProviderManager`s.
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
        &self,
        role: ProviderRole,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        let chain = self.resolve(role);
        let mut params = params.clone();
        params.role = Some(role);
        chain.chat(messages, tools, &params, cache_breakpoints).await
    }

    async fn chat_stream(
        &self,
        role: ProviderRole,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        let chain = self.resolve(role);
        let mut params = params.clone();
        params.role = Some(role);
        chain
            .chat_stream(messages, tools, &params, cache_breakpoints)
            .await
    }

    fn chain_for(&self, role: ProviderRole) -> Arc<ProviderManager> {
        self.resolve(role)
    }

    fn default_model(&self, role: ProviderRole) -> String {
        self.resolve(role).default_model().to_string()
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo nextest run -p providers router::tests
```

Expected: all 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/router.rs crates/providers/src/lib.rs
git commit -m "feat(providers): DefaultProviderRouter with role-keyed chain resolution"
```

---

## Phase 2 — Config schema

### Task 4: Add `RouterConfig` types in a new module

**Files:**
- Create: `crates/config/src/schema/router.rs`
- Modify: `crates/config/src/schema/mod.rs` (add `pub mod router;`)

- [ ] **Step 1: Write failing test for `RouterConfig` deserialization**

Create `crates/config/src/schema/router.rs`:

```rust
//! Router configuration — replaces `ProviderManagerConfig`.
//!
//! See spec `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`.

use std::collections::HashMap;

use providers::ProviderRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouterConfig {
    #[serde(default)]
    pub agent_default: ChainConfig,
    #[serde(default)]
    pub cognitive_default: ChainConfig,

    /// Optional per-role overrides.
    #[serde(default)]
    pub roles: HashMap<ProviderRole, ChainConfig>,

    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChainConfig {
    /// Provider name from `providers.*` (e.g., "anthropic", "openai", "groq").
    /// Empty string means "no chain configured" — Default impl yields this.
    #[serde(default)]
    pub primary: String,
    /// Optional ordered list of fallback provider names.
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Optional model override; defaults to the provider's registry default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        Self {
            failure_threshold: 5,
            reset_timeout_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_empty() {
        let c = RouterConfig::default();
        assert_eq!(c.agent_default.primary, "");
        assert_eq!(c.cognitive_default.primary, "");
        assert!(c.roles.is_empty());
        assert_eq!(c.circuit_breaker.failure_threshold, 5);
        assert_eq!(c.circuit_breaker.reset_timeout_secs, 60);
    }

    #[test]
    fn deserialize_minimal() {
        let json = serde_json::json!({
            "agentDefault":     { "primary": "anthropic", "fallbacks": ["openai"] },
            "cognitiveDefault": { "primary": "groq",      "fallbacks": ["deepseek"] }
        });
        let c: RouterConfig = serde_json::from_value(json).unwrap();
        assert_eq!(c.agent_default.primary, "anthropic");
        assert_eq!(c.agent_default.fallbacks, vec!["openai".to_string()]);
        assert_eq!(c.cognitive_default.primary, "groq");
        assert!(c.roles.is_empty());
    }

    #[test]
    fn deserialize_with_role_overrides() {
        let json = serde_json::json!({
            "agentDefault":     { "primary": "anthropic" },
            "cognitiveDefault": { "primary": "groq" },
            "roles": {
                "reforge_synth": { "primary": "anthropic" },
                "notes_gen":     { "primary": "anthropic", "fallbacks": ["openai"] }
            }
        });
        let c: RouterConfig = serde_json::from_value(json).unwrap();
        assert_eq!(c.roles.len(), 2);
        let synth = c.roles.get(&ProviderRole::ReforgeSynth).unwrap();
        assert_eq!(synth.primary, "anthropic");
        let notes = c.roles.get(&ProviderRole::NotesGen).unwrap();
        assert_eq!(notes.fallbacks, vec!["openai".to_string()]);
    }

    #[test]
    fn unknown_field_fails() {
        let json = serde_json::json!({
            "agentDefault": { "primary": "anthropic", "extra": 1 }
        });
        let result: Result<RouterConfig, _> = serde_json::from_value(json);
        assert!(result.is_err(), "deny_unknown_fields should reject `extra`");
    }
}
```

- [ ] **Step 2: Wire the module into `crates/config/src/schema/mod.rs`**

Add a line:

```rust
pub mod router;
```

(Put it alphabetically next to `pub mod providers;`.)

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p config router::tests
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/router.rs crates/config/src/schema/mod.rs
git commit -m "feat(config): RouterConfig with chains + role overrides"
```

---

### Task 5: Wire `RouterConfig` into the `Config` root and remove `ProviderManagerConfig`

**Files:**
- Modify: `crates/config/src/schema/core.rs:33` (import line) and `:147` (struct field)
- Modify: `crates/config/src/schema/providers.rs` — delete `ProviderManagerConfig` and its 3 tests
- Modify: `crates/config/src/schema/cognitive.rs` — remove `model`, `provider`, `graph_linker_model` fields (3 fields) — see Task 6 for details

This task replaces the legacy `provider_manager` block. We're pre-release, so this is a hard cut.

- [ ] **Step 1: Modify the `Config` struct field**

In `crates/config/src/schema/core.rs`:

Change line 33 from:

```rust
use super::providers::{ProviderManagerConfig, ProvidersConfig};
```

to:

```rust
use super::providers::ProvidersConfig;
use super::router::RouterConfig;
```

Change line 145-147 from:

```rust
    /// Provider manager routing (primary/fallback/classifier)
    #[serde(default)]
    pub provider_manager: ProviderManagerConfig,
```

to:

```rust
    /// Router config — role-keyed chain dispatch (replaces provider_manager).
    #[serde(default)]
    pub router: RouterConfig,
```

- [ ] **Step 2: Delete `ProviderManagerConfig` from `crates/config/src/schema/providers.rs`**

Remove lines 102-115 (the struct + doc comment) and lines 232-259 (its 3 unit tests):

```rust
// DELETE:
/// Provider manager configuration for primary/fallback/classifier routing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManagerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_model: Option<String>,
}
```

And the 3 tests `test_provider_manager_config_default`, `test_provider_manager_config_serde`, `test_provider_manager_config_empty_json`.

- [ ] **Step 3: Run config tests**

```bash
cargo nextest run -p config
```

Expected: pass. The `provider_manager` field is gone but `router` defaults work via `#[serde(default)]`.

- [ ] **Step 4: Run workspace build to find downstream callers**

```bash
cargo build --workspace 2>&1 | grep "error\[" | head -30
```

Expected: errors at every site that referenced `config.provider_manager.fallback` or `config.provider_manager.classifier_model`. Two known sites in `crates/providers/src/factory.rs`:
- `factory.rs:118` — `let pm_config = &config.provider_manager;`
- `factory.rs:152` — same.

These are fixed in Task 7 (factory rewrite). Until then, the build is broken — proceed to Task 6 first since it's independent (just config changes).

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/core.rs crates/config/src/schema/providers.rs
git commit -m "refactor(config): replace provider_manager with router config block"
```

---

### Task 6: Remove migrated fields from `CognitiveConfig`

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs:19-39` — remove `model`, `provider`, `graph_linker_model`

These three fields move into `router.cognitive_default.{primary,model}` and `router.roles.graph_link.model` respectively. Behavior fields (`temperature`, `max_tokens`, `dynamic_facts_enabled`, all the `_limit` / `_threshold` knobs) stay because they're used per-call in `cognitive_chat_params()`.

- [ ] **Step 1: Locate downstream callers before removing**

```bash
grep -rn "config\.cognitive\.model\|config\.cognitive\.provider\|cognitive\.graph_linker_model" crates/ src/ 2>/dev/null
```

Note the sites; they will need to read from `config.router.cognitive_default` instead. The factory rewrite in Task 7 handles `factory.rs::create_cognitive_provider`. Other sites are noted in Task 7's grep output.

- [ ] **Step 2: Edit `crates/config/src/schema/cognitive.rs`**

Remove the three fields (and their doc comments and `skip_serializing_if`):

```rust
// DELETE these lines:
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_linker_model: Option<String>,
```

Keep all other fields.

- [ ] **Step 3: Run config tests**

```bash
cargo nextest run -p config
```

Expected: pass. Other crates may not yet build — fixed in Tasks 7 and 8.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/cognitive.rs
git commit -m "refactor(config): drop cognitive.{model,provider,graph_linker_model} (moved to router)"
```

---

## Phase 3 — Factory + AppCore wiring

### Task 7: Rewrite `crates/providers/src/factory.rs` to produce a `ProviderRouter`

**Files:**
- Modify: `crates/providers/src/factory.rs` — replace `create_provider_with_failover`, `create_provider_with_failover_full`, `create_cognitive_provider` with `create_router`. Keep `create_provider` (used by NoopProvider setup-wizard path) and `cognitive_chat_params` (still used per-call).

Read the spec §6 and §7 before starting; this is the heaviest single task.

- [ ] **Step 1: Replace the body of `crates/providers/src/factory.rs`**

The replacement is large — keep `create_provider`, `try_create_from_spec`, and `cognitive_chat_params` exactly as they are today; replace everything else (the `create_provider_with_failover*`, `create_fallback_provider`, `create_classifier_provider`, `create_cognitive_provider` functions) with a single `create_router` plus its helpers.

Replace the imports at the top (currently lines 7-16) with:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use common::{ConfigError, Result};
use config::Config;

use crate::adapters::{AnthropicNativeProvider, OpenAiCompatProvider};
use crate::manager::ProviderManager;
use crate::registry::{ProviderRegistry, ProviderSpec};
use crate::router::{DefaultProviderRouter, ProviderRouter};
use crate::types::{ChatParams, DynProvider};
use crate::ProviderRole;
use config::schema::router::ChainConfig;
```

Replace `create_provider_with_failover`, `create_provider_with_failover_full`, `create_fallback_provider`, `create_classifier_provider`, and `create_cognitive_provider` with:

```rust
/// Build a `ProviderRouter` from `Config`.
///
/// Resolves the two named default chains (`agent_default`, `cognitive_default`)
/// plus any per-role overrides in `config.router.roles`. Each chain is wrapped
/// in its own `ProviderManager` (independent retry / circuit breaker).
///
/// Returns `(router, agent_default_model)` so callers that previously read
/// `agents.defaults.model` from a single resolved primary still have the
/// info they need.
pub fn create_router(config: &Config) -> Result<(Arc<dyn ProviderRouter>, String)> {
    let agent_default =
        build_chain(config, &config.router.agent_default, &config.agents.defaults.model)
            .ok_or_else(|| {
                ConfigError::MissingField(
                    "router.agent_default has no usable provider — add an API key under providers.* or set router.agent_default.primary".to_string(),
                )
            })?;

    let cognitive_default = build_chain(
        config,
        &config.router.cognitive_default,
        &config.agents.defaults.model,
    )
    .unwrap_or_else(|| {
        warn!("router.cognitive_default has no usable provider; falling back to agent_default for cognitive roles");
        agent_default.clone()
    });

    let mut overrides: HashMap<ProviderRole, Arc<ProviderManager>> = HashMap::new();
    for (role, chain_cfg) in &config.router.roles {
        match build_chain(config, chain_cfg, &config.agents.defaults.model) {
            Some(chain) => {
                overrides.insert(*role, chain);
            }
            None => {
                warn!(
                    "router.roles.{} has no usable provider (primary='{}'); using tier default",
                    serde_json::to_string(role).unwrap_or_default(),
                    chain_cfg.primary
                );
            }
        }
    }

    let resolved_model = config.agents.defaults.model.clone();
    let router: Arc<dyn ProviderRouter> = Arc::new(DefaultProviderRouter::new(
        agent_default,
        cognitive_default,
        overrides,
    ));
    Ok((router, resolved_model))
}

/// Build one `ProviderManager` from a `ChainConfig`.
///
/// Returns `None` when `primary` references a provider with no API key
/// (no chain can be built). Fallbacks with no API key are dropped with
/// a warn and the chain is built without them.
fn build_chain(
    config: &Config,
    chain: &ChainConfig,
    default_model: &str,
) -> Option<Arc<ProviderManager>> {
    if chain.primary.is_empty() {
        return None;
    }

    let model = chain.model.as_deref().unwrap_or(default_model);

    let primary_spec = ProviderRegistry::find_by_name(&chain.primary)?;
    let primary = try_create_from_spec(primary_spec, config, model)?;

    // Build fallback if the first one is usable; ignore the rest until we
    // need richer chains.
    let fallback = chain
        .fallbacks
        .iter()
        .find_map(|fb_name| {
            let spec = ProviderRegistry::find_by_name(fb_name)?;
            let provider = try_create_from_spec(spec, config, model);
            if provider.is_none() {
                warn!(
                    "router fallback '{fb_name}' configured but unusable (no API key); skipping"
                );
            }
            provider
        });

    info!(
        "router chain built: primary={} fallback={:?} model={}",
        chain.primary,
        fallback.as_ref().map(|_| chain.fallbacks.first().cloned().unwrap_or_default()),
        model
    );

    Some(Arc::new(ProviderManager::new(primary, fallback, None)))
}
```

(`try_create_from_spec`, `create_provider`, and `cognitive_chat_params` remain unchanged in this file.)

- [ ] **Step 2: Delete the old factory functions**

Confirm the following functions no longer exist in `crates/providers/src/factory.rs`:
- `create_provider_with_failover`
- `create_provider_with_failover_full`
- `create_fallback_provider`
- `create_classifier_provider`
- `create_cognitive_provider`

- [ ] **Step 3: Update `crates/providers/src/lib.rs` re-exports**

Replace the factory `pub use` block (currently lines 50-53):

```rust
// -- Factory --
pub use factory::{
    cognitive_chat_params, create_provider_with_failover,
    create_provider_with_failover_full, create_provider, create_cognitive_provider,
};
```

with:

```rust
// -- Factory --
pub use factory::{cognitive_chat_params, create_provider, create_router};
```

- [ ] **Step 4: Add a unit test for `create_router`**

Append to `crates/providers/src/factory.rs` inside `mod tests`:

```rust
    #[test]
    fn create_router_with_no_keys_fails_loudly() {
        let config = Config::default();
        let result = create_router(&config);
        assert!(result.is_err(), "router with no API keys must fail");
    }

    #[test]
    fn create_router_with_only_agent_default_succeeds() {
        let mut config = Config::default();
        config.providers.anthropic.api_key = config::Secret::new("sk-test".to_string());
        config.router.agent_default.primary = "anthropic".to_string();
        // cognitive_default empty; should fall back to agent_default

        let result = create_router(&config);
        assert!(result.is_ok());
        let (_router, model) = result.unwrap();
        assert_eq!(model, config.agents.defaults.model);
    }
```

- [ ] **Step 5: Run providers tests**

```bash
cargo nextest run -p providers
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/providers/src/factory.rs crates/providers/src/lib.rs
git commit -m "feat(providers): create_router factory replaces create_provider_with_failover*"
```

---

### Task 8: Add `router` field to `AppCore` and construct it in `init/storage.rs`

**Files:**
- Modify: `crates/app-core/src/state.rs` (add `router` field, KEEP `cognitive_provider` for transitional period)
- Modify: `crates/app-core/src/init/storage.rs` (replace `create_provider_with_failover_full` with `create_router`)
- Modify: `crates/app-core/src/init/mod.rs` (init order — store router into AppCore)

- [ ] **Step 1: Add `router` field on `AppCore`**

In `crates/app-core/src/state.rs`, near the `cognitive_provider` field (around line 93), add:

```rust
    /// Role-keyed LLM router — replaces both `cognitive_provider` and the
    /// agent-loop primary lineage. See `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`.
    pub router: Arc<dyn providers::ProviderRouter>,
```

(`Arc<dyn ProviderRouter>` not `Option<...>` — startup fails loudly if no provider is configured, matching `create_router`'s behavior.)

- [ ] **Step 2: Update `StorageResult` and `init_storage`**

In `crates/app-core/src/init/storage.rs:15-18`, replace `provider_manager` with `router`:

```rust
pub struct StorageResult {
    pub config: Config,
    pub storage_pool: storage::StoragePool,
    pub repos: storage::Repos,
    pub vector_store: Option<VectorStore>,
    pub note_repo: cognitive::NoteRepo,
    pub provider: providers::DynProvider,        // KEEP — transitional, removed in Task N
    pub router: Arc<dyn providers::ProviderRouter>,
    pub provider_manager: Option<Arc<providers::ProviderManager>>,  // KEEP — circuit breaker callbacks
}
```

In the body of `init_storage` (around lines 161-200), replace the `create_provider_with_failover_full` call with parallel router construction. **Both** `provider` and `router` are returned during the transition; the agent-loop wiring still wants the bare `DynProvider`.

```rust
    let (router, resolved_model) = providers::create_router(&config).unwrap_or_else(|e| {
        warn!(
            "No LLM router configured ({e}), using noop — setup wizard will handle configuration"
        );
        let noop: providers::DynProvider = Arc::new(providers::NoopProvider);
        let mgr = Arc::new(providers::ProviderManager::new(noop, None, None));
        let router: Arc<dyn providers::ProviderRouter> = Arc::new(
            providers::DefaultProviderRouter::new(mgr.clone(), mgr, std::collections::HashMap::new())
        );
        (router, config.agents.defaults.model.clone())
    });

    // Transitional: derive a bare `DynProvider` for the agent-loop wiring
    // until that's migrated. Resolve it to the agent_default chain.
    let provider: providers::DynProvider = router.chain_for(providers::ProviderRole::Default);

    // The Option<Arc<ProviderManager>> exposed for circuit-breaker callbacks
    // becomes the agent_default chain. (Cognitive chain has its own breaker;
    // we only persist agent_default for now.)
    let provider_manager = Some(router.chain_for(providers::ProviderRole::Default));

    // Existing circuit-breaker persistence wiring stays — same callback,
    // same restore behavior.
    if let Some(ref manager) = provider_manager {
        if let Err(e) = storage::circuit_breaker::ensure_table(&storage_pool).await {
            warn!("circuit breaker table init failed (non-fatal): {e}");
        } else {
            if let Ok(Some(dt)) = storage::circuit_breaker::load(&storage_pool).await {
                manager.restore_circuit_state(dt).await;
            }
            let pool = storage_pool.clone();
            let cb: providers::OnCircuitOpen = Arc::new(move |open_until| {
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = storage::circuit_breaker::save(&pool, open_until).await {
                        tracing::warn!("circuit breaker persist failed: {e}");
                    }
                });
            });
            manager.set_circuit_open_callback(cb).await;
        }
    }

    config.agents.defaults.model = resolved_model;

    Ok(StorageResult {
        config,
        storage_pool,
        repos,
        vector_store,
        note_repo,
        provider,
        router,
        provider_manager,
    })
```

- [ ] **Step 3: Update `init_app_core` (or wherever `AppCore` is constructed)**

In `crates/app-core/src/init/mod.rs`, locate the `AppCore { ... }` struct literal and add `router: storage_result.router.clone()` next to `cognitive_provider`. Leave the existing `cognitive_provider` derivation in place (we migrate sites off it batch-by-batch).

```bash
grep -n "AppCore {" crates/app-core/src/init/mod.rs
```

Around the located construction site, add `router: storage_result.router.clone(),` to the struct literal.

- [ ] **Step 4: Build and check**

```bash
cargo build -p app-core
```

Expected: clean build (the workspace as a whole still has the `Distiller` removal errors — those are fixed in Tasks 13-14).

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/storage.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): add router to AppCore, construct in init/storage"
```

---

## Phase 4 — Migrate call sites: Batch A (coding-memory, 2 sites)

These two sites already stamp roles; they just need to switch from `Arc<ProviderManager>` to `Arc<dyn ProviderRouter>`, and `Distiller` becomes `Cognitive`.

### Task 9: Migrate `coding-memory/src/distiller/phase_b.rs`

**Files:**
- Modify: `crates/coding-memory/src/distiller/phase_b.rs` (struct field at ~line 138, call at line 174)
- Modify: `crates/coding-memory/src/distiller/mod.rs` (the type at line 152)
- Modify: any caller that constructs `LlmInvocation`

- [ ] **Step 1: Locate the struct + call**

```bash
grep -n "Arc<ProviderManager>\|provider_manager\|provider\.chat_with_role" crates/coding-memory/src/distiller/phase_b.rs crates/coding-memory/src/distiller/mod.rs
```

- [ ] **Step 2: Change struct field type**

In `crates/coding-memory/src/distiller/phase_b.rs`, find:

```rust
pub struct LlmInvocation {
    pub provider: Arc<ProviderManager>,
    // ...
}
```

Change to:

```rust
pub struct LlmInvocation {
    pub router: Arc<dyn providers::ProviderRouter>,
    // ...
}
```

In `crates/coding-memory/src/distiller/mod.rs:152` (the `DistillerInner` struct):

```rust
struct DistillerInner {
    provider: Arc<ProviderManager>,
    // ...
}
```

Change to:

```rust
struct DistillerInner {
    router: Arc<dyn providers::ProviderRouter>,
    // ...
}
```

- [ ] **Step 3: Change the call site at phase_b.rs:174**

Find the existing call:

```rust
let fut = inv.provider.chat_with_role(
    providers::ProviderRole::Distiller,
    &messages,
    Some(tools),
    &params,
    &[],
);
```

Replace with:

```rust
let fut = inv.router.chat(
    providers::ProviderRole::Cognitive,
    &messages,
    Some(tools),
    &params,
    &[],
);
```

(Distiller folds into Cognitive per spec §5.)

- [ ] **Step 4: Update upstream wiring**

Find every constructor for `Distiller` / `DistillerInner` / `LlmInvocation`:

```bash
grep -rn "Distiller::new\|DistillerInner\|LlmInvocation {" crates/ src/ 2>/dev/null
```

For each such site, replace `provider_manager: Arc<ProviderManager>` (the input) with `router: Arc<dyn providers::ProviderRouter>`. The `app-core/init/mod.rs:996-1001` block ("wraps distiller…") becomes:

```rust
let distiller = Distiller::new(
    storage_pool.clone(),
    state.router.clone(),
    /* other args */
);
```

Drop the `provider_manager.unwrap_or_else(|| ProviderManager::new(...))` wrapping fallback — the router always exists.

- [ ] **Step 5: Run coding-memory tests**

```bash
cargo nextest run -p coding-memory
```

Expected: pass. The proptest in `tests/cross_cli_normalization.rs` should be unaffected.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/distiller/ crates/app-core/src/init/mod.rs
git commit -m "refactor(coding-memory): distiller takes ProviderRouter (Distiller→Cognitive role)"
```

---

### Task 10: Migrate `coding-memory/src/skills.rs`

**Files:**
- Modify: `crates/coding-memory/src/skills.rs` (field at line 71, call at line 212)

- [ ] **Step 1: Locate**

```bash
grep -n "provider_manager\|ProviderManager" crates/coding-memory/src/skills.rs
```

- [ ] **Step 2: Change the struct field**

Replace the `provider_manager: Option<Arc<ProviderManager>>` field with:

```rust
    router: Option<Arc<dyn providers::ProviderRouter>>,
```

Update the builder method `with_provider_manager` to `with_router`:

```rust
pub fn with_router(mut self, router: Arc<dyn providers::ProviderRouter>) -> Self {
    self.router = Some(router);
    self
}
```

- [ ] **Step 3: Change the call site at line 212**

Replace:

```rust
.chat_with_role(ProviderRole::ReforgeRules, &messages, None, &params, &[])
```

with:

```rust
.chat(ProviderRole::ReforgeRules, &messages, None, &params, &[])
```

(method moves from `chat_with_role` on `ProviderManager` to `chat` on `ProviderRouter`; role unchanged).

- [ ] **Step 4: Update test in `crates/coding-memory/tests/skill_evolver_llm_drafted.rs`**

The test currently asserts the source contains `ProviderRole::ReforgeRules` — that's still true (the role didn't change). Run it to confirm:

```bash
cargo nextest run -p coding-memory skill_evolver_llm_drafted
```

If it fails because the test also asserts `chat_with_role`, replace `chat_with_role` with `chat` in the test's source-string assertion.

- [ ] **Step 5: Update upstream construction**

```bash
grep -rn "with_provider_manager\|SkillManager::new\|SkillManager {" crates/ src/ 2>/dev/null
```

Each site should now call `.with_router(state.router.clone())` instead of `.with_provider_manager(...)`.

- [ ] **Step 6: Run coding-memory tests**

```bash
cargo nextest run -p coding-memory
```

Expected: green.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/skills.rs crates/coding-memory/tests/ crates/app-core/
git commit -m "refactor(coding-memory): skill evolver takes ProviderRouter"
```

---

## Phase 5 — Migrate call sites: Batch B (cognitive crate, 2 sites)

### Task 11: Migrate `cognitive/src/services/session_memory.rs`

**Files:**
- Modify: `crates/cognitive/src/services/session_memory.rs` (~line 202, plus the config struct holding the provider)

- [ ] **Step 1: Locate the chat call and the provider field**

```bash
grep -n "provider\|chat\|DynProvider" crates/cognitive/src/services/session_memory.rs | head -20
```

- [ ] **Step 2: Change config struct**

Find `pub struct SessionMemoryConfig` (or whichever struct holds the optional provider). Change:

```rust
pub provider: Option<DynProvider>,
```

to:

```rust
pub router: Option<Arc<dyn providers::ProviderRouter>>,
```

- [ ] **Step 3: Change the call site at ~line 202**

Replace:

```rust
provider.chat(&messages, None, &params, &[]).await
```

with:

```rust
router.chat(providers::ProviderRole::Cognitive, &messages, None, &params, &[]).await
```

- [ ] **Step 4: Update callers of `SessionMemoryConfig`**

```bash
grep -rn "SessionMemoryConfig\|SessionMemoryService::start" crates/ src/ 2>/dev/null
```

Each construction site replaces `provider: cognitive_provider.clone()` with `router: Some(state.router.clone())`.

- [ ] **Step 5: Run cognitive tests**

```bash
cargo nextest run -p cognitive
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/session_memory.rs crates/app-core/
git commit -m "refactor(cognitive): session_memory uses ProviderRouter (Cognitive role)"
```

---

### Task 12: Migrate `cognitive/src/services/atom_extraction.rs`

Same pattern as Task 11. The call is at ~line 608, and the service constructor is `AtomExtractionService::start`.

**Files:**
- Modify: `crates/cognitive/src/services/atom_extraction.rs`

- [ ] **Step 1: Replace provider field/param with router**

Same edits as Task 11 — `DynProvider` field becomes `Arc<dyn ProviderRouter>`, `provider.chat(...)` becomes `router.chat(ProviderRole::Cognitive, ...)`.

- [ ] **Step 2: Update upstream wiring**

```bash
grep -rn "AtomExtractionService::start\|AtomExtractionService {" crates/ src/ 2>/dev/null
```

Replace `provider: cognitive_provider.clone()` with `router: state.router.clone()`.

- [ ] **Step 3: Run cognitive tests**

```bash
cargo nextest run -p cognitive
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/atom_extraction.rs crates/app-core/
git commit -m "refactor(cognitive): atom_extraction uses ProviderRouter (Cognitive role)"
```

---

## Phase 6 — Migrate call sites: Batch C (agent adapters, ~17 sites)

This batch refactors `crates/agent/src/adapters/*` — the largest concentration of sites. The pattern is mechanical; for each handler:

1. Change the field type: `provider: DynProvider` → `router: Arc<dyn providers::ProviderRouter>`.
2. Change the constructor signature accordingly.
3. Change every `.chat(messages, ...)` call to `.chat(ProviderRole::X, messages, ...)`.
4. Update the upstream constructor caller in `app-core/handlers/cognitive/mod.rs` or `app-core/init/*`.

**Pattern reference** (used by every task in this phase):

```rust
// Before:
pub struct LlmFooHandler { provider: DynProvider, params: ChatParams }
impl LlmFooHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self { provider, params: params.with_response_format(ResponseFormat::JsonObject) }
    }
}
// call site:
self.provider.chat(&messages, None, &self.params, &[]).await

// After:
pub struct LlmFooHandler { router: Arc<dyn providers::ProviderRouter>, params: ChatParams }
impl LlmFooHandler {
    pub fn new(router: Arc<dyn providers::ProviderRouter>, params: ChatParams) -> Self {
        Self { router, params: params.with_response_format(ResponseFormat::JsonObject) }
    }
}
// call site:
self.router.chat(ProviderRole::Foo, &messages, None, &self.params, &[]).await
```

### Task 13: Migrate `agent/src/adapters/cognitive_handlers.rs` (12 handlers, 12 sites)

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/app-core/src/handlers/cognitive/mod.rs` (constructor wiring)

Per-site role mapping:

| Line | Handler | Role |
|---|---|---|
| 254 | `LlmConflictResolver` | `Cognitive` |
| 552 | `LlmExtractionHandler` | `Cognitive` |
| 806 | `LlmConsolidationHandler` | `Consolidate` |
| 915 | `LlmGraphLinkHandler` | `GraphLink` |
| 1081 | `LlmDeepConsolidationHandler` | `Consolidate` |
| 1266 | `LlmCoachingReasonerHandler` | `Coach` |
| 1343 | `LlmExtractionCriticHandler` | `Cognitive` |
| 1414 | `LlmCommunityMembershipHandler` | `GraphLink` (verify-then-route — see Step 0) |
| 1504 | `LlmMicroReforgeHandler` | `Consolidate` |
| 1554 | `LlmQueryPredictorHandler` | `Cognitive` |
| 1646 | `LlmHierarchicalSummarizer` | `Cognitive` |
| 1694 | `LlmTemporalPrunerHandler` | `Cognitive` |

- [ ] **Step 0: Verify `LlmCommunityMembershipHandler` is alive**

```bash
grep -rn "LlmCommunityMembershipHandler::new\|LlmCommunityMembershipHandler {" crates/ src/ 2>/dev/null | grep -v "test\|/tests/"
```

If the only matches are inside test modules, mark this handler for deletion (note in the commit message and skip the migration). If a production caller exists, route the migration to `GraphLink` per the table above.

- [ ] **Step 1: For each of the 12 handlers, apply the pattern reference**

Use a per-handler checklist (12 sub-checkboxes — keep the granularity):

  - [ ] LlmConflictResolver — `provider` → `router`; call site `.chat(ProviderRole::Cognitive, ...)`.
  - [ ] LlmExtractionHandler — `provider` → `router`; `.chat(ProviderRole::Cognitive, ...)`.
  - [ ] LlmConsolidationHandler — `.chat(ProviderRole::Consolidate, ...)`.
  - [ ] LlmGraphLinkHandler — `.chat(ProviderRole::GraphLink, ...)`.
  - [ ] LlmDeepConsolidationHandler — `.chat(ProviderRole::Consolidate, ...)`.
  - [ ] LlmCoachingReasonerHandler — `.chat(ProviderRole::Coach, ...)`.
  - [ ] LlmExtractionCriticHandler — `.chat(ProviderRole::Cognitive, ...)`.
  - [ ] LlmCommunityMembershipHandler — `.chat(ProviderRole::GraphLink, ...)` OR delete per Step 0.
  - [ ] LlmMicroReforgeHandler — `.chat(ProviderRole::Consolidate, ...)`.
  - [ ] LlmQueryPredictorHandler — `.chat(ProviderRole::Cognitive, ...)`.
  - [ ] LlmHierarchicalSummarizer — `.chat(ProviderRole::Cognitive, ...)`.
  - [ ] LlmTemporalPrunerHandler — `.chat(ProviderRole::Cognitive, ...)`.

For each: open the file, jump to the listed line number, apply the field/constructor/call-site change per the pattern reference at the top of Phase 6.

- [ ] **Step 2: Update `crates/app-core/src/handlers/cognitive/mod.rs`**

Locate `build_*` constructor functions:

```bash
grep -n "pub fn build_" crates/app-core/src/handlers/cognitive/mod.rs
```

Each `build_*_handler` function takes a `cognitive_provider: DynProvider` parameter and constructs the struct. Change the parameter to `router: Arc<dyn providers::ProviderRouter>`, and pass `router` (not `cognitive_provider`) to the handler constructor.

The callers of `build_*_handler` (`app-core/init/mod.rs`, `app-core/init/cron.rs`, etc.) pass `state.cognitive_provider.clone()`; update them to `state.router.clone()`.

- [ ] **Step 3: Run agent tests**

```bash
cargo nextest run -p agent -p app-core
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs crates/app-core/src/handlers/cognitive/mod.rs crates/app-core/src/init/
git commit -m "refactor(agent): cognitive_handlers use ProviderRouter (12 sites, role-tagged)"
```

---

### Task 14: Migrate `agent/src/adapters/reforge_handlers.rs` (5 handlers, 7 sites)

**Files:**
- Modify: `crates/agent/src/adapters/reforge_handlers.rs`

Per-site role mapping:

| Line | Handler / method | Role |
|---|---|---|
| 275 | `LlmReforgeHandler::synthesize` | `ReforgeSynth` |
| 293 | `LlmReforgeHandler::review` | `ReforgeReview` |
| 320 | `LlmReforgeHandler::narrate` | `Cognitive` |
| 417 | `LlmGraphEnrichmentHandler` | `GraphLink` |
| 537 | `LlmGraphEnrichmentHandler` (community-naming) | `GraphLink` |
| 642 | `LlmCrossCliSynthesisHandler` | `Cognitive` |
| 745 | `LlmSkillDiscoveryHandler` | `Cognitive` |

Note: `LlmReforgeHandler` has **3 methods**, each stamping a different role. The struct holds one `router` field; only the call sites differ.

- [ ] **Step 1: Apply pattern to LlmReforgeHandler**

Field rename + constructor change as per Phase 6 pattern reference. At lines 275, 293, 320, the three calls become:

```rust
// :275 (synthesize)
self.router.chat(ProviderRole::ReforgeSynth, &messages, None, &self.params, &[]).await
// :293 (review)
self.router.chat(ProviderRole::ReforgeReview, &messages, None, &self.params, &[]).await
// :320 (narrate — note: prose, not JSON, so this method does NOT use the with_response_format params; verify in code)
self.router.chat(ProviderRole::Cognitive, &messages, None, &self.params_for_narrate, &[]).await
```

If the narrate method shares `self.params` (which has `JsonObject` response format set), narration may currently rely on a different params struct. Check the actual code; the goal is to keep behavior identical except for the role tag.

- [ ] **Step 2: Apply pattern to LlmGraphEnrichmentHandler**

Two call sites in the same handler — both stamp `GraphLink`. Apply pattern.

- [ ] **Step 3: Apply pattern to LlmCrossCliSynthesisHandler**

One call site (line 642). Stamp `Cognitive`. Apply pattern.

- [ ] **Step 4: Apply pattern to LlmSkillDiscoveryHandler**

One call site (line 745). Stamp `Cognitive`. Apply pattern.

- [ ] **Step 5: Update upstream wiring**

```bash
grep -rn "LlmReforgeHandler::new\|LlmGraphEnrichmentHandler::new\|LlmCrossCliSynthesisHandler::new\|LlmSkillDiscoveryHandler::new" crates/ src/ 2>/dev/null
```

Each constructor caller should now receive `router: Arc<dyn ProviderRouter>` instead of `provider: DynProvider`.

- [ ] **Step 6: Run agent tests**

```bash
cargo nextest run -p agent
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/adapters/reforge_handlers.rs crates/app-core/
git commit -m "refactor(agent): reforge_handlers use ProviderRouter (7 sites, role-tagged)"
```

---

### Task 15: Migrate other `agent/src/adapters/` files (5 files, 5 sites)

**Files:**
- Modify: `crates/agent/src/adapters/llm_summary.rs` (line 110) — `Cognitive`
- Modify: `crates/agent/src/adapters/query_rewriter.rs` (line 538) — `Cognitive`
- Modify: `crates/agent/src/adapters/multi_query.rs` (line 144) — `Cognitive`
- Modify: `crates/agent/src/adapters/llm_rerank.rs` (line 126) — `Cognitive`
- Modify: `crates/agent/src/adapters/productivity.rs` (3 sites: lines 33, 50, 72) — all `Cognitive`
- Modify: `crates/agent/src/adapters/mirror_handlers.rs` (lines 61, 79) — both `Cognitive`

For each file, apply the Phase 6 pattern reference. Per-file checklist:

- [ ] llm_summary.rs — field, constructor, call site at line 110 → `Cognitive`.
- [ ] query_rewriter.rs — field is `Option<DynProvider>`; becomes `Option<Arc<dyn ProviderRouter>>`. Call at line 538 → `Cognitive`.
- [ ] multi_query.rs — same `Option<DynProvider>` pattern. Call at line 144 → `Cognitive`.
- [ ] llm_rerank.rs — same pattern. Call at line 126 → `Cognitive`.
- [ ] productivity.rs — single `provider` field; 3 call sites (33, 50, 72), all `Cognitive`. **Note:** this is the file that today receives the *primary* `DynProvider` not `cognitive_provider` per the discovery scan; switching to `Cognitive` role is correct.
- [ ] mirror_handlers.rs — `LlmNarrativeHandler` field; 2 call sites (61, 79), both `Cognitive`.

- [ ] **Step 1: Run agent tests after each file**

```bash
cargo nextest run -p agent
```

Expected: pass.

- [ ] **Step 2: Commit per file (or batched if all green)**

```bash
git add crates/agent/src/adapters/{llm_summary,query_rewriter,multi_query,llm_rerank,productivity,mirror_handlers}.rs crates/app-core/
git commit -m "refactor(agent): adapters use ProviderRouter (8 sites, all Cognitive)"
```

---

## Phase 7 — Migrate call sites: Batch D (agent handlers + execution + autotuner + subagent)

### Task 16: Migrate `agent/src/handlers/coding_synthesis.rs` and `rule_artifacts.rs`

Both are single-call handlers with a `provider: DynProvider` field. Apply the Phase 6 pattern.

- [ ] **Step 1: coding_synthesis.rs at line 52** → `ReforgeSynth`.
- [ ] **Step 2: rule_artifacts.rs at line 49** → `ReforgeRules`.
- [ ] **Step 3: Update upstream callers** — `app-core/src/init/cron.rs` (lines ~638 and ~660 per the discovery scan).
- [ ] **Step 4: Run agent + app-core tests**

```bash
cargo nextest run -p agent -p app-core
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/handlers/{coding_synthesis,rule_artifacts}.rs crates/app-core/src/init/cron.rs
git commit -m "refactor(agent): coding_synthesis + rule_artifacts handlers via router"
```

---

### Task 17: Migrate `agent/src/execution/core.rs` (main agent loop)

**Files:**
- Modify: `crates/agent/src/execution/core.rs` (lines 289 streaming, 588 non-streaming)
- Modify: `crates/agent/src/agent_loop/builder.rs` (the wiring point that injects the provider into ExecutionCore)

This is the highest-stakes migration in the plan: it's the main user-facing chat loop. **The role here is `Default`.**

- [ ] **Step 1: Change `ExecutionCore.provider` field**

Find the struct definition:

```bash
grep -n "pub struct ExecutionCore\|provider:" crates/agent/src/execution/core.rs | head
```

Change:

```rust
pub struct ExecutionCore {
    provider: DynProvider,
    // ...
}
```

to:

```rust
pub struct ExecutionCore {
    router: Arc<dyn providers::ProviderRouter>,
    // ...
}
```

Update the constructor signature accordingly.

- [ ] **Step 2: Change call sites**

Line 289 (streaming):

```rust
self.provider.chat_stream(messages, tools, &params, breakpoints).await
```

becomes:

```rust
self.router.chat_stream(providers::ProviderRole::Default, messages, tools, &params, breakpoints).await
```

Line 588 (non-streaming fallback):

```rust
self.provider.chat(messages, tools, &params, breakpoints).await
```

becomes:

```rust
self.router.chat(providers::ProviderRole::Default, messages, tools, &params, breakpoints).await
```

- [ ] **Step 3: Update `agent_loop/builder.rs`**

```bash
grep -n "ExecutionCore::new\|provider\.clone" crates/agent/src/agent_loop/builder.rs | head
```

Replace `provider.clone()` with `router.clone()` at the construction site (line ~1797 per discovery). The `AgentLoopBuilder` itself takes `router: Arc<dyn ProviderRouter>` as a parameter (replacing the existing `DynProvider` parameter).

- [ ] **Step 4: Update `AgentLoop::new` callers**

```bash
grep -rn "AgentLoop::new\|AgentLoopBuilder" crates/ src/ 2>/dev/null
```

Each caller passes `state.router.clone()` (or wherever the router lives at the call site) instead of `state.provider.clone()`.

- [ ] **Step 5: Update the `DecomposerLlmAdapter` at builder.rs:60**

This is the InsightForge query-decomposition adapter. Migrate its provider field to a router field; stamp `Cognitive`.

- [ ] **Step 6: Run agent tests AND simulation tests**

```bash
cargo nextest run -p agent
cargo nextest run -E 'test(simulation)'
```

Expected: pass. The simulation suite is the agent-loop smoke test; if it fails, the wiring is wrong.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/core.rs crates/agent/src/agent_loop/builder.rs crates/app-core/
git commit -m "refactor(agent): main loop uses ProviderRouter::Default + chat_stream"
```

---

### Task 18: Migrate `agent/src/autotuner/mod.rs` (was a bug — primary→Cognitive)

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs` (line 771)
- Modify: `crates/app-core/src/init/cron.rs` (line ~88, the autotuner wiring)

Per discovery: autotuner currently uses the *expensive primary* for nightly trial generation. Migrating to `Cognitive` role is intentional — fixes a cost-bug.

- [ ] **Step 1: Apply pattern to AutoTunerOrchestrator**

Field `provider: DynProvider` → `router: Arc<dyn ProviderRouter>`. Call at line 771 → `ProviderRole::Cognitive`.

- [ ] **Step 2: Update wiring at `init/cron.rs:88`**

Replace `provider.clone()` with `state.router.clone()`.

- [ ] **Step 3: Run agent tests**

```bash
cargo nextest run -p agent autotuner
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/autotuner/mod.rs crates/app-core/src/init/cron.rs
git commit -m "fix(agent): autotuner uses Cognitive role (was unintentionally on primary)"
```

---

### Task 19: Migrate `agent/src/subagent.rs`

**Files:**
- Modify: `crates/agent/src/subagent.rs`

Subagent delegates to `ExecutionCore`; the migration is structural — it holds a `provider: DynProvider` (line ~656 and the `SubagentManager.provider` field). Replace with router.

- [ ] **Step 1: Apply pattern**

Field `provider: DynProvider` → `router: Arc<dyn ProviderRouter>`. Pass `router.clone()` into `ExecutionCore::new`.

- [ ] **Step 2: Update wiring at `agent_loop/builder.rs:630`**

`Arc::clone(&provider)` → `Arc::clone(&router)`.

- [ ] **Step 3: Run agent tests**

```bash
cargo nextest run -p agent
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/subagent.rs crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): subagent uses ProviderRouter"
```

---

## Phase 8 — Migrate call sites: Batch E (app-core handlers, ~13 sites)

### Task 20: Migrate `app-core/src/handlers/notes/insight.rs` (6 chat sites)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

Per spec, all sites in this file stamp `NotesGen`.

Per-site checklist:

- [ ] Line 203 (changes-summary) → `NotesGen`
- [ ] Line 365 (changes-summary `.chat`) → `NotesGen`
- [ ] Line 575 (scenario challenge) → `NotesGen`
- [ ] Line 680 (regenerate tab) → `NotesGen`
- [ ] Line 1197 (insight pipeline streaming `.chat_stream`) → `NotesGen`
- [ ] Line 1259 (insight pipeline non-streaming `.chat`) → `NotesGen`

For each: replace `self.cognitive_provider.chat(...)` (or the local provider arg) with `self.router.chat(ProviderRole::NotesGen, ...)`. Stream sites use `chat_stream`.

The wiring at line 121 (`generate_insight` picks provider from state) is updated to read `state.router` instead of `state.cognitive_provider`.

- [ ] **Step 1: Update the struct field**

`InsightHandler.cognitive_provider: DynProvider` → `InsightHandler.router: Arc<dyn ProviderRouter>`.

- [ ] **Step 2: Apply per-site changes**

Per the checklist above.

- [ ] **Step 3: Update `InsightPipelineArgs` struct (if it carries the provider)**

Replace `provider: DynProvider` with `router: Arc<dyn ProviderRouter>`.

- [ ] **Step 4: Update upstream construction**

```bash
grep -rn "InsightHandler::new\|InsightHandler {" crates/ 2>/dev/null
```

Replace `cognitive_provider` arg with `router`.

- [ ] **Step 5: Run app-core tests**

```bash
cargo nextest run -p app-core
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "refactor(app-core/notes): insight handler uses NotesGen role"
```

---

### Task 21: Migrate `app-core/src/handlers/notes/insight_chat.rs` (1 site)

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_chat.rs:37`

- [ ] **Step 1: Apply pattern → `NotesGen` (streaming `chat_stream`).**
- [ ] **Step 2: Update upstream construction.**
- [ ] **Step 3: Run tests.**

```bash
cargo nextest run -p app-core
```

- [ ] **Step 4: Commit.**

```bash
git add crates/app-core/src/handlers/notes/insight_chat.rs
git commit -m "refactor(app-core/notes): insight_chat streaming uses NotesGen role"
```

---

### Task 22: Migrate `app-core/src/handlers/notes/{card_generation,practice,grading,distractors,language}.rs`

All sites stamp `Cognitive`. Per-file per-line checklist:

- [ ] `card_generation.rs:155` → `Cognitive`
- [ ] `practice.rs:244` → `Cognitive`
- [ ] `grading.rs:294` → `Cognitive`
- [ ] `grading.rs:355` → `Cognitive`
- [ ] `distractors.rs:85` → `Cognitive`
- [ ] `language.rs:37` → `Cognitive`
- [ ] `language.rs:102` → `Cognitive`
- [ ] `language.rs:336` → `Cognitive`
- [ ] `language.rs:376` → `Cognitive`
- [ ] `language.rs:419` → `Cognitive`

Several of these read the provider via `cognitive_chat_context()` (a state helper). Update that helper to return `(router, params)` instead of `(provider, params)`.

- [ ] **Step 1: Update `cognitive_chat_context()` on `AppCore`**

```bash
grep -n "fn cognitive_chat_context" crates/app-core/src/state.rs
```

Change return type from `(DynProvider, ChatParams)` to `(Arc<dyn ProviderRouter>, ChatParams)`. The implementation reads `self.router.clone()` instead of `self.cognitive_provider.clone()`.

- [ ] **Step 2: Apply per-file changes**

Each call site that previously did `provider.chat(...)` now does `router.chat(ProviderRole::Cognitive, ...)`.

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p app-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/ crates/app-core/src/state.rs
git commit -m "refactor(app-core/notes): card/practice/grading/distractors/language → Cognitive role"
```

---

### Task 23: Migrate `app-core/src/coding/title_service.rs`

**Files:**
- Modify: `crates/app-core/src/coding/title_service.rs:24` (struct param) and `:73` (chat call)

Auto-titles use `Cognitive` (collapsed from the original `Title` proposal).

- [ ] **Step 1: Change function signature**

```rust
pub async fn autogenerate_title(
    router: Arc<dyn providers::ProviderRouter>,
    /* other args */
) -> Result<String> { ... }
```

- [ ] **Step 2: Change call site at line 73**

```rust
router.chat(providers::ProviderRole::Cognitive, &messages, None, &params, &[]).await
```

- [ ] **Step 3: Update the test harness in this file**

The existing tests construct a fake provider; they now need to construct a fake `ProviderRouter`. Provide a small `MockRouter` test struct (or reuse a fake from `crates/providers/tests/`).

- [ ] **Step 4: Wire the call site that invokes `autogenerate_title`**

Per the discovery scan, the function is "complete and tested but not yet wired to any real caller (only called from its own test module)." The plan in `docs/superpowers/plans/2026-05-07-coding-sidebar-titles-and-running-state.md` will add a real caller in `coding_message_send`. For this migration we just ensure the function signature matches what the future caller will expect.

- [ ] **Step 5: Run tests**

```bash
cargo nextest run -p app-core title
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding/title_service.rs
git commit -m "refactor(app-core/coding): title_service takes ProviderRouter (Cognitive role)"
```

---

## Phase 9 — Migrate call sites: Batch F (app-core init wiring + dead code)

### Task 24: Clean up `app-core/init/coaching.rs` dead parameter

**Files:**
- Modify: `crates/app-core/src/init/coaching.rs:34`

Per the discovery scan: `_cognitive_provider: Option<&DynProvider>` is accepted but unused (underscore prefix).

- [ ] **Step 1: Remove the parameter from the function signature**

Find the function and delete `_cognitive_provider: Option<&DynProvider>` from its argument list.

- [ ] **Step 2: Update callers**

```bash
grep -rn "init_coaching\|init::coaching::" crates/ src/ 2>/dev/null
```

Remove the `cognitive_provider` argument at each call site.

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p app-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/coaching.rs crates/app-core/src/init/mod.rs
git commit -m "refactor(app-core): remove dead _cognitive_provider param from init_coaching"
```

---

### Task 25: Migrate `app-core/init/cron.rs:1764` nightly insight polish

**Files:**
- Modify: `crates/app-core/src/init/cron.rs:1764`

This is the nightly cross-domain insight polishing call inside a closure. Replace the captured `cognitive_provider` with the captured `router`, stamp `Cognitive`.

- [ ] **Step 1: Update the closure**

```rust
let router = state.router.clone();
// inside the closure:
router.chat(providers::ProviderRole::Cognitive, &messages, None, &params, &[]).await
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p app-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "refactor(app-core): nightly insight polish uses router (Cognitive role)"
```

---

### Task 26: Migrate `app-core/init/productivity.rs` constructor

**Files:**
- Modify: `crates/app-core/src/init/productivity.rs:124-126`

`ProductivityHandlerImpl` was migrated in Task 15 (Batch C); this task updates its constructor wiring in init.

- [ ] **Step 1: Replace `cognitive_provider` arg with `router`**
- [ ] **Step 2: Run tests**
- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/productivity.rs
git commit -m "refactor(app-core): productivity init wires router"
```

---

## Phase 10 — Final cleanup

### Task 27: Remove `AppCore.cognitive_provider` and the transitional `provider` field

After all batches, no code path holds `state.cognitive_provider` or `state.provider` — they exist only as transitional plumbing.

- [ ] **Step 1: Verify no remaining readers**

```bash
grep -rn "\.cognitive_provider\|state\.provider\b" crates/app-core/src/ crates/agent/ 2>/dev/null
```

Expected: no matches. If any remain, migrate them now (apply Phase 6 pattern with the appropriate role).

- [ ] **Step 2: Remove the fields from `AppCore`**

In `crates/app-core/src/state.rs:93`, remove:

```rust
    pub cognitive_provider: Option<providers::DynProvider>,
```

Find and remove `pub provider: providers::DynProvider` if present (or the analogous transitional field).

- [ ] **Step 3: Remove the fields from `StorageResult`**

In `crates/app-core/src/init/storage.rs`, remove `provider: providers::DynProvider` from the struct and from the `Ok(StorageResult { ... })` literal.

- [ ] **Step 4: Build the workspace**

```bash
cargo build --workspace
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/storage.rs
git commit -m "refactor(app-core): remove transitional cognitive_provider + provider fields"
```

---

### Task 28: Remove `ProviderManager::chat_with_role` inherent method

**Files:**
- Modify: `crates/providers/src/manager.rs:318-335`

After Task 10, no caller uses `chat_with_role` — it's dead inherent code. The trait method on `ProviderRouter` replaced it.

- [ ] **Step 1: Verify no callers**

```bash
grep -rn "chat_with_role" crates/ src/ 2>/dev/null
```

Expected: no matches outside this file.

- [ ] **Step 2: Delete the impl block**

In `crates/providers/src/manager.rs`, remove lines 318-335 (the entire `impl ProviderManager { pub async fn chat_with_role(...) ... }` block).

- [ ] **Step 3: Run providers tests**

```bash
cargo nextest run -p providers
```

- [ ] **Step 4: Commit**

```bash
git add crates/providers/src/manager.rs
git commit -m "refactor(providers): remove ProviderManager::chat_with_role (replaced by router)"
```

---

### Task 29: Verification gates

- [ ] **Step 1: Workspace build clean**

```bash
cargo build --workspace
```

Expected: zero errors.

- [ ] **Step 2: Workspace tests green**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

Expected: all tests pass.

- [ ] **Step 3: Clippy clean**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: zero warnings (per `CLAUDE.md` policy). The `desktop` crate has pre-existing exceptions; those are documented in its lib.rs and not affected by this work.

- [ ] **Step 4: Format check**

```bash
cargo fmt --all --check
```

Expected: clean.

- [ ] **Step 5: KCA gates**

```bash
./scripts/run_kca_validation.sh
```

Expected: pass. The cognitive pipeline gains failover; this is structurally a quality improvement, not a regression risk.

- [ ] **Step 6: Manual smoke test (browser-only dev)**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Open `localhost:1420`. Send a chat message. Open a coding session. Verify:
- Main chat works (`Default` role).
- Coding distiller fires after a turn (`Cognitive` role per Task 9).
- No new error logs about provider resolution.

---

### Task 30: Migration cheat-sheet for in-flight dev configs

**Files:**
- Create: `docs/superpowers/notes/2026-05-07-router-migration-cheatsheet.md`

Per the spec, the schema cut is hard. Other developers running this branch need a one-page guide.

- [ ] **Step 1: Write the cheat-sheet**

```markdown
# Router migration cheat-sheet

Your dev `~/.klyntbot/config.json` (or `~/.klyntbot-dev/config.json`)
will reject on startup after this branch lands. Here's how to fix it.

## Old config (delete this block)
```json
{
  "providerManager": { "fallback": "openai", "classifierModel": "claude-haiku" },
  "cognitive": { "model": "groq/llama-3", "provider": "groq", "graphLinkerModel": "..." }
}
```

## New config
```json
{
  "router": {
    "agentDefault":     { "primary": "anthropic", "fallbacks": ["openai"] },
    "cognitiveDefault": { "primary": "groq",      "fallbacks": ["deepseek"], "model": "llama-3" }
  },
  "cognitive": {
    "temperature": 0.2,
    "maxTokens": 1024
  }
}
```

The `cognitive` block keeps `temperature`, `maxTokens`, and all the
`*Limit` / `*Threshold` knobs. Only the `model`, `provider`,
`graphLinkerModel` fields move into `router`.

## Per-handler model overrides

If you used `coding_memory.reforge.synth_model` etc., move them into
`router.roles.<name>.model`:

```json
{
  "router": {
    "roles": {
      "reforge_synth": { "primary": "anthropic", "model": "claude-opus-4-7" }
    }
  }
}
```

## Verifying

`KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev` should boot with no
config errors.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/notes/2026-05-07-router-migration-cheatsheet.md
git commit -m "docs(notes): router migration cheat-sheet for in-flight dev configs"
```

---

## Self-review notes

- **Spec coverage:** Every spec section maps to at least one task. §5 (enum) → Task 1. §6 (trait/impl) → Tasks 2-3. §7 (config) → Tasks 4-6. §8 (migration plan, all batches) → Tasks 9-26. §9 (testing) → Tasks 1-3 unit tests + Task 29 verification. §10 (Phase 2 sketch) → no task; future work as documented.
- **Placeholder check:** None. Every step has either concrete code, a concrete grep command, or a concrete file:line target.
- **Type consistency:** All references to `Arc<dyn providers::ProviderRouter>` match. `ProviderRole::Cognitive`, `Default`, `ReforgeSynth`, `ReforgeRules`, `ReforgeReview`, `NotesGen`, `Coach`, `Consolidate`, `GraphLink` are the only role names used; spelling matches §5 of the spec.
- **Order dependency:** Phase 1 (router infra) and Phase 2 (config) can be done in parallel because they're independent. Phase 3 (factory + AppCore wiring) depends on both. Phase 4 onwards depends on Phase 3. The order in this plan is the safest sequential path; an ambitious engineer could do 1+2 in parallel.
- **Build will be red between Tasks 5 and 7** — config has no `provider_manager`, factory hasn't been rewritten yet. That's documented at the top of Task 7.
- **`Distiller` removal causes a brief red zone** between Task 1 and Tasks 9-10 — documented in Task 1's note. Engineer can do Tasks 1, 9, 10 in immediate succession to minimize the window.
- **Estimated total LOC change:** ~900 net addition (router infra + tests) – ~400 deletion (legacy factory, ProviderManagerConfig) ≈ +500 net, plus pure refactor at ~50 sites that's net-zero.
