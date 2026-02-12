# Klyntbot Workspace Architecture

## 1. Current State Analysis

### 1.1 Module Structure (17,384 lines across 55 .rs files)

| Module | Files | Lines | Description |
|--------|-------|-------|-------------|
| config | 3 | 1,646 | Schema (Secret, Config structs) + loader (JSON I/O) |
| channels | 8 | 3,306 | 6 platform impls + Channel trait + ChannelManager |
| agent | 5 | 2,286 | AgentLoop, ContextBuilder, MemoryStore, SkillManager, SubagentManager |
| tools | 7 | 1,892 | Tool trait + 5 tools + ToolRegistry |
| providers | 4 | 1,355 | LlmProvider trait, OpenAiCompat, Registry, Transcription |
| bus | 2 | 458 | InboundMessage, OutboundMessage, MessageBus |
| session | 1 | 479 | Session, SessionManager (JSONL persistence) |
| cron | 2 | 1,284 | CronService, CronJob, CronSchedule, CronStore |
| heartbeat | 1 | 229 | HeartbeatService |
| cli | 8 | 1,849 | Commands, Chat REPL, Serve, Status, Wizard, etc. |
| utils | 2 | 1,309 | helpers + terminal formatting |
| error | 1 | 270 | KlyntbotError enum + domain-specific error types |
| types | 1 | 245 | ChannelName, ChatId, SessionKey, MessageRole |

### 1.2 Dependency Graph (Directed Edges: A → B means A depends on B)

```
                    ┌──────────────┐
                    │   main.rs    │
                    │  (binary)    │
                    └──────┬───────┘
                           │
                    ┌──────┴───────┐
                    │     cli      │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
       ┌──────┴─────┐ ┌───┴────┐ ┌─────┴──────┐
       │   agent     │ │channels│ │  cron       │
       │             │ │manager │ │  heartbeat  │
       └──┬──┬───┬───┘ └──┬──┬─┘ └──┬──────────┘
          │  │   │        │  │      │
    ┌─────┘  │   └────┐   │  │      │
    │        │        │   │  │      │
┌───┴──┐ ┌──┴────┐ ┌─┴───┴──┴──┐   │
│tools │ │session│ │    bus     │───┘
└──┬───┘ └──┬────┘ └─────┬─────┘
   │        │            │
┌──┴────────┴────────────┴──────────┐
│         providers                 │
└──────────────┬────────────────────┘
               │
┌──────────────┴────────────────────┐
│           config                  │
└──────────────┬────────────────────┘
               │
┌──────────────┴────────────────────┐
│     error  +  types  +  utils     │  ← foundational layer
└───────────────────────────────────┘
```

### 1.3 Critical Dependency Observations

1. **`error`** and **`types`** are leaf dependencies — used by every module.
2. **`config`** depends only on `serde`, `dirs`, `std` — no internal deps.
3. **`bus/events`** depends on `types` (ChannelName, ChatId, SessionKey).
4. **`bus/queue`** depends on `bus/events` + `error`.
5. **`providers/types`** depends on `error` + `types` (MessageRole) + `async_trait` + `futures_util`.
6. **`providers/mod`** depends on `config` (to read API keys).
7. **`tools/mod`** defines the `Tool` trait — depends only on `error` + `types`.
8. **Tool implementations** each depend on `Tool` trait + `error`, some on `bus` (MessageTool), `agent` (SpawnTool), `cron` (CronTool).
9. **`channels/*`** depend on `bus`, `config::schema`, `error`, `Channel` trait.
10. **`channels/manager`** depends on all channel implementations + `bus` + `config`.
11. **`agent/agent_loop`** is the "hub" — depends on `bus`, `config`, `error`, `providers`, `session`, `tools/*`, `cron`.
12. **`cli/serve`** wires everything together: `AgentLoop`, `ChannelManager`, `CronService`, `HeartbeatService`, `MessageBus`.
13. **Circular concern**: `tools/spawn` → `agent/subagent` AND `agent/agent_loop` → `tools/*`. This is managed because SpawnTool takes an `Arc<SubagentManager>` at construction, not the full AgentLoop.

### 1.4 External Dependencies by Domain

| Crate | Used By |
|-------|---------|
| `tokio` | everywhere (async runtime) |
| `serde`, `serde_json` | everywhere (serialization) |
| `async-trait` | providers, tools, channels |
| `reqwest` | providers, tools/web, channels/telegram |
| `tokio-tungstenite` | channels/discord, channels/whatsapp, channels/qq, channels/slack |
| `tracing` | everywhere (logging) |
| `thiserror` | error |
| `anyhow` | CLI handlers |
| `chrono` | bus/events, session, cron, agent/memory |
| `uuid` | agent/subagent, cron |
| `cron` (crate) | cron/service |
| `rustyline` | cli/chat |
| `clap` | cli/commands |
| `serde_yaml` | agent/skills |
| `scraper`, `html2text` | tools/web |
| `url` | tools/web |
| `regex` | tools/shell, channels/telegram |
| `base64` | agent/context |
| `mime_guess` | agent/context |
| `shellexpand` | tools/filesystem |
| `which` | agent/skills |
| `dirs` | config, agent/agent_loop |
| `futures-util` | providers/types, agent/agent_loop |
| Email deps (optional) | channels/email |

---

## 2. Proposed Workspace Architecture

### 2.1 Crate Layout

```
klyntbot/
├── Cargo.toml              ← workspace root
├── crates/
│   ├── klyntbot-core/      ← errors, types, shared traits
│   ├── klyntbot-config/    ← configuration schema + loader
│   ├── klyntbot-bus/       ← message bus (events + queue)
│   ├── klyntbot-providers/ ← LLM provider trait + OpenAI-compat impl
│   ├── klyntbot-tools/     ← Tool trait + all tool implementations
│   ├── klyntbot-session/   ← session management
│   ├── klyntbot-channels/  ← Channel trait + all platform impls
│   ├── klyntbot-agent/     ← AgentLoop, context, memory, skills, subagent
│   ├── klyntbot-cron/      ← cron service + types
│   ├── klyntbot-heartbeat/ ← heartbeat service
│   └── klyntbot-cli/       ← CLI commands + chat REPL
├── src/
│   ├── main.rs             ← binary entry point (thin)
│   └── lib.rs              ← re-export facade crate
├── tests/                  ← integration tests (use facade crate)
└── skills/                 ← bundled skill .md files
```

### 2.2 Crate Dependency DAG (No Cycles)

```
Layer 0 (Foundation):   klyntbot-core
                            │
Layer 1 (Data):         klyntbot-config    klyntbot-bus
                         │                    │
                         └───────┬────────────┘
                                 │
Layer 2 (Services):     klyntbot-providers   klyntbot-session   klyntbot-cron
                         │                    │                  │
                         └────────┬───────────┴──────────────────┘
                                  │
Layer 3 (Capabilities): klyntbot-tools
                                  │
Layer 4 (Platforms):    klyntbot-channels   klyntbot-heartbeat
                         │                    │
                         └────────┬───────────┘
                                  │
Layer 5 (Orchestration):klyntbot-agent
                                  │
Layer 6 (UI):           klyntbot-cli
                                  │
Layer 7 (Binary):       klyntbot (facade + bin)
```

### 2.3 Detailed Crate Specifications

#### `klyntbot-core` (Layer 0)
**Purpose**: Foundation types shared by all crates.

**Contents**:
- `error.rs` — `KlyntbotError`, `ToolError`, `ProviderError`, `ChannelError`, `SessionError`, `ConfigError`, `CronError`, `Result<T>`
- `types.rs` — `ChannelName`, `ChatId`, `SessionKey`, `MessageRole`
- `utils/helpers.rs` — pure utility functions (no IO)
- `utils/terminal.rs` — terminal formatting helpers

**Dependencies**: `thiserror`, `serde`, `serde_json`, `reqwest` (for `ProviderError::Http`)

**Design Decision**: `reqwest::Error` is used in `ProviderError::Http`. Rather than adding reqwest to core, we will change `ProviderError::Http` to wrap a `String` instead. The `From<reqwest::Error>` conversion moves to `klyntbot-providers`. This keeps core dependency-light.

```rust
// In klyntbot-core:
pub enum ProviderError {
    Http(String),           // Changed from reqwest::Error
    InvalidResponse(String),
    RateLimited,
    AuthFailed,
}

// In klyntbot-providers:
impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        ProviderError::Http(e.to_string())
    }
}
```

**Estimated size**: ~515 lines

---

#### `klyntbot-config` (Layer 1)
**Purpose**: Configuration schema and file I/O.

**Contents**:
- `schema.rs` — `Config`, `Secret<T>`, all channel/provider/tool config structs
- `loader.rs` — `load()`, `save()`, `init()`, `config_dir()`, `config_path()`, `load_with_env_overrides()`

**Dependencies**: `klyntbot-core`, `serde`, `serde_json`, `dirs`, `shellexpand`

**Estimated size**: ~1,646 lines

---

#### `klyntbot-bus` (Layer 1)
**Purpose**: Async message bus for channel↔agent communication.

**Contents**:
- `events.rs` — `InboundMessage`, `OutboundMessage`
- `queue.rs` — `MessageBus`

**Dependencies**: `klyntbot-core`, `tokio`, `serde`, `serde_json`, `chrono`

**Estimated size**: ~458 lines

---

#### `klyntbot-providers` (Layer 2)
**Purpose**: LLM provider abstraction and implementations.

**Contents**:
- `types.rs` — `LlmProvider` trait, `Message`, `LlmResponse`, `ToolCall`, `ChatParams`, `DynProvider`, streaming types
- `openai_compat.rs` — `OpenAiCompatProvider` implementation
- `registry.rs` — `ProviderRegistry`, `ProviderSpec`, model-to-provider mapping
- `transcription.rs` — `TranscriptionProvider`
- `mod.rs` — `create_provider()` function

**Dependencies**: `klyntbot-core`, `klyntbot-config`, `async-trait`, `reqwest`, `serde`, `serde_json`, `futures-util`, `tokio`, `tracing`, `base64`

**Design Note**: `create_provider()` needs `Config` to resolve API keys. This is the only cross-cutting dependency from providers to config.

**Estimated size**: ~1,355 lines

---

#### `klyntbot-session` (Layer 2)
**Purpose**: Conversation session persistence.

**Contents**:
- `manager.rs` — `Session`, `SessionMessage`, `SessionInfo`, `SessionManager`

**Dependencies**: `klyntbot-core`, `serde`, `serde_json`, `chrono`, `tokio`, `tracing`

**Estimated size**: ~479 lines

---

#### `klyntbot-cron` (Layer 2)
**Purpose**: Cron job scheduling.

**Contents**:
- `types.rs` — `CronJob`, `CronJobState`, `CronPayload`, `CronSchedule`, `CronStore`
- `service.rs` — `CronService`

**Dependencies**: `klyntbot-core`, `tokio`, `chrono`, `serde`, `serde_json`, `uuid`, `cron`, `tracing`

**Estimated size**: ~1,284 lines

---

#### `klyntbot-tools` (Layer 3)
**Purpose**: Tool trait definition and all tool implementations.

**Contents**:
- `mod.rs` — `Tool` trait, `DynTool`, `RoutingContext`, `validate_value()`
- `registry.rs` — `ToolRegistry`
- `filesystem.rs` — `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `ListDirTool`, `register_fs_tools()`
- `shell.rs` — `ExecTool`
- `web.rs` — `WebSearchTool`, `WebFetchTool`
- `message.rs` — `MessageTool` (depends on bus OutboundMessage sender)
- `spawn.rs` — `SpawnTool` (takes opaque `Arc<dyn SpawnHandler>` trait)
- `cron_tool.rs` — `CronTool` (takes opaque `Arc<dyn CronHandler>` trait)

**Dependencies**: `klyntbot-core`, `klyntbot-bus` (for MessageTool's outbound sender type), `async-trait`, `serde_json`, `tokio`, `reqwest`, `tracing`, `regex`, `shellexpand`, `scraper`, `html2text`, `url`, `urlencoding`

**Design Decision — Breaking the Circular Dependency**:

The current code has:
- `tools/spawn.rs` → `agent::SubagentManager` (tool depends on agent)
- `tools/cron_tool.rs` → `cron::CronService` (tool depends on cron)
- `agent/agent_loop.rs` → `tools/*` (agent depends on tools)

**Solution**: Introduce handler traits in `klyntbot-tools`:

```rust
// In klyntbot-tools/src/spawn.rs
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;
}

pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}
```

```rust
// In klyntbot-tools/src/cron_tool.rs
#[async_trait]
pub trait CronHandler: Send + Sync {
    async fn create_job(&self, ...) -> Result<String>;
    async fn list_jobs(&self) -> Result<String>;
    async fn delete_job(&self, id: &str) -> Result<String>;
}

pub struct CronTool {
    handler: Option<Arc<dyn CronHandler>>,
}
```

Then `klyntbot-agent` implements `SpawnHandler` for `SubagentManager`, and `klyntbot-cron` implements `CronHandler` for `CronService`. Dependency inversion achieved.

**Estimated size**: ~1,892 lines

---

#### `klyntbot-channels` (Layer 4)
**Purpose**: Channel trait and platform implementations.

**Contents**:
- `mod.rs` — `Channel` trait, `DynChannel`, `check_allowlist()`, `reconnect_loop()`
- `telegram.rs`, `discord.rs`, `slack.rs`, `whatsapp.rs`, `qq.rs`, `email.rs`
- `manager.rs` — `ChannelManager`

**Dependencies**: `klyntbot-core`, `klyntbot-bus`, `klyntbot-config`, `klyntbot-providers` (for TranscriptionProvider in telegram), `async-trait`, `reqwest`, `tokio`, `tokio-tungstenite`, `serde`, `serde_json`, `regex`, `tracing`

**Feature Flags**:
```toml
[features]
default = ["email"]
email = ["dep:async-imap", "dep:lettre", "dep:mail-parser", "dep:native-tls", "dep:tokio-native-tls"]
```

**Estimated size**: ~3,306 lines

---

#### `klyntbot-heartbeat` (Layer 4)
**Purpose**: Periodic agent wake-up service.

**Contents**:
- `service.rs` — `HeartbeatService`

**Dependencies**: `klyntbot-core`, `tokio`, `tracing`

**Estimated size**: ~229 lines

---

#### `klyntbot-agent` (Layer 5)
**Purpose**: Core agent orchestration.

**Contents**:
- `agent_loop.rs` — `AgentLoop`
- `context.rs` — `ContextBuilder`
- `memory.rs` — `MemoryStore`
- `skills.rs` — `SkillManager`, `Skill`
- `subagent.rs` — `SubagentManager` (implements `SpawnHandler` from klyntbot-tools)

**Dependencies**: `klyntbot-core`, `klyntbot-bus`, `klyntbot-config`, `klyntbot-providers`, `klyntbot-session`, `klyntbot-tools`, `klyntbot-cron`, `tokio`, `tracing`, `uuid`, `chrono`, `base64`, `mime_guess`, `serde_yaml`, `which`, `futures-util`, `dirs`

**Skills Files**: Bundled via `include_str!()` from `../../skills/*/SKILL.md` (workspace-relative path).

**Estimated size**: ~2,286 lines

---

#### `klyntbot-cli` (Layer 6)
**Purpose**: CLI commands and REPL.

**Contents**:
- `commands.rs` — `Cli`, `Commands` (clap derive)
- `chat.rs` — `handle_chat()` interactive REPL
- `serve.rs` — `handle_serve()` daemon mode
- `status.rs` — `handle_status()`
- `channels.rs` — `handle_channels()`
- `cron.rs` — `handle_cron()`
- `config_cmd.rs` — `handle_config()`
- `skills.rs` — `handle_skills()`
- `wizard.rs` — `run_wizard()`

**Dependencies**: `klyntbot-core`, `klyntbot-config`, `klyntbot-bus`, `klyntbot-providers`, `klyntbot-agent`, `klyntbot-channels`, `klyntbot-cron`, `klyntbot-heartbeat`, `klyntbot-session`, `klyntbot-tools`, `clap`, `rustyline`, `tokio`, `anyhow`, `tracing`, `tracing-subscriber`

**Estimated size**: ~1,849 lines

---

#### `klyntbot` (Root — Facade + Binary, Layer 7)
**Purpose**: Thin re-export facade and binary entry point.

**Contents**:
- `src/lib.rs` — re-exports from all workspace crates
- `src/main.rs` — binary entry point (delegates to `klyntbot-cli`)

**Dependencies**: All workspace crates (re-exports)

**Estimated size**: ~130 lines

---

## 3. Feature Flag Strategy

### 3.1 Workspace-Level Features

```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]

[features]
default = ["email"]
email = ["klyntbot-channels/email"]
```

### 3.2 Per-Crate Features

| Crate | Feature | Purpose |
|-------|---------|---------|
| `klyntbot-channels` | `email` | Email channel (IMAP/SMTP deps) |
| `klyntbot-tools` | `web` | Web search/fetch tools (default on) |
| `klyntbot-providers` | `streaming` | Streaming support (default on) |

### 3.3 Feature Propagation

Features flow downward through dependencies:
- Root `email` → `klyntbot-channels/email`
- No other cross-crate feature propagation needed currently

---

## 4. Shared Abstraction Strategy

### 4.1 Error Hierarchy (in `klyntbot-core`)

The `KlyntbotError` enum remains the top-level error, with `From` impls for each domain error. Each crate uses `klyntbot_core::Result<T>` as the standard return type.

**Important**: Individual crates can define their own `From` impls to convert external errors (like `reqwest::Error`) into the domain errors defined in core.

### 4.2 Trait Locations

| Trait | Crate | Reason |
|-------|-------|--------|
| `LlmProvider` | `klyntbot-providers` | Provider-specific, with streaming types |
| `Tool` | `klyntbot-tools` | Tool-specific, with parameter validation |
| `Channel` | `klyntbot-channels` | Channel-specific, with bus interaction |
| `SpawnHandler` | `klyntbot-tools` | Abstraction for dependency inversion |
| `CronHandler` | `klyntbot-tools` | Abstraction for dependency inversion |

### 4.3 Type Re-exports

The root `klyntbot` crate re-exports commonly used types to maintain backward compatibility:

```rust
// klyntbot/src/lib.rs
pub use klyntbot_core::{KlyntbotError, Result, ChannelName, ChatId, SessionKey, MessageRole};
pub use klyntbot_config::Config;
pub use klyntbot_bus::{InboundMessage, OutboundMessage, MessageBus};
pub use klyntbot_providers::{LlmProvider, DynProvider, Message, LlmResponse, create_provider};
pub use klyntbot_tools::{Tool, DynTool};
pub use klyntbot_channels::{Channel, DynChannel, ChannelManager};
pub use klyntbot_agent::{AgentLoop, ContextBuilder, MemoryStore, SkillManager, SubagentManager};
pub use klyntbot_session::{Session, SessionManager};
pub use klyntbot_cron::{CronJob, CronService};
pub use klyntbot_heartbeat::HeartbeatService;
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

Each crate retains its inline `#[cfg(test)] mod tests` blocks. These test the crate in isolation.

### 5.2 Integration Tests

Integration tests live in the workspace root `tests/` directory and depend on the facade `klyntbot` crate. The existing test files map as:

| Current Test File | Stays In | Reason |
|-------------------|----------|--------|
| `tests/agent_loop_tests.rs` | root `tests/` | Cross-crate integration |
| `tests/channel_tests.rs` | root `tests/` | Cross-crate integration |
| `tests/integration_tests.rs` | root `tests/` | Full-stack integration |
| `tests/memory_and_context_tests.rs` | `klyntbot-agent` unit tests | Agent-internal |
| `tests/mock_provider.rs` | root `tests/` (shared helper) | Used by multiple test files |
| `tests/provider_tests.rs` | `klyntbot-providers` unit tests | Provider-internal |
| `tests/skills_tests.rs` | `klyntbot-agent` unit tests | Agent-internal |

### 5.3 Test Utilities

Create a `klyntbot-test-utils` dev-dependency crate (or module) containing:
- `MockProvider` (currently in `tests/mock_provider.rs`)
- Test fixtures and helpers

---

## 6. Build Optimization Strategy

### 6.1 Parallel Compilation

With 11 crates, the Rust compiler can compile independent crates in parallel:
- **Layer 0**: `klyntbot-core` (must compile first)
- **Layer 1**: `klyntbot-config` + `klyntbot-bus` (parallel)
- **Layer 2**: `klyntbot-providers` + `klyntbot-session` + `klyntbot-cron` (parallel)
- **Layer 3**: `klyntbot-tools`
- **Layer 4**: `klyntbot-channels` + `klyntbot-heartbeat` (parallel)
- **Layer 5**: `klyntbot-agent`
- **Layer 6**: `klyntbot-cli`
- **Layer 7**: `klyntbot` (facade)

Critical path: core → config → providers → tools → agent → cli → binary (7 serial steps, but each step is smaller).

### 6.2 Incremental Compilation Benefits

Changing a file in one crate only recompiles that crate and its dependents:

| Changed Crate | Recompiles |
|---------------|------------|
| `klyntbot-core` | Everything (foundation) |
| `klyntbot-config` | config, providers, channels, agent, cli, binary |
| `klyntbot-bus` | bus, tools, channels, agent, cli, binary |
| `klyntbot-providers` | providers, channels, agent, cli, binary |
| `klyntbot-tools` | tools, agent, cli, binary |
| `klyntbot-channels` | channels, cli, binary |
| `klyntbot-session` | session, agent, cli, binary |
| `klyntbot-cron` | cron, agent, cli, binary |
| `klyntbot-heartbeat` | heartbeat, cli, binary |
| `klyntbot-agent` | agent, cli, binary |
| `klyntbot-cli` | cli, binary |

**Key win**: Changing a channel implementation (e.g., telegram.rs) only recompiles `klyntbot-channels` and above — not providers, tools, session, cron, etc.

### 6.3 Workspace-Level Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/klyntbot-core",
    "crates/klyntbot-config",
    "crates/klyntbot-bus",
    "crates/klyntbot-providers",
    "crates/klyntbot-session",
    "crates/klyntbot-cron",
    "crates/klyntbot-tools",
    "crates/klyntbot-channels",
    "crates/klyntbot-heartbeat",
    "crates/klyntbot-agent",
    "crates/klyntbot-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"

[workspace.dependencies]
# Internal crates
klyntbot-core = { path = "crates/klyntbot-core" }
klyntbot-config = { path = "crates/klyntbot-config" }
klyntbot-bus = { path = "crates/klyntbot-bus" }
klyntbot-providers = { path = "crates/klyntbot-providers" }
klyntbot-session = { path = "crates/klyntbot-session" }
klyntbot-cron = { path = "crates/klyntbot-cron" }
klyntbot-tools = { path = "crates/klyntbot-tools" }
klyntbot-channels = { path = "crates/klyntbot-channels" }
klyntbot-heartbeat = { path = "crates/klyntbot-heartbeat" }
klyntbot-agent = { path = "crates/klyntbot-agent" }
klyntbot-cli = { path = "crates/klyntbot-cli" }

# Shared external dependencies (version pinned at workspace level)
tokio = { version = "1.49.0", features = ["rt-multi-thread", "macros", "time", "sync", "io-util", "net", "signal", "fs", "process"] }
async-trait = "0.1.89"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
reqwest = { version = "0.13.2", features = ["json", "rustls", "multipart", "stream"] }
tokio-tungstenite = { version = "0.28.0", features = ["rustls-tls-native-roots"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.22", features = ["env-filter"] }
thiserror = "2.0.18"
anyhow = "1.0.101"
chrono = { version = "0.4.43", features = ["serde"] }
uuid = { version = "1.20.0", features = ["v4", "serde"] }
dirs = "6.0.0"
cron = "0.15.0"
clap = { version = "4.5.57", features = ["derive"] }
rustyline = "17.0.2"
base64 = "0.22.1"
regex = "1.11.2"
shellexpand = "3.1.0"
serde_yaml = "0.9.34"
scraper = "0.22.0"
html2text = "0.13.1"
url = "2.5.4"
which = "7.0.0"
mime_guess = "2.0.5"
urlencoding = "2.1"
futures-util = "0.3.31"
tempfile = "3.14"

# Optional email deps
async-imap = { version = "0.11.2", default-features = false, features = ["runtime-tokio"] }
lettre = { version = "0.11.19", features = ["tokio1-native-tls"] }
mail-parser = "0.11.1"
native-tls = "0.2.13"
tokio-native-tls = "0.3"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

Individual crate Cargo.toml files use `dep.workspace = true` to inherit versions:

```toml
# Example: crates/klyntbot-core/Cargo.toml
[package]
name = "klyntbot-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
thiserror.workspace = true
serde = { workspace = true }
serde_json = { workspace = true }
```

---

## 7. Migration Strategy

### 7.1 Order of Operations

The refactoring should proceed bottom-up through the dependency layers:

1. **Create workspace structure** — set up `Cargo.toml` workspace, create `crates/` directories
2. **Extract `klyntbot-core`** — move error.rs, types.rs, utils/ (no other crate deps)
3. **Extract `klyntbot-config`** — move config/ (depends only on core)
4. **Extract `klyntbot-bus`** — move bus/ (depends only on core)
5. **Extract `klyntbot-providers`** — move providers/ (depends on core + config)
6. **Extract `klyntbot-session`** — move session/ (depends only on core)
7. **Extract `klyntbot-cron`** — move cron/ (depends only on core)
8. **Extract `klyntbot-tools`** — move tools/ + add handler traits (depends on core + bus)
9. **Extract `klyntbot-channels`** — move channels/ (depends on core + bus + config + providers)
10. **Extract `klyntbot-heartbeat`** — move heartbeat/ (depends only on core)
11. **Extract `klyntbot-agent`** — move agent/ + implement handler traits (depends on most crates)
12. **Extract `klyntbot-cli`** — move cli/ (depends on everything)
13. **Update root crate** — thin facade + binary
14. **Migrate tests** — update imports, move per-crate tests
15. **Verify** — `cargo test --workspace`, `cargo clippy --workspace`

### 7.2 Key Refactoring Points

1. **`ProviderError::Http`**: Change from `#[from] reqwest::Error` to `Http(String)`. Add `From<reqwest::Error>` in `klyntbot-providers`.

2. **SpawnTool/CronTool dependency inversion**: Introduce `SpawnHandler` and `CronHandler` traits in `klyntbot-tools`. Implement them in `klyntbot-agent` and `klyntbot-cron` respectively.

3. **`crate::` → crate-specific imports**: All `use crate::bus::*` becomes `use klyntbot_bus::*`, etc.

4. **`include_str!` paths for skills**: Adjust relative paths in `klyntbot-agent` to point to `../../skills/*/SKILL.md`.

5. **Config re-export in providers**: `create_provider()` takes `&Config` — this is fine as `klyntbot-providers` depends on `klyntbot-config`.

### 7.3 Backward Compatibility

The root `klyntbot` crate re-exports everything, so existing code like `use klyntbot::AgentLoop` continues to work. Tests in `tests/` that use `use klyntbot::*` need no changes.

---

## 8. Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Circular dependency discovered during extraction | High | Handler traits (SpawnHandler, CronHandler) break all known cycles |
| `include_str!` paths break for skills | Medium | Use `CARGO_MANIFEST_DIR` or workspace-relative paths |
| Test failures from import changes | Medium | Run `cargo test` after each crate extraction |
| Build time regression from workspace overhead | Low | Workspace resolver = "2" is well-optimized |
| Feature flag propagation issues | Medium | Test with `--all-features` and `--no-default-features` |

---

## 9. Success Criteria

- [ ] All 11 crates compile independently
- [ ] `cargo test --workspace` passes all 242 tests
- [ ] `cargo clippy --workspace` reports zero warnings
- [ ] No circular dependencies (enforced by Cargo)
- [ ] Feature flags work correctly (`--features email` / `--no-default-features`)
- [ ] Binary output is identical in functionality
- [ ] Root `klyntbot` facade maintains backward-compatible re-exports
