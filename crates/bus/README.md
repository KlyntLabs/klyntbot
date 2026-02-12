# bus

**Async message bus for channel ↔ agent communication.**

## Overview

`bus` provides the message passing infrastructure for klyntbot:
- Inbound messages from channels to agent
- Outbound messages from agent to channels
- Asynchronous MPSC queues with backpressure
- Message routing and multiplexing

## Contents

### Message Types

```rust
use bus::{InboundMessage, OutboundMessage};
use common::{ChannelName, ChatId};

// Messages from channels TO agent
pub struct InboundMessage {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub user_id: Option<String>,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub timestamp: DateTime<Utc>,
}

// Messages from agent TO channels
pub struct OutboundMessage {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub content: String,
    pub reply_to: Option<String>,
}
```

### Message Bus

```rust
use bus::MessageBus;
use tokio::sync::mpsc;

// Create message bus
let (bus, inbound_rx, outbound_tx) = MessageBus::new(100);

// Channels send inbound messages
let inbound_tx = bus.inbound_sender();
inbound_tx.send(InboundMessage { ... }).await?;

// Agent receives inbound messages
while let Some(msg) = inbound_rx.recv().await {
    process_message(msg).await?;
}

// Agent sends outbound messages
outbound_tx.send(OutboundMessage { ... }).await?;

// Channels receive outbound messages
let outbound_rx = bus.outbound_receiver();
while let Some(msg) = outbound_rx.recv().await {
    send_to_channel(msg).await?;
}
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
bus.workspace = true
```

Example:

```rust
use bus::{MessageBus, InboundMessage, OutboundMessage};
use common::{ChannelName, ChatId};

#[tokio::main]
async fn main() {
    let (bus, mut inbound_rx, outbound_tx) = MessageBus::new(100);

    // Spawn agent task
    tokio::spawn(async move {
        while let Some(msg) = inbound_rx.recv().await {
            println!("Agent received: {}", msg.content);

            // Send response
            outbound_tx.send(OutboundMessage {
                channel: msg.channel,
                chat_id: msg.chat_id,
                content: "Response".into(),
                reply_to: None,
            }).await.ok();
        }
    });

    // Send message from channel
    bus.inbound_sender().send(InboundMessage {
        channel: ChannelName::Cli,
        chat_id: ChatId("user123".into()),
        user_id: Some("user123".into()),
        content: "Hello!".into(),
        attachments: vec![],
        timestamp: Utc::now(),
    }).await.ok();
}
```

## Architecture

### Flow Diagram

```
┌───────────┐         ┌─────────────┐         ┌──────────┐
│ Channels  │         │ MessageBus  │         │  Agent   │
│           │         │             │         │          │
│ Telegram  │─Inbound→│  Queue(100) │────────→│AgentLoop │
│ Discord   │         │             │         │          │
│ Slack     │         │             │         │          │
│           │←Outbound│  Queue(100) │←────────│          │
└───────────┘         └─────────────┘         └──────────┘
```

### Backpressure

Queues are **bounded** with capacity 100:
- If queue is full, `send()` waits (backpressure)
- Prevents memory exhaustion from message floods
- Preserves message ordering (FIFO)

### Single Consumer Pattern

- **Inbound**: Multiple channels send → single agent receives
- **Outbound**: Single agent sends → multiple channels receive (cloned receivers)

## Design Principles

1. **Async-native** — Built on `tokio::sync::mpsc`
2. **Bounded queues** — Backpressure prevents memory issues
3. **Type-safe routing** — Channel and chat ID in every message
4. **Ordered delivery** — MPSC queues preserve message order
5. **Attachment support** — Images, documents, voice messages

## Message Attachments

```rust
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}
```

Channels can send:
- Images (PNG, JPG)
- Documents (PDF, TXT)
- Voice messages (transcribed via Groq Whisper)

## Dependencies

- `common` — Error types, shared types
- `tokio` — Async runtime and MPSC channels
- `serde`, `serde_json` — Serialization
- `chrono` — Timestamps

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Channel Development](../channels/README.md)
