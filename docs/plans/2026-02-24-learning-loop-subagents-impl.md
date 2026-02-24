# Learning Loop + Hierarchical Sub-Agents Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the feedback loops in klyntbot's adaptive orchestration system — strategy persistence, specialized sub-agents, goal metrics, and user satisfaction collection.

**Architecture:** Four independent features that connect: (1) pipeline writes strategy records after every message, closing the orchestrator's read-only feedback loop; (2) sub-agents get LLM-selected profiles with restricted tool sets; (3) goals replace JSON blobs with typed plan-completion columns; (4) channel reactions map to satisfaction scores backfilled to strategy records.

**Tech Stack:** Rust, SQLite (sqlx migrations), tokio async, Extism (existing), serde_json

---

## Task 1: Migration 002 — Schema Changes

**Files:**
- Create: `crates/storage/migrations/002_learning_loop.sql`

This migration adds `chat_id` to `strategy_records` and replaces JSON blob columns on `goals` with typed columns.

**Step 1: Create the migration file**

```sql
-- 002_learning_loop.sql
-- Adds chat_id to strategy_records for reaction-based satisfaction.
-- Replaces JSON blob columns on goals with typed plan-completion columns.

-- Strategy records: add chat_id for linking reactions to records
ALTER TABLE strategy_records ADD COLUMN chat_id TEXT;
CREATE INDEX idx_strategy_records_chat_id ON strategy_records(chat_id);

-- Goals: add typed plan-completion columns
ALTER TABLE goals ADD COLUMN plans_completed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN plans_failed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE goals ADD COLUMN avg_duration_ms INTEGER;
ALTER TABLE goals ADD COLUMN last_plan_at TEXT;
```

Note: We keep `metrics` and `metadata` columns for now (SQLite cannot DROP COLUMN in older versions). The Rust structs will stop reading them; they'll be dead columns.

**Step 2: Verify migration runs**

Run: `cargo nextest run -p storage -- migration`
Expected: All existing storage tests still pass (migrations auto-run on `connect_in_memory()`)

**Step 3: Commit**

```bash
git add crates/storage/migrations/002_learning_loop.sql
git commit -m "feat(storage): add migration 002 for learning loop schema changes"
```

---

## Task 2: StrategyRecordRow — Add `chat_id` Field

**Files:**
- Modify: `crates/storage/src/rows/learning.rs:23-36` (StrategyRecordRow struct)
- Modify: `crates/storage/src/repos/strategy.rs` (create query, all SELECT queries)

**Step 1: Write the test**

Add to `crates/storage/src/repos/strategy.rs` in the existing `#[cfg(test)]` module (or create one):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_create_strategy_record_with_chat_id() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.get());

        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-1".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 1,
            iterations_used: 3,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 1200,
            chat_id: Some("tg:12345".to_string()),
        };

        let created = repo.create(&row).await.unwrap();
        assert_eq!(created.chat_id, Some("tg:12345".to_string()));

        let fetched = repo.get(row.id).await.unwrap();
        assert_eq!(fetched.chat_id, Some("tg:12345".to_string()));
    }

    #[tokio::test]
    async fn test_create_strategy_record_without_chat_id() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.get());

        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-2".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 200,
            chat_id: None,
        };

        let created = repo.create(&row).await.unwrap();
        assert_eq!(created.chat_id, None);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(test_create_strategy_record_with_chat_id)'`
Expected: FAIL — `StrategyRecordRow` has no `chat_id` field

**Step 3: Add `chat_id` to StrategyRecordRow**

In `crates/storage/src/rows/learning.rs`, add to the `StrategyRecordRow` struct after `response_time_ms`:

```rust
    pub chat_id: Option<String>,
```

**Step 4: Update StrategyRepo create() query**

In `crates/storage/src/repos/strategy.rs`, the `create()` method's INSERT query needs `chat_id` added to both the column list and the VALUES bind. Update the query to include `chat_id` as the 12th column. Also update the `get()` and other SELECT queries if they use explicit column lists (if they use `SELECT *`, they'll pick it up from `FromRow`).

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p storage -E 'test(test_create_strategy_record)'`
Expected: PASS (both tests)

**Step 6: Commit**

```bash
git add crates/storage/src/rows/learning.rs crates/storage/src/repos/strategy.rs
git commit -m "feat(storage): add chat_id to StrategyRecordRow"
```

---

## Task 3: DispatchResult — Add `iterations_used`

**Files:**
- Modify: `crates/agent/src/execution/dispatch.rs:21-30` (DispatchResult struct)
- Modify: `crates/agent/src/execution/dispatch.rs:90-200` (all DispatchResult return sites)

**Step 1: Write the test**

In `crates/agent/src/execution/dispatch.rs`, find or create the `#[cfg(test)]` module and add:

```rust
#[test]
fn test_dispatch_result_has_iterations_used() {
    let result = DispatchResult {
        content: "hello".to_string(),
        final_strategy: context_engine::ExecutionStrategy::DirectResponse,
        escalation_count: 0,
        usage: providers::Usage::default(),
        iterations_used: 1,
    };
    assert_eq!(result.iterations_used, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_dispatch_result_has_iterations_used)'`
Expected: FAIL — no field `iterations_used` on `DispatchResult`

**Step 3: Add `iterations_used` to DispatchResult**

In `crates/agent/src/execution/dispatch.rs` line 21, add to the struct:

```rust
pub struct DispatchResult {
    pub content: String,
    pub final_strategy: ExecutionStrategy,
    pub escalation_count: u32,
    pub usage: Usage,
    pub iterations_used: u32,  // NEW
}
```

**Step 4: Update all DispatchResult construction sites**

There are ~8 places in `dispatch.rs` where `DispatchResult { ... }` is returned. For each:

- `DirectOutcome::Response` (line ~101): `iterations_used: 1`
- `DirectOutcome::EscalateToToolAssisted` max escalation (line ~110): `iterations_used: 0`
- `ReactOutcome::Response` (line ~146): extract `iterations` from the match arm: `ReactOutcome::Response { content, usage, iterations, .. }` → `iterations_used: iterations`
- `ReactOutcome::EscalateToAutonomous` max escalation (line ~159): `iterations_used: 0`
- `ReactOutcome::MaxIterationsReached` (line ~176): use the engine's `max_iterations` value. Capture it: at line 127, `let max_iter = *max_iterations;`, then `iterations_used: max_iter`
- `AutonomousTask` plan engine path (line ~194): `result.iterations_used` already on the PlanGenerateEngine's DispatchResult — this will need updating in PlanGenerateEngine too OR set a default. For now, if PlanGenerateEngine returns DispatchResult, it also needs `iterations_used`. Check if it constructs DispatchResult.
- `AutonomousTask` ReactPlus fallback: same pattern as ToolAssisted

**Step 5: Fix PlanGenerateEngine if it constructs DispatchResult**

Check `crates/agent/src/execution/plan_generate.rs` for `DispatchResult` construction. Add `iterations_used` there too.

**Step 6: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: PASS (all tests compile and pass with the new field)

**Step 7: Commit**

```bash
git add crates/agent/src/execution/dispatch.rs crates/agent/src/execution/plan_generate.rs
git commit -m "feat(agent): add iterations_used to DispatchResult"
```

---

## Task 4: AgentPipeline — Add `strategy_repo` and Step 6

**Files:**
- Modify: `crates/agent/src/pipeline.rs:68-75` (AgentPipeline struct)
- Modify: `crates/agent/src/pipeline.rs:78-92` (constructor)
- Modify: `crates/agent/src/pipeline.rs:216-239` (after Step 5, before return)
- Modify: `crates/agent/src/agent_loop/builder.rs:749-763` (pipeline construction)

**Step 1: Write the test**

In `crates/agent/src/pipeline.rs`, add to the existing `#[cfg(test)]` module (starts at line 242):

```rust
#[tokio::test]
async fn test_pipeline_writes_strategy_record() {
    // This test verifies that process_message writes a StrategyRecordRow
    // to the strategy_repo after processing.
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);

    let provider = MockPipelineProvider::new(vec![LlmResponse {
        content: Some("Hello!".to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }]);

    let orchestrator = Arc::new(
        crate::orchestrator::Orchestrator::new(provider.clone(), "test-model")
    );
    let tools = Arc::new(RwLock::new(ToolRegistry::new()));
    let core = Arc::new(ExecutionCore::new(provider.clone(), tools));
    let dispatch = Arc::new(crate::execution::EngineDispatch::new(core));
    let cost_tracker = Arc::new(crate::cost_tracker::CostTracker::new(repos.usage.clone()));

    let config = PipelineConfig {
        execution_model: "test-model".to_string(),
        system_prompt: "You are a test bot.".to_string(),
        context_window: 4096,
        max_response_tokens: 1024,
        channel: "test".to_string(),
        provider_name: "mock".to_string(),
    };

    let pipeline = AgentPipeline::new(
        orchestrator, dispatch, cost_tracker, config,
    ).with_strategy_repo(repos.strategies.clone());

    let ctx = tools::RoutingContext::new("test".into(), "chat-1".into());
    let result = pipeline
        .process_message("hi", vec![], &[], &[], &ctx, None, None)
        .await
        .unwrap();

    // Verify a strategy record was written
    let records = repos.strategies
        .list_by_date_range(
            chrono::Utc::now() - chrono::Duration::minutes(1),
            chrono::Utc::now() + chrono::Duration::minutes(1),
        )
        .await
        .unwrap();
    assert!(!records.is_empty(), "Expected at least one strategy record");
    assert_eq!(records[0].chat_id, Some("chat-1".to_string()));
    assert!(records[0].success);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_pipeline_writes_strategy_record)'`
Expected: FAIL — `with_strategy_repo` method doesn't exist

**Step 3: Add `strategy_repo` to AgentPipeline**

In `crates/agent/src/pipeline.rs`, modify the struct and add a builder method:

```rust
pub struct AgentPipeline {
    orchestrator: Arc<Orchestrator>,
    context_engine: ContextEngine,
    engine_dispatch: Arc<EngineDispatch>,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,  // NEW
}
```

Add to `impl AgentPipeline`:

```rust
pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
    self.strategy_repo = Some(repo);
    self
}
```

Update constructor to initialize `strategy_repo: None`.

**Step 4: Add Step 6 — Write strategy record**

In `process_message()`, after Step 5 (line ~230, after the cost tracker block) and before the final `Ok(PipelineResult { ... })`:

```rust
// Step 6: Record strategy outcome (best-effort)
if let Some(ref strategy_repo) = self.strategy_repo {
    let record = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: uuid::Uuid::new_v4().to_string(),
        predicted_strategy: format!("{:?}", classification.strategy),
        actual_strategy: strategy_name.clone(),
        escalation_count: dispatch_result.escalation_count as i32,
        iterations_used: dispatch_result.iterations_used as i32,
        max_iterations: match &classification.strategy {
            context_engine::ExecutionStrategy::ToolAssisted { max_iterations } => {
                *max_iterations as i32
            }
            context_engine::ExecutionStrategy::AutonomousTask { max_iterations } => {
                *max_iterations as i32
            }
            _ => 1,
        },
        success: validation.is_valid,
        user_satisfaction: None,
        response_time_ms: classify_start.elapsed().as_millis() as i64,
        chat_id: Some(ctx.chat_id.to_string()),
    };
    if let Err(e) = strategy_repo.create(&record).await {
        warn!("Pipeline: failed to record strategy: {}", e);
    }
}
```

Note: `classify_start` is already defined at line 113 — it measures from the start of `process_message()`, giving total response time.

**Step 5: Update builder.rs to pass strategy_repo**

In `crates/agent/src/agent_loop/builder.rs` around line 759, change:

```rust
let pipeline = Arc::new(crate::pipeline::AgentPipeline::new(
    orchestrator, dispatch, cost_tracker, pipeline_config,
).with_strategy_repo(repos.strategies.clone()));
```

**Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_pipeline)'`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/agent/src/pipeline.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): write strategy records in pipeline Step 6"
```

---

## Task 5: SubagentProfile Enum + Read-Only FS Helper

**Files:**
- Modify: `crates/agent/src/subagent.rs` (add `SubagentProfile` enum)
- Modify: `crates/tools/src/filesystem.rs` (add `register_fs_read_tools()`)

**Step 1: Write the test for SubagentProfile**

In `crates/agent/src/subagent.rs`, add to the `#[cfg(test)]` module:

```rust
#[test]
fn test_subagent_profile_default_is_general() {
    let profile = SubagentProfile::default();
    assert!(matches!(profile, SubagentProfile::General));
}

#[test]
fn test_subagent_profile_from_str() {
    assert!(matches!(
        SubagentProfile::from_str("research"),
        Ok(SubagentProfile::Research)
    ));
    assert!(matches!(
        SubagentProfile::from_str("code"),
        Ok(SubagentProfile::Code)
    ));
    assert!(matches!(
        SubagentProfile::from_str("analyst"),
        Ok(SubagentProfile::Analyst)
    ));
    assert!(matches!(
        SubagentProfile::from_str("general"),
        Ok(SubagentProfile::General)
    ));
    assert!(matches!(
        SubagentProfile::from_str("unknown"),
        Ok(SubagentProfile::General) // unknown defaults to General
    ));
}

#[test]
fn test_subagent_profile_max_iterations() {
    assert_eq!(SubagentProfile::General.max_iterations(), 15);
    assert_eq!(SubagentProfile::Research.max_iterations(), 10);
    assert_eq!(SubagentProfile::Code.max_iterations(), 15);
    assert_eq!(SubagentProfile::Analyst.max_iterations(), 5);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_subagent_profile)'`
Expected: FAIL — `SubagentProfile` not defined

**Step 3: Implement SubagentProfile**

At the top of `crates/agent/src/subagent.rs` (after imports), add:

```rust
use std::str::FromStr;

/// Specialized profiles for sub-agents with different tool sets and behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentProfile {
    /// Full access: filesystem + shell + web (default)
    General,
    /// Web + filesystem read-only, no shell
    Research,
    /// Filesystem + shell, no web
    Code,
    /// Filesystem read-only only (pure reasoning)
    Analyst,
}

impl Default for SubagentProfile {
    fn default() -> Self {
        Self::General
    }
}

impl FromStr for SubagentProfile {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "research" => Self::Research,
            "code" => Self::Code,
            "analyst" => Self::Analyst,
            _ => Self::General,
        })
    }
}

impl SubagentProfile {
    /// Maximum iteration count for this profile.
    pub fn max_iterations(&self) -> u32 {
        match self {
            Self::General => 15,
            Self::Research => 10,
            Self::Code => 15,
            Self::Analyst => 5,
        }
    }

    /// System prompt role preamble for this profile.
    pub fn role_prompt(&self) -> &'static str {
        match self {
            Self::General => "You are a general-purpose subagent. You have full access to filesystem, shell, and web tools.",
            Self::Research => "You are a research specialist. Focus on finding and synthesizing information from web sources and files. You have read-only file access and web tools, but no shell.",
            Self::Code => "You are a code specialist. Focus on reading, writing, and executing code to complete the task. You have filesystem and shell access, but no web tools.",
            Self::Analyst => "You are an analyst. Reason about the information provided. You have read-only file access but no shell or web tools.",
        }
    }
}
```

**Step 4: Add `register_fs_read_tools()` to filesystem.rs**

Write the test first in `crates/tools/src/filesystem.rs`:

```rust
#[test]
fn test_register_fs_read_tools_only_registers_read_and_list() {
    let mut registry = crate::registry::ToolRegistry::new();
    register_fs_read_tools(&mut registry, None);
    let defs = registry.get_definitions();
    let names: Vec<&str> = defs.iter().filter_map(|d| d["function"]["name"].as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_dir"));
    assert!(!names.contains(&"write_file"));
    assert!(!names.contains(&"edit_file"));
}
```

Then implement:

```rust
/// Register read-only filesystem tools (ReadFile + ListDir only).
/// Used by Research and Analyst sub-agent profiles.
pub fn register_fs_read_tools(
    registry: &mut crate::registry::ToolRegistry,
    allowed_dir: Option<PathBuf>,
) {
    registry.register(ReadFileTool::new(allowed_dir.clone()));
    registry.register(ListDirTool::new(allowed_dir));
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_subagent_profile)' && cargo nextest run -p tools -E 'test(test_register_fs_read_tools)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/agent/src/subagent.rs crates/tools/src/filesystem.rs
git commit -m "feat(agent): add SubagentProfile enum and read-only fs tool registration"
```

---

## Task 6: SpawnHandler Trait + SpawnTool — Add `profile` Parameter

**Files:**
- Modify: `crates/tools/src/spawn.rs:14-24` (SpawnHandler trait)
- Modify: `crates/tools/src/spawn.rs:65-80` (SpawnTool parameters)
- Modify: `crates/tools/src/spawn.rs` (SpawnTool execute)
- Modify: `crates/agent/src/subagent.rs:219-230` (SpawnHandler impl)

**Step 1: Update SpawnHandler trait**

In `crates/tools/src/spawn.rs`, change the trait to accept a profile string:

```rust
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: String,           // NEW: "general", "research", "code", "analyst"
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;
}
```

**Step 2: Update SpawnTool parameters schema**

In the `parameters()` method, add `profile` to the properties:

```json
"profile": {
    "type": "string",
    "description": "Sub-agent specialization profile. Options: general (default, full access), research (web + read-only files), code (files + shell, no web), analyst (read-only files, pure reasoning)",
    "enum": ["general", "research", "code", "analyst"]
}
```

**Step 3: Update SpawnTool execute() to extract and pass profile**

In the `execute()` method, extract profile from args:

```rust
let profile = args.get("profile")
    .and_then(|v| v.as_str())
    .unwrap_or("general")
    .to_string();
```

Pass it to the handler call.

**Step 4: Update SubagentManager SpawnHandler impl**

In `crates/agent/src/subagent.rs`, update the `impl SpawnHandler for SubagentManager` (line 219):

```rust
#[async_trait]
impl SpawnHandler for SubagentManager {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: String,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String {
        let profile = SubagentProfile::from_str(&profile).unwrap_or_default();
        self.spawn(task, label, profile, origin_channel, origin_chat_id)
            .await
    }
}
```

**Step 5: Update SubagentManager::spawn() inherent method signature**

Change the inherent `spawn()` method (line 137) to accept `SubagentProfile`:

```rust
pub async fn spawn(
    &self,
    task: String,
    label: Option<String>,
    profile: SubagentProfile,   // CHANGED from no profile
    origin_channel: String,
    origin_chat_id: String,
) -> String
```

Pass `profile` into the spawned task (it will be used in Task 7).

**Step 6: Run tests**

Run: `cargo nextest run -p tools -E 'test(spawn)' && cargo nextest run -p agent -E 'test(subagent)'`
Expected: PASS (existing tests may need profile arg added — use `SubagentProfile::General` or `"general".to_string()`)

**Step 7: Commit**

```bash
git add crates/tools/src/spawn.rs crates/agent/src/subagent.rs
git commit -m "feat(tools): add profile parameter to SpawnHandler and SpawnTool"
```

---

## Task 7: SubagentManager — Profile-Based Tool Registration and Prompts

**Files:**
- Modify: `crates/agent/src/subagent.rs:236-317` (run_subagent_task + build_subagent_prompt)

**Step 1: Write the test**

In `crates/agent/src/subagent.rs` tests module:

```rust
#[test]
fn test_build_subagent_prompt_includes_profile_role() {
    let prompt = build_subagent_prompt(
        std::path::Path::new("/tmp"),
        "Research Rust patterns",
        SubagentProfile::Research,
    );
    assert!(prompt.contains("research specialist"));
    assert!(prompt.contains("Research Rust patterns"));
}

#[test]
fn test_build_subagent_prompt_general_profile() {
    let prompt = build_subagent_prompt(
        std::path::Path::new("/tmp"),
        "Do something",
        SubagentProfile::General,
    );
    assert!(prompt.contains("general-purpose subagent"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_build_subagent_prompt)'`
Expected: FAIL — `build_subagent_prompt` takes 2 args, not 3

**Step 3: Update `run_subagent_task()` to accept and use profile**

Change signature:

```rust
async fn run_subagent_task(
    provider: &DynProvider,
    workspace: &std::path::Path,
    model: &str,
    task: &str,
    config: SubagentConfig,
    profile: SubagentProfile,   // NEW
) -> std::result::Result<(String, String), Box<dyn std::error::Error + Send + Sync>>
```

Replace the tool registration block (lines 249-272) with profile-based registration:

```rust
// Build subagent tool registry based on profile
let mut tools = ToolRegistry::new();

let allowed_dir = if config.restrict_to_workspace {
    Some(workspace.to_path_buf())
} else {
    None
};

match profile {
    SubagentProfile::General => {
        register_fs_tools(&mut tools, allowed_dir);
        tools.register(ExecTool::new(
            config.exec_timeout,
            Some(workspace.to_path_buf()),
            config.restrict_to_workspace,
        ));
        tools.register(WebSearchTool::new(
            config.brave_api_key,
            config.web_max_results,
        ));
        tools.register(WebFetchTool::new());
    }
    SubagentProfile::Research => {
        tools::filesystem::register_fs_read_tools(&mut tools, allowed_dir);
        tools.register(WebSearchTool::new(
            config.brave_api_key,
            config.web_max_results,
        ));
        tools.register(WebFetchTool::new());
    }
    SubagentProfile::Code => {
        register_fs_tools(&mut tools, allowed_dir);
        tools.register(ExecTool::new(
            config.exec_timeout,
            Some(workspace.to_path_buf()),
            config.restrict_to_workspace,
        ));
    }
    SubagentProfile::Analyst => {
        tools::filesystem::register_fs_read_tools(&mut tools, allowed_dir);
    }
}
```

Update the engine construction to use profile iteration limits:

```rust
let engine = ReactPlusEngine::new(core)
    .with_max_iterations(profile.max_iterations())
    .with_reflection_mode(ReflectionMode::OnFailure);
```

**Step 4: Update `build_subagent_prompt()` to include profile**

```rust
fn build_subagent_prompt(workspace: &std::path::Path, task: &str, profile: SubagentProfile) -> String {
    let tool_description = match profile {
        SubagentProfile::General => "- Read and write files in the workspace\n- Execute shell commands\n- Search the web and fetch web pages",
        SubagentProfile::Research => "- Read files in the workspace (read-only)\n- Search the web and fetch web pages",
        SubagentProfile::Code => "- Read and write files in the workspace\n- Execute shell commands",
        SubagentProfile::Analyst => "- Read files in the workspace (read-only)",
    };

    format!(
        r#"# Subagent

{}

## Your Task
{}

## Rules
1. Stay focused - complete only the assigned task, nothing else
2. Your final response will be reported back to the main agent
3. Do not initiate conversations or take on side tasks
4. Be concise but informative in your findings

## What You Can Do
{}

## What You Cannot Do
- Send messages directly to users (no message tool available)
- Spawn other subagents
- Access the main agent's conversation history

## Workspace
Your workspace is at: {}

When you have completed the task, provide a clear summary of your findings or actions."#,
        profile.role_prompt(),
        task,
        tool_description,
        workspace.display()
    )
}
```

**Step 5: Update spawn() to pass profile through to run_subagent_task**

In `SubagentManager::spawn()`, pass the `profile` parameter into the spawned tokio task and then to `run_subagent_task()`.

**Step 6: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/agent/src/subagent.rs
git commit -m "feat(agent): profile-based tool registration and prompts for sub-agents"
```

---

## Task 8: GoalRow — Replace JSON Blobs with Typed Columns

**Files:**
- Modify: `crates/storage/src/rows/goal.rs:7-19` (GoalRow struct)
- Modify: `crates/storage/src/repos/goal.rs` (all queries)

**Step 1: Write the test**

In `crates/storage/src/repos/goal.rs`, add/update tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_goal_typed_columns_defaults() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = GoalRepo::new(pool.get());

        let row = GoalRow {
            id: uuid::Uuid::new_v4(),
            title: "Ship v1".to_string(),
            description: "Release version 1".to_string(),
            status: "Active".to_string(),
            priority: 1,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        };

        repo.create(&row).await.unwrap();
        let fetched = repo.get(row.id).await.unwrap();
        assert_eq!(fetched.plans_completed, 0);
        assert_eq!(fetched.plans_failed, 0);
        assert_eq!(fetched.avg_duration_ms, None);
        assert_eq!(fetched.last_plan_at, None);
    }

    #[tokio::test]
    async fn test_increment_completed_computes_rolling_average() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = GoalRepo::new(pool.get());

        let id = uuid::Uuid::new_v4();
        let row = GoalRow {
            id,
            title: "Test goal".to_string(),
            description: "".to_string(),
            status: "Active".to_string(),
            priority: 3,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        };
        repo.create(&row).await.unwrap();

        // First completion: 1000ms
        repo.increment_completed(id, 1000).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_completed, 1);
        assert_eq!(g.avg_duration_ms, Some(1000));

        // Second completion: 3000ms → avg = (1000 + 3000) / 2 = 2000
        repo.increment_completed(id, 3000).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_completed, 2);
        assert_eq!(g.avg_duration_ms, Some(2000));
    }

    #[tokio::test]
    async fn test_increment_failed() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = GoalRepo::new(pool.get());

        let id = uuid::Uuid::new_v4();
        let row = GoalRow {
            id,
            title: "Test goal".to_string(),
            description: "".to_string(),
            status: "Active".to_string(),
            priority: 3,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        };
        repo.create(&row).await.unwrap();

        repo.increment_failed(id).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_failed, 1);

        repo.increment_failed(id).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_failed, 2);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(test_goal_typed)'`
Expected: FAIL — GoalRow has no `plans_completed` field

**Step 3: Update GoalRow struct**

In `crates/storage/src/rows/goal.rs`:

```rust
#[derive(Debug, Clone, FromRow)]
pub struct GoalRow {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i16,
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub plans_completed: i32,      // REPLACES metrics
    pub plans_failed: i32,         // REPLACES metadata
    pub avg_duration_ms: Option<i64>,
    pub last_plan_at: Option<DateTime<Utc>>,
}
```

Note: Keep `metrics` and `metadata` columns in SQLite (can't drop), but remove from Rust struct. `FromRow` will ignore extra columns. Actually — `FromRow` with `SELECT *` will fail if there are columns not in the struct. So we need to use explicit column lists in all queries, OR keep the fields as ignored. The safest approach: **use explicit SELECT column lists** in all GoalRepo queries.

**Step 4: Update GoalRepo queries**

Update all queries in `crates/storage/src/repos/goal.rs` to use explicit column lists that match the new struct:

```rust
const GOAL_COLUMNS: &str = "id, title, description, status, priority, target_date, created_at, updated_at, plans_completed, plans_failed, avg_duration_ms, last_plan_at";
```

Update `create()`: INSERT must include the new columns.
Update `get()`: `SELECT {GOAL_COLUMNS} FROM goals WHERE id = ?`
Update `list()`: `SELECT {GOAL_COLUMNS} FROM goals ...`
Update `update()`: SET all new columns.
Remove `update_metrics()` (replaced by `increment_completed`/`increment_failed`).

**Step 5: Add `increment_completed()` and `increment_failed()`**

```rust
/// Increment plans_completed and update avg_duration_ms with rolling average.
pub async fn increment_completed(
    &self,
    id: Uuid,
    plan_duration_ms: i64,
) -> Result<(), StorageError> {
    // Compute new average: ((old_avg * count) + new) / (count + 1)
    let result = sqlx::query(
        "UPDATE goals SET \
         plans_completed = plans_completed + 1, \
         avg_duration_ms = CASE \
           WHEN avg_duration_ms IS NULL THEN ?1 \
           ELSE ((avg_duration_ms * plans_completed) + ?1) / (plans_completed + 1) \
         END, \
         last_plan_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), \
         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?2"
    )
    .bind(plan_duration_ms)
    .bind(id.to_string())
    .execute(&self.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::NotFound(format!("Goal {}", id)));
    }
    Ok(())
}

/// Increment plans_failed counter.
pub async fn increment_failed(&self, id: Uuid) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE goals SET \
         plans_failed = plans_failed + 1, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?1"
    )
    .bind(id.to_string())
    .execute(&self.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::NotFound(format!("Goal {}", id)));
    }
    Ok(())
}
```

**Step 6: Run tests**

Run: `cargo nextest run -p storage -E 'test(test_goal)'`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/storage/src/rows/goal.rs crates/storage/src/repos/goal.rs
git commit -m "feat(storage): replace goal JSON blobs with typed plan-completion columns"
```

---

## Task 9: Goal Domain Type + Conversions — Update for Typed Columns

**Files:**
- Modify: `crates/goal/src/types.rs:12-28` (Goal struct)
- Modify: `crates/goal/src/conversions.rs` (goal_to_row, row_to_goal, compute_progress)
- Modify: `crates/agent/src/goal_handler.rs` (uses Goal)
- Modify: `crates/tools/src/goal_tool.rs` (if it reads metrics directly)

**Step 1: Write the test**

In `crates/goal/src/types.rs`, update the existing serde roundtrip test:

```rust
#[test]
fn test_goal_serde_roundtrip_typed_columns() {
    let now = Utc::now();
    let goal = Goal {
        id: Uuid::new_v4(),
        title: "Ship v2".to_string(),
        description: "Release version 2.0".to_string(),
        status: GoalStatus::Active,
        priority: 1,
        target_date: None,
        created_at: now,
        updated_at: now,
        plans_completed: 3,
        plans_failed: 1,
        avg_duration_ms: Some(5000),
        last_plan_at: Some(now),
        linked_project_ids: vec![Uuid::new_v4()],
    };

    let json = serde_json::to_string(&goal).unwrap();
    let deserialized: Goal = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.plans_completed, 3);
    assert_eq!(deserialized.plans_failed, 1);
    assert_eq!(deserialized.avg_duration_ms, Some(5000));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p goal -E 'test(test_goal_serde_roundtrip_typed)'`
Expected: FAIL — no field `plans_completed` on `Goal`

**Step 3: Update Goal domain type**

In `crates/goal/src/types.rs`, replace `metrics` and `metadata` with typed fields:

```rust
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub priority: u8,
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub plans_completed: i32,                    // REPLACES metrics
    pub plans_failed: i32,                       // REPLACES metadata
    pub avg_duration_ms: Option<i64>,            // NEW
    pub last_plan_at: Option<DateTime<Utc>>,     // NEW
    pub linked_project_ids: Vec<Uuid>,
}
```

Remove `Metric` struct and `GoalProgress.metrics` field. Update `compute_progress()`:

```rust
pub fn compute_progress(goal: &Goal) -> GoalProgress {
    let total = goal.plans_completed + goal.plans_failed;
    let completion = if total == 0 {
        0.0
    } else {
        (goal.plans_completed as f64 / total as f64) * 100.0
    };

    let summary = if total == 0 {
        "No plans executed yet".to_string()
    } else {
        format!(
            "{:.0}% completion rate ({} of {} plans succeeded)",
            completion, goal.plans_completed, total
        )
    };

    GoalProgress {
        goal_id: goal.id,
        completion_percentage: completion,
        summary,
    }
}
```

Update `GoalProgress` to remove `metrics` field:

```rust
pub struct GoalProgress {
    pub goal_id: Uuid,
    pub completion_percentage: f64,
    pub summary: String,
}
```

**Step 4: Update conversions.rs**

```rust
pub fn goal_to_row(goal: &Goal) -> storage::GoalRow {
    storage::GoalRow {
        id: goal.id,
        title: goal.title.clone(),
        description: goal.description.clone(),
        status: goal.status.to_string(),
        priority: goal.priority as i16,
        target_date: goal.target_date,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
        plans_completed: goal.plans_completed,
        plans_failed: goal.plans_failed,
        avg_duration_ms: goal.avg_duration_ms,
        last_plan_at: goal.last_plan_at,
    }
}

pub fn row_to_goal(row: storage::GoalRow, linked_project_ids: Vec<Uuid>) -> Goal {
    Goal {
        id: row.id,
        title: row.title,
        description: row.description,
        status: GoalStatus::from_str(&row.status).unwrap_or_default(),
        priority: row.priority as u8,
        target_date: row.target_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
        plans_completed: row.plans_completed,
        plans_failed: row.plans_failed,
        avg_duration_ms: row.avg_duration_ms,
        last_plan_at: row.last_plan_at,
        linked_project_ids,
    }
}
```

**Step 5: Fix all compilation errors**

Search for uses of `goal.metrics`, `goal.metadata`, `Metric`, `GoalProgress.metrics` across the codebase and update:
- `crates/agent/src/goal_handler.rs` — update `create_goal()` if it sets metrics/metadata
- `crates/tools/src/goal_tool.rs` — update if it reads metrics
- Any test files that construct `Goal` instances

**Step 6: Run tests**

Run: `cargo nextest run -p goal && cargo nextest run -p agent -E 'test(goal)'`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/goal/src/types.rs crates/goal/src/conversions.rs crates/agent/src/goal_handler.rs crates/tools/src/goal_tool.rs
git commit -m "feat(goal): replace metrics/metadata JSON with typed plan-completion columns"
```

---

## Task 10: PlanCompletionHandler — Use Typed Columns

**Files:**
- Modify: `crates/agent/src/plan_completion_handler.rs:29-73`

**Step 1: Write the test**

In `crates/agent/src/plan_completion_handler.rs`, add or update tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn test_on_plan_completed_increments_completed() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let goal_repo = storage::GoalRepo::new(pool.get());

        let goal_id = uuid::Uuid::new_v4();
        let row = storage::GoalRow {
            id: goal_id,
            title: "Test".to_string(),
            description: "".to_string(),
            status: "Active".to_string(),
            priority: 3,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        };
        goal_repo.create(&row).await.unwrap();

        let handler = PlanCompletionHandlerImpl::new(goal_repo.clone());
        handler
            .on_plan_completed(&uuid::Uuid::new_v4(), Some(goal_id), true, "done")
            .await
            .unwrap();

        let g = goal_repo.get(goal_id).await.unwrap();
        assert_eq!(g.plans_completed, 1);
        assert_eq!(g.plans_failed, 0);
    }

    #[tokio::test]
    async fn test_on_plan_completed_increments_failed() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let goal_repo = storage::GoalRepo::new(pool.get());

        let goal_id = uuid::Uuid::new_v4();
        let row = storage::GoalRow {
            id: goal_id,
            title: "Test".to_string(),
            description: "".to_string(),
            status: "Active".to_string(),
            priority: 3,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        };
        goal_repo.create(&row).await.unwrap();

        let handler = PlanCompletionHandlerImpl::new(goal_repo.clone());
        handler
            .on_plan_completed(&uuid::Uuid::new_v4(), Some(goal_id), false, "failed")
            .await
            .unwrap();

        let g = goal_repo.get(goal_id).await.unwrap();
        assert_eq!(g.plans_completed, 0);
        assert_eq!(g.plans_failed, 1);
    }
}
```

**Step 2: Run test to verify it fails (or produces wrong results)**

Run: `cargo nextest run -p agent -E 'test(test_on_plan_completed)'`
Expected: FAIL — current implementation writes JSON metadata, not typed columns

**Step 3: Rewrite `on_plan_completed()`**

Replace the body of `on_plan_completed()` in `plan_completion_handler.rs`:

```rust
async fn on_plan_completed(
    &self,
    _plan_id: &Uuid,
    goal_id: Option<Uuid>,
    success: bool,
    _summary: &str,
) -> Result<()> {
    let goal_id = match goal_id {
        Some(id) => id,
        None => return Ok(()), // No linked goal, nothing to update
    };

    if success {
        // Use 0 duration for now — caller can provide actual duration later
        // via increment_completed directly
        self.repo.increment_completed(goal_id, 0).await?;
    } else {
        self.repo.increment_failed(goal_id).await?;
    }

    Ok(())
}
```

Note: The plan duration can be computed by the caller (`run_plan_execution()` in `agent_loop.rs`) and passed directly to `increment_completed()`. For now, we pass 0 and can refine later.

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_on_plan_completed)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/agent/src/plan_completion_handler.rs
git commit -m "feat(agent): rewrite PlanCompletionHandler to use typed goal columns"
```

---

## Task 11: GoalTool — Add `metrics` Action

**Files:**
- Modify: `crates/tools/src/goal_tool.rs` (add "metrics" action)
- Modify: `crates/agent/src/goal_handler.rs` (add `goal_metrics()` method to GoalHandler)

**Step 1: Add `goal_metrics` to GoalHandler trait**

In `crates/tools/src/goal_tool.rs`, add to the `GoalHandler` trait:

```rust
async fn goal_metrics(&self, goal_id: &Uuid) -> Result<String>;
```

**Step 2: Implement in GoalHandlerImpl**

In `crates/agent/src/goal_handler.rs`:

```rust
async fn goal_metrics(&self, goal_id: &Uuid) -> Result<String> {
    let goal = conversions::load_goal(&self.repo, goal_id)
        .await?
        .ok_or_else(|| GoalError::NotFound(goal_id.to_string()))?;

    let total = goal.plans_completed + goal.plans_failed;
    let completion_rate = if total > 0 {
        format!("{:.0}% ({} of {} plans)", (goal.plans_completed as f64 / total as f64) * 100.0, goal.plans_completed, total)
    } else {
        "No plans executed yet".to_string()
    };

    let avg_duration = match goal.avg_duration_ms {
        Some(ms) if ms >= 3_600_000 => format!("{:.1} hours", ms as f64 / 3_600_000.0),
        Some(ms) if ms >= 60_000 => format!("{:.1} minutes", ms as f64 / 60_000.0),
        Some(ms) => format!("{} ms", ms),
        None => "N/A".to_string(),
    };

    let last_activity = match goal.last_plan_at {
        Some(dt) => dt.format("%Y-%m-%d %H:%M UTC").to_string(),
        None => "Never".to_string(),
    };

    // Count active plans if plan_repo is available
    let active_plans = if let Some(ref plan_repo) = self.plan_repo {
        let plans = plan_repo.list(None, None, Some(*goal_id)).await?;
        plans.iter().filter(|p| p.status == "Executing" || p.status == "Approved").count()
    } else {
        0
    };

    Ok(format!(
        "Goal: \"{}\"\nCompletion rate: {}\nAverage plan duration: {}\nLast activity: {}\nActive plans: {}",
        goal.title, completion_rate, avg_duration, last_activity, active_plans
    ))
}
```

**Step 3: Add "metrics" action to GoalTool execute()**

In `crates/tools/src/goal_tool.rs`, add the action to the parameters schema enum and the execute match:

```rust
"metrics" => {
    let goal_id = get_uuid_param(&args, "goal_id")?;
    handler.goal_metrics(&goal_id).await
}
```

Add `"metrics"` to the enum in the parameters schema.

**Step 4: Run tests**

Run: `cargo nextest run -p tools -E 'test(goal)' && cargo nextest run -p agent -E 'test(goal)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/tools/src/goal_tool.rs crates/agent/src/goal_handler.rs
git commit -m "feat(tools): add metrics action to GoalTool"
```

---

## Task 12: MessageKind Enum + InboundMessage Update

**Files:**
- Modify: `crates/bus/src/events.rs:13-57` (InboundMessage)
- Modify: `crates/bus/src/lib.rs` (re-export MessageKind)

**Step 1: Write the test**

In `crates/bus/src/events.rs`, add to existing tests or create:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_default_kind_is_text() {
        let msg = InboundMessage::new("telegram", "user1", "chat1", "hello");
        assert!(matches!(msg.kind, MessageKind::Text));
    }

    #[test]
    fn test_inbound_message_with_reaction_kind() {
        let msg = InboundMessage::new("telegram", "user1", "chat1", "👍")
            .with_kind(MessageKind::Reaction);
        assert!(matches!(msg.kind, MessageKind::Reaction));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p bus -E 'test(test_inbound_message)'`
Expected: FAIL — `MessageKind` not defined

**Step 3: Implement MessageKind and update InboundMessage**

In `crates/bus/src/events.rs`, add the enum before `InboundMessage`:

```rust
/// Distinguishes text messages from reactions (emoji on a previous message).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MessageKind {
    #[default]
    Text,
    Reaction,
}
```

Add `kind` field to `InboundMessage`:

```rust
pub struct InboundMessage {
    pub channel: ChannelName,
    pub sender_id: String,
    pub chat_id: ChatId,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub media: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub kind: MessageKind,  // NEW
}
```

Update `InboundMessage::new()` to set `kind: MessageKind::Text`.

Add builder method:

```rust
pub fn with_kind(mut self, kind: MessageKind) -> Self {
    self.kind = kind;
    self
}
```

**Step 4: Re-export from bus crate**

In `crates/bus/src/lib.rs`, add `MessageKind` to the public exports.

**Step 5: Run tests**

Run: `cargo nextest run -p bus && cargo build --workspace`
Expected: PASS (the workspace build verifies no downstream breakage)

**Step 6: Commit**

```bash
git add crates/bus/src/events.rs crates/bus/src/lib.rs
git commit -m "feat(bus): add MessageKind enum (Text vs Reaction) to InboundMessage"
```

---

## Task 13: StrategyRepo — `set_satisfaction_for_chat()`

**Files:**
- Modify: `crates/storage/src/repos/strategy.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_set_satisfaction_for_chat() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.get());

    let now = chrono::Utc::now();

    // Create a record with chat_id
    let row = StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: now,
        request_id: "req-1".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "DirectResponse".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 1,
        success: true,
        user_satisfaction: None,
        response_time_ms: 500,
        chat_id: Some("tg:123".to_string()),
    };
    repo.create(&row).await.unwrap();

    // Set satisfaction
    let since = now - chrono::Duration::minutes(5);
    let updated = repo
        .set_satisfaction_for_chat("tg:123", since, 1.0)
        .await
        .unwrap();
    assert!(updated);

    // Verify
    let fetched = repo.get(row.id).await.unwrap();
    assert_eq!(fetched.user_satisfaction, Some(1.0));
}

#[tokio::test]
async fn test_set_satisfaction_no_match() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.get());

    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let updated = repo
        .set_satisfaction_for_chat("nonexistent", since, 1.0)
        .await
        .unwrap();
    assert!(!updated);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(test_set_satisfaction)'`
Expected: FAIL — method doesn't exist

**Step 3: Implement `set_satisfaction_for_chat()`**

In `crates/storage/src/repos/strategy.rs`:

```rust
/// Update user_satisfaction on the most recent strategy record for a chat.
/// Returns true if a record was updated, false if no matching record found.
pub async fn set_satisfaction_for_chat(
    &self,
    chat_id: &str,
    since: DateTime<Utc>,
    satisfaction: f32,
) -> Result<bool, StorageError> {
    // SQLite doesn't support UPDATE...ORDER BY...LIMIT in standard syntax.
    // Use a subquery to find the most recent record's ID.
    let result = sqlx::query(
        "UPDATE strategy_records SET user_satisfaction = ?1 \
         WHERE id = ( \
           SELECT id FROM strategy_records \
           WHERE chat_id = ?2 AND timestamp >= ?3 \
           ORDER BY timestamp DESC LIMIT 1 \
         )"
    )
    .bind(satisfaction)
    .bind(chat_id)
    .bind(since.to_rfc3339())
    .execute(&self.pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(test_set_satisfaction)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/storage/src/repos/strategy.rs
git commit -m "feat(storage): add set_satisfaction_for_chat to StrategyRepo"
```

---

## Task 14: AgentLoop — Reaction Handler

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:188-253` (process_message)

**Step 1: Write the satisfaction mapping function and test**

Add a helper function (can be in agent_loop/mod.rs or a new module):

```rust
/// Map emoji reactions to satisfaction scores.
/// Returns None for unrecognized emoji (silently ignored).
fn reaction_to_satisfaction(emoji: &str) -> Option<f32> {
    match emoji.trim() {
        "\u{1F44D}" | "👍" => Some(1.0),   // thumbs up
        "\u{2764}" | "❤️" | "❤" => Some(1.0),   // heart
        "\u{1F389}" | "🎉" => Some(1.0),   // party
        "\u{1F44E}" | "👎" => Some(0.0),   // thumbs down
        "\u{1F615}" | "😕" => Some(0.0),   // confused
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reaction_to_satisfaction_positive() {
        assert_eq!(reaction_to_satisfaction("👍"), Some(1.0));
        assert_eq!(reaction_to_satisfaction("❤️"), Some(1.0));
        assert_eq!(reaction_to_satisfaction("🎉"), Some(1.0));
    }

    #[test]
    fn test_reaction_to_satisfaction_negative() {
        assert_eq!(reaction_to_satisfaction("👎"), Some(0.0));
        assert_eq!(reaction_to_satisfaction("😕"), Some(0.0));
    }

    #[test]
    fn test_reaction_to_satisfaction_unknown() {
        assert_eq!(reaction_to_satisfaction("🤔"), None);
        assert_eq!(reaction_to_satisfaction("hello"), None);
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_reaction_to_satisfaction)'`
Expected: PASS

**Step 3: Add reaction handling to `process_message()`**

In `crates/agent/src/agent_loop/mod.rs`, at the top of `process_message()` (after validation, before the system message check at line 196), add:

```rust
// Handle reaction messages — update satisfaction, no LLM call
if msg.kind == bus::MessageKind::Reaction {
    return self.handle_reaction(&msg).await;
}
```

Add the handler method to `impl AgentLoop`:

```rust
/// Handle emoji reactions by mapping to satisfaction scores.
/// Updates the most recent strategy_record for this chat. No response sent.
async fn handle_reaction(&self, msg: &InboundMessage) -> Result<()> {
    let score = match reaction_to_satisfaction(&msg.content) {
        Some(s) => s,
        None => {
            debug!("Ignoring unrecognized reaction emoji: {}", msg.content);
            return Ok(());
        }
    };

    // strategy_repo is on the pipeline's internal state — we need access.
    // The pipeline exposes strategy_repo via a getter, or we store it separately.
    // For now, access via the repos stored on AgentLoop.
    if let Some(ref strategy_repo) = self.strategy_repo {
        let since = chrono::Utc::now() - chrono::Duration::minutes(5);
        match strategy_repo.set_satisfaction_for_chat(
            msg.chat_id.as_str(),
            since,
            score,
        ).await {
            Ok(true) => {
                info!("Updated satisfaction score {} for chat {}", score, msg.chat_id);
            }
            Ok(false) => {
                debug!("No recent strategy record found for chat {}", msg.chat_id);
            }
            Err(e) => {
                warn!("Failed to update satisfaction: {}", e);
            }
        }
    }

    Ok(())
}
```

Note: `AgentLoop` needs a `strategy_repo: Option<storage::StrategyRepo>` field. Add it to the struct and wire it in the builder (`builder.rs`).

**Step 4: Wire strategy_repo into AgentLoop**

In `crates/agent/src/agent_loop/mod.rs`, add to `AgentLoop` struct:

```rust
strategy_repo: Option<storage::StrategyRepo>,
```

In `crates/agent/src/agent_loop/builder.rs`, set it from `repos.strategies.clone()`.

**Step 5: Run tests**

Run: `cargo nextest run -p agent`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): handle emoji reactions as satisfaction feedback"
```

---

## Task 15: Channel Adapters — Emit Reaction Events

**Files:**
- Modify: `crates/channels/src/telegram.rs`
- Modify: `crates/channels/src/discord.rs`
- Modify: `crates/channels/src/slack.rs`

This task depends on each channel's API library. The pattern is the same for all three:

**Step 1: Telegram — Add reaction event handler**

In the Telegram channel's event handler (where incoming messages are processed), add a handler for `MessageReactionUpdated` (or equivalent teloxide event):

```rust
// When a reaction is added to a bot message
if let Some(reaction) = update.reaction() {
    let emoji = extract_emoji(&reaction);
    let msg = InboundMessage::new(
        "telegram",
        &reaction.user_id.to_string(),
        &reaction.chat_id.to_string(),
        &emoji,
    ).with_kind(MessageKind::Reaction);

    if let Err(e) = inbound_tx.send(msg).await {
        warn!("Failed to send reaction event: {}", e);
    }
    return; // Don't process as text
}
```

**Step 2: Discord — Add reaction event handler**

In the Discord channel's event handler, add a handler for `ReactionAdd` events:

```rust
async fn reaction_add(&self, _ctx: Context, reaction: Reaction) {
    let emoji = reaction.emoji.to_string();
    let msg = InboundMessage::new(
        "discord",
        &reaction.user_id.map(|u| u.to_string()).unwrap_or_default(),
        &reaction.channel_id.to_string(),
        &emoji,
    ).with_kind(MessageKind::Reaction);

    if let Err(e) = self.inbound_tx.send(msg).await {
        warn!("Failed to send Discord reaction event: {}", e);
    }
}
```

**Step 3: Slack — Add reaction event handler**

In the Slack channel's event handler, add handling for `reaction_added` events:

```rust
"reaction_added" => {
    let emoji = event["reaction"].as_str().unwrap_or("");
    // Slack uses text names like "thumbsup" — convert to unicode
    let unicode_emoji = slack_reaction_to_unicode(emoji);
    let msg = InboundMessage::new(
        "slack",
        event["user"].as_str().unwrap_or(""),
        event["item"]["channel"].as_str().unwrap_or(""),
        &unicode_emoji,
    ).with_kind(MessageKind::Reaction);

    if let Err(e) = self.inbound_tx.send(msg).await {
        warn!("Failed to send Slack reaction event: {}", e);
    }
}
```

Add helper for Slack emoji name mapping:

```rust
fn slack_reaction_to_unicode(name: &str) -> String {
    match name {
        "thumbsup" | "+1" => "👍".to_string(),
        "thumbsdown" | "-1" => "👎".to_string(),
        "heart" => "❤️".to_string(),
        "tada" => "🎉".to_string(),
        "confused" => "😕".to_string(),
        other => format!(":{}:", other),
    }
}
```

**Step 4: Run workspace build**

Run: `cargo build --workspace`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add crates/channels/src/telegram.rs crates/channels/src/discord.rs crates/channels/src/slack.rs
git commit -m "feat(channels): emit reaction events from Telegram, Discord, and Slack"
```

---

## Task 16: Full Integration Smoke Test

**Files:**
- Create: `tests/learning_loop_test.rs` (or add to existing integration test file)

**Step 1: Write integration test**

```rust
//! Integration test for the learning loop: strategy persistence + satisfaction.

use klyntbot::storage::StoragePool;

#[tokio::test]
async fn test_strategy_record_written_after_pipeline() {
    // This test verifies the end-to-end flow:
    // 1. Pipeline processes a message
    // 2. StrategyRecordRow is written with correct chat_id
    // 3. Satisfaction can be backfilled via set_satisfaction_for_chat

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::storage::Repos::from_pool(&pool);

    // Create a strategy record (simulating what the pipeline writes)
    let record = klyntbot::storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "test-req".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 1,
        iterations_used: 3,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 1500,
        chat_id: Some("tg:user123".to_string()),
    };
    repos.strategies.create(&record).await.unwrap();

    // Simulate reaction: set satisfaction
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let updated = repos.strategies
        .set_satisfaction_for_chat("tg:user123", since, 1.0)
        .await
        .unwrap();
    assert!(updated);

    // Verify satisfaction was set
    let fetched = repos.strategies.get(record.id).await.unwrap();
    assert_eq!(fetched.user_satisfaction, Some(1.0));
    assert_eq!(fetched.chat_id, Some("tg:user123".to_string()));
}

#[tokio::test]
async fn test_goal_plan_completion_metrics() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::storage::Repos::from_pool(&pool);

    let goal_id = uuid::Uuid::new_v4();
    let row = klyntbot::storage::GoalRow {
        id: goal_id,
        title: "Ship v1".to_string(),
        description: "Release version 1".to_string(),
        status: "Active".to_string(),
        priority: 1,
        target_date: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        plans_completed: 0,
        plans_failed: 0,
        avg_duration_ms: None,
        last_plan_at: None,
    };
    repos.goals.create(&row).await.unwrap();

    // Simulate 2 completed plans and 1 failed
    repos.goals.increment_completed(goal_id, 60_000).await.unwrap(); // 1 min
    repos.goals.increment_completed(goal_id, 120_000).await.unwrap(); // 2 min
    repos.goals.increment_failed(goal_id).await.unwrap();

    let g = repos.goals.get(goal_id).await.unwrap();
    assert_eq!(g.plans_completed, 2);
    assert_eq!(g.plans_failed, 1);
    assert_eq!(g.avg_duration_ms, Some(90_000)); // (60k + 120k) / 2
    assert!(g.last_plan_at.is_some());
}
```

**Step 2: Run integration tests**

Run: `cargo nextest run --test learning_loop_test`
Expected: PASS

**Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace && cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: All PASS, 0 clippy warnings, formatting clean

**Step 4: Commit**

```bash
git add tests/learning_loop_test.rs
git commit -m "test: add integration tests for learning loop and goal metrics"
```

---

## Summary

| Task | Feature | Key Files |
|------|---------|-----------|
| 1 | Schema | `migrations/002_learning_loop.sql` |
| 2 | Strategy persistence | `rows/learning.rs`, `repos/strategy.rs` |
| 3 | Strategy persistence | `execution/dispatch.rs` |
| 4 | Strategy persistence | `pipeline.rs`, `builder.rs` |
| 5 | Sub-agent profiles | `subagent.rs`, `filesystem.rs` |
| 6 | Sub-agent profiles | `spawn.rs`, `subagent.rs` |
| 7 | Sub-agent profiles | `subagent.rs` |
| 8 | Goal metrics | `rows/goal.rs`, `repos/goal.rs` |
| 9 | Goal metrics | `goal/types.rs`, `goal/conversions.rs` |
| 10 | Goal metrics | `plan_completion_handler.rs` |
| 11 | Goal metrics | `goal_tool.rs`, `goal_handler.rs` |
| 12 | Satisfaction | `bus/events.rs` |
| 13 | Satisfaction | `repos/strategy.rs` |
| 14 | Satisfaction | `agent_loop/mod.rs`, `builder.rs` |
| 15 | Satisfaction | `telegram.rs`, `discord.rs`, `slack.rs` |
| 16 | Integration | `tests/learning_loop_test.rs` |
