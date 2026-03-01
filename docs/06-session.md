# Session

## Purpose

The `session` crate (Layer 2) manages conversation state for Klyntbot. Every chat interaction -- whether from Telegram, Discord, Slack, WhatsApp, Email, or QQ -- happens within a session that tracks the full message history. The crate provides an in-memory cache with LRU eviction backed by SQLite persistence via the `storage::SessionRepo`, ensuring that sessions survive restarts while active conversations are served from memory.

This crate sits between storage (Layer 1.5) and the agent (Layer 5). The agent gets or creates a session for each incoming message, appends user and assistant messages to it, and relies on the session to provide conversation history for LLM context.

## Key Types

### Session

A conversation session containing:

- **`key: String`** -- A unique identifier in the format `channel:chat_id` (e.g., `telegram:12345`, `discord:98765`). This key is used for both in-memory lookup and database persistence.
- **`messages: Vec<SessionMessage>`** -- The ordered message history.
- **`created_at` / `updated_at`** -- Timestamps tracking when the session was created and last modified.
- **`metadata: HashMap<String, serde_json::Value>`** -- Extensible key-value metadata associated with the session.

`Session` provides methods to add messages and retrieve history:

- `add_message(role, content)` -- Appends a message with a generated UUID and the current timestamp.
- `add_message_with_request_id(role, content, request_id)` -- Same as above but with a correlation ID for tracing.
- `add_structured_message(role, content, request_id, tool_calls, metadata)` -- Appends a message with full structured data including tool call records and extensible metadata.
- `get_history(max_messages)` -- Returns a slice of the most recent N messages. Uses index arithmetic rather than copying, returning `&[SessionMessage]`.
- `clear()` -- Removes all messages from the session.

### SessionMessage

A single message within a session:

- **`id: String`** -- UUID v4 string, generated automatically on creation.
- **`role: String`** -- One of `system`, `user`, `assistant`, or `tool`.
- **`content: String`** -- The message text.
- **`timestamp: DateTime<Utc>`** -- When the message was created.
- **`request_id: Option<String>`** -- Optional correlation ID linking related messages across a request/response cycle.
- **`tool_calls: Option<serde_json::Value>`** -- Structured tool invocation data (function name, arguments, result) for assistant messages that triggered tool use.
- **`metadata: Option<serde_json::Value>`** -- Extensible metadata such as reasoning traces, content parts, or entity card data.

### SessionManager

The central type that coordinates in-memory caching, LRU eviction, SQL persistence, and compaction. It is `Clone + Send + Sync` -- all clones share the same underlying data structures, so it can be stored directly without wrapping in `Arc<RwLock<...>>`.

Internal structure:
- **`sessions: Arc<DashMap<String, Arc<TokioMutex<Session>>>>`** -- A concurrent hash map from session key to a per-session async mutex. `DashMap` provides lock-free concurrent reads for different keys.
- **`lru_order: Arc<StdMutex<VecDeque<String>>>`** -- Tracks access order for LRU eviction. Uses a standard (non-async) mutex because the critical section is a fast in-memory operation.
- **`max_cache_size: usize`** -- Maximum number of sessions to keep in memory (default: 1000).
- **`sql_repo: storage::SessionRepo`** -- The underlying SQL repository for durable storage.

### SessionInfo

A lightweight struct returned by `SessionManager::list()` containing the session key, timestamps, and message count. Used for session listing without loading full message histories.

## How It Works

### Session Lifecycle

1. **Get or Create** -- When a message arrives on any channel, the agent calls `session_manager.get_or_create(key)` with a key like `telegram:12345`.

2. **Cache Check** -- The manager first updates the LRU order (moving the key to the back of the queue) and evicts overflow sessions. Then it checks the `DashMap` for a cached session.

3. **Database Fallback** -- If the session is not in memory, the manager queries `SessionRepo::get_session()` and `SessionRepo::get_messages()` to reconstruct the full `Session` from SQL rows. The `row_to_session` method converts `SessionRow` + `Vec<SessionMessageRow>` into the domain `Session` type.

4. **New Session** -- If the session does not exist in the database either, the manager creates it by upserting a new row in the `sessions` table and returning a fresh `Session::new(key)`.

5. **Locking** -- The returned value is `Arc<TokioMutex<Session>>`. The caller locks this mutex to read or modify the session. Because each session has its own mutex, concurrent access to *different* sessions proceeds without blocking. Only concurrent access to the *same* session serializes.

6. **Message Addition** -- The agent appends messages to the locked session via `add_message()` or `add_structured_message()`.

7. **Persistence** -- After processing, the agent calls `session_manager.save(&session)`. This:
   - Upserts the session metadata to the `sessions` table.
   - Batch-inserts all messages using `INSERT OR IGNORE` (idempotent -- duplicate UUIDs are silently skipped). Messages are chunked into groups of 124 to stay under SQLite's 999 bind parameter limit.
   - Checks the message count against a compaction threshold (1000 messages). If exceeded, inserts a system marker message and deletes the oldest messages, keeping only the most recent 500.

### LRU Eviction

When the in-memory cache exceeds `max_cache_size` (1000 sessions), the oldest sessions (front of the `VecDeque`) are evicted:

1. The LRU lock is held briefly (sync mutex) to pop keys from the front.
2. The LRU lock is released.
3. Each evicted session is locked (async), saved to SQL, and removed from the `DashMap`.
4. A warning is logged if saving fails during eviction.

This two-phase approach avoids holding the LRU lock during async I/O.

### Session Compaction

When the SQL message count for a session exceeds 1000, the manager compacts it:

1. Inserts a system marker message: `"[Session compacted: N older messages removed]"`.
2. Deletes all messages except the most recent 500 (using a subquery with `ORDER BY timestamp DESC LIMIT`).

This prevents unbounded message growth in long-running sessions while preserving recent context.

### Session Reset and Deletion

- `reset_session(key)` removes the session from the in-memory cache unconditionally (both `DashMap` and LRU queue), then deletes it from the database. Messages are cascade-deleted by SQLite foreign keys.
- `delete(key)` performs the same operation and returns whether the database row existed.
- `delete_stale_sessions(ttl_days)` on the underlying `SessionRepo` removes sessions not updated within the TTL. Used for periodic cleanup.

### Concurrency Model

The session crate uses a per-session locking strategy:

- **DashMap** provides concurrent access to different sessions without a global lock. Multiple tasks can call `get_or_create` for different keys simultaneously.
- **TokioMutex per session** serializes access to a single session. Only one task can modify a given session at a time, but tasks on different sessions run in parallel.
- **StdMutex for LRU** is used because the LRU update is a fast, non-async operation (queue manipulation). This avoids the overhead of a tokio mutex for a critical section that never awaits.
- **DashMap::entry().or_insert()** handles the race where two tasks try to create the same session concurrently -- only one insertion wins, both get back the same `Arc<TokioMutex<Session>>`.

The `SessionManager` is `Clone` because all its fields are `Arc`-wrapped. This means it can be passed by value to async tasks, stored in tool registries, and shared across the application without additional wrapping.

### Data Flow to/from SQLite

The `SessionManager` does not access SQLite directly. It delegates all persistence to `storage::SessionRepo`, converting between domain types and row types:

**Domain to SQL (save path):**
- `Session.metadata` is serialized to `serde_json::Value` and passed to `SessionRepo::upsert_session()`.
- `Vec<SessionMessage>` is decomposed into parallel arrays (ids, roles, contents, timestamps, request_ids, tool_calls, metadata) and passed to `SessionRepo::batch_add_messages()`.

**SQL to Domain (load path):**
- `SessionRow` (key, metadata JSON, timestamps) + `Vec<SessionMessageRow>` (id, session_key, role, content, timestamp, request_id, tool_calls, metadata) are combined by `row_to_session()` into a `Session` with fully populated `messages` and `metadata`.

## Connections

### Depends on

- **common** (Layer 0) -- `KlyntbotError`, `Result` type alias.
- **storage** (Layer 1.5) -- `SessionRepo`, `SessionRow`, `SessionMessageRow`, `StorageError` for persistence.

### Depended on by

- **agent** (Layer 5) -- Uses `SessionManager` to manage conversation state for the agent loop, context building, and memory operations.
- **cli** (Layer 6) -- Uses `SessionManager` for session status/management commands.
- **klyntbot** (Layer 7) -- Re-exports `SessionManager` and `Session` via the facade.
