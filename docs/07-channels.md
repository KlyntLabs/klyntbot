# Channels

Crate path: `crates/channels/`
Layer: 4 (depends on `common`, `bus`, `config`, `providers`)

---

## Section 1: Narrative Overview

### What channels are and why they exist

A channel is an adapter that bridges an external chat platform to Klyntbot's internal message bus. Each channel handles the platform-specific protocol (HTTP polling, WebSocket gateway, IMAP/SMTP) while exposing a uniform `Channel` trait to the rest of the system. This decouples the agent loop, session management, and tool execution from any particular chat platform. Six implementations ship today: Telegram, Discord, WhatsApp, Slack, Email, and QQ.

### The Channel trait

The `Channel` trait (`src/lib.rs:38-77`) defines the surface every platform adapter must implement:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;

    // Default no-ops / stubs:
    async fn send_typing(&self, _chat_id: &str) -> Result<()>;
    fn supports_interaction(&self) -> bool;
    async fn send_interaction(&self, _chat_id: &str, _request: &InteractionRequest) -> Result<FormResponse>;
}
```

`start()` is a long-running task. It receives a shared `MessageBus` and publishes `InboundMessage` values when users send messages. `send()` delivers an `OutboundMessage` that arrives from the agent via the bus's outbound channel. `send_typing()` and `send_interaction()` have default implementations (no-op and error respectively) so channels can opt in.

`DynChannel` (`src/lib.rs:80`) is the type alias `Arc<dyn Channel>` used throughout the manager.

### ChannelManager -- initialization, lifecycle, message routing

`ChannelManager` (`src/manager.rs`) orchestrates every enabled channel. Construction requires an `Arc<Config>` and an `Arc<MessageBus>`. The constructor takes ownership of the bus's outbound receiver (`bus.take_outbound_rx()`), which means only one `ChannelManager` can exist per bus instance. A second instantiation returns `ChannelError::InvalidConfig`.

**Initialization** (`initialize_channels`, line 78) iterates the config's channel entries. For each enabled channel, the `init_channel!` macro (line 36) logs the attempt, calls the channel constructor, wraps the result in `Arc<dyn Channel>`, and inserts it into the `HashMap<String, DynChannel>` keyed by channel name string. Telegram receives an extra `groq_api_key` argument for voice transcription.

**Startup** (`start_all`, line 130) calls `initialize_channels`, then spawns each channel's `start()` in a dedicated `tokio::spawn` task. It also spawns an outbound dispatcher task that reads from `outbound_rx` in an infinite loop. When a message arrives the dispatcher looks up the target channel by `msg.channel`, calls `channel.send(&msg)`, and on failure sends a user-facing error message back through the same channel. The error feedback avoids infinite loops: if the error message itself fails, it logs and gives up.

**Shutdown** (`stop_all`, line 222) iterates all channels and calls `stop()`.

### Channel implementations

#### Telegram (`src/telegram.rs`)

Protocol: HTTP long polling against the Telegram Bot API (`https://api.telegram.org/bot{token}/`). No external library (teloxide); uses raw `reqwest` calls.

Key behaviors:
- **Long polling** (`poll_updates`, line 146): Loops calling `getUpdates` with a 30-second timeout, processing each update sequentially. On polling error, waits 5 seconds before retrying.
- **Update dispatch** (`handle_update`, line 252): Routes to `handle_callback_query` for inline keyboard button presses, `handle_reaction_update` for message reactions, or normal message handling.
- **Voice transcription**: If a Groq API key is configured, voice messages are downloaded, saved to `~/.klyntbot/media/`, and transcribed via `TranscriptionProvider` from the `providers` crate. Falls back to `[voice: path]` when transcription is unavailable.
- **Media download** (`download_media`, line 421): Calls `getFile` then downloads from `https://api.telegram.org/file/bot{token}/{path}`. Files are saved to `~/.klyntbot/media/` with a truncated file_id as filename.
- **Typing indicators** (`start_typing`/`stop_typing`, lines 559-606): Spawns a background task per chat that calls `sendChatAction` every 4 seconds (Telegram typing indicators expire after ~5 seconds). Tasks are stored in `typing_tasks` and aborted when a reply is sent or the channel stops.
- **Bot commands**: `/start`, `/help`, `/reset`. The `/reset` command publishes a special `__RESET_SESSION__` message via the bus system channel.
- **Message sending** (`Channel::send`, line 706): Formats content through `TelegramFormatter` (markdown to HTML), splits by the 4096-character limit, sends each chunk with `parse_mode: "HTML"`. If HTML parsing fails on a chunk, falls back to plain text. Small 100ms delays between chunks prevent rate limiting.
- **API retry**: `api_call_with_retry` (line 94) retries up to 3 times with exponential backoff (1s, 2s, 4s).
- **Interactions**: `supports_interaction()` returns `true`. `send_interaction()` (line 767) handles all four answer types: `SingleSelect` renders an inline keyboard with buttons in rows of 2; `YesNo` renders two buttons; `MultiSelect` is simplified to a single selection; `FreeText` sends a prompt and waits for the next user message. Callbacks use `DashMap<String, PendingCallback>` keyed by `"{chat_id}:{question_id}"` with a 5-minute timeout via `oneshot` channels.

#### Discord (`src/discord.rs`)

Protocol: WebSocket Gateway v10 (`wss://gateway.discord.gg/?v=10&encoding=json`) plus REST API v10 for sending.

Key behaviors:
- **WebSocket lifecycle**: Implements `WsHandler`. On connect, returns `HeartbeatStrategy::None` because Discord manages its own heartbeat via the HELLO opcode. The `on_text_message` handler parses the gateway JSON and dispatches by opcode.
- **HELLO (op 10)** (`handle_hello`, line 73): Reads the heartbeat interval, spawns a dedicated heartbeat task that sends `{"op": 1, "d": seq}` at that interval, then sends the IDENTIFY payload with the bot token and intent flags.
- **Event dispatch (op 0)**: Routes `MESSAGE_CREATE` to `handle_message_create`, `MESSAGE_REACTION_ADD` to `handle_reaction_add`, and `INTERACTION_CREATE` to `handle_interaction_create`.
- **Reconnect (op 7)** and **Invalid Session (op 9)**: Return `Ok(false)` from `on_text_message` to signal disconnection, which triggers the `reconnect_loop`.
- **Attachment handling**: Downloads attachments up to 20 MB (`MAX_ATTACHMENT_BYTES`), saves to `~/.klyntbot/media/`, and includes paths in the inbound message's `media` field.
- **Sending** (`send`, line 781): Formats through `PassthroughFormatter` (Discord supports markdown natively), splits at 2000 characters, sends via REST with retry logic and rate-limit handling (HTTP 429 with `retry_after`).
- **Typing indicators**: Background task calls the `/channels/{id}/typing` endpoint every 8 seconds.
- **Interactions**: `supports_interaction()` returns `true`. Uses Discord message components: buttons (type 2) in ActionRows (type 1) for 5 or fewer options, StringSelectMenu (type 3) for more. INTERACTION_CREATE events are acknowledged with type 6 (DEFERRED_UPDATE_MESSAGE). `custom_id` format: `"askuser:{channel_id}:{question_id}:{value}"`.
- **Reactions**: Extracts unicode or custom emoji from `MESSAGE_REACTION_ADD` events and publishes as `MessageKind::Reaction`.

#### WhatsApp (`src/whatsapp.rs`)

Protocol: WebSocket bridge to an external Node.js Baileys server at `config.bridge_url` (default: `ws://localhost:3001`).

Key behaviors:
- **WsHandler**: Stores the write half on connect; clears it on disconnect. Uses the default timeout-based heartbeat (30-second ping).
- **Message types**: Handles `"qr"` (logs QR code for authentication), `"message"` (user message), `"status"`, and `"error"`.
- **Sending**: Serializes `{"type": "send", "to": chat_id, "text": chunk}` as WebSocket text frames. Content is formatted through `PlainTextFormatter` and split at 4000 characters.
- **Chat ID fallback**: If no `chatId` field is present, falls back to the sender ID.

#### Slack (`src/slack.rs`)

Protocol: Socket Mode WebSocket plus REST API for sending.

Key behaviors:
- **Authentication**: Calls `auth.test` with the bot token to retrieve the bot's user ID, then calls `apps.connections.open` with the app-level token to get a WebSocket URL.
- **Socket Mode**: Each received envelope contains an `envelope_id` that must be ACKed immediately. The channel handles `events_api` envelopes (messages, app mentions, reactions) and `interactive` envelopes (block_actions from buttons/selects).
- **Message filtering**: Ignores bot/system messages (any subtype). In non-DM channels, only responds to `app_mention` events or messages containing `<@BOT_USER_ID>`. Strips the bot mention from the text before publishing.
- **Thread support**: Extracts `thread_ts` from events and stores it in metadata. Replies in non-DM channels use `thread_ts` to maintain thread context.
- **Reaction handling**: Adds an `:eyes:` reaction to incoming messages (best-effort). Converts incoming `reaction_added` events from Slack shortcodes to Unicode emoji via `slack_reaction_to_unicode`.
- **Interactions**: `supports_interaction()` returns `true`. Uses Block Kit: `section` blocks for text and `actions` blocks containing button elements. Yes/No buttons use `primary`/`danger` styles. `action_id` format: `"askuser:{channel_id}:{question_id}:{value}"`.
- **Sending**: Formats through `SlackFormatter` (markdown to Slack mrkdwn), splits at 8000 characters, sends via `chat.postMessage`.

#### Email (`src/email.rs`)

Protocol: IMAP polling for inbound, SMTP for outbound. Feature-gated behind the `email` Cargo feature (on by default).

Key behaviors:
- **Consent gate**: Requires `config.consent_granted = true` before starting. Also validates that all six required fields are present (IMAP host/user/password, SMTP host/user/password).
- **IMAP polling** (`poll_imap`, line 89): Connects with TLS or plain TCP depending on `imap_use_ssl`. Selects the configured mailbox (default: `INBOX`), searches for `UNSEEN` messages. Uses `BODY.PEEK[]` to avoid auto-marking as read.
- **Deduplication**: Tracks processed UIDs in a `HashSet` (capped at 10,000 entries, then cleared).
- **Body extraction** (`process_email_body`, line 237): Prefers `text/plain`; falls back to HTML converted via `html2text`. Truncates at `max_body_chars` (default: 12,000).
- **Threading**: Stores the last subject and Message-ID per sender. Outbound replies include `In-Reply-To` and `References` headers. Subject line uses configurable prefix (default: `"Re: "`), avoiding double-prefix if the subject already starts with `"re:"`.
- **SMTP sending** (`send_email`, line 339): Uses `lettre` with `SmtpTransport::relay`. The actual send runs on `spawn_blocking` since lettre's transport is synchronous.
- **Auto-reply guard**: If `auto_reply_enabled` is `false`, outbound messages are silently skipped.

#### QQ (`src/qq.rs`)

Protocol: QQ Bot API via REST authentication and WebSocket gateway (`wss://api.sgroup.qq.com/websocket`).

Key behaviors:
- **Authentication** (`authenticate`, line 87): Posts to `/app/getAppAccessToken` with `appId` and `clientSecret`. Stores the access token for REST API calls.
- **Gateway events** (`handle_gateway_event`, line 121): Parses opcodes: 10 (HELLO), 0 (dispatch with event types `READY`, `C2C_MESSAGE_CREATE`, `DIRECT_MESSAGE_CREATE`), 7 (RECONNECT), 9 (INVALID_SESSION), 11 (HEARTBEAT_ACK).
- **Deduplication**: Uses a `VecDeque` capped at 1000 entries. When full, the oldest is evicted.
- **Sending** (`send_c2c_message`, line 230): Posts to `/v2/users/{openid}/messages` with `QQBot {token}` authorization. Content is formatted through `PlainTextFormatter` and split at 4000 characters.

### WebSocket manager (`src/ws_manager.rs`)

The `WebSocketManager` extracts the common connect-heartbeat-readloop-shutdown pattern shared by Discord, WhatsApp, QQ, and Slack. Each channel implements the `WsHandler` trait instead of managing raw WebSocket plumbing.

**WsHandler trait** (line 77):
- `on_connected(&self, write)` -- called after connection, before the read loop. Can return an overridden `HeartbeatStrategy`.
- `on_text_message(&self, text, write)` -- called for each text frame. Returns `Ok(true)` to continue, `Ok(false)` to disconnect.
- `on_disconnected(&self)` -- cleanup after the read loop exits.

**HeartbeatStrategy** (line 33):
- `Timeout { timeout, build_payload }` -- sends a keepalive when no message arrives within `timeout`. Used by WhatsApp, QQ, and Slack.
- `None` -- no automatic heartbeat. Discord uses this because it spawns its own heartbeat task from the HELLO event.

**WebSocketManager::run** (line 107):
1. Connects with a configurable timeout (default 30s).
2. Splits into read/write halves. Write is wrapped in `Arc<Mutex<WsSink>>`.
3. Calls `handler.on_connected()`.
4. Enters the read loop. On timeout, sends the heartbeat payload. On text message, delegates to handler. On close frame, responds per RFC 6455.
5. Sends a graceful close frame.
6. Calls `handler.on_disconnected()`.

Callers wrap `manager.run()` in `reconnect_loop` for automatic reconnection on error.

### Message formatting (`src/formatter.rs`)

The formatter system normalizes LLM-generated markdown to each channel's native format. Four formatter implementations:

| Formatter | Channel(s) | Strategy |
|-----------|-----------|----------|
| `TelegramFormatter` | Telegram | Markdown to Telegram HTML. Protects code blocks/inline code with sentinels, escapes HTML entities, converts bold/italic/strikethrough/links to HTML tags, restores code with HTML escaping. Prevents attribute injection in link URLs by escaping `"` to `&quot;`. |
| `PassthroughFormatter` | Discord | No transformation. Discord supports markdown natively. |
| `SlackFormatter` | Slack | Markdown to Slack mrkdwn. Headers become `*bold*`. `**bold**` becomes `*bold*`. `~~strike~~` becomes `~strike~`. `[text](url)` becomes `<url\|text>`. Code block language identifiers are stripped. |
| `PlainTextFormatter` | WhatsApp, Email, QQ | Strips all markdown. Code blocks/inline code unwrapped. Links become `"text (url)"`. Headers/blockquotes/bold/italic/strikethrough reduced to plain text. Bullets normalized to `"- "`. |

`formatter_for(channel_name)` (line 18) returns a static reference to the appropriate formatter.

All regex sets are lazily compiled via `OnceLock` and reused for the lifetime of the process.

### Message splitting (`src/utils.rs`)

`split_message(text, limit)` chunks a message to fit within platform character limits. The splitting priority is:

1. Paragraph break (double newline)
2. Line break (single newline)
3. Sentence break (`. `, `! `, `? `)
4. Word break (space)
5. Hard character split at a safe UTF-8 boundary (uses `truncate_at_boundary` from `common`)

`max_length(channel)` returns the per-channel limit:

| Channel | Limit |
|---------|-------|
| Telegram | 4096 |
| Discord | 2000 |
| WhatsApp | 4000 |
| Slack | 8000 |
| Email | 8000 |
| Default | 4000 |

### Allowlist / access control

`check_allowlist(allow_from, sender_id)` (`src/lib.rs:83-104`) implements the shared authorization check:
- Empty allowlist: everyone is allowed.
- Exact match against the allowlist.
- Compound ID match: if `sender_id` contains `|` (e.g., Telegram's `"12345|username"`), each part is checked independently against the allowlist.

Every channel calls this in `is_allowed()` and during inbound message processing. Denied senders are logged at `warn` level and silently dropped.

### Reconnection strategy

`reconnect_loop(name, running, connect)` (`src/lib.rs:108-123`) wraps any async connect function in an infinite retry loop. On error it logs the failure, waits 5 seconds, and retries. The loop exits when `running` (an `Arc<AtomicBool>`) is set to `false`.

Used by Discord, WhatsApp, QQ, and Slack. Telegram uses its own polling loop with a 5-second backoff on error instead.

### How channels bridge to the message bus

Inbound flow:
1. A channel receives a platform-specific event (HTTP update, WebSocket message, IMAP fetch).
2. It extracts sender ID, chat ID, and content. Checks the allowlist.
3. Constructs an `InboundMessage` with channel name, sender, chat ID, content, and optional metadata/media.
4. Calls `bus.publish_inbound(msg)` which sends it to the agent loop via `tokio::mpsc`.

Outbound flow:
1. The agent loop calls `bus.send_outbound(msg)` with an `OutboundMessage` specifying the target channel and chat ID.
2. The `ChannelManager`'s dispatcher task receives the message from `outbound_rx`.
3. It looks up the channel by name and calls `channel.send(&msg)`.
4. The channel formats the content, splits it, and sends via the platform API.

---

## Section 2: API Reference

### Channel trait

**File**: `src/lib.rs:38-77`

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;
    async fn send_typing(&self, _chat_id: &str) -> Result<()>;           // default: Ok(())
    fn supports_interaction(&self) -> bool;                                // default: false
    async fn send_interaction(&self, _chat_id: &str, _request: &InteractionRequest) -> Result<FormResponse>;  // default: Err
}
```

| Method | Description |
|--------|-------------|
| `name()` | Returns the channel identifier string (e.g., `"telegram"`, `"discord"`). |
| `start(bus)` | Long-running task. Connects to the platform and publishes inbound messages to the bus. |
| `stop()` | Signals the channel to shut down gracefully. Typically sets `running` to `false`. |
| `send(msg)` | Delivers an outbound message. Formats content, splits by limit, sends via platform API. |
| `is_allowed(sender_id)` | Checks if the sender passes the allowlist. |
| `send_typing(chat_id)` | Sends a typing/activity indicator. Default no-op. |
| `supports_interaction()` | Whether the channel supports structured UI (buttons, menus). Default `false`. |
| `send_interaction(chat_id, request)` | Sends an `InteractionRequest` and collects a `FormResponse`. Default returns error. |

### DynChannel

**File**: `src/lib.rs:80`

```rust
pub type DynChannel = Arc<dyn Channel>;
```

### ChannelManager

**File**: `src/manager.rs:51-234`

```rust
pub struct ChannelManager {
    channels: Arc<RwLock<HashMap<String, DynChannel>>>,
    bus: Arc<MessageBus>,
    outbound_rx: Option<mpsc::Receiver<OutboundMessage>>,
    config: Arc<Config>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: Arc<Config>, bus: Arc<MessageBus>) -> Result<Self>` | Creates the manager. Takes ownership of `bus.take_outbound_rx()`. Fails if the receiver was already taken. |
| `initialize_channels` | `async fn initialize_channels(&self) -> Result<()>` | Constructs and inserts all enabled channels into the internal map. |
| `start_all` | `async fn start_all(&mut self) -> Result<()>` | Initializes channels, spawns each in a task, spawns the outbound dispatcher, and awaits all tasks. |
| `stop_all` | `async fn stop_all(&self) -> Result<()>` | Calls `stop()` on every channel. |

### TelegramChannel

**File**: `src/telegram.rs:37-46`

```rust
pub struct TelegramChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: TelegramConfig, groq_api_key: Option<String>) -> Result<Self>` | Builds an HTTP client (with optional proxy), constructs the API base URL, and optionally creates a `TranscriptionProvider`. |

**Config requirements**: `TelegramConfig` with `token` (bot token), optional `proxy` URL, optional `allow_from` list.

**Interaction support**: Yes. Supports `SingleSelect`, `YesNo`, `MultiSelect` (simplified to single), `FreeText`.

### DiscordChannel

**File**: `src/discord.rs:40-50`

```rust
pub struct DiscordChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: DiscordConfig) -> Result<Self>` | Builds an HTTP client. |

**Config requirements**: `DiscordConfig` with `token` (bot token), `gateway_url` (default: `wss://gateway.discord.gg/?v=10&encoding=json`), `intents` (bitfield), optional `allow_from`.

**Interaction support**: Yes. Supports `SingleSelect` (buttons or StringSelectMenu), `YesNo`, `MultiSelect` (simplified), `FreeText`.

### WhatsAppChannel

**File**: `src/whatsapp.rs:20-25`

```rust
pub struct WhatsAppChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: WhatsAppConfig) -> Result<Self>` | Stores the config. No external connections at construction. |

**Config requirements**: `WhatsAppConfig` with `bridge_url` (default: `ws://localhost:3001`), optional `allow_from`.

**Interaction support**: No.

### SlackChannel

**File**: `src/slack.rs:37-45`

```rust
pub struct SlackChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: SlackConfig) -> Result<Self>` | Builds an HTTP client. |

**Config requirements**: `SlackConfig` with `bot_token`, `app_token` (app-level token for Socket Mode), optional `allow_from`, `mode` (default: `"socket"`), `group_policy`.

**Interaction support**: Yes. Uses Block Kit with button elements and select menus.

### EmailChannel

**File**: `src/email.rs:23-29` (feature-gated: `#[cfg(feature = "email")]`)

```rust
pub struct EmailChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: EmailConfig) -> Result<Self>` | Stores the config. |

**Config requirements**: `EmailConfig` with `consent_granted: true`, IMAP host/port/username/password, SMTP host/port/username/password. Defaults: IMAP port 993, SMTP port 587, mailbox `"INBOX"`, TLS enabled, poll interval 30s, max body 12000 chars, auto-reply enabled, mark seen true, subject prefix `"Re: "`.

**Interaction support**: No.

### QQChannel

**File**: `src/qq.rs:25-33`

```rust
pub struct QQChannel { /* fields omitted */ }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: QQConfig) -> Result<Self>` | Builds an HTTP client. |

**Config requirements**: `QQConfig` with `app_id` and `secret`, optional `allow_from`.

**Interaction support**: No.

### WebSocketManager

**File**: `src/ws_manager.rs:97-99`

```rust
pub struct WebSocketManager {
    running: Arc<AtomicBool>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(running: Arc<AtomicBool>) -> Self` | Creates a manager bound to a shared running flag. |
| `run` | `async fn run(&self, config: &WsConfig, handler: &(dyn WsHandler + '_)) -> Result<()>` | Runs a single WebSocket session: connect, handshake, read loop, close, cleanup. Returns when the connection drops or `running` is `false`. |

### WsHandler trait

**File**: `src/ws_manager.rs:76-92`

```rust
#[async_trait]
pub trait WsHandler: Send + Sync {
    async fn on_connected(&self, write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>>;
    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool>;
    async fn on_disconnected(&self) {}
}
```

| Method | Description |
|--------|-------------|
| `on_connected(write)` | Post-connect handshake. Return `Some(strategy)` to override heartbeat, or `None` to keep config default. |
| `on_text_message(text, write)` | Handle a text frame. `Ok(true)` continues; `Ok(false)` disconnects. |
| `on_disconnected()` | Cleanup after read loop exit. Default no-op. |

### WsConfig

**File**: `src/ws_manager.rs:47-69`

```rust
pub struct WsConfig {
    pub url: String,
    pub connect_timeout: Duration,       // default: 30s
    pub heartbeat: HeartbeatStrategy,     // default: Timeout { 30s, Ping }
}
```

### HeartbeatStrategy

**File**: `src/ws_manager.rs:33-44`

```rust
pub enum HeartbeatStrategy {
    Timeout {
        timeout: Duration,
        build_payload: Box<dyn Fn() -> WsMessage + Send + Sync>,
    },
    None,
}
```

### Type aliases

**File**: `src/ws_manager.rs:27-30`

```rust
pub type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>;
pub type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
```

### ChannelFormatter trait

**File**: `src/formatter.rs:13-15`

```rust
pub trait ChannelFormatter: Send + Sync {
    fn format(&self, markdown: &str) -> String;
}
```

### Formatter structs

**File**: `src/formatter.rs`

| Struct | Line | Implements | Behavior |
|--------|------|------------|----------|
| `TelegramFormatter` | 67 | `ChannelFormatter` | Markdown to Telegram HTML with code protection, HTML escaping, and link injection prevention. |
| `PassthroughFormatter` | 178 | `ChannelFormatter` | Identity function. |
| `SlackFormatter` | 209 | `ChannelFormatter` | Markdown to Slack mrkdwn. |
| `PlainTextFormatter` | 275 | `ChannelFormatter` | Strip all markdown to plain text. |

### formatter_for

**File**: `src/formatter.rs:18-31`

```rust
pub fn formatter_for(channel: &str) -> &'static dyn ChannelFormatter
```

Returns the statically-allocated formatter for the named channel. Falls back to `PlainTextFormatter` for unknown channel names.

### Utility functions

#### check_allowlist

**File**: `src/lib.rs:83-104`

```rust
pub fn check_allowlist(allow_from: &[String], sender_id: &str) -> bool
```

Returns `true` if the sender is authorized. Empty allowlist permits everyone. Checks exact matches and each `|`-separated part of compound IDs.

#### reconnect_loop

**File**: `src/lib.rs:108-123`

```rust
pub async fn reconnect_loop<F, Fut>(name: &str, running: &Arc<AtomicBool>, mut connect: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<()>>,
```

Calls `connect()` in a loop while `running` is `true`. On error, logs and waits 5 seconds before retrying.

#### max_length

**File**: `src/utils.rs:6-15`

```rust
pub fn max_length(channel: &str) -> usize
```

Returns the character limit for the given channel name.

#### split_message

**File**: `src/utils.rs:25-49`

```rust
pub fn split_message(text: &str, limit: usize) -> Vec<String>
```

Splits text into chunks within the given character limit. Prefers semantic split points (paragraph, line, sentence, word) over hard splits. Safe for multibyte UTF-8.

### Public re-exports

**File**: `src/lib.rs:27-34`

```rust
pub use discord::DiscordChannel;
#[cfg(feature = "email")]
pub use email::EmailChannel;
pub use manager::ChannelManager;
pub use qq::QQChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use whatsapp::WhatsAppChannel;
```

### Modules

| Module | Visibility | Feature gate |
|--------|-----------|-------------|
| `discord` | `pub` | -- |
| `email` | `pub` | `#[cfg(feature = "email")]` |
| `formatter` | `pub` | -- |
| `manager` | `pub` | -- |
| `qq` | `pub` | -- |
| `slack` | `pub` | -- |
| `telegram` | `pub` | -- |
| `utils` | `pub` | -- |
| `whatsapp` | `pub` | -- |
| `ws_manager` | `pub` | -- |
