# Remove TaskTool Hand-Wiring — Use FeaturePackage::tools()

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the hand-wired `TaskTool` construction and registration in `AgentLoopBuilder::build()` (lines 1252-1318) and replace it with a `FeaturePackage::tools()` iteration through a `TasksFeature` instance.

**Architecture:** The `TasksFeature` already has `with_task_tool()` and `tools()` from Task 13. We need to: (1) add builder methods on `TasksFeature` for the injected capabilities so it can build the `TaskTool` itself, OR (2) construct the `TaskTool` in the agent builder and pass it to `TasksFeature::with_task_tool()`. Option 2 is simpler and preserves the builder pattern. Then remove the hand-wired block and add a `native_features` iteration for tool registration.

**Tech Stack:** Rust, async_trait, tools_core::FeaturePackage

---

## File Structure

- `crates/agent/src/agent_loop/builder.rs` — Remove hand-wired TaskTool block (lines 1252-1318), preserve `todo_embedding_handler` and `progress_handler` logic, add `native_features` iteration
- `crates/feature-tasks/src/lib.rs` — Add builder methods for individual capabilities (area_repo, embedding_handler, progress_handler, domain_bus, alarm_writer) if not present
- `crates/feature-tasks/tests/feature_package_test.rs` — Add test for `TasksFeature::tools()` with full configuration

---

## Task 1: Add Capability Builder Methods to TasksFeature

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs:42-68`

The `TasksFeature` currently only stores `pool` and `task_tool`. We need builder methods that accept the individual capabilities and store them, so `tools()` can construct the `TaskTool` lazily. However, since `TaskTool` builder methods consume `self`, it's simpler to just pass a fully-built `TaskTool` via `with_task_tool()`. 

**Decision:** The `TasksFeature` already has `with_task_tool()`. We'll use that in the agent builder. No changes needed to `TasksFeature`.

- [ ] **Step 1: Verify no changes needed to TasksFeature**

The existing `TasksFeature::with_task_tool()` is sufficient. Skip to Task 2.

---

## Task 2: Refactor AgentLoopBuilder to Use TasksFeature

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:1240-1359`

Replace the hand-wired TaskTool block with:
1. Construct `TaskTool` as before (preserving all logic)
2. Create `TasksFeature` and pass the tool via `with_task_tool()`
3. Add `native_features` iteration after all individual tool registrations
4. Remove the direct `tool_registry.register(task_tool)` call

**Key constraints:**
- Preserve `todo_embedding_handler` variable (used by MemoryTool at line 1368)
- Preserve `progress_handler` variable (used by OkrTool at line 1323)
- The `TasksFeature` must be stored in a `native_features` vector for iteration

- [ ] **Step 1: Refactor the TaskTool block to use TasksFeature**

```rust
        // ── Feature-tasks tool (requires real pool) ───────────────────────
        let mut native_features: Vec<Box<dyn FeaturePackage>> = vec![];
        
        if self.pool.is_some() {
            let pool_ref = storage_pool.inner();
            let task_repo = storage::TaskRepo::new(pool_ref.clone());
            let area_repo = storage::AreaRepo::new(pool_ref.clone());
            let mut task_tool = feature_tasks::TaskTool::new(
                task_repo,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
                config.timezone.clone(),
            )
            .with_area_repo(area_repo);

            // Task embedding (semantic search)
            if let (true, Some(vs)) = (config.todo.search.enabled, self.vector_store.clone()) {
                let task_embed_impl =
                    Arc::new(crate::adapters::task_embedding::TaskEmbeddingAdapter::new(
                        Arc::clone(&embedding_engine),
                        vs.clone(),
                    ));

                let memory_embed_impl = Arc::new(tools::EmbeddingEngineImpl::new(
                    Arc::clone(&embedding_engine),
                    vs.clone(),
                ));

                task_tool = task_tool
                    .with_embedding_handler(
                        Arc::clone(&task_embed_impl) as Arc<dyn feature_tasks::EmbeddingHandler>
                    )
                    .with_embedding_store(vs)
                    .with_search_config(
                        config.todo.search.semantic_threshold,
                        config.todo.search.rrf_k,
                    );

                todo_embedding_handler =
                    Some(Arc::clone(&memory_embed_impl) as Arc<dyn tools::EmbeddingHandler>);
            } else {
                todo_embedding_handler = None;
            }

            // Inject progress handler for KR→Objective cascade
            let progress_handler: Arc<dyn tools_core::ProgressHandler> =
                Arc::new(crate::adapters::progress::ProgressHandlerImpl::new(
                    repos.key_results.clone(),
                    repos.objectives.clone(),
                    repos.tasks.clone(),
                ));
            task_tool = task_tool.with_progress_handler(Arc::clone(&progress_handler));

            // Wire DomainEventBus for task lifecycle events
            if let Some(ref domain_bus) = self.domain_event_bus {
                task_tool = task_tool.with_domain_bus(Arc::clone(domain_bus));
            }

            // Wire alarm writer (task_alarms repo + FireStore)
            {
                let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
                    repos.scheduled_fires.clone(),
                ));
                task_tool = task_tool.with_alarm_writer(repos.task_alarms.clone(), fire_store);
            }

            // Register via FeaturePackage
            let tasks_feature = feature_tasks::TasksFeature::new()
                .with_task_tool(Arc::new(task_tool));
            native_features.push(Box::new(tasks_feature));

            // ── OKR tool (needs same progress handler) ────────────────────
            tool_registry.register(
                OkrTool::new(repos.objectives.clone(), repos.key_results.clone())
                    .with_progress_handler(Arc::clone(&progress_handler)),
            );
        } else {
            // No pool available (e.g., test environment)
            todo_embedding_handler = None;

            // OKR tool without progress handler
            tool_registry.register(OkrTool::new(
                repos.objectives.clone(),
                repos.key_results.clone(),
            ));
        }
```

- [ ] **Step 2: Add native_features iteration after all tool registrations**

After all the individual `tool_registry.register(...)` calls (around line 1478+), add:

```rust
        // ── Native feature packages ──────────────────────────────────────
        for feature in &native_features {
            for tool in feature.tools() {
                tool_registry.register_dyn(tool);
            }
        }
```

- [ ] **Step 3: Build and test**

Run:
```bash
cargo build --workspace
cargo nextest run -p agent -p feature-tasks
```

Expected: Build succeeds, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): remove TaskTool hand-wiring; use FeaturePackage::tools()"
```

---

## Task 3: Add Test for TasksFeature with Full Tool Configuration

**Files:**
- Modify: `crates/feature-tasks/tests/feature_package_test.rs`

- [ ] **Step 1: Add integration test**

```rust
#[tokio::test]
async fn tasks_feature_tools_returns_task_tool_when_configured() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    
    // Create minimal schema
    sqlx::query("CREATE TABLE IF NOT EXISTS areas (id TEXT PRIMARY KEY, name TEXT NOT NULL, color TEXT, status TEXT)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT OR IGNORE INTO areas (id, name, color, status) VALUES ('test-area', 'Test Area', '#000', 'active')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(r#"CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
        area_id TEXT NOT NULL, project_id TEXT, key_result_id TEXT, objective_id TEXT,
        parent_id TEXT, status_label_id TEXT, group_id TEXT,
        priority INTEGER, position INTEGER NOT NULL DEFAULT 0,
        due_date INTEGER, tags TEXT NOT NULL DEFAULT '[]',
        status TEXT NOT NULL DEFAULT 'todo',
        task_type TEXT NOT NULL DEFAULT 'manual',
        focused_at INTEGER, focus_deadline INTEGER,
        focus_expired_count INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
        updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
        completed_at INTEGER, completed INTEGER NOT NULL DEFAULT 0,
        total_tracked_secs INTEGER NOT NULL DEFAULT 0,
        estimated_minutes INTEGER, actual_minutes INTEGER,
        calendar_event_uid TEXT, last_reminded_at INTEGER,
        recurrence_rule TEXT, recurrence_parent_id TEXT,
        is_template INTEGER NOT NULL DEFAULT 0, next_instance_date INTEGER,
        energy_level TEXT DEFAULT 'medium',
        estimated_focus_blocks INTEGER, complexity_score INTEGER,
        scheduled_start INTEGER, scheduled_end INTEGER
    )"#)
        .execute(&db)
        .await
        .unwrap();
    
    let repo = storage::TaskRepo::new(db);
    let task_tool = feature_tasks::TaskTool::new(repo, 3, 8, "UTC".to_string());
    let feature = feature_tasks::TasksFeature::new()
        .with_task_tool(Arc::new(task_tool));
    
    let tools = feature.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "tasks");
}
```

- [ ] **Step 2: Run test**

```bash
cargo nextest run -p feature-tasks tasks_feature_tools_returns_task_tool_when_configured
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/tests/feature_package_test.rs
git commit -m "test(feature-tasks): add TasksFeature::tools() integration test"
```

---

## Self-Review

**1. Spec coverage:**
- ✅ Remove hand-wired TaskTool block (lines 1252-1318) — Task 2
- ✅ Use `FeaturePackage::tools()` iteration — Task 2
- ✅ Preserve `todo_embedding_handler` and `progress_handler` — Task 2
- ✅ Add `native_features` iteration — Task 2
- ✅ Tests pass — Task 2, Task 3

**2. Placeholder scan:** No placeholders found.

**3. Type consistency:** All types match existing codebase patterns.

---

## Execution Handoff

**Plan complete.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
