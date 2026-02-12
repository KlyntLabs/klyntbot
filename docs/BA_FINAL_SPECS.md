# BA Final Specifications

> Business Analyst verification of all gaps from ACCEPTANCE_CRITERIA.md.
> Each gap has been independently verified against the actual Rust and Python source code.
> Date: 2026-02-12

---

## Verification Summary

| Gap ID | ACCEPTANCE_CRITERIA Status | BA Verified Status | Notes |
|--------|---------------------------|--------------------|-------|
| GAP-2.1 | RESOLVED | **CONFIRMED RESOLVED** | Tool context wiring works |
| GAP-2.2 | RESOLVED | **CONFIRMED RESOLVED** | Discord IDENTIFY works |
| GAP-2.3 | RESOLVED | **CONFIRMED RESOLVED** | Subagent has 6 tools |
| GAP-2.4 | RESOLVED | **CONFIRMED RESOLVED** | SpawnTool/CronTool wired |
| GAP-3.1a | RESOLVED | **CONFIRMED RESOLVED** | Telegram typing indicator |
| GAP-3.1b | RESOLVED | **CONFIRMED RESOLVED** | Telegram proxy |
| GAP-3.1c | RESOLVED | **CONFIRMED RESOLVED** | Telegram HTML fallback |
| GAP-3.1d | OPEN | **RECLASSIFIED: RESOLVED** | See below |
| GAP-3.2a | RESOLVED | **CONFIRMED RESOLVED** | Discord attachments |
| GAP-3.2b | RESOLVED | **CONFIRMED RESOLVED** | Discord typing |
| GAP-3.2c | RESOLVED | **CONFIRMED RESOLVED** | Discord shared WS |
| GAP-3.2d | PARTIAL | **CONFIRMED OPEN** | Hardcoded constants |
| GAP-3.3a | OPEN | **CONFIRMED OPEN** | 3 channel configs missing |
| GAP-3.3b | OPEN | **CONFIRMED OPEN** | 5 provider configs missing |
| GAP-3.3c | OPEN | **CONFIRMED OPEN** | extra_headers missing |
| GAP-3.3d | OPEN | **CONFIRMED OPEN** | GatewayConfig missing |
| GAP-3.4 | RESOLVED | **CONFIRMED RESOLVED** | CLI REPL complete |
| GAP-3.5 | RESOLVED | **CONFIRMED RESOLVED** | Heartbeat wired |
| GAP-3.6 | OPEN | **CONFIRMED OPEN** | Critical: no LLM call |
| GAP-3.7 | OPEN | **CONFIRMED OPEN** | No nanobot fallback |
| GAP-3.8 | OPEN | **CONFIRMED OPEN** | Only 6 env vars |
| GAP-3.9 | PARTIAL | **CONFIRMED PARTIAL** | See detailed breakdown |
| GAP-4.1 | OPEN | **RECLASSIFIED: RESOLVED** | brave_api_key IS used |
| GAP-4.2 | OPEN | **RECLASSIFIED: PARTIAL** | allowed_commands not wired |
| GAP-4.3 | OPEN | **RECLASSIFIED: RESOLVED** | LRU eviction implemented |
| GAP-4.4 | OPEN | **RECLASSIFIED: RESOLVED** | Skills in system prompt |

### Reclassification Details

**GAP-3.1d** reclassified **OPEN -> RESOLVED**: `telegram.rs:401-417` publishes a system message with sender_id `"telegram_reset"` and content `"__RESET_SESSION__"`. The agent loop at `agent_loop.rs:344-361` handles this by clearing the session and saving it to disk. Full pipeline works.

**GAP-4.1** reclassified **OPEN -> RESOLVED**: `agent_loop.rs:69-70,109-116` reads `config.tools.web.brave_api_key` and `config.tools.web.max_results`, passing them to both `WebSearchTool::new()` and `SubagentManager::new()`.

**GAP-4.3** reclassified **OPEN -> RESOLVED**: `session/manager.rs:82-135` implements full LRU eviction with `VecDeque<String>`, `max_cache_size` (default 1000), and `with_capacity()` constructor. Evicted sessions are saved to disk before removal.

**GAP-4.4** reclassified **OPEN -> RESOLVED**: `context.rs:131-138` includes `skills.generate_summary()` in the system prompt, and always-loaded skills are embedded as full content sections. `SkillManager` loads from workspace.

---

## TRULY OPEN GAPS — Detailed Specifications

---

### GAP-3.2d: Discord Gateway URL and Intents From Config

**Status: OPEN**
**Assigned to: Task #5 (Channel Dev)**
**Complexity: Low**

#### Problem

`discord.rs:23-24` defines hardcoded constants:
```rust
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const INTENTS: i64 = 37377;
```

These are used at:
- `discord.rs:55,57` — `connect_async(GATEWAY_URL)`
- `discord.rs:141` — `"intents": INTENTS` in IDENTIFY payload

The config schema already has `gateway_url` and `intents` on `DiscordConfig` with correct defaults.

#### Required Changes

1. **Remove constants** at `discord.rs:23-24` (`GATEWAY_URL`, `INTENTS`)
2. **Replace `GATEWAY_URL` with `self.config.gateway_url`** at `discord.rs:55,57`
3. **Replace `INTENTS` with `self.config.intents`** at `discord.rs:141`

#### Exact Code Locations

| File | Line | Current | Replace With |
|------|------|---------|--------------|
| `discord.rs` | 23 | `const GATEWAY_URL: &str = "wss://..."` | DELETE |
| `discord.rs` | 24 | `const INTENTS: i64 = 37377` | DELETE |
| `discord.rs` | 55 | `info!("Connecting to Discord Gateway: {}", GATEWAY_URL)` | `info!("Connecting to Discord Gateway: {}", self.config.gateway_url)` |
| `discord.rs` | 57 | `connect_async(GATEWAY_URL)` | `connect_async(&self.config.gateway_url)` |
| `discord.rs` | 141 | `"intents": INTENTS` | `"intents": self.config.intents` |

---

### GAP-3.3a: Missing Channel Configs (feishu, dingtalk, mochat)

**Status: OPEN**
**Assigned to: Task #2 (Config Dev)**
**Complexity: Medium**

#### Python Reference (nanobot/config/schema.py)

**FeishuConfig** (lines 23-30):
```
Fields:
  enabled: bool = False
  app_id: str = ""
  app_secret: str = ""
  encrypt_key: str = ""
  verification_token: str = ""
  allow_from: list[str] = []
```

**DingTalkConfig** (lines 33-38):
```
Fields:
  enabled: bool = False
  client_id: str = ""
  client_secret: str = ""
  allow_from: list[str] = []
```

**MochatConfig** (lines 90-113):
```
Fields:
  enabled: bool = False
  base_url: str = "https://mochat.io"
  socket_url: str = ""
  socket_path: str = "/socket.io"
  socket_disable_msgpack: bool = False
  socket_reconnect_delay_ms: int = 1000
  socket_max_reconnect_delay_ms: int = 10000
  socket_connect_timeout_ms: int = 10000
  refresh_interval_ms: int = 30000
  watch_timeout_ms: int = 25000
  watch_limit: int = 100
  retry_delay_ms: int = 500
  max_retry_attempts: int = 0
  claw_token: str = ""
  agent_user_id: str = ""
  sessions: list[str] = []
  panels: list[str] = []
  allow_from: list[str] = []
  mention: MochatMentionConfig = MochatMentionConfig()
  groups: dict[str, MochatGroupRule] = {}
  reply_delay_mode: str = "non-mention"
  reply_delay_ms: int = 120000
```

**MochatMentionConfig**:
```
  require_in_groups: bool = False
```

**MochatGroupRule**:
```
  require_mention: bool = False
```

#### Rust Structs to Add

Add to `schema.rs` the following structs, then add fields to `ChannelsConfig`:

```rust
/// Feishu/Lark channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub encrypt_key: String,
    #[serde(default)]
    pub verification_token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// DingTalk channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DingTalkConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// Mochat mention configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MochatMentionConfig {
    #[serde(default)]
    pub require_in_groups: bool,
}

/// Mochat per-group rule
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MochatGroupRule {
    #[serde(default)]
    pub require_mention: bool,
}

/// Mochat channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MochatConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mochat_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub socket_url: String,
    #[serde(default = "default_mochat_socket_path")]
    pub socket_path: String,
    #[serde(default)]
    pub socket_disable_msgpack: bool,
    #[serde(default = "default_1000u64")]
    pub socket_reconnect_delay_ms: u64,
    #[serde(default = "default_10000u64")]
    pub socket_max_reconnect_delay_ms: u64,
    #[serde(default = "default_10000u64")]
    pub socket_connect_timeout_ms: u64,
    #[serde(default = "default_30000u64")]
    pub refresh_interval_ms: u64,
    #[serde(default = "default_25000u64")]
    pub watch_timeout_ms: u64,
    #[serde(default = "default_100u32")]
    pub watch_limit: u32,
    #[serde(default = "default_500u64")]
    pub retry_delay_ms: u64,
    #[serde(default)]
    pub max_retry_attempts: u32,
    #[serde(default)]
    pub claw_token: String,
    #[serde(default)]
    pub agent_user_id: String,
    #[serde(default)]
    pub sessions: Vec<String>,
    #[serde(default)]
    pub panels: Vec<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub mention: MochatMentionConfig,
    #[serde(default)]
    pub groups: HashMap<String, MochatGroupRule>,
    #[serde(default = "default_reply_delay_mode")]
    pub reply_delay_mode: String,
    #[serde(default = "default_120000u64")]
    pub reply_delay_ms: u64,
}
```

Add to `ChannelsConfig`:
```rust
#[serde(default)]
pub feishu: FeishuConfig,
#[serde(default)]
pub dingtalk: DingTalkConfig,
#[serde(default)]
pub mochat: MochatConfig,
```

#### Default Value Functions Needed

```rust
fn default_mochat_base_url() -> String { "https://mochat.io".to_string() }
fn default_mochat_socket_path() -> String { "/socket.io".to_string() }
fn default_reply_delay_mode() -> String { "non-mention".to_string() }
// Numeric defaults can use simple const functions or dedicated fns
```

---

### GAP-3.3b: Missing Provider Configs

**Status: OPEN**
**Assigned to: Task #2 (Config Dev)**
**Complexity: Low**

#### Python Reference (nanobot/config/schema.py:178-191)

Missing providers in Rust `ProvidersConfig`:
- `zhipu` — ZhiPu AI (GLM models)
- `dashscope` — Alibaba Cloud Tongyi Qianwen
- `moonshot` — Moonshot AI (Kimi)
- `minimax` — MiniMax
- `aihubmix` — AiHubMix API gateway

All use the standard `ProviderConfig` struct (api_key, api_base, extra_headers).

#### Required Changes

Add to `ProvidersConfig` in `schema.rs`:
```rust
#[serde(default)]
pub zhipu: ProviderConfig,

#[serde(default)]
pub dashscope: ProviderConfig,

#[serde(default)]
pub moonshot: ProviderConfig,

#[serde(default)]
pub minimax: ProviderConfig,

#[serde(default)]
pub aihubmix: ProviderConfig,
```

No new structs needed — they reuse `ProviderConfig`.

---

### GAP-3.3c: Missing ProviderConfig.extra_headers

**Status: OPEN**
**Assigned to: Task #2 (Config Dev)**
**Complexity: Low**

#### Python Reference (nanobot/config/schema.py:175)

```python
extra_headers: dict[str, str] | None = None
```

Used by providers like AiHubMix that require custom HTTP headers (e.g., `APP-Code`).

#### Required Changes

Add to `ProviderConfig` in `schema.rs`:
```rust
#[serde(default)]
#[serde(skip_serializing_if = "Option::is_none")]
pub extra_headers: Option<HashMap<String, String>>,
```

**Note:** Uses `HashMap<String, String>` (add `use std::collections::HashMap;` if not already imported in schema.rs).

#### Runtime Usage

The LLM provider HTTP clients should inject these headers into API requests. This is a follow-up concern for the provider implementation, not just the schema.

---

### GAP-3.3d: Missing GatewayConfig

**Status: OPEN**
**Assigned to: Task #2 (Config Dev)**
**Complexity: Low**

#### Python Reference (nanobot/config/schema.py:194-197)

```python
class GatewayConfig(BaseModel):
    host: str = "0.0.0.0"
    port: int = 18790
```

**IMPORTANT:** Python nanobot uses port `18790` as default, NOT 8080 as stated in ACCEPTANCE_CRITERIA.md.

#### Required Changes

Add struct:
```rust
/// Gateway/HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_host")]
    pub host: String,

    #[serde(default = "default_gateway_port")]
    pub port: u16,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_gateway_host(),
            port: default_gateway_port(),
        }
    }
}

fn default_gateway_host() -> String { "0.0.0.0".to_string() }
fn default_gateway_port() -> u16 { 18790 }
```

Add to `Config`:
```rust
#[serde(default)]
pub gateway: GatewayConfig,
```

---

### GAP-3.6: process_system_message Does Not Run Through LLM

**Status: OPEN (CRITICAL)**
**Assigned to: Task #4 (Agent Core Dev)**
**Complexity: High**

#### Python Reference (nanobot/agent/loop.py:252-349)

The Python `_process_system_message` method does the following:

1. Parse `channel:chat_id` from `msg.chat_id` (using `split(":", 1)`)
2. Get or create session for the origin `channel:chat_id`
3. Update tool contexts (message, spawn, cron) to the origin channel
4. Build messages using `context.build_messages()` with session history + subagent result as current message
5. Run the full agent loop (up to `max_iterations`) calling LLM, executing tool calls
6. Save to session: `[System: {sender_id}] {content}` as "user" role, final response as "assistant" role
7. Return `OutboundMessage` to `origin_channel:origin_chat_id`

#### Current Rust Behavior (agent_loop.rs:340-387)

The Rust code currently:
1. Handles session reset messages (correct, keep this) ✓
2. Parses `channel:chat_id` from `msg.chat_id` ✓
3. Saves system message to session as "system" role ✓
4. **Returns Ok(()) without calling LLM** ← THE GAP
5. **Does NOT publish any OutboundMessage** ← THE GAP

#### Required Changes

After the session reset check (line 361), the `process_system_message` method should:

1. Parse origin channel/chat_id (existing code, lines 363-371)
2. Update tool contexts for origin channel (call `self.update_tool_contexts(origin_channel, origin_chat_id).await`)
3. Save system message to session as a "user" message (format: `[System: {sender_id}] {content}`)
4. Get session history
5. Build LLM messages using `context_builder.build_messages()`
6. Run the same agent loop as `process_message()` (call LLM, handle tool calls, up to max_iterations)
7. Save assistant response to session
8. Publish `OutboundMessage` to `origin_channel:origin_chat_id` via `self.bus.publish_outbound()`

#### Behavioral Contract

```
Input:  InboundMessage { channel: "system", chat_id: "telegram:12345",
                          sender_id: "subagent_research", content: "Research result: ..." }

Step 1: Parse -> origin_channel="telegram", origin_chat_id="12345"
Step 2: session_key = "telegram:12345"
Step 3: Update tool contexts for ("telegram", "12345")
Step 4: Add "[System: subagent_research] Research result: ..." to session as "user"
Step 5: Build messages from session history
Step 6: Call LLM -> get natural language response (may include tool calls)
Step 7: Save LLM response to session as "assistant"
Step 8: Publish OutboundMessage { channel: "telegram", chat_id: "12345", content: <response> }
```

#### Error Handling

- If LLM call fails during system message processing: log error, subagent result is still saved to session, return error
- If origin channel:chat_id parse fails: existing code handles this with warning (keep as-is)

---

### GAP-3.7: Config Loader Missing Nanobot Fallback

**Status: OPEN**
**Assigned to: Task #3 (Config Dev)**
**Complexity: Medium**

#### Python Reference (nanobot/config/loader.py)

The Python loader:
1. Checks `~/.nanobot/config.json` (single path, no fallback needed in Python since it IS the nanobot dir)
2. If found, parses JSON and runs `_migrate_config()` + `convert_keys()` (camelCase -> snake_case)
3. If not found, returns `Config()` defaults

For klyntbot, the loader should:
1. Check `~/.klyntbot/config.json` first
2. If not found, check `~/.nanobot/config.json` as fallback
3. If nanobot config found, load it, migrate, save to klyntbot path

#### Migration Logic (Python loader.py:65-72)

```python
def _migrate_config(data: dict) -> dict:
    tools = data.get("tools", {})
    exec_cfg = tools.get("exec", {})
    if "restrictToWorkspace" in exec_cfg and "restrictToWorkspace" not in tools:
        tools["restrictToWorkspace"] = exec_cfg.pop("restrictToWorkspace")
    return data
```

This migrates `tools.exec.restrictToWorkspace` -> `tools.restrictToWorkspace`.

#### Key Conversion (Python loader.py:75-81)

The Python nanobot stores config in camelCase JSON but Pydantic uses snake_case. The `convert_keys()` function converts camelCase to snake_case during load. Since Rust serde uses `rename_all = "camelCase"`, the Rust code can deserialize camelCase JSON directly.

#### Required Changes to `loader.rs`

Modify `load()` function:

```rust
pub fn load() -> Result<Config> {
    let klyntbot_path = config_path(); // ~/.klyntbot/config.json

    if klyntbot_path.exists() {
        let content = fs::read_to_string(&klyntbot_path).map_err(ConfigError::Io)?;
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;
        return Ok(config);
    }

    // Fallback: try nanobot config
    let nanobot_path = dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".nanobot")
        .join("config.json");

    if nanobot_path.exists() {
        info!("Migrating config from ~/.nanobot/config.json to ~/.klyntbot/config.json");
        let content = fs::read_to_string(&nanobot_path).map_err(ConfigError::Io)?;

        // Parse and migrate
        let mut data: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| {
                warn!("Failed to parse nanobot config, using defaults: {}", e);
                ConfigError::Invalid(format!("Nanobot config parse error: {}", e))
            })?;

        // Migrate: tools.exec.restrictToWorkspace -> tools.restrictToWorkspace
        migrate_config(&mut data);

        let config: Config = serde_json::from_value(data)
            .map_err(|e| ConfigError::Invalid(format!("Failed to migrate config: {}", e)))?;

        // Save migrated config to klyntbot path
        if let Err(e) = save(&config) {
            warn!("Failed to save migrated config: {}", e);
        }

        return Ok(config);
    }

    // Neither exists
    Ok(Config::default())
}

fn migrate_config(data: &mut serde_json::Value) {
    if let Some(tools) = data.get_mut("tools").and_then(|v| v.as_object_mut()) {
        if let Some(exec) = tools.get_mut("exec").and_then(|v| v.as_object_mut()) {
            if let Some(rtw) = exec.remove("restrictToWorkspace") {
                if !tools.contains_key("restrictToWorkspace") {
                    tools.insert("restrictToWorkspace".to_string(), rtw);
                }
            }
        }
    }
}
```

---

### GAP-3.8: Config Loader Limited Env Var Overrides

**Status: OPEN**
**Assigned to: Task #3 (Config Dev)**
**Complexity: Medium**

#### Python Reference

Python nanobot uses Pydantic's built-in env var support:
```python
class Config(BaseSettings):
    class Config:
        env_prefix = "NANOBOT_"
        env_nested_delimiter = "__"
```

This automatically maps ANY config field to an env var. For klyntbot, we use manual mapping with `KLYNTBOT_` prefix.

#### Current Coverage (loader.rs:60-89)

Currently handled (6 vars):
1. `KLYNTBOT_AGENTS__DEFAULTS__MODEL`
2. `KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE`
3. `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY`
4. `KLYNTBOT_PROVIDERS__OPENAI__API_KEY`
5. `KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY`
6. `KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY`

#### Required Additional Env Vars

**Provider API keys** (add for all providers):
| Env Var | Config Path |
|---------|-------------|
| `KLYNTBOT_PROVIDERS__GEMINI__API_KEY` | `providers.gemini.api_key` |
| `KLYNTBOT_PROVIDERS__GROQ__API_KEY` | `providers.groq.api_key` |
| `KLYNTBOT_PROVIDERS__VLLM__API_KEY` | `providers.vllm.api_key` |
| `KLYNTBOT_PROVIDERS__ZHIPU__API_KEY` | `providers.zhipu.api_key` |
| `KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY` | `providers.dashscope.api_key` |
| `KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY` | `providers.moonshot.api_key` |
| `KLYNTBOT_PROVIDERS__MINIMAX__API_KEY` | `providers.minimax.api_key` |
| `KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY` | `providers.aihubmix.api_key` |

**Channel tokens**:
| Env Var | Config Path |
|---------|-------------|
| `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN` | `channels.telegram.token` |
| `KLYNTBOT_CHANNELS__DISCORD__TOKEN` | `channels.discord.token` |
| `KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN` | `channels.slack.bot_token` |
| `KLYNTBOT_CHANNELS__SLACK__APP_TOKEN` | `channels.slack.app_token` |

**Agent defaults**:
| Env Var | Config Path | Parse Type |
|---------|-------------|------------|
| `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE` | `agents.defaults.temperature` | `f32` |
| `KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS` | `agents.defaults.max_tokens` | `u32` |

**Tools**:
| Env Var | Config Path |
|---------|-------------|
| `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY` | `tools.web.brave_api_key` |

#### Implementation Pattern

For string overrides:
```rust
if let Ok(val) = std::env::var("KLYNTBOT_PROVIDERS__GEMINI__API_KEY") {
    config.providers.gemini.api_key = val;
}
```

For numeric overrides:
```rust
if let Ok(val) = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE") {
    if let Ok(temp) = val.parse::<f32>() {
        config.agents.defaults.temperature = temp;
    }
}
```

---

### GAP-3.9: Email Config Fields Not Used at Runtime

**Status: PARTIAL**
**Assigned to: Task #5 (Channel Dev)**
**Complexity: Medium**

#### Detailed Field-by-Field Status

| Config Field | Schema Present? | Runtime Used? | Status |
|-------------|-----------------|---------------|--------|
| `consent_granted` | Yes | Yes (`email.rs:45`) | DONE |
| `max_body_chars` | Yes | Yes (`email.rs:246-248`) | DONE |
| `imap_mailbox` | Yes | Yes (`email.rs:119`) | DONE |
| `mark_seen` | Yes | Yes (`email.rs:173`) | DONE |
| `auto_reply_enabled` | Yes (schema) | **NO** (send() doesn't check) | **OPEN** |
| `imap_use_ssl` | Yes (schema) | **NO** (always TLS) | **OPEN** |
| `poll_interval_seconds` | **NO** (missing from schema) | **NO** (hardcoded 30s) | **OPEN** |
| `subject_prefix` | **NO** (missing from schema) | **NO** (hardcoded "Re: ") | **OPEN** |

#### Required Changes

**1. Add missing schema fields to `EmailConfig`:**

```rust
// In EmailConfig struct:
#[serde(default = "default_poll_interval_seconds")]
pub poll_interval_seconds: u32,

#[serde(default = "default_subject_prefix")]
pub subject_prefix: String,

// Default functions:
fn default_poll_interval_seconds() -> u32 { 30 }
fn default_subject_prefix() -> String { "Re: ".to_string() }
```

Also update the `Default` impl for `EmailConfig` to include these fields.

**2. `auto_reply_enabled` check in `send()`** (`email.rs:403`):

Add check at the start of `send()`:
```rust
async fn send(&self, msg: &OutboundMessage) -> Result<()> {
    if !self.config.auto_reply_enabled {
        info!("Skip automatic email reply: auto_reply_enabled is false");
        return Ok(());
    }
    // ... existing code
}
```

**3. `imap_use_ssl` in `poll_imap()`** (`email.rs:89-104`):

Currently always creates a TLS connector. Should check `self.config.imap_use_ssl`:

```rust
if self.config.imap_use_ssl {
    // Current TLS path (lines 89-107)
} else {
    // Plain TCP connection
    let tcp_stream = TcpStream::connect((...)).await?;
    let client = async_imap::Client::new(tcp_stream);
    // ... rest of login/select/search
}
```

**4. `poll_interval_seconds` in `start()`** (`email.rs:385`):

Replace:
```rust
let poll_interval = Duration::from_secs(30); // hardcoded
```
With:
```rust
let poll_interval = Duration::from_secs(self.config.poll_interval_seconds.max(5) as u64);
```

**5. `subject_prefix` in `reply_subject()`** (`email.rs:357-369`):

Replace hardcoded `"Re: "`:
```rust
fn reply_subject(&self, base_subject: &str) -> String {
    let subject = if base_subject.is_empty() { "nanobot reply" } else { base_subject };
    let prefix = if self.config.subject_prefix.is_empty() {
        "Re: "
    } else {
        &self.config.subject_prefix
    };
    if subject.to_lowercase().starts_with("re:") {
        subject.to_string()
    } else {
        format!("{}{}", prefix, subject)
    }
}
```

---

### GAP-4.2: ExecTool allowed_commands Not Wired

**Status: PARTIAL** (timeout and restrict_to_workspace work; allowed_commands does not)
**Assigned to: Task #5 (Channel Dev) or separate**
**Complexity: Low**

#### Problem

`ExecToolConfig` in schema has `allowed_commands: Vec<String>`, but `ExecTool::new()` at `agent_loop.rs:102-106` only passes `timeout`, `working_dir`, and `restrict_to_workspace`. The `allowed_commands` are never passed to `ExecTool`.

The `ExecTool` struct does have `allow_patterns: Vec<Regex>` (shell.rs:20) but it's initialized as empty.

#### Required Changes

**Option A (Recommended):** Pass `allowed_commands` to `ExecTool::new()` and compile them into `allow_patterns`.

Modify `ExecTool::new()` signature:
```rust
pub fn new(
    timeout_secs: u64,
    working_dir: Option<PathBuf>,
    restrict_to_workspace: bool,
    allowed_commands: Vec<String>,
) -> Self {
    // ... existing deny_patterns code ...

    let compiled_allow = allowed_commands
        .iter()
        .filter_map(|cmd| Regex::new(&format!(r"^{}\b", regex::escape(cmd))).ok())
        .collect();

    Self {
        timeout: Duration::from_secs(timeout_secs),
        working_dir,
        deny_patterns: compiled_deny,
        allow_patterns: compiled_allow,
        restrict_to_workspace,
    }
}
```

Update call site at `agent_loop.rs:102-106`:
```rust
tool_registry.register(ExecTool::new(
    config.tools.exec.timeout,
    Some(workspace.clone()),
    config.tools.restrict_to_workspace,
    config.tools.exec.allowed_commands.clone(),
));
```

---

## Config Schema Summary — All Fields

### Complete ChannelsConfig After Changes

```
channels:
  telegram:    TelegramConfig     (existing)
  discord:     DiscordConfig      (existing)
  whatsapp:    WhatsAppConfig     (existing)
  slack:       SlackConfig        (existing)
  email:       EmailConfig        (existing, add poll_interval_seconds + subject_prefix)
  qq:          QQConfig           (existing)
  feishu:      FeishuConfig       (NEW)
  dingtalk:    DingTalkConfig     (NEW)
  mochat:      MochatConfig       (NEW)
```

### Complete ProvidersConfig After Changes

```
providers:
  anthropic:   ProviderConfig     (existing)
  openai:      ProviderConfig     (existing)
  openrouter:  ProviderConfig     (existing)
  deepseek:    ProviderConfig     (existing)
  gemini:      ProviderConfig     (existing)
  groq:        ProviderConfig     (existing)
  vllm:        ProviderConfig     (existing)
  zhipu:       ProviderConfig     (NEW)
  dashscope:   ProviderConfig     (NEW)
  moonshot:    ProviderConfig     (NEW)
  minimax:     ProviderConfig     (NEW)
  aihubmix:    ProviderConfig     (NEW)
```

### Complete ProviderConfig After Changes

```
ProviderConfig:
  api_key:        String                          (existing)
  api_base:       Option<String>                  (existing)
  extra_headers:  Option<HashMap<String, String>>  (NEW)
```

### Complete Root Config After Changes

```
Config:
  agents:    AgentsConfig     (existing)
  channels:  ChannelsConfig   (existing, expanded)
  providers: ProvidersConfig  (existing, expanded)
  tools:     ToolsConfig      (existing)
  gateway:   GatewayConfig    (NEW)
```

---

## Implementation Priority (Revised)

| Priority | Gap | Complexity | Blocker? |
|----------|-----|------------|----------|
| 1 | GAP-3.3a-d (Config schema) | Medium | Blocks env var overrides |
| 2 | GAP-3.3c (extra_headers) | Low | Part of schema batch |
| 3 | GAP-3.7 (Nanobot fallback) | Medium | Independent |
| 4 | GAP-3.8 (Env var expansion) | Medium | Depends on new provider fields |
| 5 | GAP-3.2d (Discord config) | Low | Independent |
| 6 | GAP-3.9 (Email runtime) | Medium | Independent |
| 7 | GAP-4.2 (ExecTool allowed_commands) | Low | Independent |
| 8 | GAP-3.6 (process_system_message) | **High** | Independent, critical |

---

## Test Requirements Summary

| Gap | Unit Tests | Integration Tests |
|-----|-----------|-------------------|
| GAP-3.2d | Discord config deserialization | Discord connects with custom gateway_url |
| GAP-3.3a | New config structs default + round-trip | Existing configs still deserialize |
| GAP-3.3b | New provider fields default + round-trip | — |
| GAP-3.3c | extra_headers serialization | — |
| GAP-3.3d | GatewayConfig default values | — |
| GAP-3.6 | Mock LLM call in process_system_message | Subagent result -> LLM -> outbound message |
| GAP-3.7 | Load from nanobot path, migration | Round-trip migration test |
| GAP-3.8 | Each env var override | Env var precedence over file |
| GAP-3.9 | auto_reply_enabled=false skips send | poll_interval from config |
| GAP-4.2 | allowed_commands blocks/allows | — |
