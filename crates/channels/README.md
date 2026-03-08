# channels

**Channel trait and chat platform integrations.**

## Overview

`channels` provides chat platform integrations for klyntbot:
- `Channel` trait for platform implementations
- 6 ready channels + 3 planned
- Async WebSocket and HTTP transports
- Automatic reconnection and rate limiting
- Attachment handling (images, documents, voice)

## Contents

### Channel Trait

```rust
use channels::Channel;
use bus::{InboundMessage, OutboundMessage};
use async_trait::async_trait;

#[async_trait]
pub trait Channel: Send + Sync {
    async fn start(&self, outbound_rx: mpsc::Receiver<OutboundMessage>) -> Result<()>;
    fn name(&self) -> ChannelName;
}
```

### Supported Channels

| Channel | Transport | Status | Features |
|---------|-----------|:------:|----------|
| **Telegram** | Bot API (long polling) | ✅ Ready | Voice transcription, markdown, typing indicators |
| **Discord** | WebSocket Gateway v10 | ✅ Ready | Auto-reconnect, rate limits, attachments |
| **Slack** | Socket Mode | ✅ Ready | DM/group policy, thread replies |
| **Email** | IMAP + SMTP | ✅ Ready | HTML parsing, threading, consent gate |
| **Feishu** | WebSocket | 🔧 Planned | Lark long connection |
| **DingTalk** | Stream Mode | 🔧 Planned | OAuth2, batch send |
| **Mochat** | Socket.IO | 🔧 Planned | Reply delay, cursor tracking |

### Channel Manager

```rust
use channels::{ChannelManager, start_channels};
use config::Config;
use bus::MessageBus;

// Start all enabled channels
let config = Config::load()?;
let bus = MessageBus::new(100);

let channels = start_channels(&config, &bus).await?;
println!("Started {} channels", channels.len());

// Channels run independently, multiplexed via message bus
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
channels.workspace = true

# Optional: email channel
channels = { workspace = true, features = ["email"] }
```

Example:

```rust
use channels::telegram::TelegramChannel;
use config::Config;
use bus::MessageBus;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let (bus, inbound_rx, outbound_tx) = MessageBus::new(100);

    // Start Telegram channel
    if config.channels.telegram.enabled {
        let telegram = TelegramChannel::new(&config, &bus)?;
        tokio::spawn(async move {
            telegram.start(outbound_rx).await
        });
    }

    // Process messages
    while let Some(msg) = inbound_rx.recv().await {
        println!("Received: {}", msg.content);
    }

    Ok(())
}
```

## Channel Implementations

### Telegram

**Transport**: Long polling (HTTPS)
**Features**:
- Voice message transcription via Groq Whisper
- Markdown-to-HTML conversion
- Typing indicators
- Proxy support
- Attachment download

```toml
[channels.telegram]
enabled = true
token = "123456:ABC-DEF..."
allowFrom = ["user_id_1", "user_id_2"]
proxy = "http://proxy:8080"  # Optional
```

### Discord

**Transport**: WebSocket Gateway v10
**Features**:
- Auto-reconnect with exponential backoff
- Rate limit handling
- Attachment download
- Typing indicators
- Intents: GUILD_MESSAGES, DIRECT_MESSAGES, MESSAGE_CONTENT

```toml
[channels.discord]
enabled = true
token = "Bot YOUR_BOT_TOKEN"
allowFrom = ["user_id_1"]
gatewayUrl = "wss://gateway.discord.gg/?v=10&encoding=json"
intents = 37377  # Required intents bitmask
```

### Slack

**Transport**: Socket Mode (WebSocket)
**Features**:
- DM and group policy (mention/open/allowlist)
- Thread-based replies
- Markdown formatting
- App token + bot token auth

```toml
[channels.slack]
enabled = true
botToken = "xoxb-..."
appToken = "xapp-..."
groupPolicy = "mention"  # mention | open | allowlist
dm = { enabled = true, policy = "open" }
```

### Email

**Transport**: IMAP (receive) + SMTP (send)
**Features**:
- HTML-to-text conversion
- In-Reply-To threading
- Auto-reply toggle
- Consent gate

```toml
[channels.email]
enabled = true
consentGranted = true  # Required for sending
imapHost = "imap.gmail.com"
imapPort = 993
smtpHost = "smtp.gmail.com"
smtpPort = 587
fromAddress = "bot@example.com"
allowFrom = ["allowed@example.com"]
```

## Access Control

### Allow Lists

Each channel supports `allowFrom` to restrict access:

```toml
allowFrom = ["user_id_1", "user_id_2"]  # Only these users
allowFrom = []                          # Allow all (default)
```

### Policy Types (Slack)

- **`mention`** — Respond only when bot is mentioned
- **`open`** — Respond to all messages in channel
- **`allowlist`** — Respond only to users in `allowFrom`

## Attachment Handling

Channels download and pass attachments to agent:

```rust
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}
```

**Supported types**:
- Images: PNG, JPG, GIF
- Documents: PDF, TXT, MD
- Voice: OGG (Telegram), MP3 (transcribed via Groq)

## Reconnection Logic

WebSocket channels (Discord, Slack) auto-reconnect:

```rust
async fn reconnect_loop<F>(connect_fn: F)
where
    F: Fn() -> BoxFuture<'static, Result<()>>,
{
    let mut backoff = 1;
    loop {
        match connect_fn().await {
            Ok(_) => backoff = 1,
            Err(e) => {
                eprintln!("Connection failed: {}, retrying in {}s", e, backoff);
                sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(60);  // Exponential backoff, max 60s
            }
        }
    }
}
```

## Feature Flags

Email channel deps are optional:

```toml
[features]
default = ["email"]
email = ["dep:async-imap", "dep:lettre", "dep:mail-parser", "dep:native-tls", "dep:tokio-native-tls"]
```

Build without email:
```bash
cargo build --no-default-features -p channels
```

## Design Principles

1. **Unified interface** — All platforms implement `Channel` trait
2. **Async-native** — Built on Tokio for non-blocking I/O
3. **Auto-reconnect** — Resilient to network failures
4. **Access control** — Per-channel allowlists
5. **Rich media** — Handle attachments, voice, images

## Dependencies

- `common` — Error types, shared types
- `bus` — Message bus integration
- `config` — Configuration loading
- `providers` — Transcription (Telegram voice)
- `async-trait` — Async trait support
- `tokio`, `tokio-tungstenite` — Async runtime and WebSocket
- `reqwest` — HTTP client
- `serde`, `serde_json` — Serialization
- `regex` — Message parsing
- `tracing` — Logging
- **Email deps** (optional): `async-imap`, `lettre`, `mail-parser`, `native-tls`

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Channel Setup](../../README.md#channels)
- [Extending klyntbot](../../docs/ARCHITECTURE.md#extension-points)
