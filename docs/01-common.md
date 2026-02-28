# Common

**Path:** `crates/common/`
**Dependency layer:** 0 (foundation -- no workspace dependencies)

---

## Section 1: Narrative Overview

### Purpose

The `common` crate is the foundation layer of the Klyntbot workspace. It sits at Layer 0 of
the dependency graph and defines the types, error hierarchy, and utility functions that every
other crate in the workspace depends on. Its single responsibility is to provide shared
vocabulary -- error variants, domain newtypes, interaction protocols, and terminal rendering --
so that higher-layer crates never need to duplicate these definitions or introduce circular
dependencies.

### Dependency Position

```
 Every crate in the workspace
      |
      v
  +--------+
  | common |   Layer 0 -- depends only on external crates
  +--------+
```

All 21 workspace crates list `common` as a dependency:

| Layer | Crates |
|-------|--------|
| 0 | (common itself) |
| 1 | `config`, `bus` |
| 1.5 | `storage` |
| 2 | `providers`, `session`, `scheduling`, `calendar`, `context_engine` |
| 3 | `tools-core`, `tools`, `feature-todo`, `feature-finance`, `plugin-runtime` |
| 4 | `channels`, `heartbeat` |
| 5 | `agent`, `dashboard` |
| 6 | `cli` |
| 7 | `klyntbot` (facade) |

Because `common` has zero workspace dependencies, adding a type here never risks creating
a dependency cycle.

### Error Strategy

The error system follows a **hierarchical enum** pattern powered by `thiserror`:

```
KlyntbotError (top-level)
 |-- Bus(String)
 |-- BusDisconnected
 |-- Tool(ToolError)        <-- #[from] auto-conversion
 |-- Provider(ProviderError) <-- #[from] auto-conversion
 |-- Channel(ChannelError)   <-- #[from] auto-conversion
 |-- Session(SessionError)   <-- #[from] auto-conversion
 |-- Config(ConfigError)     <-- #[from] auto-conversion
 |-- Cron(String)
 |-- Calendar(String)
 |-- Goal(String)
 |-- Plan(String)
 |-- Storage(String)
 |-- StorageNotFound(String)
 |-- StorageConflict(String)
 |-- Io(std::io::Error)     <-- #[from] auto-conversion
 |-- Json(serde_json::Error) <-- #[from] auto-conversion
```

**Design decisions:**

1. **Domain sub-enums** (`ToolError`, `ProviderError`, `ChannelError`, `SessionError`,
   `ConfigError`) give each subsystem structured variants with meaningful fields
   (e.g., `ProviderError::RateLimited { provider, retry_after }`).

2. **`#[from]` conversions** on the sub-enums let any crate use `?` to propagate domain
   errors into `KlyntbotError` without manual mapping.

3. **String-based variants** (`Cron`, `Calendar`, `Goal`, `Plan`, `Storage`) are used
   for domain areas where a single message string is sufficient and a dedicated sub-enum
   would be over-engineering.

4. **`Result<T>`** is aliased to `std::result::Result<T, KlyntbotError>` and used across
   the entire workspace.

### Newtype Strategy

The `types` module replaces primitive `String` parameters with newtypes:

```
              ChannelName("telegram")
                    |
    SessionKey -----+------ ChatId("123456")
       |
  "telegram:123456"
```

`SessionKey` composes `ChannelName` and `ChatId` with a colon separator and can be split
back into its parts. All three types implement `Display`, `From<String>`, `From<&str>`,
`Serialize`, and `Deserialize`.

`MessageRole` maps the four LLM conversation roles (`system`, `user`, `assistant`, `tool`)
to an enum. Two parsing strategies exist:

- `From<&str>` -- lenient, defaults unknown strings to `User` (with a tracing warning).
- `parse_strict(&str)` -- returns `Err(KlyntbotError::Bus(...))` for unknown strings.

### Interaction Protocol (prompts module)

The `prompts` module defines a protocol for structured user interactions. It exists in
`common` (Layer 0) so that the `ask_user` tool (Layer 3) can construct requests and the
CLI (Layer 6) can render them without a circular dependency.

```
LLM generates JSON arguments
        |
        v
  InteractionRequest { title, questions: Vec<Question> }
        |
        v
  CLI renders tabbed UI  ------>  FormResponse::Completed(Vec<Answer>)
        |                         or FormResponse::Cancelled
        v
  Answer { question_id, value: AnswerValue }
```

Question types: `SingleSelect`, `MultiSelect`, `YesNo`, `FreeText`.
Answer types: `Selected`, `MultiSelected`, `YesNo`, `Text`, `Skipped`.

All types use tagged JSON serialization (`#[serde(tag = "type", rename_all = "snake_case")]`).

### Utility Modules

The `utils` module tree provides shared functions grouped by concern:

```
utils/
  mod.rs           -- re-exports helpers::*, terminal::*, StreamRenderer
  helpers.rs       -- JSON extraction, string truncation, LLM fence stripping
  date.rs          -- timezone-aware date parsing (RFC3339, ISO, natural language)
  notify.rs        -- cross-platform OS notifications (macOS/Linux/Windows)
  stream_renderer.rs -- real-time LLM streaming with post-completion markdown re-render
  terminal/
    mod.rs          -- re-exports all sub-modules
    colors.rs       -- ANSI codes, NO_COLOR detection, display width, status indicators
    tables.rs       -- Unicode-aware table rendering
    markdown.rs     -- Markdown-to-terminal converter
    boxes.rs        -- Box drawing, banner, wizard UI, error display
    spinners.rs     -- Braille spinner (thread-based)
    thinking_renderer.rs -- Pipeline stage tracing with animated spinners
```

### Terminal Rendering Flow

```
User sends message
       |
       v
ThinkingRenderer   -- animated spinner showing pipeline stages
  (classifying -> context -> executing -> tool calls)
       |
       v
StreamRenderer     -- prints raw tokens as they arrive
       |
       v
finalize()         -- erases raw output, re-renders with MarkdownRenderer
       |
       v
MarkdownRenderer   -- converts markdown to ANSI-styled terminal output
  (headers, bold, italic, code blocks, tables, lists, links)
```

All terminal output respects `NO_COLOR` and non-TTY environments by falling back to
plain text and skipping cursor manipulation.

---

## Section 2: API Reference

### Module: `error` (`crates/common/src/error.rs`)

#### `KlyntbotError` (enum, line 7)

Top-level error type for the workspace.

| Variant | Type | `#[from]` | Description |
|---------|------|-----------|-------------|
| `Bus(String)` | `String` | No | Message bus errors |
| `BusDisconnected` | unit | No | Bus channel closed |
| `Tool(ToolError)` | `ToolError` | Yes | Tool execution errors |
| `Provider(ProviderError)` | `ProviderError` | Yes | LLM provider errors |
| `Channel(ChannelError)` | `ChannelError` | Yes | Chat channel errors |
| `Session(SessionError)` | `SessionError` | Yes | Session persistence errors |
| `Config(ConfigError)` | `ConfigError` | Yes | Configuration errors |
| `Cron(String)` | `String` | No | Cron scheduling errors |
| `Calendar(String)` | `String` | No | CalDAV sync errors |
| `Goal(String)` | `String` | No | Goal management errors |
| `Plan(String)` | `String` | No | Plan execution errors |
| `Storage(String)` | `String` | No | Generic storage errors |
| `StorageNotFound(String)` | `String` | No | Entity not found in storage |
| `StorageConflict(String)` | `String` | No | Storage uniqueness conflict |
| `Io(std::io::Error)` | `std::io::Error` | Yes | I/O errors |
| `Json(serde_json::Error)` | `serde_json::Error` | Yes | JSON serialization errors |

#### `ToolError` (enum, line 59)

| Variant | Fields | Description |
|---------|--------|-------------|
| `NotFound(String)` | tool name | Tool not registered |
| `InvalidParams(String)` | message | Bad tool arguments |
| `ExecutionFailed(String)` | message | Runtime failure |
| `PermissionDenied(String)` | message | Access denied |

#### `ProviderError` (enum, line 75)

| Variant | Fields | Description |
|---------|--------|-------------|
| `Http(String)` | message | HTTP transport error |
| `InvalidResponse(String)` | message | Unparseable LLM response |
| `RateLimited { provider, retry_after }` | `String`, `Option<u64>` | Rate limit hit; optional seconds to wait |
| `AuthFailed { provider, config_key }` | `String`, `String` | Authentication failure with config hint |

#### `ChannelError` (enum, line 97)

| Variant | Fields | Description |
|---------|--------|-------------|
| `ConnectionFailed(String)` | message | Failed to connect to platform |
| `SendFailed(String)` | message | Failed to send message |
| `InvalidConfig(String)` | message | Bad channel configuration |

#### `SessionError` (enum, line 110)

| Variant | Fields | `#[from]` | Description |
|---------|--------|-----------|-------------|
| `NotFound(String)` | session id | No | Session does not exist |
| `LoadFailed(String)` | message | No | Deserialization failure |
| `SaveFailed(String)` | message | No | Serialization/write failure |
| `Io(std::io::Error)` | io error | Yes | I/O error |
| `Json(serde_json::Error)` | json error | Yes | JSON error |

#### `ConfigError` (enum, line 129)

| Variant | Fields | `#[from]` | Description |
|---------|--------|-----------|-------------|
| `NotFound(String)` | path | No | Config file missing |
| `Invalid(String)` | message | No | Malformed config |
| `MissingField(String)` | field name | No | Required field absent |
| `Io(std::io::Error)` | io error | Yes | I/O error |
| `Json(serde_json::Error)` | json error | Yes | JSON error |

#### `Result<T>` (type alias, line 147)

```rust
pub type Result<T> = std::result::Result<T, KlyntbotError>;
```

---

### Module: `types` (`crates/common/src/types.rs`)

#### Constants (lines 12-14)

| Constant | Value | Purpose |
|----------|-------|---------|
| `SYSTEM_CHANNEL` | `"system"` | Internal system messages |
| `CLI_CHANNEL` | `"cli"` | CLI-originated messages |
| `TELEGRAM_RESET_SENDER` | `"telegram_reset"` | Telegram session reset marker |

#### `ChannelName` (struct, line 18)

Newtype over `String`. Represents a chat platform name (e.g., `"telegram"`, `"discord"`).

Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(name: impl Into<String>) -> Self` | Construct from any string-like value |
| `as_str` | `fn as_str(&self) -> &str` | Borrow inner string |

Trait impls: `Display`, `From<String>`, `From<&str>`

#### `ChatId` (struct, line 49)

Newtype over `String`. Identifies a specific chat/conversation within a channel.

Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(id: impl Into<String>) -> Self` | Construct from any string-like value |
| `as_str` | `fn as_str(&self) -> &str` | Borrow inner string |

Trait impls: `Display`, `From<String>`, `From<&str>`

#### `SessionKey` (struct, line 82)

Newtype over `String`. Composite key in the format `"channel:chat_id"`.

Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(channel: &ChannelName, chat_id: &ChatId) -> Self` | Build from typed parts |
| `from_parts` | `fn from_parts(channel: &str, chat_id: &str) -> Self` | Build from raw strings |
| `as_str` | `fn as_str(&self) -> &str` | Borrow inner string |
| `split` | `fn split(&self) -> Option<(ChannelName, ChatId)>` | Decompose back into parts; returns `None` if no colon found |

Trait impls: `Display`, `From<String>`, `From<&str>`

#### `MessageRole` (enum, line 128)

Serialized as lowercase (`#[serde(rename_all = "lowercase")]`).

| Variant | Serialized value |
|---------|-----------------|
| `System` | `"system"` |
| `User` | `"user"` |
| `Assistant` | `"assistant"` |
| `Tool` | `"tool"` |

| Method | Signature | Description |
|--------|-----------|-------------|
| `parse_strict` | `fn parse_strict(s: &str) -> Result<Self>` | Strict parsing; returns `Err(KlyntbotError::Bus)` for unknown values |

Trait impls: `Display`, `From<&str>` (lenient -- unknown values default to `User` with `tracing::warn`)

---

### Module: `prompts` (`crates/common/src/prompts.rs`)

All types derive `Debug`, `Clone`, `Serialize`, `Deserialize`.

#### `InteractionRequest` (struct, line 13)

| Field | Type | Description |
|-------|------|-------------|
| `title` | `String` | Title displayed above the tabbed UI |
| `questions` | `Vec<Question>` | 1-4 questions to present simultaneously |

#### `Question` (struct, line 22)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Machine-readable ID for answer correlation |
| `title` | `String` | Short tab header label (recommended 12 chars max) |
| `text` | `String` | Full question text shown when tab is active |
| `answer_type` | `AnswerType` | Expected answer format |

#### `AnswerType` (enum, line 37)

Tagged JSON: `#[serde(tag = "type", rename_all = "snake_case")]`

| Variant | Fields | Description |
|---------|--------|-------------|
| `SingleSelect` | `options: Vec<AnswerOption>` | Pick exactly one |
| `MultiSelect` | `options: Vec<AnswerOption>` | Pick one or more |
| `YesNo` | `default: Option<bool>` | Boolean toggle |
| `FreeText` | `placeholder: Option<String>` | Free-form text input |

#### `AnswerOption` (struct, line 50)

| Field | Type | Description |
|-------|------|-------------|
| `value` | `String` | Machine-readable value returned in answer |
| `label` | `String` | Human-readable display text |
| `description` | `Option<String>` | Optional sub-label |

#### `Answer` (struct, line 61)

| Field | Type | Description |
|-------|------|-------------|
| `question_id` | `String` | Matches `Question.id` |
| `value` | `AnswerValue` | The user's response |

#### `AnswerValue` (enum, line 70)

Tagged JSON: `#[serde(tag = "type", rename_all = "snake_case")]`

| Variant | Fields | Description |
|---------|--------|-------------|
| `Selected` | `value: String` | Single selection |
| `MultiSelected` | `values: Vec<String>` | Multiple selections |
| `YesNo` | `answer: bool` | Boolean response |
| `Text` | `content: String` | Free-form text |
| `Skipped` | (none) | User skipped the question |

#### `FormResponse` (enum, line 86)

| Variant | Fields | Description |
|---------|--------|-------------|
| `Completed(Vec<Answer>)` | answers | User completed all questions |
| `Cancelled` | (none) | User pressed Esc/Ctrl+C |

---

### Module: `entity_card` (`crates/common/src/entity_card.rs`)

#### `EntityCard` (struct, line 10)

Serialized with `#[serde(rename_all = "camelCase")]`.

| Field | Type | Serde behavior | Description |
|-------|------|----------------|-------------|
| `entity_type` | `String` | required | Kind of entity (e.g., `"todo"`, `"project"`) |
| `entity_id` | `String` | required | Unique ID |
| `title` | `String` | required | Display title |
| `subtitle` | `Option<String>` | required | Secondary text |
| `route` | `Option<String>` | required | Dashboard route |
| `icon_hint` | `String` | required | Icon identifier for UI rendering |
| `metadata` | `HashMap<String, Value>` | `skip_serializing_if = "is_empty"` | Arbitrary key-value metadata |

Derives: `Debug`, `Clone`, `Serialize`

---

### Module: `utils::helpers` (`crates/common/src/utils/helpers.rs`)

Re-exported at `common::utils::*` level.

| Function | Signature | Description |
|----------|-----------|-------------|
| `extract_json_array` | `fn extract_json_array(s: &str) -> &str` | Finds the first `[` and last `]` in a string containing prose/markdown. Falls back to full input if no match. |
| `extract_json_object` | `fn extract_json_object(s: &str) -> Option<&str>` | Finds the first `{` and last `}`. Returns `None` if no matching pair. |
| `strip_llm_fences` | `fn strip_llm_fences(s: &str) -> &str` | Removes ` ```json ` and ` ``` ` fences from LLM output. Zero-allocation (returns slice). |
| `truncate_at_boundary` | `fn truncate_at_boundary(s: &str, max_bytes: usize) -> &str` | Truncates at a valid UTF-8 char boundary. Returns original if already short enough. |
| `truncate_chars` | `fn truncate_chars(s: &str, max_chars: usize, suffix: &str) -> String` | Truncates to `max_chars` Unicode scalars, appending `suffix` if cut. |
| `tool_def_name` | `fn tool_def_name(def: &serde_json::Value) -> Option<&str>` | Extracts function name from OpenAI-style `{"type":"function","function":{"name":"..."}}` tool definition. |
| `format_timestamp_ms` | `fn format_timestamp_ms(ms: i64) -> String` | Converts millisecond timestamp to RFC 3339 string. Falls back to `Utc::now()` on invalid input. |

---

### Module: `utils::date` (`crates/common/src/utils/date.rs`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `parse_datetime` | `fn parse_datetime(s: &str, fallback_tz: &str) -> Option<DateTime<Utc>>` | Multi-format parser. Accepts RFC 3339, ISO datetime, `"YYYY-MM-DD HH:MM[:SS]"`, date-only, and natural language (`"today"`, `"tomorrow"`, `"yesterday"`, `"next monday"`, `"in 3 days"`, `"in 2 weeks"`). Non-timezone strings are interpreted in `fallback_tz` (IANA name). Returns `None` for empty/unparseable input. |
| `format_datetime_local` | `fn format_datetime_local(dt: &DateTime<Utc>, timezone: &str, fmt: &str) -> String` | Formats a UTC datetime in the given timezone using a `chrono` format string. Falls back to UTC if timezone is invalid. |
| `timezone_utc_offset` | `fn timezone_utc_offset(timezone: &str) -> String` | Returns current UTC offset string (e.g., `"+07:00"`). Uses live time for DST awareness. Returns `"+00:00"` for invalid timezone. |

Parse priority order:

1. RFC 3339 with timezone (e.g., `"2026-02-17T21:00:00+07:00"`)
2. ISO datetime without timezone (e.g., `"2026-02-17T21:00:00"`)
3. `"YYYY-MM-DD HH:MM:SS"` space-separated
4. `"YYYY-MM-DD HH:MM"` space-separated
5. Date only `"YYYY-MM-DD"` (midnight in fallback timezone)
6. Natural language relative dates

---

### Module: `utils::notify` (`crates/common/src/utils/notify.rs`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `send_os_notification` | `async fn send_os_notification(title: &str, body: &str) -> Result<()>` | Sends a native OS notification. Platform-specific: macOS uses `osascript`, Linux uses `notify-send`, Windows uses PowerShell toast. Unsupported platforms get a silent no-op. Input is sanitized against shell injection. |

Internal sanitization helpers (not public):

- `sanitize_for_applescript` (macOS) -- escapes `\` and `"`, strips control characters.
- `sanitize_for_powershell` (Windows) -- doubles `'`, strips control characters.

---

### Module: `utils::stream_renderer` (`crates/common/src/utils/stream_renderer.rs`)

Re-exported as `common::utils::StreamRenderer`.

#### `StreamRenderer` (struct, line 36)

Renders streamed LLM output token-by-token, then erases and re-renders with markdown
formatting on completion. Handles TTY vs. non-TTY graceful degradation.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Create renderer; auto-detects TTY and terminal width |
| `on_content_chunk` | `fn on_content_chunk(&mut self, chunk: &str)` | Append and print a streaming content token |
| `on_tool_start` | `fn on_tool_start(&mut self, name: &str, args: &Value)` | Print tool invocation with formatted args |
| `on_tool_end` | `fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64)` | Update tool status line in-place (TTY) or print result (non-TTY) |
| `on_iteration_start` | `fn on_iteration_start(&mut self, iteration: usize, _max: usize)` | No-op (suppressed for clean output) |
| `pause` | `fn pause(&mut self)` | Pause rendering for interactive prompts |
| `resume` | `fn resume(&mut self, prompt_lines: u16)` | Resume rendering after interactive prompt |
| `mark_cancelled` | `fn mark_cancelled(&mut self)` | Flag response as cancelled (Ctrl+C) |
| `finalize` | `fn finalize(&mut self) -> String` | Erase raw output, return markdown-rendered string |
| `elapsed_secs` | `fn elapsed_secs(&self) -> f64` | Seconds since renderer creation |
| `draw_separator` | `fn draw_separator(label: Option<&str>) -> String` | Static method; draws horizontal rule with optional label |

Implements `Default`.

---

### Module: `utils::terminal::colors` (`crates/common/src/utils/terminal/colors.rs`)

#### ANSI Constants (lines 12-61)

| Constant | Code | Description |
|----------|------|-------------|
| `RESET` | `\x1b[0m` | Reset all formatting |
| `PROMPT` | `\x1b[2;34m` | Dim blue |
| `HEADER` | `\x1b[2;37m` | Dim white |
| `TOOL` | `\x1b[36m` | Cyan |
| `SUCCESS` | `\x1b[32m` | Green |
| `ERROR` | `\x1b[31m` | Red |
| `WARNING` | `\x1b[33m` | Yellow |
| `DIM` | `\x1b[90m` | Gray |
| `SEPARATOR` | `\x1b[2;90m` | Dim gray |
| `BRAND` | `\x1b[38;5;208m` | Orange (theme color) |
| `HIGHLIGHT` | `\x1b[38;5;214m` | Bright orange |
| `INFO` | `\x1b[34m` | Blue |
| `ACCENT` | `\x1b[35m` | Magenta |
| `BOLD` | `\x1b[1m` | Bold weight |
| `ITALIC` | `\x1b[3m` | Italic style |
| `UNDERLINE` | `\x1b[4m` | Underline style |
| `STRIKETHROUGH` | `\x1b[9m` | Strikethrough style |

#### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `colors_enabled` | `fn colors_enabled() -> bool` | Returns `false` if `NO_COLOR` is set or stdout is not a TTY |
| `color` | `fn color(code: &str) -> &str` | Returns the ANSI code if colors enabled, empty string otherwise |
| `colorize` | `fn colorize(text: &str, code: &str) -> String` | Wraps text in `{code}{text}{RESET}` if colors enabled |
| `display_width` | `fn display_width(s: &str) -> usize` | Visible column width, skipping ANSI escapes, using Unicode widths |
| `pad_to_width` | `fn pad_to_width(s: &str, target: usize) -> String` | Right-pads string to target display width |

#### Status Indicator Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `status_success()` | `String` | Green checkmark `"checkmark"` |
| `status_disabled()` | `String` | Gray circle `"circle"` |
| `status_error()` | `String` | Red X `"cross"` |
| `status_warning()` | `String` | Yellow exclamation |
| `status_progress()` | `String` | Orange arrow |
| `status_active()` | `String` | Orange filled circle |

#### `BoxChars` (struct, line 169)

| Field | Type | Description |
|-------|------|-------------|
| `top_left` | `&'static str` | Top-left corner |
| `top_right` | `&'static str` | Top-right corner |
| `bottom_left` | `&'static str` | Bottom-left corner |
| `bottom_right` | `&'static str` | Bottom-right corner |
| `horizontal` | `&'static str` | Horizontal line segment |
| `vertical` | `&'static str` | Vertical line segment |
| `horizontal_down` | `&'static str` | T-junction pointing down |
| `horizontal_up` | `&'static str` | T-junction pointing up |
| `vertical_right` | `&'static str` | T-junction pointing right |

| Associated constant / method | Description |
|-------------------------------|-------------|
| `BoxChars::UNICODE` | Unicode box-drawing characters (`+`, `|`, etc.) |
| `BoxChars::ASCII` | ASCII fallback (`+`, `-`, `\|`) |
| `BoxChars::get() -> &'static BoxChars` | Returns `UNICODE` if colors enabled, `ASCII` otherwise |

---

### Module: `utils::terminal::tables` (`crates/common/src/utils/terminal/tables.rs`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `draw_table` | `fn draw_table(headers: &[&str], rows: &[Vec<String>]) -> String` | Renders a bordered table with Unicode-aware column alignment. Columns auto-size to content. Returns empty string for empty headers. |

---

### Module: `utils::terminal::markdown` (`crates/common/src/utils/terminal/markdown.rs`)

#### `MarkdownRenderer` (struct, line 8)

Stateless converter from markdown text to ANSI-styled terminal output.

| Method | Signature | Description |
|--------|-----------|-------------|
| `render` | `fn render(markdown: &str) -> String` | Converts full markdown document. Handles: headers (bold, underline for H1/H2), bold (`**`), italic (`*`/`_`), inline code, fenced code blocks (delegated to `draw_code_block`), strikethrough (`~~`), links (`[text](url)`), blockquotes (`>`), unordered lists (`-`/`*`), ordered lists, and tables (delegated to `draw_table`). |

Private helpers: `render_inline`, `flush_table`, `parse_ordered_list`.

---

### Module: `utils::terminal::boxes` (`crates/common/src/utils/terminal/boxes.rs`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `draw_banner` | `fn draw_banner(model: &str) -> String` | Startup banner with ASCII logo, model name, and usage tips. Falls back to plain text in non-TTY. |
| `draw_step_progress` | `fn draw_step_progress(current: usize, total: usize) -> String` | Wizard progress bar with colored circles and connectors. |
| `draw_wizard_step_header` | `fn draw_wizard_step_header(current: usize, total: usize, title: &str) -> String` | Full step header with progress bar, step number, and title. |
| `draw_step_line` | `fn draw_step_line(text: &str) -> String` | Content line prefixed with branded vertical bar. |
| `draw_step_footer` | `fn draw_step_footer() -> String` | Vertical line connector to next step. |
| `draw_box` | `fn draw_box(content: &str, header: Option<&str>) -> String` | Wraps multiline text in a Unicode/ASCII box with optional header. |
| `draw_code_block` | `fn draw_code_block(code: &str, language: Option<&str>) -> String` | Code block rendering (delegates to `draw_box` with language as header). |
| `display_error` | `fn display_error(title: &str, problem: &str, fix_steps: &[&str], docs: Option<&str>) -> String` | Structured error display with title, problem, numbered fix steps, and optional docs link. |

---

### Module: `utils::terminal::spinners` (`crates/common/src/utils/terminal/spinners.rs`)

#### `Spinner` (struct, line 14)

Thread-based braille spinner for indicating long-running operations. Uses 8 braille
animation frames at 100ms intervals. Auto-stops on `Drop`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(message: impl Into<String>) -> Self` | Create with display message |
| `start` | `fn start(&mut self)` | Spawn animation thread |
| `stop` | `fn stop(&mut self)` | Stop animation and clear line |
| `set_message` | `fn set_message(&mut self, message: impl Into<String>)` | Update spinner message |

Implements `Drop` (calls `stop`).

---

### Module: `utils::terminal::thinking_renderer` (`crates/common/src/utils/terminal/thinking_renderer.rs`)

#### `ThinkingRenderer` (struct, line 30)

Renders pipeline execution stages with animated spinners. Supports three modes:
- **Normal TTY:** Single animated line that updates in-place.
- **Verbose TTY:** Multi-line trace with checkmarks and durations.
- **Non-TTY:** Static lines (no animation).

| Public field | Type | Description |
|--------------|------|-------------|
| `verbose` | `bool` | Verbose mode flag |
| `rendered_lines` | `u16` | Total lines printed (for collapse) |
| `tool_count` | `usize` | Number of tools executed |
| `iteration_count` | `usize` | Current iteration number |

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(verbose: bool, is_tty: bool) -> Self` | Create renderer with mode flags |
| `set_spinner` | `fn set_spinner(&mut self, message: impl Into<String>)` | Set spinner message (DIM color, 2-space indent) |
| `tick` | `fn tick(&mut self)` | Advance spinner one frame. Call at ~80ms intervals. No-op when idle. |
| `is_spinning` | `fn is_spinning(&self) -> bool` | Whether a spinner is active |
| `on_classification_complete` | `fn on_classification_complete(&mut self, strategy: &str, confidence: f32, source: &str, duration_ms: u64)` | Log classification result |
| `on_context_assembled` | `fn on_context_assembled(&mut self, total_tokens: usize, budget: usize, duration_ms: u64)` | Log context assembly |
| `on_execution_started` | `fn on_execution_started(&mut self, engine: &str, max_iterations: usize)` | Log engine startup |
| `on_iteration_start` | `fn on_iteration_start(&mut self, iteration: usize, max: usize)` | Log iteration begin |
| `on_tool_start` | `fn on_tool_start(&mut self, name: &str)` | Log tool invocation |
| `on_tool_end` | `fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64)` | Log tool completion |
| `collapse` | `fn collapse(&mut self)` | Erase all thinking trace lines from terminal |
| `separator_label` | `fn separator_label(&self, model: &str, elapsed_secs: f64) -> String` | Build summary string (e.g., `"o4-mini . 1.2s, 2 tools, 3 iters"`) |

---

### Root Re-exports (`crates/common/src/lib.rs`)

The crate root re-exports the most frequently used items for ergonomic imports:

```rust
// Error types
pub use error::{ChannelError, ConfigError, KlyntbotError, ProviderError, Result, SessionError, ToolError};

// Domain types
pub use types::{ChannelName, ChatId, MessageRole, SessionKey, CLI_CHANNEL, SYSTEM_CHANNEL, TELEGRAM_RESET_SENDER};

// Interaction protocol
pub use prompts::{Answer, AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question};

// Entity card
pub use entity_card::EntityCard;
```

This allows consuming crates to write `use common::{Result, KlyntbotError, ChannelName}` without
navigating sub-modules.
