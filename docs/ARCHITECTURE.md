# Klyntbot Architecture

This document describes the architecture of klyntbot's multi-crate workspace, explaining the design decisions, dependency relationships, and extension points.

## Table of Contents

1. [Overview](#overview)
2. [Workspace Structure](#workspace-structure)
3. [Crate Dependency Graph](#crate-dependency-graph)
4. [Core Abstractions](#core-abstractions)
5. [Extension Points](#extension-points)
6. [Build System](#build-system)
7. [Testing Strategy](#testing-strategy)
8. [Design Patterns](#design-patterns)

---

## Overview

Klyntbot is structured as a Cargo workspace with 12 crates organized into 8 dependency layers. This architecture enables:

- **Parallel compilation** — Independent crates in the same layer compile simultaneously
- **Incremental builds** — Changes to one crate only recompile dependents
- **Clear boundaries** — Each crate has a well-defined responsibility
- **Dependency inversion** — Higher layers depend on traits defined in lower layers
- **Feature flags** — Optional functionality can be disabled at compile time

### Design Philosophy

1. **Foundation first** — `common` contains only zero-dependency types and errors
2. **One-way dependencies** — Dependencies flow upward through layers (no cycles)
3. **Trait-based extensibility** — Adding providers, tools, or channels means implementing a trait
4. **Explicit is better** — Handler traits make dependencies visible in type signatures
5. **Re-export facade** — The root `klyntbot` crate maintains backward compatibility

---

## Workspace Structure

```
klyntbot/
├── Cargo.toml              ← workspace root
├── crates/
│   ├── common/      ← Layer 0: Foundation types
│   ├── config/    ← Layer 1: Configuration
│   ├── bus/       ← Layer 1: Message bus
│   ├── providers/ ← Layer 2: LLM providers
│   ├── session/   ← Layer 2: Session persistence
│   ├── scheduling/      ← Layer 2: Scheduling
│   ├── calendar/  ← Layer 2: CalDAV client & sync engine
│   ├── tools/     ← Layer 3: Tool system
│   ├── channels/  ← Layer 4: Chat platforms
│   ├── heartbeat/ ← Layer 4: Periodic wake-up
│   ├── agent/     ← Layer 5: Agent orchestration
│   └── cli/       ← Layer 6: Command-line interface
├── src/
│   ├── lib.rs              ← Layer 7: Re-export facade
│   └── main.rs             ← Binary entry point
├── tests/                  ← Integration tests
└── skills/                 ← Bundled skill definitions
```

### Crate Sizes

| Crate | Lines | Files | Description |
|-------|-------|-------|-------------|
| `common` | ~515 | 4 | Error types, shared types, utilities |
| `config` | ~1,646 | 2 | Configuration schema and loader |
| `bus` | ~458 | 2 | Async message bus |
| `providers` | ~1,355 | 4 | LLM provider abstraction |
| `session` | ~479 | 1 | Session persistence |
| `scheduling` | ~1,284 | 2 | Cron scheduling service |
| `calendar` | ~2,921 | 12 | CalDAV client, sync engine, provider adapters (Apple, Google, Generic) |
| `tools` | ~1,892 | 7 | Tool trait + implementations |
| `channels` | ~3,306 | 8 | Channel trait + platform impls |
| `heartbeat` | ~229 | 1 | Heartbeat service |
| `agent` | ~2,286 | 5 | Agent loop and orchestration |
| `cli` | ~1,849 | 9 | CLI commands and REPL |
| **Total** | **~18,220** | **57** | |

---

## Crate Dependency Graph

### Layered Dependency DAG

```
Layer 0 (Foundation):
    common
        │
        ├─────────────────────────────────┐
        │                                 │
Layer 1 (Data):
    config              bus
        │                                 │
        ├─────────────────────────────────┘
        │
        ├─────────────────┬───────────────┬───────────────┐
        │                 │               │               │
Layer 2 (Services):
    providers  session  scheduling  calendar
        │                 │               │               │
        └─────────────────┴───────────────┴───────────────┘
        │
Layer 3 (Capabilities):
    tools
        │
        ├─────────────────────────────────┐
        │                                 │
Layer 4 (Platforms):
    channels          heartbeat
        │                                 │
        └─────────────────┬───────────────┘
                          │
Layer 5 (Orchestration):
    agent
        │
Layer 6 (UI):
    cli
        │
Layer 7 (Binary):
    klyntbot (facade + bin)
```

### Dependency Rules

1. **Foundation layer** (`common`) has no internal dependencies
2. **Data layer** crates depend only on core
3. **Service layer** crates depend on core + data layer
4. **Capability layer** (`tools`) depends on services it needs
5. **Platform layer** (`channels`, `heartbeat`) depends on capabilities
6. **Orchestration layer** (`agent`) depends on most crates
7. **UI layer** (`cli`) depends on all functional crates
8. **Binary layer** re-exports everything for backward compatibility

**No circular dependencies are possible** — Cargo enforces the DAG at compile time.

---

## Core Abstractions

### Error Handling

All errors flow through `common`:

```rust
// In common/src/error.rs
pub enum KlyntbotError {
    Tool(ToolError),
    Provider(ProviderError),
    Channel(ChannelError),
    Session(SessionError),
    Config(ConfigError),
    Cron(CronError),
    Internal(String),
}

pub type Result<T> = std::result::Result<T, KlyntbotError>;
```

Each domain error has automatic `From` conversions:

```rust
// In providers
pub fn call_llm(...) -> Result<LlmResponse> {
    let resp = reqwest::get(url).await?;  // reqwest::Error → ProviderError → KlyntbotError
    Ok(...)
}
```

### Shared Types

Core types in `common`:

```rust
// Message roles
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

// Channel identification
pub enum ChannelName { Telegram, Discord, Slack, ... }
pub struct ChatId(String);
pub struct SessionKey { channel: ChannelName, chat_id: ChatId }
```

### Trait-Based Extension

Six primary extension traits:

| Trait | Crate | Purpose |
|-------|-------|---------|
| `LlmProvider` | `providers` | Add LLM provider implementations |
| `Tool` | `tools` | Add new agent tools |
| `Channel` | `channels` | Add chat platform integrations |
| `SpawnHandler` | `tools` | Abstraction for subagent spawning |
| `CronHandler` | `tools` | Abstraction for cron job management |
| `CalendarHandler` | `tools` | Abstraction for calendar sync (sync, list events, create events, status) |

---

## Extension Points

### Adding a New LLM Provider

**1. Implement the `LlmProvider` trait in `providers`:**

```rust
// In providers/src/my_provider.rs
use common::Result;
use async_trait::async_trait;

pub struct MyProvider {
    api_key: String,
    base_url: String,
}

#[async_trait]
impl LlmProvider for MyProvider {
    async fn complete(&self, messages: &[Message], params: &ChatParams) -> Result<LlmResponse> {
        // Implementation
    }
}
```

**2. Register in the provider registry:**

```rust
// In providers/src/registry.rs
pub fn detect_provider(model: &str, config: &Config) -> Result<DynProvider> {
    if model.contains("my-model") {
        return Ok(Arc::new(MyProvider::new(config)?));
    }
    // ... existing providers
}
```

**3. Add configuration in `config`:**

```rust
// In config/src/schema.rs
pub struct ProvidersConfig {
    pub my_provider: Option<ProviderApiConfig>,
    // ... existing providers
}
```

### Adding a New Tool

**1. Implement the `Tool` trait in `tools`:**

```rust
// In tools/src/my_tool.rs
use common::Result;
use async_trait::async_trait;
use serde_json::Value;

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }

    fn description(&self) -> &str {
        "Does something useful"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "Input data" }
            },
            "required": ["input"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        // Implementation
    }
}
```

**2. Register the tool:**

```rust
// In tools/src/registry.rs
pub fn create_tool_registry(...) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(MyTool));
    // ... existing tools
    registry
}
```

### Adding a New Channel

**1. Implement the `Channel` trait in `channels`:**

```rust
// In channels/src/my_channel.rs
use common::Result;
use bus::{InboundMessage, OutboundMessage};
use async_trait::async_trait;
use tokio::sync::mpsc;

pub struct MyChannel {
    inbound: mpsc::Sender<InboundMessage>,
}

#[async_trait]
impl Channel for MyChannel {
    async fn start(&self, outbound: mpsc::Receiver<OutboundMessage>) -> Result<()> {
        // Connect to platform, send outbound messages, push inbound messages
    }

    fn name(&self) -> ChannelName {
        ChannelName::MyChannel
    }
}
```

**2. Add to channel manager:**

```rust
// In channels/src/manager.rs
pub async fn start_channels(config: &Config, bus: &MessageBus) -> Result<Vec<DynChannel>> {
    let mut channels = vec![];

    if config.channels.my_channel.enabled {
        channels.push(Arc::new(MyChannel::new(config, bus)?));
    }
    // ... existing channels
}
```

**3. Add configuration:**

```rust
// In config/src/schema.rs
pub struct ChannelsConfig {
    pub my_channel: MyChannelConfig,
    // ... existing channels
}
```

---

## Build System

### Workspace Configuration

The root `Cargo.toml` defines shared dependencies:

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.dependencies]
# Internal crates
common = { path = "crates/common" }
config = { path = "crates/config" }
# ... all workspace crates

# External dependencies (version pinned)
tokio = { version = "1.49", features = [...] }
serde = { version = "1.0", features = ["derive"] }
# ... shared dependencies
```

Individual crates inherit versions:

```toml
# Example: crates/tools/Cargo.toml
[package]
name = "tools"
version.workspace = true
edition.workspace = true

[dependencies]
common.workspace = true
bus.workspace = true
tokio.workspace = true
async-trait.workspace = true
```

### Feature Flags

| Feature | Crate | Purpose |
|---------|-------|---------|
| `email` | `channels` | Email channel (IMAP/SMTP) |

Feature propagation from root:

```toml
# Root Cargo.toml
[features]
default = ["email"]
email = ["channels/email"]
```

### Compilation Parallelism

With 12 crates in 8 layers, the compiler can parallelize:

- **Layer 0**: `common` (first, serial)
- **Layer 1**: `config` + `bus` (parallel)
- **Layer 2**: `providers` + `session` + `scheduling` + `calendar` (parallel)
- **Layer 3**: `tools` (serial)
- **Layer 4**: `channels` + `heartbeat` (parallel)
- **Layer 5**: `agent` (serial)
- **Layer 6**: `cli` (serial)
- **Layer 7**: `klyntbot` facade (serial)

**Critical path**: 8 serial steps, but each step is smaller than the original monolith.

### Incremental Builds

| Changed Crate | Recompiles |
|---------------|------------|
| `common` | All (foundation) |
| `config` | 7 crates |
| `bus` | 7 crates |
| `providers` | 5 crates |
| `calendar` | 3 crates (agent, cli, klyntbot) |
| `tools` | 4 crates |
| `channels` | 2 crates (channels, cli) |
| `session` | 3 crates |
| `scheduling` | 3 crates |
| `heartbeat` | 2 crates |
| `agent` | 2 crates (agent, cli) |
| `cli` | 1 crate (cli only) |

**Key win**: Changing `telegram.rs` only recompiles `channels` and `cli`.

---

## Testing Strategy

### Unit Tests

Each crate has inline `#[cfg(test)] mod tests` for crate-internal testing:

```rust
// In common/src/error.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        // Test From<ToolError> for KlyntbotError
    }
}
```

Run with: `cargo test -p common`

### Integration Tests

Integration tests live in `tests/` and depend on the facade crate:

```rust
// In tests/agent_loop_tests.rs
use klyntbot::{AgentLoop, Config, MessageBus};

#[tokio::test]
async fn test_agent_loop_basic_chat() {
    // Cross-crate integration test
}
```

Run with: `cargo test --workspace`

### Test Organization

| Test File | Location | Purpose |
|-----------|----------|---------|
| `agent_loop_tests.rs` | `tests/` | Cross-crate agent integration |
| `channel_tests.rs` | `tests/` | Cross-crate channel integration |
| `integration_tests.rs` | `tests/` | Full-stack integration |
| `mock_provider.rs` | `tests/` | Shared test helper |
| Provider unit tests | `providers` | Provider-internal |
| Agent unit tests | `agent` | Agent-internal |
| Skills unit tests | `agent` | Skills-internal |

---

## Design Patterns

### Dependency Inversion

**Problem**: `SpawnTool` needs to call `SubagentManager`, but `SubagentManager` depends on the entire `AgentLoop` which depends on all tools (circular dependency).

**Solution**: Define a trait in `tools`:

```rust
// In tools/src/spawn.rs
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(&self, task: String, label: Option<String>, ...) -> String;
}

pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}
```

Implement the trait in `agent`:

```rust
// In agent/src/subagent.rs
impl SpawnHandler for SubagentManager {
    async fn spawn(&self, task: String, ...) -> String {
        // Implementation
    }
}
```

Inject at construction:

```rust
// In agent/src/agent_loop.rs
let spawn_tool = SpawnTool::new(Some(Arc::clone(&subagent_mgr) as Arc<dyn SpawnHandler>));
```

**Same pattern for `CronTool`/`CronHandler` and `CalendarTool`/`CalendarHandler`.**

The `CalendarHandler` trait is defined in `tools/src/calendar_tool.rs` and implemented by `CalendarSyncAdapter` in `agent/src/calendar_sync_adapter.rs`. It provides `sync_calendar()`, `list_events()`, `create_event()`, and `get_status()` methods, with the adapter wrapping the `calendar` crate's `SyncEngine` and provider-specific clients.

### Re-export Facade

The root `klyntbot` crate re-exports all public types for backward compatibility:

```rust
// In klyntbot/src/lib.rs
pub use common::{KlyntbotError, Result, ChannelName, ChatId, MessageRole};
pub use config::Config;
pub use bus::{InboundMessage, OutboundMessage, MessageBus};
pub use providers::{LlmProvider, create_provider};
pub use tools::{Tool, ToolRegistry};
pub use channels::{Channel, ChannelManager};
pub use agent::{AgentLoop, ContextBuilder, MemoryStore};
pub use session::SessionManager;
pub use scheduling::CronService;
```

Existing code using `use klyntbot::AgentLoop` works unchanged.

### Handler Traits

For any cross-layer interaction where direct dependencies would create cycles:

1. Define a trait in the **lower layer** (e.g., `tools`)
2. Implement the trait in the **higher layer** (e.g., `agent`)
3. Inject the implementation as `Arc<dyn Trait>` at construction time

This keeps dependencies flowing upward while allowing runtime polymorphism.

---

## Rationale for Key Decisions

### Why 12 crates?

Each crate represents a **logical boundary** with a single responsibility:
- `common` = foundation types
- `config` = configuration I/O
- `bus` = message passing
- `providers` = LLM abstraction
- `session` = conversation persistence
- `scheduling` = cron jobs
- `calendar` = CalDAV client & sync engine
- `tools` = agent capabilities
- `channels` = platform integrations
- `heartbeat` = periodic wake-up
- `agent` = orchestration logic
- `cli` = user interface

Fewer crates would lose modularity; more would add unnecessary complexity.

### Why layer providers, session, and cron at the same level?

They are **independent services** with no dependencies on each other:
- Providers talk to LLM APIs
- Session manages conversation history
- Cron handles scheduled jobs
- Calendar handles CalDAV sync

They all depend on `config` and `common`, making them Layer 2.

### Why put tools at Layer 3?

Tools need access to **services** (providers for transcription, bus for messaging, cron for scheduling). Placing tools above services allows them to depend on what they need without creating cycles.

### Why separate `bus` from `common`?

`bus` depends on `tokio::sync::mpsc` and `chrono`, which are not needed by `core`. Keeping `core` dependency-light makes it fast to compile and easy to use in other contexts.

---

## Workspace Commands

```bash
# Build the entire workspace
cargo build --workspace

# Build a specific crate
cargo build -p agent

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p tools

# Check for errors without building
cargo check --workspace

# Run clippy on all crates
cargo clippy --workspace --all-targets --all-features

# Build the binary in release mode
cargo build --release

# Build without default features
cargo build --no-default-features

# Build with specific features
cargo build --features email
```

---

## Summary

Klyntbot's workspace architecture achieves:
- **Clear separation of concerns** via 12 focused crates
- **No circular dependencies** via dependency inversion patterns
- **Parallel compilation** via layered dependency graph
- **Incremental builds** via crate-level isolation
- **Extensibility** via trait-based abstractions
- **Backward compatibility** via re-export facade

The architecture is designed for **maintainability** (easy to navigate), **performance** (fast compilation), and **extensibility** (add providers/tools/channels by implementing traits).
