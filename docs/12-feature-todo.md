# feature-todo

## Purpose

The `feature-todo` crate is a self-contained task management feature package for Klyntbot. It sits at **Layer 3** of the workspace architecture (alongside the tools layer) and implements the `FeaturePackage` trait from `tools-core`. It provides a single `TodoTool` with 27 actions, domain types for tasks and their associated data, and trait abstractions that allow the tool to call into higher layers (agent, calendar) without circular dependencies.

## Key Types

### Domain types

**`Todo`** -- The central task struct. Fields include:
- Identity: `id` (8-char UUID prefix), `title`, `description`, `tags`
- Scheduling: `priority` (1-5), `due_date`, `estimated_minutes`, `status`
- Focus tracking: `focused_at`, `focus_deadline`, `focus_expired_count`
- Hierarchy: `parent_id`, `project_id`
- Dependencies: `blocked_by`, `blocks` (populated at load time from the dependency table)
- Attachments and time tracking: `attachments: Vec<Attachment>`, `time_entries: Vec<TimeEntry>`, `total_tracked_secs`
- Calendar integration: `calendar_event_uid`
- Recurrence: `recurrence_rule` (RRULE string), `recurrence_parent_id`, `is_template`, `next_instance_date`
- Timestamps: `created_at`, `updated_at`, `completed_at`, `last_reminded_at`

**`TodoStatus`** -- Four-state lifecycle: `Todo` -> `Doing` -> `Done` -> `Archived`.

**`Attachment`** -- A file, URL, or note attached to a task. Typed by `AttachmentType` (File, Url, Note).

**`TimeEntry`** -- A time tracking record with start/end timestamps, duration, optional note, and `TimeEntrySource` (Focus or Manual).

### Configuration

**`TodoConfig`** -- Top-level config with:
- `max_focus_slots` (default 3): how many tasks can be focused simultaneously
- `focus_deadline_hours` (default 8): hours before a focus session expires
- `timezone` (default "UTC"): user's local timezone
- `enrichment: EnrichmentConfig`: `enabled` (default true), `auto_apply_threshold` (default 0.70)
- `search: SearchConfig`: `enabled` (default true), `semantic_threshold` (default 0.5), `embedding_model` (default "paraphrase-multilingual-MiniLM-L12-v2"), `rrf_k` (default 60)

### Trait abstractions (dependency inversion)

**`EnrichmentHandler`** -- `async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>`. Defined here, implemented in the agent crate's enrichment engine.

**`EnrichmentFeedbackHandler`** -- `async fn record_feedback(&self, entry: EnrichmentFeedbackEntry) -> Result<()>`. Records whether enrichment suggestions were accepted or rejected.

**`EmbeddingHandler`** -- Two methods: `embed_todo` (generate and store an embedding for a task) and `embed_query` (embed a search query string into a vector). Implemented by the agent's embedding engine using fastembed.

**`CalendarSyncHandler`** -- Three methods: `sync_calendar` (trigger full sync), `push_task` (push a single task to calendar), and `remove_event` (remove a calendar event by UID). Implemented by the calendar handler in the agent crate.

### Enrichment types

**`EnrichmentSuggestion<T>`** -- A suggestion with a `value: T`, `confidence: f64` (0.0-1.0), and `reasoning: String`.

**`EnrichmentResult`** -- Aggregates optional suggestions for `priority`, `estimated_minutes`, and `due_date`. The `is_empty()` method checks whether all fields are None.

**`EnrichmentFeedbackEntry`** -- Records the outcome of an enrichment suggestion: task ID, field name, suggested value, actual value, whether it was accepted, confidence, and timestamp.

### Task complexity

**`TaskComplexitySignals`** -- Evaluates whether a task warrants plan-based execution. Scores 0-6 based on five signals:
- `has_dependencies` (+2 points)
- `subtask_count >= 3` (+1 point)
- `dependency_depth >= 2` (+1 point)
- `estimated_minutes >= 60` (+1 point)
- `priority <= 2` (high priority, +1 point)

The `is_plan_worthy(threshold)` method checks if the score meets the threshold. The `evaluate_task_complexity` function queries the database to compute these signals, including recursive dependency depth (capped at 5 levels).

### Feature package

**`TodoFeature`** -- Implements `FeaturePackage` with:
- `name()` -> "todo"
- `tools()` -> the single `TodoTool`
- `migrations()` -> one SQL migration creating the todos, attachments, time_entries, and dependencies tables
- `config_key()` -> "todo"
- `default_config()` -> serialized `TodoConfig`
- `health_check()` -> attempts a summary query against the database

## How It Works

### TodoTool actions (27 actions grouped by category)

**CRUD** -- `add`, `update`, `delete`, `show`, `complete`. Creating a task generates an 8-char ID, optionally runs enrichment (auto-applying suggestions above the confidence threshold), generates an embedding, and pushes to calendar if a due date is set. Completing a task sets `completed_at` and transitions status to Done.

**Listing and summary** -- `list` (filtered by status, tag, priority, project, with limit), `summary` (counts by status), `tree` (hierarchical view of parent-child relationships).

**Search** -- Three modes:
- `search` -- keyword matching via SQL substring on title and description
- `search_semantic` -- embeds the query via `EmbeddingHandler`, then performs approximate nearest neighbor (ANN) search in LanceDB, filtering by cosine similarity threshold
- `search_hybrid` -- runs both keyword and semantic searches, merges results using Reciprocal Rank Fusion (RRF) with configurable k parameter. The `hybrid_merge` function in `search.rs` delegates to the `rrf_merge` utility from `tools-core`. Each result is tagged with its source: "keyword", "semantic", or "both".

**Focus** -- `focus` (mark a task as focused with a deadline based on `focus_deadline_hours`), `unfocus` (clear focus state). The tool enforces `max_focus_slots` to limit concurrent focused tasks.

**Hierarchy and movement** -- `add_subtask` (create a child task under a parent), `move` (reassign parent or project).

**Attachments** -- `attach` (add a file, URL, or note attachment to a task), `detach` (remove an attachment by ID).

**Time tracking** -- `log_time` (record a manual time entry with duration and optional note).

**Dependencies** -- `add_dependency` (mark one task as blocked by another), `remove_dependency` (remove a blocking relationship).

**Reporting** -- `report` (generate a productivity report for a given period, aggregating completed/created tasks, time tracked, and focus sessions by project).

**Recurring tasks** -- `recur` (set an RRULE on a task, making it a recurring template), `list_recurring` (list all recurring templates), `delete_recurring` (remove a recurring template and optionally its spawned instances). RRULE support covers FREQ (daily/weekly/monthly/yearly), INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY, COUNT, UNTIL, and EXDATE. Unsupported parameters (BYSETPOS, WKST, EXRULE, RDATE) are rejected at validation time. The `humanize_rrule` function converts RRULE strings to human-readable descriptions.

**Enrichment** -- `enrich` (manually trigger enrichment analysis on a task). Enrichment infers priority from urgency keywords, estimates duration from task-size keywords, and suggests due dates based on priority level.

**Planning and execution** -- `plan` (generate a prioritized work plan from the top N tasks by score), `execute` (evaluate task complexity and either start the task directly or suggest creating a plan for complex tasks). The scoring formula is `(urgency * priority_weight) + (age_days * 0.1)`, where urgency increases sharply for overdue or same-day tasks.

### Enrichment system

The enrichment engine uses keyword-based heuristics:
- **Priority**: keywords like "urgent", "critical", "blocker" -> priority 1; "bug", "fix" -> priority 2; "feature", "refactor" -> priority 3; "cleanup", "typo" -> priority 4
- **Duration**: "typo", "rename" -> 15 min; "fix", "patch" -> 30 min; "feature", "implement" -> 60 min; "refactor", "overhaul" -> 120 min
- **Due date**: urgent tasks -> today; important tasks -> this week; normal tasks -> no suggestion

Suggestions include a confidence score. When the confidence exceeds `auto_apply_threshold` (default 0.70), the suggestion is auto-applied to the task. Feedback is recorded via `EnrichmentFeedbackHandler` for each field suggestion.

### Row-domain conversions

All domain types have bidirectional `From` implementations to/from their storage row counterparts (`TodoRow`, `TodoAttachmentRow`, `TodoTimeEntryRow`). The `Todo` type's `blocked_by` and `blocks` fields and the `attachments` and `time_entries` vectors are populated separately via `load_full_todo`, which runs four parallel queries (`list_attachments`, `list_time_entries`, `get_blockers`, `get_blocking`) using `tokio::try_join!`.

## Connections

**Depends on:**
- `common` (error types, Result alias)
- `storage` (TodoRepo, TodoRow, VectorStore, row types)
- `tools-core` (Tool trait, FeaturePackage trait, ParamExtractor, RoutingContext, rrf_merge, Searchable)
- `rrule` (RRULE parsing library)
- `chrono`, `serde`, `serde_json`, `uuid`, `async-trait`, `tracing`

**Depended on by:**
- `agent` (implements EnrichmentHandler, EmbeddingHandler, CalendarSyncHandler; constructs TodoFeature)
- `klyntbot` (re-exports via facade)
- Integration tests
