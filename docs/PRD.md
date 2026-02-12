# klyntbot Product Requirements Document (PRD)

> **Version**: 1.0.0
> **Date**: 2026-02-11
> **Status**: Draft
> **Author**: Business Analyst (klyntbot-dev team)
> **Source**: Full analysis of [nanobot v0.1.3.post6](https://github.com/HKUDS/nanobot) codebase (~3,510 lines of Python)

---

## 1. Executive Summary

**klyntbot** is a ground-up Rust rewrite of nanobot, an ultra-lightweight personal AI assistant platform. nanobot delivers core agent functionality in ~3,500 lines of Python. klyntbot aims to preserve every feature while delivering order-of-magnitude improvements in startup time, memory footprint, binary size, and throughput.

### Why Rewrite in Rust?

| Dimension | nanobot (Python) | klyntbot (Rust) Target |
|-----------|-----------------|----------------------|
| Memory (idle) | ~80-150 MB (Python runtime + deps) | <10 MB |
| Startup time | ~2-5 seconds (import chain) | <100 ms |
| Binary size | ~200 MB+ (venv/pip) | <20 MB (single static binary) |
| Distribution | `pip install` + Python 3.11+ | Single binary download, `curl \| sh` |
| Concurrency | asyncio (GIL-bound) | tokio (true parallelism) |
| Dependencies | 20+ PyPI packages | Minimal, statically linked |

### Key Principle

**Feature parity first.** Every feature in nanobot must have a corresponding implementation in klyntbot. The rewrite adds no new features in v1.0 -- it is purely a performance and packaging improvement.

---

## 2. Product Vision

> An ultra-lightweight, blazing-fast, open-source AI assistant platform that runs anywhere as a single binary, connects to any chat platform, speaks to any LLM, and gives users full control over their AI assistant's personality, memory, and skills.

### Design Pillars

1. **Zero-friction deployment**: Download one binary, run one command, start chatting
2. **Universal connectivity**: Every major chat platform and LLM provider supported out of the box
3. **Developer-friendly**: Clean architecture, easy to extend with new providers/channels/tools
4. **Resource-efficient**: Run on a $5/month VPS, a Raspberry Pi, or a spare laptop
5. **Privacy-first**: All data stored locally, no telemetry, user controls everything

---

## 3. Goals & Non-Goals

### Goals

- **G1**: 1:1 feature parity with nanobot v0.1.3.post6
- **G2**: Single static binary for Linux (x86_64, aarch64) and macOS (x86_64, aarch64)
- **G3**: <10 MB idle memory, <100 ms startup, <20 MB binary
- **G4**: <5 ms processing overhead per message (excluding LLM API latency)
- **G5**: Zero-copy message passing within the agent pipeline where possible
- **G6**: Config file compatibility -- `~/.nanobot/config.json` works as-is (with alias `~/.klyntbot/config.json`)
- **G7**: Drop-in replacement for nanobot with migration guide

### Non-Goals

- **NG1**: GUI or web dashboard (CLI and chat channels only in v1.0)
- **NG2**: Embedded LLM inference (we call external APIs, not run models locally)
- **NG3**: Plugin/extension API beyond the skills system (v2.0 scope)
- **NG4**: Multi-user/multi-tenant mode (single-user assistant)
- **NG5**: End-to-end encryption of session data (local filesystem trust model)

---

## 4. Feature Matrix

### P0 -- Must Have (MVP)

These features are required for the first usable release.

| # | Feature | nanobot Source | Description |
|---|---------|---------------|-------------|
| P0-1 | **Core Agent Loop** | `agent/loop.py` | Receive message -> build context -> call LLM -> execute tools -> return response. Max iteration limit (default 20). |
| P0-2 | **CLI Interface** | `cli/commands.py` | `klyntbot agent -m "..."` (single message), `klyntbot agent` (interactive REPL), `klyntbot onboard`, `klyntbot status`, `klyntbot gateway` |
| P0-3 | **Message Bus** | `bus/queue.py`, `bus/events.py` | Async MPMC message bus with InboundMessage/OutboundMessage types, channel-based subscriber dispatch |
| P0-4 | **Configuration System** | `config/schema.py`, `config/loader.py` | JSON config at `~/.klyntbot/config.json` (also reads `~/.nanobot/config.json`), camelCase JSON <-> snake_case internal, Pydantic-equivalent validation, env var override (`KLYNTBOT_*`) |
| P0-5 | **Session Management** | `session/manager.py` | JSONL-based session persistence at `~/.klyntbot/sessions/`, in-memory LRU cache, max 50 messages in LLM context, session key = `channel:chat_id` |
| P0-6 | **Context Builder** | `agent/context.py` | System prompt assembly from bootstrap files (AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md), memory injection, skills summary, current time/runtime/workspace metadata |
| P0-7 | **Filesystem Tools** | `agent/tools/filesystem.py` | `read_file`, `write_file`, `edit_file` (exact string replacement with uniqueness check), `list_dir` -- all with optional workspace restriction and path traversal protection |
| P0-8 | **Shell Tool** | `agent/tools/shell.py` | `exec` -- run shell commands with configurable timeout (default 60s), safety guard with deny patterns (rm -rf, fork bombs, disk ops, etc.), optional workspace restriction, output truncation at 10KB |
| P0-9 | **Web Tools** | `agent/tools/web.py` | `web_search` (Brave Search API, configurable result count 1-10), `web_fetch` (HTTP GET with Readability-based content extraction, markdown/text modes, URL validation, redirect following, max 50KB) |
| P0-10 | **Message Tool** | `agent/tools/message.py` | `message` -- send messages to specific chat channels with channel/chat_id targeting, context-aware defaults |
| P0-11 | **Tool Registry** | `agent/tools/registry.py`, `agent/tools/base.py` | Dynamic tool registration, JSON Schema parameter definitions, OpenAI function-calling format, parameter validation, async execution |
| P0-12 | **LLM Provider (OpenAI-compatible)** | `providers/litellm_provider.py`, `providers/base.py` | Abstract `LLMProvider` trait: `chat()` with messages/tools/model/max_tokens/temperature, `LLMResponse` with content + tool_calls + usage + reasoning_content. Initial implementation: direct OpenAI-compatible HTTP client (replaces LiteLLM). |
| P0-13 | **Provider Registry** | `providers/registry.py` | `ProviderSpec` metadata (name, keywords, env_key, litellm_prefix, skip_prefixes, gateway detection, model overrides), model-name keyword matching, gateway auto-detection by API key prefix or base URL |
| P0-14 | **Telegram Channel** | `channels/telegram.py` | Long polling via Telegram Bot API, markdown-to-HTML conversion, /start /reset /help commands, media download (photos, voice, audio, documents), typing indicators, voice transcription via Groq Whisper, proxy support, session reset |
| P0-15 | **Discord Channel** | `channels/discord.py` | WebSocket Gateway (raw implementation, not library-dependent), IDENTIFY/HEARTBEAT, MESSAGE_CREATE handling, REST API for sending, rate limit handling with retry, typing indicators, attachment download |

### P1 -- Should Have

| # | Feature | nanobot Source | Description |
|---|---------|---------------|-------------|
| P1-1 | **WhatsApp Channel** | `channels/whatsapp.py` | WebSocket bridge to Node.js Baileys process, QR code login flow, message/status/error handling, reconnection logic |
| P1-2 | **Feishu Channel** | `channels/feishu.py` | WebSocket long connection via lark-oapi, message deduplication (ordered cache, 1000 cap), reaction indicators, interactive card messages with markdown + table rendering, thread-safe async bridge |
| P1-3 | **Slack Channel** | `channels/slack.py` | Socket Mode via slack-sdk, app_mention + message events, DM/group policy (open/mention/allowlist), bot mention stripping, thread-based replies, :eyes: reaction |
| P1-4 | **DingTalk Channel** | `channels/dingtalk.py` | Stream Mode via dingtalk-stream SDK, OAuth2 access token management with auto-refresh, batch send API, markdown message format |
| P1-5 | **Email Channel** | `channels/email.py` | IMAP polling (SSL/non-SSL), SMTP sending (TLS/SSL), consent gate, HTML-to-text extraction, In-Reply-To threading, subject prefix, date-range fetch for historical queries, UID-based deduplication |
| P1-6 | **Mochat Channel** | `channels/mochat.py` | Socket.IO with msgpack support, HTTP polling fallback, reply delay modes, mention handling, per-group rules, cursor-based message tracking |
| P1-7 | **QQ Channel** | `channels/qq.py` | botpy SDK, WebSocket, C2C (private) messages, sandbox mode support |
| P1-8 | **All 12 LLM Providers** | `providers/registry.py` | OpenRouter, Anthropic, OpenAI, DeepSeek, Gemini, Zhipu, DashScope (Qwen), Moonshot (Kimi), MiniMax, AiHubMix, vLLM/local, Groq -- each with correct env var setup, model prefix routing, gateway detection, per-model overrides |
| P1-9 | **Cron Service** | `cron/service.py`, `cron/types.py` | Persistent job store (JSON), three schedule types (at/every/cron), croniter-based cron expression evaluation, timer-based wakeup, job execution through agent loop, result delivery to channels, enable/disable/delete jobs |
| P1-10 | **Heartbeat Service** | `heartbeat/service.py` | Periodic agent wake-up (default 30 min), reads HEARTBEAT.md from workspace, skips if no actionable content, HEARTBEAT_OK token detection |
| P1-11 | **Memory System** | `agent/memory.py` | Long-term memory (MEMORY.md), daily notes (YYYY-MM-DD.md), recent memory retrieval (last N days), memory context injection into system prompt |
| P1-12 | **Skills System** | `agent/skills.py` | Workspace skills + built-in skills, YAML frontmatter metadata, requirement checking (bins, env vars), progressive loading (summary in prompt, full content via read_file), always-on skills |
| P1-13 | **Voice Transcription** | `providers/transcription.py` | Groq Whisper API integration (whisper-large-v3), audio file upload, used by Telegram channel for voice/audio messages |
| P1-14 | **Cron CLI Commands** | `cli/commands.py` | `klyntbot cron add/list/remove/enable/run` with --name, --message, --every, --cron, --at, --deliver, --to, --channel options |

### P2 -- Nice to Have

| # | Feature | nanobot Source | Description |
|---|---------|---------------|-------------|
| P2-1 | **Subagent Spawning** | `agent/subagent.py`, `agent/tools/spawn.py` | Background task execution with isolated context, limited tool set (no message/spawn tools), 15-iteration cap, result announcement via system message back to main agent |
| P2-2 | **Onboarding Wizard** | `cli/commands.py` (`onboard`) | Interactive setup: create config, workspace, bootstrap files (AGENTS.md, SOUL.md, USER.md), memory directory, skills directory |
| P2-3 | **WhatsApp Bridge** | `bridge/` (Node.js) | Separate Node.js process using @whiskeysockets/baileys for WhatsApp Web protocol, QR auth, WebSocket IPC |
| P2-4 | **Built-in Skills** | `skills/` directory | Bundled skills: cron, github, weather, skill-creator, summarize, tmux (each with SKILL.md) |
| P2-5 | **Channel Status CLI** | `cli/commands.py` | `klyntbot channels status` -- table showing all channels with enabled/config status |
| P2-6 | **Docker Support** | `Dockerfile` | Container image for gateway deployment |

---

## 5. Detailed Feature Specifications

### 5.1 Core Agent Loop (P0-1)

The agent loop is the central processing engine. It follows a strict cycle:

```
InboundMessage -> Session Lookup -> Context Build -> LLM Call -> Tool Execution -> Response
```

**Behavior:**
1. Consumes messages from the inbound message bus queue (async, 1s poll timeout)
2. Looks up or creates a session using `channel:chat_id` as key
3. Sets tool contexts (message tool, spawn tool, cron tool get current channel/chat_id)
4. Builds messages array: system prompt + history (up to 50 messages) + current user message
5. Calls LLM with messages + tool definitions
6. If LLM returns tool_calls: execute each tool, add results to messages, loop (up to `max_iterations`)
7. If LLM returns plain content: that's the final response
8. Saves user message and assistant response to session
9. Publishes OutboundMessage to the bus

**System Messages:**
- Messages from channel="system" (subagent results) are routed through `_process_system_message`
- The `chat_id` field contains `"origin_channel:origin_chat_id"` for routing back
- System messages are logged with `[System: sender_id]` prefix in session history

**Direct Processing:**
- `process_direct()` method for CLI and cron usage -- wraps content in InboundMessage and processes

**Error Handling:**
- On exception during processing, sends error message back to the originating channel
- Tool execution errors are caught and returned as error strings (not exceptions)

### 5.2 CLI Interface (P0-2)

**Commands:**

| Command | Description |
|---------|-------------|
| `klyntbot onboard` | Initialize config and workspace with templates |
| `klyntbot agent -m "msg"` | Single message mode with thinking spinner |
| `klyntbot agent` | Interactive REPL with prompt_toolkit (history, paste support) |
| `klyntbot agent --no-markdown` | Plain text output (no Rich rendering) |
| `klyntbot agent --logs` | Show runtime logs during chat |
| `klyntbot agent --session ID` | Specify session ID (default: `cli:default`) |
| `klyntbot gateway` | Start gateway with all services |
| `klyntbot gateway --port N` | Custom gateway port (default: 18790) |
| `klyntbot gateway --verbose` | Enable debug logging |
| `klyntbot status` | Show config path, workspace, model, provider API key status |
| `klyntbot channels status` | Table of channel enabled/config status |
| `klyntbot channels login` | WhatsApp QR code link flow |
| `klyntbot cron list` | List scheduled jobs (--all for disabled) |
| `klyntbot cron add` | Add job with schedule options |
| `klyntbot cron remove <id>` | Remove a job |
| `klyntbot cron enable <id>` | Enable/disable a job |
| `klyntbot cron run <id>` | Manually trigger a job |
| `klyntbot --version` | Show version |

**Interactive Mode:**
- Uses prompt_toolkit (Rust equivalent: `rustyline` or custom) for line editing, history, paste
- Exit commands: `exit`, `quit`, `/exit`, `/quit`, `:q`, Ctrl+D, Ctrl+C
- Terminal state save/restore for clean exit
- Input flush between responses (discard keystrokes during generation)
- File-based command history at `~/.klyntbot/history/cli_history`

### 5.3 Message Bus (P0-3)

Two async MPMC queues:
- **Inbound** (`InboundMessage`): channels -> agent
- **Outbound** (`OutboundMessage`): agent -> channels

**InboundMessage fields:**
```
channel: String      // "telegram", "discord", "whatsapp", etc.
sender_id: String    // User identifier
chat_id: String      // Chat/channel identifier
content: String      // Message text
timestamp: DateTime  // Auto-set on creation
media: Vec<String>   // Local file paths for downloaded media
metadata: HashMap    // Channel-specific data (message_id, thread_ts, etc.)
```

Derived property: `session_key = "{channel}:{chat_id}"`

**OutboundMessage fields:**
```
channel: String
chat_id: String
content: String
reply_to: Option<String>   // Message ID to reply to
media: Vec<String>         // Media file paths
metadata: HashMap          // Pass-through for channel needs (e.g. Slack thread_ts)
```

**Dispatch:**
- ChannelManager runs outbound dispatcher as background task
- Matches `msg.channel` to registered channel, calls `channel.send(msg)`
- Unknown channels are logged as warnings

### 5.4 Configuration System (P0-4)

**File location**: `~/.klyntbot/config.json` (also reads `~/.nanobot/config.json` for migration)

**JSON format** (camelCase in file, snake_case internally):

```json
{
  "agents": {
    "defaults": {
      "workspace": "~/.klyntbot/workspace",
      "model": "anthropic/claude-opus-4-5",
      "maxTokens": 8192,
      "temperature": 0.7,
      "maxToolIterations": 20
    }
  },
  "providers": {
    "openrouter": { "apiKey": "sk-or-...", "apiBase": null, "extraHeaders": null },
    "anthropic": { "apiKey": "", "apiBase": null },
    "openai": { "apiKey": "" },
    "deepseek": { "apiKey": "" },
    "groq": { "apiKey": "" },
    "gemini": { "apiKey": "" },
    "zhipu": { "apiKey": "" },
    "dashscope": { "apiKey": "" },
    "moonshot": { "apiKey": "" },
    "minimax": { "apiKey": "" },
    "aihubmix": { "apiKey": "", "apiBase": null, "extraHeaders": null },
    "vllm": { "apiKey": "dummy", "apiBase": "http://localhost:8000/v1" }
  },
  "channels": {
    "telegram": { "enabled": false, "token": "", "allowFrom": [], "proxy": null },
    "discord": { "enabled": false, "token": "", "allowFrom": [], "gatewayUrl": "wss://gateway.discord.gg/?v=10&encoding=json", "intents": 37377 },
    "whatsapp": { "enabled": false, "bridgeUrl": "ws://localhost:3001", "allowFrom": [] },
    "feishu": { "enabled": false, "appId": "", "appSecret": "", "encryptKey": "", "verificationToken": "", "allowFrom": [] },
    "dingtalk": { "enabled": false, "clientId": "", "clientSecret": "", "allowFrom": [] },
    "slack": { "enabled": false, "mode": "socket", "botToken": "", "appToken": "", "groupPolicy": "mention", "dm": { "enabled": true, "policy": "open" } },
    "email": { "enabled": false, "consentGranted": false, "imapHost": "", "smtpHost": "", "..." : "..." },
    "mochat": { "enabled": false, "baseUrl": "https://mochat.io", "clawToken": "", "..." : "..." },
    "qq": { "enabled": false, "appId": "", "secret": "", "allowFrom": [] }
  },
  "gateway": { "host": "0.0.0.0", "port": 18790 },
  "tools": {
    "web": { "search": { "apiKey": "", "maxResults": 5 } },
    "exec": { "timeout": 60 },
    "restrictToWorkspace": false
  }
}
```

**Provider Matching Logic:**
1. Match by model-name keyword (registry order = priority)
2. Fallback: first provider with non-empty `apiKey`
3. Gateway detection: by API key prefix (e.g. `sk-or-` -> OpenRouter) or base URL keyword

**Environment Variable Override:**
- Prefix: `KLYNTBOT_` with `__` as nested delimiter
- Example: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4`

**Config Migration:**
- `tools.exec.restrictToWorkspace` -> `tools.restrictToWorkspace` (legacy migration)

### 5.5 Session Management (P0-5)

**Storage format**: JSONL files at `~/.klyntbot/sessions/{safe_key}.jsonl`

```jsonl
{"_type":"metadata","created_at":"2026-02-11T10:00:00","updated_at":"2026-02-11T10:05:00","metadata":{}}
{"role":"user","content":"Hello","timestamp":"2026-02-11T10:00:00"}
{"role":"assistant","content":"Hi there!","timestamp":"2026-02-11T10:00:01"}
```

**Session structure:**
- `key`: String (`channel:chat_id`)
- `messages`: Vec of `{role, content, timestamp}`
- `created_at`, `updated_at`: DateTime
- `metadata`: HashMap

**Behavior:**
- In-memory LRU cache of active sessions
- `get_or_create(key)`: check cache -> load from disk -> create new
- `get_history(max_messages=50)`: returns last N messages in `{role, content}` format for LLM
- `save(session)`: writes full JSONL (metadata line + all messages)
- `delete(key)`: removes from cache and disk
- `list_sessions()`: glob `*.jsonl`, read metadata lines, sort by updated_at desc

### 5.6 Context Builder (P0-6)

Assembles the system prompt from multiple sources:

```
Identity Section (runtime info)
  + Bootstrap Files (AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md)
  + Memory (long-term MEMORY.md + today's notes)
  + Always-on Skills (full content)
  + Available Skills (summary with paths for lazy loading)
  + Current Session info (channel, chat_id)
```

Sections are joined with `\n\n---\n\n`.

**Identity section** includes:
- Agent name and capabilities description
- Current date/time (formatted: `YYYY-MM-DD HH:MM (Weekday)`)
- Runtime info (OS, architecture)
- Workspace path and key file locations
- Important behavioral instructions (when to use message tool vs direct response)

**Media handling:**
- Image attachments are base64-encoded inline as `image_url` content parts
- Non-image media referenced by path

**Reasoning content:**
- Supports `reasoning_content` field from thinking models (DeepSeek-R1, Kimi)
- Preserved in message history to prevent model rejection

### 5.7 Tool System (P0-7 through P0-11)

#### Tool Abstraction (base.py)

Every tool implements:
```rust
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> JsonSchema;
    async fn execute(&self, params: Value) -> String;
    fn validate_params(&self, params: &Value) -> Vec<String>;  // JSON Schema validation
    fn to_schema(&self) -> Value;  // OpenAI function format
}
```

Parameter validation supports: type checking, enum, min/max, minLength/maxLength, required fields, nested objects, arrays.

#### Tool Registry (registry.py)

```rust
struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

Methods: `register()`, `unregister()`, `get()`, `has()`, `get_definitions()` (all tools in OpenAI format), `execute(name, params)`.

#### Filesystem Tools (filesystem.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `read_file` | `path: String` | Read UTF-8 file content, path resolution with optional workspace restriction |
| `write_file` | `path: String, content: String` | Write content, create parent dirs, report bytes written |
| `edit_file` | `path: String, old_text: String, new_text: String` | Find-and-replace with uniqueness check (warns if >1 match, errors if 0) |
| `list_dir` | `path: String` | List directory contents with type indicators |

All filesystem tools enforce `allowed_dir` restriction when `restrictToWorkspace` is enabled.

#### Shell Tool (shell.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `exec` | `command: String, working_dir?: String` | Execute shell command via subprocess |

**Safety guards:**
- Deny patterns (regex): `rm -rf`, `del /f`, `rmdir /s`, `format/mkfs/diskpart`, `dd if=`, disk writes, `shutdown/reboot/poweroff`, fork bombs
- Allow patterns (optional): if set, only matching commands are permitted
- Workspace restriction: blocks path traversal (`../`), validates absolute paths are within workspace
- Output truncation at 10,000 characters
- Configurable timeout (default 60s)

#### Web Tools (web.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `web_search` | `query: String, count?: i32(1-10)` | Brave Search API, returns titles/URLs/snippets |
| `web_fetch` | `url: String, extractMode?: "markdown"\|"text", maxChars?: i32` | HTTP GET, Readability extraction, JSON/HTML/raw handling |

**web_fetch details:**
- URL validation: must be http/https with valid domain
- Max 5 redirects (DoS prevention)
- JSON content returned formatted
- HTML: Readability extraction -> markdown conversion (links, headings, lists, block elements)
- Output truncation at configurable maxChars (default 50,000)
- Response includes: url, finalUrl, status, extractor, truncated flag, length, text

#### Message Tool (message.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `message` | `content: String, channel?: String, chat_id?: String` | Send message via bus to specific channel/chat |

Context-aware: defaults to current session's channel and chat_id.

#### Spawn Tool (spawn.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `spawn` | `task: String, label?: String` | Spawn background subagent with isolated context |

#### Cron Tool (cron.py)

| Tool | Parameters | Behavior |
|------|-----------|----------|
| `cron` | `action: "add"\|"list"\|"remove", message?: String, every_seconds?: i32, cron_expr?: String, job_id?: String` | Schedule/list/remove recurring tasks |

### 5.8 LLM Provider System (P0-12, P0-13, P1-8)

#### Provider Trait

```rust
trait LLMProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDef]>,
        model: Option<&str>,
        max_tokens: u32,     // default 4096
        temperature: f32,    // default 0.7
    ) -> LLMResponse;

    fn default_model(&self) -> &str;
}
```

#### LLMResponse

```rust
struct LLMResponse {
    content: Option<String>,
    tool_calls: Vec<ToolCallRequest>,
    finish_reason: String,
    usage: Usage,                     // prompt_tokens, completion_tokens, total_tokens
    reasoning_content: Option<String>, // For thinking models
}

struct ToolCallRequest {
    id: String,
    name: String,
    arguments: Value,  // Parsed from JSON string
}
```

#### Provider Registry (12 providers)

| Name | Keywords | Env Key | Prefix | Type |
|------|----------|---------|--------|------|
| `openrouter` | `openrouter` | `OPENROUTER_API_KEY` | `openrouter/` | Gateway (detect by `sk-or-` prefix) |
| `aihubmix` | `aihubmix` | `OPENAI_API_KEY` | `openai/` | Gateway (detect by base URL, strips model prefix) |
| `anthropic` | `anthropic`, `claude` | `ANTHROPIC_API_KEY` | (none) | Standard |
| `openai` | `openai`, `gpt` | `OPENAI_API_KEY` | (none) | Standard |
| `deepseek` | `deepseek` | `DEEPSEEK_API_KEY` | `deepseek/` | Standard |
| `gemini` | `gemini` | `GEMINI_API_KEY` | `gemini/` | Standard |
| `zhipu` | `zhipu`, `glm`, `zai` | `ZAI_API_KEY` | `zai/` | Standard (also sets `ZHIPUAI_API_KEY`) |
| `dashscope` | `qwen`, `dashscope` | `DASHSCOPE_API_KEY` | `dashscope/` | Standard |
| `moonshot` | `moonshot`, `kimi` | `MOONSHOT_API_KEY` | `moonshot/` | Standard (Kimi K2.5: temperature>=1.0) |
| `minimax` | `minimax` | `MINIMAX_API_KEY` | `minimax/` | Standard |
| `vllm` | `vllm` | `HOSTED_VLLM_API_KEY` | `hosted_vllm/` | Local |
| `groq` | `groq` | `GROQ_API_KEY` | `groq/` | Auxiliary (also used for transcription) |

**Model resolution logic:**
1. If gateway mode: apply gateway prefix, optionally strip existing prefix
2. If standard mode: match by keyword, apply litellm_prefix (skip if already prefixed)

**Note for Rust implementation**: Replace LiteLLM (Python) with direct HTTP client. All providers use OpenAI-compatible `/v1/chat/completions` API format. The provider registry handles model name prefixing that LiteLLM expects -- in Rust we route directly to the correct API base URL.

### 5.9 Channel System (P0-14, P0-15, P1-1 through P1-7)

#### Channel Trait

```rust
trait Channel {
    fn name(&self) -> &str;
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn send(&self, msg: OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;
}
```

**BaseChannel behavior:**
- `is_allowed()`: if `allow_from` is empty, allow all; otherwise check exact match and `|`-separated compound IDs
- `_handle_message()`: check permissions, construct InboundMessage, publish to bus

#### Channel Manager

- Initializes channels based on config (lazy import pattern for optional deps)
- Starts all channels concurrently (`tokio::join!`)
- Runs outbound dispatcher as background task
- Routes outbound messages to matching channel by name

#### Channel Implementations

**Telegram** (P0-14):
- Transport: Long polling via Telegram Bot API
- Features: /start, /reset (clear session), /help commands
- Media: Photo, voice, audio, document download to `~/.klyntbot/media/`
- Voice: Transcription via Groq Whisper (if configured)
- Formatting: Markdown -> Telegram HTML conversion (code blocks, bold, italic, strikethrough, links, lists)
- Typing: Continuous "typing..." indicator while processing
- Auth: Token-based, configurable proxy (HTTP/SOCKS5)
- Sender ID: `{numeric_id}|{username}` for allowlist compatibility

**Discord** (P0-15):
- Transport: Direct WebSocket to Discord Gateway (v10)
- Protocol: HELLO -> IDENTIFY -> heartbeat loop -> MESSAGE_CREATE events
- Features: Message replies with reference, attachment download (20MB limit)
- Sending: REST API POST to `/channels/{id}/messages` with retry on 429 (rate limit)
- Typing: Periodic POST to typing endpoint (8s interval)
- Reconnection: Auto-reconnect on gateway RECONNECT (op 7) or INVALID_SESSION (op 9)

**WhatsApp** (P1-1):
- Transport: WebSocket to Node.js bridge (Baileys)
- Protocol: JSON messages with type field (message/status/qr/error)
- Sender ID: Phone number or LID format
- Bridge URL: configurable (default `ws://localhost:3001`)

**Feishu** (P1-2):
- Transport: WebSocket long connection via lark-oapi SDK
- Features: Message dedup (OrderedDict cache, 1000 cap), emoji reactions (THUMBSUP on receive)
- Sending: Interactive card messages with markdown + native table rendering
- Threading: Sync callback from WS thread -> async bridge to tokio runtime
- Reply routing: p2p -> sender open_id, group -> chat_id

**Slack** (P1-3):
- Transport: Socket Mode (WebSocket via slack-sdk)
- Events: `message`, `app_mention`
- Policies: group_policy (mention/open/allowlist), DM policy (open/allowlist), DM enabled flag
- Features: Bot mention stripping, thread-based replies in channels, :eyes: reaction
- Dedup: Skip message events that duplicate app_mention events

**DingTalk** (P1-4):
- Transport: Stream Mode (WebSocket via dingtalk-stream SDK)
- Auth: OAuth2 access token with 60s-early refresh
- Sending: Robot batch send API with sampleMarkdown format
- Currently: Private chat only (group messages get private replies)

**Email** (P1-5):
- Inbound: IMAP polling (SSL/non-SSL), configurable interval (default 30s)
- Outbound: SMTP (TLS/SSL), auto-reply with In-Reply-To/References headers
- Safety: `consent_granted` must be true to enable mailbox access
- Features: HTML-to-text extraction, subject threading, date-range queries, UID deduplication (100K cap)
- Config: from_address, mark_seen, max_body_chars (12000), auto_reply_enabled

**Mochat** (P1-6):
- Transport: Socket.IO (msgpack or JSON), HTTP polling fallback
- Features: Reply delay modes, mention configuration, per-group rules, cursor-based tracking
- Auth: claw_token, agent_user_id

**QQ** (P1-7):
- Transport: botpy SDK with WebSocket
- Features: C2C (private) messages, sandbox mode
- Auth: AppID + AppSecret from QQ Open Platform

### 5.10 Cron Service (P1-9)

**Schedule types:**
- `at`: One-time execution at specific timestamp (ms)
- `every`: Recurring at fixed interval (ms)
- `cron`: Cron expression via croniter (e.g. `0 9 * * *`)

**Job lifecycle:**
1. Created with schedule, message, optional delivery config
2. Service computes next_run_at_ms
3. Timer task sleeps until next job is due
4. On timer tick: find all due jobs, execute each through agent loop
5. If `deliver=true`: publish OutboundMessage to specified channel/chat_id
6. Update state (last_run_at_ms, last_status, last_error)
7. For `at` jobs: disable after run (or delete if `delete_after_run=true`)
8. For recurring: recompute next_run_at_ms
9. Persist to JSON store

**Storage**: `~/.klyntbot/cron/jobs.json`

```json
{
  "version": 1,
  "jobs": [{
    "id": "abc12345",
    "name": "daily check",
    "enabled": true,
    "schedule": { "kind": "cron", "expr": "0 9 * * *", "tz": null },
    "payload": { "kind": "agent_turn", "message": "Good morning!", "deliver": true, "channel": "telegram", "to": "123456" },
    "state": { "nextRunAtMs": 1707638400000, "lastRunAtMs": null, "lastStatus": null, "lastError": null },
    "createdAtMs": 1707552000000,
    "updatedAtMs": 1707552000000,
    "deleteAfterRun": false
  }]
}
```

### 5.11 Heartbeat Service (P1-10)

- Interval: 30 minutes (configurable)
- On tick: reads `HEARTBEAT.md` from workspace
- If file is empty or contains only headers/checkboxes: skip (no agent call)
- If actionable content: send `HEARTBEAT_PROMPT` to agent via `process_direct()`
- If agent responds with "HEARTBEAT_OK": log and continue
- Otherwise: log that task was completed

### 5.12 Memory System (P1-11)

**Storage**: `{workspace}/memory/`
- `MEMORY.md`: Long-term memory (persistent facts, preferences, notes)
- `YYYY-MM-DD.md`: Daily notes (auto-dated)

**API:**
- `get_memory_context()`: returns combined long-term + today's notes for system prompt
- `read_today()`, `append_today()`: daily note management
- `read_long_term()`, `write_long_term()`: MEMORY.md management
- `get_recent_memories(days=7)`: combine last N days of daily notes
- `list_memory_files()`: glob for date-named files, sorted newest first

### 5.13 Skills System (P1-12)

**Skill structure:**
```
skills/
  github/
    SKILL.md    # Instructions for the agent
  weather/
    SKILL.md
```

**Sources** (priority order):
1. Workspace skills: `{workspace}/skills/*/SKILL.md`
2. Built-in skills: bundled in binary

**Frontmatter (YAML):**
```yaml
---
description: "Search GitHub repositories and issues"
metadata: '{"nanobot": {"requires": {"bins": ["gh"]}, "always": false}}'
---
```

**Progressive loading:**
- `always=true` skills: full content injected into system prompt
- Other skills: XML summary in system prompt with name, description, path, availability
- Agent reads full skill content via `read_file` tool when needed

**Requirement checking:**
- `bins`: checks for binary in PATH (e.g. `gh`, `tmux`)
- `env`: checks for environment variable

**Built-in skills**: cron, github, weather, skill-creator, summarize, tmux

### 5.14 Subagent System (P2-1)

- Spawned via `spawn` tool with task description and optional label
- Runs as async background task with isolated context
- Limited tool set: read_file, write_file, list_dir, exec, web_search, web_fetch (no message, no spawn, no cron)
- 15-iteration cap (vs 20 for main agent)
- Focused system prompt explaining constraints
- On completion: announces result via system InboundMessage routed back to original channel/chat
- Announcement prompt asks main agent to summarize naturally for user

### 5.15 Voice Transcription (P1-13)

- Provider: Groq Whisper API (whisper-large-v3 model)
- Endpoint: `https://api.groq.com/openai/v1/audio/transcriptions`
- Input: multipart file upload
- Output: transcribed text string
- Used by: Telegram channel for voice/audio messages
- Graceful degradation: if no Groq API key, returns empty string

---

## 6. Non-Functional Requirements

### 6.1 Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Idle memory | <10 MB RSS | `ps -o rss` with gateway running, no active requests |
| Peak memory (single request) | <50 MB | During agent loop with tool execution |
| Startup time | <100 ms | Time from process start to "gateway started" log |
| Message processing overhead | <5 ms | Time from bus receive to LLM API call (excludes network) |
| Binary size | <20 MB | Stripped release binary |
| Concurrent channels | All 9 simultaneously | All channels connected and processing |

### 6.2 Reliability

- **Graceful degradation**: Failed channels don't crash the gateway
- **Reconnection**: All WebSocket channels auto-reconnect with exponential backoff
- **Error isolation**: Tool execution errors return error strings, never panic
- **Session durability**: Sessions persist across restarts via JSONL files
- **Cron persistence**: Jobs survive process restart via JSON store

### 6.3 Security

- **Path traversal protection**: All file tools validate resolved paths
- **Command injection prevention**: Shell tool has deny-pattern regex guard
- **Workspace sandboxing**: Optional `restrictToWorkspace` mode
- **Access control**: Per-channel `allowFrom` lists
- **URL validation**: web_fetch validates http(s) scheme and domain presence
- **Redirect limiting**: Max 5 redirects on web_fetch
- **Output truncation**: Shell (10KB), web_fetch (50KB) to prevent memory exhaustion
- **No secrets in logs**: API keys masked in status output
- **Config file permissions**: Recommend `chmod 600`

### 6.4 Compatibility

- **Config format**: 100% compatible with nanobot's `config.json` (camelCase JSON)
- **Session format**: Compatible JSONL format (can read nanobot sessions)
- **Workspace layout**: Same directory structure (memory/, skills/, bootstrap files)
- **API contracts**: Same tool schemas, same LLM message format (OpenAI standard)
- **CLI**: Same command names and flags

### 6.5 Build & Distribution

- **Targets**: Linux x86_64, Linux aarch64, macOS x86_64, macOS aarch64
- **Linking**: Static (musl on Linux, native on macOS)
- **CI**: GitHub Actions with cross-compilation
- **Install methods**: Binary download, `cargo install klyntbot`, Homebrew formula
- **Docker**: Multi-stage build, scratch/alpine base

---

## 7. User Stories

### US-1: First-time Setup
> As a user, I want to download a single binary and run `klyntbot onboard` to get a working AI assistant in under 2 minutes, so I can start chatting immediately.

### US-2: CLI Chat
> As a developer, I want to run `klyntbot agent -m "explain this error"` from my terminal and get an AI response, so I can quickly get help while coding.

### US-3: Interactive Session
> As a user, I want an interactive chat mode with command history and paste support, so I can have natural multi-turn conversations with my assistant.

### US-4: Telegram Bot
> As a user, I want to configure a Telegram bot token and chat with my AI assistant from my phone, so I can get help anywhere.

### US-5: Multi-Channel
> As a user, I want to enable Telegram, Discord, and Slack simultaneously and have the same assistant respond on all channels with separate conversation histories.

### US-6: Web Research
> As a user, I want my assistant to search the web and fetch page content, so it can answer questions about current events and documentation.

### US-7: File Operations
> As a user, I want my assistant to read, write, and edit files in my workspace, so it can help me with coding tasks and note-taking.

### US-8: Scheduled Tasks
> As a user, I want to schedule daily reminders like "Check my email and summarize important messages", so my assistant works proactively.

### US-9: Memory
> As a user, I want my assistant to remember important information across sessions, so it doesn't forget my preferences and past conversations.

### US-10: Custom Skills
> As a user, I want to add custom skill files that teach my assistant new capabilities (like using GitHub or checking weather), so I can extend its functionality.

### US-11: Workspace Security
> As a system administrator, I want to enable `restrictToWorkspace` mode, so the assistant can only access files and run commands within its designated workspace.

### US-12: Background Tasks
> As a user, I want my assistant to spawn background tasks for complex operations (like researching a topic), so I can continue chatting while it works.

### US-13: Voice Messages
> As a Telegram user, I want to send voice messages that get transcribed automatically, so I can interact with my assistant hands-free.

### US-14: Session Reset
> As a user, I want to reset my conversation history with `/reset` on Telegram, so I can start a fresh conversation when the context gets too long.

### US-15: Multi-Provider
> As a user, I want to configure multiple LLM providers and switch between models, so I can use the best model for each task and have fallback options.

### US-16: Heartbeat Tasks
> As a user, I want to put tasks in HEARTBEAT.md and have my assistant check them every 30 minutes, so time-sensitive tasks get handled automatically.

### US-17: Email Assistant
> As a user, I want my assistant to poll my email inbox and optionally reply to messages, so it acts as a personal email assistant.

### US-18: Local LLM
> As a privacy-conscious user, I want to point klyntbot at my local vLLM server, so all my conversations stay on my machine.

---

## 8. Data Models

### 8.1 Message Types

```rust
// Inbound: channel -> agent
struct InboundMessage {
    channel: String,
    sender_id: String,
    chat_id: String,
    content: String,
    timestamp: DateTime<Utc>,
    media: Vec<String>,
    metadata: HashMap<String, Value>,
}

// Outbound: agent -> channel
struct OutboundMessage {
    channel: String,
    chat_id: String,
    content: String,
    reply_to: Option<String>,
    media: Vec<String>,
    metadata: HashMap<String, Value>,
}
```

### 8.2 Session

```rust
struct Session {
    key: String,
    messages: Vec<SessionMessage>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    metadata: HashMap<String, Value>,
}

struct SessionMessage {
    role: String,        // "user", "assistant", "system"
    content: String,
    timestamp: DateTime<Utc>,
}
```

### 8.3 LLM Types

```rust
struct LLMResponse {
    content: Option<String>,
    tool_calls: Vec<ToolCallRequest>,
    finish_reason: String,
    usage: Usage,
    reasoning_content: Option<String>,
}

struct ToolCallRequest {
    id: String,
    name: String,
    arguments: Value,
}

struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
```

### 8.4 Cron Types

```rust
enum ScheduleKind { At, Every, Cron }

struct CronSchedule {
    kind: ScheduleKind,
    at_ms: Option<i64>,
    every_ms: Option<i64>,
    expr: Option<String>,
    tz: Option<String>,
}

struct CronPayload {
    kind: String,           // "agent_turn" or "system_event"
    message: String,
    deliver: bool,
    channel: Option<String>,
    to: Option<String>,
}

struct CronJobState {
    next_run_at_ms: Option<i64>,
    last_run_at_ms: Option<i64>,
    last_status: Option<String>,
    last_error: Option<String>,
}

struct CronJob {
    id: String,
    name: String,
    enabled: bool,
    schedule: CronSchedule,
    payload: CronPayload,
    state: CronJobState,
    created_at_ms: i64,
    updated_at_ms: i64,
    delete_after_run: bool,
}
```

### 8.5 Provider Spec

```rust
struct ProviderSpec {
    name: &'static str,
    keywords: &'static [&'static str],
    env_key: &'static str,
    display_name: &'static str,
    litellm_prefix: &'static str,
    skip_prefixes: &'static [&'static str],
    env_extras: &'static [(&'static str, &'static str)],
    is_gateway: bool,
    is_local: bool,
    detect_by_key_prefix: &'static str,
    detect_by_base_keyword: &'static str,
    default_api_base: &'static str,
    strip_model_prefix: bool,
    model_overrides: &'static [(&'static str, &'static [(&'static str, Value)])],
}
```

### 8.6 Configuration Types

See Section 5.4 for the complete config schema. Key nested types:

- `Config` (root): agents, channels, providers, gateway, tools
- `AgentDefaults`: workspace, model, max_tokens, temperature, max_tool_iterations
- `ProviderConfig`: api_key, api_base, extra_headers
- `ChannelsConfig`: telegram, discord, whatsapp, feishu, dingtalk, slack, email, mochat, qq
- `ToolsConfig`: web (search), exec (timeout), restrict_to_workspace

---

## 9. Configuration Schema

See Section 5.4 for the complete JSON schema with all fields, defaults, and descriptions.

**Key design decisions for Rust:**
- Use `serde` with `#[serde(rename_all = "camelCase")]` for JSON compatibility
- Implement `Default` for all config types to match nanobot defaults
- Support environment variable override via `envy` or custom parser
- Config file discovery: `~/.klyntbot/config.json` -> `~/.nanobot/config.json` -> defaults

---

## 10. Tool Schemas

### 10.1 read_file
```json
{
  "type": "function",
  "function": {
    "name": "read_file",
    "description": "Read the contents of a file at the given path.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string", "description": "The file path to read" }
      },
      "required": ["path"]
    }
  }
}
```

### 10.2 write_file
```json
{
  "type": "function",
  "function": {
    "name": "write_file",
    "description": "Write content to a file at the given path. Creates parent directories if needed.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string", "description": "The file path to write to" },
        "content": { "type": "string", "description": "The content to write" }
      },
      "required": ["path", "content"]
    }
  }
}
```

### 10.3 edit_file
```json
{
  "type": "function",
  "function": {
    "name": "edit_file",
    "description": "Edit a file by replacing old_text with new_text. The old_text must exist exactly in the file.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string", "description": "The file path to edit" },
        "old_text": { "type": "string", "description": "The exact text to find and replace" },
        "new_text": { "type": "string", "description": "The text to replace with" }
      },
      "required": ["path", "old_text", "new_text"]
    }
  }
}
```

### 10.4 list_dir
```json
{
  "type": "function",
  "function": {
    "name": "list_dir",
    "description": "List the contents of a directory.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string", "description": "The directory path to list" }
      },
      "required": ["path"]
    }
  }
}
```

### 10.5 exec
```json
{
  "type": "function",
  "function": {
    "name": "exec",
    "description": "Execute a shell command and return its output. Use with caution.",
    "parameters": {
      "type": "object",
      "properties": {
        "command": { "type": "string", "description": "The shell command to execute" },
        "working_dir": { "type": "string", "description": "Optional working directory for the command" }
      },
      "required": ["command"]
    }
  }
}
```

### 10.6 web_search
```json
{
  "type": "function",
  "function": {
    "name": "web_search",
    "description": "Search the web. Returns titles, URLs, and snippets.",
    "parameters": {
      "type": "object",
      "properties": {
        "query": { "type": "string", "description": "Search query" },
        "count": { "type": "integer", "description": "Results (1-10)", "minimum": 1, "maximum": 10 }
      },
      "required": ["query"]
    }
  }
}
```

### 10.7 web_fetch
```json
{
  "type": "function",
  "function": {
    "name": "web_fetch",
    "description": "Fetch URL and extract readable content (HTML -> markdown/text).",
    "parameters": {
      "type": "object",
      "properties": {
        "url": { "type": "string", "description": "URL to fetch" },
        "extractMode": { "type": "string", "enum": ["markdown", "text"], "default": "markdown" },
        "maxChars": { "type": "integer", "minimum": 100 }
      },
      "required": ["url"]
    }
  }
}
```

### 10.8 message
```json
{
  "type": "function",
  "function": {
    "name": "message",
    "description": "Send a message to the user. Use this when you want to communicate something.",
    "parameters": {
      "type": "object",
      "properties": {
        "content": { "type": "string", "description": "The message content to send" },
        "channel": { "type": "string", "description": "Optional: target channel (telegram, discord, etc.)" },
        "chat_id": { "type": "string", "description": "Optional: target chat/user ID" }
      },
      "required": ["content"]
    }
  }
}
```

### 10.9 spawn
```json
{
  "type": "function",
  "function": {
    "name": "spawn",
    "description": "Spawn a subagent to handle a task in the background. Use this for complex or time-consuming tasks that can run independently. The subagent will complete the task and report back when done.",
    "parameters": {
      "type": "object",
      "properties": {
        "task": { "type": "string", "description": "The task for the subagent to complete" },
        "label": { "type": "string", "description": "Optional short label for the task (for display)" }
      },
      "required": ["task"]
    }
  }
}
```

### 10.10 cron
```json
{
  "type": "function",
  "function": {
    "name": "cron",
    "description": "Schedule reminders and recurring tasks. Actions: add, list, remove.",
    "parameters": {
      "type": "object",
      "properties": {
        "action": { "type": "string", "enum": ["add", "list", "remove"], "description": "Action to perform" },
        "message": { "type": "string", "description": "Reminder message (for add)" },
        "every_seconds": { "type": "integer", "description": "Interval in seconds (for recurring tasks)" },
        "cron_expr": { "type": "string", "description": "Cron expression like '0 9 * * *' (for scheduled tasks)" },
        "job_id": { "type": "string", "description": "Job ID (for remove)" }
      },
      "required": ["action"]
    }
  }
}
```

---

## 11. Provider Interface Contract

Every LLM provider must support:

### Required

1. **chat()**: Send messages with optional tool definitions, receive text response and/or tool calls
2. **Model routing**: Accept model name string, route to correct API endpoint
3. **Tool calling**: Support OpenAI-format function calling (tool_choice: "auto")
4. **Streaming**: Not required for v1.0 (batch responses only)

### Request Format (OpenAI-compatible)

```json
{
  "model": "anthropic/claude-opus-4-5",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "...", "tool_calls": [...]},
    {"role": "tool", "tool_call_id": "...", "name": "...", "content": "..."}
  ],
  "tools": [{"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}],
  "tool_choice": "auto",
  "max_tokens": 4096,
  "temperature": 0.7
}
```

### Response Parsing

Must handle:
- `choices[0].message.content` (text response)
- `choices[0].message.tool_calls` (function calls with JSON string arguments)
- `choices[0].finish_reason`
- `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`
- `choices[0].message.reasoning_content` (optional, for thinking models)

### Authentication

- API key passed via `Authorization: Bearer {key}` header
- Optional custom base URL (for proxies, local servers)
- Optional extra headers (e.g. `APP-Code` for AiHubMix)

---

## 12. Channel Interface Contract

Every chat channel must implement:

### Required Methods

1. **start()**: Connect to platform, begin listening for messages (long-running)
2. **stop()**: Disconnect and cleanup resources
3. **send(OutboundMessage)**: Deliver a message to the specified chat_id

### Required Behavior

1. **Message handling**: Parse platform messages -> call `_handle_message()` -> publish to bus
2. **Access control**: Call `is_allowed(sender_id)` before processing
3. **Reconnection**: Auto-reconnect on connection loss (exponential backoff)
4. **Typing indicators**: Show "typing" while agent processes (channel-dependent)
5. **Error resilience**: Log errors, don't crash the channel

### InboundMessage Construction

Each channel must provide:
- `channel`: channel name string (e.g. "telegram")
- `sender_id`: unique user identifier (platform-specific)
- `chat_id`: unique chat/conversation identifier
- `content`: extracted text content
- `media`: list of downloaded file paths (optional)
- `metadata`: platform-specific data (message_id, thread info, etc.)

### OutboundMessage Handling

Each channel must:
- Extract `chat_id` to determine recipient
- Format `content` for the platform (markdown -> HTML for Telegram, etc.)
- Handle `reply_to` if supported
- Pass through `metadata` for platform-specific features (e.g. Slack thread_ts)

---

## 13. Migration Strategy

### 13.1 For Users

1. **Install klyntbot**: Download binary or `cargo install klyntbot`
2. **Config compatibility**: klyntbot reads `~/.nanobot/config.json` as fallback
3. **Copy config** (optional): `cp ~/.nanobot/config.json ~/.klyntbot/config.json`
4. **Workspace**: Same structure, same path (or configure new path)
5. **Sessions**: klyntbot reads nanobot's JSONL session format
6. **Cron jobs**: Same JSON format, same path
7. **Replace command**: `alias nanobot=klyntbot` or update scripts

### 13.2 Breaking Changes

| Area | nanobot | klyntbot | Migration |
|------|---------|----------|-----------|
| Binary name | `nanobot` | `klyntbot` | Alias or update scripts |
| Config path | `~/.nanobot/` | `~/.klyntbot/` (reads both) | Auto-fallback |
| Dependencies | pip/Python | None (static binary) | Just download |
| WhatsApp bridge | Bundled Node.js | Separate optional binary | Install separately if needed |
| LLM routing | LiteLLM library | Direct HTTP | Transparent (same API) |

### 13.3 What's Preserved

- All config field names and structure
- All CLI commands and flags
- All tool names and parameter schemas
- Session file format
- Workspace directory layout
- Bootstrap file names
- Skill file format
- Cron job format

### 13.4 What's Improved

- **10-100x faster startup**: No Python import chain
- **10-15x less memory**: No Python runtime, GC, or pip packages
- **Single binary**: No dependency management
- **True concurrency**: tokio vs asyncio (no GIL)
- **Static typing**: Compile-time safety vs runtime errors
- **Smaller attack surface**: No pip supply chain, fewer dependencies

---

## 14. Success Metrics

### 14.1 Performance (Measured in CI)

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| Startup time | <100 ms | `time klyntbot status` (cold start) |
| Idle RSS | <10 MB | `ps -o rss` after 60s idle gateway |
| Binary size | <20 MB | `ls -la target/release/klyntbot` |
| Agent overhead | <5 ms | Instrumented timing around bus->LLM call |

### 14.2 Functional (Test Suite)

| Metric | Target |
|--------|--------|
| Tool test coverage | 100% of tool schemas |
| Provider test coverage | All 12 providers with mock API |
| Channel test coverage | All 9 channels with mock connections |
| Config compatibility | nanobot config files parse correctly |
| Session compatibility | nanobot session files load correctly |
| CLI parity | All commands produce equivalent output |

### 14.3 Adoption

| Metric | Target (6 months) |
|--------|-------------------|
| GitHub stars | 500+ |
| Binary downloads | 1,000+ |
| Active users (self-reported) | 100+ |
| Community contributions | 10+ PRs merged |

---

## Appendix A: Workspace Directory Layout

```
~/.klyntbot/
├── config.json          # Main configuration
├── sessions/            # JSONL session files
│   ├── telegram_123456.jsonl
│   └── cli_default.jsonl
├── cron/
│   └── jobs.json        # Scheduled job store
├── media/               # Downloaded media files
│   └── abc123.jpg
├── history/
│   └── cli_history      # Interactive mode command history
└── workspace/           # Agent workspace
    ├── AGENTS.md         # Agent instructions
    ├── SOUL.md           # Personality definition
    ├── USER.md           # User information
    ├── TOOLS.md          # Tool usage guidelines
    ├── IDENTITY.md       # Identity overrides
    ├── HEARTBEAT.md      # Heartbeat task file
    ├── memory/
    │   ├── MEMORY.md     # Long-term memory
    │   └── 2026-02-11.md # Daily notes
    └── skills/
        └── custom-skill/
            └── SKILL.md
```

## Appendix B: Built-in Skills

| Skill | Description | Requirements |
|-------|-------------|-------------|
| `cron` | Natural language scheduling | (none) |
| `github` | GitHub repository operations | `gh` CLI |
| `weather` | Weather information | (none) |
| `skill-creator` | Create new skills | (none) |
| `summarize` | Summarize content | (none) |
| `tmux` | Terminal multiplexer integration | `tmux` |

## Appendix C: Security Model

### Threat Model

1. **Untrusted user input**: Mitigated by allowFrom lists, command deny patterns
2. **Path traversal**: Mitigated by path resolution and workspace restriction
3. **Command injection**: Mitigated by deny-pattern regex on shell tool
4. **Resource exhaustion**: Mitigated by timeouts, output truncation, iteration limits
5. **Credential exposure**: Config file permissions, no logging of API keys

### Defense Layers

1. **Channel layer**: allowFrom access control
2. **Tool layer**: Parameter validation, path restriction, command filtering
3. **Agent layer**: Max iteration limit, error isolation
4. **System layer**: File permissions, process isolation (recommended)

---

*End of PRD*
