# cli

**Command-line interface and REPL.**

## Overview

`cli` provides the CLI commands and interactive REPL for klyntbot:
- Command parsing with `clap`
- Interactive chat REPL with history
- Gateway server mode
- Status, config, and management commands
- Setup wizard

## Contents

### CLI Commands

```rust
use cli::{Cli, Commands};
use clap::Parser;

let cli = Cli::parse();

match cli.command {
    Commands::Chat { message } => handle_chat(message).await?,
    Commands::Serve { port } => handle_serve(port).await?,
    Commands::Status => handle_status().await?,
    Commands::Init => handle_init().await?,
    Commands::Channels { subcommand } => handle_channels(subcommand).await?,
    Commands::Cron { subcommand } => handle_cron(subcommand).await?,
    Commands::Config { subcommand } => handle_config(subcommand).await?,
    Commands::Skills { subcommand } => handle_skills(subcommand).await?,
}
```

### Available Commands

| Command | Description |
|---------|-------------|
| `klyntbot chat` | Interactive chat (REPL) |
| `klyntbot chat "message"` | Single message mode |
| `klyntbot serve` | Start gateway (all enabled channels) |
| `klyntbot init` | Initialize config and workspace |
| `klyntbot status` | Show config, provider, workspace info |
| `klyntbot channels` | Channel management |
| `klyntbot cron` | Cron job management |
| `klyntbot config` | Config commands |
| `klyntbot skills` | Skill management |

### Chat REPL

```rust
use cli::handle_chat;

// Interactive mode with history
// Supports:
// - Markdown rendering
// - Command history (up/down arrows)
// - Multi-line input
// - Ctrl+C to cancel
// - Ctrl+D to exit

handle_chat(None).await?;
```

**Features**:
- Persistent command history (`~/.klyntbot/history/cli_history`)
- Markdown rendering with syntax highlighting
- Typing indicators during LLM calls
- Clean startup (no banner spam)
- Multi-line input support

### Gateway Server

```rust
use cli::handle_serve;

// Start all enabled channels
handle_serve(Some(8080)).await?;

// Starts:
// - Channel manager (Telegram, Discord, etc.)
// - Agent loop
// - Cron service
// - Heartbeat service
// - Message bus
```

### Status Command

```rust
use cli::handle_status;

handle_status().await?;

// Displays:
// - Config location and validity
// - Configured providers (masked keys)
// - Enabled channels
// - Workspace location
// - Active sessions
```

### Init Command

```rust
use cli::handle_init;

handle_init().await?;

// Creates:
// - ~/.klyntbot/config.json (default config)
// - ~/.klyntbot/workspace/ (workspace directory)
// - Workspace files (AGENTS.md, SOUL.md, etc.)
```

## Usage

The CLI is typically invoked via the root `klyntbot` binary:

```bash
# Build the CLI
cargo build --release

# Run commands
./target/release/klyntbot chat
./target/release/klyntbot serve
./target/release/klyntbot status
```

As a library:

```toml
[dependencies]
cli.workspace = true
```

```rust
use cli::{Cli, Commands};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { message } => {
            cli::handle_chat(message).await?;
        }
        _ => {}
    }

    Ok(())
}
```

## Command Details

### Chat

**Interactive mode**:
```bash
klyntbot chat
> Hello!
Assistant: Hi there! How can I help you?
> What can you do?
Assistant: I can help with...
> exit
```

**Single message mode**:
```bash
klyntbot chat "What is Rust?"
Assistant: Rust is a systems programming language...
```

### Serve

**Default port**:
```bash
klyntbot serve
[INFO] Starting klyntbot gateway on 0.0.0.0:18790
[INFO] Telegram: Connected
[INFO] Discord: Connected
```

**Custom port**:
```bash
klyntbot serve --port 8080
[INFO] Starting klyntbot gateway on 0.0.0.0:8080
```

### Channels

```bash
# List channels
klyntbot channels list

# Login to WhatsApp (QR code)
klyntbot channels login whatsapp

# Test channel
klyntbot channels test telegram
```

### Cron

```bash
# List jobs
klyntbot cron list

# Add job
klyntbot cron add \
  --name "daily-reminder" \
  --cron "0 9 * * *" \
  --message "Good morning!"

# Delete job
klyntbot cron delete <job-id>
```

### Config

```bash
# Show config
klyntbot config show

# Validate config
klyntbot config validate

# Edit config (opens $EDITOR)
klyntbot config edit
```

### Skills

```bash
# List skills
klyntbot skills list

# Show skill content
klyntbot skills show cron

# Create new skill
klyntbot skills create my-skill
```

## REPL Features

### Command History

```bash
# Navigate with arrows
↑  # Previous command
↓  # Next command
```

History stored in: `~/.klyntbot/history/cli_history`

### Markdown Rendering

```
> Explain Rust ownership

# Rust Ownership

Rust's ownership system...

**Key principles:**
- Each value has a single owner
- Values are dropped when owner goes out of scope
```

### Multi-line Input

```
> Can you help me with
| a multi-line
| question?
```

Press Enter after each line, then empty line to send.

## Error Handling

All commands return `Result<()>` with detailed error messages:

```rust
// Config not found
Error: Config file not found at ~/.klyntbot/config.json
Help: Run `klyntbot init` to create default config

// Invalid model
Error: Unknown model: gpt-99
Help: Check model name in config or set ANTHROPIC_API_KEY

// Channel auth failed
Error: Telegram authentication failed
Help: Check token in config.channels.telegram.token
```

## Design Principles

1. **User-friendly** — Clear error messages, helpful hints
2. **Interactive** — REPL with history and markdown
3. **Composable** — Commands can be scripted
4. **Async-native** — Non-blocking CLI operations
5. **Zero-config** — Reasonable defaults, explicit overrides

## Dependencies

- `common` — Error types
- `config` — Configuration
- `bus` — Message bus
- `providers` — LLM providers
- `agent` — Agent loop
- `channels` — Channel manager
- `scheduling` — Cron service
- `heartbeat` — Heartbeat service
- `session` — Session manager
- `tools` — Tool registry
- `clap` — CLI argument parsing
- `rustyline` — REPL with history
- `tokio` — Async runtime
- `anyhow` — Error handling
- `tracing`, `tracing-subscriber` — Logging

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [CLI Reference](../../README.md#cli-reference)
- [Configuration](../../README.md#configuration)
