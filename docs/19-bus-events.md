# Bus Crate — Async Message Bus & Event Types

The `bus` crate (`crates/bus/`) provides the asynchronous message bus that decouples chat channels from the agent loop. It sits at Layer 1 of the workspace dependency graph, depending only on `common` (Layer 0) and external crates (`tokio`, `serde`, `chrono`, `uuid`). Total size is approximately 1,400 LOC across four source files.

---

## Section 1: Narrative Overview

### What This Crate Does

The bus crate solves a fundamental architectural problem: chat channels (Telegram, Discord, Slack, WhatsApp, Email, QQ) must push messages into the system, and the agent loop must consume and respond to them, without either side knowing about the other. The bus provides two independent, bounded, async queues — one inbound (channels to agent) and one outbound (agent to channels) — plus a separate broadcast bus for internal learning events.

### MessageBus Design

`MessageBus` wraps two pairs of `tokio::sync::mpsc` channels, each with a configurable buffer size. In production, the buffer is set to 100 (`crates/cli/src/serve.rs`, line 41):

```rust
let bus = Arc::new(MessageBus::new(100));
```

The design uses single-consumer semantics for both directions:

- **Inbound channel**: Multiple chat platform tasks publish `InboundMessage`s concurrently via cloned senders. The single receiver is extracted by `AgentLoop` to drive the main processing loop.
- **Outbound channel**: The agent publishes `OutboundMessage`s after processing. The single receiver is extracted by `ChannelManager`, which dispatches each message to the correct platform channel.

Each receiver is wrapped in `Mutex<Option<Receiver>>` and extracted via `take()` — guaranteeing at most one consumer per direction at runtime. Attempting to take a receiver twice returns `None`.

### Event Types and Their Lifecycle

There are two message types with distinct lifecycles:

**InboundMessage** — born when a chat platform receives a user message. The channel adapter constructs it with the platform name, sender ID, chat ID, and text content. Optional fields include media URLs (for images/files) and a metadata map for channel-specific extras. Before entering the bus, the message is validated against a 64 KB size limit (`MAX_MESSAGE_SIZE = 65536`). The `session_key()` method derives a `SessionKey` in the format `channel:chat_id`, which the agent uses to route messages to the correct conversation session.

**OutboundMessage** — born inside the agent loop after the LLM produces a response. It carries the target channel name, chat ID, response text, an optional `reply_to` message ID, and optional media URLs. The agent publishes it to the outbound queue; the `ChannelManager`'s dispatcher loop picks it up and calls the appropriate channel's `send()` method.

### How Channels Push Inbound Messages

Each channel adapter receives a shared `Arc<MessageBus>` when started. When a message arrives from the platform (e.g., a Telegram update), the channel constructs an `InboundMessage` and calls `bus.publish_inbound(msg).await`. This validates the message size and sends it into the mpsc channel. If the buffer is full, the call awaits backpressure (the sender suspends until the agent loop consumes a message).

Channels can also obtain a raw `mpsc::Sender<InboundMessage>` via `bus.inbound_sender()` for cases where they need to pass the sender to a spawned task without carrying the full `MessageBus` reference.

Example from the Telegram channel (`crates/channels/src/telegram.rs`, lines 410-415):

```rust
let mut inbound = InboundMessage::new("telegram", sender_id, chat_id_str, content);
inbound.media = media_paths;
bus.publish_inbound(inbound).await
    .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;
```

### How the Agent Consumes and Responds

At startup, the `AgentLoop` extracts the inbound receiver via `take_inbound_rx()` before being wrapped in `Arc`. The main loop (`run_with_rx`) calls `inbound_rx.recv().await` in a loop, processing each message through the intent pipeline (heuristic analysis, LLM classification, context assembly, engine execution). After producing a response, it constructs an `OutboundMessage` and calls `self.bus.publish_outbound(out_msg).await` (`crates/agent/src/agent_loop/mod.rs`, line 303).

The `ChannelManager` extracts the outbound receiver in its constructor (`crates/channels/src/manager.rs`, line 62) and spawns a dispatcher task that loops on `outbound_rx.recv().await`, matching each message's `channel` field to the registered channel and calling `channel.send(&msg).await` (`crates/channels/src/manager.rs`, lines 168-210). If delivery fails, it sends a user-facing error message back through the same channel.

Additionally, the `NotificationDispatcher` (`crates/agent/src/notifications.rs`, line 12) holds a cloned `mpsc::Sender<OutboundMessage>` obtained via `bus.outbound_sender()`, allowing the reminder engine and cron subsystem to push notifications through the same outbound pipeline without going through the agent loop.

### Learning Events Subsystem

Separate from the main message bus, the `LearningEventBus` uses `tokio::sync::broadcast` for fan-out delivery to multiple independent subscribers. Unlike the mpsc-based `MessageBus`, every subscriber receives every event.

The `LearningService` (`crates/agent/src/learning/service.rs`) runs periodic analysis of outcome data (success rates, user feedback) to tune the adaptive confidence threshold. After each analysis cycle, it publishes events through the `LearningEventBus`. The `AgentLoop` builder subscribes to these events and updates its confidence threshold atomically when a `ThresholdChanged` event arrives (`crates/agent/src/agent_loop/builder.rs`, line 650-655).

The learning event bus is created with a capacity of 16, which is sufficient since analysis cycles run infrequently (default: hourly).

---

## Section 2: API Reference

### Module Structure

```
crates/bus/src/
  lib.rs              — Re-exports; declares pub modules
  events.rs           — InboundMessage, OutboundMessage, MessageKind, MAX_MESSAGE_SIZE
  queue.rs            — MessageBus struct
  learning_events.rs  — LearningEvent enum, LearningEventBus struct
```

**`lib.rs`** (`crates/bus/src/lib.rs`, lines 1-11) re-exports:
- `events::{InboundMessage, MessageKind, OutboundMessage}`
- `learning_events::{LearningEvent, LearningEventBus}`
- `queue::MessageBus`

---

### `MessageBus`

**File:** `crates/bus/src/queue.rs`, lines 12-86

```rust
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<Option<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Mutex<Option<mpsc::Receiver<OutboundMessage>>>,
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(buffer_size: usize) -> Self` | Creates a bus with two mpsc channel pairs, each with `buffer_size` capacity. (line 21) |
| `take_inbound_rx` | `fn take_inbound_rx(&self) -> Option<Receiver<InboundMessage>>` | Extracts the inbound receiver. Returns `None` on second call. (line 34) |
| `take_outbound_rx` | `fn take_outbound_rx(&self) -> Option<Receiver<OutboundMessage>>` | Extracts the outbound receiver. Returns `None` on second call. (line 39) |
| `publish_inbound` | `async fn publish_inbound(&self, msg: InboundMessage) -> Result<()>` | Validates message size, logs via `tracing::debug`, sends to inbound channel. Returns `KlyntbotError::Bus` if validation fails, `KlyntbotError::BusDisconnected` if receiver is dropped. (line 44) |
| `publish_outbound` | `async fn publish_outbound(&self, msg: OutboundMessage) -> Result<()>` | Logs via `tracing::debug`, sends to outbound channel. Returns `KlyntbotError::BusDisconnected` if receiver is dropped. (line 62) |
| `inbound_sender` | `fn inbound_sender(&self) -> mpsc::Sender<InboundMessage>` | Returns a cloned sender handle for inbound messages. Useful for passing to spawned tasks. (line 78) |
| `outbound_sender` | `fn outbound_sender(&self) -> mpsc::Sender<OutboundMessage>` | Returns a cloned sender handle for outbound messages. Used by `NotificationDispatcher`. (line 83) |

**Error variants used:**
- `KlyntbotError::Bus(String)` — message validation failure (content exceeds 64 KB). Defined in `crates/common/src/error.rs`, line 9.
- `KlyntbotError::BusDisconnected` — the receiver half has been dropped. Defined in `crates/common/src/error.rs`, line 12.

---

### `InboundMessage`

**File:** `crates/bus/src/events.rs`, lines 22-94

```rust
pub struct InboundMessage {
    pub channel: ChannelName,
    pub sender_id: String,
    pub chat_id: ChatId,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub media: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub kind: MessageKind,
}
```

#### Fields

| Field | Type | Description |
|-------|------|-------------|
| `channel` | `ChannelName` | Platform identifier (e.g., "telegram", "discord", "slack", "whatsapp", "email", "qq"). Newtype over `String` from `common`. |
| `sender_id` | `String` | User identifier within the channel (platform-specific format). |
| `chat_id` | `ChatId` | Chat/group/conversation identifier. Newtype over `String` from `common`. |
| `content` | `String` | Message text. Must be at most 65,536 bytes (validated by `publish_inbound`). |
| `timestamp` | `DateTime<Utc>` | Message creation time. Defaults to `Utc::now()` via `#[serde(default = "Utc::now")]`. |
| `media` | `Vec<String>` | URLs for attached images, files, or other media. Empty by default. |
| `metadata` | `HashMap<String, Value>` | Channel-specific metadata (e.g., Telegram message IDs, Discord guild info). Empty by default. |
| `kind` | `MessageKind` | Whether this is a text message or an emoji reaction. Defaults to `Text`. |

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(channel, sender_id, chat_id, content) -> Self` | Constructor with required fields. All params accept `impl Into<T>`. Timestamp set to now, media/metadata empty, kind is `Text`. (line 54) |
| `with_kind` | `fn with_kind(self, kind: MessageKind) -> Self` | Builder method to set the message kind. (line 73) |
| `session_key` | `fn session_key(&self) -> SessionKey` | Derives a `SessionKey` as `"channel:chat_id"`. Used by the agent loop for session routing. (line 79) |
| `validate` | `fn validate(&self) -> Result<(), String>` | Checks `content.len() <= MAX_MESSAGE_SIZE` (65,536 bytes). Returns human-readable error string on failure. (line 84) |

---

### `OutboundMessage`

**File:** `crates/bus/src/events.rs`, lines 97-149

```rust
pub struct OutboundMessage {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub content: String,
    pub reply_to: Option<String>,
    pub media: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

#### Fields

| Field | Type | Description |
|-------|------|-------------|
| `channel` | `ChannelName` | Target platform for delivery. Must match a registered channel in `ChannelManager`. |
| `chat_id` | `ChatId` | Target chat/conversation. |
| `content` | `String` | Response text to send. |
| `reply_to` | `Option<String>` | Platform-specific message ID to reply to. Skipped in serialization when `None`. |
| `media` | `Vec<String>` | URLs of media to attach to the response. Empty by default. |
| `metadata` | `HashMap<String, Value>` | Channel-specific send options. Empty by default. |

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(channel, chat_id, content) -> Self` | Constructor with required fields. All params accept `impl Into<T>`. (line 123) |
| `with_reply_to` | `fn with_reply_to(self, message_id) -> Self` | Builder method to set `reply_to`. (line 139) |
| `with_media` | `fn with_media(self, media_url) -> Self` | Builder method to append a media URL. Chainable. (line 145) |

---

### `MessageKind`

**File:** `crates/bus/src/events.rs`, lines 13-18

```rust
pub enum MessageKind {
    #[default]
    Text,
    Reaction,
}
```

| Variant | Description |
|---------|-------------|
| `Text` | Standard text message (default). |
| `Reaction` | Emoji reaction on a previous message (e.g., a thumbs-up). Content holds the emoji string. |

Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Default`, `Serialize`, `Deserialize`.

---

### `MAX_MESSAGE_SIZE`

**File:** `crates/bus/src/events.rs`, line 10

```rust
pub const MAX_MESSAGE_SIZE: usize = 65536;
```

Maximum allowed byte length for `InboundMessage.content`. Enforced by `InboundMessage::validate()`, which is called inside `MessageBus::publish_inbound()`.

---

### `LearningEvent`

**File:** `crates/bus/src/learning_events.rs`, lines 12-25

```rust
pub enum LearningEvent {
    ThresholdChanged {
        old_threshold: f32,
        new_threshold: f32,
        reason: String,
    },
    AnalysisCompleted {
        total_outcomes: usize,
        suggested_threshold: f32,
    },
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `ThresholdChanged` | `old_threshold: f32`, `new_threshold: f32`, `reason: String` | Emitted when the adaptive confidence threshold changes. The `reason` field is a human-readable tag (e.g., `"adaptive_analysis"`). |
| `AnalysisCompleted` | `total_outcomes: usize`, `suggested_threshold: f32` | Emitted after every analysis cycle, regardless of whether the threshold changed. Reports the number of outcomes analyzed and the suggested threshold. |

Derives: `Debug`, `Clone`, `Serialize`, `Deserialize`.

---

### `LearningEventBus`

**File:** `crates/bus/src/learning_events.rs`, lines 31-59

```rust
pub struct LearningEventBus {
    tx: broadcast::Sender<LearningEvent>,
}
```

Uses `tokio::sync::broadcast` for multi-consumer fan-out. Unlike the mpsc-based `MessageBus`, every subscriber receives an independent copy of every published event.

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(capacity: usize) -> Self` | Creates a broadcast bus. Production uses capacity 16 (`crates/agent/src/agent_loop/builder.rs`, line 650). (line 40) |
| `publish` | `async fn publish(&self, event: LearningEvent)` | Sends an event to all current subscribers. Silently no-ops if there are no subscribers (does not error). (line 48) |
| `subscribe` | `fn subscribe(&self) -> broadcast::Receiver<LearningEvent>` | Returns a new independent receiver. Each receiver gets all events published after the subscribe call. (line 56) |

**Note:** `LearningEventBus` is typically wrapped in `Arc` and shared between the `LearningService` (producer) and `AgentLoop` (consumer). The `AgentLoop` builder spawns a background task that reads from its subscription and updates the confidence threshold atomically via `AtomicU32` when `ThresholdChanged` events arrive.
