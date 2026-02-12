# Workspace Usage Examples

This document provides practical examples of working with klyntbot's multi-crate workspace.

## Table of Contents

1. [Basic Workspace Commands](#basic-workspace-commands)
2. [Building Specific Crates](#building-specific-crates)
3. [Adding a Custom Tool](#adding-a-custom-tool)
4. [Adding a New Provider](#adding-a-new-provider)
5. [Creating a Channel Integration](#creating-a-channel-integration)
6. [Working with Tests](#working-with-tests)
7. [Feature Development Workflow](#feature-development-workflow)

---

## Basic Workspace Commands

### Build Everything

```bash
# Build all crates in the workspace
cargo build --workspace

# Build in release mode
cargo build --workspace --release

# Build with all features
cargo build --workspace --all-features

# Build without default features
cargo build --workspace --no-default-features
```

### Test Everything

```bash
# Run all tests
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test
cargo test test_agent_loop --workspace

# Run tests for a specific crate
cargo test -p klyntbot-tools
```

### Check and Lint

```bash
# Check all crates
cargo check --workspace

# Run clippy on all crates
cargo clippy --workspace --all-targets --all-features

# Format all code
cargo fmt --all
```

---

## Building Specific Crates

### Build a Single Crate

```bash
# Build just the core crate
cargo build -p klyntbot-core

# Build just the agent crate
cargo build -p klyntbot-agent

# Build in release mode
cargo build -p klyntbot-cli --release
```

### Check Dependencies

```bash
# Show dependency tree
cargo tree -p klyntbot-agent

# Show reverse dependencies (what depends on this crate)
cargo tree -p klyntbot-core -i

# Check for outdated dependencies
cargo outdated
```

### Watch for Changes

```bash
# Install cargo-watch
cargo install cargo-watch

# Auto-rebuild on changes
cargo watch -x "build -p klyntbot-agent"

# Auto-test on changes
cargo watch -x "test -p klyntbot-tools"
```

---

## Adding a Custom Tool

### Step 1: Create Tool File

Create `crates/klyntbot-tools/src/calculator.rs`:

```rust
use async_trait::async_trait;
use serde_json::Value;
use klyntbot_core::{Result, ToolError};
use super::{Tool, RoutingContext};

pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Performs basic arithmetic operations"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "First operand"
                },
                "b": {
                    "type": "number",
                    "description": "Second operand"
                }
            },
            "required": ["operation", "a", "b"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let operation = args["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("Missing operation".into()))?;

        let a = args["a"]
            .as_f64()
            .ok_or_else(|| ToolError::InvalidParameters("Invalid number a".into()))?;

        let b = args["b"]
            .as_f64()
            .ok_or_else(|| ToolError::InvalidParameters("Invalid number b".into()))?;

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed("Division by zero".into()).into());
                }
                a / b
            }
            _ => return Err(ToolError::InvalidParameters("Unknown operation".into()).into()),
        };

        Ok(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_calculator_add() {
        let tool = CalculatorTool;
        let result = tool
            .execute(
                serde_json::json!({
                    "operation": "add",
                    "a": 5,
                    "b": 3
                }),
                &RoutingContext::default(),
            )
            .await
            .unwrap();

        assert_eq!(result, "8");
    }

    #[tokio::test]
    async fn test_calculator_divide_by_zero() {
        let tool = CalculatorTool;
        let result = tool
            .execute(
                serde_json::json!({
                    "operation": "divide",
                    "a": 5,
                    "b": 0
                }),
                &RoutingContext::default(),
            )
            .await;

        assert!(result.is_err());
    }
}
```

### Step 2: Export Module

Add to `crates/klyntbot-tools/src/mod.rs`:

```rust
mod calculator;
pub use calculator::CalculatorTool;
```

### Step 3: Register Tool

Update `crates/klyntbot-tools/src/registry.rs`:

```rust
pub fn create_tool_registry(/* params */) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Existing tools
    registry.register(Arc::new(ReadFileTool::new(workspace.clone())));
    registry.register(Arc::new(WriteFileTool::new(workspace.clone())));

    // New calculator tool
    registry.register(Arc::new(CalculatorTool));

    registry
}
```

### Step 4: Test

```bash
# Run tool tests
cargo test -p klyntbot-tools calculator

# Build the workspace
cargo build --workspace

# Test end-to-end
./target/debug/klyntbot chat "Calculate 5 + 3"
```

---

## Adding a New Provider

### Step 1: Create Provider Implementation

Create `crates/klyntbot-providers/src/custom_provider.rs`:

```rust
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use klyntbot_core::{Result, ProviderError};
use klyntbot_config::Config;
use super::{LlmProvider, Message, LlmResponse, ChatParams};

pub struct CustomProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl CustomProvider {
    pub fn new(config: &Config) -> Result<Self> {
        let api_key = config
            .providers
            .custom
            .as_ref()
            .and_then(|c| c.api_key.as_ref())
            .ok_or(ProviderError::AuthFailed)?
            .expose()
            .to_string();

        let base_url = config
            .providers
            .custom
            .as_ref()
            .and_then(|c| c.api_base.as_ref())
            .cloned()
            .unwrap_or_else(|| "https://api.custom.com/v1".into());

        Ok(Self {
            api_key,
            base_url,
            client: Client::new(),
        })
    }
}

#[async_trait]
impl LlmProvider for CustomProvider {
    async fn complete(
        &self,
        messages: &[Message],
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "model": params.model,
                "messages": messages,
                "max_tokens": params.max_tokens,
                "temperature": params.temperature,
            }))
            .send()
            .await?;

        let data: serde_json::Value = response.json().await?;

        Ok(LlmResponse {
            content: data["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            tool_calls: None,
            finish_reason: "stop".into(),
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _params: &ChatParams,
    ) -> Result<impl Stream<Item = Result<String>>> {
        // Streaming implementation
        unimplemented!()
    }
}
```

### Step 2: Add Config Schema

Update `crates/klyntbot-config/src/schema.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersConfig {
    pub anthropic: Option<ProviderApiConfig>,
    pub openai: Option<ProviderApiConfig>,
    // Add new provider
    pub custom: Option<ProviderApiConfig>,
}
```

### Step 3: Register Provider

Update `crates/klyntbot-providers/src/registry.rs`:

```rust
pub fn detect_provider(model: &str, config: &Config) -> Result<DynProvider> {
    // Custom provider detection
    if model.contains("custom-") {
        return Ok(Arc::new(CustomProvider::new(config)?));
    }

    // Existing providers...
}
```

### Step 4: Test

```bash
# Add config
cat >> ~/.klyntbot/config.json <<EOF
{
  "providers": {
    "custom": {
      "apiKey": "your-key",
      "apiBase": "https://api.custom.com/v1"
    }
  },
  "agents": {
    "defaults": {
      "model": "custom-model-name"
    }
  }
}
EOF

# Test
cargo test -p klyntbot-providers
./target/debug/klyntbot chat "Hello!"
```

---

## Creating a Channel Integration

### Step 1: Implement Channel Trait

Create `crates/klyntbot-channels/src/matrix.rs`:

```rust
use async_trait::async_trait;
use tokio::sync::mpsc;
use klyntbot_core::{Result, ChannelName, ChatId};
use klyntbot_bus::{InboundMessage, OutboundMessage};
use klyntbot_config::Config;
use super::Channel;

pub struct MatrixChannel {
    token: String,
    inbound_tx: mpsc::Sender<InboundMessage>,
}

impl MatrixChannel {
    pub fn new(config: &Config, inbound_tx: mpsc::Sender<InboundMessage>) -> Result<Self> {
        let token = config
            .channels
            .matrix
            .as_ref()
            .ok_or(ChannelError::NotConfigured)?
            .token
            .clone();

        Ok(Self { token, inbound_tx })
    }
}

#[async_trait]
impl Channel for MatrixChannel {
    async fn start(&self, mut outbound_rx: mpsc::Receiver<OutboundMessage>) -> Result<()> {
        loop {
            tokio::select! {
                // Receive from Matrix
                incoming = self.receive_from_matrix() => {
                    if let Some(msg) = incoming? {
                        self.inbound_tx.send(msg).await.ok();
                    }
                }

                // Send to Matrix
                Some(outbound) = outbound_rx.recv() => {
                    self.send_to_matrix(outbound).await?;
                }
            }
        }
    }

    fn name(&self) -> ChannelName {
        ChannelName::Matrix
    }
}

impl MatrixChannel {
    async fn receive_from_matrix(&self) -> Result<Option<InboundMessage>> {
        // Matrix client implementation
        todo!()
    }

    async fn send_to_matrix(&self, msg: OutboundMessage) -> Result<()> {
        // Send implementation
        todo!()
    }
}
```

### Step 2: Add to Channel Manager

Update `crates/klyntbot-channels/src/manager.rs`:

```rust
pub async fn start_channels(config: &Config, bus: &MessageBus) -> Result<Vec<DynChannel>> {
    let mut channels: Vec<DynChannel> = vec![];

    // Existing channels...

    // Matrix channel
    if let Some(matrix_config) = &config.channels.matrix {
        if matrix_config.enabled {
            let matrix = MatrixChannel::new(config, bus.inbound_sender())?;
            channels.push(Arc::new(matrix));
        }
    }

    Ok(channels)
}
```

### Step 3: Add Configuration

Update `crates/klyntbot-config/src/schema.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub matrix: Option<MatrixConfig>,  // New
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixConfig {
    pub enabled: bool,
    pub token: String,
    pub homeserver: String,
    pub allow_from: Vec<String>,
}
```

---

## Working with Tests

### Run Tests for Modified Code

```bash
# If you changed klyntbot-tools
cargo test -p klyntbot-tools

# If you changed klyntbot-agent
cargo test -p klyntbot-agent

# If you changed multiple crates
cargo test -p klyntbot-tools -p klyntbot-agent
```

### Add Integration Test

Create `tests/calculator_test.rs`:

```rust
use klyntbot::{AgentLoop, Config, MessageBus};
use klyntbot_core::{ChannelName, ChatId};
use klyntbot_bus::InboundMessage;
use chrono::Utc;

#[tokio::test]
async fn test_calculator_tool_integration() {
    let config = Config::default();
    let (bus, mut inbound_rx, outbound_tx) = MessageBus::new(100);
    let agent = AgentLoop::new(config, bus.clone(), "/tmp/workspace".into())
        .await
        .unwrap();

    // Send calculation request
    bus.inbound_sender()
        .send(InboundMessage {
            channel: ChannelName::Cli,
            chat_id: ChatId("test".into()),
            user_id: Some("test".into()),
            content: "Calculate 15 * 3".into(),
            attachments: vec![],
            timestamp: Utc::now(),
        })
        .await
        .ok();

    // Process and verify response contains "45"
}
```

Run with:
```bash
cargo test --test calculator_test
```

---

## Feature Development Workflow

### Example: Adding Streaming Support

**Goal**: Add streaming responses to a provider.

**Step 1**: Plan which crates are affected
- `klyntbot-providers` — Implement streaming
- `klyntbot-agent` — Handle streaming responses
- `klyntbot-cli` — Display streaming output

**Step 2**: Create feature branch
```bash
git checkout -b feature/streaming-responses
```

**Step 3**: Implement in `klyntbot-providers`
```bash
# Make changes to streaming implementation
vim crates/klyntbot-providers/src/openai_compat.rs

# Test just this crate
cargo test -p klyntbot-providers

# Check for errors
cargo check -p klyntbot-providers
```

**Step 4**: Update `klyntbot-agent` to use streaming
```bash
# Make changes to agent loop
vim crates/klyntbot-agent/src/agent_loop.rs

# Test with providers
cargo test -p klyntbot-agent -p klyntbot-providers
```

**Step 5**: Update CLI to display streaming
```bash
# Update chat handler
vim crates/klyntbot-cli/src/chat.rs

# Test end-to-end
cargo build --workspace
./target/debug/klyntbot chat "Hello!"
```

**Step 6**: Final checks
```bash
# All tests
cargo test --workspace

# Clippy
cargo clippy --workspace --all-targets --all-features

# Format
cargo fmt --all

# Integration test
cargo test --test agent_loop_tests
```

**Step 7**: Commit and PR
```bash
git add .
git commit -m "feat(providers): add streaming support for responses"
git push origin feature/streaming-responses
```

---

## Tips and Tricks

### Fast Iteration

```bash
# Check only (no codegen, faster than build)
cargo check -p klyntbot-agent

# Watch for changes
cargo watch -x "check -p klyntbot-agent"
```

### Dependency Management

```bash
# Update a single dependency
cargo update -p serde

# Update all dependencies
cargo update
```

### Benchmarking

```bash
# Build time comparison
hyperfine "cargo clean && cargo build --workspace"

# Binary size
ls -lh target/release/klyntbot
```

### Debugging

```bash
# Verbose build output
cargo build -vv

# Show why a crate depends on another
cargo tree -p klyntbot-agent -i klyntbot-core

# Expand macros
cargo expand -p klyntbot-core
```

---

## Summary

The workspace structure enables:
- **Focused development** — Work on one crate at a time
- **Fast iteration** — Only rebuild changed crates
- **Parallel builds** — Independent crates compile simultaneously
- **Clear dependencies** — Cargo enforces layered architecture

For more examples, see:
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [MIGRATION.md](./MIGRATION.md)
- [CONTRIBUTING.md](../CONTRIBUTING.md)
