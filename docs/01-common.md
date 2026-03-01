# common

## Purpose

The `common` crate is the Layer 0 foundation of the Klyntbot workspace. Every other crate depends on it, either directly or transitively. It provides the unified error hierarchy, strong domain types that replace raw strings throughout the system, interactive prompt structures used by the `ask_user` tool, and a collection of TUI/terminal utilities for the CLI experience. Because it sits at the bottom of the dependency graph, nothing in `common` depends on any other workspace crate.

## Key Types

### Error Hierarchy

All error handling in Klyntbot flows through a single top-level enum and five domain-specific sub-enums:

**`KlyntbotError`** -- the root error type. Variants fall into two groups:

| Variant | Source | Purpose |
|---------|--------|---------|
| `Tool(ToolError)` | `From` impl | Tool lookup, parameter validation, execution failure, permission denial |
| `Provider(ProviderError)` | `From` impl | HTTP errors, invalid LLM responses, rate limiting (with optional retry-after), auth failures (includes the config key to check) |
| `Channel(ChannelError)` | `From` impl | Connection failures, send failures, invalid channel config |
| `Session(SessionError)` | `From` impl | Session not found, load/save failures (wraps IO and JSON errors) |
| `Config(ConfigError)` | `From` impl | Config file not found, invalid config, missing required fields (wraps IO and JSON errors) |
| `Bus(String)` | Direct | Message bus validation errors (e.g., oversized messages) |
| `BusDisconnected` | Direct | The mpsc channel was dropped |
| `Cron(String)` | Direct | Cron scheduling errors |
| `Calendar(String)` | Direct | Calendar sync failures |
| `Goal(String)` | Direct | Goal management errors |
| `Plan(String)` | Direct | Plan state machine violations |
| `Storage(String)` | Direct | SQLite/LanceDB persistence errors |
| `StorageNotFound(String)` | Direct | Entity not found in storage (used for 404-style responses) |
| `StorageConflict(String)` | Direct | Duplicate key or constraint violation |
| `Io(std::io::Error)` | `From` impl | General IO errors |
| `Json(serde_json::Error)` | `From` impl | JSON serialization/deserialization errors |

**`Result<T>`** is a type alias for `std::result::Result<T, KlyntbotError>`, used as the standard return type across the workspace.

The `From` implementations on `KlyntbotError` allow sub-errors to be converted automatically via `?`. This means a function in the `tools` crate can return a `ToolError`, and the calling code in the `agent` crate can propagate it as a `KlyntbotError` without explicit conversion.

### Domain Types (`types.rs`)

These newtypes replace raw `String` identifiers to prevent accidental misuse (e.g., passing a chat ID where a channel name is expected):

- **`ChannelName`** -- wraps a `String` identifying a chat platform ("telegram", "discord", "slack", etc.). Implements `Display`, `Hash`, `Eq`, and converts from `String` and `&str`.
- **`ChatId`** -- wraps a `String` identifying a specific chat or conversation within a channel. Same trait surface as `ChannelName`.
- **`SessionKey`** -- a composite key formatted as `"channel:chat_id"` (e.g., `"telegram:123456"`). Constructed from a `ChannelName` + `ChatId` pair via `SessionKey::new()`. Can be split back into its parts with `split()`. Used as the primary key for conversation sessions.
- **`MessageRole`** -- an enum with four variants: `System`, `User`, `Assistant`, `Tool`. Serializes to lowercase strings. Has two parsing paths: `From<&str>` silently defaults unknown strings to `User` (convenient in hot paths), while `parse_strict()` returns a `KlyntbotError` for unknown values (safe at system boundaries).

Three well-known constants are also defined: `SYSTEM_CHANNEL`, `CLI_CHANNEL`, and `TELEGRAM_RESET_SENDER`.

### Entity Cards (`entity_card.rs`)

`EntityCard` is a lightweight struct emitted by tools when they create or modify entities (todos, projects, plans, etc.). It carries:

- `entity_type` and `entity_id` -- what was created
- `title` and optional `subtitle` -- for display
- `route` -- optional deep link for dashboard navigation
- `icon_hint` -- suggested icon for UI rendering
- `metadata` -- arbitrary key-value pairs (skipped in serialization when empty)

Entity cards flow through the agent's `RoutingContext` to the event stream, enabling real-time UI updates without coupling tools to specific frontends.

### Interactive Prompts (`prompts.rs`)

These types power the `ask_user` tool, which presents structured multi-question forms to users. They live in `common` (Layer 0) so both the `tools` crate (which creates them) and the `cli` crate (which renders them) can use them without circular dependencies.

- **`InteractionRequest`** -- a titled form containing 1-4 `Question`s to present simultaneously.
- **`Question`** -- has a machine-readable `id`, a short tab-header `title`, full `text`, and an `AnswerType`.
- **`AnswerType`** -- tagged enum with four variants: `SingleSelect` (pick one from options), `MultiSelect` (pick many), `YesNo` (boolean toggle with optional default), `FreeText` (open input with optional placeholder).
- **`AnswerOption`** -- a selectable choice with a machine-readable `value`, display `label`, and optional `description`.
- **`Answer`** -- a user's response to a single question, pairing a `question_id` with an `AnswerValue`.
- **`AnswerValue`** -- tagged enum: `Selected`, `MultiSelected`, `YesNo`, `Text`, or `Skipped`.
- **`FormResponse`** -- either `Completed(Vec<Answer>)` or `Cancelled` (user pressed Esc/Ctrl+C).

All prompt types are `Serialize + Deserialize` with `snake_case` tag names for JSON interop.

### TUI Utilities (`utils/`)

The `utils` module provides shared helper functions and terminal rendering infrastructure:

**`date.rs`** -- a single-source-of-truth date parser that every crate uses:
- `parse_datetime(s, fallback_tz)` accepts RFC3339, ISO datetime, date-only, "YYYY-MM-DD HH:MM" formats, and natural language relative dates ("today", "tomorrow", "next friday", "in 3 days", "in 2 weeks"). Non-timezone strings are interpreted in the given fallback timezone.
- `format_datetime_local(dt, timezone, fmt)` formats a UTC datetime for display in a specific timezone.
- `timezone_utc_offset(timezone)` returns the current UTC offset string for a timezone (e.g., "+07:00").

**`helpers.rs`** -- small utility functions used throughout the codebase:
- `extract_json_array(s)` / `extract_json_object(s)` -- find JSON structures inside prose or markdown text (common when parsing LLM output).
- `strip_llm_fences(s)` -- remove markdown code fences from LLM responses.
- `truncate_at_boundary(s, max_bytes)` / `truncate_chars(s, max_chars, suffix)` -- safe UTF-8-aware string truncation.
- `tool_def_name(def)` -- extract the function name from an OpenAI-style tool definition JSON value.
- `format_timestamp_ms(ms)` -- convert millisecond timestamps to ISO 8601 strings.

**`notify.rs`** -- cross-platform native OS notifications:
- `send_os_notification(title, body)` sends desktop notifications using platform-specific mechanisms (AppleScript on macOS, `notify-send` on Linux, PowerShell toast on Windows).
- Input is sanitized to prevent shell injection attacks on each platform.

**`stream_renderer.rs`** -- `StreamRenderer` manages real-time LLM output display:
- During streaming, raw text is printed token-by-token for instant feedback.
- Tool executions are shown as in-progress spinners that update in-place to success/failure indicators when complete.
- On finalize, the raw output is erased and replaced with a markdown-rendered version.
- Handles pause/resume for interactive prompts, cancellation indicators, and non-TTY graceful degradation.

**`terminal/`** -- a submodule collection for CLI rendering:
- `colors.rs` -- ANSI color output with `NO_COLOR` and TTY detection support.
- `spinners.rs` -- braille spinner for "thinking" indicators.
- `boxes.rs` -- box drawing for response display.
- `tables.rs` -- table rendering utilities.
- `markdown.rs` -- terminal markdown renderer for formatting LLM output.
- `thinking_renderer.rs` -- renders extended thinking/chain-of-thought blocks.

## How It Works

### Error Propagation

The error system uses a two-level pattern. Domain-specific errors (`ToolError`, `ProviderError`, etc.) are defined as separate enums so that crates at their own layer can work with focused error types. Each domain error auto-converts into `KlyntbotError` via `#[from]` attributes generated by the `thiserror` crate.

A typical flow: a tool in Layer 4 returns `Err(ToolError::ExecutionFailed(...))`. The agent loop in Layer 5 calls the tool through the `Tool` trait, which returns `common::Result<Value>`. The `?` operator automatically converts `ToolError` into `KlyntbotError::Tool(...)` via the `From` impl. The agent loop can then pattern-match on the error variant to decide how to respond (e.g., retry, inform the user, or escalate).

### Interactive Prompt Flow

1. The LLM decides to call the `ask_user` tool and provides a JSON `InteractionRequest` (title + questions).
2. The tool validates the request (1-4 questions) and passes it to the rendering layer.
3. The CLI renders a tabbed UI using the terminal utilities. Channel integrations render platform-appropriate alternatives.
4. The user completes or cancels the form, producing a `FormResponse`.
5. The response is serialized back as the tool's return value and fed into the next LLM iteration.

### Stream Rendering Flow

1. `StreamRenderer::new()` captures the terminal width and TTY state.
2. As LLM tokens arrive, `on_content_chunk()` prints them immediately and tracks visual line counts (accounting for terminal wrapping).
3. When tools execute, `on_tool_start()` prints a spinner line and `on_tool_end()` overwrites it in-place with a success/failure indicator (using cursor movement on TTY terminals).
4. `finalize()` erases all raw output and reprints the full response through the markdown renderer, producing clean formatted output.

## Connections

**Depended on by:** Every crate in the workspace (directly or transitively). The `config`, `bus`, `storage`, `providers`, `session`, `tools`, `channels`, `agent`, `cli`, and `klyntbot` crates all import types from `common`.

**Depends on:** No workspace crates. External dependencies include `thiserror` (error derive), `serde`/`serde_json` (serialization), `chrono`/`chrono_tz` (date/time), `crossterm` (terminal control), and `tokio` (async process for notifications).
