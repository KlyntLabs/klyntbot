<div align="center">
  <img src="docs/klyntbot_logo.png" alt="klyntbot" width="420">

  <h1>klyntbot</h1>

  <p><strong>A high-performance AI agent framework, rewritten in Rust.</strong></p>

  <p>
    <a href="https://github.com/KlyntLabs/klyntbot/releases"><img src="https://img.shields.io/github/v/release/KlyntLabs/klyntbot?style=flat-square&color=blue" alt="Release"></a>
    <img src="https://img.shields.io/badge/rust-1.75+-orange?style=flat-square&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/tests-242%20passed-brightgreen?style=flat-square" alt="Tests">
    <img src="https://img.shields.io/badge/clippy-0%20warnings-brightgreen?style=flat-square" alt="Clippy">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"></a>
  </p>

  <p>
    <a href="#quick-start">Quick Start</a> &middot;
    <a href="#features">Features</a> &middot;
    <a href="#benchmarks">Benchmarks</a> &middot;
    <a href="#channels">Channels</a> &middot;
    <a href="#architecture">Architecture</a> &middot;
    <a href="#migration-from-nanobot">Migration</a>
  </p>
</div>

---

## Why klyntbot

[nanobot](https://github.com/HKUDS/nanobot) proved that a full-featured AI agent can be built in under 4,000 lines of Python. klyntbot takes that same architecture and rewrites it ground-up in Rust to solve the problems that Python cannot:

- **Python's import chain** adds 180ms+ before a single line of your code runs. klyntbot starts in **5.8ms**.
- **LiteLLM alone** consumes **155 MB** of RAM just to import. klyntbot's entire process idles at **8.7 MB**.
- **pip install** pulls in 60+ packages totaling 194 MB. klyntbot is a **single 10 MB binary** with zero runtime dependencies.
- **The GIL** limits Python to one thread of execution. klyntbot uses **tokio** for true multi-threaded async I/O.

The result is the same agent -- same tools, same channels, same config format -- running with dramatically fewer resources. Deploy it on a $5 VPS, a Raspberry Pi, or a container with a 20 MB image.

---

## Benchmarks

All measurements taken on Apple M-series, comparing klyntbot v0.1.0 (Rust) against nanobot v0.1.3 (Python 3.13):

| Metric | nanobot (Python) | klyntbot (Rust) | Delta |
|--------|:---:|:---:|:---:|
| **Startup time** | 186.5 ms | 5.8 ms | **32x faster** |
| **Memory (CLI)** | 14.6 MB | 8.3 MB | **1.8x less** |
| **Memory (full import)** | 155 MB | 8.7 MB | **17.8x less** |
| **Install size** | 194 MB (site-packages) | 10 MB (single binary) | **19.4x smaller** |
| **Runtime deps** | 60+ Python packages | 0 | **Zero-dep** |
| **Test suite** | ~30 tests | 242 tests | **8x coverage** |
| **Clippy warnings** | N/A | 0 | Clean |

> The memory difference comes from eliminating LiteLLM (155 MB on import). klyntbot replaces it with a ~400-line OpenAI-compatible HTTP client that handles all 12+ providers directly via `reqwest`.

---

## Quick Start

**1. Build from source**

```bash
git clone https://github.com/KlyntLabs/klyntbot.git
cd klyntbot
cargo build --release
```

**2. Initialize**

```bash
./target/release/klyntbot init
```

**3. Configure** -- add your API key to `~/.klyntbot/config.json`:

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-your-key-here"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-sonnet-4-5-20250929"
    }
  }
}
```

**4. Chat**

```bash
./target/release/klyntbot chat "Hello, what can you do?"

# Or start interactive mode
./target/release/klyntbot chat
```

> Get API keys from: [Anthropic](https://console.anthropic.com/) | [OpenRouter](https://openrouter.ai/keys) (200+ models) | [OpenAI](https://platform.openai.com/) | [DeepSeek](https://platform.deepseek.com/)

---

## Features

### Core Agent

- **Agent Loop** -- Receive message, build context, call LLM, execute tools, return response. Configurable iteration limit (default 20) prevents runaway tool loops.
- **Persistent Memory** -- Long-term memory (`MEMORY.md`) and daily notes (`YYYY-MM-DD.md`) survive across sessions and restarts.
- **Progressive Skill Loading** -- Built-in skills (cron, github, weather, tmux, summarize) plus custom skills in your workspace. Always-on skills load into the system prompt; others load on-demand to save tokens.
- **Session Management** -- Per-channel, per-user conversation history stored as JSONL. In-memory LRU cache for active sessions with configurable history depth (default 50 messages).
- **Background Tasks** -- Spawn subagents for long-running work. They execute independently with an isolated tool set and report results back to the main conversation.

### Tool System

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents with optional workspace restriction |
| `write_file` | Write files, auto-creating parent directories |
| `edit_file` | Find-and-replace with uniqueness validation |
| `list_dir` | List directory contents with type indicators |
| `exec` | Execute shell commands with safety guards, timeout enforcement, and output truncation |
| `web_search` | Search the web via Brave Search API |
| `web_fetch` | Fetch and extract readable content from URLs (HTML to markdown) |
| `message` | Send messages to users through any connected channel |
| `spawn` | Create background subagents for complex tasks |
| `cron` | Schedule recurring tasks with natural language or cron expressions |

### Safety

- **Command deny patterns** -- Blocks `rm -rf`, fork bombs, disk formatting, system shutdown, and other destructive operations via regex matching.
- **Workspace sandboxing** -- Optional `restrictToWorkspace` mode confines all file and shell operations to the workspace directory. Path traversal attempts are rejected.
- **Access control** -- Per-channel `allowFrom` lists restrict which users can interact with the agent.
- **Output truncation** -- Shell output capped at 10 KB, web fetch at 50 KB, preventing memory exhaustion.
- **API key protection** -- Keys are never logged. Status display masks sensitive values.

### Scheduling

- **Cron service** -- Three schedule types: `at` (one-time), `every` (interval), `cron` (expression). Jobs persist to disk and survive restarts.
- **Heartbeat service** -- Periodic agent wake-up (configurable interval). Reads `HEARTBEAT.md` for actionable tasks and executes them proactively.

---

## Channels

Connect the same agent to multiple chat platforms simultaneously. Each channel maintains separate conversation history.

| Channel | Transport | Status | Notes |
|---------|-----------|:------:|-------|
| **Telegram** | Bot API (long polling) | Ready | Voice transcription (Groq Whisper), markdown-to-HTML, typing indicators, proxy support |
| **Discord** | WebSocket Gateway v10 | Ready | Auto-reconnect, rate limit handling, attachment download, typing indicators |
| **WhatsApp** | WebSocket bridge | Ready | Requires Node.js bridge (Baileys). QR code auth via `klyntbot channels login` |
| **Slack** | Socket Mode | Ready | DM and group policy (mention/open/allowlist), thread-based replies |
| **Email** | IMAP + SMTP | Ready | Consent gate, HTML-to-text extraction, In-Reply-To threading, auto-reply toggle |
| **QQ** | WebSocket (botpy) | Ready | C2C private messages, sandbox mode support |
| **Feishu** | WebSocket | Planned | Lark long connection, no public IP required |
| **DingTalk** | Stream Mode | Planned | OAuth2 token management, batch send API |
| **Mochat** | Socket.IO | Planned | Reply delay modes, cursor-based message tracking |

<details>
<summary><strong>Telegram setup</strong></summary>

1. Create a bot via [@BotFather](https://t.me/BotFather) on Telegram
2. Copy the token and add it to your config:

```json
{
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "YOUR_BOT_TOKEN",
      "allowFrom": ["YOUR_USER_ID"]
    }
  }
}
```

3. Start the gateway:

```bash
klyntbot serve
```

</details>

<details>
<summary><strong>Discord setup</strong></summary>

1. Create an application at [discord.com/developers](https://discord.com/developers/applications)
2. Enable **MESSAGE CONTENT INTENT** in Bot settings
3. Generate an invite URL with `bot` scope and `Send Messages` + `Read Message History` permissions

```json
{
  "channels": {
    "discord": {
      "enabled": true,
      "token": "YOUR_BOT_TOKEN",
      "allowFrom": ["YOUR_USER_ID"]
    }
  }
}
```

4. Start the gateway:

```bash
klyntbot serve
```

</details>

<details>
<summary><strong>All channel configurations</strong></summary>

Full channel config block with all options:

```json
{
  "channels": {
    "telegram": {
      "enabled": false,
      "token": "",
      "allowFrom": [],
      "proxy": null
    },
    "discord": {
      "enabled": false,
      "token": "",
      "allowFrom": [],
      "gatewayUrl": "wss://gateway.discord.gg/?v=10&encoding=json",
      "intents": 37377
    },
    "whatsapp": {
      "enabled": false,
      "bridgeUrl": "ws://localhost:3001",
      "allowFrom": []
    },
    "slack": {
      "enabled": false,
      "botToken": "",
      "appToken": "",
      "groupPolicy": "mention",
      "dm": { "enabled": true, "policy": "open" }
    },
    "email": {
      "enabled": false,
      "consentGranted": false,
      "imapHost": "",
      "imapPort": 993,
      "smtpHost": "",
      "smtpPort": 587,
      "fromAddress": "",
      "allowFrom": []
    },
    "qq": {
      "enabled": false,
      "appId": "",
      "secret": "",
      "allowFrom": []
    }
  }
}
```

</details>

---

## LLM Providers

klyntbot supports 12+ LLM providers through a unified OpenAI-compatible HTTP client. No LiteLLM dependency -- model routing is handled by a provider registry with keyword-based auto-detection.

| Provider | Models | Type | Get API Key |
|----------|--------|------|-------------|
| **Anthropic** | Claude 4.5/4.6 (Opus, Sonnet, Haiku) | Direct | [console.anthropic.com](https://console.anthropic.com) |
| **OpenAI** | GPT-4o, GPT-4, o1, o3 | Direct | [platform.openai.com](https://platform.openai.com) |
| **DeepSeek** | DeepSeek-R1, DeepSeek-V3 | Direct | [platform.deepseek.com](https://platform.deepseek.com) |
| **Google** | Gemini 2.0, Gemini Pro | Direct | [aistudio.google.com](https://aistudio.google.com) |
| **Groq** | Llama 3.x, Mixtral, Whisper (transcription) | Direct | [console.groq.com](https://console.groq.com) |
| **OpenRouter** | 200+ models from all providers | Gateway | [openrouter.ai](https://openrouter.ai) |
| **AiHubMix** | Multi-provider gateway | Gateway | [aihubmix.com](https://aihubmix.com) |
| **Zhipu** | GLM-4, GLM-Z | Direct | [open.bigmodel.cn](https://open.bigmodel.cn) |
| **DashScope** | Qwen models | Direct | [dashscope.console.aliyun.com](https://dashscope.console.aliyun.com) |
| **Moonshot** | Kimi K2.5 | Direct | [platform.moonshot.cn](https://platform.moonshot.cn) |
| **MiniMax** | MiniMax models | Direct | [platform.minimax.io](https://platform.minimax.io) |
| **vLLM** | Any local model | Local | -- |

**Provider auto-detection**: Set a model name in config, and klyntbot resolves the correct provider by keyword matching. `claude-sonnet-4-5-20250929` routes to Anthropic, `gpt-4o` routes to OpenAI, `deepseek-r1` routes to DeepSeek. Gateway providers (OpenRouter, AiHubMix) are detected by API key prefix or base URL.

### Local models

Run with any OpenAI-compatible server (vLLM, Ollama, LM Studio):

```json
{
  "providers": {
    "vllm": {
      "apiKey": "dummy",
      "apiBase": "http://localhost:8000/v1"
    }
  },
  "agents": {
    "defaults": {
      "model": "meta-llama/Llama-3.1-8B-Instruct"
    }
  }
}
```

---

## Architecture

```
                            +------------------+
                            |   CLI / REPL     |
                            |   (clap derive)  |
                            +--------+---------+
                                     |
                   +-----------------+------------------+
                   |                                    |
          +--------v---------+              +-----------v----------+
          |   Gateway Mode   |              |    Agent Mode        |
          |  (all channels)  |              |    (direct CLI)      |
          +--------+---------+              +-----------+----------+
                   |                                    |
                   +----------------+-------------------+
                                    |
                     +--------------v---------------+
                     |         MessageBus           |
                     |      (tokio::sync::mpsc)     |
                     |   +--------+  +---------+   |
                     |   |Inbound |  |Outbound |   |
                     |   | Queue  |  | Queue   |   |
                     |   +---+----+  +----^----+   |
                     +-------+-------------+-------+
                             |             |
           +-----------------+--+          |
           |                    |          |
+----------v-----------+  +----v----------v------+
|   Channel Manager    |  |     Agent Loop       |
|  +----------------+  |  |  +----------------+  |
|  | Telegram       |<-+--+  | Context Builder|  |
|  | Discord        |  |  |  | Memory Store   |  |
|  | WhatsApp       |  |  |  | Skill Manager  |  |
|  | Slack / Email  |  |  |  +----------------+  |
|  | QQ             |  |  |  +----------------+  |
|  +----------------+  |  |  | Tool Registry  |  |
+-----------------------+  |  | LLM Provider   |  |
                           |  | Subagent Mgr   |  |
+------------------+       |  +----------------+  |
| Cron Service     +-------+                      |
| Heartbeat Service|       +----------------------+
+------------------+
                           +------------------+
                           | Session Manager  |
                           | (JSONL + cache)  |
                           +------------------+
```

### Key design decisions

| Decision | Rationale |
|----------|-----------|
| `tokio::mpsc` for message bus | Bounded channels with backpressure. Single consumer (agent loop) preserves message ordering. |
| `reqwest` for LLM calls | Direct OpenAI-compatible HTTP. No LiteLLM overhead. All 12+ providers use the same `/v1/chat/completions` endpoint format. |
| `serde` for config | Compile-time schema validation via derive macros. camelCase JSON serialization matches nanobot's config format exactly. |
| `thiserror` for errors | 7 error types (Tool, Provider, Channel, Session, Config, Cron, Klyntbot) with automatic `From` conversions. |
| `Arc<dyn Trait>` for tools/providers | Runtime polymorphism with shared ownership across async tasks. `Send + Sync` bounds enforced at compile time. |
| Feature-gated channels | Email deps (IMAP, SMTP, TLS) are optional. Minimal builds exclude unused channel code. |

---

## CLI Reference

```bash
klyntbot chat                    # Interactive chat (REPL with history)
klyntbot chat "message"          # Single message mode
klyntbot serve                   # Start gateway (all enabled channels)
klyntbot serve --port 8080       # Custom gateway port
klyntbot init                    # Initialize config and workspace
klyntbot status                  # Show config, provider, workspace info
klyntbot channels                # Channel management
klyntbot cron list               # List scheduled jobs
klyntbot cron add --name "daily" --cron "0 9 * * *" --message "Good morning!"
klyntbot config show             # Display current configuration
klyntbot config validate         # Validate config file
klyntbot skills list             # List available skills
```

### Environment variables

```bash
# Provider API keys (override config)
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export OPENROUTER_API_KEY="sk-or-..."
export GROQ_API_KEY="gsk_..."

# Config overrides (KLYNTBOT_ prefix, __ as nested separator)
export KLYNTBOT_AGENTS__DEFAULTS__MODEL="gpt-4o"
export KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

---

## Configuration

Config file: `~/.klyntbot/config.json` (also reads `~/.nanobot/config.json` as fallback for migration)

<details>
<summary><strong>Full config reference</strong></summary>

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
    "openrouter": { "apiKey": "", "apiBase": null, "extraHeaders": null },
    "anthropic": { "apiKey": "" },
    "openai": { "apiKey": "" },
    "deepseek": { "apiKey": "" },
    "groq": { "apiKey": "" },
    "gemini": { "apiKey": "" },
    "zhipu": { "apiKey": "" },
    "dashscope": { "apiKey": "" },
    "moonshot": { "apiKey": "" },
    "minimax": { "apiKey": "" },
    "aihubmix": { "apiKey": "", "apiBase": null },
    "vllm": { "apiKey": "dummy", "apiBase": "http://localhost:8000/v1" }
  },
  "channels": {
    "telegram": { "enabled": false, "token": "", "allowFrom": [] },
    "discord": { "enabled": false, "token": "", "allowFrom": [] },
    "whatsapp": { "enabled": false, "bridgeUrl": "ws://localhost:3001", "allowFrom": [] },
    "slack": { "enabled": false, "botToken": "", "appToken": "", "groupPolicy": "mention" },
    "email": { "enabled": false, "consentGranted": false, "imapHost": "", "smtpHost": "" },
    "qq": { "enabled": false, "appId": "", "secret": "" }
  },
  "gateway": { "host": "0.0.0.0", "port": 18790 },
  "tools": {
    "web": { "search": { "apiKey": "", "maxResults": 5 } },
    "exec": { "timeout": 60 },
    "restrictToWorkspace": false
  }
}
```

</details>

### Workspace layout

```
~/.klyntbot/
  config.json                   # Main configuration
  sessions/                     # Conversation history (JSONL per session)
  cron/jobs.json                # Scheduled job store
  media/                        # Downloaded media files
  history/cli_history           # REPL command history
  workspace/
    AGENTS.md                   # Agent instructions (behavior)
    SOUL.md                     # Personality definition
    USER.md                     # User preferences and info
    TOOLS.md                    # Tool usage guidelines
    IDENTITY.md                 # Identity overrides
    HEARTBEAT.md                # Periodic task definitions
    memory/
      MEMORY.md                 # Long-term persistent memory
      2026-02-12.md             # Daily notes (auto-dated)
    skills/
      custom-skill/SKILL.md     # User-defined skills
```

---

## Skills

klyntbot ships with 6 built-in skills compiled directly into the binary:

| Skill | Description | Requires |
|-------|-------------|----------|
| **cron** | Natural language scheduling | -- |
| **github** | Repository operations, issues, PRs | `gh` CLI |
| **weather** | Weather forecasts and conditions | -- |
| **summarize** | Document and content summarization | -- |
| **tmux** | Terminal multiplexer integration | `tmux` |
| **skill-creator** | Create new custom skills | -- |

### Custom skills

Add skills to `~/.klyntbot/workspace/skills/your-skill/SKILL.md`:

```markdown
---
description: "Your skill description"
metadata: '{"nanobot": {"requires": {"bins": ["your-tool"]}, "always": false}}'
---

# Skill Name

Instructions for the agent...
```

- `always: true` -- Skill content is always injected into the system prompt
- `always: false` -- Skill summary appears in prompt; full content loaded via `read_file` on demand
- `requires.bins` -- Skill only available when specified binaries are in `$PATH`
- `requires.env` -- Skill only available when specified env vars are set

---

## Security

klyntbot runs shell commands and reads/writes files on behalf of an LLM. The security model provides multiple defense layers:

| Layer | Mechanism | Default |
|-------|-----------|---------|
| **Channel** | `allowFrom` per-channel user allowlists | Allow all |
| **Workspace** | `restrictToWorkspace` confines all I/O to workspace dir | Off |
| **Shell** | Deny-pattern regex blocks destructive commands | On |
| **Agent** | Max tool iteration limit prevents infinite loops | 20 |
| **Network** | Max 5 redirects on web_fetch, URL validation | On |
| **Output** | Truncation limits (shell: 10 KB, web: 50 KB) | On |

**Blocked commands** (always enforced): `rm -rf`, `del /f`, `rmdir /s`, `format`, `mkfs`, `diskpart`, `dd if=`, `shutdown`, `reboot`, `poweroff`, fork bombs.

For production deployments:

```json
{
  "tools": { "restrictToWorkspace": true },
  "channels": {
    "telegram": { "allowFrom": ["YOUR_USER_ID"] }
  }
}
```

Recommended: `chmod 600 ~/.klyntbot/config.json` to protect API keys.

---

## Migration from nanobot

klyntbot is designed as a drop-in replacement for nanobot. The config format, workspace layout, session format, and CLI commands are compatible.

### Steps

```bash
# klyntbot reads nanobot's config as fallback
# Existing ~/.nanobot/config.json works without changes

# Or copy explicitly
cp ~/.nanobot/config.json ~/.klyntbot/config.json

# Sessions, memory, and skills carry over
# Same workspace directory structure
```

### What stays the same

- Config JSON format (camelCase field names, identical schema)
- Session JSONL files (readable by both)
- Workspace directory layout (AGENTS.md, SOUL.md, memory/, skills/)
- Tool names and parameter schemas
- Cron job format
- Skill file format (YAML frontmatter + markdown)

### What changes

| Area | nanobot | klyntbot |
|------|---------|----------|
| Binary | `nanobot` | `klyntbot` |
| Config path | `~/.nanobot/` | `~/.klyntbot/` (reads both) |
| Install | `pip install nanobot-ai` | Single binary download |
| LLM routing | LiteLLM library | Direct HTTP (same API format) |
| Commands | `nanobot agent` / `nanobot gateway` | `klyntbot chat` / `klyntbot serve` |
| Runtime | Python 3.11+ | None (statically linked) |

---

## Development

### Building

```bash
# Debug build
cargo build

# Release build (optimized, LTO, stripped)
cargo build --release

# Release binary location
./target/release/klyntbot
```

### Testing

```bash
# Run all 242 tests
cargo test

# Unit tests only
cargo test --lib

# Specific test suite
cargo test --test integration_tests
cargo test --test agent_loop_tests
cargo test --test skills_tests

# With output
cargo test -- --nocapture
```

### Linting

```bash
cargo clippy --all-targets --all-features
cargo fmt --check
```

### Project structure

```
klyntbot/
  src/
    main.rs                     # Entry point, CLI dispatch
    lib.rs                      # Library public API
    error.rs                    # Unified error types (7 enums)
    agent/                      # Agent loop, context, memory, skills, subagents
    bus/                        # Async message bus (tokio::mpsc)
    channels/                   # Chat platform integrations
    cli/                        # CLI commands (clap derive)
    config/                     # Config schema + loader
    cron/                       # Scheduled job service
    heartbeat/                  # Periodic wake-up service
    providers/                  # LLM provider abstraction + registry
    session/                    # Session persistence (JSONL + LRU cache)
    tools/                      # Tool implementations
    utils/                      # Path helpers, terminal formatting
  workspace/                    # Default workspace templates
  skills/                       # Built-in skill definitions
  tests/                        # Integration tests
  docs/                         # Architecture, PRD, design docs
```

### Code metrics

| Metric | Value |
|--------|-------|
| Source lines | 16,300 |
| Test lines | 2,436 |
| Test cases | 242 |
| Modules | 12 |
| Error types | 7 |
| Tools | 10 |
| Providers | 12 |
| Channels | 9 (6 ready, 3 planned) |

---

## Background

klyntbot exists because of a straightforward observation: AI agents spend 99% of their wall-clock time waiting for network I/O (LLM API calls, channel WebSockets, web fetches). Python's overhead -- import time, GIL contention, memory footprint -- adds nothing during that wait. A systems language eliminates that overhead entirely while preserving the same architecture.

The rewrite preserves nanobot's core insight: a useful AI agent doesn't need 430,000 lines of code. It needs a message bus, a tool registry, a context builder, and clean abstractions for providers and channels. klyntbot implements all of this in 16,300 lines of Rust with 242 tests, zero Clippy warnings, and a 10 MB binary.

### Design principles

1. **Feature parity first** -- Every nanobot feature has a corresponding klyntbot implementation. No features were cut for the rewrite.
2. **Zero-friction deployment** -- Download one binary, run `klyntbot init`, start chatting. No runtime, no package manager, no virtual environments.
3. **Config compatibility** -- klyntbot reads nanobot's `config.json` without changes. Migration is copying a binary.
4. **Async-native** -- All I/O runs on tokio. Channels, agent loop, cron, and heartbeat operate as independent async tasks.
5. **Trait-based extensibility** -- Adding a new provider, channel, or tool means implementing a trait. The registry pattern handles the rest.

---

## Acknowledgments

- [nanobot](https://github.com/HKUDS/nanobot) by HKUDS -- the original Python implementation that proved an agent framework can be ultra-lightweight
- [Anthropic](https://anthropic.com) -- Claude API
- The Rust ecosystem -- `tokio`, `serde`, `reqwest`, `clap`, `thiserror`, and the many crates that make this possible

---

## License

[MIT](LICENSE)

---

<p align="center">
  <sub>klyntbot -- high-performance AI agent framework, built with Rust</sub>
</p>
