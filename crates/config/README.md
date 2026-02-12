# config

**Configuration schema and file I/O for klyntbot.**

## Overview

`config` handles all configuration management for klyntbot:
- Configuration schema definition
- JSON file loading and saving
- Environment variable overrides
- Secret masking for sensitive values

## Contents

### Configuration Schema

```rust
use config::{Config, Secret};

// Top-level config structure
pub struct Config {
    pub agents: AgentsConfig,
    pub providers: ProvidersConfig,
    pub channels: ChannelsConfig,
    pub gateway: GatewayConfig,
    pub tools: ToolsConfig,
}

// Secret wrapper with automatic masking
pub struct Secret<T> {
    inner: T,
}
```

**Config sections:**
- `AgentsConfig` — Agent behavior, workspace, model settings
- `ProvidersConfig` — LLM provider API keys and base URLs
- `ChannelsConfig` — Channel tokens and settings
- `GatewayConfig` — Gateway server host/port
- `ToolsConfig` — Tool restrictions and API keys

### Loading Configuration

```rust
use config::{load, load_with_env_overrides};

// Load from default location (~/.klyntbot/config.json)
let config = load()?;

// Load with environment variable overrides
let config = load_with_env_overrides()?;
```

**Environment variable override format:**
```bash
# Pattern: KLYNTBOT_SECTION__SUBSECTION__FIELD
export KLYNTBOT_AGENTS__DEFAULTS__MODEL="gpt-4o"
export KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
export ANTHROPIC_API_KEY="sk-ant-..."  # Direct provider key
```

### Saving Configuration

```rust
use config::save;

let mut config = Config::default();
config.agents.defaults.model = "claude-sonnet-4-5".into();

save(&config)?;  // Saves to ~/.klyntbot/config.json
```

### Path Utilities

```rust
use config::{config_dir, config_path};

// Get config directory path
let dir = config_dir();  // ~/.klyntbot/

// Get config file path
let path = config_path();  // ~/.klyntbot/config.json
```

### Secret Masking

```rust
use config::Secret;
use serde_json::to_string;

let api_key = Secret::new("sk-ant-secret123".to_string());

// Serializes as "***" for logging/display
println!("{}", to_string(&api_key)?);  // "***"

// Access real value
let real_key = api_key.expose();
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
config.workspace = true
```

Example:

```rust
use config::{load, Config};

fn main() -> Result<()> {
    let config = load()?;

    println!("Workspace: {}", config.agents.defaults.workspace);
    println!("Model: {}", config.agents.defaults.model);

    if let Some(key) = &config.providers.anthropic.api_key {
        println!("Anthropic configured: Yes");
    }

    Ok(())
}
```

## Configuration Format

### File Structure

```json
{
  "agents": {
    "defaults": {
      "workspace": "~/.klyntbot/workspace",
      "model": "anthropic/claude-sonnet-4-5-20250929",
      "maxTokens": 8192,
      "temperature": 0.7,
      "maxToolIterations": 20
    }
  },
  "providers": {
    "anthropic": { "apiKey": "sk-ant-..." },
    "openai": { "apiKey": "sk-..." },
    "openrouter": { "apiKey": "sk-or-..." }
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "123:ABC",
      "allowFrom": ["user_id"]
    }
  },
  "tools": {
    "restrictToWorkspace": false,
    "web": {
      "search": { "apiKey": "", "maxResults": 5 }
    }
  }
}
```

### Config Path

Default location: `~/.klyntbot/config.json`

## Design Principles

1. **Typed schema** — Use Rust structs, not generic JSON
2. **Serde derive** — Automatic serialization with camelCase
3. **Secret protection** — API keys never logged or displayed
4. **Environment overrides** — Config can be overridden without file edits

## Dependencies

- `common` — Error types
- `serde`, `serde_json` — Serialization
- `dirs` — Home directory resolution
- `shellexpand` — Tilde expansion in paths

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Configuration Reference](../../README.md#configuration)
