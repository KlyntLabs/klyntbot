# config

## Purpose

The `config` crate is a Layer 1 crate that defines the complete configuration schema for Klyntbot and handles loading/saving the config file. It owns the `Config` root struct, which composes 16 section structs across 16 schema submodules. Configuration lives at `~/.klyntbot/config.json` and uses camelCase JSON keys. The crate also provides the `Secret<T>` wrapper for safe handling of API keys and tokens, and a minimal-diff save strategy that only writes fields that differ from defaults.

## Key Types

### `Secret<T>`

A wrapper type that redacts sensitive values in `Debug` and `Display` output. All API keys, tokens, and passwords throughout the config use `Secret<String>`.

- `Secret::new(value)` -- wrap a value
- `.expose()` -- access the inner value (the only way to read it)
- `.into_inner()` -- consume the wrapper
- `.is_empty()` -- check if the inner string is empty (convenience for `Secret<String>`)
- `Debug` and `Display` always print `[REDACTED]`
- Serializes transparently via `#[serde(transparent)]` -- the JSON contains the raw value, but log output never leaks it

### `Config` (Root Struct)

The root configuration struct composes all section configs. Every field uses `#[serde(default)]` so that partial JSON files deserialize successfully, filling in defaults for omitted sections. The struct derives `Default` so a zero-config startup is always possible.

Fields on `Config`:

| Field | Type | Purpose |
|-------|------|---------|
| `agents` | `AgentsConfig` | Default model, workspace path, temperature, token limits, tool iteration cap, subagent concurrency |
| `channels` | `ChannelsConfig` | Per-platform settings for Telegram, Discord, WhatsApp, Slack, Email, QQ, Feishu, DingTalk, Mochat |
| `providers` | `ProvidersConfig` | API keys and endpoints for 12 LLM providers |
| `tools` | `ToolsConfig` | Web search keys, browser automation, workspace restriction, per-channel permissions |
| `gateway` | `GatewayConfig` | HTTP server host and port (default: 127.0.0.1:18790) |
| `todo` | `TodoConfig` | Task focus mode, notifications, enrichment, semantic search, daily planning, plan complexity |
| `confidence` | `ConfidenceConfig` | Threshold for triggering ask_user, per-tool overrides |
| `calendar` | `CalendarConfig` | Multi-provider CalDAV sync (Apple, Google, Generic), conflict resolution, bidirectional sync |
| `project` | `ProjectConfig` | Project management enabled flag |
| `conversation` | `ConversationConfig` | Embedding, search, session history limits, memory decay and consolidation |
| `learning` | `LearningConfig` | Adaptive confidence threshold system (analysis interval, bounds, minimum outcomes) |
| `finance` | `FinanceConfig` | Currency, budgeting method, inflation, expected returns, price refresh, scheduling, categories |
| `orchestrator` | `OrchestratorConfig` | Intent pipeline tuning (heuristic threshold, classifier timeout, plan visibility, escalation limits) |
| `provider_manager` | `ProviderManagerConfig` | Primary/fallback/classifier provider routing |
| `timezone` | `String` | Auto-detected IANA timezone (fallback: UTC) |
| `data_dir` | `Option<String>` | Override for storage directory (default: `~/.klyntbot`) |
| `packs` | `PacksConfig` | Enabled feature packs and derived skill list |
| `plugins` | `PluginsConfig` | Plugin system toggle, registry URL, sandbox memory limit, network policy |

Helper methods on `Config`:
- `workspace_path()` -- expands `~` in the workspace path to the home directory
- `data_dir_path()` -- resolves the data directory with tilde expansion
- `active_provider_name()` -- detects which LLM provider to use (explicit field first, then auto-detect from configured API keys)
- `is_provider_configured(name)` -- checks if a provider has a non-empty API key
- `set_provider_key(name, key)` -- sets an API key by provider name

### Config Sections in Detail

**`AgentsConfig` / `AgentDefaults`** -- controls the agent's LLM behavior. Defaults: model `anthropic/claude-opus-4-5`, temperature 0.7, max tokens 8192, max tool iterations 20, max concurrent subagents 3, workspace `~/.klyntbot/workspace`. An optional explicit `provider` field overrides model-name-based auto-detection.

**`ProvidersConfig`** -- holds a `ProviderConfig` for each of 12 supported providers: Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini, Groq, vLLM, Zhipu, DashScope, Moonshot, MiniMax, and AIHubMix. Each `ProviderConfig` has an `api_key`, optional `api_base` URL override, optional `extra_headers`, a `native` flag (for provider-specific API formats), `cache_system_prompt` (Anthropic prompt caching), optional `extended_thinking` config (budget tokens, task types), and an optional `api_version` override.

**`ChannelsConfig`** -- contains configs for 9 chat platforms. Each channel config follows a common pattern: `enabled` flag, authentication credentials (wrapped in `Secret`), and an `allow_from` list for access control. Platform-specific fields include Discord gateway URL/intents, WhatsApp bridge URL, Slack bot/app tokens and socket mode, Email IMAP/SMTP settings, QQ app ID, Feishu encrypt key, DingTalk client credentials, and Mochat socket/sessions/panels.

**`ToolsConfig`** -- controls tool behavior. `restrict_to_workspace` limits file operations. `WebToolsConfig` holds the Brave Search API key and max results. `BrowserConfig` controls browser automation with a `TrustLevel` enum (`Strict`/`Autonomous`/`Full`) and session timeout. Optional `PermissionsConfig` enables per-channel tool access control with levels like "readOnly", "standard", "elevated", "admin".

**`CalendarConfig`** -- supports multiple simultaneous calendar providers via a `Vec<CalendarProviderConfig>`. The provider enum is internally tagged (`"type": "apple"` / `"google"` / `"genericCaldav"`). Each provider variant has its own credential fields (Apple uses username/password with iCloud CalDAV, Google uses OAuth2 tokens, Generic uses configurable CalDAV URL). Shared settings include sync interval (default: 5 minutes), auto-sync of due dates, calendar name, conflict resolution strategy, and bidirectional sync.

**`TodoConfig`** -- manages the task system. Sub-configs include focus mode (max slots, deadline hours), notification targets and timing, enrichment (keyword-based metadata inference with confidence threshold), semantic search (embedding model, similarity threshold, RRF parameter), and daily planning (time trigger). Also controls automatic plan suggestion for complex tasks.

**`FinanceConfig`** -- configures the personal finance system. Sub-configs cover inflation assumptions, expected returns by asset class, budgeting method (standard or six-jar with configurable ratios), price refresh intervals, scheduling (daily review, weekly report, budget check times), and auto-categorization confidence.

**`ConversationConfig`** -- controls conversation memory. Sub-configs for embedding (which channels/roles to exclude), search (threshold, max results), session management (history limit, TTL, cleanup interval), and memory decay (half-life for time-weighted scoring, max age for pruning, optional consolidation).

**`OrchestratorConfig`** -- tunes the intent pipeline. Controls heuristic confidence threshold (0.85 default), LLM classifier timeout (2000ms), default plan visibility ("on_failure"), plan complexity threshold (3), and max escalations (1).

**`PacksConfig`** -- tracks enabled feature packs and the skills they contribute. Default packs: task-management, productivity, ai-intelligence, developer-tools. `PackTier` enum (Core/Recommended/Optional) controls wizard presentation.

**`PluginsConfig`** -- configures the WASM plugin system: enabled flag, registry URL, sandbox memory limit (64MB default), and network access policy.

## How It Works

### Config Loading

The `loader.rs` module provides both async (`load()`, `save()`) and sync (`load_sync()`, `save_sync()`) variants for config I/O.

**Loading flow:**
1. `config_path()` resolves to `~/.klyntbot/config.json` via `dirs::home_dir()`.
2. If the file exists, it is read and deserialized via `serde_json::from_str()`. Missing fields get their `#[serde(default)]` values.
3. If the file does not exist, `Config::default()` is returned (zero-config startup).

**Saving flow (minimal diff):**
1. The full config is serialized to a `serde_json::Value`.
2. A fresh `Config::default()` is also serialized.
3. `diff_json()` recursively compares the two, producing a minimal JSON object containing only fields that differ from defaults.
4. Empty objects are pruned (if all children match defaults, the parent is omitted).
5. The minimal JSON is pretty-printed and written to disk.

This means a user who only configures one provider and one channel will see a tiny config file with just those settings, rather than hundreds of lines of defaults.

### Environment Variable Overrides

`load_with_env_overrides()` loads the config file first, then applies overrides from environment variables. The convention is:

- Prefix: `KLYNTBOT_`
- Nesting separator: `__` (double underscore)
- Example: `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...`

Three internal macros handle different value types:
- `env_string!` -- direct string assignment
- `env_secret!` -- wraps the value in `Secret::new()`
- `env_parse!` -- parses typed values (f32, u32, etc.)

Currently, overrides are defined for agent defaults (model, workspace, temperature, max tokens), all 12 provider API keys, data directory, channel tokens (Telegram, Discord, Slack), and web tool keys (Brave).

### camelCase Convention

All config structs use `#[serde(rename_all = "camelCase")]`. This means Rust struct fields use `snake_case` internally (e.g., `max_tokens`, `api_key`), but the JSON file uses `camelCase` (e.g., `maxTokens`, `apiKey`). This is enforced consistently across all 16 schema modules. Snake_case keys in JSON will fail to deserialize.

### Initialization

`init()` creates the directory structure (`~/.klyntbot/`, `~/.klyntbot/sessions/`, `~/.klyntbot/workspace/`) and saves a default config if none exists. This is called by the `klyntbot init` CLI command.

## Connections

**Depends on:** `common` (for `ConfigError`, `Result`).

**Depended on by:** Nearly every crate in the workspace reads config at startup. The `agent` crate uses it to configure the LLM provider, tool registry, and intent pipeline. The `channels` crate reads channel-specific configs. The `cli` crate uses it for the setup wizard. The `providers` crate reads API keys and endpoints. The `storage` crate uses `data_dir` to locate the database. The `tools` crate checks permissions and workspace restrictions.
