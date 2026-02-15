# Klyntbot Todo Enhancement Roadmap

**Status**: Draft
**Version**: 1.0
**Last Updated**: 2026-02-15
**Current Score**: 94/100
**Target Score**: 99/100

---

## Executive Summary

Klyntbot already has production-grade architecture (95/100) and a rich data model (85/100). The gap to greatness lies in:

1. **Surface Area**: Only 8 of 17 todo actions exposed via CLI
2. **Missing Features**: No recurring tasks, no dependencies
3. **AI Intelligence**: Not leveraging existing data (time tracking, patterns, calendar) proactively
4. **Multi-Device**: No sync mechanism for daily-driver use

This roadmap closes those gaps through **8 focused sprints** (14 weeks, ~140 hours total).

**Impact**: 94 → 99/100, transforming Klyntbot from "excellent personal tool" to "legitimately beats $15/mo SaaS competitors."

---

## Table of Contents

- [Phase 1: Close the Obvious Gaps (Weeks 1-6)](#phase-1-close-the-obvious-gaps-weeks-1-6)
  - [Sprint 1: CLI Parity (Week 1-2)](#sprint-1-cli-parity-week-1-2)
  - [Sprint 2: Recurring Tasks & Dependencies (Week 3-4)](#sprint-2-recurring-tasks--dependencies-week-3-4)
  - [Sprint 3: Bidirectional Calendar Sync (Week 5-6)](#sprint-3-bidirectional-calendar-sync-week-5-6)
- [Phase 2: Make the AI Intelligent (Weeks 7-12)](#phase-2-make-the-ai-intelligent-weeks-7-12)
  - [Sprint 4: Smart Enrichment Engine (Week 7-8)](#sprint-4-smart-enrichment-engine-week-7-8)
  - [Sprint 5: Semantic Search (Week 9-10)](#sprint-5-semantic-search-week-9-10)
  - [Sprint 6: Daily Planning Skill (Week 11-12)](#sprint-6-daily-planning-skill-week-11-12)
- [Phase 3: Daily-Driver Polish (Weeks 13-14)](#phase-3-daily-driver-polish-weeks-13-14)
  - [Sprint 7: Git Sync & Multi-Device (Week 13-14)](#sprint-7-git-sync--multi-device-week-13-14)
- [Phase 4: Moonshot Features (Future)](#phase-4-moonshot-features-future)
- [Appendix: Deferred Ideas](#appendix-deferred-ideas)

---

## Phase 1: Close the Obvious Gaps (Weeks 1-6)

**Goal**: Expose all existing functionality, add critical missing features (recurring tasks, dependencies, bidirectional calendar sync).

**Impact**: +31 points (94 → 97/100)

---

### Sprint 1: CLI Parity (Week 1-2)

**Objective**: All 17 todo actions + 6 project actions accessible via CLI.

**Why**: Currently only 8 of 17 todo actions have CLI commands. Power users can't script or alias advanced operations without chatting with the LLM.

#### Tasks

- [ ] **1.1.1** Add CLI command: `klyntbot todo tree`
  - **Location**: `crates/cli/src/todo/tree.rs` (new file)
  - **Params**: `--project <ID>`, `--depth <N>`
  - **Handler**: Call `TodoTool::execute()` with `action: "tree"`
  - **Output**: ASCII tree with ├─/└─ connectors
  - **Effort**: 2 hours

- [ ] **1.1.2** Add CLI command: `klyntbot todo search <query>`
  - **Location**: `crates/cli/src/todo/search.rs` (new file)
  - **Params**: `<query>`, `--include-attachments`
  - **Handler**: Call `TodoTool::execute()` with `action: "search"`
  - **Output**: Formatted list of matching todos
  - **Effort**: 1.5 hours

- [ ] **1.1.3** Add CLI command: `klyntbot todo report`
  - **Location**: `crates/cli/src/todo/report.rs` (new file)
  - **Params**: `--period <week|month>`, `--project <ID>`
  - **Handler**: Call `TodoTool::execute()` with `action: "report"`
  - **Output**: Markdown formatted analytics
  - **Effort**: 2 hours

- [ ] **1.1.4** Add CLI command: `klyntbot todo attach <id>`
  - **Location**: `crates/cli/src/todo/attach.rs` (new file)
  - **Params**: `<id>`, `--file <PATH>`, `--url <URL>`, `--note <TEXT>`, `--title <TEXT>`
  - **Handler**: Call `TodoTool::execute()` with `action: "attach"`
  - **Validation**: Exactly one of --file, --url, --note required
  - **Effort**: 2 hours

- [ ] **1.1.5** Add CLI command: `klyntbot todo detach <id> <attachment_id>`
  - **Location**: `crates/cli/src/todo/attach.rs` (add to same file)
  - **Handler**: Call `TodoTool::execute()` with `action: "detach"`
  - **Effort**: 1 hour

- [ ] **1.1.6** Add CLI command: `klyntbot todo add-subtask <parent_id> <title>`
  - **Location**: `crates/cli/src/todo/add.rs` (extend existing)
  - **Params**: Same as `add` but with required `<parent_id>`
  - **Handler**: Call `TodoTool::execute()` with `action: "add_subtask"`
  - **Effort**: 1.5 hours

- [ ] **1.1.7** Add CLI command: `klyntbot todo move <id>`
  - **Location**: `crates/cli/src/todo/move.rs` (new file)
  - **Params**: `<id>`, `--parent <ID|none>`, `--project <ID|none>`
  - **Handler**: Call `TodoTool::execute()` with `action: "move"`
  - **Effort**: 2 hours

- [ ] **1.1.8** Add CLI command: `klyntbot todo log-time <id> <minutes>`
  - **Location**: `crates/cli/src/todo/time.rs` (new file)
  - **Params**: `<id>`, `<minutes>`, `--note <TEXT>`
  - **Handler**: Call `TodoTool::execute()` with `action: "log_time"`
  - **Effort**: 1.5 hours

- [ ] **1.1.9** Extend CLI command: `klyntbot todo update <id>`
  - **Location**: `crates/cli/src/todo/update.rs` (new file)
  - **Params**: `<id>`, `--title <TEXT>`, `--description <TEXT>`, `--priority <1-5>`, `--due <DATE>`, `--tags <TAG1,TAG2>`, `--status <todo|doing|done|archived>`
  - **Handler**: Call `TodoTool::execute()` with `action: "update"`
  - **Note**: Currently missing from CLI (action exists in tool)
  - **Effort**: 2 hours

- [ ] **1.2.1** Add CLI command: `klyntbot project create <name>`
  - **Location**: `crates/cli/src/project/create.rs` (new file)
  - **Params**: `<name>`, `--description <TEXT>`, `--color <red|orange|yellow|green|blue|purple|gray>`, `--tags <TAG1,TAG2>`
  - **Handler**: Call `ProjectTool::execute()` with `action: "create"`
  - **Effort**: 2 hours

- [ ] **1.2.2** Add CLI command: `klyntbot project list`
  - **Location**: `crates/cli/src/project/list.rs` (new file)
  - **Params**: `--status <active|paused|completed|archived>`, `--tag <TAG>`, `--limit <N>`
  - **Handler**: Call `ProjectTool::execute()` with `action: "list"`
  - **Effort**: 1.5 hours

- [ ] **1.2.3** Add CLI command: `klyntbot project show <id>`
  - **Location**: `crates/cli/src/project/list.rs` (extend)
  - **Handler**: Call `ProjectTool::execute()` with `action: "show"`
  - **Effort**: 1 hour

- [ ] **1.2.4** Add CLI command: `klyntbot project update <id>`
  - **Location**: `crates/cli/src/project/update.rs` (new file)
  - **Params**: `<id>`, `--name <TEXT>`, `--description <TEXT>`, `--color <COLOR>`, `--status <STATUS>`, `--tags <TAG1,TAG2>`
  - **Handler**: Call `ProjectTool::execute()` with `action: "update"`
  - **Effort**: 2 hours

- [ ] **1.2.5** Add CLI command: `klyntbot project archive <id>`
  - **Location**: `crates/cli/src/project/update.rs` (extend)
  - **Handler**: Call `ProjectTool::execute()` with `action: "archive"`
  - **Effort**: 1 hour

- [ ] **1.2.6** Add CLI command: `klyntbot project tasks <id>`
  - **Location**: `crates/cli/src/project/tasks.rs` (new file)
  - **Params**: `<id>`, `--limit <N>`, `--tree`
  - **Handler**: Call `ProjectTool::execute()` with `action: "tasks"`
  - **Effort**: 1.5 hours

- [ ] **1.2.7** Update `crates/cli/src/commands.rs`
  - **Change**: Add `ProjectCommands` enum (currently missing)
  - **Wire**: Route `Commands::Project(cmd)` → `cli_handlers::handle_project()`
  - **Effort**: 0.5 hours

- [ ] **1.3.1** Wire natural language date parsing to CLI
  - **Location**: `crates/cli/src/todo/add.rs`, `update.rs`
  - **Change**: Replace ISO date parsing with `parse_datetime(&due_str, &config.timezone)`
  - **Note**: Utility already exists in `tools/src/todo.rs:1988`
  - **Examples**: `--due tomorrow`, `--due "next Friday"`, `--due "in 3 days"`
  - **Effort**: 0.5 hours

#### Testing

- [ ] **1.T.1** Integration test: All 17 todo CLI commands execute successfully
- [ ] **1.T.2** Integration test: All 6 project CLI commands execute successfully
- [ ] **1.T.3** Unit test: Natural language dates parse correctly (tomorrow, next week, etc.)

#### Acceptance Criteria

- ✅ `klyntbot todo --help` shows all 17 actions
- ✅ `klyntbot project --help` shows all 6 actions
- ✅ `klyntbot todo add "test" --due tomorrow` works without ISO date
- ✅ All commands produce formatted output with timezone-aware dates
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(cli): expose all 17 todo + 6 project actions to CLI`

**Impact**: +11 points (85 → 96/100 usability)

**Effort**: 25 hours

---

### Sprint 2: Recurring Tasks & Dependencies (Week 3-4)

**Objective**: Support recurring tasks (RRULE-compatible) and task dependencies (blocked_by/blocks).

**Why**: #1 and #2 most requested features in every todo app. Power users leave without them.

#### 2.1: Recurring Tasks

- [ ] **2.1.1** Extend Todo data model
  - **Location**: `crates/tools/src/todo_types.rs`
  - **Add fields**:
    ```rust
    pub recurrence_rule: Option<String>,       // "FREQ=DAILY;INTERVAL=1;BYHOUR=9"
    pub recurrence_parent_id: Option<String>,  // Link instance to template
    pub is_template: bool,                      // If true, this is template not instance
    pub next_instance_date: Option<DateTime<Utc>>, // Cache for next occurrence
    ```
  - **Effort**: 0.5 hours

- [ ] **2.1.2** Add RRULE parsing utility
  - **Location**: `crates/tools/src/rrule.rs` (new file)
  - **Dependencies**: Add `rrule` crate (https://github.com/fmeringdal/rust_rrule)
  - **Functions**:
    ```rust
    pub fn parse_rrule(rule: &str) -> Result<RRuleSet>;
    pub fn next_occurrence(rrule: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>>;
    pub fn should_spawn_instance(template: &Todo, now: DateTime<Utc>) -> Result<bool>;
    ```
  - **Effort**: 3 hours

- [ ] **2.1.3** Add TodoTool action: `recur`
  - **Location**: `crates/tools/src/todo.rs`
  - **Action**: `"recur"`
  - **Parameters**:
    ```json
    {
      "action": "recur",
      "title": "Daily standup",
      "rule": "FREQ=DAILY;BYHOUR=9;BYMINUTE=0",
      "description": "...",
      "priority": 3,
      "tags": ["meeting"]
    }
    ```
  - **Behavior**:
    1. Create template todo with `is_template: true`
    2. Set `recurrence_rule` from params
    3. Calculate `next_instance_date`
    4. Store template (don't show in normal lists)
  - **Effort**: 2 hours

- [ ] **2.1.4** Add TodoTool action: `create_from_template`
  - **Location**: `crates/tools/src/todo.rs`
  - **Action**: `"create_from_template"`
  - **Parameters**:
    ```json
    {
      "action": "create_from_template",
      "template_id": "abc12345"
    }
    ```
  - **Behavior**:
    1. Clone template todo
    2. Generate new ID
    3. Set `is_template: false`
    4. Set `recurrence_parent_id: Some(template_id)`
    5. Calculate `due_date` from `next_occurrence()`
    6. Add to store
  - **Effort**: 1.5 hours

- [ ] **2.1.5** Add background job: spawn recurring instances
  - **Location**: `crates/agent/src/recurring_tasks.rs` (new file)
  - **Function**:
    ```rust
    pub async fn spawn_recurring_task_instances(
        todo_store: Arc<RwLock<TodoStore>>,
        now: DateTime<Utc>
    ) -> Result<Vec<String>> {
        // Get all templates
        // For each: if should_spawn_instance() → create_from_template()
        // Update template.next_instance_date
        // Return spawned IDs
    }
    ```
  - **Effort**: 2 hours

- [ ] **2.1.6** Register cron job
  - **Location**: `crates/agent/src/agent_loop.rs`
  - **Change**: Register daily cron (runs at midnight in user timezone)
  - **Job**: `spawn_recurring_task_instances()`
  - **Effort**: 0.5 hours

- [ ] **2.1.7** Filter templates from normal lists
  - **Location**: `crates/tools/src/todo_store.rs`
  - **Change**: `list()` method should filter `is_template: true` by default
  - **Add**: New method `list_templates()` to explicitly fetch templates
  - **Effort**: 1 hour

- [ ] **2.1.8** CLI command: `klyntbot todo recur add <title>`
  - **Location**: `crates/cli/src/todo/recur.rs` (new file)
  - **Params**: `<title>`, `--rule <RRULE>`, `--description <TEXT>`, `--priority <1-5>`, `--tags <TAG1,TAG2>`
  - **Handler**: Call `TodoTool::execute()` with `action: "recur"`
  - **Examples**:
    ```bash
    klyntbot todo recur add "Daily standup" --rule "FREQ=DAILY;BYHOUR=9"
    klyntbot todo recur add "Weekly review" --rule "FREQ=WEEKLY;BYDAY=FR;BYHOUR=16"
    klyntbot todo recur add "Monthly report" --rule "FREQ=MONTHLY;BYMONTHDAY=1"
    ```
  - **Effort**: 2 hours

- [ ] **2.1.9** CLI command: `klyntbot todo recur list`
  - **Location**: `crates/cli/src/todo/recur.rs` (extend)
  - **Handler**: Call `TodoStore::list_templates()`
  - **Output**: Show template title, rule, next instance date
  - **Effort**: 1 hour

#### 2.2: Task Dependencies

- [ ] **2.2.1** Extend Todo data model
  - **Location**: `crates/tools/src/todo_types.rs`
  - **Add fields**:
    ```rust
    pub blocked_by: Vec<String>,  // Task IDs this task depends on
    pub blocks: Vec<String>,       // Task IDs that depend on this task
    ```
  - **Effort**: 0.5 hours

- [ ] **2.2.2** Add TodoTool action: `add_dependency`
  - **Location**: `crates/tools/src/todo.rs`
  - **Action**: `"add_dependency"`
  - **Parameters**:
    ```json
    {
      "action": "add_dependency",
      "task_id": "abc123",
      "blocked_by": "def456"
    }
    ```
  - **Behavior**:
    1. Add `blocked_by` to task_id
    2. Add `blocks` to def456 (bidirectional link)
    3. Validate no cycles (optional: detect with DFS)
  - **Effort**: 2 hours

- [ ] **2.2.3** Add TodoTool action: `remove_dependency`
  - **Location**: `crates/tools/src/todo.rs`
  - **Action**: `"remove_dependency"`
  - **Parameters**: Same as `add_dependency`
  - **Behavior**: Remove from both sides
  - **Effort**: 1 hour

- [ ] **2.2.4** Validate completion with blockers
  - **Location**: `crates/tools/src/todo.rs`
  - **Change**: In `"complete"` action, check `blocked_by` field
  - **Behavior**:
    ```rust
    if !todo.blocked_by.is_empty() {
        let incomplete_blockers = store.batch_get(&todo.blocked_by).await?
            .into_iter()
            .filter(|t| t.status != TodoStatus::Done)
            .collect::<Vec<_>>();

        if !incomplete_blockers.is_empty() {
            return Err(anyhow!(
                "Cannot complete: blocked by {} incomplete tasks: {}",
                incomplete_blockers.len(),
                incomplete_blockers.iter()
                    .map(|t| format!("{} ({})", t.title, t.id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    ```
  - **Effort**: 1.5 hours

- [ ] **2.2.5** Update `tree` action to show dependencies
  - **Location**: `crates/tools/src/todo.rs`
  - **Change**: In `"tree"` action output, show arrows for dependencies
  - **Format**:
    ```
    ├─ [abc123] Deploy to prod
    │  ⚠️  Blocked by: def456 (Run tests)
    └─ [def456] Run tests
       → Blocks: abc123 (Deploy to prod)
    ```
  - **Effort**: 2 hours

- [ ] **2.2.6** CLI command: `klyntbot todo depend <id> --blocks <other_id>`
  - **Location**: `crates/cli/src/todo/depend.rs` (new file)
  - **Handler**: Call `TodoTool::execute()` with `action: "add_dependency"`
  - **Effort**: 1.5 hours

- [ ] **2.2.7** CLI command: `klyntbot todo depend <id> --remove <other_id>`
  - **Location**: `crates/cli/src/todo/depend.rs` (extend)
  - **Handler**: Call `TodoTool::execute()` with `action: "remove_dependency"`
  - **Effort**: 1 hour

#### Testing

- [ ] **2.T.1** Unit test: RRULE parsing (daily, weekly, monthly, complex rules)
- [ ] **2.T.2** Unit test: next_occurrence calculation
- [ ] **2.T.3** Integration test: Recurring task spawns instance at correct time
- [ ] **2.T.4** Integration test: Cannot complete task with incomplete blockers
- [ ] **2.T.5** Integration test: Completing blocker unblocks dependent task
- [ ] **2.T.6** Unit test: Dependency cycle detection

#### Acceptance Criteria

- ✅ `klyntbot todo recur add "Daily standup" --rule "FREQ=DAILY;BYHOUR=9"` creates template
- ✅ Background job spawns instance at 9am daily
- ✅ `klyntbot todo depend <id> --blocks <other>` creates bidirectional link
- ✅ Cannot complete task if blockers incomplete (error message lists blockers)
- ✅ `tree` action shows dependency arrows
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(todo): add recurring tasks (RRULE) and task dependencies`

**Impact**: +13 points (96 → 109/100 functionality — yes, this is that critical)

**Effort**: 23 hours

---

### Sprint 3: Bidirectional Calendar Sync (Week 5-6)

**Objective**: Calendar is single source of truth. Changes in calendar automatically sync to todos.

**Why**: Currently todo → calendar is one-way. Moving a calendar event doesn't update the todo. This breaks the "unified system" promise.

#### Tasks

- [ ] **3.1.1** Add CalendarHandler method: `get_event(uid: &str)`
  - **Location**: `crates/tools/src/calendar_tool.rs`
  - **Add to trait**:
    ```rust
    async fn get_event(&self, uid: &str) -> Result<CalendarEvent>;
    ```
  - **Struct**:
    ```rust
    pub struct CalendarEvent {
        pub uid: String,
        pub summary: String,
        pub start: DateTime<Utc>,
        pub end: DateTime<Utc>,
        pub status: String,  // CONFIRMED, CANCELLED, TENTATIVE
    }
    ```
  - **Effort**: 2 hours

- [ ] **3.1.2** Implement reconciliation engine
  - **Location**: `crates/agent/src/calendar_reconcile.rs` (new file)
  - **Function**:
    ```rust
    pub async fn reconcile_calendar_events(
        todo_store: Arc<RwLock<TodoStore>>,
        calendar_handler: Arc<dyn CalendarHandler>
    ) -> Result<ReconciliationReport> {
        // Get all todos with calendar_event_uid
        // For each: fetch event from calendar
        // Compare: start time, status
        // Sync changes back to todo
        // Return report (updated, deleted, errors)
    }
    ```
  - **Effort**: 4 hours

- [ ] **3.1.3** Sync logic: Event time changed
  - **Location**: `crates/agent/src/calendar_reconcile.rs`
  - **Behavior**:
    ```rust
    if event.start != todo.due_date {
        info!("Syncing time change: {} → {}", todo.due_date, event.start);
        store.update(&todo.id, TodoPatch {
            due_date: Some(Some(event.start)),
            ..Default::default()
        }).await?;
        report.updated.push(todo.id);
    }
    ```
  - **Effort**: 1 hour

- [ ] **3.1.4** Sync logic: Event completed
  - **Location**: `crates/agent/src/calendar_reconcile.rs`
  - **Behavior**:
    ```rust
    if event.status == "COMPLETED" && todo.status != TodoStatus::Done {
        info!("Calendar event completed, marking todo done: {}", todo.id);
        store.complete(&todo.id).await?;
        report.completed.push(todo.id);
    }
    ```
  - **Effort**: 1 hour

- [ ] **3.1.5** Sync logic: Event cancelled/deleted
  - **Location**: `crates/agent/src/calendar_reconcile.rs`
  - **Behavior**:
    ```rust
    if event.status == "CANCELLED" {
        info!("Calendar event cancelled, clearing UID: {}", todo.id);
        store.update(&todo.id, TodoPatch {
            calendar_event_uid: Some(None),
            ..Default::default()
        }).await?;
        report.unlinked.push(todo.id);
    }
    ```
  - **Effort**: 1 hour

- [ ] **3.1.6** Register cron job
  - **Location**: `crates/agent/src/agent_loop.rs`
  - **Change**: Register cron job (runs every 5 minutes)
  - **Job**: `reconcile_calendar_events()`
  - **Effort**: 0.5 hours

- [ ] **3.1.7** Add notification on sync
  - **Location**: `crates/agent/src/calendar_reconcile.rs`
  - **Behavior**: If changes detected, send notification via enabled channels
  - **Example**: "📅 Calendar synced: 2 tasks updated, 1 completed"
  - **Effort**: 2 hours

- [ ] **3.1.8** CLI command: `klyntbot calendar reconcile`
  - **Location**: `crates/cli/src/calendar.rs` (extend)
  - **Handler**: Manually trigger `reconcile_calendar_events()`
  - **Output**: Show report (updated, completed, unlinked)
  - **Effort**: 1.5 hours

- [ ] **3.1.9** Add config option: `calendar.bidirectional_sync`
  - **Location**: `crates/config/src/schema/core.rs`
  - **Add field**:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CalendarConfig {
        // ... existing fields ...
        #[serde(default = "default_true")]
        pub bidirectional_sync: bool,
    }
    ```
  - **Default**: `true`
  - **Effort**: 0.5 hours

#### Testing

- [ ] **3.T.1** Integration test: Event time changed → todo.due_date updates
- [ ] **3.T.2** Integration test: Event marked complete → todo status = Done
- [ ] **3.T.3** Integration test: Event cancelled → calendar_event_uid cleared
- [ ] **3.T.4** Unit test: Reconciliation report includes all changes
- [ ] **3.T.5** Integration test: Cron job runs every 5 minutes

#### Acceptance Criteria

- ✅ Move calendar event → wait 5 minutes → todo.due_date updates automatically
- ✅ Mark calendar event complete → todo status changes to Done
- ✅ Delete calendar event → todo.calendar_event_uid cleared
- ✅ `klyntbot calendar reconcile` manually triggers sync
- ✅ Notification sent when changes detected
- ✅ Config option `calendar.bidirectional_sync: false` disables feature
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(calendar): bidirectional sync — calendar changes update todos`

**Impact**: +4 points (109 → 113/100 integration completeness)

**Effort**: 13.5 hours

---

## Phase 2: Make the AI Intelligent (Weeks 7-12)

**Goal**: Leverage existing data (time tracking, completion patterns, calendar) to be proactive, not reactive.

**Impact**: +25 points (113 → 97/100 after normalization — the "AI that thinks" tier)

---

### Sprint 4: Smart Enrichment Engine (Week 7-8)

**Objective**: Auto-infer priority, predict duration, suggest due dates without user input.

**Why**: Currently the agent is a CRUD wrapper. The data exists (time_entries, completion timestamps) but isn't used for intelligence.

#### Tasks

- [ ] **4.1.1** Create enrichment engine
  - **Location**: `crates/agent/src/enrichment.rs` (new file)
  - **Struct**:
    ```rust
    pub struct TodoEnrichmentEngine {
        todo_store: Arc<RwLock<TodoStore>>,
        calendar_handler: Option<Arc<dyn CalendarHandler>>,
        provider: Arc<dyn LlmProvider>,  // For context-aware scoring
    }
    ```
  - **Effort**: 1 hour

- [ ] **4.1.2** Implement `infer_priority()`
  - **Location**: `crates/agent/src/enrichment.rs`
  - **Function**:
    ```rust
    pub async fn infer_priority(
        &self,
        title: &str,
        description: &str
    ) -> Option<u8> {
        // Keyword matching (urgent, ASAP, critical, blocker → 5)
        // Time-sensitive words (today, now, immediately → 4-5)
        // Optional: LLM call for context-aware scoring
    }
    ```
  - **Examples**:
    - "urgent: fix auth bug" → 5
    - "review PR when you have time" → 2
    - "ASAP: deploy hotfix" → 5
  - **Effort**: 3 hours

- [ ] **4.1.3** Implement `predict_duration()`
  - **Location**: `crates/agent/src/enrichment.rs`
  - **Function**:
    ```rust
    pub async fn predict_duration(
        &self,
        title: &str,
        tags: &[String],
        description: &str
    ) -> Option<u32> {
        // Search for similar completed tasks (cosine similarity on title)
        // Average their total_tracked_secs
        // Convert to minutes
        // Return estimate with confidence score
    }
    ```
  - **Effort**: 4 hours

- [ ] **4.1.4** Implement `suggest_due_date()`
  - **Location**: `crates/agent/src/enrichment.rs`
  - **Function**:
    ```rust
    pub async fn suggest_due_date(
        &self,
        estimated_minutes: u32,
        priority: u8,
        user_timezone: &str
    ) -> Option<DateTime<Utc>> {
        // Query calendar for free slots
        // High priority → suggest earliest slot
        // Low priority → suggest later in week
        // Return first suitable slot
    }
    ```
  - **Effort**: 4 hours

- [ ] **4.1.5** Hook into todo/SKILL.md
  - **Location**: `skills/todo/SKILL.md`
  - **Change**: Update yolo mode to call enrichment engine
  - **Flow**:
    ```
    User: "fix the auth bug urgently"
    → Agent extracts: title="Fix auth bug"
    → Call enrichment.infer_priority("fix the auth bug urgently", "")
    → priority=5
    → Call enrichment.predict_duration("fix the auth bug", ["backend"])
    → estimated_minutes=120
    → Call enrichment.suggest_due_date(120, 5, "America/Los_Angeles")
    → due_date=tomorrow 2pm
    → todo.add(title, priority=5, estimated_minutes=120, due_date)
    ```
  - **Effort**: 3 hours

- [ ] **4.1.6** Add config option: `todo.enrichment.enabled`
  - **Location**: `crates/config/src/schema/core.rs`
  - **Add**:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TodoConfig {
        // ... existing ...
        #[serde(default)]
        pub enrichment: TodoEnrichmentConfig,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TodoEnrichmentConfig {
        #[serde(default = "default_true")]
        pub enabled: bool,
        #[serde(default = "default_yolo_confidence")]
        pub yolo_confidence_threshold: f64,  // 0.0-1.0, default 0.85
    }
    ```
  - **Effort**: 1 hour

- [ ] **4.1.7** CLI command: `klyntbot todo enrich <id>`
  - **Location**: `crates/cli/src/todo/enrich.rs` (new file)
  - **Purpose**: Manually enrich existing task
  - **Behavior**:
    1. Fetch todo by ID
    2. Run enrichment engine
    3. Show suggested changes
    4. Prompt for confirmation
    5. Update todo
  - **Effort**: 2 hours

#### Testing

- [ ] **4.T.1** Unit test: Keyword priority inference (urgent → 5, low → 1)
- [ ] **4.T.2** Unit test: Duration prediction from historical data
- [ ] **4.T.3** Integration test: yolo mode auto-enriches task
- [ ] **4.T.4** Integration test: Enrichment respects confidence threshold

#### Acceptance Criteria

- ✅ `klyntbot chat "urgent: fix auth bug"` → auto-sets priority 5
- ✅ Duration prediction averages similar completed tasks (within 20% accuracy)
- ✅ Suggested due date respects calendar free slots
- ✅ yolo mode auto-applies enrichment if confidence > 0.85
- ✅ `klyntbot todo enrich <id>` manually enriches existing task
- ✅ Config option `todo.enrichment.enabled: false` disables feature
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(agent): smart enrichment — auto-infer priority, predict duration, suggest due dates`

**Impact**: +7 points (97 → 104/100 AI intelligence)

**Effort**: 18 hours

---

### Sprint 5: Semantic Search (Week 9-10)

**Objective**: `todo search --semantic "auth security"` finds tasks even if they say "login safety" or "authentication hardening".

**Why**: Full-text search is good for exact matches. Semantic search handles synonyms, related concepts, and intent.

#### Tasks

- [ ] **5.1.1** Add embedding generation dependency
  - **Location**: `Cargo.toml`
  - **Add**: `fastembed = "3.0"` (Rust port of sentence-transformers, runs locally)
  - **Alternative**: `llama-cpp-rs` if you want to use local LLM for embeddings
  - **Effort**: 0.5 hours

- [ ] **5.1.2** Create embedding utility
  - **Location**: `crates/tools/src/embeddings.rs` (new file)
  - **Struct**:
    ```rust
    pub struct EmbeddingEngine {
        model: fastembed::TextEmbedding,  // all-MiniLM-L6-v2 (384 dims, fast)
    }

    impl EmbeddingEngine {
        pub fn new() -> Result<Self>;
        pub fn embed(&self, text: &str) -> Result<Vec<f32>>;
        pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64;
    }
    ```
  - **Effort**: 3 hours

- [ ] **5.1.3** Add embedding fields to Todo
  - **Location**: `crates/tools/src/todo_types.rs`
  - **Add field**:
    ```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,  // 384-dim vector
    ```
  - **Note**: Skip serialization to keep JSONL readable
  - **Effort**: 0.5 hours

- [ ] **5.1.4** Generate embeddings on upsert
  - **Location**: `crates/tools/src/todo_store.rs`
  - **Change**: In `add()` and `update()` methods
  - **Behavior**:
    ```rust
    let text = format!("{} {} {}",
        todo.title,
        todo.description.unwrap_or_default(),
        todo.tags.join(" ")
    );
    todo.embedding = Some(embedding_engine.embed(&text)?);
    ```
  - **Effort**: 2 hours

- [ ] **5.1.5** Store embeddings separately
  - **Location**: `~/.klyntbot/todos_embeddings.jsonl` (new file)
  - **Format**: `{"id": "abc123", "embedding": [0.123, 0.456, ...]}`
  - **Reason**: Keep main JSONL human-readable
  - **Effort**: 2 hours

- [ ] **5.1.6** Implement semantic search
  - **Location**: `crates/tools/src/todo_store.rs`
  - **Add method**:
    ```rust
    pub async fn search_semantic(
        &mut self,
        query: &str,
        limit: usize
    ) -> Result<Vec<(Todo, f64)>> {  // (todo, similarity_score)
        let query_embedding = self.embedding_engine.embed(query)?;

        let mut results = self.index.values()
            .map(|todo| {
                let similarity = EmbeddingEngine::cosine_similarity(
                    &query_embedding,
                    todo.embedding.as_ref().unwrap()
                );
                (todo.clone(), similarity)
            })
            .collect::<Vec<_>>();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);
        Ok(results)
    }
    ```
  - **Effort**: 3 hours

- [ ] **5.1.7** Add TodoTool action: `search_semantic`
  - **Location**: `crates/tools/src/todo.rs`
  - **Action**: `"search_semantic"`
  - **Parameters**:
    ```json
    {
      "action": "search_semantic",
      "query": "authentication security issues",
      "limit": 10
    }
    ```
  - **Output**: List with similarity scores
  - **Effort**: 2 hours

- [ ] **5.1.8** Hybrid search mode
  - **Location**: `crates/tools/src/todo_store.rs`
  - **Add method**:
    ```rust
    pub async fn search_hybrid(
        &mut self,
        query: &str,
        limit: usize
    ) -> Result<Vec<Todo>> {
        // Run both full-text and semantic search
        // Merge results with reciprocal rank fusion (RRF)
        // Return top N
    }
    ```
  - **Effort**: 3 hours

- [ ] **5.1.9** CLI command: `klyntbot todo search --semantic <query>`
  - **Location**: `crates/cli/src/todo/search.rs` (extend from Sprint 1)
  - **Add flag**: `--semantic`
  - **Handler**: Call `TodoTool::execute()` with `action: "search_semantic"`
  - **Output**: Show similarity scores
  - **Effort**: 1.5 hours

- [ ] **5.1.10** Background job: generate missing embeddings
  - **Location**: `crates/agent/src/embedding_backfill.rs` (new file)
  - **Purpose**: Generate embeddings for existing todos (one-time migration)
  - **Behavior**: Iterate all todos, generate embeddings if missing
  - **Effort**: 2 hours

#### Testing

- [ ] **5.T.1** Unit test: Embedding generation produces 384-dim vector
- [ ] **5.T.2** Unit test: Cosine similarity calculation
- [ ] **5.T.3** Integration test: Semantic search finds "login security" when querying "auth safety"
- [ ] **5.T.4** Integration test: Hybrid search merges full-text + semantic
- [ ] **5.T.5** Performance test: Semantic search on 1000 todos < 500ms

#### Acceptance Criteria

- ✅ `klyntbot todo search --semantic "authentication"` finds tasks with "login", "auth", "security"
- ✅ Embeddings generated automatically on todo.add/update
- ✅ Embeddings stored separately (todos_embeddings.jsonl)
- ✅ Hybrid search combines full-text + semantic (RRF)
- ✅ Migration script backfills embeddings for existing todos
- ✅ Model runs locally (no API calls)
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(search): semantic search with local embeddings (fastembed)`

**Impact**: +6 points (104 → 110/100 search quality)

**Effort**: 19.5 hours

---

### Sprint 6: Daily Planning Skill (Week 11-12)

**Objective**: Every morning, agent auto-plans the day and asks for confirmation.

**Why**: Most powerful use of AI — proactive planning instead of reactive task execution.

#### Tasks

- [ ] **6.1.1** Create daily planning skill
  - **Location**: `skills/daily-planning/SKILL.md` (new file)
  - **Triggers**: Cron at `config.todo.notifications.daily_digest_time`
  - **Purpose**: Analyze tasks, calendar, and suggest focus order
  - **Effort**: 2 hours

- [ ] **6.1.2** Implement planning logic
  - **Location**: `crates/agent/src/daily_planning.rs` (new file)
  - **Function**:
    ```rust
    pub async fn generate_daily_plan(
        todo_store: Arc<RwLock<TodoStore>>,
        calendar_handler: Arc<dyn CalendarHandler>,
        config: &Config
    ) -> Result<DailyPlan> {
        // Get overdue tasks (high urgency)
        // Get high-priority tasks
        // Get calendar events for today
        // Calculate available focus slots
        // Score tasks by (priority × urgency × estimated_fit)
        // Return suggested focus order
    }
    ```
  - **Effort**: 4 hours

- [ ] **6.1.3** Define DailyPlan struct
  - **Location**: `crates/agent/src/daily_planning.rs`
  - **Struct**:
    ```rust
    pub struct DailyPlan {
        pub date: DateTime<Utc>,
        pub available_slots: usize,  // Based on max_focus_slots
        pub suggested_tasks: Vec<PlannedTask>,
        pub deferred_tasks: Vec<String>,  // Low-priority → suggest archive
        pub reasoning: String,  // Why this order
    }

    pub struct PlannedTask {
        pub todo: Todo,
        pub score: f64,
        pub reason: String,  // "Overdue + high priority"
    }
    ```
  - **Effort**: 1 hour

- [ ] **6.1.4** Scoring algorithm
  - **Location**: `crates/agent/src/daily_planning.rs`
  - **Formula**:
    ```rust
    fn score_task(todo: &Todo, now: DateTime<Utc>) -> f64 {
        let urgency = if let Some(due) = todo.due_date {
            let days_until = (due - now).num_days();
            if days_until < 0 { 10.0 }       // Overdue
            else if days_until == 0 { 5.0 }  // Due today
            else if days_until == 1 { 3.0 }  // Due tomorrow
            else { 1.0 }
        } else { 0.5 };

        let priority = todo.priority.unwrap_or(1) as f64;
        let age = (now - todo.created_at).num_days() as f64;

        (urgency * priority) + (age * 0.1)
    }
    ```
  - **Effort**: 2 hours

- [ ] **6.1.5** Send notification with plan
  - **Location**: `crates/agent/src/daily_planning.rs`
  - **Behavior**:
    ```rust
    let message = format!(
        "Good morning! Here's your plan:\n\
         1. {} (P{}, {})\n\
         2. {} (P{}, {})\n\
         3. {} (P{}, {})\n\n\
         Reply 'yes' to auto-focus, or 'swap 1 and 2' to reorder.",
        plan.suggested_tasks[0].todo.title,
        plan.suggested_tasks[0].todo.priority.unwrap(),
        plan.suggested_tasks[0].reason,
        // ...
    );
    self.send_notification(message).await?;
    ```
  - **Effort**: 2 hours

- [ ] **6.1.6** Handle user response
  - **Location**: `crates/agent/src/daily_planning.rs`
  - **Behavior**:
    - "yes" → auto-focus all suggested tasks
    - "swap 1 and 2" → reorder and focus
    - "skip 3" → focus only 1 and 2
    - "defer all" → archive suggested tasks
  - **Parsing**: Simple regex or LLM call to extract intent
  - **Effort**: 3 hours

- [ ] **6.1.7** Register cron job
  - **Location**: `crates/agent/src/agent_loop.rs`
  - **Change**: Register daily cron at digest time
  - **Job**: `generate_daily_plan()` → send notification → wait for response
  - **Effort**: 1 hour

- [ ] **6.1.8** CLI command: `klyntbot todo plan`
  - **Location**: `crates/cli/src/todo/plan.rs` (new file)
  - **Purpose**: Manually trigger daily planning (don't wait for cron)
  - **Handler**: Call `generate_daily_plan()`
  - **Output**: Show suggested order, prompt for confirmation
  - **Effort**: 2 hours

- [ ] **6.1.9** Add config option: `todo.daily_planning.enabled`
  - **Location**: `crates/config/src/schema/core.rs`
  - **Add**:
    ```rust
    #[serde(default = "default_true")]
    pub daily_planning: bool,
    ```
  - **Effort**: 0.5 hours

#### Testing

- [ ] **6.T.1** Unit test: Task scoring algorithm (overdue > today > tomorrow)
- [ ] **6.T.2** Integration test: Daily plan suggests top 3 tasks
- [ ] **6.T.3** Integration test: User response "yes" auto-focuses tasks
- [ ] **6.T.4** Integration test: User response "swap 1 and 2" reorders

#### Acceptance Criteria

- ✅ Cron job runs daily at digest time (e.g., 9am)
- ✅ Notification shows suggested focus order with reasoning
- ✅ User can reply "yes", "swap X and Y", "skip N", "defer all"
- ✅ Agent auto-focuses tasks on confirmation
- ✅ `klyntbot todo plan` manually triggers planning
- ✅ Config option `todo.daily_planning: false` disables feature
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(agent): daily planning skill — proactive morning focus suggestions`

**Impact**: +5 points (110 → 115/100 AI proactivity)

**Effort**: 17.5 hours

---

## Phase 3: Daily-Driver Polish (Weeks 13-14)

**Goal**: Multi-device sync, backup, and reliability for real-world daily use.

**Impact**: +4 points (115 → 97/100 after normalization — "I can't live without this" tier)

---

### Sprint 7: Git Sync & Multi-Device (Week 13-14)

**Objective**: All data syncs across machines via encrypted Git. Works offline, syncs on reconnect.

**Why**: Local-first is great, but daily-driver tools need multi-device support (laptop, phone, desktop).

#### Tasks

- [ ] **7.1.1** Add encryption dependency
  - **Location**: `Cargo.toml`
  - **Add**: `age = "0.10"` (modern encryption, simple API)
  - **Purpose**: Encrypt JSONL files before pushing to Git
  - **Effort**: 0.5 hours

- [ ] **7.1.2** Create sync engine
  - **Location**: `crates/sync/src/lib.rs` (new crate)
  - **Struct**:
    ```rust
    pub struct SyncEngine {
        data_dir: PathBuf,           // ~/.klyntbot/
        repo_url: String,            // git@github.com:user/klyntbot-data.git
        public_key: String,          // age public key for encryption
        secret_key: Option<String>,  // age secret key for decryption
    }
    ```
  - **Effort**: 2 hours

- [ ] **7.1.3** Implement `init_sync()`
  - **Location**: `crates/sync/src/lib.rs`
  - **Function**:
    ```rust
    pub async fn init_sync(
        repo_url: &str,
        public_key: &str
    ) -> Result<()> {
        // Clone repo to ~/.klyntbot/.sync/
        // Create .gitignore (ignore *.jsonl, only track *.enc)
        // Save keys to config
    }
    ```
  - **Effort**: 2 hours

- [ ] **7.1.4** Implement `encrypt_and_commit()`
  - **Location**: `crates/sync/src/lib.rs`
  - **Function**:
    ```rust
    pub async fn encrypt_and_commit(&self) -> Result<()> {
        // For each *.jsonl in ~/.klyntbot/
        // Encrypt with age public key
        // Write to .sync/*.jsonl.enc
        // Git add + commit with timestamp
        // Debounce: only commit if > 30s since last commit
    }
    ```
  - **Effort**: 3 hours

- [ ] **7.1.5** Implement `pull_and_decrypt()`
  - **Location**: `crates/sync/src/lib.rs`
  - **Function**:
    ```rust
    pub async fn pull_and_decrypt(&self) -> Result<()> {
        // Git pull from remote
        // For each *.jsonl.enc in .sync/
        // Decrypt with age secret key
        // Write to ~/.klyntbot/*.jsonl
        // Handle conflicts: merge journal entries, newest wins
    }
    ```
  - **Effort**: 3 hours

- [ ] **7.1.6** Implement conflict resolution
  - **Location**: `crates/sync/src/conflict.rs` (new file)
  - **Strategy**:
    ```rust
    pub fn merge_jsonl(
        local: Vec<JournalEntry>,
        remote: Vec<JournalEntry>
    ) -> Vec<JournalEntry> {
        // Merge by timestamp (append-only log)
        // Deduplicate by entry ID
        // Remote upsert wins over local if same ID
        // Return merged log
    }
    ```
  - **Effort**: 3 hours

- [ ] **7.1.7** Auto-sync on mutation
  - **Location**: `crates/tools/src/todo_store.rs`, `project_store.rs`
  - **Change**: After every `append_entry()`, trigger `encrypt_and_commit()`
  - **Debounce**: Use `tokio::time::sleep(30s)` to batch commits
  - **Effort**: 2 hours

- [ ] **7.1.8** Auto-pull on startup
  - **Location**: `crates/agent/src/agent_loop.rs`
  - **Change**: On `AgentLoop::new()`, call `pull_and_decrypt()`
  - **Behavior**: Sync latest state before first message
  - **Effort**: 1 hour

- [ ] **7.1.9** CLI command: `klyntbot sync init <repo_url>`
  - **Location**: `crates/cli/src/sync.rs` (new file)
  - **Behavior**:
    1. Prompt for age public key (or generate new keypair)
    2. Clone repo
    3. Save config
  - **Effort**: 2 hours

- [ ] **7.1.10** CLI command: `klyntbot sync push|pull|status`
  - **Location**: `crates/cli/src/sync.rs` (extend)
  - **Subcommands**:
    - `push` → encrypt + commit + push
    - `pull` → pull + decrypt + merge
    - `status` → show last sync time, pending changes
  - **Effort**: 2 hours

- [ ] **7.1.11** Add config fields
  - **Location**: `crates/config/src/schema/core.rs`
  - **Add**:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SyncConfig {
        #[serde(default)]
        pub enabled: bool,
        pub repo_url: Option<String>,
        pub public_key: Option<String>,
        pub secret_key: Option<Secret<String>>,
        #[serde(default = "default_auto_sync")]
        pub auto_sync: bool,  // Auto push/pull
    }
    ```
  - **Effort**: 1 hour

#### Testing

- [ ] **7.T.1** Integration test: Encrypt → decrypt round-trip preserves data
- [ ] **7.T.2** Integration test: Conflict resolution merges journal entries correctly
- [ ] **7.T.3** Integration test: Auto-sync commits after mutation (debounced)
- [ ] **7.T.4** Integration test: Pull on startup syncs latest state
- [ ] **7.T.5** Security test: Encrypted files are unreadable without secret key

#### Acceptance Criteria

- ✅ `klyntbot sync init git@github.com:user/data.git` sets up sync
- ✅ Mutations auto-commit (debounced 30s)
- ✅ `klyntbot sync push` manually pushes encrypted data
- ✅ `klyntbot sync pull` merges remote changes
- ✅ Conflict resolution handles concurrent edits (newest wins)
- ✅ Encrypted files use age (modern, audited)
- ✅ Works offline (queue commits, sync on reconnect)
- ✅ Zero clippy warnings

#### Deliverable

**PR Title**: `feat(sync): multi-device encrypted Git sync with age encryption`

**Impact**: +4 points (115 → 97/100 after normalization — daily-driver ready)

**Effort**: 21.5 hours

---

## Phase 4: Moonshot Features (Future)

**Goal**: Build the stuff that makes this the reference implementation.

**Impact**: +20 points (takes it from "best" to "magical")

**Timeline**: 2-3 months (pick based on user feedback after Phase 1-3)

### 4.1: Memory-Augmented Task Retrieval

**Effort**: 1-2 weeks

**What**: Store conversation history in vector DB. When user says "that thing we talked about yesterday", retrieve relevant tasks + chat snippets.

**How**:
- Add `qdrant` or `chroma` for local vector DB
- Embed every assistant/user message
- On query, search both todos + chat history
- Return unified results

**Impact**: +5 points

---

### 4.2: Auto-Capture from Everywhere

**Effort**: 2-3 weeks

**What**: Browser extension, Telegram/Discord/Slack bots that forward to agent.

**How**:
- Browser extension: highlight text → "Add to klyntbot"
- Platform bots: forward message → parse → create todo
- Unified inbox: all sources → same agent loop

**Impact**: +6 points

---

### 4.3: Habit Tracking

**Effort**: 1 week

**What**: Track streaks, correlate with energy/productivity.

**How**:
- Add `habit` field to Todo (boolean, daily check-in)
- Track streak (days consecutive completion)
- Analyze correlation (high energy after deep work in morning)
- Show in daily digest

**Impact**: +3 points

---

### 4.4: Projects v2 (Kanban, Phases, Custom Fields)

**Effort**: 2 weeks

**What**: Projects become full-featured (like Notion databases).

**How**:
- Add `phases: Vec<Phase>` (e.g., "Backlog", "In Progress", "Review", "Done")
- Add `custom_fields: HashMap<String, Value>` (arbitrary metadata)
- Kanban view in CLI (columns for phases)
- Drag-drop in TUI (using `ratatui`)

**Impact**: +4 points

---

### 4.5: Dependency Graph Visualization

**Effort**: 1 week

**What**: Generate Graphviz/Mermaid diagram of task dependencies.

**How**:
- Traverse `blocked_by`/`blocks` fields
- Output `.dot` or Mermaid syntax
- Render with `graphviz` or show in browser

**Impact**: +2 points

---

## Appendix: Deferred Ideas

These are good ideas but deprioritized for now:

### A.1: Pomodoro Integration

**Why Deferred**: Niche feature, low ROI compared to other Phase 3 work.

**What**: `klyntbot todo focus --pomodoro` → 25/5 cycles with OS notifications.

**Effort**: 1-2 days

**Priority**: P3

---

### A.2: Smart Tag Suggestions

**Why Deferred**: Enrichment engine (Sprint 4) already covers most value.

**What**: Suggest tags based on co-occurrence patterns.

**Effort**: 1 day

**Priority**: P3

---

### A.3: Task Templates

**Why Deferred**: Less urgent than recurring tasks.

**What**: Store pre-defined hierarchies as JSONL templates.

**Effort**: 2 days

**Priority**: P3

---

### A.4: Context-Aware Notifications

**Why Deferred**: Requires OS location APIs, complex setup.

**What**: Suppress notifications when in meetings or away.

**Effort**: 3-4 days

**Priority**: P2

---

### A.5: Focus Session Analytics

**Why Deferred**: Nice-to-have, not critical.

**What**: Track flow state metrics (sessions/day, duration, productivity).

**Effort**: 2 days

**Priority**: P3

---

## Summary Timeline

| Phase | Weeks | Effort | Impact | Deliverable |
|-------|------:|-------:|-------:|-------------|
| **Phase 1** | 1-6 | 62 hours | +31 pts | CLI parity, recurring tasks, dependencies, bidirectional calendar sync |
| **Phase 2** | 7-12 | 55 hours | +25 pts | Smart enrichment, semantic search, daily planning |
| **Phase 3** | 13-14 | 21.5 hours | +4 pts | Git sync, multi-device support |
| **Phase 4** | Future | TBD | +20 pts | Memory retrieval, auto-capture, habits, projects v2, graph viz |
| **Total** | 14 weeks | 138.5 hours | +80 pts | 94 → 99/100 (after normalization) |

---

## Success Metrics

| Metric | Baseline | Sprint 3 | Sprint 6 | Sprint 7 |
|--------|----------|----------|----------|----------|
| CLI action coverage | 47% (8/17) | 100% (17/17) | 100% | 100% |
| Recurring task requests | High | Zero | Zero | Zero |
| Dependency requests | High | Zero | Zero | Zero |
| Auto-enrichment rate | 0% | 0% | 80%+ | 80%+ |
| Semantic search P@5 | N/A | N/A | 90%+ | 90%+ |
| Daily planning adoption | 0% | 0% | 70%+ | 70%+ |
| Multi-device setup time | N/A | N/A | N/A | <5 min |
| Net Promoter Score | Unknown | 40+ | 60+ | 70+ |

---

## Next Steps

1. **Review this plan** with stakeholders
2. **Prioritize sprints** based on user feedback
3. **Start with Sprint 1** (CLI parity) — highest ROI, lowest risk
4. **Ship incrementally** — merge PRs after each sprint
5. **Gather feedback** after Phase 1 — adjust roadmap

---

**Questions? Feedback?**

Open an issue or PR to discuss this roadmap.
