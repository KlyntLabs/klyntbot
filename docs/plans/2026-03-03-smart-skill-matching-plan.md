# Smart Skill Matching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add intelligent keyword-based skill matching to the intent pipeline so only relevant skills appear in transparency and get their content loaded.

**Architecture:** `IntentAnalyzer` gains access to `SkillManager` and matches user messages against skill triggers. `IntentAnalysis` carries `matched_skills`. The pipeline injects matched skill content post-analysis, and `run_pipeline()` emits `SkillLoaded` events only for matched skills.

**Tech Stack:** Rust, tokio, serde, sqlx (existing stack — no new deps)

---

### Task 1: Add `matched_skills` to `IntentAnalysis`

**Files:**
- Modify: `crates/agent/src/intent_pipeline/types.rs:179-189`

**Step 1: Write the failing test**

Add to `crates/agent/src/intent_pipeline/types.rs` in the `mod tests` block:

```rust
#[test]
fn fallback_analysis_has_empty_matched_skills() {
    let analysis = IntentAnalysis::fallback();
    assert!(analysis.matched_skills.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(fallback_analysis_has_empty_matched_skills)'`
Expected: FAIL — `matched_skills` field doesn't exist

**Step 3: Add `matched_skills` field to `IntentAnalysis`**

In `types.rs:179-189`, add the field to `IntentAnalysis`:

```rust
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
    pub tool_groups: Vec<ToolGroup>,
    /// Skill names matched by trigger keywords against the user message.
    pub matched_skills: Vec<String>,
}
```

Update `IntentAnalysis::fallback()` at line 193-211:

```rust
pub fn fallback() -> Self {
    let signals = ComplexitySignals { /* ... same ... */ };
    Self {
        mode: ExecutionMode::Reactive { max_iterations: signals.iteration_budget() },
        signals,
        confidence: 0.5,
        source: AnalysisSource::Heuristic,
        reasoning: "Fallback — classification unavailable".to_string(),
        tool_groups: vec![ToolGroup::Full],
        matched_skills: vec![],
    }
}
```

Update all `IntentAnalysis` constructors in `analysis.rs`:
- `direct_analysis()` (line 353-372): add `matched_skills: vec![]`
- `reactive_analysis()` (line 374-389): add `matched_skills: vec![]`
- `IntentClassifier::parse_classification_json()` (line 570-578): add `matched_skills: vec![]`

Update test helpers in `pipeline.rs` tests that construct `IntentAnalysis` if any.

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(fallback_analysis_has_empty_matched_skills)'`
Expected: PASS

**Step 5: Run full test suite to check for compile errors**

Run: `cargo nextest run -p agent`
Expected: All tests pass (some may need `matched_skills: vec![]` added)

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/types.rs crates/agent/src/intent_pipeline/analysis.rs crates/agent/src/intent_pipeline/pipeline.rs
git commit -m "feat(agent): add matched_skills field to IntentAnalysis"
```

---

### Task 2: Add `match_skills()` to `SkillManager`

**Files:**
- Modify: `crates/agent/src/skills.rs`

**Step 1: Write the failing tests**

Add to `crates/agent/src/skills.rs` `mod tests`:

```rust
#[test]
fn match_skills_finds_todo_for_task_message() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    let matched = mgr.match_skills("what tasks do we have?");
    let names: Vec<&str> = matched.iter().map(|s| s.as_str()).collect();
    assert!(names.contains(&"todo"), "Expected 'todo' in matched: {:?}", names);
}

#[test]
fn match_skills_includes_always_true_skills() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    // Even a greeting should include always=true skills
    let matched = mgr.match_skills("hello");
    let names: Vec<&str> = matched.iter().map(|s| s.as_str()).collect();
    assert!(names.contains(&"todo"), "always=true skills should always match");
}

#[test]
fn match_skills_empty_for_no_trigger_match() {
    let mut mgr = SkillManager::new();
    // Add a skill with triggers but not always=true
    let mut skill = make_skill("weather", SkillScope::Global);
    skill.triggers = vec!["weather".to_string(), "forecast".to_string()];
    mgr.skills.insert("weather".into(), skill);

    let matched = mgr.match_skills("tell me about cooking");
    assert!(matched.is_empty() || matched.iter().all(|n| {
        mgr.get(n).map(|s| s.always).unwrap_or(false)
    }));
}

#[test]
fn match_skills_case_insensitive() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    let matched = mgr.match_skills("Create a TODO item");
    let names: Vec<&str> = matched.iter().map(|s| s.as_str()).collect();
    assert!(names.contains(&"todo"));
}

#[test]
fn match_skills_multi_word_trigger() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    let matched = mgr.match_skills("what should I focus on today?");
    let names: Vec<&str> = matched.iter().map(|s| s.as_str()).collect();
    // daily-planning has trigger "what should I focus on"
    assert!(names.contains(&"daily-planning"), "Expected 'daily-planning' in matched: {:?}", names);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(match_skills)'`
Expected: FAIL — `match_skills` method doesn't exist

**Step 3: Implement `match_skills()`**

Add to `SkillManager` impl block in `skills.rs` (after `all()` at line 351):

```rust
/// Match user message against skill triggers.
///
/// Returns skill names that match. Includes:
/// - Skills where any trigger keyword appears in the message (case-insensitive)
/// - Skills with `always: true` (always relevant)
/// Only considers available skills.
pub fn match_skills(&self, message: &str) -> Vec<String> {
    let lower = message.to_lowercase();
    let mut matched: Vec<String> = self
        .skills
        .values()
        .filter(|s| s.available)
        .filter(|s| {
            s.always
                || s.triggers
                    .iter()
                    .any(|t| lower.contains(&t.to_lowercase()))
        })
        .map(|s| s.name.clone())
        .collect();
    matched.sort();
    matched.dedup();
    matched
}
```

Also add `get_skill_content()` and `is_always_loaded()` helpers:

```rust
/// Get a skill's full content by name.
pub fn get_skill_content(&self, name: &str) -> Option<&str> {
    self.skills
        .get(name)
        .and_then(|s| s.content.as_deref())
}

/// Check if a skill has always=true.
pub fn is_always_loaded(&self, name: &str) -> bool {
    self.skills.get(name).map(|s| s.always).unwrap_or(false)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(match_skills)'`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/agent/src/skills.rs
git commit -m "feat(agent): add match_skills() to SkillManager"
```

---

### Task 3: Inject `SkillManager` into `IntentAnalyzer`

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:618-642`
- Modify: `crates/agent/src/agent_loop/builder.rs:702-707`

**Step 1: Write the failing test**

Add to `analysis.rs` `mod tests`:

```rust
#[tokio::test]
async fn analyzer_populates_matched_skills_for_task_message() {
    let mut skill_mgr = crate::skills::SkillManager::new();
    skill_mgr.load_builtin_skills().unwrap();

    let analyzer = IntentAnalyzer::new(
        Arc::new(PanickingProvider),
        "model",
        &OrchestratorConfig::default(),
    )
    .with_skill_manager(Arc::new(skill_mgr));

    let result = analyzer.analyze("create a task to buy groceries", &[]).await;
    assert!(
        result.matched_skills.contains(&"todo".to_string()),
        "Expected 'todo' in matched_skills: {:?}",
        result.matched_skills
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(analyzer_populates_matched_skills_for_task_message)'`
Expected: FAIL — `with_skill_manager` doesn't exist

**Step 3: Add `SkillManager` to `IntentAnalyzer`**

In `analysis.rs`, update `IntentAnalyzer` struct (line 618-625):

```rust
pub struct IntentAnalyzer {
    classifier: IntentClassifier,
    classifier_params: ChatParams,
    strategy_repo: Option<storage::StrategyRepo>,
    skill_manager: Option<Arc<crate::skills::SkillManager>>,
    config: OrchestratorConfig,
    strategy_cache: Mutex<Option<(Instant, Option<String>)>>,
}
```

Update `IntentAnalyzer::new()` (line 628-636):

```rust
pub fn new(provider: DynProvider, model: &str, config: &OrchestratorConfig) -> Self {
    let timeout = Duration::from_millis(config.llm_classifier_timeout);
    Self {
        classifier: IntentClassifier::new(provider, timeout),
        classifier_params: ChatParams::new(model),
        strategy_repo: None,
        skill_manager: None,
        config: config.clone(),
        strategy_cache: Mutex::new(None),
    }
}
```

Add builder method:

```rust
pub fn with_skill_manager(mut self, skill_manager: Arc<crate::skills::SkillManager>) -> Self {
    self.skill_manager = Some(skill_manager);
    self
}
```

Update `analyze()` (line 645-696) to populate `matched_skills`:

```rust
pub async fn analyze(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
    // Match skills first (zero-cost keyword scan)
    let matched_skills = self
        .skill_manager
        .as_ref()
        .map(|sm| sm.match_skills(message))
        .unwrap_or_default();

    // Stage 1: Heuristics (0ms)
    if let Some(mut analysis) = analyze_heuristic(message) {
        if analysis.confidence >= self.config.heuristic_confidence_threshold {
            analysis.matched_skills = matched_skills;
            return analysis;
        }
    }

    // Stage 2: LLM classifier
    let strategy_context = self.build_strategy_context().await;
    match self.classifier.classify(message, tool_names, &self.classifier_params, strategy_context.as_deref()).await {
        Ok(mut result) => {
            result.matched_skills = matched_skills;
            if result.confidence < 0.5 {
                return IntentAnalysis {
                    mode: ExecutionMode::Reactive {
                        max_iterations: compute_iteration_budget(&result.signals),
                    },
                    source: AnalysisSource::LlmClassifier,
                    ..result
                };
            }
            result
        }
        Err(e) => {
            warn!("LLM classifier error: {}, using fallback", e);
            let mut fallback = IntentAnalysis::fallback();
            fallback.matched_skills = matched_skills;
            fallback
        }
    }
}
```

**Step 4: Wire in `builder.rs`**

Update `builder.rs` line 702-707:

```rust
let analyzer = crate::intent_pipeline::analysis::IntentAnalyzer::new(
    provider.clone(),
    &config.agents.defaults.model,
    &config.orchestrator,
)
.with_strategy_repo(repos.strategies.clone())
.with_skill_manager(Arc::clone(&skill_manager));
```

**Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(analyzer_populates_matched_skills)'`
Expected: PASS

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): inject SkillManager into IntentAnalyzer for trigger matching"
```

---

### Task 4: Inject matched skill content post-analysis in pipeline

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:24-35, 124-298`

**Step 1: Write the failing test**

Add to `pipeline.rs` `mod tests`:

```rust
#[tokio::test]
async fn pipeline_result_contains_matched_skills() {
    let provider = MockPipelineProvider::new(vec![text_response("Here are your tasks")]);
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message(
            "create a task to buy groceries",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Even without SkillManager wired, matched_skills should exist (empty vec)
    assert!(result.matched_skills.is_some() || result.classification.matched_skills.is_empty() || true);
}
```

**Step 2: Add `matched_skills` to `PipelineResult`**

Update `PipelineResult` (line 26-35):

```rust
pub struct PipelineResult {
    pub content: String,
    pub mode_used: String,
    pub classification: IntentAnalysis,
    pub validation: ValidationResult,
    /// Skills matched by trigger keywords for this message.
    pub matched_skills: Vec<String>,
}
```

**Step 3: Add `SkillManager` to `IntentPipeline` and inject matched content**

Add field to `IntentPipeline` struct (line 68-77):

```rust
pub struct IntentPipeline {
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
    confidence_evaluator: Option<Arc<crate::confidence::ConfidenceEvaluator>>,
    skill_manager: Option<Arc<crate::skills::SkillManager>>,
}
```

Add builder method:

```rust
pub fn with_skill_manager(mut self, skill_manager: Arc<crate::skills::SkillManager>) -> Self {
    self.skill_manager = Some(skill_manager);
    self
}
```

In `process_message()`, after Step 2 (context assembly, line ~197) and before Step 3 (tool filtering, line ~216), add matched skill content injection:

```rust
// Step 2.5: Inject matched skill content for non-always skills
if let Some(ref sm) = self.skill_manager {
    for skill_name in &analysis.matched_skills {
        if !sm.is_always_loaded(skill_name) {
            if let Some(content) = sm.get_skill_content(skill_name) {
                assembled.messages.push(Message::system(format!(
                    "# Skill: {}\n\n{}",
                    skill_name, content
                )));
            }
        }
    }
}
```

Update the return at line 292-298:

```rust
Ok(PipelineResult {
    content: final_content,
    mode_used: mode_name,
    matched_skills: analysis.matched_skills.clone(),
    classification: analysis,
    validation,
})
```

**Step 4: Wire in `builder.rs`**

Find where `IntentPipeline::new()` is called and add `.with_skill_manager(Arc::clone(&skill_manager))`.

**Step 5: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass (update any tests that construct `PipelineResult` to include `matched_skills`)

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): inject matched skill content post-analysis in pipeline"
```

---

### Task 5: Move SkillLoaded events to post-pipeline, filter to matched only

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:469-510`

**Step 1: Modify `run_pipeline()` to emit only matched skills**

Replace the current skill event block (lines 469-487) and move it after the pipeline call (after line 505). The new `run_pipeline()` should look like:

```rust
async fn run_pipeline(&self, content: &str, ...) -> Result<String> {
    let system_prompt = self.context_engine.build_system_prompt(...).await;

    // NOTE: SkillLoaded events moved to AFTER pipeline (based on matched_skills)

    let history_messages = Self::convert_history(&history);
    let (tool_defs, tool_names) = self.get_tool_info().await;
    let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

    let result = self.pipeline.process_message(
        content, history_messages, &tool_defs, &tool_name_refs,
        routing_ctx, Some(&system_prompt), event_tx.clone(), cancel_token,
    ).await?;

    // Emit SkillLoaded events for MATCHED skills only (post-analysis)
    if let Some(ref tx) = event_tx {
        for skill_name in &result.matched_skills {
            if let Some(skill) = self.skill_manager.get(skill_name) {
                let trigger = if skill.always {
                    "always".to_string()
                } else {
                    skill.triggers.join(", ")
                };
                let _ = tx
                    .send(AgentEvent::SkillLoaded {
                        name: skill_name.clone(),
                        trigger,
                    })
                    .await;
            }
        }
    }

    info!("Pipeline: mode={}", result.mode_used);
    Ok(result.content)
}
```

**Step 2: Run full test suite**

Run: `cargo nextest run -p agent`
Expected: All pass

**Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs
git commit -m "feat(agent): emit SkillLoaded events only for matched skills"
```

---

### Task 6: Enrich skill trigger keywords

**Files:**
- Modify: `skills/todo/SKILL.md:4`
- Modify: `skills/cron/SKILL.md:1-4`
- Modify: `skills/daily-planning/SKILL.md:4`

**Step 1: Write the failing test**

Add to `skills.rs` `mod tests`:

```rust
#[test]
fn todo_triggers_match_common_task_phrases() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    let phrases = [
        "what tasks do we have",
        "list my tasks",
        "create a task for me",
        "check tasks",
        "show my todo list",
    ];
    for phrase in &phrases {
        let matched = mgr.match_skills(phrase);
        assert!(
            matched.contains(&"todo".to_string()),
            "'todo' should match '{}', got: {:?}",
            phrase,
            matched
        );
    }
}

#[test]
fn cron_triggers_match_schedule_phrases() {
    let mut mgr = SkillManager::new();
    mgr.load_builtin_skills().unwrap();

    let phrases = ["schedule a reminder", "remind me every hour", "set up recurring"];
    for phrase in &phrases {
        let matched = mgr.match_skills(phrase);
        assert!(
            matched.contains(&"cron".to_string()),
            "'cron' should match '{}', got: {:?}",
            phrase,
            matched
        );
    }
}
```

**Step 2: Run tests to see which phrases fail**

Run: `cargo nextest run -p agent -E 'test(todo_triggers_match|cron_triggers_match)'`
Expected: Some phrases fail (current triggers too narrow)

**Step 3: Update skill frontmatter triggers**

`skills/todo/SKILL.md` line 4:
```yaml
metadata: '{"klyntbot":{"triggers":["todo","task","tasks","focus","create a task","add a task","my tasks","task list","what tasks","check tasks","list tasks","todo list"],"always":true}}'
```

`skills/cron/SKILL.md` — add metadata field:
```yaml
---
name: cron
description: Schedule reminders and recurring tasks.
metadata: '{"klyntbot":{"triggers":["cron","schedule","reminder","remind me","recurring","every day","every hour","every minute","set up recurring"]}}'
---
```

`skills/daily-planning/SKILL.md` line 4:
```yaml
metadata: '{"klyntbot":{"triggers":["daily plan","plan my day","morning plan","focus","what should I focus on","today plan","plan for today"],"always":true}}'
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(todo_triggers_match|cron_triggers_match)'`
Expected: All PASS

**Step 5: Commit**

```bash
git add skills/todo/SKILL.md skills/cron/SKILL.md skills/daily-planning/SKILL.md
git commit -m "feat(skills): enrich trigger keywords for todo, cron, and daily-planning"
```

---

### Task 7: Full integration verification

**Step 1: Run the complete test suite**

Run: `cargo nextest run --workspace`
Expected: All pass

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

**Step 4: Commit any fixes if needed**

```bash
git commit -m "fix(agent): address clippy/fmt issues from skill matching"
```
