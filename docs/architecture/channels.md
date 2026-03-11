# Channels Architecture

## Overview

The `channels` crate provides a unified abstraction for integrating with chat platforms. Each platform (Telegram, Discord, Slack, Email) implements the `Channel` trait, which standardizes message ingestion, delivery, access control, and structured interactions. Channels communicate with the agent runtime exclusively through the `MessageBus` -- a dual-queue async message bus defined in the `bus` crate.

This design decouples platform-specific protocol handling from the agent's core logic. Adding a new platform requires only implementing the `Channel` trait and registering it in `ChannelManager`.

**Key files:**

- `crates/channels/src/lib.rs` -- `Channel` trait, `DynChannel`, `check_allowlist`, `reconnect_loop`
- `crates/channels/src/manager.rs` -- `ChannelManager`
- `crates/bus/src/queue.rs` -- `MessageBus`
- `crates/bus/src/events.rs` -- `InboundMessage`, `OutboundMessage`
- `crates/config/src/schema/channels.rs` -- per-platform config structs

## Channel Trait

Every platform adapter implements this async trait:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel name (e.g., "telegram", "discord")
    fn name(&self) -> &str;

    /// Start the channel (long-running task)
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;

    /// Stop the channel
    async fn stop(&self) -> Result<()>;

    /// Send a message through this channel
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;

    /// Check if sender is allowed
    fn is_allowed(&self, sender_id: &str) -> bool;

    /// Send a typing indicator (default: no-op)
    async fn send_typing(&self, _chat_id: &str) -> Result<()> { Ok(()) }

    /// Whether this channel supports structured interactions (buttons, menus)
    fn supports_interaction(&self) -> bool { false }

    /// Send a structured interaction request using platform-native UI
    async fn send_interaction(
        &self,
        _chat_id: &str,
        _request: &InteractionRequest,
    ) -> Result<FormResponse> { /* default: error */ }
}
```

Channels are stored as `DynChannel = Arc<dyn Channel>` for dynamic dispatch.

**Method responsibilities:**

| Method | Purpose |
|---|---|
| `name()` | Returns the channel identifier used in `InboundMessage.channel` and routing |
| `start()` | Runs the receive loop (long polling, WebSocket, IMAP poll). Blocks until stopped. |
| `stop()` | Sets an `AtomicBool` flag that the receive loop checks to exit |
| `send()` | Delivers an `OutboundMessage` to the platform, handling formatting and chunking |
| `is_allowed()` | Delegates to `check_allowlist` with the channel's `allow_from` config |
| `send_typing()` | Sends a typing indicator. Telegram resends every 4s (expires after 5s). |
| `supports_interaction()` | Returns `true` for Telegram, Discord, and Slack |
| `send_interaction()` | Sends inline keyboards/buttons and waits for user response via `InteractionTracker` |

## ChannelManager

`ChannelManager` orchestrates the lifecycle of all enabled channels. It holds:

- `channels: Arc<RwLock<HashMap<String, DynChannel>>>` -- registered channel instances
- `bus: Arc<MessageBus>` -- shared message bus reference
- `outbound_rx` -- one-shot receiver for the outbound queue (taken on `start_all`)
- `config: Arc<Config>` -- application configuration

### Initialization flow

1. `ChannelManager::new(config, bus)` takes ownership of the outbound receiver via `bus.take_outbound_rx()`. This can only be called once -- a second call returns an error.

2. `initialize_channels()` iterates over the config. For each platform where `enabled: true`, it creates the channel and inserts it into the `channels` map using the `init_channel!` macro.

3. `start_all()` spawns each channel's `start()` method in its own `tokio::spawn` task. It also spawns an **outbound dispatcher** task that reads from the outbound queue and routes each `OutboundMessage` to the correct channel by matching `msg.channel` against the channels map.

### Outbound dispatcher

The dispatcher loop:
- Reads `OutboundMessage` values from `outbound_rx`
- Looks up the target channel by name in the channels map
- Calls `channel.send(&msg)`
- On failure, sends a user-facing error message back through the same channel (with safeguards against infinite retry loops)
- Converts internal errors to user-friendly descriptions via `user_facing_error()`

### Error handling

Failed sends produce user-facing fallback messages. The dispatcher distinguishes:
- `ConnectionFailed` -- "Channel may be temporarily unavailable"
- `SendFailed` with rate limit indicators -- "Rate limited -- please wait"
- Other errors -- "An unexpected error occurred"

## MessageBus

`MessageBus` is the central decoupling point between channels and the agent runtime. It uses two independent `tokio::sync::mpsc` channels:

```
Channels ──publish_inbound()──> [inbound queue] ──take_inbound_rx()──> Agent
Agent ──publish_outbound()──> [outbound queue] ──take_outbound_rx()──> ChannelManager
```

### Structure

```rust
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<Option<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Mutex<Option<mpsc::Receiver<OutboundMessage>>>,
}
```

### One-shot receiver pattern

Each receiver (`inbound_rx`, `outbound_rx`) is wrapped in `Mutex<Option<...>>`. The `take_inbound_rx()` and `take_outbound_rx()` methods call `.take()`, transferring ownership exactly once. This enforces a single-consumer model: the agent owns the inbound receiver, `ChannelManager` owns the outbound receiver. A second call returns `None`.

### Message size limit

`InboundMessage::validate()` rejects messages with content exceeding `MAX_MESSAGE_SIZE` (64 KB / 65,536 bytes). Validation runs inside `publish_inbound()` before the message enters the queue.

### Message types

**InboundMessage** (channel to agent):

| Field | Type | Description |
|---|---|---|
| `channel` | `ChannelName` | Source platform identifier |
| `sender_id` | `String` | User ID (may be compound: `"12345\|username"`) |
| `chat_id` | `ChatId` | Conversation/channel identifier |
| `content` | `String` | Message text |
| `timestamp` | `DateTime<Utc>` | When the message was received |
| `media` | `Vec<String>` | Attached media file paths or URLs |
| `metadata` | `HashMap<String, Value>` | Channel-specific data (e.g., Slack `thread_ts`) |
| `kind` | `MessageKind` | `Text` (default) or `Reaction` |

**OutboundMessage** (agent to channel):

| Field | Type | Description |
|---|---|---|
| `channel` | `ChannelName` | Target platform |
| `chat_id` | `ChatId` | Target conversation |
| `content` | `String` | Response text |
| `reply_to` | `Option<String>` | Message ID to reply to |
| `media` | `Vec<String>` | Media URLs to attach |
| `metadata` | `HashMap<String, Value>` | Channel-specific data |

**Session key:** `InboundMessage::session_key()` returns `SessionKey` in the format `"channel:chat_id"` (e.g., `"telegram:123456"`), used to correlate messages to agent sessions.

## Platform Implementations

### Telegram

| Aspect | Detail |
|---|---|
| **File** | `crates/channels/src/telegram.rs` |
| **Protocol** | HTTP long polling via Telegram Bot API (`getUpdates` with 30s timeout) |
| **Config struct** | `TelegramConfig` |
| **Config fields** | `enabled`, `token` (bot token), `allow_from`, `proxy` (optional SOCKS5/HTTP proxy) |
| **Interactions** | Supported. Uses `InlineKeyboardMarkup` with callback queries. |
| **Typing** | Resends `sendChatAction` every 4s (Telegram expires typing after ~5s). Uses `TypingManager`. |
| **Media** | Downloads photos, voice, audio, documents via `getFile` API. Saves to `~/.klyntbot/media/`. |
| **Voice** | Transcribes voice messages using Groq (if API key configured). Falls back to `[voice: path]`. |
| **Formatting** | Converts Markdown to Telegram HTML (`<b>`, `<i>`, `<code>`, `<pre>`, `<a>`). Falls back to plain text on parse errors. |
| **Message limits** | Splits long messages into chunks respecting the platform's max length. 100ms delay between chunks. |
| **Commands** | `/start`, `/reset` (clears session), `/help`. Registers commands via `setMyCommands`. |
| **Reactions** | Receives `message_reaction` updates and publishes as `MessageKind::Reaction`. |
| **Retry logic** | API calls retry up to 3 times with exponential backoff (1s, 2s, 4s). |
| **Sender ID format** | `"user_id\|username"` compound format (both parts checked against allowlist) |

### Discord

| Aspect | Detail |
|---|---|
| **File** | `crates/channels/src/discord.rs` |
| **Protocol** | Raw WebSocket to Discord Gateway (`wss://gateway.discord.gg/?v=10&encoding=json`). No serenity dependency. |
| **Config struct** | `DiscordConfig` |
| **Config fields** | `enabled`, `token` (bot token), `allow_from`, `gateway_url`, `intents` (default: 46593 -- guild messages, DMs, reactions, message content) |
| **Interactions** | Supported. Uses Discord message components (buttons, select menus) via REST API. |
| **Typing** | Sends typing indicator via REST API. Uses `TypingManager`. |
| **WebSocket** | Uses the shared `WebSocketManager` for connection management, heartbeating (Gateway heartbeat interval from `HELLO`), and automatic reconnection. |
| **Formatting** | Converts to Discord-flavored Markdown. |
| **Attachment limit** | 20 MB per attachment (`MAX_ATTACHMENT_BYTES`). |
| **Reactions** | Receives `MESSAGE_REACTION_ADD` gateway events and publishes as `MessageKind::Reaction`. |
| **Gateway intents** | Configured via `intents` field. Default enables `GUILD_MESSAGES`, `GUILD_MESSAGE_REACTIONS`, `DIRECT_MESSAGES`, `DIRECT_MESSAGE_REACTIONS`, `MESSAGE_CONTENT`. |

### Slack

| Aspect | Detail |
|---|---|
| **File** | `crates/channels/src/slack.rs` |
| **Protocol** | Socket Mode WebSocket (`apps.connections.open` for URL, then WebSocket with envelope-based messaging). |
| **Config struct** | `SlackConfig` |
| **Config fields** | `enabled`, `bot_token`, `app_token`, `allow_from`, `mode` (default: `"socket"`), `group_policy` (default: `"none"`), `group_allow_from`, `dm` (DM-specific config) |
| **Interactions** | Supported. Uses Block Kit (buttons via `actions` blocks, `static_select` menus). Handles `block_actions` interactive payloads. |
| **Authentication** | Calls `auth.test` on startup to get the bot's own user ID (used to filter self-messages and strip `<@BOT>` mentions). |
| **Envelope handling** | Parses Socket Mode envelopes. Sends `envelope_id` ACK immediately. Routes `events_api` and `interactive` types. |
| **Message routing** | In channels/groups, only responds to `app_mention` events or messages containing `<@BOT_ID>`. Always responds in DMs (`channel_type: "im"`). |
| **Threading** | Preserves `thread_ts` in metadata. Replies in threads for non-DM channels. DMs are never threaded. |
| **Reactions** | Adds `:eyes:` reaction on receipt (best-effort). Receives `reaction_added` events and publishes as `MessageKind::Reaction`. Converts Slack shortcodes to Unicode emoji. |
| **Bot mention stripping** | Removes `<@BOT_ID>` from message text before publishing to bus. |
| **Reconnection** | Uses `reconnect_loop` with 5s delay. Gets a fresh socket URL on each reconnection attempt. WebSocket heartbeat timeout: 35s. |

### Email

| Aspect | Detail |
|---|---|
| **File** | `crates/channels/src/email.rs` |
| **Feature gate** | `#[cfg(feature = "email")]` -- included by default but can be excluded |
| **Protocol** | IMAP polling for inbound (configurable interval, default 30s), SMTP for outbound |
| **Config struct** | `EmailConfig` |
| **Config fields** | `enabled`, `imap_host`, `imap_port` (993), `imap_username`, `imap_password`, `imap_mailbox` (`"INBOX"`), `imap_use_ssl` (true), `smtp_host`, `smtp_port` (587), `smtp_username`, `smtp_password`, `smtp_use_tls` (true), `smtp_use_ssl`, `from_address`, `allow_from`, `consent_granted`, `auto_reply_enabled` (true), `max_body_chars` (12000), `mark_seen` (true), `poll_interval_seconds` (30), `subject_prefix` (`"Re: "`) |
| **Consent required** | `consent_granted` must be `true` or startup fails with an explicit privacy warning. |
| **Interactions** | Not supported (`supports_interaction()` returns `false`). |
| **Message parsing** | Uses `mail_parser` for RFC 5322 parsing. Prefers `text/plain`; falls back to `html2text` conversion of HTML body. Truncates to `max_body_chars`. |
| **Threading** | Uses `In-Reply-To` and `References` headers for email threading. Tracks `Message-ID` per sender. |
| **Reply subject** | Prepends `subject_prefix` (default `"Re: "`) unless subject already starts with `re:` (case-insensitive). |
| **Deduplication** | Tracks processed UIDs in an in-memory `HashSet`. Clears after 10,000 entries. |
| **Auto-reply** | Controlled by `auto_reply_enabled`. When `false`, outbound messages are silently dropped. |
| **SMTP** | Uses `lettre` with `SmtpTransport`. Send runs via `spawn_blocking` to avoid blocking the async runtime. |

### Feishu/Lark (Config only)

| Aspect | Detail |
|---|---|
| **Config struct** | `FeishuConfig` |
| **Config fields** | `enabled`, `app_id`, `app_secret`, `encrypt_key`, `verification_token`, `allow_from` |
| **Status** | Configuration defined but no channel implementation yet. |

### DingTalk (Config only)

| Aspect | Detail |
|---|---|
| **Config struct** | `DingTalkConfig` |
| **Config fields** | `enabled`, `client_id`, `client_secret`, `allow_from` |
| **Status** | Configuration defined but no channel implementation yet. |

### Mochat (Config only)

| Aspect | Detail |
|---|---|
| **Config struct** | `MochatConfig` |
| **Config fields** | `enabled`, `base_url` (default: `"https://mochat.io"`), `socket_url`, `claw_token`, `agent_user_id`, `sessions`, `panels`, `allow_from` |
| **Status** | Configuration defined but no channel implementation yet. |

## Access Control

Every channel has an `allow_from: Vec<String>` config field. Access checking uses `check_allowlist()` from `crates/channels/src/lib.rs`:

1. **Empty list = open access.** If `allow_from` is empty, all senders are permitted.
2. **Exact match.** The sender ID is checked against each entry in the list.
3. **Compound ID splitting.** If the sender ID contains `|` (e.g., `"12345|username"`), each part is checked independently. This lets you allowlist by either numeric ID or username.

Slack has additional granularity:
- `group_allow_from` for channel/group messages (separate from DM access)
- `dm.allow_from` for DM-specific access control
- `group_policy` controls how the bot responds in group channels

## Reconnection

The `reconnect_loop` helper in `lib.rs` provides a standard reconnection pattern for channels using persistent connections (Discord WebSocket, Slack Socket Mode):

```rust
pub async fn reconnect_loop<F, Fut>(name: &str, running: &Arc<AtomicBool>, mut connect: F)
```

- Calls `connect()` in a loop while `running` is `true`
- On error, logs the error and waits 5 seconds before retrying
- Exits when `running` is set to `false` (via `stop()`)

Discord and Slack also use `WebSocketManager` (from `crates/channels/src/ws_manager.rs`) which handles WebSocket connection setup, heartbeating, and message dispatch through the `WsHandler` trait.

## Adding a New Channel

1. **Define config** in `crates/config/src/schema/channels.rs`. Add a new struct (e.g., `WhatsAppConfig`) with `enabled`, credential fields, and `allow_from`. Add the field to `ChannelsConfig`.

2. **Implement `Channel` trait** in a new file `crates/channels/src/{platform}.rs`. At minimum:
   - `name()` returns the platform identifier
   - `start()` runs the receive loop, publishing `InboundMessage` values to the bus
   - `stop()` sets an `AtomicBool` to exit the loop
   - `send()` delivers `OutboundMessage` to the platform
   - `is_allowed()` delegates to `check_allowlist`

3. **Register in `lib.rs`**: Add `pub mod {platform}` and `pub use {platform}::{PlatformChannel}`.

4. **Register in `ChannelManager`**: Add an `init_channel!` block in `initialize_channels()` for the new platform.

5. **Add a formatter** in `crates/channels/src/formatter.rs` if the platform uses a different markup format (HTML, mrkdwn, plain text).

6. **Consider interactions**: If the platform has native UI elements (buttons, menus), implement `supports_interaction()` returning `true` and provide `send_interaction()`. Use `InteractionTracker` for async callback resolution.

7. **Consider reconnection**: For WebSocket-based channels, use `WebSocketManager` and implement `WsHandler`. For polling-based channels, use a simple `while running` loop with sleep intervals.
