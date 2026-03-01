# Channels

## Purpose

The `channels` crate (Layer 4) connects Klyntbot to six chat platforms -- Telegram, Discord, Slack, WhatsApp, QQ, and Email. Each platform has a concrete struct that implements the `Channel` trait, handling platform-specific protocols (HTTP polling, WebSocket gateways, IMAP/SMTP) while presenting a uniform interface to the rest of the system. The `ChannelManager` orchestrates lifecycle (init, start, stop) for all enabled channels and runs a single outbound dispatcher loop that routes agent responses back to the correct platform.

No channel contains business logic. Every inbound message is published to the `MessageBus` as an `InboundMessage`; every outbound message arrives from the bus as an `OutboundMessage`. The agent crate consumes one side, the channels crate consumes the other.

## Key Types

### Channel Trait

The `Channel` trait is the contract every platform implements:

| Method | Purpose |
|--------|---------|
| `name()` | Returns a static string identifier (`"telegram"`, `"discord"`, etc.). |
| `start(bus)` | Long-running task: connect to the platform, listen for messages, publish `InboundMessage` to the bus. |
| `stop()` | Set a running flag to false, causing the `start()` loop to exit. |
| `send(msg)` | Deliver an `OutboundMessage` to the platform. Handles formatting, chunking, and retries. |
| `is_allowed(sender_id)` | Check sender against the per-channel allowlist. |
| `send_typing(chat_id)` | Start a typing indicator (default: no-op). |
| `supports_interaction()` | Whether the channel has native UI elements (buttons, menus). Default: false. |
| `send_interaction(chat_id, request)` | Send a structured `InteractionRequest` and wait for the user's response. Only called when `supports_interaction()` is true. |

`DynChannel` is the type alias `Arc<dyn Channel>`, used everywhere channels are stored or passed around.

### ChannelManager

Holds a `HashMap<String, DynChannel>` behind an `Arc<RwLock<...>>`. Construction takes a `Config` and a `MessageBus`. The `start_all()` method initializes channels via an `init_channel!` macro that checks the enabled flag, logs, creates the channel, and inserts it into the map. Each channel is spawned as an independent tokio task. A dedicated outbound dispatcher task takes ownership of the bus's `outbound_rx` receiver and loops forever, reading `OutboundMessage` values, looking up the target channel by name, and calling `send()`. If send fails, a user-facing error message is sent back to the same channel as a best-effort fallback.

### Shared Infrastructure

**`WebSocketManager`** -- a generic WebSocket session manager shared by Discord, Slack, WhatsApp, and QQ. It owns the connect-heartbeat-read-close lifecycle. Channels implement the `WsHandler` trait (three callbacks: `on_connected`, `on_text_message`, `on_disconnected`) and pass their protocol-specific logic through those hooks. The manager supports two heartbeat strategies: `Timeout` (send keepalive on inactivity, used by WhatsApp/QQ/Slack) and `None` (Discord manages its own heartbeat externally). Channels wrap `WebSocketManager::run()` in the top-level `reconnect_loop()` helper, which retries on error with a 5-second backoff.

**`InteractionTracker`** -- manages pending structured interaction callbacks using a `DashMap<String, PendingCallback>` keyed by `"{chat_id}:{question_id}"`. Two callback variants: `Single` (for button presses) and `FreeText` (for text replies). Both use `oneshot` channels with a 5-minute timeout. Telegram, Discord, and Slack all share this tracker to avoid duplicating wait-and-resolve logic.

**`TypingManager`** -- manages per-chat typing indicator background tasks. Each call to `start()` spawns a repeating tokio task that calls the channel's typing API at a configurable interval (4s for Telegram, 8s for Discord). `stop()` aborts the task. This prevents the typing indicator from expiring while the agent is processing.

**`ChannelFormatter`** -- trait with a `format(markdown) -> String` method. A static `formatter_for(channel_name)` function returns the right formatter: Telegram gets markdown-to-HTML conversion (with code block extraction and HTML entity escaping), Discord gets passthrough (native markdown), Slack gets mrkdwn conversion, and WhatsApp/Email/QQ get plain text stripping.

**`split_message`** -- splits text into chunks that fit each platform's character limit (Telegram 4096, Discord 2000, Slack 8000, etc.). Splitting priority: paragraph breaks, line breaks, sentence breaks, word breaks, hard character split.

### Allowlist

The `check_allowlist()` function gates every inbound message. If the allowlist is empty, all senders are accepted. Otherwise it checks exact match and compound IDs (e.g., `"123456|username"` matches if either `"123456"` or `"username"` is in the list).

## Platform Details

### Telegram

**Protocol**: HTTP long polling against the Bot API (no teloxide dependency). Uses `reqwest` with a 30-second timeout and configurable proxy support.

**Inbound**: Polls `getUpdates` with a 30-second timeout in a loop, processing `message`, `message_reaction`, and `callback_query` update types. Text messages, photos (largest size), voice notes, audio, and documents are all handled. Voice messages are transcribed via Groq Whisper when a Groq API key is configured; otherwise the raw file path is included.

**Outbound**: Sends HTML-formatted messages via `sendMessage`. Messages exceeding 4096 characters are chunked. If HTML parsing fails, the chunk is retried as plain text.

**Interactions**: Supports `SingleSelect`, `MultiSelect`, `YesNo`, and `FreeText` question types. Select and yes/no questions render as `InlineKeyboardMarkup` buttons with callback data in the format `askuser:{chat_id}:{question_id}:{value}`. Button presses are resolved via `answerCallbackQuery` + editing the original message to show the selection. Free-text responses are intercepted from normal message flow by checking for a pending `FreeText` callback on the chat.

**Commands**: `/start`, `/reset` (publishes a reset system message to clear the session), `/help`.

### Discord

**Protocol**: Raw WebSocket connection to the Discord Gateway (no serenity dependency). Uses `WebSocketManager` with `HeartbeatStrategy::None` because Discord requires its own heartbeat cadence derived from the HELLO payload.

**Inbound**: On HELLO (op 10), spawns a dedicated heartbeat task at the server-specified interval, then sends IDENTIFY with the bot token and configured intents. Processes `MESSAGE_CREATE` and `MESSAGE_REACTION_ADD` dispatch events. Bot messages are filtered out. Media attachments up to 20MB are downloaded and saved to `~/.klyntbot/media/`.

**Outbound**: Sends messages via the REST API (`POST /channels/{id}/messages`). Long messages are chunked to 2000 characters. Supports file attachments via multipart form upload.

**Interactions**: Supports structured interactions via Discord components (buttons and select menus). Uses `InteractionTracker` for callback resolution, with callback data embedded in `custom_id` fields.

### Slack

**Protocol**: Socket Mode WebSocket. Authenticates via `auth.test` to get the bot user ID, then connects to a WebSocket URL obtained from `apps.connections.open`.

**Inbound**: Receives Socket Mode envelopes and immediately ACKs each one. Handles `events_api` envelopes (message and app_mention events) and `interactive` envelopes (block_actions from button/select clicks). In channels and groups, the bot only responds to @mentions or `app_mention` events; in DMs, all messages are processed. Bot mention text is stripped from the message content. An `:eyes:` reaction is added to each processed message.

**Outbound**: Sends via `chat.postMessage` REST API. Thread replies are used for non-DM channels by passing `thread_ts`.

**Interactions**: Block Kit UI -- buttons in an `actions` block for select/yes-no, with `action_id` in the `askuser:{channel}:{question_id}:{value}` format. Free-text interactions send a plain message and wait for the next reply.

### WhatsApp

**Protocol**: WebSocket connection to a Node.js Baileys bridge (default `ws://localhost:3001`). Uses `WebSocketManager` with timeout-based heartbeat.

**Inbound**: Parses JSON messages from the bridge with types `qr` (authentication QR code), `message` (user message), `status`, and `error`. QR codes are logged for terminal scanning.

**Outbound**: Sends JSON payloads (`{"type": "send", "to": ..., "text": ...}`) over the WebSocket. Messages are formatted as plain text and chunked to 4000 characters.

**Interactions**: Not supported (`supports_interaction()` returns false).

### QQ

**Protocol**: WebSocket connection to the QQ Bot API gateway (`wss://api.sgroup.qq.com/websocket`). Authenticates via REST to get an access token before connecting.

**Inbound**: Processes gateway opcodes -- HELLO (op 10), Dispatch (op 0) with event types `READY`, `C2C_MESSAGE_CREATE`, and `DIRECT_MESSAGE_CREATE`. Includes message deduplication via a bounded `VecDeque` (cap 1000) of processed message IDs.

**Outbound**: Sends C2C messages via the REST API (`POST /v2/users/{openid}/messages`). Formatted as plain text, chunked to 4000 characters.

**Interactions**: Not supported.

### Email

**Protocol**: IMAP polling for inbound, SMTP for outbound. Feature-gated behind the `email` Cargo feature (on by default).

**Inbound**: Connects via IMAP (with optional TLS) at a configurable poll interval (minimum 5 seconds, default 30). Searches for `UNSEEN` messages, fetches bodies, parses with `mail_parser`, and extracts plain text (with HTML-to-text fallback via `html2text`). Body is truncated to `max_body_chars` (default 12000). Email metadata (message ID, subject, date) is preserved for threading. UID-based deduplication prevents reprocessing (cap 10000). Requires explicit `consent_granted: true` in config.

**Outbound**: Sends via SMTP with `lettre`. Auto-reply can be disabled via `auto_reply_enabled`. Replies include `In-Reply-To` and `References` headers for email threading. Subject prefixing avoids double `Re:`.

**Interactions**: Not supported.

## How It Works

### Message Flow

1. A user sends a message on any platform.
2. The channel's `start()` loop receives it via the platform's protocol.
3. The channel checks the allowlist. If denied, the message is dropped with a warning log.
4. The channel constructs an `InboundMessage` with the channel name, sender ID, chat ID, and content, then publishes it to the `MessageBus` via `bus.publish_inbound()`.
5. The agent loop (in the `agent` crate) consumes the `InboundMessage`, processes it through the intent pipeline, and publishes an `OutboundMessage` back to the bus.
6. The `ChannelManager`'s outbound dispatcher receives the `OutboundMessage`, looks up the target channel by name, applies the channel-specific formatter, chunks the message if needed, and calls `send()`.

### Structured Interactions

When a tool (like `ask_user`) needs user input with buttons or menus:

1. The tool creates an `InteractionRequest` with typed questions (`SingleSelect`, `YesNo`, `MultiSelect`, `FreeText`).
2. The agent checks if the current channel supports interactions via `supports_interaction()`.
3. If supported, `send_interaction()` is called, which renders platform-native UI (inline keyboards on Telegram, Block Kit on Slack, components on Discord).
4. The `InteractionTracker` registers a `PendingCallback` with a `oneshot::Sender`.
5. When the user responds (button click or text reply), the inbound handler resolves the callback, sending the value through the oneshot channel.
6. `send_interaction()` receives the value and returns a `FormResponse::Completed` with all answers.

### Reconnection

Channels using WebSocket connections (Discord, Slack, WhatsApp, QQ) wrap their `WebSocketManager::run()` call inside `reconnect_loop()`, which retries with a 5-second delay whenever the connection drops, as long as the `running` flag is true.

## Connections

### Dependencies (what channels imports)

- `bus` (Layer 1): `MessageBus`, `InboundMessage`, `OutboundMessage`, `MessageKind`
- `common` (Layer 0): error types, `InteractionRequest`, `FormResponse`, utility functions
- `config` (Layer 1): per-channel config schemas (`TelegramConfig`, `DiscordConfig`, etc.)
- `providers` (Layer 2): `TranscriptionProvider` (Telegram voice transcription)

### Dependents (what imports channels)

- `klyntbot` (Layer 7): constructs `ChannelManager` and starts all channels in `serve` command
