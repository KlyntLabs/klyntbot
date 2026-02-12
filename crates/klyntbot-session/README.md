# klyntbot-session

**Conversation session persistence and management.**

## Overview

`klyntbot-session` manages conversation history:
- Per-channel, per-user session storage
- JSONL file format for persistence
- In-memory LRU cache for active sessions
- Configurable history depth
- Session metadata tracking

## Contents

### Session Management

```rust
use klyntbot_session::{SessionManager, SessionKey};
use klyntbot_core::{ChannelName, ChatId};

// Create session manager
let session_dir = PathBuf::from("~/.klyntbot/sessions");
let manager = SessionManager::new(session_dir, 50);  // 50 message history

// Get or create session
let key = SessionKey::new(ChannelName::Telegram, ChatId("user123".into()));
let session = manager.get_session(&key).await?;

// Add message to session
session.add_message(MessageRole::User, "Hello!").await?;
session.add_message(MessageRole::Assistant, "Hi there!").await?;

// Get session history
let messages = session.get_messages();
for msg in messages {
    println!("{:?}: {}", msg.role, msg.content);
}
```

### Session Types

```rust
pub struct Session {
    key: SessionKey,
    messages: Vec<SessionMessage>,
    metadata: SessionInfo,
}

pub struct SessionMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

pub struct SessionInfo {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}
```

### Storage Format

Sessions are stored as JSONL (one JSON object per line):

```
~/.klyntbot/sessions/telegram_user123.jsonl
```

```jsonl
{"role":"user","content":"Hello!","timestamp":"2026-02-12T10:00:00Z"}
{"role":"assistant","content":"Hi there!","timestamp":"2026-02-12T10:00:01Z"}
{"role":"user","content":"How are you?","timestamp":"2026-02-12T10:00:05Z"}
```

### LRU Caching

Sessions are cached in memory with LRU eviction:

```rust
// Cache holds up to N active sessions
let manager = SessionManager::new(session_dir, 50);

// First access loads from disk
let session1 = manager.get_session(&key1).await?;  // Disk read

// Subsequent access uses cache
let session1_again = manager.get_session(&key1).await?;  // Cache hit

// Cache eviction on overflow
let session_100 = manager.get_session(&key100).await?;  // Evicts oldest
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-session.workspace = true
```

Example:

```rust
use klyntbot_session::{SessionManager, SessionKey};
use klyntbot_core::{ChannelName, ChatId, MessageRole};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let session_dir = PathBuf::from("/tmp/sessions");
    let manager = SessionManager::new(session_dir, 50);

    let key = SessionKey::new(
        ChannelName::Cli,
        ChatId("test_user".into())
    );

    let session = manager.get_session(&key).await?;

    // Add conversation
    session.add_message(MessageRole::User, "What is Rust?").await?;
    session.add_message(
        MessageRole::Assistant,
        "Rust is a systems programming language..."
    ).await?;

    // Session persisted to: /tmp/sessions/cli_test_user.jsonl

    Ok(())
}
```

## History Management

### Depth Limiting

Sessions maintain a rolling window of messages:

```rust
let manager = SessionManager::new(session_dir, 20);  // Keep last 20 messages

// Older messages are automatically truncated when limit exceeded
```

### Manual Truncation

```rust
// Clear session history
session.clear().await?;

// Trim to last N messages
session.truncate(10).await?;
```

## Concurrency

Session operations are **async** and **thread-safe**:

- Multiple tasks can access different sessions concurrently
- Same session is synchronized via internal locks
- JSONL appends are atomic

## Design Principles

1. **JSONL format** — Human-readable, append-efficient, diff-friendly
2. **LRU caching** — Active sessions in memory, inactive on disk
3. **Rolling window** — Configurable message history depth
4. **Async I/O** — Non-blocking file operations with Tokio
5. **Per-session files** — Independent storage per channel+chat

## File Naming

Session filenames encode the session key:

```
Format: {channel}_{chat_id}.jsonl

Examples:
telegram_123456789.jsonl
discord_987654321.jsonl
cli_user_test.jsonl
```

## Dependencies

- `klyntbot-core` — Error types, shared types
- `serde`, `serde_json` — Serialization
- `tokio` — Async file I/O
- `chrono` — Timestamps
- `tracing` — Logging

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Agent Loop](../klyntbot-agent/README.md)
