# bus

## Purpose

The `bus` crate is a Layer 1 crate that provides the async message-passing infrastructure connecting chat channels to the agent loop. It decouples message producers (Telegram, Discord, Slack, and other channel integrations) from message consumers (the agent's processing loop) using Tokio mpsc channels. The bus also includes a separate broadcast-based event system for the learning subsystem. No business logic lives here -- it is purely plumbing.

## Key Types

### `MessageBus`

The central message routing structure. It holds two independent Tokio mpsc channel pairs: one for inbound messages (channels to agent) and one for outbound messages (agent to channels).

```
Channel integrations ──publish_inbound()──> [inbound mpsc] ──take_inbound_rx()──> Agent loop
Agent loop ──publish_outbound()──> [outbound mpsc] ──take_outbound_rx()──> Channel router
```

**Construction:**
- `MessageBus::new(buffer_size)` creates both channel pairs with the specified buffer capacity.

**Publishing (producer side):**
- `publish_inbound(msg)` -- validates message size (64 KB limit) and sends to the inbound channel. Returns `KlyntbotError::Bus` if validation fails, `KlyntbotError::BusDisconnected` if the receiver was dropped.
- `publish_outbound(msg)` -- sends to the outbound channel. Returns `BusDisconnected` on channel closure.

**Consuming (consumer side):**
- `take_inbound_rx()` -- takes ownership of the inbound `mpsc::Receiver`. Returns `Option` because it can only be called once (the receiver is wrapped in a `Mutex<Option<...>>`).
- `take_outbound_rx()` -- same pattern for the outbound receiver.

**Sender cloning:**
- `inbound_sender()` -- returns a clone of the inbound `mpsc::Sender`. Channels call this to get their own sender handle.
- `outbound_sender()` -- returns a clone of the outbound `mpsc::Sender`.

### `InboundMessage`

A message received from a chat channel, flowing toward the agent. Fields:

| Field | Type | Purpose |
|-------|------|---------|
| `channel` | `ChannelName` | Which platform sent the message ("telegram", "discord", etc.) |
| `sender_id` | `String` | User identifier within the channel |
| `chat_id` | `ChatId` | Chat/conversation identifier |
| `content` | `String` | Message text (validated to max 64 KB) |
| `timestamp` | `DateTime<Utc>` | When the message was sent (defaults to now) |
| `media` | `Vec<String>` | URLs of attached images, files, etc. |
| `metadata` | `HashMap<String, Value>` | Channel-specific metadata (message IDs, thread info, etc.) |
| `kind` | `MessageKind` | Whether this is a text message or an emoji reaction |

Key methods:
- `InboundMessage::new(channel, sender_id, chat_id, content)` -- convenient constructor with sensible defaults.
- `.with_kind(MessageKind::Reaction)` -- builder method for setting the message kind.
- `.session_key()` -- derives a `SessionKey` ("channel:chat_id") for session lookup.
- `.validate()` -- checks the 64 KB content size limit.

### `OutboundMessage`

A message flowing from the agent back to a chat channel. Fields:

| Field | Type | Purpose |
|-------|------|---------|
| `channel` | `ChannelName` | Target platform |
| `chat_id` | `ChatId` | Target chat/conversation |
| `content` | `String` | Response text |
| `reply_to` | `Option<String>` | Optional message ID to reply to (for threading) |
| `media` | `Vec<String>` | Media URLs to attach |
| `metadata` | `HashMap<String, Value>` | Channel-specific metadata |

Builder methods: `.with_reply_to(id)`, `.with_media(url)`.

### `MessageKind`

A simple enum distinguishing regular text messages from emoji reactions:

- `MessageKind::Text` (default)
- `MessageKind::Reaction`

This allows the agent to handle reactions differently from regular messages (e.g., as feedback signals rather than conversation input).

### `LearningEventBus`

A separate broadcast channel for the learning subsystem. Unlike the mpsc-based `MessageBus` (single consumer), the `LearningEventBus` uses `tokio::sync::broadcast` so that multiple subscribers each receive every event independently.

- `LearningEventBus::new(capacity)` -- creates a broadcast channel (capacity of 16 is typical).
- `.publish(event)` -- sends an event to all current subscribers. No-op if there are no subscribers.
- `.subscribe()` -- returns a new `broadcast::Receiver`. Each subscriber gets its own independent stream.

### `LearningEvent`

Events emitted by the learning service after analysis runs:

- `ThresholdChanged { old_threshold, new_threshold, reason }` -- the adaptive confidence threshold changed, with a human-readable reason.
- `AnalysisCompleted { total_outcomes, suggested_threshold }` -- a full analysis cycle completed (the threshold may or may not have actually changed).

## How It Works

### Message Flow Architecture

The `MessageBus` sits at the center of Klyntbot's runtime, acting as a fully decoupled pipe between the channel layer and the agent layer.

**Startup sequence:**
1. The `klyntbot serve` command creates a `MessageBus` with a configured buffer size.
2. The bus is wrapped in an `Arc` and shared with both sides.
3. Each channel integration (Telegram, Discord, etc.) calls `bus.inbound_sender()` to get a cloned sender. When messages arrive from users, the channel converts them into `InboundMessage` structs and publishes them via `publish_inbound()`.
4. The agent loop calls `bus.take_inbound_rx()` once at startup to take ownership of the receiver. It then loops on `rx.recv()`, processing each inbound message through the intent pipeline.
5. The outbound side mirrors this: the agent publishes responses via `publish_outbound()`, and a channel router takes the outbound receiver and dispatches messages to the appropriate channel's `send()` method based on the `channel` field.

**Key design decisions:**

The inbound and outbound channels are completely independent. Publishing to one does not block or affect the other. Messages from different channels are interleaved on the single inbound channel -- the agent processes them in arrival order, using the `session_key()` to maintain per-conversation state.

### The "Take Once" Pattern

The receivers (`inbound_rx`, `outbound_rx`) are wrapped in `Mutex<Option<mpsc::Receiver>>`. The `take_*_rx()` methods call `.take()` on the `Option`, which returns `Some(receiver)` on the first call and `None` on all subsequent calls. This enforces that exactly one consumer owns each receiver, which is a requirement of Tokio's mpsc channels (they are multi-producer, single-consumer).

This pattern avoids the need for `Arc<Mutex<Receiver>>` at the consumption site. The consumer takes full ownership and can call `.recv()` without any locking overhead in the hot path.

### Message Validation

Inbound messages are validated before entering the channel. Currently, the only validation is a 64 KB size limit on `content` (`MAX_MESSAGE_SIZE = 65536`). This prevents a single oversized message from consuming excessive memory in the buffer. The validation happens at the `publish_inbound()` call site, returning an error to the channel integration before the message enters the mpsc channel.

### Backpressure

The mpsc channels have a bounded buffer (set at construction). When the buffer is full, `publish_inbound()` will await until a slot opens up. This provides natural backpressure -- if the agent cannot process messages fast enough, channels will slow down their publishing rate. This prevents unbounded memory growth under load.

### Learning Event Broadcasting

The `LearningEventBus` serves a different purpose than the `MessageBus`. While message flow is point-to-point (many channels to one agent), learning events are broadcast (one learning service to many subscribers). The agent loop subscribes to receive threshold change notifications so it can update its confidence settings in real time. Future consumers (dashboards, monitoring) can subscribe independently without affecting each other.

The broadcast channel is fire-and-forget on the producer side -- if no subscribers exist, events are silently dropped. If a subscriber falls behind (its internal buffer fills up), it will receive a `Lagged` error indicating missed events rather than blocking the producer.

## Connections

**Depends on:** `common` (for `ChannelName`, `ChatId`, `SessionKey`, `KlyntbotError`, `Result`).

**Depended on by:** The `channels` crate uses the inbound sender to publish user messages. The `agent` crate takes the inbound receiver to process messages and uses the outbound sender to publish responses. The `klyntbot` facade crate creates the bus and wires both sides together at startup. The learning subsystem in the `agent` crate publishes to and subscribes from the `LearningEventBus`.
