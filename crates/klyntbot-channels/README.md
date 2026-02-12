# klyntbot-channels

**Channel trait and chat platform integrations.**

## Overview

`klyntbot-channels` provides chat platform integrations for klyntbot:
- `Channel` trait for platform implementations
- 6 ready channels + 3 planned
- Async WebSocket and HTTP transports
- Automatic reconnection and rate limiting
- Attachment handling (images, documents, voice)

## Contents

### Channel Trait

```rust
use klyntbot_channels::Channel;
use klyntbot_bus::{InboundMessage, OutboundMessage};
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
| **WhatsApp** | WebSocket bridge | ✅ Ready | QR code auth, media download |
| **Slack** | Socket Mode | ✅ Ready | DM/group policy, thread replies |
| **Email** | IMAP + SMTP | ✅ Ready | HTML parsing, threading, consent gate |
| **QQ** | WebSocket (botpy) | ✅ Ready | C2C private messages, sandbox mode |
| **Feishu** | WebSocket | 🔧 Planned | Lark long connection |
| **DingTalk** | Stream Mode | 🔧 Planned | OAuth2, batch send |
| **Mochat** | Socket.IO | 🔧 Planned | Reply delay, cursor tracking |

### Channel Manager

```rust
use klyntbot_channels::{ChannelManager, start_channels};
use klyntbot_config::Config;
use klyntbot_bus::MessageBus;

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
klyntbot-channels.workspace = true

# Optional: email channel
klyntbot-channels = { workspace = true, features = ["email"] }
```

Example:

```rust
use klyntbot_channels::telegram::TelegramChannel;
use klyntbot_config::Config;
use klyntbot_bus::MessageBus;

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

### WhatsApp

**Transport**: WebSocket bridge (Baileys)
**Features**:
- QR code authentication
- Media download
- Group message handling

**Requires**: Node.js bridge running on `ws://localhost:3001`

```toml
[channels.whatsapp]
enabled = true
bridgeUrl = "ws://localhost:3001"
allowFrom = ["phone_number"]
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

### QQ

**Transport**: WebSocket (botpy)
**Features**:
- C2C private messages
- Sandbox mode support

```toml
[channels.qq]
enabled = true
appId = "your_app_id"
secret = "your_secret"
allowFrom = ["user_id"]
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

WebSocket channels (Discord, Slack, QQ) auto-reconnect:

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
cargo build --no-default-features -p klyntbot-channels
```

## Design Principles

1. **Unified interface** — All platforms implement `Channel` trait
2. **Async-native** — Built on Tokio for non-blocking I/O
3. **Auto-reconnect** — Resilient to network failures
4. **Access control** — Per-channel allowlists
5. **Rich media** — Handle attachments, voice, images

## Dependencies

- `klyntbot-core` — Error types, shared types
- `klyntbot-bus` — Message bus integration
- `klyntbot-config` — Configuration loading
- `klyntbot-providers` — Transcription (Telegram voice)
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
