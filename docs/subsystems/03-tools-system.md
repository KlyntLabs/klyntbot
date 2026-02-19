# 03 — Tools System (Layer 3)

> Deep-dive analysis of `crates/tools/src/` — 29 source files, ~6,500 lines of code.

---

## Table of Contents

1. [Tool Trait Interface](#1-tool-trait-interface)
2. [Tool Registry](#2-tool-registry)
3. [Parameter Extraction](#3-parameter-extraction)
4. [Tool Implementations](#4-tool-implementations)
   - 4.1 [TodoTool (26 actions)](#41-todotool)
   - 4.2 [ProjectTool (6 actions)](#42-projecttool)
   - 4.3 [PlanTool (6 actions)](#43-plantool)
   - 4.4 [GoalTool (6 actions)](#44-goaltool)
   - 4.5 [CalendarTool (4 actions)](#45-calendartool)
   - 4.6 [CronTool (3 actions)](#46-crontool)
   - 4.7 [LearningTool (3 actions)](#47-learningtool)
   - 4.8 [MemoryTool (4 actions)](#48-memorytool)
   - 4.9 [SpawnTool (1 action)](#49-spawntool)
   - 4.10 [MessageTool (1 action)](#410-messagetool)
   - 4.11 [AskUserTool (1 action)](#411-askusertool)
   - 4.12 [ExecTool (1 action)](#412-exectool)
   - 4.13 [Filesystem Tools (4 tools)](#413-filesystem-tools)
   - 4.14 [Web Tools (2 tools)](#414-web-tools)
5. [Handler Traits (Dependency Inversion)](#5-handler-traits)
6. [Embedding Engine](#6-embedding-engine)
7. [Enrichment System](#7-enrichment-system)
8. [Todo Types & Data Model](#8-todo-types--data-model)
9. [Supporting Utilities](#9-supporting-utilities)
10. [Gap Analysis](#10-gap-analysis)

---

## 1. Tool Trait Interface

**File**: `lib.rs:60-110` (approximate)

The `Tool` trait is the core extension point for all agent capabilities:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;            // JSON Schema object
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;

    // Provided methods (default implementations):
    fn to_schema(&self) -> Value;             // OpenAI function-calling format
    fn validate_params(&self, params: &Value) -> Vec<String>;  // JSON Schema validation
}
```

### Key Design Decisions

- **`async fn execute`**: All tools are async, enabling I/O-bound operations (HTTP calls, file I/O, database queries).
- **`Value` in, `String` out**: Tools receive `serde_json::Value` arguments and return plain text results. This keeps the interface simple — the LLM consumes text, not structured data.
- **`RoutingContext`**: Carries `channel: ChannelName`, `chat_id: ChatId`, and an optional `interaction_tx: Option<mpsc::Sender<InteractionBundle>>` for tools like `ask_user` that need to block on user input.
- **`to_schema()`**: Default implementation wraps `name()`, `description()`, and `parameters()` in OpenAI's function-calling JSON format (`{"type": "function", "function": {...}}`).
- **`validate_params()`**: Recursive JSON Schema validation via `validate_value()` — checks `type`, `required`, `enum`, `minimum`/`maximum`, `minItems`/`maxItems`, nested `properties`, and `items` for arrays. Returns a `Vec<String>` of validation errors (empty = valid).

### Supporting Types

| Type | Purpose |
|------|---------|
| `DynTool` | `Arc<dyn Tool>` — type alias for dynamic dispatch |
| `RoutingContext` | Channel + chat routing, optional `interaction_tx` for interactive tools |
| `InteractionBundle` | Pairs an `InteractionRequest` with a `oneshot::Sender<FormResponse>` for ask_user |

---

## 2. Tool Registry

**File**: `registry.rs` (~120 lines)

The `ToolRegistry` is a named-map container for all registered tools:

```rust
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    cached_definitions: Mutex<Option<Vec<Value>>>,
}
```

### Operations

| Method | Behavior |
|--------|----------|
| `register(tool)` | Inserts tool by `tool.name()`, invalidates cache |
| `unregister(name)` | Removes tool, invalidates cache |
| `get(name) -> Option<DynTool>` | Lookup by name (returns `Arc` clone) |
| `has(name) -> bool` | Existence check |
| `get_definitions() -> Vec<Value>` | Cached array of `to_schema()` for all tools — rebuilt on first access after invalidation |
| `execute(name, args, ctx) -> Result<String>` | Validates params, then calls `tool.execute()` |
| `tool_names() -> Vec<String>` | Sorted list of all registered tool names |
| `len()`, `is_empty()` | Collection metrics |

### Cache Strategy

The `cached_definitions` field uses a `Mutex<Option<Vec<Value>>>` pattern:
- Set to `None` when any tool is registered/unregistered (invalidation).
- Lazily rebuilt on first `get_definitions()` call after invalidation.
- Subsequent calls return the cached vector (cheap clone of `Vec<Value>`).
- This avoids recomputing schemas every LLM request (they don't change mid-session).

### Validation on Execute

Before calling `tool.execute()`, the registry calls `tool.validate_params(&args)`. If validation returns errors, it returns a `ToolError::InvalidParams` with all error messages joined. This gives the LLM structured feedback to self-correct.

---

## 3. Parameter Extraction

**File**: `params.rs` (~300 lines including tests)

`ParamExtractor<'a>` is a zero-cost wrapper around `&Value` that provides typed, ergonomic parameter extraction:

```rust
pub struct ParamExtractor<'a> {
    args: &'a Value,
}
```

### Extractor Methods

| Method | Returns | On Missing |
|--------|---------|------------|
| `required_str(key)` | `&str` | `ToolError::InvalidParams` |
| `required_i64(key)` | `i64` | `ToolError::InvalidParams` |
| `required_u64(key)` | `u64` | `ToolError::InvalidParams` |
| `required_bool(key)` | `bool` | `ToolError::InvalidParams` |
| `required_array(key)` | `&Vec<Value>` | `ToolError::InvalidParams` |
| `required_object(key)` | `&Map<String, Value>` | `ToolError::InvalidParams` |
| `optional_str(key)` | `Option<&str>` | `None` |
| `optional_i64(key)` | `Option<i64>` | `None` |
| `optional_u64(key)` | `Option<u64>` | `None` |
| `optional_f64(key)` | `Option<f64>` | `None` |
| `optional_bool(key)` | `Option<bool>` | `None` |
| `optional_array(key)` | `Option<&Vec<Value>>` | `None` |
| `str_or(key, default)` | `&str` | `default` |
| `i64_or(key, default)` | `i64` | `default` |
| `string_array_or_empty(key)` | `Vec<String>` | `vec![]` |

All `required_*` methods produce clear error messages: `"Missing required parameter: {key}"` or `"Parameter '{key}' must be a {type}"`.

Every tool in the crate uses `ParamExtractor` — it eliminates the repetitive `.get().and_then().ok_or()` chains.

---

## 4. Tool Implementations

### 4.1 TodoTool

**File**: `todo.rs` (~1,200 lines)
**Name**: `"todo"`
**Actions**: 26

The largest and most complex tool. Manages the complete task lifecycle.

#### Actions

| Action | Parameters | Description |
|--------|-----------|-------------|
| `add` | title, description, priority, due_date, tags, project_id, parent_id, confirmed | Create task (with creation guard + auto-enrichment) |
| `list` | status, priority, project_id, tag, sort_by, limit | List tasks with filtering |
| `update` | id, title, description, priority, due_date, tags, project_id | Update task fields |
| `complete` | id | Complete task (with dependency checking + cascade) |
| `delete` | id | Soft-delete task |
| `show` | id | Show full task detail |
| `summary` | — | Overall task statistics |
| `focus` | id | Mark task as focused (max slots enforced) |
| `unfocus` | id | Remove focus from task |
| `add_subtask` | parent_id, title, description | Create subtask under parent |
| `move` | id, new_parent_id | Reparent task (cycle detection) |
| `attach` | id, url, name, attachment_type | Add attachment to task |
| `detach` | id, attachment_id | Remove attachment |
| `log_time` | id, minutes, note | Log time entry |
| `tree` | id | Display subtask tree |
| `search` | query | Keyword search (title + description) |
| `search_semantic` | query, threshold, limit | pgvector ANN cosine similarity |
| `search_hybrid` | query, threshold, limit | RRF merge of keyword + semantic |
| `report` | — | Daily planning report with scoring |
| `add_dependency` | id, depends_on | Add blocker relationship |
| `remove_dependency` | id, depends_on | Remove blocker |
| `recur` | id, rrule | Set recurrence rule on task |
| `list_recurring` | — | List all recurring tasks |
| `delete_recurring` | id | Remove recurrence rule |
| `enrich` | id | Manually trigger enrichment |
| `plan` | — | Generate daily plan with priority scoring |

#### Internal Fields

```rust
pub struct TodoTool {
    repo: storage::TodoRepo,                              // PostgreSQL repository
    calendar_handler: Option<Arc<dyn CalendarHandler>>,    // Calendar sync after mutations
    enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,// AI-powered field inference
    embedding_handler: Option<Arc<dyn EmbeddingHandler>>,  // Semantic search embeddings
    embedding_repo: Option<storage::EmbeddingRepo>,        // pgvector ANN queries
    feedback_handler: Option<Arc<dyn EnrichmentFeedbackHandler>>, // Learning feedback
    max_focus_slots: usize,                                // Config: max focused tasks
    focus_deadline_hours: u64,                             // Config: focus auto-expire
    semantic_threshold: f64,                               // Config: cosine similarity cutoff
    rrf_k: u32,                                           // Config: RRF k parameter
    timezone: String,                                     // Config: user timezone
    creation_mode: config::CreationMode,                  // Config: ask-first / yolo / party
}
```

#### Creation Guard

When `creation_mode == AskFirst`, the `should_guard_creation()` method blocks task creation if the LLM has hallucinated optional fields. It counts how many optional fields (priority, due_date, tags, description, project_id, parent_id) are filled in the request. If 2+ are set and `confirmed != true`, the tool returns an error instructing the LLM to use `ask_user` first.

#### Daily Planning Score

The `plan` action computes a priority score for each incomplete task:

```
score = (urgency * priority_weight) + (age_days * 0.1)
```

Where urgency tiers are:
- Overdue: 10
- Due today: 5
- Due tomorrow: 3
- Due in future: 1
- No due date: 0

And `priority_weight` maps priority 1→4, 2→3, 3→2, 4→1 (inverted — lower priority number = higher weight).

#### Side Effects on Mutations

After task creation/update/completion:
1. **Auto-enrichment** (on `add`): Calls `enrichment_handler.enrich_task()` to infer priority, duration, due date. Auto-applies suggestions above 0.7 confidence.
2. **Auto-embedding** (on `add`/`update`): Calls `embedding_handler.embed_todo()` to generate/update the task's embedding vector.
3. **Calendar sync** (on `add`/`update`/`complete`/`delete`): Calls `calendar_handler.sync_calendar()` to push changes to CalDAV.
4. **Dependency check** (on `complete`): Queries `incomplete_blockers()` and blocks completion if any exist.
5. **Cascade completion** (on `complete`): Marks all subtasks as Done.

---

### 4.2 ProjectTool

**File**: `project_tool.rs` (~200 lines)
**Name**: `"project"`
**Actions**: 6

| Action | Parameters | Description |
|--------|-----------|-------------|
| `create` | name, description, color | Create project |
| `list` | status | List projects with optional status filter |
| `show` | id | Show project detail |
| `update` | id, name, description, status, color | Update project fields |
| `archive` | id | Archive project |
| `tasks` | id, status | List tasks belonging to a project |

Holds `project_repo: storage::ProjectRepo` and `todo_repo: storage::TodoRepo`. The `tasks` action queries the todo repo filtered by `project_id`.

---

### 4.3 PlanTool

**File**: `plan_tool.rs` (~280 lines including tests)
**Name**: `"plan"`
**Actions**: 6

| Action | Parameters | Description |
|--------|-----------|-------------|
| `create` | title, description, session_key, goal_id | Create draft plan |
| `show` | plan_id | Show plan details + progress |
| `approve` | plan_id | Approve plan for execution |
| `abandon` | plan_id | Abandon plan |
| `status` | session_key | Show active plan for session |
| `execute` | plan_id | Trigger plan execution |

Uses `PlanHandler` trait (defined here, implemented in agent crate). The `session_key` defaults to `"{channel}:{chat_id}"` from `RoutingContext`.

**Supporting file**: `plan_response.rs` — Parses natural language daily plan responses:
- `PlanAction` enum: `Accept(Vec<String>)`, `Swap(String, String)`, `Skip(String)`, `DeferAll`
- `parse_plan_response()` uses regex patterns to interpret user responses like "accept all", "swap task-1 and task-2", "skip task-3", "defer everything"

---

### 4.4 GoalTool

**File**: `goal_tool.rs` (~250 lines)
**Name**: `"goal"`
**Actions**: 6

| Action | Parameters | Description |
|--------|-----------|-------------|
| `create` | title, description, target_date, tags | Create goal |
| `list` | status | List goals with optional status filter |
| `show` | id | Show goal detail |
| `update` | id, title, description, status, target_date | Update goal fields |
| `delete` | id | Delete goal |
| `progress` | id | Calculate goal progress |

Uses `GoalHandler` trait. Goal IDs are UUIDs parsed from string parameters. Progress is delegated entirely to the handler implementation.

---

### 4.5 CalendarTool

**File**: `calendar_tool.rs` (~120 lines)
**Name**: `"calendar"`
**Actions**: 4

| Action | Parameters | Description |
|--------|-----------|-------------|
| `sync` | — | Trigger immediate CalDAV sync |
| `list_events` | start_date, end_date | List events in date range |
| `create_event` | title, start, end, description, location | Create calendar event |
| `status` | — | Show sync status |

Uses `CalendarHandler` trait with 6 methods:
- `sync_calendar()`, `list_events()`, `create_event()`, `get_status()`, `get_event()`, `get_events_for_reconciliation()`

---

### 4.6 CronTool

**File**: `cron_tool.rs` (~180 lines)
**Name**: `"cron"`
**Actions**: 3

| Action | Parameters | Description |
|--------|-----------|-------------|
| `add` | schedule, task, label | Schedule recurring job |
| `list` | — | List all cron jobs |
| `remove` | id | Remove cron job |

Uses `CronHandler` trait. The schedule is a `CronSchedule` enum:
- `Every { minutes: u64 }` — simple interval
- `Cron { expression: String }` — standard cron expression

Serialized as `{"type": "every", "minutes": N}` or `{"type": "cron", "expression": "..."}`.

---

### 4.7 LearningTool

**File**: `learning_tool.rs` (~150 lines)
**Name**: `"learning"`
**Actions**: 3

| Action | Parameters | Description |
|--------|-----------|-------------|
| `status` | — | Show learning system status (tool usage, active tools, thresholds) |
| `analyze` | — | Trigger immediate analysis of tool usage patterns |
| `history` | — | Show threshold adjustment history |

Uses `LearningHandler` trait with methods:
- `get_status() -> LearningStatus` — returns `total_observations`, `active_tools`, `top_tools`, `last_analysis`
- `analyze_now()` — triggers analysis
- `get_threshold_history() -> Vec<ThresholdEntry>` — returns history of threshold adjustments

**Supporting types**:
- `ToolSummary { name, success_count, failure_count, avg_latency_ms }`
- `LearningStatus { total_observations, active_tools, top_tools, last_analysis }`
- `ThresholdEntry { field, old_value, new_value, reason, timestamp }`

---

### 4.8 MemoryTool

**File**: `memory_tool.rs` (~510 lines)
**Name**: `"memory"`
**Actions**: 4

| Action | Parameters | Description |
|--------|-----------|-------------|
| `search_conversations` | query, limit, threshold | Semantic search over conversation history |
| `search_all` | query, limit, threshold | Unified search across todos + conversations (RRF) |
| `purge` | filter, session_key, before_date | Clear conversation embeddings |
| `status` | — | Show memory store statistics |

**Internal fields** (all optional, injected via builder pattern):
- `conversation_handler: Option<Arc<dyn ConversationEmbeddingHandler>>`
- `todo_repo: Option<storage::TodoRepo>`
- `todo_embedding_handler: Option<Arc<dyn EmbeddingHandler>>`
- `embedding_repo: Option<storage::EmbeddingRepo>`
- `semantic_threshold: f64` (default 0.5)
- `rrf_k: u32` (default 60)

**Unified search (`search_all`)** is the most complex operation:
1. Searches conversations via `conversation_handler.search()`
2. Searches todos via keyword (SQL substring) + semantic (pgvector ANN)
3. Merges todo keyword + semantic results using RRF
4. Merges conversation + todo results using RRF
5. Formats output with source tags (`[Conversation|source]`, `[Todo|source]`)

**Purge filters**:
- `PurgeFilter::All` — delete everything
- `PurgeFilter::BySessionKey(key)` — delete by session
- `PurgeFilter::Before(datetime)` — delete before date

---

### 4.9 SpawnTool

**File**: `spawn.rs` (~100 lines)
**Name**: `"spawn"`
**Actions**: 1

| Parameter | Required | Description |
|-----------|----------|-------------|
| `task` | Yes | Task description for the subagent |
| `label` | No | Short display label |

Uses `SpawnHandler` trait:
```rust
async fn spawn(&self, task: String, label: Option<String>,
               origin_channel: String, origin_chat_id: String) -> String;
```

Routes origin channel/chat_id from `RoutingContext` to the handler so the subagent can send results back to the correct conversation.

---

### 4.10 MessageTool

**File**: `message.rs` (~80 lines)
**Name**: `"message"`
**Actions**: 1

| Parameter | Required | Description |
|-----------|----------|-------------|
| `content` | Yes | Message text |
| `channel` | No | Target channel (defaults to `ctx.channel`) |
| `chat_id` | No | Target chat ID (defaults to `ctx.chat_id`) |

Sends an `OutboundMessage` to the `bus` via `mpsc::Sender<OutboundMessage>`. Enables cross-channel communication — a tool call in Telegram can send a message to Discord.

---

### 4.11 AskUserTool

**File**: `ask_user.rs` (~920 lines including tests)
**Name**: `"ask_user"`
**Actions**: 1

The interactive clarification tool. Allows the agent to present structured questions to the user.

**Question types**:
- `single_select` — Pick one from options
- `multi_select` — Pick multiple from options
- `yes_no` — Boolean with optional default
- `free_text` — Open text input with optional placeholder

**Flow**:
1. Parse JSON args → `InteractionRequest` (flat schema, with backward-compat nested format)
2. If `ctx.interaction_tx` is `Some`, send `InteractionBundle` via mpsc and block on oneshot response
3. If `ctx.interaction_tx` is `None` (non-TTY), return text fallback with instructions for the LLM to present questions conversationally

**Response formatting**:
- `format_semantic_response()` — Rich output showing selected answers, descriptions, and other available options for full context
- `format_cancelled_response()` — Lists questions that were presented with instructions to offer alternatives
- `format_text_fallback()` — Formats questions as text when interactive UI is unavailable

**Constraints**:
- 1-4 questions per call (silently capped, not errored)
- Question titles truncated to 20 chars for tab display
- Auto-generated question IDs if not provided
- At least 2 options required for select questions

---

### 4.12 ExecTool

**File**: `shell.rs` (~430 lines including tests)
**Name**: `"exec"`
**Actions**: 1

| Parameter | Required | Description |
|-----------|----------|-------------|
| `command` | Yes | Shell command to execute |
| `working_dir` | No | Working directory override |

**Safety guards** (compiled once via `Lazy<Vec<Regex>>`):

| Category | Blocked Patterns |
|----------|-----------------|
| Destructive | `rm -rf`, `del /f`, `rmdir /s`, `format`, `mkfs`, `diskpart`, `dd if=` |
| System power | `shutdown`, `reboot`, `poweroff` |
| Network exploits | `curl | sh`, `wget | sh`, `nc -e`/`nc -l` |
| Permission escalation | `sudo`, `su -`, `chmod 777`, `chown root`, `passwd` |
| System changes | `iptables`, `firewall-cmd`, `crontab -r`/`-e` |
| Code injection | Fork bomb pattern |

**Workspace restriction** (`restrict_to_workspace: bool`):
- Blocks `../` path traversal
- Validates absolute paths (Windows + POSIX) against `working_dir` via `canonicalize()`
- Both Windows (`C:\...`) and POSIX (`/...`) path patterns detected via compiled regex

**Execution**:
- Cross-platform: `cmd /C` on Windows, `sh -c` on Unix
- Configurable timeout (defaults vary by consumer)
- Output truncation at 10,000 chars
- Captures stdout, stderr, and exit code separately
- `kill_on_drop(true)` prevents orphan processes

---

### 4.13 Filesystem Tools

**File**: `filesystem.rs` (~590 lines including tests)

Four tools sharing a common `FsToolBase` with optional directory restriction:

| Tool | Name | Parameters | Description |
|------|------|-----------|-------------|
| `ReadFileTool` | `read_file` | path | Read file contents |
| `WriteFileTool` | `write_file` | path, content | Write file (creates parent dirs) |
| `EditFileTool` | `edit_file` | path, old_text, new_text | Find-and-replace (exact match, unique occurrence) |
| `ListDirTool` | `list_dir` | path | List directory with file/folder icons |

**Path resolution** (`resolve_path()`):
- Expands `~` via `shellexpand::tilde()`
- Canonicalizes paths
- Enforces `allowed_dir` restriction (if configured)

**EditFileTool safety**:
- Rejects if `old_text` not found in file
- Rejects if `old_text` appears more than once (ambiguous edit) with count in error
- Single `replacen()` for exactly one replacement

**Convenience function**: `register_fs_tools(registry, allowed_dir)` — registers all 4 tools at once.

---

### 4.14 Web Tools

**File**: `web.rs` (~290 lines)

| Tool | Name | Parameters | Description |
|------|------|-----------|-------------|
| `WebSearchTool` | `web_search` | query, count | Brave Search API (titles, URLs, snippets) |
| `WebFetchTool` | `web_fetch` | url, extract_mode, max_chars | Fetch URL → text/markdown |

**WebSearchTool**:
- Uses Brave Search API (`api.search.brave.com/res/v1/web/search`)
- Requires API key (from config)
- 1-10 results, configurable
- Formats as numbered list with title, URL, description

**WebFetchTool**:
- HTTP/HTTPS only, validates URL scheme and host
- Content-type aware: JSON → pretty-print, HTML → `html2text` conversion, other → plain text
- 30-second timeout, 5 redirect limit
- Output truncation at configurable `max_chars` (default 50,000)

---

## 5. Handler Traits (Dependency Inversion)

All handler traits follow the same pattern: **defined in tools (Layer 3), implemented in agent (Layer 5), injected as `Arc<dyn Trait>` at construction**. This breaks what would otherwise be circular dependencies.

| Trait | File | Methods | Implementor |
|-------|------|---------|-------------|
| `SpawnHandler` | `spawn.rs` | `spawn()` | `SubagentManager` |
| `CronHandler` | `cron_tool.rs` | `add_job()`, `list_jobs()`, `remove_job()` | Scheduling crate adapter |
| `CalendarHandler` | `calendar_tool.rs` | `sync_calendar()`, `list_events()`, `create_event()`, `get_status()`, `get_event()`, `get_events_for_reconciliation()` | Calendar handler adapter |
| `PlanHandler` | `plan_tool.rs` | `create_plan()`, `get_plan()`, `get_active_plan()`, `approve_plan()`, `abandon_plan()`, `get_step_context()`, `execute_plan()` | Plan executor |
| `PlanCompletionHandler` | `plan_tool.rs` | `on_plan_completed()` | Goal metric updater |
| `GoalHandler` | `goal_tool.rs` | `create_goal()`, `get_goal()`, `list_goals()`, `update_goal()`, `delete_goal()`, `calculate_progress()` | Goal service |
| `LearningHandler` | `learning_tool.rs` | `get_status()`, `analyze_now()`, `get_threshold_history()` | Learning engine |
| `EnrichmentHandler` | `enrichment.rs` | `enrich_task()` | AI enrichment engine |
| `EmbeddingHandler` | `embedding_engine.rs` | `embed_todo()`, `embed_query()`, `is_available()` | `EmbeddingEngineImpl` |
| `ConversationEmbeddingHandler` | `conversation_embedding.rs` | `embed_message()`, `search()`, `purge()`, `status()`, `is_available()` | Conversation embedding store |
| `EnrichmentFeedbackHandler` | `learning_feedback.rs` | `record_feedback()` | Learning system |

**Total: 11 handler traits**, providing a clean boundary between the tools crate and higher-level logic.

---

## 6. Embedding Engine

### Core Engine

**File**: `embedding_engine.rs` (~200 lines)

```rust
pub struct EmbeddingEngine {
    model: Mutex<Option<TextEmbedding>>,
}
```

- **Lazy initialization**: Model (`paraphrase-multilingual-MiniLM-L12-v2`, 384 dimensions) is loaded on first `embed()` call, not at construction.
- **Thread-safe**: `Mutex<Option<TextEmbedding>>` for interior mutability.
- **Operations**:
  - `embed(text) -> Vec<f32>` — Single text embedding (384-dim vector)
  - `embed_batch(texts) -> Vec<Vec<f32>>` — Batch embedding
  - `cosine_similarity(a, b) -> f64` — Vector similarity
  - `is_available() -> bool` — Whether model is loaded

### EmbeddingEngineImpl

Implements `EmbeddingHandler` trait. Wraps `Arc<EmbeddingEngine>` + `storage::EmbeddingRepo`:
- `embed_todo(todo)` — Composes text as `"{title} {description} {tags}"`, runs in `spawn_blocking`, stores via repo
- `embed_query(query) -> Vec<f32>` — Embeds search query text

### Conversation Embedding Store

**File**: `conversation_embedding.rs` (~300 lines)

```rust
pub struct ConversationEmbeddingStore {
    inner: TokioRwLock<ConversationEmbeddingStoreInner>,
}
```

Uses `TokioRwLock` (not std `Mutex`) for async interior mutability. Supports:
- Embedding conversation messages with session context
- Semantic search with cosine similarity threshold
- Purge by session, date, or all
- Status reporting (total embeddings, indexed channels, date range)

### Legacy Embedding Store

**File**: `embedding_store.rs` — JSONL + SQL dual-mode storage. Supports:
- JSONL append-only file for local persistence
- SQL-backed pgvector for production
- Auto-compaction at threshold (100 stale entries)

### Search Utils

**File**: `search_utils.rs` (~100 lines)

**Reciprocal Rank Fusion (RRF)**:
```
score(d) = Σ 1/(k + rank_i)
```

Where `k` is a smoothing parameter (default 60) and `rank_i` is the rank in each result list.

**Source tracking**: Bitmask flags — keyword=1, semantic=2, both=3. Formatted as `"keyword"`, `"semantic"`, or `"keyword+semantic"` in output.

`SearchResult` enum: `Todo(Box<Todo>)` | `Conversation(ConversationEmbeddingRecord)` — unified type for cross-domain search.

---

## 7. Enrichment System

**File**: `enrichment.rs` (~60 lines)

### Data Types

```rust
pub struct EnrichmentSuggestion<T: Clone> {
    pub value: T,
    pub confidence: f64,   // 0.0 - 1.0
    pub reasoning: String, // Human-readable explanation
}

pub struct EnrichmentResult {
    pub priority: Option<EnrichmentSuggestion<i32>>,
    pub estimated_minutes: Option<EnrichmentSuggestion<i32>>,
    pub due_date: Option<EnrichmentSuggestion<String>>,
}
```

### EnrichmentHandler Trait

```rust
#[async_trait]
pub trait EnrichmentHandler: Send + Sync {
    async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>;
}
```

Implemented in agent crate using keyword analysis:
- **Priority inference**: Keywords → priority level (urgent/critical→1, bug/fix→2, feature/enhance→3, cleanup/chore→4)
- **Duration prediction**: Keywords → minutes (typo/rename→15, fix/patch→30, feature/implement→60, refactor/overhaul→120)
- **Due date suggestion**: Priority → deadline (urgent→today, important→this week, normal→none)

### Feedback Loop

**File**: `learning_feedback.rs` (~35 lines)

```rust
pub struct EnrichmentFeedbackEntry {
    pub task_id: String,
    pub field: String,           // "priority", "estimated_minutes", "due_date"
    pub suggested_value: String, // JSON-serialized
    pub actual_value: Option<String>, // None = accepted as-is
    pub accepted: bool,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
}
```

The `EnrichmentFeedbackHandler` trait records feedback when users accept or modify enrichment suggestions. This feeds into the learning system for threshold calibration.

---

## 8. Todo Types & Data Model

**File**: `todo_types.rs` (~500 lines)

### Todo Struct

The core task data model with 25+ fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Unique identifier |
| `title` | `String` | Task title |
| `description` | `Option<String>` | Detailed description |
| `status` | `TodoStatus` | Todo / Doing / Done / Archived |
| `priority` | `Option<i32>` | 1 (urgent) to 4 (low) |
| `due_date` | `Option<DateTime<Utc>>` | Deadline |
| `tags` | `Vec<String>` | Categorization tags |
| `project_id` | `Option<String>` | Parent project |
| `parent_id` | `Option<String>` | Parent task (hierarchical) |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |
| `completed_at` | `Option<DateTime<Utc>>` | Completion timestamp |
| `focused` | `bool` | Whether task is in focus |
| `focus_started_at` | `Option<DateTime<Utc>>` | When focus was set |
| `estimated_minutes` | `Option<i32>` | Duration estimate |
| `actual_minutes` | `Option<i32>` | Time spent |
| `time_entries` | `Vec<TimeEntry>` | Detailed time tracking |
| `attachments` | `Vec<Attachment>` | File/URL attachments |
| `dependencies` | `Vec<String>` | Task IDs this blocks on |
| `rrule` | `Option<String>` | Recurrence rule (RRULE format) |
| `is_template` | `bool` | Whether this is a recurring template |
| `next_instance_date` | `Option<DateTime<Utc>>` | Next recurrence spawn time |
| `order` | `i32` | Display order |
| `sort_order` | `i32` | Sort position |

### Supporting Types

| Type | Variants/Fields |
|------|-----------------|
| `TodoStatus` | `Todo`, `Doing`, `Done`, `Archived` |
| `TodoPatch` | Partial update struct (all fields `Option`) |
| `TodoFilter` | Filter by status, priority, project_id, tag, search query, limit, offset |
| `TodoSummary` | `total`, `by_status: HashMap`, `by_priority: HashMap`, `overdue`, `focused` |
| `Attachment` | `id`, `url`, `name`, `attachment_type`, `added_at` |
| `AttachmentType` | `File`, `Link`, `Image`, `Document`, `Other` |
| `TimeEntry` | `id`, `started_at`, `ended_at`, `minutes`, `note`, `source` |
| `TimeEntrySource` | `Manual`, `Timer`, `PomodoroSession` |

### Row Conversions

`Todo` implements `From<storage::TodoRow>` for seamless conversion from PostgreSQL rows. The conversion handles:
- JSON deserialization of `tags`, `dependencies`, `time_entries`, `attachments` from `serde_json::Value` columns
- `TodoStatus` mapping from string column values
- Timestamp handling (`NaiveDateTime` → `DateTime<Utc>`)

### Project Types

**File**: `project_types.rs` (~200 lines)

| Type | Description |
|------|-------------|
| `Project` | `id`, `name`, `description`, `status`, `color`, `created_at`, `updated_at` |
| `ProjectStatus` | `Active`, `Paused`, `Completed`, `Archived` |
| `ProjectColor` | `Red`, `Orange` (default), `Yellow`, `Green`, `Blue`, `Purple`, `Gray` |
| `ProjectPatch` | Partial update (all fields `Option`) |
| `ProjectFilter` | Filter by status |

---

## 9. Supporting Utilities

### RRULE Utils

**File**: `rrule_utils.rs` (~270 lines including tests)

Wraps the `rrule` crate for recurring task support:

| Function | Purpose |
|----------|---------|
| `validate_rrule(rule)` | Validates syntax + rejects unsupported V1 params (COUNT, UNTIL, EXDATE, etc.) |
| `next_occurrence(rule, after)` | Computes next datetime after given time |
| `should_spawn_instance(next_date, now)` | Returns true if instance is due |
| `humanize_rrule(rule)` | Converts RRULE to human-readable string |

**V1 Supported**: FREQ (DAILY/WEEKLY/MONTHLY/YEARLY), INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY.

**V1 Blocked**: BYSETPOS, WKST, EXDATE, EXRULE, RDATE, COUNT, UNTIL.

### Plan Response Parser

**File**: `plan_response.rs` (~100 lines)

Parses natural language responses to daily plan proposals:

| Pattern | Result |
|---------|--------|
| "accept all", "sounds good", "yes" | `Accept(all_task_ids)` |
| "accept task-1, task-3" | `Accept(specified_ids)` |
| "swap task-1 and task-2" | `Swap(id1, id2)` |
| "skip task-1" | `Skip(id)` |
| "defer all", "not today", "push everything" | `DeferAll` |

Uses `Regex` patterns compiled at parse time.

### Legacy Stores

**Files**: `todo_store.rs` (~2,300 lines), `project_store.rs` (~400 lines), `embedding_store.rs` (~300 lines)

All use the same **append-only JSONL journal** pattern:
```rust
enum JournalEntry<T> {
    Upsert(T),
    Delete { id: String },
}
```

With an in-memory `HashMap` index and `Vec<String>` for ordering. Auto-compaction at 100 stale entries. These are **superseded by PostgreSQL repos** but still present in the codebase.

---

## 10. Gap Analysis

### Active Gaps / Known Limitations

1. **Plan execution parameter generation**: `execute_step()` passes `{}` as tool arguments (noted in CLAUDE.md). Tools must work without explicit parameters, or parameter generation needs enhancement.

2. **Legacy JSONL stores still present**: `todo_store.rs` (2,300 lines), `project_store.rs` (400 lines), `embedding_store.rs` (300 lines) are superseded by PostgreSQL repos but not yet removed. These account for ~3,000 lines of dead code.

3. **RRULE V1 limitations**: No support for COUNT, UNTIL, EXDATE, EXRULE, RDATE, BYSETPOS, WKST. Tasks cannot have bounded recurrence (e.g., "every Monday for 4 weeks") or exception dates.

4. **No streaming tool results**: All tools return a single `String`. Long-running operations (web fetches, plan execution) don't stream partial results.

5. **Conversation embedding store dual-mode**: The `ConversationEmbeddingStore` uses `TokioRwLock` with interior mutability and an `AsyncOnceCell` lazy init pattern that adds complexity. Could be simplified now that PostgreSQL is the primary backend.

6. **MemoryTool unified search is O(n)**: The `search_all` action loads all todos via `todo_repo.list()`, then filters in-memory for keyword search. Should use SQL `ILIKE` or full-text search instead.

7. **No tool authorization**: Any tool can be called by the LLM with any parameters. There's no per-tool permission model (e.g., restricting `exec` or `write_file` based on user role).

8. **WebSearchTool hardcoded to Brave**: No provider abstraction for web search — only Brave Search API is supported.

### Potential Improvements

1. **Remove legacy stores**: Delete `todo_store.rs`, `project_store.rs`, `embedding_store.rs` and their JSONL persistence — all data is now in PostgreSQL.

2. **SQL-based keyword search**: Replace in-memory keyword filtering in `MemoryTool.search_all()` with PostgreSQL `ILIKE` or `tsvector` full-text search.

3. **RRULE V2**: Add support for COUNT, UNTIL, and EXDATE to enable bounded recurrence and exception dates.

4. **Tool permission model**: Add optional authorization checks to `ToolRegistry.execute()` based on tool name + user context.

5. **Batch embedding updates**: Current approach embeds one task at a time. The `EmbeddingEngine.embed_batch()` method exists but isn't used by `TodoTool`.

6. **Error recovery in ask_user**: When the interactive channel is dropped mid-question, the tool errors. Could implement a timeout-based fallback to text mode.

---

## Appendix: File Inventory

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | ~150 | Tool trait, RoutingContext, module declarations, re-exports |
| `registry.rs` | ~120 | ToolRegistry with cached definitions |
| `params.rs` | ~300 | ParamExtractor with typed getters |
| `todo.rs` | ~1,200 | TodoTool (26 actions) |
| `todo_types.rs` | ~500 | Todo struct, TodoStatus, TodoPatch, etc. |
| `todo_store.rs` | ~2,300 | Legacy JSONL store (superseded by PostgreSQL) |
| `project_tool.rs` | ~200 | ProjectTool (6 actions) |
| `project_types.rs` | ~200 | Project struct, ProjectStatus, etc. |
| `project_store.rs` | ~400 | Legacy JSONL store (superseded) |
| `plan_tool.rs` | ~280 | PlanTool (6 actions) + PlanHandler trait |
| `plan_response.rs` | ~100 | Natural language plan response parser |
| `goal_tool.rs` | ~250 | GoalTool (6 actions) + GoalHandler trait |
| `calendar_tool.rs` | ~120 | CalendarTool (4 actions) + CalendarHandler trait |
| `cron_tool.rs` | ~180 | CronTool (3 actions) + CronHandler trait |
| `learning_tool.rs` | ~150 | LearningTool (3 actions) + LearningHandler trait |
| `learning_feedback.rs` | ~35 | EnrichmentFeedbackHandler trait |
| `memory_tool.rs` | ~510 | MemoryTool (4 actions) — conversation + unified search |
| `spawn.rs` | ~100 | SpawnTool + SpawnHandler trait |
| `message.rs` | ~80 | MessageTool — cross-channel messaging |
| `ask_user.rs` | ~920 | AskUserTool — interactive structured questions |
| `shell.rs` | ~430 | ExecTool — shell execution with safety guards |
| `filesystem.rs` | ~590 | 4 filesystem tools (read, write, edit, list) |
| `web.rs` | ~290 | WebSearchTool + WebFetchTool |
| `enrichment.rs` | ~60 | EnrichmentHandler trait + suggestion types |
| `embedding_engine.rs` | ~200 | EmbeddingEngine + EmbeddingHandler trait |
| `embedding_store.rs` | ~300 | Legacy JSONL + SQL dual-mode store |
| `conversation_embedding.rs` | ~300 | ConversationEmbeddingStore + handler trait |
| `search_utils.rs` | ~100 | RRF merge + SearchResult enum |
| `rrule_utils.rs` | ~270 | RRULE validation, next occurrence, humanization |

**Total**: ~10,000 lines (including tests), 29 files, 15+ tool implementations, 11 handler traits.
