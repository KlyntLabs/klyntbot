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

Klyntbot is structured as a Cargo workspace with 11 crates organized into 8 dependency layers. This architecture enables:

- **Parallel compilation** — Independent crates in the same layer compile simultaneously
- **Incremental builds** — Changes to one crate only recompile dependents
- **Clear boundaries** — Each crate has a well-defined responsibility
- **Dependency inversion** — Higher layers depend on traits defined in lower layers
- **Feature flags** — Optional functionality can be disabled at compile time

### Design Philosophy

1. **Foundation first** — `klyntbot-core` contains only zero-dependency types and errors
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
│   ├── klyntbot-core/      ← Layer 0: Foundation types
│   ├── klyntbot-config/    ← Layer 1: Configuration
│   ├── klyntbot-bus/       ← Layer 1: Message bus
│   ├── klyntbot-providers/ ← Layer 2: LLM providers
│   ├── klyntbot-session/   ← Layer 2: Session persistence
│   ├── klyntbot-cron/      ← Layer 2: Scheduling
│   ├── klyntbot-tools/     ← Layer 3: Tool system
│   ├── klyntbot-channels/  ← Layer 4: Chat platforms
│   ├── klyntbot-heartbeat/ ← Layer 4: Periodic wake-up
│   ├── klyntbot-agent/     ← Layer 5: Agent orchestration
│   └── klyntbot-cli/       ← Layer 6: Command-line interface
├── src/
│   ├── lib.rs              ← Layer 7: Re-export facade
│   └── main.rs             ← Binary entry point
├── tests/                  ← Integration tests
└── skills/                 ← Bundled skill definitions
```

### Crate Sizes

| Crate | Lines | Files | Description |
|-------|-------|-------|-------------|
| `klyntbot-core` | ~515 | 4 | Error types, shared types, utilities |
| `klyntbot-config` | ~1,646 | 2 | Configuration schema and loader |
| `klyntbot-bus` | ~458 | 2 | Async message bus |
| `klyntbot-providers` | ~1,355 | 4 | LLM provider abstraction |
| `klyntbot-session` | ~479 | 1 | Session persistence |
| `klyntbot-cron` | ~1,284 | 2 | Cron scheduling service |
| `klyntbot-tools` | ~1,892 | 7 | Tool trait + implementations |
| `klyntbot-channels` | ~3,306 | 8 | Channel trait + platform impls |
| `klyntbot-heartbeat` | ~229 | 1 | Heartbeat service |
| `klyntbot-agent` | ~2,286 | 5 | Agent loop and orchestration |
| `klyntbot-cli` | ~1,849 | 9 | CLI commands and REPL |
| **Total** | **~15,299** | **45** | |

---

## Crate Dependency Graph

### Layered Dependency DAG

```
Layer 0 (Foundation):
    klyntbot-core
        │
        ├─────────────────────────────────┐
        │                                 │
Layer 1 (Data):
    klyntbot-config              klyntbot-bus
        │                                 │
        ├─────────────────────────────────┘
        │
        ├─────────────────┬───────────────┬───────────────┐
        │                 │               │               │
Layer 2 (Services):
    klyntbot-providers  klyntbot-session  klyntbot-cron  │
        │                 │               │               │
        └─────────────────┴───────────────┴───────────────┘
        │
Layer 3 (Capabilities):
    klyntbot-tools
        │
        ├─────────────────────────────────┐
        │                                 │
Layer 4 (Platforms):
    klyntbot-channels          klyntbot-heartbeat
        │                                 │
        └─────────────────┬───────────────┘
                          │
Layer 5 (Orchestration):
    klyntbot-agent
        │
Layer 6 (UI):
    klyntbot-cli
        │
Layer 7 (Binary):
    klyntbot (facade + bin)
```

### Dependency Rules

1. **Foundation layer** (`klyntbot-core`) has no internal dependencies
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

All errors flow through `klyntbot-core`:

```rust
// In klyntbot-core/src/error.rs
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
// In klyntbot-providers
pub fn call_llm(...) -> Result<LlmResponse> {
    let resp = reqwest::get(url).await?;  // reqwest::Error → ProviderError → KlyntbotError
    Ok(...)
}
```

### Shared Types

Core types in `klyntbot-core`:

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

Four primary extension traits:

| Trait | Crate | Purpose |
|-------|-------|---------|
| `LlmProvider` | `klyntbot-providers` | Add LLM provider implementations |
| `Tool` | `klyntbot-tools` | Add new agent tools |
| `Channel` | `klyntbot-channels` | Add chat platform integrations |
| `SpawnHandler` | `klyntbot-tools` | Abstraction for subagent spawning |
| `CronHandler` | `klyntbot-tools` | Abstraction for cron job management |

---

## Extension Points

### Adding a New LLM Provider

**1. Implement the `LlmProvider` trait in `klyntbot-providers`:**

```rust
// In klyntbot-providers/src/my_provider.rs
use klyntbot_core::Result;
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
// In klyntbot-providers/src/registry.rs
pub fn detect_provider(model: &str, config: &Config) -> Result<DynProvider> {
    if model.contains("my-model") {
        return Ok(Arc::new(MyProvider::new(config)?));
    }
    // ... existing providers
}
```

**3. Add configuration in `klyntbot-config`:**

```rust
// In klyntbot-config/src/schema.rs
pub struct ProvidersConfig {
    pub my_provider: Option<ProviderApiConfig>,
    // ... existing providers
}
```

### Adding a New Tool

**1. Implement the `Tool` trait in `klyntbot-tools`:**

```rust
// In klyntbot-tools/src/my_tool.rs
use klyntbot_core::Result;
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
// In klyntbot-tools/src/registry.rs
pub fn create_tool_registry(...) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(MyTool));
    // ... existing tools
    registry
}
```

### Adding a New Channel

**1. Implement the `Channel` trait in `klyntbot-channels`:**

```rust
// In klyntbot-channels/src/my_channel.rs
use klyntbot_core::Result;
use klyntbot_bus::{InboundMessage, OutboundMessage};
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
// In klyntbot-channels/src/manager.rs
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
// In klyntbot-config/src/schema.rs
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
klyntbot-core = { path = "crates/klyntbot-core" }
klyntbot-config = { path = "crates/klyntbot-config" }
# ... all workspace crates

# External dependencies (version pinned)
tokio = { version = "1.49", features = [...] }
serde = { version = "1.0", features = ["derive"] }
# ... shared dependencies
```

Individual crates inherit versions:

```toml
# Example: crates/klyntbot-tools/Cargo.toml
[package]
name = "klyntbot-tools"
version.workspace = true
edition.workspace = true

[dependencies]
klyntbot-core.workspace = true
klyntbot-bus.workspace = true
tokio.workspace = true
async-trait.workspace = true
```

### Feature Flags

| Feature | Crate | Purpose |
|---------|-------|---------|
| `email` | `klyntbot-channels` | Email channel (IMAP/SMTP) |

Feature propagation from root:

```toml
# Root Cargo.toml
[features]
default = ["email"]
email = ["klyntbot-channels/email"]
```

### Compilation Parallelism

With 11 crates in 8 layers, the compiler can parallelize:

- **Layer 0**: `klyntbot-core` (first, serial)
- **Layer 1**: `klyntbot-config` + `klyntbot-bus` (parallel)
- **Layer 2**: `klyntbot-providers` + `klyntbot-session` + `klyntbot-cron` (parallel)
- **Layer 3**: `klyntbot-tools` (serial)
- **Layer 4**: `klyntbot-channels` + `klyntbot-heartbeat` (parallel)
- **Layer 5**: `klyntbot-agent` (serial)
- **Layer 6**: `klyntbot-cli` (serial)
- **Layer 7**: `klyntbot` facade (serial)

**Critical path**: 8 serial steps, but each step is smaller than the original monolith.

### Incremental Builds

| Changed Crate | Recompiles |
|---------------|------------|
| `klyntbot-core` | All (foundation) |
| `klyntbot-config` | 6 crates |
| `klyntbot-bus` | 6 crates |
| `klyntbot-providers` | 5 crates |
| `klyntbot-tools` | 4 crates |
| `klyntbot-channels` | 2 crates (channels, cli) |
| `klyntbot-session` | 3 crates |
| `klyntbot-cron` | 3 crates |
| `klyntbot-heartbeat` | 2 crates |
| `klyntbot-agent` | 2 crates (agent, cli) |
| `klyntbot-cli` | 1 crate (cli only) |

**Key win**: Changing `telegram.rs` only recompiles `klyntbot-channels` and `klyntbot-cli`.

---

## Testing Strategy

### Unit Tests

Each crate has inline `#[cfg(test)] mod tests` for crate-internal testing:

```rust
// In klyntbot-core/src/error.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        // Test From<ToolError> for KlyntbotError
    }
}
```

Run with: `cargo test -p klyntbot-core`

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
| Provider unit tests | `klyntbot-providers` | Provider-internal |
| Agent unit tests | `klyntbot-agent` | Agent-internal |
| Skills unit tests | `klyntbot-agent` | Skills-internal |

---

## Design Patterns

### Dependency Inversion

**Problem**: `SpawnTool` needs to call `SubagentManager`, but `SubagentManager` depends on the entire `AgentLoop` which depends on all tools (circular dependency).

**Solution**: Define a trait in `klyntbot-tools`:

```rust
// In klyntbot-tools/src/spawn.rs
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(&self, task: String, label: Option<String>, ...) -> String;
}

pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}
```

Implement the trait in `klyntbot-agent`:

```rust
// In klyntbot-agent/src/subagent.rs
impl SpawnHandler for SubagentManager {
    async fn spawn(&self, task: String, ...) -> String {
        // Implementation
    }
}
```

Inject at construction:

```rust
// In klyntbot-agent/src/agent_loop.rs
let spawn_tool = SpawnTool::new(Some(Arc::clone(&subagent_mgr) as Arc<dyn SpawnHandler>));
```

**Same pattern for `CronTool` and `CronHandler`.**

### Re-export Facade

The root `klyntbot` crate re-exports all public types for backward compatibility:

```rust
// In klyntbot/src/lib.rs
pub use klyntbot_core::{KlyntbotError, Result, ChannelName, ChatId, MessageRole};
pub use klyntbot_config::Config;
pub use klyntbot_bus::{InboundMessage, OutboundMessage, MessageBus};
pub use klyntbot_providers::{LlmProvider, create_provider};
pub use klyntbot_tools::{Tool, ToolRegistry};
pub use klyntbot_channels::{Channel, ChannelManager};
pub use klyntbot_agent::{AgentLoop, ContextBuilder, MemoryStore};
pub use klyntbot_session::SessionManager;
pub use klyntbot_cron::CronService;
```

Existing code using `use klyntbot::AgentLoop` works unchanged.

### Handler Traits

For any cross-layer interaction where direct dependencies would create cycles:

1. Define a trait in the **lower layer** (e.g., `klyntbot-tools`)
2. Implement the trait in the **higher layer** (e.g., `klyntbot-agent`)
3. Inject the implementation as `Arc<dyn Trait>` at construction time

This keeps dependencies flowing upward while allowing runtime polymorphism.

---

## Rationale for Key Decisions

### Why 11 crates?

Each crate represents a **logical boundary** with a single responsibility:
- `core` = foundation types
- `config` = configuration I/O
- `bus` = message passing
- `providers` = LLM abstraction
- `tools` = agent capabilities
- `channels` = platform integrations
- `agent` = orchestration logic
- `cli` = user interface

Fewer crates would lose modularity; more would add unnecessary complexity.

### Why layer providers, session, and cron at the same level?

They are **independent services** with no dependencies on each other:
- Providers talk to LLM APIs
- Session manages conversation history
- Cron handles scheduled jobs

They all depend on `config` and `core`, making them Layer 2.

### Why put tools at Layer 3?

Tools need access to **services** (providers for transcription, bus for messaging, cron for scheduling). Placing tools above services allows them to depend on what they need without creating cycles.

### Why separate `klyntbot-bus` from `klyntbot-core`?

`bus` depends on `tokio::sync::mpsc` and `chrono`, which are not needed by `core`. Keeping `core` dependency-light makes it fast to compile and easy to use in other contexts.

---

## Workspace Commands

```bash
# Build the entire workspace
cargo build --workspace

# Build a specific crate
cargo build -p klyntbot-agent

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p klyntbot-tools

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
- **Clear separation of concerns** via 11 focused crates
- **No circular dependencies** via dependency inversion patterns
- **Parallel compilation** via layered dependency graph
- **Incremental builds** via crate-level isolation
- **Extensibility** via trait-based abstractions
- **Backward compatibility** via re-export facade

The architecture is designed for **maintainability** (easy to navigate), **performance** (fast compilation), and **extensibility** (add providers/tools/channels by implementing traits).
