# klyntbot-core

**Foundation types and error handling for klyntbot.**

## Overview

`klyntbot-core` is the foundational layer of klyntbot's workspace architecture. It provides:
- Unified error types used across all crates
- Shared type definitions (channels, sessions, messages)
- Pure utility functions (no I/O dependencies)

This crate has no internal dependencies (only depends on standard external crates like `serde`, `thiserror`).

## Contents

### Error Types

```rust
use klyntbot_core::{KlyntbotError, Result};

// Top-level error enum with automatic conversions
pub enum KlyntbotError {
    Tool(ToolError),
    Provider(ProviderError),
    Channel(ChannelError),
    Session(SessionError),
    Config(ConfigError),
    Cron(CronError),
    Internal(String),
}

// Standardized Result type
pub type Result<T> = std::result::Result<T, KlyntbotError>;
```

**Domain-specific errors:**
- `ToolError` — Tool execution failures
- `ProviderError` — LLM provider errors (API, rate limits, auth)
- `ChannelError` — Channel connection/messaging errors
- `SessionError` — Session persistence errors
- `ConfigError` — Configuration loading/validation errors
- `CronError` — Cron scheduling errors

### Shared Types

```rust
use klyntbot_core::{ChannelName, ChatId, SessionKey, MessageRole};

// Channel identification
pub enum ChannelName {
    Telegram,
    Discord,
    Slack,
    WhatsApp,
    Email,
    QQ,
    Cli,
}

// Chat identifier (platform-specific)
pub struct ChatId(String);

// Session key (unique per channel + chat)
pub struct SessionKey {
    pub channel: ChannelName,
    pub chat_id: ChatId,
}

// Message roles for LLM conversations
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}
```

### Utility Functions

```rust
use klyntbot_core::utils::{truncate_output, expand_path, format_error};

// Terminal formatting helpers
pub fn format_error(msg: &str) -> String;
pub fn format_success(msg: &str) -> String;

// Path utilities
pub fn expand_path(path: &str) -> Result<PathBuf>;

// Output truncation
pub fn truncate_output(output: &str, max_len: usize) -> String;
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-core.workspace = true
```

Example:

```rust
use klyntbot_core::{Result, KlyntbotError, ChannelName, ChatId};

fn process_message(channel: ChannelName, chat_id: ChatId) -> Result<String> {
    if chat_id.0.is_empty() {
        return Err(KlyntbotError::Internal("Empty chat ID".into()));
    }

    Ok(format!("Processed message from {:?}", channel))
}
```

## Error Conversion

All domain errors automatically convert to `KlyntbotError`:

```rust
use klyntbot_core::{Result, ToolError, ProviderError};

fn call_tool() -> Result<String> {
    Err(ToolError::ExecutionFailed("Tool crashed".into()).into())
}

fn call_provider() -> Result<String> {
    Err(ProviderError::AuthFailed.into())
}
```

## Design Principles

1. **Zero internal dependencies** — Only depends on `serde`, `thiserror`, etc.
2. **Lightweight** — Fast to compile, minimal overhead
3. **Shared foundation** — Used by all other klyntbot crates
4. **Pure utilities** — No I/O, async, or runtime dependencies in utils

## Dependencies

- `thiserror` — Error derive macros
- `serde` — Serialization framework
- `serde_json` — JSON serialization

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Migration Guide](../../docs/MIGRATION.md)
