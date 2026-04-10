# Channels -- Platform Integration Layer

The `channels` crate (layer 5) defines how Klyntbot connects to external messaging platforms. It provides a single `Channel` trait that all adapters implement, a `ChannelManager` that orchestrates lifecycle and outbound dispatch, and shared utilities for reconnection, typing indicators, message formatting, and structured interactions.

Related docs: [core-infrastructure.md](core-infrastructure.md), [agent-runtime.md](agent-runtime.md)

---

## Channel Trait

Every platform adapter implements this trait, defined in `crates/channels/src/lib.rs`:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;

    // Default no-ops:
    async fn send_typing(&self, _chat_id: &str) -> Result<()> { Ok(()) }
    fn supports_interaction(&self) -> bool { false }
    async fn send_interaction(&self, _chat_id: &str, _request: &InteractionRequest) -> Result<FormResponse> { Err(...) }
}
```

`DynChannel = Arc<dyn Channel>` is the type-erased handle stored by `ChannelManager`.

Key design decisions:

- `start()` is long-running (blocking). The manager spawns each channel in its own `tokio::spawn` task.
- `stop()` flips an `AtomicBool` flag. The channel's internal loop checks this flag and exits.
- `send()` handles formatting and chunking internally (see Message Splitting below).
- `is_allowed()` delegates to a shared `check_allowlist()` helper that supports exact match and compound IDs (e.g. `"123456|username"` matches allowlist entry `"123456"` or `"username"`).
- `send_interaction()` is opt-in. Only channels with native UI elements (buttons, select menus) override `supports_interaction()` to return `true`.

---

## Message Types

Defined in `crates/bus/src/events.rs`:

### InboundMessage

| Field       | Type                             | Description                                            |
|-------------|----------------------------------|--------------------------------------------------------|
| `channel`   | `ChannelName` (String)           | Source channel name (`"telegram"`, `"discord"`, etc.)  |
| `sender_id` | `String`                         | User identifier (platform-specific)                    |
| `chat_id`   | `ChatId` (String)                | Chat/channel/thread identifier                         |
| `content`   | `String`                         | Message text (max `MAX_MESSAGE_SIZE` = 64 KB)          |
| `timestamp` | `DateTime<Utc>`                  | Defaults to `Utc::now()`                               |
| `media`     | `Vec<String>`                    | Local file paths for downloaded attachments             |
| `metadata`  | `HashMap<String, serde_json::Value>` | Channel-specific data (thread_ts, message_id, etc.) |
| `kind`      | `MessageKind`                    | `Text` (default), `Reaction`, or `Voice`               |

Session routing key: `"{channel}:{chat_id}"` via `InboundMessage::session_key()`.

### OutboundMessage

| Field       | Type                             | Description                           |
|-------------|----------------------------------|---------------------------------------|
| `channel`   | `ChannelName`                    | Target channel name                   |
| `chat_id`   | `ChatId`                         | Target chat/channel identifier        |
| `content`   | `String`                         | Markdown text (formatted per-channel) |
| `reply_to`  | `Option<String>`                 | Message ID to reply to                |
| `media`     | `Vec<String>`                    | Media URLs to attach                  |
| `metadata`  | `HashMap<String, serde_json::Value>` | Channel-specific data             |

---

## Channel Comparison

| Feature                | Telegram              | Discord                 | Slack                      | Email                   |
|------------------------|-----------------------|-------------------------|----------------------------|-------------------------|
| **Transport**          | HTTP long polling     | WebSocket Gateway v10   | Socket Mode WebSocket      | IMAP polling + SMTP     |
| **Auth**               | Bot API token         | Bot token + intents     | Bot token + App token      | IMAP/SMTP credentials   |
| **Interactions**       | Inline keyboards      | Buttons + select menus  | Block Kit (buttons/selects)| None                    |
| **Media (inbound)**    | Photo, voice, audio, documents | Attachments (20 MB limit) | N/A                  | N/A                     |
| **Voice transcription**| Groq (optional)       | N/A                     | N/A                        | N/A                     |
| **Message limit**      | 4,096 chars           | 2,000 chars             | 8,000 chars                | 8,000 chars             |
| **Typing indicator**   | 4s interval           | 8s interval             | N/A                        | N/A                     |
| **Formatting**         | Markdown to HTML      | Passthrough (native MD) | Markdown to mrkdwn         | Markdown to plain text  |
| **Reactions (inbound)**| Emoji reactions       | Unicode + custom emoji  | Slack reaction names       | N/A                     |
| **Feature gate**       | Always                | Always                  | Always                     | `email` feature flag    |

---

## Telegram

**File:** `crates/channels/src/adapters/telegram.rs`

Direct HTTP calls to the Bot API. No teloxide dependency.

### Connection lifecycle

1. `start()` calls `poll_updates()` which long-polls `getUpdates` with a 30-second timeout.
2. Each poll returns an array of updates. The adapter advances `offset` to acknowledge processed updates.
3. On poll error, waits 5 seconds before retrying.

### Inbound message handling

Updates are dispatched by type:

- **`callback_query`** -- button press from inline keyboard. Acknowledged via `answerCallbackQuery`, then resolved through `InteractionTracker`.
- **`message_reaction`** -- emoji reaction. Published as `MessageKind::Reaction`.
- **`message`** -- text, photo, voice, audio, or document. Commands (`/start`, `/help`, `/reset`) are handled inline.

Compound sender IDs are built as `"{user_id}|{username}"` when a username is available.

### Media handling

Photos, voice messages, audio, and documents are downloaded via `getFile` + HTTP download to `{data_dir}/media/`. Voice messages are optionally transcribed using Groq's transcription API. Content includes descriptors like `[image: /path]`, `[transcription: text]`, `[voice: /path]`.

### Outbound

Messages are formatted as HTML (`TelegramFormatter` converts markdown to Telegram HTML), split into chunks of 4,096 characters, and sent via `sendMessage` with `parse_mode: "HTML"`. If HTML parsing fails on a chunk, it falls back to plain text. A 100ms delay separates multi-chunk sends to avoid rate limiting.

### Retry logic

`api_call_with_retry()` retries up to 3 times with exponential backoff (1s, 2s, 4s). Proxy support is configurable via `TelegramConfig.proxy`.

---

## Discord

**File:** `crates/channels/src/adapters/discord.rs`

Raw WebSocket Gateway connection. No serenity dependency.

### Connection lifecycle

1. `start()` wraps `WebSocketManager::run()` in `reconnect_loop()` for automatic reconnection.
2. The `WsHandler` implementation uses `HeartbeatStrategy::None` because Discord manages its own heartbeat:
   - On `op 10` (HELLO): extract `heartbeat_interval`, spawn a dedicated heartbeat task sending `op 1` at that interval, then send `op 2` (IDENTIFY) with bot token and intents.
   - The heartbeat task tracks the gateway sequence number (`seq`) for proper resumption.
3. On `op 11` (HEARTBEAT ACK): no action needed (logged at debug level).
4. On `op 0` (DISPATCH): route by event type.

### Gateway event routing

| Event                   | Handler                       | Action                                |
|-------------------------|-------------------------------|---------------------------------------|
| `READY`                 | Logged                        | Confirms successful connection        |
| `MESSAGE_CREATE`        | `handle_message_create()`     | Publish `InboundMessage`              |
| `MESSAGE_REACTION_ADD`  | `handle_reaction_add()`       | Publish as `MessageKind::Reaction`    |
| `INTERACTION_CREATE`    | `handle_interaction_create()` | Resolve pending `InteractionTracker`  |

Bot messages are filtered by checking `author.bot == true`.

### Attachments

Attachments are downloaded to `{data_dir}/media/` with filenames formatted as `{attachment_id}_{sanitized_filename}`. Attachments exceeding `MAX_ATTACHMENT_BYTES` (20 MB) are skipped with a `[too large]` placeholder.

### Outbound

Messages are sent via REST (`POST /channels/{id}/messages`). Discord supports native markdown, so the `PassthroughFormatter` is used (no conversion). Messages are split at 2,000 characters. Rich embeds and file uploads go through `multipart/form-data`.

### Interactions

Buttons and select menus use Discord's component system. Callback data is encoded as `"askuser:{channel_id}:{question_id}:{value}"`. `INTERACTION_CREATE` events are acknowledged with type 6 (`DEFERRED_UPDATE_MESSAGE`).

---

## Slack

**File:** `crates/channels/src/adapters/slack.rs`

Socket Mode WebSocket connection. No webhook endpoint required.

### Connection lifecycle

1. `start()` authenticates via `auth.test` to discover the bot's user ID (used to filter self-messages and detect mentions).
2. Obtains a Socket Mode WebSocket URL via `apps.connections.open` (uses the App token, not the Bot token).
3. Wraps `WebSocketManager::run()` in `reconnect_loop()`. A fresh socket URL is requested on each reconnection attempt.
4. Uses `HeartbeatStrategy::Timeout` with a 35-second timeout sending WebSocket pings.

### Envelope protocol

All Slack Socket Mode messages arrive as envelopes with an `envelope_id`. The adapter immediately sends an ACK for every envelope:

```json
{ "envelope_id": "<id>" }
```

Envelope types handled:

- **`events_api`** -- contains a nested event payload. Routed by `event.type`:
  - `message` -- user text message (subtype messages are ignored as system/bot messages).
  - `app_mention` -- direct @mention of the bot in a channel.
  - `reaction_added` -- emoji reaction on a message.
- **`interactive`** -- `block_actions` payloads from button presses and select menu selections. Resolved through `InteractionTracker`.

### Channel/DM routing

In channels and groups, the bot only responds to:
- Direct `@mentions` (`app_mention` events).
- Messages containing `<@{bot_user_id}>`.

In DMs (`channel_type == "im"`), all messages are processed. Bot mention text (`<@UXXXXXX>`) is stripped from the content before publishing.

### Acknowledgment feedback

On receiving a message, the bot adds an `:eyes:` reaction as a visual acknowledgment (best-effort, failures are logged but not propagated).

### Outbound

Messages are formatted via `SlackFormatter` (markdown bold `**x**` becomes `*x*`, links become `<url|text>`, headers become bold). Thread context is preserved: non-DM messages include `thread_ts` for threading. Messages are split at 8,000 characters.

### Interactions

Uses Block Kit. Buttons encode `action_id` as `"askuser:{channel}:{question_id}:{value}"`. Select menus use `"askuser:{channel}:{question_id}"` with the value in `selected_option.value`.

---

## Email

**File:** `crates/channels/src/adapters/email.rs`

Feature-gated behind `email` (on by default). Uses `async-imap` for inbound, `lettre` for outbound.

### Privacy and consent

The email channel requires explicit consent (`config.channels.email.consentGranted = true`). Without this flag, `start()` returns an error explaining the privacy implications.

### Inbound (IMAP)

1. Connects via TCP with optional TLS (`imap_use_ssl`, default true).
2. Logs in and selects the configured mailbox (default: `INBOX`).
3. Searches for `UNSEEN` messages.
4. For each message:
   - Deduplicates by UID (in-memory `HashSet`, cleared at 10,000 entries).
   - Parses with `mail-parser`. Extracts sender, subject, message ID, date.
   - Prefers `text/plain` body; falls back to `text/html` converted via `html2text`.
   - Truncates body at `max_body_chars` (default: 12,000).
   - Publishes to bus with email-specific metadata (message_id, subject, date, sender_email, uid).
   - Optionally marks as `\Seen` (`mark_seen`, default true).
5. Polls at `poll_interval_seconds` (default: 30, minimum: 5).

### Outbound (SMTP)

- Auto-reply is controlled by `auto_reply_enabled` (default: true).
- Subject uses the last received subject for that sender, prefixed with `"Re: "` (configurable via `subject_prefix`). No double `Re:` prefix.
- Threading via `In-Reply-To` and `References` headers when a prior message ID is available.
- SMTP send happens on `spawn_blocking` (lettre's `SmtpTransport` is synchronous).
- Messages are formatted as plain text and split at 8,000 characters.

### Configuration defaults

| Field                  | Default       |
|------------------------|---------------|
| `imap_port`            | 993           |
| `smtp_port`            | 587           |
| `imap_mailbox`         | `INBOX`       |
| `imap_use_ssl`         | true          |
| `smtp_use_tls`         | true          |
| `consent_granted`      | false         |
| `auto_reply_enabled`   | true          |
| `max_body_chars`       | 12,000        |
| `mark_seen`            | true          |
| `poll_interval_seconds`| 30            |

---

## Channel Manager

**File:** `crates/channels/src/manager.rs`

Orchestrates channel lifecycle and outbound message routing.

### Initialization

`ChannelManager::new()` takes ownership of the `MessageBus` outbound receiver (`bus.take_outbound_rx()`). This is a one-shot operation -- calling `new()` twice panics.

`initialize_channels()` uses the `init_channel!` macro, which for each channel:
1. Checks `config.enabled`.
2. Constructs the adapter (e.g. `TelegramChannel::new()`).
3. Wraps in `Arc<dyn Channel>` and inserts into the channel map.

### Outbound dispatch (per-channel fan-out)

`start_all()` creates an isolated `mpsc::channel(32)` queue per channel. A dispatcher task reads from the shared `outbound_rx` and routes messages to the appropriate per-channel queue based on `msg.channel`. This isolation prevents a slow or stuck channel from blocking delivery to other channels.

Error handling on send failure:
1. Log the error.
2. Attempt to send a user-facing error message back to the same chat (with a human-readable description).
3. If the error feedback also fails, log and give up.

User-facing error messages are intentionally vague (no stack traces, no internal details). Rate-limit errors (429) get a specific "please wait" message.

### Lifecycle

```
ChannelManager::new(config, bus)
    |
    v
initialize_channels()      -- create adapters for enabled channels
    |
    v
start_all()                -- spawn channel tasks + outbound dispatcher
    |                         (blocks until all tasks exit)
    v
stop_all()                 -- set running=false on all channels
```

---

## Message Flow

```
External Platform (Telegram/Discord/Slack/Email)
  |
  | (long poll / WebSocket / IMAP poll)
  v
Channel Adapter
  |
  | InboundMessage
  v
MessageBus.inbound_tx  ------>  AgentLoop (subscribes to inbound)
                                  |
                                  | (LLM processing, tool calls)
                                  |
                                  v
                                OutboundMessage
                                  |
MessageBus.outbound_tx  <---------+
  |
  v
ChannelManager dispatcher
  |
  | (routes by msg.channel)
  v
Per-channel mpsc queue
  |
  v
Channel.send()
  |
  | (format, split, API call)
  v
External Platform
```

---

## WebSocket Manager

**File:** `crates/channels/src/ws_manager.rs`

Generic WebSocket lifecycle manager shared by Discord and Slack. Eliminates duplicated connect/heartbeat/read-loop/shutdown boilerplate.

### WsHandler trait

Channels implement `WsHandler` to inject protocol-specific logic:

```rust
#[async_trait]
pub trait WsHandler: Send + Sync {
    async fn on_connected(&self, write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>>;
    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool>;
    async fn on_disconnected(&self) {}
}
```

- `on_connected()` is called after the TCP+TLS handshake. Used for protocol handshakes (Discord IDENTIFY). Can override the heartbeat strategy.
- `on_text_message()` handles each incoming text frame. Returns `Ok(true)` to continue, `Ok(false)` to disconnect.
- `on_disconnected()` is called on connection drop for cleanup.

### HeartbeatStrategy

| Variant                | Used by  | Behavior                                                      |
|------------------------|----------|---------------------------------------------------------------|
| `Timeout { timeout, build_payload }` | Slack | Sends `build_payload()` if no message received within `timeout`. |
| `None`                 | Discord  | No automatic heartbeat. Channel manages its own heartbeat task. |

### Session lifecycle

`WebSocketManager::run()` handles a single session:

1. Connect with configurable timeout (default: 30s).
2. Call `handler.on_connected()`.
3. Enter read loop with the active heartbeat strategy.
4. On exit (error, server close, or shutdown flag): send a graceful close frame.
5. Call `handler.on_disconnected()`.

Callers wrap `run()` in `reconnect_loop()` for automatic reconnection with 5-second backoff.

---

## Shared Utilities

### reconnect_loop

**File:** `crates/channels/src/lib.rs`

```rust
async fn reconnect_loop<F, Fut>(name: &str, running: &Arc<AtomicBool>, connect: F)
```

Generic reconnection wrapper. Calls `connect()` in a loop. On error, logs and waits 5 seconds before retrying. Exits when `running` is set to `false`.

### TypingManager

**File:** `crates/channels/src/shared/typing.rs`

Manages per-chat typing indicator background tasks. Each channel provides its own send function and interval:

- Telegram: 4-second interval (typing expires after ~5s).
- Discord: 8-second interval (typing expires after ~10s).

`start(chat_id, interval, send_fn)` spawns a repeating task. `stop(chat_id)` aborts it. Starting a new typing task for the same chat automatically aborts the previous one.

### InteractionTracker

**File:** `crates/channels/src/shared/interaction.rs`

Thread-safe interaction state tracking using `DashMap`. Manages pending callbacks for structured interactions (button presses, free-text replies).

Interaction flow:
1. `send_interaction()` sends the UI (inline keyboard, Block Kit blocks, etc.).
2. Registers a `PendingCallback` via `wait_for_callback()` or `wait_for_free_text()`.
3. Blocks on a `oneshot::channel` with a 5-minute timeout.
4. Inbound event handler calls `resolve_single()` or `resolve_free_text()` to complete the callback.

Callback keys are formatted as `"{chat_id}:{question_id}"`. For free-text interactions, `find_free_text_key(chat_id)` scans for any pending free-text callback in that chat, allowing inbound message handlers to intercept ordinary messages as interaction responses.

### ChannelFormatter

**File:** `crates/channels/src/formatter.rs`

Per-channel markdown conversion. `formatter_for(channel_name)` returns a static formatter:

| Channel   | Formatter            | Strategy                                      |
|-----------|----------------------|-----------------------------------------------|
| Telegram  | `TelegramFormatter`  | Markdown to HTML (`<b>`, `<code>`, `<pre>`)   |
| Discord   | `PassthroughFormatter` | No conversion (native markdown support)     |
| Slack     | `SlackFormatter`     | Markdown to mrkdwn (`*bold*`, `<url\|text>`)  |
| Email     | `PlainTextFormatter` | Strip all markdown to plain text              |

The Telegram formatter protects code blocks and inline code from HTML escaping using sentinel sequences, then restores them with proper `<code>`/`<pre>` tags.

### Message splitting

**File:** `crates/channels/src/utils.rs`

`split_message(text, limit)` splits long messages at natural boundaries, in priority order:

1. Paragraph breaks (double newlines)
2. Line breaks (single newlines)
3. Sentence breaks (`. `, `! `, `? `)
4. Word breaks (spaces)
5. Hard character split (last resort, respects UTF-8 boundaries)

Per-channel limits: Telegram 4,096, Discord 2,000, Slack 8,000, Email 8,000, default 4,000.

---

## Adding a New Channel

1. Create `crates/channels/src/adapters/{name}.rs` implementing the `Channel` trait.
2. Add `pub mod {name}` and `pub use {name}::{Name}Channel` in `crates/channels/src/adapters/mod.rs`.
3. Add the config struct in `crates/config/src/schema/` (must include `enabled: bool` and `allow_from: Vec<String>`).
4. Add `init_channel!` call in `ChannelManager::initialize_channels()`.
5. Add formatting entry in `formatter_for()` and message limit in `max_length()`.
6. For WebSocket-based channels: implement `WsHandler` and use `WebSocketManager` + `reconnect_loop()`.
7. For interaction support: override `supports_interaction()`, implement `send_interaction()`, and use `InteractionTracker`.

---

## Key Files

| File                                          | Purpose                                     |
|-----------------------------------------------|---------------------------------------------|
| `crates/channels/src/lib.rs`                  | `Channel` trait, `check_allowlist`, `reconnect_loop` |
| `crates/channels/src/manager.rs`              | `ChannelManager`, `init_channel!` macro     |
| `crates/channels/src/ws_manager.rs`           | `WebSocketManager`, `WsHandler`, heartbeat  |
| `crates/channels/src/adapters/telegram.rs`    | Telegram Bot API adapter                    |
| `crates/channels/src/adapters/discord.rs`     | Discord Gateway adapter                     |
| `crates/channels/src/adapters/slack.rs`       | Slack Socket Mode adapter                   |
| `crates/channels/src/adapters/email.rs`       | Email IMAP/SMTP adapter                     |
| `crates/channels/src/formatter.rs`            | Per-channel markdown formatting             |
| `crates/channels/src/utils.rs`                | Message splitting with boundary detection   |
| `crates/channels/src/shared/typing.rs`        | `TypingManager` for typing indicators       |
| `crates/channels/src/shared/interaction.rs`   | `InteractionTracker` for structured UI      |
| `crates/bus/src/events.rs`                    | `InboundMessage`, `OutboundMessage`, `MessageKind` |

---

*Related docs: [Core Infrastructure](core-infrastructure.md) | [Agent Runtime](agent-runtime.md) | [Features](features.md)*
