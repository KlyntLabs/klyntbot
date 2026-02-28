# Config

**Crate path:** `crates/config/`
**Dependency layer:** 1 (depends only on `common`)

---

## Section 1: Narrative Overview

### Purpose

The `config` crate is the single source of truth for all runtime configuration in the Klyntbot workspace. It defines the complete configuration schema as Rust structs, handles loading from disk and environment variables, and provides save logic that writes only non-default values. Every other crate that needs configuration -- from `providers` to `agent` to `channels` -- receives a `Config` struct (or a sub-section of it) at construction time.

The crate lives at dependency layer 1 and depends only on `common` (for error types). This keeps the configuration schema importable by every higher layer without introducing circular dependencies.

### Config Loading Strategy

Configuration is resolved through a three-layer merge strategy:

1. **Compiled defaults.** Every struct implements `Default`, producing a fully valid configuration with sensible values. A freshly initialized Klyntbot works out of the box with zero user configuration.

2. **JSON file on disk.** The canonical config file lives at `~/.klyntbot/config.json`. When present, it is deserialized on top of defaults. Because every field carries `#[serde(default)]` or `#[serde(default = "...")]`, the file only needs to contain the fields the user has changed -- missing fields silently fall back to defaults.

3. **Environment variable overrides.** The function `load_with_env_overrides()` applies env vars on top of the loaded config. Variables use the `KLYNTBOT_` prefix with `__` (double underscore) as the nesting separator.

Loading functions are provided in both async and sync variants:

| Function | Async | Purpose |
|----------|-------|---------|
| `load()` | Yes | Primary loader for the agent loop and request handlers |
| `load_sync()` | No | For constructors, the setup wizard, and tests |
| `load_with_env_overrides()` | Yes | `load()` + env var overlay; used at binary entry point |

All loading functions are defined in `crates/config/src/loader.rs`.

### Minimal Save (Diff-Based Persistence)

When saving, the `save()` and `save_sync()` functions do not dump the entire config. Instead they compute a recursive JSON diff between the current config and `Config::default()`, then write only the keys that differ. This keeps `~/.klyntbot/config.json` minimal and human-readable: a user who only configured an Anthropic API key gets a config file with just that key and nothing else.

The diff logic is in `diff_json()` at `crates/config/src/loader.rs:108`. Empty objects produced by the diff are pruned so parent keys do not appear unnecessarily.

### Schema Design Conventions

- **camelCase JSON keys.** Every struct carries `#[serde(rename_all = "camelCase")]`. Rust fields use snake_case; the JSON file uses camelCase. There are zero snake_case keys in the serialized output.

- **`Secret<T>` wrapper.** API keys, tokens, and passwords are wrapped in `Secret<String>` (defined at `crates/config/src/schema/core.rs:35`). `Secret` implements `Serialize`/`Deserialize` transparently but prints `[REDACTED]` for both `Debug` and `Display`. Access the inner value through `.expose()`.

- **Optional fields skip serialization when `None`.** Fields like `api_base`, `provider`, and `data_dir` use `#[serde(skip_serializing_if = "Option::is_none")]` so they do not appear in output when unset.

- **Shared default helpers.** Commonly reused defaults (`default_true()`, `default_semantic_threshold()`) are defined once in `core.rs` and imported via `pub(crate)` by the sub-modules that need them.

### Environment Variable Overrides

The `load_with_env_overrides()` function (at `crates/config/src/loader.rs:169`) applies overrides using three internal macros:

| Macro | Purpose | Example |
|-------|---------|---------|
| `env_string!` | Sets a `String` field | `KLYNTBOT_AGENTS__DEFAULTS__MODEL` |
| `env_parse!` | Parses a numeric field | `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE` (f32) |
| `env_secret!` | Sets a `Secret<String>` field | `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY` |

The naming convention maps struct nesting to double underscores:

```
config.providers.anthropic.api_key
  -->  KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY

config.agents.defaults.model
  -->  KLYNTBOT_AGENTS__DEFAULTS__MODEL
```

The following env vars are explicitly handled:

**Agent defaults:**
- `KLYNTBOT_AGENTS__DEFAULTS__MODEL`
- `KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE`
- `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE`
- `KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS`

**Provider API keys** (all via `env_secret!`):
- `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY`
- `KLYNTBOT_PROVIDERS__OPENAI__API_KEY`
- `KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY`
- `KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY`
- `KLYNTBOT_PROVIDERS__GEMINI__API_KEY`
- `KLYNTBOT_PROVIDERS__GROQ__API_KEY`
- `KLYNTBOT_PROVIDERS__VLLM__API_KEY`
- `KLYNTBOT_PROVIDERS__ZHIPU__API_KEY`
- `KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY`
- `KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY`
- `KLYNTBOT_PROVIDERS__MINIMAX__API_KEY`
- `KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY`

**Data directory:**
- `KLYNTBOT_DATA_DIR`

**Channel tokens:**
- `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN`
- `KLYNTBOT_CHANNELS__DISCORD__TOKEN`
- `KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN`
- `KLYNTBOT_CHANNELS__SLACK__APP_TOKEN`

**Tool keys:**
- `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY`

### How Other Crates Consume Config

The `config` crate re-exports all public types from `lib.rs` (line 8-19). Downstream crates import directly:

```rust
use config::{Config, Secret, TelegramConfig};
```

The `klyntbot` facade crate at the top of the dependency tree also re-exports `Config` and `StoragePool`, so integration tests use `klyntbot::Config`.

At startup, the binary calls `config::load_with_env_overrides()` to produce a `Config`. Individual sub-structs are then passed by reference or cloned into the components that need them (e.g., `config.providers.anthropic` is handed to the Anthropic provider, `config.channels.telegram` to the Telegram channel).

The `Config` struct also provides several convenience methods:

- `workspace_path()` -- expands `~` in the workspace setting and returns a `PathBuf`.
- `data_dir_path()` -- resolves the data directory (defaults to `~/.klyntbot`).
- `active_provider_name()` -- detects which LLM provider is active (explicit `provider` field first, then auto-detection by checking which keys are configured).
- `is_provider_configured(name)` -- checks if a provider has a non-empty API key.
- `set_provider_key(name, key)` -- sets the API key for a provider by name.

---

## Section 2: API Reference

### Root Config Struct

**File:** `crates/config/src/schema/core.rs:78-139`

```rust
pub struct Config
```

All fields carry `#[serde(default)]` and use camelCase JSON keys.

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `agents` | `AgentsConfig` | `AgentsConfig::default()` | `agents` | Agent behavior defaults |
| `channels` | `ChannelsConfig` | `ChannelsConfig::default()` | `channels` | Chat platform integrations |
| `providers` | `ProvidersConfig` | `ProvidersConfig::default()` | `providers` | LLM provider credentials and settings |
| `tools` | `ToolsConfig` | `ToolsConfig::default()` | `tools` | Tool-level configuration |
| `gateway` | `GatewayConfig` | `GatewayConfig::default()` | `gateway` | HTTP server settings |
| `todo` | `TodoConfig` | `TodoConfig::default()` | `todo` | Task management system |
| `confidence` | `ConfidenceConfig` | `ConfidenceConfig::default()` | `confidence` | Confidence evaluation engine |
| `calendar` | `CalendarConfig` | `CalendarConfig::default()` | `calendar` | Calendar sync providers |
| `project` | `ProjectConfig` | `ProjectConfig::default()` | `project` | Project management |
| `conversation` | `ConversationConfig` | `ConversationConfig::default()` | `conversation` | Conversation memory and embedding |
| `learning` | `LearningConfig` | `LearningConfig::default()` | `learning` | Adaptive confidence thresholds |
| `finance` | `FinanceConfig` | `FinanceConfig::default()` | `finance` | Finance tracking system |
| `orchestrator` | `OrchestratorConfig` | `OrchestratorConfig::default()` | `orchestrator` | Intent pipeline configuration |
| `provider_manager` | `ProviderManagerConfig` | `ProviderManagerConfig::default()` | `providerManager` | Primary/fallback/classifier routing |
| `timezone` | `String` | Auto-detected system timezone, fallback `"UTC"` | `timezone` | IANA timezone |
| `data_dir` | `Option<String>` | `None` (resolves to `~/.klyntbot`) | `dataDir` | Data directory override; skipped in JSON when `None` |
| `packs` | `PacksConfig` | `PacksConfig::default()` | `packs` | Feature pack selection |
| `plugins` | `PluginsConfig` | `PluginsConfig::default()` | `plugins` | Plugin system settings |

**Methods on `Config`:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `workspace_path` | `fn workspace_path(&self) -> PathBuf` | Expands `~` in `agents.defaults.workspace` |
| `data_dir_path` | `fn data_dir_path(&self) -> PathBuf` | Resolves data dir, defaults to `~/.klyntbot` |
| `active_provider_name` | `fn active_provider_name(&self) -> &str` | Detects active LLM provider (explicit or auto-detect) |
| `is_provider_configured` | `fn is_provider_configured(&self, name: &str) -> bool` | Checks if a provider has a non-empty API key |
| `set_provider_key` | `fn set_provider_key(&mut self, provider_name: &str, key: String)` | Sets API key for a named provider |

### Secret Wrapper

**File:** `crates/config/src/schema/core.rs:35-75`

```rust
pub struct Secret<T>(T);
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: T) -> Self` | Wrap a value |
| `expose` | `fn expose(&self) -> &T` | Access the inner value |
| `into_inner` | `fn into_inner(self) -> T` | Consume and unwrap |
| `is_empty` | `fn is_empty(&self) -> bool` | (Only for `Secret<String>`) check if empty |

`Debug` and `Display` both print `[REDACTED]`. Serializes/deserializes transparently via `#[serde(transparent)]`.

---

### Loader Functions

**File:** `crates/config/src/loader.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `config_path` | 12 | `fn config_path() -> Result<PathBuf>` | Returns `~/.klyntbot/config.json` |
| `config_dir` | 21 | `fn config_dir() -> Result<PathBuf>` | Returns `~/.klyntbot/` |
| `load` | 30 | `async fn load() -> Result<Config>` | Load from file or return defaults |
| `save` | 49 | `async fn save(config: &Config) -> Result<()>` | Save diff-only JSON to file |
| `load_sync` | 71 | `fn load_sync() -> Result<Config>` | Synchronous variant of `load` |
| `save_sync` | 89 | `fn save_sync(config: &Config) -> Result<()>` | Synchronous variant of `save` |
| `load_with_env_overrides` | 169 | `async fn load_with_env_overrides() -> Result<Config>` | Load + apply env var overrides |
| `exists` | 294 | `fn exists() -> bool` | Check if config file exists |
| `init` | 299 | `async fn init() -> Result<()>` | Create directory structure and default config |

---

### Agents

**File:** `crates/config/src/schema/agents.rs`

#### `AgentsConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `defaults` | `AgentDefaults` | `AgentDefaults::default()` | `defaults` |

#### `AgentDefaults`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `workspace` | `String` | `"~/.klyntbot/workspace"` | `workspace` | Working directory for file tools |
| `model` | `String` | `"anthropic/claude-opus-4-5"` | `model` | Default LLM model identifier |
| `provider` | `Option<String>` | `None` | `provider` | Explicit provider override; skipped when `None` |
| `max_tokens` | `u32` | `8192` | `maxTokens` | Max output tokens per LLM call |
| `temperature` | `f32` | `0.7` | `temperature` | Sampling temperature |
| `max_tool_iterations` | `u32` | `20` | `maxToolIterations` | Max ReAct loop iterations |
| `max_concurrent_subagents` | `usize` | `3` | `maxConcurrentSubagents` | Max parallel subagent tasks |

---

### Providers

**File:** `crates/config/src/schema/providers.rs`

#### `ProvidersConfig`

Contains one `ProviderConfig` field per supported LLM provider. All fields default to `ProviderConfig::default()`.

| Field | JSON Key |
|-------|----------|
| `anthropic` | `anthropic` |
| `openai` | `openai` |
| `openrouter` | `openrouter` |
| `deepseek` | `deepseek` |
| `gemini` | `gemini` |
| `groq` | `groq` |
| `vllm` | `vllm` |
| `zhipu` | `zhipu` |
| `dashscope` | `dashscope` |
| `moonshot` | `moonshot` |
| `minimax` | `minimax` |
| `aihubmix` | `aihubmix` |

#### `ProviderConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `api_key` | `Secret<String>` | `""` | `apiKey` | Provider API key |
| `api_base` | `Option<String>` | `None` | `apiBase` | Custom API endpoint; skipped when `None` |
| `extra_headers` | `Option<HashMap<String, String>>` | `None` | `extraHeaders` | Additional HTTP headers; skipped when `None` |
| `native` | `bool` | `false` | `native` | Use native API format (e.g., Anthropic Messages API) |
| `cache_system_prompt` | `bool` | `false` | `cacheSystemPrompt` | Enable prompt caching (Anthropic-specific) |
| `extended_thinking` | `Option<ExtendedThinkingConfig>` | `None` | `extendedThinking` | Chain-of-thought configuration; skipped when `None` |
| `api_version` | `Option<String>` | `None` | `apiVersion` | API version header (Anthropic-specific); skipped when `None` |

#### `ExtendedThinkingConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | -- | `enabled` | Enable extended thinking |
| `budget_tokens` | `usize` | `10000` | `budgetTokens` | Token budget for thinking |
| `use_for` | `Vec<String>` | `[]` | `useFor` | Task types that should use thinking |

#### `ProviderManagerConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `primary` | `Option<String>` | `None` | `primary` | Primary provider name |
| `fallback` | `Option<String>` | `None` | `fallback` | Fallback provider name |
| `classifier_model` | `Option<String>` | `None` | `classifierModel` | Model for complexity classifier |

---

### Channels

**File:** `crates/config/src/schema/channels.rs`

#### `ChannelsConfig`

Contains one field per chat platform. All default to their respective `Default` impls with `enabled: false`.

| Field | Type | JSON Key |
|-------|------|----------|
| `telegram` | `TelegramConfig` | `telegram` |
| `discord` | `DiscordConfig` | `discord` |
| `whatsapp` | `WhatsAppConfig` | `whatsapp` |
| `slack` | `SlackConfig` | `slack` |
| `email` | `EmailConfig` | `email` |
| `qq` | `QQConfig` | `qq` |
| `feishu` | `FeishuConfig` | `feishu` |
| `dingtalk` | `DingTalkConfig` | `dingtalk` |
| `mochat` | `MochatConfig` | `mochat` |

#### `TelegramConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `token` | `Secret<String>` | `""` | `token` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |
| `proxy` | `Option<String>` | `None` | `proxy` |

#### `DiscordConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `token` | `Secret<String>` | `""` | `token` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |
| `gateway_url` | `String` | `"wss://gateway.discord.gg/?v=10&encoding=json"` | `gatewayUrl` |
| `intents` | `u32` | `46593` | `intents` |

The default intents bitmask includes: `GUILD_MESSAGES`, `GUILD_MESSAGE_REACTIONS`, `DIRECT_MESSAGES`, `DIRECT_MESSAGE_REACTIONS`, and `MESSAGE_CONTENT`.

#### `WhatsAppConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `bridge_url` | `String` | `"ws://localhost:3001"` | `bridgeUrl` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

#### `SlackConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `bot_token` | `Secret<String>` | `""` | `botToken` |
| `app_token` | `Secret<String>` | `""` | `appToken` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |
| `mode` | `String` | `"socket"` | `mode` |
| `group_policy` | `String` | `"none"` | `groupPolicy` |
| `group_allow_from` | `Vec<String>` | `[]` | `groupAllowFrom` |
| `dm` | `SlackDmConfig` | `SlackDmConfig::default()` | `dm` |

#### `SlackDmConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

#### `EmailConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `imap_host` | `String` | `""` | `imapHost` |
| `imap_port` | `u16` | `993` | `imapPort` |
| `imap_username` | `String` | `""` | `imapUsername` |
| `imap_password` | `Secret<String>` | `""` | `imapPassword` |
| `imap_mailbox` | `String` | `"INBOX"` | `imapMailbox` |
| `imap_use_ssl` | `bool` | `true` | `imapUseSsl` |
| `smtp_host` | `String` | `""` | `smtpHost` |
| `smtp_port` | `u16` | `587` | `smtpPort` |
| `smtp_username` | `String` | `""` | `smtpUsername` |
| `smtp_password` | `Secret<String>` | `""` | `smtpPassword` |
| `smtp_use_tls` | `bool` | `true` | `smtpUseTls` |
| `smtp_use_ssl` | `bool` | `false` | `smtpUseSsl` |
| `from_address` | `String` | `""` | `fromAddress` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |
| `consent_granted` | `bool` | `false` | `consentGranted` |
| `auto_reply_enabled` | `bool` | `true` | `autoReplyEnabled` |
| `max_body_chars` | `u32` | `12000` | `maxBodyChars` |
| `mark_seen` | `bool` | `true` | `markSeen` |
| `poll_interval_seconds` | `u32` | `30` | `pollIntervalSeconds` |
| `subject_prefix` | `String` | `"Re: "` | `subjectPrefix` |

#### `QQConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `app_id` | `String` | `""` | `appId` |
| `secret` | `Secret<String>` | `""` | `secret` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

#### `FeishuConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `app_id` | `String` | `""` | `appId` |
| `app_secret` | `Secret<String>` | `""` | `appSecret` |
| `encrypt_key` | `Secret<String>` | `""` | `encryptKey` |
| `verification_token` | `String` | `""` | `verificationToken` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

#### `DingTalkConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `client_id` | `String` | `""` | `clientId` |
| `client_secret` | `Secret<String>` | `""` | `clientSecret` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

#### `MochatConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `base_url` | `String` | `"https://mochat.io"` | `baseUrl` |
| `socket_url` | `String` | `""` | `socketUrl` |
| `claw_token` | `Secret<String>` | `""` | `clawToken` |
| `agent_user_id` | `String` | `""` | `agentUserId` |
| `sessions` | `Vec<String>` | `[]` | `sessions` |
| `panels` | `Vec<String>` | `[]` | `panels` |
| `allow_from` | `Vec<String>` | `[]` | `allowFrom` |

---

### Calendar

**File:** `crates/config/src/schema/calendar.rs`

#### `CalendarConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `providers` | `Vec<CalendarProviderConfig>` | `[]` | `providers` | List of calendar providers |
| `conflict_resolution` | `String` | `"server_wins"` | `conflictResolution` | Conflict resolution strategy |
| `bidirectional_sync` | `bool` | `true` | `bidirectionalSync` | Enable bidirectional reconciliation |

**Helper methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `is_any_enabled()` | `bool` | Any provider enabled? |
| `enabled_providers()` | `Vec<&CalendarProviderConfig>` | All enabled providers |
| `find_provider(id)` | `Option<&CalendarProviderConfig>` | Find by provider ID |
| `apple()` / `apple_mut()` | `Option<&AppleCalendarConfig>` | Apple provider accessor |
| `google()` / `google_mut()` | `Option<&GoogleCalendarConfig>` | Google provider accessor |
| `ensure_apple_mut()` | `&mut AppleCalendarConfig` | Get or create Apple config |
| `ensure_google_mut()` | `&mut GoogleCalendarConfig` | Get or create Google config |
| `min_sync_interval_secs()` | `u64` | Minimum sync interval across all enabled providers |

#### `CalendarProviderConfig` (tagged enum)

Serialized with `#[serde(tag = "type")]`. Variants:

| Variant | Tag value | Inner type |
|---------|-----------|------------|
| `Apple` | `"apple"` | `AppleCalendarConfig` |
| `Google` | `"google"` | `GoogleCalendarConfig` |
| `GenericCalDav` | `"genericCaldav"` | `GenericCalDavConfig` |

#### `AppleCalendarConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `username` | `String` | `""` | `username` |
| `password` | `Secret<String>` | `""` | `password` |
| `caldav_url` | `String` | `"https://caldav.icloud.com"` | `caldavUrl` |
| `calendar_name` | `String` | `"Personal"` | `calendarName` |
| `sync_interval_secs` | `u64` | `300` | `syncIntervalSecs` |
| `auto_sync_due_dates` | `bool` | `true` | `autoSyncDueDates` |

#### `GoogleCalendarConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `client_id` | `String` | `""` | `clientId` |
| `client_secret` | `Secret<String>` | `""` | `clientSecret` |
| `access_token` | `Secret<String>` | `""` | `accessToken` |
| `refresh_token` | `Secret<String>` | `""` | `refreshToken` |
| `calendar_id` | `String` | `"primary"` | `calendarId` |
| `calendar_name` | `String` | `"Personal"` | `calendarName` |
| `sync_interval_secs` | `u64` | `300` | `syncIntervalSecs` |
| `auto_sync_due_dates` | `bool` | `true` | `autoSyncDueDates` |

#### `GenericCalDavConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `false` | `enabled` |
| `name` | `String` | (required) | `name` |
| `caldav_url` | `String` | (required) | `caldavUrl` |
| `username` | `String` | `""` | `username` |
| `password` | `Secret<String>` | `""` | `password` |
| `calendar_name` | `String` | `"Personal"` | `calendarName` |
| `sync_interval_secs` | `u64` | `300` | `syncIntervalSecs` |
| `auto_sync_due_dates` | `bool` | `true` | `autoSyncDueDates` |

---

### Todo

**File:** `crates/config/src/schema/todo.rs`

#### `TodoConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `notifications` | `TodoNotificationConfig` | see below | `notifications` | Notification settings |
| `focus` | `TodoFocusConfig` | see below | `focus` | Focus mode settings |
| `enrichment` | `TodoEnrichmentConfig` | see below | `enrichment` | Auto-enrichment engine |
| `search` | `TodoSearchConfig` | see below | `search` | Semantic search settings |
| `daily_planning` | `DailyPlanningConfig` | see below | `dailyPlanning` | Daily planning trigger |
| `auto_plan_suggestion` | `bool` | `true` | `autoPlanSuggestion` | Suggest plans for complex tasks |
| `auto_plan_on_focus` | `bool` | `false` | `autoPlanOnFocus` | Auto-generate plan when focused |
| `plan_complexity_threshold` | `u8` | `3` | `planComplexityThreshold` | Score threshold for plan suggestions |

#### `TodoEnrichmentConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable auto-enrichment |
| `auto_apply_threshold` | `f64` | `0.85` | `autoApplyThreshold` | Confidence threshold for auto-apply |
| `use_llm` | `bool` | `false` | `useLlm` | Use LLM instead of keyword matching |

#### `TodoNotificationConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `targets` | `Vec<String>` | `["os_native"]` | `targets` |
| `focus_reminders` | `bool` | `true` | `focusReminders` |
| `daily_digest` | `bool` | `true` | `dailyDigest` |
| `daily_digest_time` | `String` | `"09:00"` | `dailyDigestTime` |

#### `TodoFocusConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `max_slots` | `usize` | `3` | `maxSlots` |
| `deadline_hours` | `u64` | `18` | `deadlineHours` |

#### `TodoSearchConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable semantic search |
| `semantic_threshold` | `f64` | `0.5` | `semanticThreshold` | Cosine similarity threshold |
| `embedding_model` | `String` | `"paraphrase-multilingual-MiniLM-L12-v2"` | `embeddingModel` | Model name for embeddings |
| `rrf_k` | `u32` | `60` | `rrfK` | Reciprocal Rank Fusion k parameter |

#### `DailyPlanningConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `true` | `enabled` |
| `planning_time` | `String` | `"08:00"` | `planningTime` |

---

### Tools

**File:** `crates/config/src/schema/tools.rs`

#### `ToolsConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `web` | `WebToolsConfig` | see below | `web` | Web tool settings |
| `browser` | `BrowserConfig` | see below | `browser` | Browser automation settings |
| `restrict_to_workspace` | `bool` | `false` | `restrictToWorkspace` | Restrict file tools to workspace |
| `permissions` | `Option<PermissionsConfig>` | `None` | `permissions` | Per-channel tool permissions |

#### `WebToolsConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `brave_api_key` | `Secret<String>` | `""` | `braveApiKey` |
| `max_results` | `u8` | `5` | `maxResults` |

#### `BrowserConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `enabled` | Enable browser automation |
| `trust_level` | `TrustLevel` | `Autonomous` | `trustLevel` | Write action confirmation gate |
| `session_timeout_secs` | `u64` | `300` | `sessionTimeoutSecs` | Session timeout in seconds |

#### `TrustLevel` (enum)

Serialized as camelCase strings.

| Variant | JSON Value | Description |
|---------|-----------|-------------|
| `Strict` | `"strict"` | Ask before every write action |
| `Autonomous` | `"autonomous"` | Ask only for dangerous actions (default) |
| `Full` | `"full"` | Execute all actions without confirmation |

#### `PermissionsConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `default_level` | `String` | `"standard"` | `defaultLevel` | Default permission level |
| `channels` | `HashMap<String, String>` | `{}` | `channels` | Per-channel overrides |

---

### Gateway

**File:** `crates/config/src/schema/gateway.rs`

#### `GatewayConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `host` | `String` | `"127.0.0.1"` | `host` |
| `port` | `u16` | `18790` | `port` |

---

### Conversation

**File:** `crates/config/src/schema/conversation.rs`

#### `ConversationConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `embedding` | `ConversationEmbeddingConfig` | see below | `embedding` |
| `search` | `ConversationSearchConfig` | see below | `search` |
| `session` | `SessionConfig` | see below | `session` |
| `memory` | `MemoryConfig` | see below | `memory` |

#### `ConversationEmbeddingConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable conversation embedding |
| `exclude_channels` | `Vec<String>` | `[]` | `excludeChannels` | Channels to skip |
| `exclude_roles` | `Vec<String>` | `["system", "tool"]` | `excludeRoles` | Roles to skip |

#### `ConversationSearchConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable conversation search |
| `semantic_threshold` | `f64` | `0.5` | `semanticThreshold` | Cosine similarity threshold |
| `max_results` | `usize` | `20` | `maxResults` | Max results returned |

#### `SessionConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `history_limit` | `usize` | `50` | `historyLimit` | Max history messages loaded |
| `ttl_days` | `u32` | `30` | `ttlDays` | Days before stale session deletion |
| `cleanup_interval_hours` | `u32` | `1` | `cleanupIntervalHours` | Cleanup service frequency |

#### `MemoryConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `decay_half_life_days` | `u32` | `138` | `decayHalfLifeDays` | Half-life for time-decay scoring (0.995^138 ~ 0.5) |
| `max_age_days` | `u32` | `90` | `maxAgeDays` | Max embedding age before pruning |
| `consolidation_enabled` | `bool` | `false` | `consolidationEnabled` | Enable background consolidation |
| `maintenance_interval_hours` | `u32` | `24` | `maintenanceIntervalHours` | Maintenance service frequency |

---

### Confidence

**File:** `crates/config/src/schema/confidence.rs`

#### `ConfidenceConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `threshold` | `f32` | `0.7` | `threshold` | Below this, ask_user is triggered |
| `enabled` | `bool` | `true` | `enabled` | Enable confidence evaluation |
| `tool_overrides` | `HashMap<String, f32>` | `{}` | `toolOverrides` | Per-tool threshold overrides |

---

### Learning

**File:** `crates/config/src/schema/learning.rs`

#### `LearningConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable learning system |
| `analysis_interval_secs` | `u64` | `3600` | `analysisIntervalSecs` | Analysis loop interval (seconds) |
| `min_threshold` | `f32` | `0.4` | `minThreshold` | Lower bound for adaptive threshold |
| `max_threshold` | `f32` | `0.9` | `maxThreshold` | Upper bound for adaptive threshold |
| `min_outcomes_for_adaptation` | `usize` | `50` | `minOutcomesForAdaptation` | Min outcomes before adaptation |

---

### Finance

**File:** `crates/config/src/schema/finance.rs`

#### `FinanceConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `true` | `enabled` |
| `default_currency` | `String` | `"USD"` | `defaultCurrency` |
| `proactivity_level` | `String` | `"full"` | `proactivityLevel` |
| `inflation` | `FinanceInflationConfig` | see below | `inflation` |
| `expected_returns` | `FinanceExpectedReturnsConfig` | see below | `expectedReturns` |
| `budgeting` | `FinanceBudgetingConfig` | see below | `budgeting` |
| `price_refresh` | `FinancePriceRefreshConfig` | see below | `priceRefresh` |
| `scheduling` | `FinanceSchedulingConfig` | see below | `scheduling` |
| `categories` | `FinanceCategoryConfig` | see below | `categories` |

#### `FinanceInflationConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `rate` | `f64` | `3.3` | `rate` |
| `source` | `String` | `"manual"` | `source` |

#### `FinanceExpectedReturnsConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `stocks` | `f64` | `10.0` | `stocks` |
| `crypto` | `f64` | `15.0` | `crypto` |
| `real_estate` | `f64` | `8.0` | `realEstate` |
| `bonds` | `f64` | `5.0` | `bonds` |

#### `FinanceBudgetingConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `default_method` | `String` | `"standard"` | `defaultMethod` |
| `alert_threshold` | `u8` | `80` | `alertThreshold` |
| `six_jar_ratios` | `SixJarRatios` | see below | `sixJarRatios` |

#### `SixJarRatios`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `essentials` | `u8` | `55` | `essentials` |
| `savings` | `u8` | `10` | `savings` |
| `investment` | `u8` | `10` | `investment` |
| `education` | `u8` | `10` | `education` |
| `entertainment` | `u8` | `10` | `entertainment` |
| `charity` | `u8` | `5` | `charity` |

#### `FinancePriceRefreshConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `enabled` | `bool` | `true` | `enabled` |
| `interval_hours` | `u32` | `4` | `intervalHours` |
| `cache_ttl_minutes` | `u32` | `15` | `cacheTtlMinutes` |

#### `FinanceSchedulingConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `daily_review_time` | `String` | `"21:00"` | `dailyReviewTime` |
| `weekly_report_day` | `String` | `"monday"` | `weeklyReportDay` |
| `budget_check_time` | `String` | `"09:00"` | `budgetCheckTime` |
| `timezone` | `Option<String>` | `None` | `timezone` |

#### `FinanceCategoryConfig`

| Field | Type | Default | JSON Key |
|-------|------|---------|----------|
| `auto_categorize` | `bool` | `true` | `autoCategorize` |
| `confidence_threshold` | `f64` | `0.8` | `confidenceThreshold` |

---

### Orchestrator

**File:** `crates/config/src/schema/orchestrator.rs`

#### `OrchestratorConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `heuristic_confidence_threshold` | `f32` | `0.85` | `heuristicConfidenceThreshold` | Threshold for accepting heuristic classification |
| `llm_classifier_timeout` | `u64` | `2000` | `llmClassifierTimeout` | LLM classifier timeout in ms |
| `llm_classifier_model` | `Option<String>` | `None` | `llmClassifierModel` | Override model for classifier; skipped when `None` |
| `default_plan_visibility` | `String` | `"on_failure"` | `defaultPlanVisibility` | Default visibility for auto-generated plans |
| `plan_complexity_threshold` | `u8` | `3` | `planComplexityThreshold` | Score threshold for planned execution (0-7) |
| `max_escalations` | `u32` | `1` | `maxEscalations` | Max escalations per request |

---

### Packs

**File:** `crates/config/src/schema/packs.rs`

#### `PacksConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `Vec<String>` | `["task-management", "productivity", "ai-intelligence", "developer-tools"]` | `enabled` | Enabled pack IDs |
| `enabled_skills` | `Vec<String>` | `[]` | `enabledSkills` | Skills computed from packs |

#### `PackTier` (enum)

Serialized as kebab-case strings.

| Variant | JSON Value |
|---------|-----------|
| `Core` | `"core"` |
| `Recommended` | `"recommended"` |
| `Optional` | `"optional"` |

---

### Plugins

**File:** `crates/config/src/schema/plugins.rs`

#### `PluginsConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable the plugin system |
| `registry_url` | `String` | `"https://plugins.klyntbot.io/index.json"` | `registryUrl` | Plugin registry URL |
| `sandbox_memory_mb` | `u32` | `64` | `sandboxMemoryMb` | Memory limit for plugin sandbox |
| `allow_network_by_default` | `bool` | `false` | `allowNetworkByDefault` | Allow plugins network access by default |

---

### Project

**File:** `crates/config/src/schema/project.rs`

#### `ProjectConfig`

| Field | Type | Default | JSON Key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable project management |
