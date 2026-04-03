# Simulator Full Integration Audit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all missing features, tools, and context sources into the simulator so every metric is based on real execution with zero mock-only paths remaining.

**Architecture:** The simulator has two execution paths — a heuristic path (always runs, uses `ActionExecutor` for DB writes) and an agent path (optional, uses real `AgentRuntime`). We fix the dual-path conflict, register all missing tools, add context sources, connect cognitive handler selection to scenario config, and replace fake token counts. Coaching and insights are wired at the level that exercises real code without requiring the full production service dependency graph.

**Tech Stack:** Rust, SQLite (in-memory), tokio, cognitive crate, feature-* crates, agent crate

---

### Task 1: Register ProductivityTool in Agent Harness

**Files:**
- Modify: `crates/simulator/src/harness.rs` (migrations section, ~line 103-130)
- Modify: `crates/simulator/src/agent_harness.rs:189-249` (register_tools)

- [ ] **Step 1: Add productivity migration to harness.rs**

In `crates/simulator/src/harness.rs`, inside the `if scenario.simulation.agent_mode {` block (after the activity_log migrations, ~line 129), add the productivity migration:

```rust
            // Productivity tables
            storage::StoragePool::run_feature_migrations(
                &inner_pool,
                &feature_productivity::ProductivityFeature::migrations_static(),
            )
            .await?;
```

- [ ] **Step 2: Register ProductivityTool in agent_harness.rs**

In `crates/simulator/src/agent_harness.rs`, inside `register_tools()` (after the WorkContextTool registration at line 243), add:

```rust
        // Productivity tool
        let prod_repos = feature_productivity::repos::ProductivityRepos::new(inner_pool.clone());
        let focus_mgr = std::sync::Arc::new(feature_productivity::FocusManager::new(
            prod_repos.clone(),
            config::FocusConfig::default(),
        ).with_domain_bus(Arc::clone(bus)));
        let aggregator = std::sync::Arc::new(
            feature_productivity::DailyAggregator::new(prod_repos.clone())
                .with_domain_bus(Arc::clone(bus)),
        );
        registry.register(feature_productivity::ProductivityTool::new(
            prod_repos,
            focus_mgr,
            aggregator,
        ));
```

- [ ] **Step 3: Build and check for compile errors**

Run: `cargo build -p simulator 2>&1 | head -30`
Expected: Successful build or minor import fixes needed.

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/harness.rs crates/simulator/src/agent_harness.rs
git commit -m "feat(simulator): register ProductivityTool in agent harness"
```

---

### Task 2: Register LearningTool in Agent Harness

**Files:**
- Modify: `crates/simulator/src/agent_harness.rs:189-249` (register_tools)

- [ ] **Step 1: Register LearningTool in register_tools()**

In `crates/simulator/src/agent_harness.rs`, inside `register_tools()` (after the ProductivityTool registration added in Task 1), add:

```rust
        // Learning tool (no handler — graceful no-op in simulation)
        registry.register(tools::LearningTool::new(None));
```

`LearningTool::new(None)` is valid — it returns a "no learning system configured" message when called without a handler. This registers the tool so the agent can discover it and attempt to use it, exercising the routing/tool-selection path.

- [ ] **Step 2: Build**

Run: `cargo build -p simulator 2>&1 | head -20`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/agent_harness.rs
git commit -m "feat(simulator): register LearningTool in agent harness"
```

---

### Task 3: Register MirrorTool and CronTool in Agent Harness

**Files:**
- Modify: `crates/simulator/src/agent_harness.rs:189-249` (register_tools)

- [ ] **Step 1: Register MirrorTool**

In `register_tools()`, add after the LearningTool:

```rust
        // Mirror tool (read-only access to self-reflection layer)
        let mirror_repo = cognitive::mirror::MirrorRepo::new(inner_pool.clone());
        let mirror_facade = std::sync::Arc::new(cognitive::mirror::MirrorFacade::new(mirror_repo));
        registry.register(tools::MirrorTool::new(mirror_facade));
```

- [ ] **Step 2: Register CronTool**

In `register_tools()`, add after MirrorTool:

```rust
        // Cron tool (no handler — read-only listing in simulation)
        registry.register(tools::CronTool::new());
```

`CronTool::new()` without a handler returns "scheduling not available" for mutation actions but is still discoverable. This exercises the agent's tool selection for automation-related messages.

- [ ] **Step 3: Build**

Run: `cargo build -p simulator 2>&1 | head -20`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/agent_harness.rs
git commit -m "feat(simulator): register MirrorTool and CronTool in agent harness"
```

---

### Task 4: Add Context Sources to ContextEngine

**Files:**
- Modify: `crates/simulator/src/agent_harness.rs:143-144` (ContextEngine construction)

The current code creates an empty ContextEngine:
```rust
let context_engine = Arc::new(context_engine::ContextEngine::new());
```

We need to add real context sources so the agent receives cognitive facts, identity info, and productivity context — matching production behavior.

- [ ] **Step 1: Replace the empty ContextEngine with one that has sources**

In `crates/simulator/src/agent_harness.rs`, replace the ContextEngine construction (line 143-144) with:

```rust
        // Build context engine with real sources matching production
        let context_sources: Vec<Box<dyn context_engine::source::ContextSource>> = vec![
            Box::new(agent::context_sources::IdentitySource::new(
                std::path::PathBuf::from("/tmp/klyntbot-sim"),
                "UTC".to_string(),
            )),
            Box::new(cognitive::CognitiveContextSource::new(
                cognitive::SemanticFactRepo::new(inner_pool.clone()),
                cognitive::ProceduralRuleRepo::new(inner_pool.clone()),
            )),
            Box::new(agent::context_sources::ProductivityContextSource::new(
                feature_productivity::repos::ProductivityRepos::new(inner_pool.clone()),
            )),
        ];
        let context_engine = Arc::new(
            context_engine::ContextEngine::new().with_sources(context_sources),
        );
```

This gives the agent:
- **IdentitySource** (priority 100): Runtime info (date, timezone, workspace)
- **CognitiveContextSource** (priority 90): Known facts and procedural rules from memory
- **ProductivityContextSource** (priority 55): Focus session state, daily patterns

- [ ] **Step 2: Build**

Run: `cargo build -p simulator 2>&1 | head -20`
Expected: Clean build. The `agent::context_sources` module is already importable since `agent` is a dependency.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/agent_harness.rs
git commit -m "feat(simulator): add cognitive and productivity context sources to agent"
```

---

### Task 5: Connect Cognitive Handler Selection to Scenario Config

**Files:**
- Modify: `tests/simulation/smoke.rs:93-105` (run_scenario function)

Currently `run_scenario()` hardcodes all four cognitive handlers to heuristic mode, ignoring `CognitiveBridgeConfig`. We fix this so that when `cognitive_llm_model` is set to a real model name in the TOML (or via `SIMULATION_COGNITIVE_LLM` env var), real LLM handlers are used.

- [ ] **Step 1: Modify run_scenario() to respect CognitiveBridgeConfig**

Replace the `run_scenario` function in `tests/simulation/smoke.rs`:

```rust
async fn run_scenario(toml: &str) -> SimulationReport {
    let scenario = Scenario::from_toml(toml).unwrap();
    let bridge_config =
        simulator::providers::CognitiveBridgeConfig::from(&scenario.simulation);

    let (extraction, consolidation, reflection): (
        Arc<dyn ExtractionHandler>,
        Arc<dyn ConsolidationHandler>,
        Arc<dyn ReflectionHandler>,
    ) = if bridge_config.is_heuristic() {
        (
            Arc::new(HeuristicExtractionHandler),
            Arc::new(HeuristicConsolidationHandler),
            Arc::new(HeuristicReflectionHandler),
        )
    } else {
        // Real LLM handlers — provider determined by cognitive_llm_model
        let provider = create_cognitive_provider(&bridge_config);
        let params = providers::types::ChatParams::new(&bridge_config.model)
            .with_temperature(bridge_config.temperature as f32);
        (
            Arc::new(klyntbot::agent::cognitive_handlers::LlmExtractionHandler::new(
                provider.clone(),
                params.clone(),
            )) as Arc<dyn ExtractionHandler>,
            Arc::new(klyntbot::agent::cognitive_handlers::LlmConsolidationHandler::new(
                provider.clone(),
                params.clone(),
            )) as Arc<dyn ConsolidationHandler>,
            Arc::new(klyntbot::agent::cognitive_handlers::LlmReflectionHandler::new(
                provider,
                params,
            )) as Arc<dyn ReflectionHandler>,
        )
    };

    let narrative: Arc<dyn cognitive::mirror::NarrativeHandler> =
        Arc::new(HeuristicNarrativeHandler);
    let harness =
        SimulationHarness::new(scenario, extraction, consolidation, reflection, narrative)
            .await
            .unwrap();
    harness.run().await.unwrap()
}

/// Create an LLM provider for cognitive handlers based on bridge config.
fn create_cognitive_provider(
    config: &simulator::providers::CognitiveBridgeConfig,
) -> providers::DynProvider {
    // Try to find a matching provider from the registry
    let model = &config.model;
    // Common pattern: model name like "deepseek-chat" → provider "deepseek"
    // or "claude-haiku-4-5-20251001" → provider "anthropic"
    let provider_name = if model.contains("claude") || model.contains("anthropic") {
        "anthropic"
    } else if model.contains("deepseek") {
        "deepseek"
    } else if model.contains("gpt") || model.contains("o1") || model.contains("o3") {
        "openai"
    } else {
        "deepseek" // default fallback
    };

    let spec = providers::ProviderRegistry::find_by_name(provider_name)
        .expect("unknown cognitive provider");
    let api_key = std::env::var(spec.env_key).unwrap_or_default();

    if provider_name == "anthropic" {
        Arc::new(providers::AnthropicNativeProvider::new(
            config::Secret::new(api_key),
            spec.default_api_base.to_string(),
            model.to_string(),
        ))
    } else {
        Arc::new(
            providers::OpenAiCompatProvider::new(spec.default_api_base, api_key, model)
                .expect("failed to create cognitive provider"),
        )
    }
}
```

Add the necessary imports at the top of the file:

```rust
use std::sync::Arc;

use cognitive::{ConsolidationHandler, ExtractionHandler, ReflectionHandler};
use klyntbot::agent::cognitive_handlers::{
    HeuristicConsolidationHandler, HeuristicExtractionHandler, HeuristicReflectionHandler,
};
use simulator::harness::SimulationHarness;
use simulator::providers::HeuristicNarrativeHandler;
use simulator::report::SimulationReport;
use simulator::scenario::Scenario;
```

Note: The existing imports already cover most of these. Only the `providers` and `config` imports may need adding if not already present. Check and add as needed:

```rust
use providers;
use config;
```

- [ ] **Step 2: Build**

Run: `cargo build --test simulation 2>&1 | head -30`
Expected: Clean build. If `providers::DynProvider` or other types are not in scope, add the necessary `use` statements.

- [ ] **Step 3: Run the smoke test to verify heuristic mode still works**

Run: `cargo nextest run --test simulation smoke_test_7_day 2>&1 | tail -20`
Expected: PASS — heuristic mode is the default, so existing tests should be unchanged.

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/smoke.rs
git commit -m "feat(simulator): connect cognitive handler selection to scenario config"
```

---

### Task 6: Fix Dual-Path DB Conflict

**Files:**
- Modify: `crates/simulator/src/harness.rs` (~line 555-638, tool action execution block)

When `agent_mode` is enabled, both the heuristic `ActionExecutor` and the agent's real tool execution write to the same DB — creating duplicate/conflicting data. We skip the heuristic tool actions when the agent handles them.

- [ ] **Step 1: Wrap heuristic tool action execution in a non-agent-mode guard**

In `crates/simulator/src/harness.rs`, find the block starting at approximately line 555:

```rust
                // Execute tool actions.
                let error_injection_rate =
                    persona_runner.current_phase_config().error_injection_rate;
```

Wrap the entire tool action loop (from line 555 through approximately line 638, ending before the cognitive extraction call) in a conditional:

```rust
                // Execute tool actions — only in heuristic mode.
                // When agent_mode is active, the AgentRuntime executes tools
                // via its own pipeline, so we skip the heuristic action executor
                // to avoid duplicate DB writes and conflicting state.
                if self.agent_harness.is_none() {
                    let error_injection_rate =
                        persona_runner.current_phase_config().error_injection_rate;
                    // ... existing tool action loop unchanged ...
                }
```

Make sure the entire `for action in &msg.tool_actions { ... }` block, including the task tracking, salience recording, and domain event publishing, is inside the `if` guard.

The variables used after this block (`msg_had_error_injection`) need a default value outside the guard:

```rust
                let mut msg_had_error_injection = false;
                if self.agent_harness.is_none() {
                    let error_injection_rate =
                        persona_runner.current_phase_config().error_injection_rate;
                    for action in &msg.tool_actions {
                        // ... existing code ...
                    }
                }
```

- [ ] **Step 2: Build**

Run: `cargo build -p simulator 2>&1 | head -20`
Expected: Clean build. Check that `msg_had_error_injection` is still accessible after the guard.

- [ ] **Step 3: Run smoke test**

Run: `cargo nextest run --test simulation smoke_test_7_day 2>&1 | tail -10`
Expected: PASS — smoke test doesn't use agent_mode, so behavior is unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/harness.rs
git commit -m "fix(simulator): skip heuristic tool actions when agent_mode is active"
```

---

### Task 7: Replace Fake Token Counts with Message-Length Estimates

**Files:**
- Modify: `crates/simulator/src/harness.rs` (~line 981-983, token tracking)
- Modify: `crates/simulator/src/harness.rs` (~line 947-956, usage records)

The current code uses a deterministic formula: `150 + ((day * 7 + msg_idx) % 120)`. We replace this with a message-length-based estimate (≈4 chars per token, typical for English text).

- [ ] **Step 1: Replace the fake token formula**

In `crates/simulator/src/harness.rs`, find the token tracking lines (~line 981):

```rust
                let simulated_tokens = 150u64 + ((day_counter as u64 * 7 + msg_idx as u64) % 120);
                metrics.accumulator_mut().total_tokens += simulated_tokens;
```

Replace with:

```rust
                // Estimate tokens from message length (~4 chars/token for English)
                // plus a base overhead for system prompt / tool definitions.
                let prompt_tokens = (msg.content.len() as u64 / 4).max(20) + 80;
                let completion_tokens = prompt_tokens / 3 + 30; // rough ratio
                let estimated_tokens = prompt_tokens + completion_tokens;
                metrics.accumulator_mut().total_tokens += estimated_tokens;
```

- [ ] **Step 2: Fix the usage record fake values**

Find the usage record insertion (~line 947-956) where `prompt_tokens: 100` and `completion_tokens: 50` are hardcoded. Update to use the same estimates:

Replace the hardcoded values with the `prompt_tokens` and `completion_tokens` computed above. If the variables are not in scope at that point, move the estimation earlier or duplicate the calculation.

- [ ] **Step 3: Build and run smoke test**

Run: `cargo build -p simulator && cargo nextest run --test simulation smoke_test_7_day 2>&1 | tail -10`
Expected: PASS — the smoke test asserts `token_efficiency` varies from 150.0, which it now will since estimates are content-length-based.

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/harness.rs
git commit -m "fix(simulator): replace fake token counts with message-length estimates"
```

---

### Task 8: Wire Coaching Event Tracking

**Files:**
- Modify: `crates/simulator/Cargo.toml` (add feature-coaching dependency)
- Modify: `crates/simulator/src/harness.rs` (add coaching subscriber)

The coaching feature is an event-driven background service. We subscribe the `PatternDetector` to domain events during simulation so coaching detection patterns are exercised with real data.

- [ ] **Step 1: Add feature-coaching dependency**

In `crates/simulator/Cargo.toml`, add to `[dependencies]`:

```toml
feature-coaching.workspace = true
```

- [ ] **Step 2: Check PatternDetector API**

Read `crates/feature-coaching/src/pattern_detector.rs` to understand the constructor and how it processes domain events. We need to know:
- What `PatternDetector::new()` takes
- How it receives events
- What output it produces

Adapt the following steps based on what you find.

- [ ] **Step 3: Add coaching event subscriber to harness**

In `crates/simulator/src/harness.rs`, after the domain event bus creation, subscribe a coaching pattern detector. The exact implementation depends on the `PatternDetector` API discovered in Step 2. The general pattern:

```rust
// Subscribe coaching pattern detector to domain events
let coaching_rx = bus.subscribe();
let coaching_cancel = cancel_token.clone();
tokio::spawn(async move {
    let mut rx = coaching_rx;
    while let Ok(event) = rx.recv().await {
        if coaching_cancel.is_cancelled() { break; }
        // PatternDetector processes the event
        // (exact API depends on Step 2 findings)
    }
});
```

If `PatternDetector` or `CoachingService` requires too many dependencies that are not already available, fall back to a simpler approach: track coaching-related domain events (task completion patterns, productivity events) via a counter in the harness.

- [ ] **Step 4: Build**

Run: `cargo build -p simulator 2>&1 | head -20`
Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/Cargo.toml crates/simulator/src/harness.rs
git commit -m "feat(simulator): wire coaching event tracking via PatternDetector"
```

---

### Task 9: Improve Insight Generation in Cron Pipeline

**Files:**
- Modify: `crates/simulator/src/harness.rs` (~line 1674-1717, CrossDomainInsight cron)

The `CrossDomainInsight` cron already generates heuristic insights and writes to `cross_domain_insights` (which feeds the `InsightUsefulness` metric). The current implementation groups facts by domain and writes a template string. We improve it to generate richer cross-domain insights that better exercise the metric.

- [ ] **Step 1: Enhance the CrossDomainInsight cron to produce more granular insights**

In `crates/simulator/src/harness.rs`, find the `CronTrigger::CrossDomainInsight` handler (~line 1674). Replace the insight generation logic with a version that creates per-domain-pair insights:

```rust
            CronTrigger::CrossDomainInsight => {
                debug!(trigger = "CrossDomainInsight", %simulated_now, "Executing cron");
                match self.fact_repo.list_all_active().await {
                    Ok(facts) if !facts.is_empty() => {
                        // Group facts by domain.
                        let mut domains: HashMap<String, Vec<String>> = HashMap::new();
                        for f in &facts {
                            domains
                                .entry(f.domain.clone())
                                .or_default()
                                .push(format!("{} {} {}", f.subject, f.predicate, f.object));
                        }
                        // Generate a cross-domain insight for each domain pair.
                        let domain_list: Vec<String> = domains.keys().cloned().collect();
                        if domain_list.len() >= 2 {
                            for i in 0..domain_list.len() {
                                for j in (i + 1)..domain_list.len() {
                                    let d1 = &domain_list[i];
                                    let d2 = &domain_list[j];
                                    let c1 = domains.get(d1).map(|v| v.len()).unwrap_or(0);
                                    let c2 = domains.get(d2).map(|v| v.len()).unwrap_or(0);
                                    let insight_text = format!(
                                        "Cross-domain connection between {} ({} facts) and {} ({} facts)",
                                        d1, c1, d2, c2
                                    );
                                    let dot_refs = serde_json::to_string(
                                        &vec![d1, d2],
                                    )
                                    .unwrap_or_default();
                                    let date = simulated_now.format("%Y-%m-%d").to_string();
                                    let _ = sqlx::query(
                                        "INSERT INTO cross_domain_insights \
                                         (date, insight_text, dot_refs) VALUES (?1, ?2, ?3)",
                                    )
                                    .bind(&date)
                                    .bind(&insight_text)
                                    .bind(&dot_refs)
                                    .execute(&self.inner_pool)
                                    .await;
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        debug!("CrossDomainInsight: no active facts yet");
                    }
                    Err(e) => {
                        debug!(error = %e, "CrossDomainInsight: failed to query facts");
                    }
                }
            }
```

This generates one insight per domain pair instead of one aggregate insight, giving the `InsightUsefulness` metric more data to work with and better exercising the cross-domain detection pipeline.

- [ ] **Step 2: Build and run smoke test**

Run: `cargo build -p simulator && cargo nextest run --test simulation smoke_test_7_day 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/harness.rs
git commit -m "feat(simulator): generate per-domain-pair cross-domain insights"
```

---

### Task 10: Add Real-Cognitive-LLM Scenario

**Files:**
- Create: `tests/simulation/scenarios/cognitive_llm_validation.toml`
- Modify: `tests/simulation/smoke.rs` (add test function)

This scenario validates the LLM extraction/consolidation pipeline end-to-end. It uses `cognitive_llm_model` set to a real model. The test skips if the API key is not set (CI-safe).

- [ ] **Step 1: Create the scenario TOML**

Create `tests/simulation/scenarios/cognitive_llm_validation.toml`:

```toml
[persona]
name = "cognitive_llm_validation"
timezone = "UTC"
language = "en"
seed = 99

[persona.messages_per_day]
onboarding = 4
routine = 3
power_user = 3
shift = 3

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "data scientist" },
    { subject = "user", predicate = "uses_tool", object = "Python" },
]

[persona.phases.onboarding]
duration_days = 5
correction_rate = 0.15
topic_weights = { tasks = 0.3, notes = 0.3, chat = 0.2, finance = 0.2 }
new_fact_introduction_rate = 0.5
tool_action_rate = 0.4

[persona.phases.routine]
duration_days = 5
correction_rate = 0.08
topic_weights = { tasks = 0.3, notes = 0.2, finance = 0.2, productivity = 0.2, chat = 0.1 }
new_fact_introduction_rate = 0.15
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 3
correction_rate = 0.05
topic_weights = { tasks = 0.2, notes = 0.2, finance = 0.2, productivity = 0.2, chat = 0.2 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.6

[persona.phases.behavior_shift]
duration_days = 2
correction_rate = 0.10
shift_description = "User starts learning ML"
new_facts = [
    { subject = "user", predicate = "learning", object = "machine learning" },
]
topic_weights = { tasks = 0.3, notes = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.3
tool_action_rate = 0.4

[simulation]
cognitive_llm_model = "deepseek-chat"
cognitive_temperature = 0.3
max_cognitive_calls_per_day = 8
regression_threshold = 75.0

[[checkpoints]]
at_day = 10
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.1 },
]

[[checkpoints]]
at_day = 15
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.2 },
]
```

- [ ] **Step 2: Add test function to smoke.rs**

Append to `tests/simulation/smoke.rs`:

```rust
#[tokio::test]
async fn run_cognitive_llm_validation() {
    // Skip if no API key available (CI-safe)
    if std::env::var("DEEPSEEK_API_KEY").map_or(true, |k| k.is_empty()) {
        eprintln!("Skipping cognitive LLM validation — DEEPSEEK_API_KEY not set");
        return;
    }

    let report =
        run_scenario(include_str!("scenarios/cognitive_llm_validation.toml")).await;

    eprintln!("\n============================================================");
    eprintln!("  COGNITIVE LLM VALIDATION: {}", report.persona);
    eprintln!("============================================================");
    eprintln!("  Simulated days:     {}", report.simulated_days);
    eprintln!("  Wall time:          {:.2}s", report.wall_time_secs);
    eprintln!("  Total messages:     {}", report.summary.total_messages);
    eprintln!(
        "  Facts extracted:    {}",
        report.summary.total_facts_extracted
    );
    eprintln!(
        "  Knowledge retention: {:.3}",
        report.summary.final_metrics.knowledge_retention
    );
    eprintln!(
        "  Fact extraction acc: {:.3}",
        report.summary.final_metrics.fact_extraction_accuracy
    );
    print_checkpoints(&report);
    eprintln!(
        "  Verdict: {}",
        if report.passed() { "PASSED" } else { "FAILED" }
    );

    assert!(report.summary.total_messages > 0);
    // Don't assert passed() — real LLM results vary
}
```

- [ ] **Step 3: Build**

Run: `cargo build --test simulation 2>&1 | head -20`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/scenarios/cognitive_llm_validation.toml tests/simulation/smoke.rs
git commit -m "feat(simulator): add real-cognitive-LLM validation scenario"
```

---

### Task 11: Build, Run Simulator, and Report Results

- [ ] **Step 1: Build the full workspace**

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: Clean build with no errors.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`
Expected: 0 warnings (fix any issues found).

- [ ] **Step 3: Run the smoke test**

Run: `cargo nextest run --test simulation smoke_test_7_day -- --nocapture 2>&1`
Expected: PASS.

- [ ] **Step 4: Run the 12-month simulation**

Run: `cargo nextest run --test simulation run_software_engineer_12mo -- --nocapture 2>&1`
Expected: Report printed to stderr with metrics, checkpoint results, and verdict.

- [ ] **Step 5: Run all simulation tests**

Run: `cargo nextest run --test simulation -- --nocapture 2>&1`
Expected: All heuristic-mode tests PASS. Real-LLM tests skip if API keys are not set.

- [ ] **Step 6: Report results to the user**

Print a summary of:
- Which tests passed/failed
- Key metric values from the 12-month simulation
- Any regressions detected
- Checkpoint pass rates
- Agent summary (if agent_mode tests ran)
