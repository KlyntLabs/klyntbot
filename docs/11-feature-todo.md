# feature-todo Crate

Self-contained task management feature package for Klyntbot. Provides the `TodoTool` (26 actions), domain types, enrichment engine, search system (keyword/semantic/hybrid), calendar sync, recurring tasks, task complexity analysis, and all associated handler traits for dependency inversion.

**Crate root:** `crates/feature-todo/`
**Layer:** 3 (depends on `common`, `storage`, `tools-core`)

---

## Section 1: Narrative Overview

### TodoTool

`TodoTool` is the primary tool implementation, registered under the name `"todo"`. It implements the `tools_core::Tool` trait and dispatches to 26 actions through a single `execute()` entry point. The action is selected by the `"action"` parameter in the JSON arguments.

**File:** `src/tool/mod.rs` (L17-303)

The tool holds:
- A `TodoRepo` for all database operations
- Optional handler trait objects (`Arc<dyn T>`) for calendar sync, enrichment, embedding, and feedback
- A `VectorStore` for LanceDB semantic search
- Configuration values: `max_focus_slots`, `focus_deadline_hours`, `timezone`, `semantic_threshold`, `rrf_k`, `enrichment_threshold`

Construction follows a builder pattern: `TodoTool::new(repo, slots, hours, tz)` returns a base instance, then `.with_calendar_handler()`, `.with_enrichment_handler()`, `.with_embedding_handler()`, `.with_embedding_store()`, `.with_search_config()`, `.with_enrichment_threshold()`, and `.with_feedback_handler()` attach optional capabilities.

Actions are organized across six files in `src/tool/actions/`:

| File | Actions |
|------|---------|
| `add.rs` | `add`, `add_subtask`, `recur` |
| `delete.rs` | `delete`, `delete_recurring` |
| `deps.rs` | `add_dependency`, `remove_dependency` |
| `execute.rs` | `execute` |
| `list.rs` | `list`, `show`, `summary`, `tree`, `report`, `plan`, `list_recurring` |
| `search.rs` | `search`, `search_semantic`, `search_hybrid` |
| `update.rs` | `update`, `complete`, `enrich`, `focus`, `unfocus`, `move`, `attach`, `detach`, `log_time` |

**Scoring helpers** (`src/tool/mod.rs` L96-145) compute a composite urgency/priority/age score used by the `plan` and `list` actions to rank tasks:
- `calculate_urgency()`: overdue=10, today=5, tomorrow=3, else=1, no date=1
- `priority_weight()`: P1=5, P2=4, P3=3, P4=2, P5=1, None=3
- `calculate_score()`: `(urgency * priority_weight) + (age_days * 0.1)`

**Full todo loading** (`load_full_todo`, `get_full_todo`) enriches a raw `TodoRow` with attachments, time entries, and dependency edges (blocked_by, blocks) via concurrent `tokio::try_join!` queries.

### TodoFeature (FeaturePackage Implementation)

`TodoFeature` wraps a `TodoTool` and implements the `tools_core::FeaturePackage` trait, making it a self-registering feature that provides tools, migrations, config defaults, and a health check.

**File:** `src/lib.rs` (L42-95)

- `name()` returns `"todo"`
- `tools()` returns a single-element vec containing the `TodoTool`
- `migrations()` returns the v1 migration SQL (`migrations/001_create_todos.sql`) which creates tables: `todos`, `todo_attachments`, `todo_time_entries`, `todo_dependencies` with appropriate indexes
- `config_key()` returns `"todo"`
- `default_config()` serializes `TodoConfig::default()`
- `health_check()` calls `repo.summary()` and returns `Healthy` or `Unhealthy`

### Enrichment Engine

The enrichment system auto-infers missing task fields using keyword analysis. It operates through the `EnrichmentHandler` trait for dependency inversion -- the actual `EnrichmentEngine` lives in the `agent` crate (Layer 5).

**File:** `src/enrichment.rs` (L1-109)

The core types are:

- `EnrichmentSuggestion<T>` -- a single suggestion with `value: T`, `confidence: f64` (0.0-1.0), and `reasoning: String`
- `EnrichmentResult` -- aggregates optional suggestions for `priority` (u8), `estimated_minutes` (u32), and `due_date` (DateTime<Utc>)
- `EnrichmentHandler` (trait) -- `async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>`

**How enrichment integrates with TodoTool:**

1. On `add` action (`src/tool/actions/add.rs` L36-88): if an `EnrichmentHandler` is attached, the newly created task is passed to `enrich_task()`. Suggestions above the `enrichment_threshold` (default 0.70) are auto-applied via a `TodoPatch` update. Feedback is recorded.
2. On `enrich` action (`src/tool/actions/update.rs` L147-230): explicit enrichment of an existing task. All suggestions are applied regardless of threshold, and detailed reasoning is returned.

**Feedback recording** (`src/handler.rs` L29-76) iterates over the three suggestion fields (priority, estimated_minutes, due_date) and records `EnrichmentFeedbackEntry` entries via the `EnrichmentFeedbackHandler` trait. When called from `add`, acceptance is determined per-field by comparing confidence against 0.7. When called from `enrich`, all fields are marked accepted.

### Search System

Three search modes are available, all invoked as TodoTool actions.

**File:** `src/tool/actions/search.rs` (L1-195), `src/search.rs` (L1-76)

**Keyword search** (`search` action): Delegates to `TodoRepo::search_by_keyword()`, which performs SQL substring matching on title and description. Returns matching `Todo` items.

**Semantic search** (`search_semantic` action): Requires both an `EmbeddingHandler` and a `VectorStore`. The query string is embedded via `embed_query()`, then LanceDB approximate nearest neighbor (ANN) search is run against the `"todo_embeddings"` table. Results are filtered by a cosine similarity threshold (configurable via the `threshold` parameter or `self.semantic_threshold`). If fewer tasks have embeddings than exist in the database, a note is appended.

**Hybrid search** (`search_hybrid` action): Runs keyword search and semantic search concurrently, then merges results via Reciprocal Rank Fusion (RRF). The `hybrid_merge()` function in `src/search.rs` delegates to `tools_core::rrf_merge()`, which:
1. Assigns rank-based scores (`1 / (k + rank)`) to keyword results
2. Assigns rank-based scores to semantic results (sorted by similarity)
3. Sums scores for items appearing in both lists
4. Tags each result with its source: `"keyword"`, `"semantic"`, or `"both"`

The RRF k parameter defaults to 60 (configurable via `SearchConfig::rrf_k`).

### Calendar Sync

Bidirectional todo-to-calendar synchronization via the `CalendarSyncHandler` trait.

**File:** `src/calendar_sync.rs` (L1-57)

The trait defines three methods:
- `sync_calendar()` -- trigger a full bidirectional sync
- `push_task(task_id)` -- push a single task to the calendar
- `remove_event(calendar_event_uid)` -- remove a calendar event by UID

The `TodoTool` calls these through convenience methods in `src/handler.rs` (L11-27):
- `push_task_to_calendar()` is called after `add`, `update`, `complete`, `move`, and `enrich`
- `remove_event_from_calendar()` is called after `delete` when the task had a `calendar_event_uid`

All calendar operations are best-effort: failures are logged via `tracing::warn` but do not fail the tool action.

**TodoCalendarSyncAdapter** (`crates/agent/src/todo_calendar_sync_adapter.rs`): The `CalendarSyncHandler` trait (defined in `feature-todo`) has a narrower interface than the `CalendarHandler` trait (defined in `tools`). The `TodoCalendarSyncAdapter` bridges these two traits -- it wraps an `Arc<dyn CalendarHandler>` and implements `CalendarSyncHandler` by delegating each method to the corresponding `CalendarHandler` method, discarding the `Value` return type where the narrower trait expects `()`. This avoids a circular dependency between the `feature-todo` crate (Layer 3) and the `agent` crate (Layer 5), where the actual `CalendarHandler` implementation lives. The three methods mapped are: `sync_calendar()` to `CalendarHandler::sync_calendar()`, `push_task(task_id)` to `CalendarHandler::push_single_task(task_id)`, and `remove_event(uid)` to `CalendarHandler::remove_single_event(uid)`.

### Embedding

Task embeddings are generated and stored via the `EmbeddingHandler` trait.

**File:** `src/embedding.rs` (L1-49)

Two methods:
- `embed_todo(todo)` -- generate and persist an embedding for a task
- `embed_query(query)` -- embed a query string, returning a `Vec<f32>` for similarity search

The `TodoTool` auto-embeds tasks on `add` (after creation) and `update` (after patch). Embedding failures are logged but do not fail the action. The actual embedding engine (fastembed, `paraphrase-multilingual-MiniLM-L12-v2`, 384 dimensions) is implemented in the `agent` crate and injected via `Arc<dyn EmbeddingHandler>`.

### Recurring Tasks (rrule_utils)

Utility module wrapping the `rrule` crate for iCalendar RRULE support.

**File:** `src/rrule_utils.rs` (L1-379)

**Supported parameters:** FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY, COUNT, UNTIL, EXDATE.
**Rejected parameters:** BYSETPOS, WKST, EXRULE, RDATE.
**Supported frequencies:** DAILY, WEEKLY, MONTHLY, YEARLY.

Key functions:
- `validate_rrule(rule)` -- parse and validate an RRULE string, rejecting unsupported parameters
- `next_occurrence(rule, after)` -- compute the next occurrence after a given datetime
- `should_spawn_instance(next_date, now)` -- check if a recurring task instance should be spawned (true when `next_date <= now`)
- `humanize_rrule(rule)` -- convert to human-readable description (e.g., `"Every week on Monday, Wednesday, Friday"`)

The `recur` action creates a template task (`is_template = true`) with a `recurrence_rule` and pre-computed `next_instance_date`. The `list_recurring` action lists all templates. The `delete_recurring` action deletes a template (existing instances become standalone tasks).

### Task Complexity Analysis

Evaluates whether a task warrants plan-based execution based on structural signals.

**File:** `src/task_complexity.rs` (L1-177)

`TaskComplexitySignals` holds five fields and computes a `complexity_score()` (0-6) as:

| Signal | Condition | Score |
|--------|-----------|-------|
| `has_dependencies` | true | +2 |
| `subtask_count` | >= 3 | +1 |
| `dependency_depth` | >= 2 | +1 |
| `estimated_minutes` | >= 60 | +1 |
| `priority` | <= 2 | +1 |

`is_plan_worthy(threshold)` returns true when `complexity_score() >= threshold`. The `execute` action uses threshold 3.

`evaluate_task_complexity(repo, task_id)` queries the database for subtask count, dependency existence, dependency depth (recursive with max depth 5), estimated minutes, and priority, returning a populated `TaskComplexitySignals`.

The `execute` action (`src/tool/actions/execute.rs`) uses this: if plan-worthy, it suggests creating a plan; otherwise, it marks the task as `"doing"`.

### Handler Traits

Four handler traits provide dependency inversion, defined at Layer 3 so `TodoTool` can call them without depending on the agent crate (Layer 5):

| Trait | File | Methods |
|-------|------|---------|
| `CalendarSyncHandler` | `src/calendar_sync.rs:12` | `sync_calendar()`, `push_task(task_id)`, `remove_event(uid)` |
| `EmbeddingHandler` | `src/embedding.rs:14` | `embed_todo(todo)`, `embed_query(query)` |
| `EnrichmentHandler` | `src/enrichment.rs:41` | `enrich_task(task)` |
| `EnrichmentFeedbackHandler` | `src/enrichment.rs:61` | `record_feedback(entry)` |

All traits are `Send + Sync` and `#[async_trait]`. They are injected as `Option<Arc<dyn Trait>>` into `TodoTool` at construction.

### Configuration

**File:** `src/config.rs` (L1-118)

Three config structs, all using `#[serde(rename_all = "camelCase")]`:

**TodoConfig** -- top-level `"todo"` config section:
- `max_focus_slots: usize` (default: 3) -- max simultaneously focused tasks
- `focus_deadline_hours: u64` (default: 8) -- hours before focus expires
- `timezone: String` (default: `"UTC"`) -- user's local timezone
- `enrichment: EnrichmentConfig` -- nested enrichment config
- `search: SearchConfig` -- nested search config

**EnrichmentConfig:**
- `enabled: bool` (default: true)
- `auto_apply_threshold: f64` (default: 0.70)

**SearchConfig:**
- `enabled: bool` (default: true)
- `semantic_threshold: f64` (default: 0.5)
- `embedding_model: String` (default: `"paraphrase-multilingual-MiniLM-L12-v2"`)
- `rrf_k: u32` (default: 60)

---

## Section 2: API Reference

### TodoTool

**File:** `src/tool/mod.rs` (L18-303)

| Field | Type | Description |
|-------|------|-------------|
| `repo` | `TodoRepo` | Database repository for all todo CRUD |
| `max_focus_slots` | `usize` | Max concurrent focused tasks |
| `focus_deadline_hours` | `u64` | Hours before focus expires |
| `timezone` | `String` | User timezone for date parsing |
| `calendar_handler` | `Option<Arc<dyn CalendarSyncHandler>>` | Calendar sync |
| `enrichment_handler` | `Option<Arc<dyn EnrichmentHandler>>` | Task enrichment |
| `embedding_handler` | `Option<Arc<dyn EmbeddingHandler>>` | Embedding generation |
| `embedding_store` | `Option<storage::VectorStore>` | LanceDB vector store |
| `semantic_threshold` | `f64` | Min cosine similarity for semantic search |
| `rrf_k` | `u32` | RRF k parameter for hybrid search |
| `feedback_handler` | `Option<Arc<dyn EnrichmentFeedbackHandler>>` | Feedback recording |
| `enrichment_threshold` | `f64` | Confidence threshold for auto-applying enrichment |

#### Constructor and Builder Methods

```rust
fn new(repo: TodoRepo, max_focus_slots: usize, focus_deadline_hours: u64, timezone: String) -> Self
fn with_calendar_handler(self, handler: Arc<dyn CalendarSyncHandler>) -> Self
fn with_enrichment_handler(self, handler: Arc<dyn EnrichmentHandler>) -> Self
fn with_embedding_handler(self, handler: Arc<dyn EmbeddingHandler>) -> Self
fn with_embedding_store(self, store: storage::VectorStore) -> Self
fn with_search_config(self, threshold: f64, rrf_k: u32) -> Self
fn with_enrichment_threshold(self, threshold: f64) -> Self
fn with_feedback_handler(self, handler: Arc<dyn EnrichmentFeedbackHandler>) -> Self
```

#### Tool Trait Implementation

- `name()` -> `"todo"`
- `description()` -> lists all 26 actions
- `parameters()` -> JSON schema with `"action"` as the only required field
- `execute(args, ctx)` -> dispatches to action handler

#### Actions

| Action | Required Params | Optional Params | Returns | File:Line |
|--------|----------------|-----------------|---------|-----------|
| `add` | `title` | `description`, `priority`, `due_date`, `tags`, `project_id` | `"Task created: {title} (ID: {id})"` + enrichment info | `add.rs:13` |
| `add_subtask` | `title`, `parent_id` | `description`, `priority`, `due_date`, `tags`, `project_id` | `"Subtask created: {title} (ID: {id}, parent: {pid})"` | `add.rs:138` |
| `recur` | `title`, `rule` | `description`, `priority`, `tags`, `project_id` | `"Recurring template created: ..."` | `add.rs:170` |
| `list` | -- | `status`, `priority_min`, `tag`, `limit`, `project_id` | Formatted task list | `list.rs:15` |
| `show` | `id` | -- | Detailed task view with all fields | `list.rs:58` |
| `summary` | -- | -- | Status counts + overdue count | `list.rs:104` |
| `tree` | -- | -- | ASCII tree of all tasks with hierarchy | `list.rs:118` |
| `report` | `period` (`"week"`/`"month"`) | `project_id` | Per-project stats: completed, created, time tracked, focus sessions, overdue | `list.rs:181` |
| `plan` | -- | `count` (default: 3, max: 10) | Ranked daily plan of top eligible tasks | `list.rs:295` |
| `list_recurring` | -- | -- | All recurring templates with schedule info | `list.rs:386` |
| `update` | `id` | `title`, `description`, `priority`, `due_date`, `tags`, `status`, `estimated_minutes` | `"Updated task: {title}"` | `update.rs:13` |
| `complete` | `id` | -- | `"Completed: {title}"` + cascade info + unblocked tasks | `update.rs:62` |
| `enrich` | `id` | -- | Applied enrichment suggestions with reasoning | `update.rs:147` |
| `focus` | `id` | -- | `"Focused on task: {id}"` | `update.rs:232` |
| `unfocus` | `id` | -- | `"Unfocused task: {id}"` | `update.rs:258` |
| `move` | `id` | `new_parent_id`, `new_project_id` | `"Moved task: {title}"` | `update.rs:276` |
| `attach` | `id`, `attachment_type`, `value` | `attachment_title` | `"Attachment added to task: {id}"` | `update.rs:320` |
| `detach` | `id`, `attachment_id` | -- | `"Attachment {aid} removed from task: {id}"` | `update.rs:342` |
| `log_time` | `id`, `duration_minutes` | `note` | `"Logged {n} minutes to task: {id}"` | `update.rs:359` |
| `search` | `query` | `limit` | Keyword-matched task list | `search.rs:11` |
| `search_semantic` | `query` | `limit`, `threshold` | Semantically similar tasks with similarity scores | `search.rs:39` |
| `search_hybrid` | `query` | `limit` | RRF-merged keyword+semantic results with source tags | `search.rs:134` |
| `delete` | `id` | -- | `"Deleted task: {id}"` | `delete.rs:9` |
| `delete_recurring` | `template_id` | -- | `"Recurring template deleted: ..."` | `delete.rs:34` |
| `add_dependency` | `task_id`, `blocked_by` | -- | `"Dependency added: ..."` | `deps.rs:9` |
| `remove_dependency` | `task_id`, `blocked_by` | -- | `"Dependency removed: ..."` | `deps.rs:24` |
| `execute` | `id` | -- | Starts task or suggests plan based on complexity | `execute.rs:11` |

### TodoFeature

**File:** `src/lib.rs` (L42-95)

```rust
pub struct TodoFeature { tool: Arc<TodoTool> }

impl TodoFeature {
    pub fn new(tool: TodoTool) -> Self
    pub fn migration_sql() -> &'static str  // returns content of 001_create_todos.sql
}

impl FeaturePackage for TodoFeature {
    fn name(&self) -> &str              // "todo"
    fn tools(&self) -> Vec<DynTool>     // [TodoTool]
    fn migrations(&self) -> Vec<FeatureMigration>  // v1: core tables
    fn config_key(&self) -> &str        // "todo"
    fn default_config(&self) -> Value   // TodoConfig::default() serialized
    async fn health_check(&self) -> Result<HealthStatus>  // repo.summary() check
}
```

### Todo

**File:** `src/types.rs` (L12-55)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | 8-char UUID prefix |
| `title` | `String` | Task title |
| `description` | `Option<String>` | Optional long description |
| `priority` | `Option<u8>` | 1 (highest) to 5 (lowest) |
| `due_date` | `Option<DateTime<Utc>>` | Deadline |
| `tags` | `Vec<String>` | Labels |
| `status` | `TodoStatus` | Current lifecycle state |
| `focused_at` | `Option<DateTime<Utc>>` | When focus started |
| `focus_deadline` | `Option<DateTime<Utc>>` | When focus expires |
| `focus_expired_count` | `u32` | Number of expired focus sessions |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last modification |
| `completed_at` | `Option<DateTime<Utc>>` | When marked done |
| `parent_id` | `Option<String>` | Parent task for subtask hierarchy |
| `project_id` | `Option<String>` | Owning project |
| `attachments` | `Vec<Attachment>` | Attached files/URLs/notes |
| `time_entries` | `Vec<TimeEntry>` | Tracked time sessions |
| `total_tracked_secs` | `u64` | Aggregate tracked time |
| `estimated_minutes` | `Option<u32>` | Estimated duration |
| `calendar_event_uid` | `Option<String>` | Linked calendar event |
| `last_reminded_at` | `Option<DateTime<Utc>>` | Last reminder sent |
| `recurrence_rule` | `Option<String>` | RRULE string for recurring tasks |
| `recurrence_parent_id` | `Option<String>` | Template this instance was spawned from |
| `is_template` | `bool` | True for recurring task templates |
| `next_instance_date` | `Option<DateTime<Utc>>` | Next scheduled spawn |
| `blocked_by` | `Vec<String>` | Task IDs blocking this task |
| `blocks` | `Vec<String>` | Task IDs this task blocks |

**Methods:**
- `generate_id() -> String` -- 8-char UUID prefix
- `default_instance() -> Self` -- blank task with generated ID and current timestamp
- `to_storage_row(&self) -> TodoRow` -- convert to storage layer row

Implements `From<TodoRow>` and `tools_core::Searchable` (via `search_id() -> &str`).

### TodoStatus

**File:** `src/types.rs` (L99-142)

| Variant | String | Display |
|---------|--------|---------|
| `Todo` | `"todo"` | `"Todo"` |
| `Doing` | `"doing"` | `"Doing"` |
| `Done` | `"done"` | `"Done"` |
| `Archived` | `"archived"` | `"Archived"` |

**Methods:**
- `from_str_loose(s: &str) -> Option<Self>` -- case-insensitive parse
- `as_str(self) -> &'static str` -- lowercase string
- `display_name(self) -> &'static str` -- capitalized string

### TodoFilter

**File:** `crates/storage/src/repos/todo_repo.rs` (L13-20)

| Field | Type | Description |
|-------|------|-------------|
| `status` | `Option<String>` | Filter by status |
| `tags` | `Option<Vec<String>>` | Filter by tag(s) |
| `project_id` | `Option<String>` | Filter by project |
| `priority_min` | `Option<i16>` | Minimum priority |
| `limit` | `Option<i64>` | Max results |
| `templates_only` | `bool` | Only return templates |

### TodoPatch

**File:** `crates/storage/src/repos/todo_repo.rs` (L879-892)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Target task ID (required) |
| `title` | `Option<String>` | New title |
| `description` | `Option<Option<String>>` | New description (outer None = no change, Some(None) = clear) |
| `priority` | `Option<Option<i16>>` | New priority |
| `due_date` | `Option<Option<DateTime<Utc>>>` | New due date |
| `tags` | `Option<Vec<String>>` | Replacement tags |
| `status` | `Option<String>` | New status string |
| `calendar_event_uid` | `Option<Option<String>>` | Calendar event UID |
| `next_instance_date` | `Option<Option<DateTime<Utc>>>` | Next recurrence date |
| `last_reminded_at` | `Option<Option<DateTime<Utc>>>` | Last reminder timestamp |
| `estimated_minutes` | `Option<Option<i32>>` | Estimated duration |
| `recurrence_rule` | `Option<Option<String>>` | RRULE string |

### TodoSummary

**File:** `crates/storage/src/repos/todo_repo.rs` (L25-30)

| Field | Type | Description |
|-------|------|-------------|
| `todo` | `i64` | Count of tasks with status "todo" |
| `doing` | `i64` | Count of tasks with status "doing" |
| `done` | `i64` | Count of tasks with status "done" |
| `total` | `i64` | Total task count |

### Attachment

**File:** `src/types.rs` (L145-155)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID string |
| `attachment_type` | `AttachmentType` | `File`, `Url`, or `Note` |
| `title` | `Option<String>` | Display title |
| `value` | `String` | File path, URL, or note content |
| `tags` | `Vec<String>` | Labels |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

### AttachmentType

**File:** `src/types.rs` (L158-183)

Variants: `File`, `Url`, `Note`. Methods: `parse(s: &str) -> Option<Self>`, `as_str(&self) -> &'static str`.

### TimeEntry

**File:** `src/types.rs` (L186-195)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID string |
| `started_at` | `DateTime<Utc>` | Start time |
| `ended_at` | `Option<DateTime<Utc>>` | End time (None = running) |
| `duration_secs` | `Option<u64>` | Computed duration |
| `note` | `Option<String>` | Session note |
| `source` | `TimeEntrySource` | `Focus` (default) or `Manual` |

### TimeEntrySource

**File:** `src/types.rs` (L198-204)

Variants: `Focus` (default), `Manual`.

### EnrichmentSuggestion\<T\>

**File:** `src/enrichment.rs` (L14-21)

| Field | Type | Description |
|-------|------|-------------|
| `value` | `T` | The suggested value |
| `confidence` | `f64` | Confidence score in [0.0, 1.0] |
| `reasoning` | `String` | Human-readable explanation |

### EnrichmentResult

**File:** `src/enrichment.rs` (L24-29)

| Field | Type | Description |
|-------|------|-------------|
| `priority` | `Option<EnrichmentSuggestion<u8>>` | Priority suggestion |
| `estimated_minutes` | `Option<EnrichmentSuggestion<u32>>` | Duration suggestion |
| `due_date` | `Option<EnrichmentSuggestion<DateTime<Utc>>>` | Due date suggestion |

**Methods:**
- `is_empty() -> bool` -- true when all fields are None

### EnrichmentFeedbackEntry

**File:** `src/enrichment.rs` (L48-57)

| Field | Type | Description |
|-------|------|-------------|
| `task_id` | `String` | Task that was enriched |
| `field` | `String` | Field name (`"priority"`, `"estimated_minutes"`, `"due_date"`) |
| `suggested_value` | `String` | Stringified suggestion |
| `actual_value` | `Option<String>` | What the user set (if different) |
| `accepted` | `bool` | Whether the suggestion was accepted |
| `confidence` | `f64` | Original confidence score |
| `timestamp` | `DateTime<Utc>` | When feedback was recorded |

### Search Functions

**File:** `src/search.rs` (L13-23)

```rust
pub fn hybrid_merge(
    keyword_results: &[Todo],
    semantic_results: &[(String, f64)],  // (task_id, cosine_similarity)
    rrf_k: u32,
) -> Vec<(Todo, f64, &'static str)>  // (todo, rrf_score, source_tag)
```

Returns results sorted by RRF score descending. Source tag is `"keyword"`, `"semantic"`, or `"both"`.

### CalendarSyncHandler

**File:** `src/calendar_sync.rs` (L12-21)

```rust
#[async_trait]
pub trait CalendarSyncHandler: Send + Sync {
    async fn sync_calendar(&self) -> Result<()>;
    async fn push_task(&self, task_id: &str) -> Result<()>;
    async fn remove_event(&self, calendar_event_uid: &str) -> Result<()>;
}
```

### EmbeddingHandler

**File:** `src/embedding.rs` (L14-21)

```rust
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    async fn embed_todo(&self, todo: &Todo) -> Result<()>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}
```

### EnrichmentHandler

**File:** `src/enrichment.rs` (L41-45)

```rust
#[async_trait]
pub trait EnrichmentHandler: Send + Sync {
    async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>;
}
```

### EnrichmentFeedbackHandler

**File:** `src/enrichment.rs` (L60-63)

```rust
#[async_trait]
pub trait EnrichmentFeedbackHandler: Send + Sync {
    async fn record_feedback(&self, entry: EnrichmentFeedbackEntry) -> Result<()>;
}
```

### TaskComplexitySignals

**File:** `src/task_complexity.rs` (L11-17)

| Field | Type | Description |
|-------|------|-------------|
| `has_dependencies` | `bool` | Task has blockers |
| `subtask_count` | `u16` | Number of direct children |
| `dependency_depth` | `u8` | Max blocker chain depth (capped at 5) |
| `estimated_minutes` | `Option<u32>` | Duration estimate |
| `priority` | `u8` | Priority value (default 3 if unset) |

**Methods:**
- `complexity_score() -> u8` -- composite score 0-6 (see scoring table above)
- `is_plan_worthy(threshold: u8) -> bool` -- true if score >= threshold

**Free function:**
```rust
pub async fn evaluate_task_complexity(repo: &TodoRepo, task_id: &str) -> TaskComplexitySignals
```

### RRule Types and Functions

**File:** `src/rrule_utils.rs`

**Frequency enum** (L14-19): `Daily`, `Weekly`, `Monthly`, `Yearly`

**RRule struct** (L46-56):

| Field | Type | Description |
|-------|------|-------------|
| `freq` | `Frequency` | Recurrence frequency |
| `interval` | `u32` | Interval between occurrences (default: 1) |
| `byday` | `Vec<String>` | Day-of-week codes (`"MO"`, `"TU"`, ...) |
| `byhour` | `Vec<u32>` | Hours |
| `byminute` | `Vec<u32>` | Minutes |
| `bymonthday` | `Vec<u32>` | Days of month |
| `count` | `Option<u32>` | Max occurrences |
| `until` | `Option<DateTime<Utc>>` | End date |
| `exdate` | `Vec<DateTime<Utc>>` | Excluded dates |

**Methods:**
- `parse(rule: &str) -> Result<Self>` -- parse an RRULE string
- `next_occurrences(from, max) -> Result<Vec<DateTime<Utc>>>` -- compute next N occurrences after `from`

**Public functions:**
- `validate_rrule(rule: &str) -> Result<()>` -- validate syntax, reject unsupported params
- `next_occurrence(rule: &str, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>>` -- next single occurrence
- `should_spawn_instance(next_date: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool` -- true if next_date <= now
- `humanize_rrule(rule: &str) -> String` -- human-readable description

### Config Types

**TodoConfig** (`src/config.rs:8-24`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_focus_slots` | `usize` | `3` | Max simultaneous focus tasks |
| `focus_deadline_hours` | `u64` | `8` | Hours before focus expires |
| `timezone` | `String` | `"UTC"` | User's local timezone |
| `enrichment` | `EnrichmentConfig` | (see below) | Enrichment settings |
| `search` | `SearchConfig` | (see below) | Search settings |

**EnrichmentConfig** (`src/config.rs:51-69`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable auto-enrichment |
| `auto_apply_threshold` | `f64` | `0.70` | Confidence threshold for auto-apply |

**SearchConfig** (`src/config.rs:80-106`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable semantic search |
| `semantic_threshold` | `f64` | `0.5` | Min cosine similarity for results |
| `embedding_model` | `String` | `"paraphrase-multilingual-MiniLM-L12-v2"` | Model name in embedding records |
| `rrf_k` | `u32` | `60` | Reciprocal Rank Fusion k parameter |

### Database Schema

**Migration:** `migrations/001_create_todos.sql`

Four tables:

- **`todos`** -- main task table with 22 columns, indexes on status, project_id, parent_id, due_date, focused_at (partial), is_template (partial)
- **`todo_attachments`** -- file/URL/note attachments, cascading delete on todo removal
- **`todo_time_entries`** -- time tracking entries with source (focus/manual), cascading delete
- **`todo_dependencies`** -- directed edge table (task_id, blocker_id) with self-reference check constraint, cascading delete
