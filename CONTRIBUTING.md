# Contributing to Klyntbot

Thank you for your interest in contributing to klyntbot! This guide covers the workspace structure, development workflow, and contribution guidelines.

## Table of Contents

1. [Workspace Overview](#workspace-overview)
2. [Development Setup](#development-setup)
3. [Making Changes](#making-changes)
4. [Testing](#testing)
5. [Code Style](#code-style)
6. [Pull Request Process](#pull-request-process)
7. [Adding Features](#adding-features)

---

## Workspace Overview

Klyntbot uses a Cargo workspace with 11 crates organized in dependency layers:

```
Layer 0: klyntbot-core          (foundation types)
Layer 1: klyntbot-config, klyntbot-bus
Layer 2: klyntbot-providers, klyntbot-session, klyntbot-cron
Layer 3: klyntbot-tools
Layer 4: klyntbot-channels, klyntbot-heartbeat
Layer 5: klyntbot-agent
Layer 6: klyntbot-cli
Layer 7: klyntbot (facade + binary)
```

**Key principle**: Dependencies flow upward only (no cycles).

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed architecture documentation.

---

## Development Setup

### Prerequisites

- Rust 1.75+ (`rustup update`)
- Cargo
- Git

### Clone and Build

```bash
git clone https://github.com/KlyntLabs/klyntbot.git
cd klyntbot

# Build all workspace crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-targets --all-features
```

### Build Individual Crates

```bash
# Build specific crate
cargo build -p klyntbot-agent

# Test specific crate
cargo test -p klyntbot-tools

# Check specific crate
cargo check -p klyntbot-providers
```

---

## Making Changes

### Finding the Right Crate

Identify which crate to modify:

| Change Type | Crate |
|-------------|-------|
| Error types, shared types | `klyntbot-core` |
| Configuration schema | `klyntbot-config` |
| Message bus logic | `klyntbot-bus` |
| LLM provider | `klyntbot-providers` |
| Tool implementation | `klyntbot-tools` |
| Chat platform integration | `klyntbot-channels` |
| Agent loop, memory, skills | `klyntbot-agent` |
| CLI commands | `klyntbot-cli` |

### Branch Naming

Use descriptive branch names:

```bash
git checkout -b feature/add-anthropic-streaming
git checkout -b fix/telegram-reconnect-loop
git checkout -b refactor/tool-parameter-validation
```

### Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <description>

[optional body]
```

**Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

**Examples**:
```
feat(providers): add streaming support for Anthropic
fix(channels): handle Discord rate limits correctly
refactor(tools): simplify file path validation
docs(architecture): update crate dependency diagram
test(agent): add tests for skill loading
```

---

## Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p klyntbot-agent

# Specific test
cargo test --test agent_loop_tests

# With output
cargo test -- --nocapture

# Single test function
cargo test test_session_persistence
```

### Test Organization

- **Unit tests**: `#[cfg(test)] mod tests` in each crate
- **Integration tests**: `tests/` directory (uses facade crate)
- **Test helpers**: `tests/mock_provider.rs`

### Writing Tests

```rust
// Unit test in klyntbot-tools/src/filesystem.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() {
        let tool = ReadFileTool::new(None);
        // Test implementation
    }
}
```

```rust
// Integration test in tests/agent_loop_tests.rs
use klyntbot::{AgentLoop, Config, MessageBus};

#[tokio::test]
async fn test_basic_chat() {
    // Cross-crate integration test
}
```

### Test Coverage

Aim for:
- **Unit tests**: All public functions
- **Integration tests**: End-to-end workflows
- **Error cases**: All error paths

---

## Code Style

### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting
cargo fmt --all --check
```

### Linting

```bash
# Run clippy
cargo clippy --workspace --all-targets --all-features

# Fix warnings
cargo clippy --workspace --fix
```

### Clippy Rules

Klyntbot requires **zero clippy warnings**. Common issues:

```rust
// ❌ Avoid
let x = some_option.unwrap();  // Use ? or pattern matching

// ✅ Prefer
let x = some_option?;
let x = some_option.unwrap_or_default();
```

### Documentation

Document public items:

```rust
/// Creates a new LLM provider for the given model.
///
/// # Arguments
///
/// * `model` - The model name (e.g., "claude-sonnet-4-5")
/// * `config` - Configuration containing API keys
///
/// # Returns
///
/// A boxed provider implementing `LlmProvider`
///
/// # Errors
///
/// Returns `ProviderError::AuthFailed` if API key is missing
pub fn create_provider(model: &str, config: &Config) -> Result<DynProvider> {
    // Implementation
}
```

---

## Pull Request Process

### Before Submitting

1. **Run checks**:
   ```bash
   cargo test --workspace
   cargo clippy --workspace --all-targets --all-features
   cargo fmt --all --check
   ```

2. **Update documentation**:
   - Add/update code comments
   - Update README if public API changes
   - Update ARCHITECTURE.md for structural changes

3. **Add tests**:
   - Unit tests for new functions
   - Integration tests for new features
   - Regression tests for bug fixes

### PR Template

```markdown
## Description

Brief description of changes.

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] Clippy passes (`cargo clippy --workspace`)
- [ ] Code formatted (`cargo fmt --all`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if applicable)

## Testing

Describe how you tested these changes.
```

### Review Process

1. **Automated checks**: CI runs tests, clippy, formatting
2. **Code review**: Maintainer reviews code
3. **Approval**: At least one approval required
4. **Merge**: Squash and merge to main

---

## Adding Features

### Adding a New Tool

1. **Create tool implementation** in `klyntbot-tools/src/my_tool.rs`:

```rust
use async_trait::async_trait;
use serde_json::Value;
use klyntbot_core::Result;
use super::{Tool, RoutingContext};

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
                "input": { "type": "string" }
            },
            "required": ["input"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let input = args["input"].as_str().unwrap();
        Ok(format!("Processed: {}", input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_tool() {
        let tool = MyTool;
        let result = tool.execute(
            serde_json::json!({"input": "test"}),
            &RoutingContext::default()
        ).await.unwrap();
        assert_eq!(result, "Processed: test");
    }
}
```

2. **Register tool** in `klyntbot-tools/src/registry.rs`:

```rust
pub fn create_tool_registry(...) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(MyTool));
    // ... existing tools
    registry
}
```

3. **Add integration test** in `tests/tool_tests.rs`:

```rust
#[tokio::test]
async fn test_my_tool_integration() {
    // End-to-end test
}
```

### Adding a New Provider

1. **Implement `LlmProvider`** in `klyntbot-providers/src/my_provider.rs`
2. **Register in provider registry** in `klyntbot-providers/src/registry.rs`
3. **Add config schema** in `klyntbot-config/src/schema.rs`
4. **Add tests**

See [ARCHITECTURE.md - Extension Points](docs/ARCHITECTURE.md#extension-points) for details.

### Adding a New Channel

1. **Implement `Channel` trait** in `klyntbot-channels/src/my_channel.rs`
2. **Add to channel manager** in `klyntbot-channels/src/manager.rs`
3. **Add config schema** in `klyntbot-config/src/schema.rs`
4. **Add integration test**

### Adding a New CLI Command

1. **Add command enum** in `klyntbot-cli/src/commands.rs`:

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands
    MyCommand {
        #[arg(short, long)]
        option: String,
    },
}
```

2. **Add handler** in `klyntbot-cli/src/my_command.rs`:

```rust
pub async fn handle_my_command(option: String) -> Result<()> {
    // Implementation
    Ok(())
}
```

3. **Wire in main handler**

---

## Workspace-Specific Guidelines

### Import Conventions

```rust
// ✅ Use crate-specific imports
use klyntbot_core::{Result, KlyntbotError};
use klyntbot_config::Config;
use klyntbot_providers::create_provider;

// ❌ Don't use crate:: for other crates
use crate::providers::create_provider;  // Wrong!
```

### Dependency Rules

- Lower layers cannot depend on higher layers
- Use handler traits for cross-layer interactions
- Add new dependencies to workspace `Cargo.toml` first

### Error Handling

```rust
// ✅ Use klyntbot_core::Result
pub fn do_something() -> Result<String> {
    Err(ToolError::ExecutionFailed("Failed".into()).into())
}

// ✅ Automatic error conversion
pub fn call_provider() -> Result<String> {
    let resp = reqwest::get(url).await?;  // reqwest::Error → ProviderError → KlyntbotError
    Ok(resp.text().await?)
}
```

### Feature Flags

If adding optional functionality:

1. **Add feature to crate's `Cargo.toml`**:
   ```toml
   [features]
   my_feature = ["dep:some-crate"]
   ```

2. **Gate code**:
   ```rust
   #[cfg(feature = "my_feature")]
   pub fn my_function() { }
   ```

3. **Update workspace `Cargo.toml`**:
   ```toml
   [features]
   my_feature = ["klyntbot-foo/my_feature"]
   ```

---

## Getting Help

- **Documentation**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Migration Guide**: [docs/MIGRATION.md](docs/MIGRATION.md)
- **Issues**: [GitHub Issues](https://github.com/KlyntLabs/klyntbot/issues)
- **Discussions**: [GitHub Discussions](https://github.com/KlyntLabs/klyntbot/discussions)

---

## Code of Conduct

Be respectful, inclusive, and collaborative. We welcome contributions from everyone.

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
