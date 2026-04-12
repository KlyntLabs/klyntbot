# Entity Store — Plan 4: Templates + Cleanup

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the Task Management and Finance template bundles (manifest + skill), wire template installation on first run, remove old feature crates from the workspace, and run end-to-end integration tests.

**Architecture:** Templates are JSON manifest files + skill directories stored in `templates/`. On first run (no databases exist), the default templates are installed via `EntityStore::install_template()`. Old feature crates (feature-tasks, feature-finance, feature-learning) are removed from the workspace. All references in agent builder, app-core, and desktop are updated.

**Tech Stack:** Rust, serde_json, YAML (skill frontmatter)

**Spec:** `docs/superpowers/specs/2026-04-12-flexible-database-engine-design.md` (Sections 9, 11)

**Depends on:** Plans 1, 2, 3

---

## Task 1: Create Task Management template

**Files:**
- Create: `templates/task-management/manifest.json`
- Create: `templates/task-management/skill/SKILL.md`
- Create: `templates/task-management/skill/references/workflows.md`

- [ ] **Step 1: Write manifest.json with full field definitions matching current task schema**

Fields: Title (text, required), Description (text), Status (select: todo/doing/done/someday), Priority (select: low/medium/high/urgent), Due Date (date), Tags (multi_select), Energy Level (select: low/medium/high/deep), Estimated Minutes (number), Actual Minutes (number), Parent (relation: self).

Views: "All Tasks" (table), "Board" (board grouped by status), "Calendar" (by due_date).

Dashboard: "Task Overview" with count widget (open tasks) + pie chart (by priority).

- [ ] **Step 2: Write SKILL.md with mature task management skill**

Include schema_hints (status as lifecycle, due_date as temporal/urgency_source, priority as ranking, energy_level as behavioral), salience declarations, context_rules, and behavioral instructions.

- [ ] **Step 3: Write references/workflows.md with task management workflows**
- [ ] **Step 4: Commit**

---

## Task 2: Create Finance template bundle

**Files:**
- Create: `templates/finance/manifest.json`
- Create: `templates/finance/skill/SKILL.md`

- [ ] **Step 1: Write manifest.json with multiple databases**

Databases: "Accounts" (name, type, balance, currency, institution), "Transactions" (date, amount, category, description, account relation), "Budgets" (category, monthly_limit, period).

Views per database: table view + appropriate extras.

- [ ] **Step 2: Write SKILL.md with finance management skill**
- [ ] **Step 3: Commit**

---

## Task 3: Wire default template installation on first run

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/entity-store/src/templates.rs`

- [ ] **Step 1: In app-core init, after EntityStore creation, check if databases table is empty**
- [ ] **Step 2: If empty, install default templates (task-management, finance) via EntityStore::install_template()**
- [ ] **Step 3: Template installation creates databases + fields + views + copies skill to ~/.klyntbot/skills/db-{id}/**
- [ ] **Step 4: Write test for first-run template installation**
- [ ] **Step 5: Commit**

---

## Task 4: Remove feature-tasks crate

**Files:**
- Delete: `crates/feature-tasks/` (entire directory)
- Modify: `Cargo.toml` (workspace root — remove from members)
- Modify: `crates/agent/Cargo.toml` (remove dependency)
- Modify: `crates/agent/src/agent_loop/builder.rs` (remove TaskTool registration)
- Modify: `crates/app-core/Cargo.toml` (remove dependency)
- Modify: `crates/app-core/src/state.rs` (remove task handler fields)
- Modify: `crates/app-core/src/handlers/mod.rs` (remove tasks module)
- Modify: `crates/app-core/src/init/mod.rs` (remove task handler wiring)
- Modify: `crates/app-core/src/init/cron.rs` (remove proactive scan job)
- Modify: `crates/desktop/src/commands/mod.rs` (remove tasks module)
- Modify: `crates/config/src/schema/mcp.rs` (remove "tasks" from exposed tools)
- Modify: `src/lib.rs` (remove feature_tasks re-export)

- [ ] **Step 1: Remove the crate directory**
- [ ] **Step 2: Remove all references — follow compiler errors**
- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

---

## Task 5: Remove feature-finance crate

**Files:**
- Delete: `crates/feature-finance/` (entire directory)
- Modify: All files that reference feature-finance (same pattern as Task 4)

- [ ] **Step 1: Remove the crate directory**
- [ ] **Step 2: Remove all references — follow compiler errors**
- [ ] **Step 3: Verify compilation**
- [ ] **Step 4: Commit**

---

## Task 6: Remove feature-learning crate

**Files:**
- Delete: `crates/feature-learning/` (entire directory)
- Modify: `crates/app-core/src/handlers/notes/card_generation.rs` (inline the prompt building)

- [ ] **Step 1: Move the ~45 lines of prompt building from feature-learning/src/card_generator.rs into card_generation.rs**
- [ ] **Step 2: Delete the crate**
- [ ] **Step 3: Remove all references**
- [ ] **Step 4: Verify compilation**
- [ ] **Step 5: Commit**

---

## Task 7: Update existing skills to reference new database tool

**Files:**
- Modify: `skills/task-management/SKILL.md`
- Modify: `skills/finance-management/SKILL.md`

- [ ] **Step 1: Update tool references from `task` → `database` and `finance` → `database`**
- [ ] **Step 2: Update action names to match DatabaseTool actions**
- [ ] **Step 3: Commit**

---

## Task 8: Remove old custom_columns system

**Files:**
- Delete: `crates/storage/src/repos/custom_column.rs`
- Modify: `crates/storage/src/repos/mod.rs` (remove custom_column)
- Delete: `desktop-ui/src/shared/hooks/useCustomColumns.ts`

- [ ] **Step 1: Remove the old custom columns repo and hook (replaced by database_fields)**
- [ ] **Step 2: Remove migration for custom_columns table (consolidate into entity-store migration)**
- [ ] **Step 3: Verify compilation**
- [ ] **Step 4: Commit**

---

## Task 9: End-to-end integration tests

**Files:**
- Create: `tests/integration/entity_store.rs`

- [ ] **Step 1: Test full lifecycle via facade crate: create database → add fields → create entities → query → add view → AI suggestion → delete**
- [ ] **Step 2: Test template installation: load task-management manifest → verify database + fields + views created**
- [ ] **Step 3: Test cross-database relations: create two databases → link entities → list relations**
- [ ] **Step 4: Test domain events: create entity → verify EntityCreated event fires**
- [ ] **Step 5: Run full workspace test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 6: Commit**

---

## Task 10: Final workspace health check

- [ ] **Step 1: Run all tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Run fmt**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`

- [ ] **Step 5: Run frontend lint**

Run: `cd desktop-ui && bun run lint`

- [ ] **Step 6: Start dev server and verify everything works end-to-end**

Run: `cd desktop-ui && bun run dev` + `cargo tauri dev`
Verify: sidebar shows databases, create/edit/delete entities works, views switch correctly, dashboard widgets render

- [ ] **Step 7: Final commit**

```bash
git commit -m "feat: entity store — complete flexible database system with AI evolution"
```
