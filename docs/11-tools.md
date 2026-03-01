# Tools

## Purpose

The `tools` crate (Layer 3) contains all concrete tool implementations and domain-level handler traits. It is split into three categories: core tools (filesystem, web, browser, ask_user, message, spawn, cron, glob, grep), domain tools (calendar, plan, goal, project, memory, learning, agent task), and embedding infrastructure (engine, store, conversation embeddings). Feature-specific tools like todo and finance live in their own crates (`feature-todo`, `feature-finance`) and depend on `tools-core` directly.

## Key Types

### Core Tools

#### Filesystem Tools (read_file, write_file, edit_file, list_dir)

Four tools for file I/O, all built on `FsToolBase` which handles workspace directory restriction. When an `allowed_dir` is configured, all path operations are validated to stay within that directory tree. Convenience functions `register_fs_tools()` and `register_fs_read_tools()` handle batch registration.

- **ReadFileTool** -- reads file contents. Permission: ReadOnly.
- **WriteFileTool** -- writes content to a file, creating parent directories if needed. Permission: Elevated.
- **EditFileTool** -- find-and-replace within a file. Requires the old text to appear exactly once for safety. Permission: Elevated.
- **ListDirTool** -- lists directory contents with file/directory markers. Permission: ReadOnly.

#### Web Tools (web_search, web_fetch)

- **WebSearchTool** -- searches the web via the Brave Search API. Returns titles, URLs, and snippets. Configurable result count (1-10).
- **WebFetchTool** -- fetches a URL and extracts readable content. Handles JSON (pretty-printed), HTML (converted to text via html2text), and plain text. Supports output truncation with a max_chars parameter (default: 50000).

#### BrowserTool (browser)

Browser automation via the `agent-browser` CLI subprocess. Supports 14 actions: navigate, snapshot, click, type, fill, press, scroll, wait, get_text, screenshot, eval, fill_form, login_flow, and submit_and_confirm.

Key safety feature: write-action guarding based on `TrustLevel` (from config):
- **Full** -- no guards, everything executes immediately.
- **Autonomous** (default) -- guards dangerous actions (clicking "Submit", "Buy", "Delete"; filling payment fields like "Card Number", "CVV").
- **Strict** -- guards all click, fill, type, and submit actions.

When guarded, the tool returns a `[CONFIRMATION_REQUIRED]` message directing the LLM to use ask_user for confirmation first. Permission: Elevated.

#### AskUserTool (ask_user)

Interactive clarification system for structured user input. Supports four question types: single_select, multi_select, yes_no, and free_text. Groups 1-4 related questions into a single call.

Three execution paths:
1. **CLI/dashboard** -- sends the request via a oneshot channel, blocks until the user responds with platform-native UI.
2. **Platform-native channel** -- uses the `InteractionChannel` trait for Telegram buttons, Discord selects, etc.
3. **Text fallback** -- formats questions conversationally for non-interactive environments.

Returns a rich semantic response with full question context (selected and unselected options, descriptions) to maximize LLM understanding.

#### MessageTool (message)

Sends messages to channels via the outbound message bus. In direct mode (CLI/dashboard), skips the bus and returns content inline since the user receives responses via the event stream.

#### SpawnTool (spawn)

Spawns background subagents for independent task execution. Supports three actions: spawn (create a new subagent), cancel (stop a running one), and status (check all subagents for a session). Uses the `SpawnHandler` trait (dependency inversion) -- the actual implementation lives in the agent crate's `SubagentManager`. Subagents can be specialized via profiles: general (full access), research (web + read-only files), or analyst (read-only files, pure reasoning). Permission: Admin.

#### CronTool (cron)

Schedules reminders and recurring tasks. Actions: add, list, remove. Supports two schedule types: interval-based (`every_seconds`) and cron-expression-based (`cron_expr`). Uses the `CronHandler` trait (dependency inversion) -- implemented by the scheduling crate's `CronService`. Emits entity cards for newly created jobs.

#### GlobTool (glob)

Finds files by glob pattern matching (e.g., `**/*.rs`, `src/**/*.ts`). Returns matching file paths sorted by modification time (most recent first). Shares `FsToolBase` with filesystem tools for workspace restriction. Permission: ReadOnly.

#### GrepTool (grep)

Searches file contents using regex patterns within a directory scope. Returns matching lines with file path and line number. Supports optional file filter patterns (e.g., `*.rs`), configurable max results (default: 20), and context lines before/after matches (0-5). Shares `FsToolBase` for workspace restriction. Permission: ReadOnly.

### Domain Tools

These tools follow a consistent handler trait pattern: the trait is defined in the tools crate (Layer 3) but implemented in the agent crate (Layer 5), injected as `Arc<dyn Handler>` at construction. This breaks what would otherwise be circular dependencies between tools and agent.

#### CalendarTool (calendar)

Manages calendar operations via the `CalendarHandler` trait. Actions include sync, list events, create event, get status, push/pull tasks to/from calendar, and remove events. The handler is implemented by `CalendarSyncAdapter` in the agent crate, which coordinates between CalDAV providers (Apple Calendar, etc.) and the local task database.

#### PlanTool (plan)

Manages multi-step execution plans via the `PlanHandler` trait. Actions include create, show, approve, abandon, execute, and preview steps. Plans follow a state machine: Draft -> Approved -> Executing -> Completed/Failed/Abandoned. The `PlanCompletionHandler` trait is called when plans finish, allowing goal metrics updates without circular deps.

#### GoalTool (goal)

Strategic goal management via the `GoalHandler` trait. Actions include create, list, show, update, delete, decompose (generates an execution plan via LLM), progress (shows linked plan statuses), and metrics (plan completion rate, average duration). Goals have status tracking, priority, deadlines, and tagging.

#### ProjectTool (project)

Manages projects and project-task relationships. Unlike most domain tools, ProjectTool uses repositories directly (`ProjectRepo`, `TodoRepo`) rather than a handler trait, since it operates at the same layer as storage. Actions: create, list, show, update, archive, and tasks (list tasks within a project). Supports status tracking, color coding, and filtering.

#### MemoryTool (memory)

Semantic search over conversation history using embeddings. Supports three search modes: conversation-only search, todo search, and unified hybrid search using Reciprocal Rank Fusion (RRF) to merge keyword and semantic results. Configurable similarity threshold and RRF k parameter.

#### LearningTool (learning)

Exposes learning system insights via the `LearningHandler` trait. Actions: status (current learning metrics), analyze (trigger immediate analysis), and history (threshold change records). Provides per-tool statistics (call counts, success rates, average duration), strategy accuracy, and adaptive threshold recommendations.

#### AgentTaskTool (agent_task)

Subagent coordination via the `AgentTaskHandler` trait. Only registered in subagent tool registries, not the parent agent's. Allows subagents to interact with a shared task board: list available tasks, claim ownership, report completion with results, or report failure. Permission: Standard.

### Embedding Infrastructure

#### EmbeddingEngine

Core embedding engine wrapping fastembed with lazy model initialization. The model (paraphrase-multilingual-MiniLM-L12-v2, ~420MB) is downloaded on first use, not at construction time. Provides:
- `embed(text)` -- generate a 384-dimensional embedding vector.
- `embed_batch(texts)` -- batch embedding for efficiency.
- `embed_async(text)` -- async wrapper running embedding on a blocking thread pool.
- `cosine_similarity(a, b)` -- vector similarity with NaN handling.

Feature-gated behind `semantic-search` -- when disabled, all embed methods return errors but the struct still exists for API compatibility.

#### EmbeddingHandler Trait

Dependency inversion for embedding operations. Defined in tools (Layer 3), enabling tests to mock without loading the 420MB model. Methods: `embed_todo(todo)` (generates and persists embedding), `embed_query(query)` (generates query vector), `is_available()`.

#### EmbeddingEngineImpl

Production implementation of `EmbeddingHandler`. Wraps `EmbeddingEngine` (for vector generation) and `VectorStore` (for LanceDB persistence). Composes searchable text from todo title, description, and tags.

#### EmbeddingStore

Lightweight in-memory embedding cache. Used alongside LanceDB persistence (handled by `storage::VectorStore`). Provides upsert, delete, get, and batch lookup operations.

#### ConversationEmbeddingStore

LanceDB-backed conversation embedding storage. All persistence delegates to the underlying `VectorStore`. Stores `ConversationEmbeddingRecord` entries with session key, role, content preview, full content, and 384-dimensional embedding vectors.

#### ConversationEmbeddingHandler Trait

Dependency inversion for the conversation embedding pipeline. Defined in tools (Layer 3), implemented in agent (Layer 5). Handles embedding new messages, searching conversation history, getting status, and purging old embeddings (by session, date, or all).

### The Handler Trait Pattern

Domain tools consistently use dependency inversion to avoid circular deps between tools (Layer 3) and agent (Layer 5):

1. Define a handler trait in tools crate: `CalendarHandler`, `PlanHandler`, `GoalHandler`, `SpawnHandler`, `CronHandler`, `LearningHandler`, `AgentTaskHandler`, `ConversationEmbeddingHandler`.
2. The tool struct holds `Option<Arc<dyn Handler>>` or `Arc<dyn Handler>`.
3. At agent startup, implementations are constructed in the agent crate and injected into the tools.
4. When the handler is `None` (optional handlers), the tool returns a graceful error explaining the handler is not available.

## How It Works

### Tool Registration

At agent startup, tools are registered into a `ToolRegistry`:

1. Filesystem tools are registered via `register_fs_tools()` with an optional workspace restriction directory.
2. Web tools are created with API keys from config.
3. Domain tools are created with their handler implementations injected.
4. Feature package tools are registered via `register_dyn()`.
5. The registry caches all tool definitions for LLM function-calling schemas.

### Tool Execution

When the LLM generates a function call:

1. The agent loop extracts the tool name and JSON arguments.
2. `ToolRegistry::execute()` looks up the tool, checks permissions, validates params, and calls `tool.execute(args, ctx)`.
3. For derived tools, the generated `Tool::execute` parses JSON to typed params and delegates to `ToolExecute::execute`.
4. For action-based tools, the `action` field is extracted and dispatched to the corresponding method.
5. The result string is fed back to the LLM as a tool response message.

### Workspace Restriction

`FsToolBase` provides path resolution and directory restriction for all filesystem-related tools (read, write, edit, list, glob, grep). When `allowed_dir` is set, paths are canonicalized and checked to ensure they start with the allowed directory prefix. Attempts to access paths outside the workspace return a `PermissionDenied` error.

## Connections

**Depends on:**
- `tools-core` (Layer 3) -- `Tool`, `ToolExecute`, `ToolParams`, `RoutingContext`, `ToolRegistry`, `ParamExtractor`
- `common` (Layer 0) -- error types, `Result`, interaction types
- `config` (Layer 1) -- `TrustLevel` for browser
- `bus` (Layer 1) -- `OutboundMessage` for the message tool
- `storage` (Layer 1.5) -- `ProjectRepo`, `TodoRepo`, `VectorStore` for direct DB access
- `calendar` (Layer 2) -- `CalendarEvent` type
- `domain` -- `Plan`, `Goal` types
- `fastembed` -- embedding model (feature-gated)
- `reqwest` -- HTTP client for web tools
- `globset`, `walkdir` -- for glob/grep file traversal
- `regex` -- for grep pattern matching

**Depended on by:**
- `agent` (Layer 5) -- registers all tools, implements handler traits, drives execution
- `klyntbot` (Layer 7) -- re-exports tool types
