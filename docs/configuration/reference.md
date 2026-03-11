# Configuration Reference

Definitive reference for every configuration field in klyntbot. Generated from the Rust schema source files in `crates/config/src/schema/`.

## Table of Contents

- [Overview](#overview)
- [Config File Lifecycle](#config-file-lifecycle)
- [Environment Variable Overrides](#environment-variable-overrides)
- [Secret Handling](#secret-handling)
- [Root Config Fields](#root-config-fields)
- [agents](#agents)
- [providers](#providers)
- [providerManager](#providermanager)
- [channels](#channels)
  - [channels.telegram](#channelstelegram)
  - [channels.discord](#channelsdiscord)
  - [channels.slack](#channelsslack)
  - [channels.email](#channelsemail)
  - [channels.feishu](#channelsfeishu)
  - [channels.dingtalk](#channelsdingtalk)
  - [channels.mochat](#channelsmochat)
- [tools](#tools)
- [gateway](#gateway)
- [todo](#todo)
- [finance](#finance)
- [confidence](#confidence)
- [project](#project)
- [conversation](#conversation)
- [learning](#learning)
- [productivity](#productivity)
- [orchestrator](#orchestrator)
- [cognitive](#cognitive)
- [workContext](#workcontext)
- [mcp](#mcp)
- [packs](#packs)
- [plugins](#plugins)
- [capture](#capture)
- [content](#content)
- [Minimal Config Examples](#minimal-config-examples)

---

## Overview

Klyntbot uses a single JSON configuration file with **camelCase** field names.

- **Location:** `~/.klyntbot/config.json`
- **Format:** JSON with camelCase keys (enforced by `#[serde(rename_all = "camelCase")]`)
- **All fields have defaults.** An empty `{}` is a valid config file. You only need to specify values that differ from defaults.
- **Partial configs work.** Missing sections or fields are filled with their default values during deserialization.

---

## Config File Lifecycle

1. **Load** -- The config file is read from `~/.klyntbot/config.json`. If absent, `Config::default()` is used.
2. **Environment variable overlay** -- The `load_with_env_overrides()` function applies `KLYNTBOT_*` environment variables on top of the file-loaded config.
3. **Use** -- The fully-resolved `Config` struct is available throughout the application.

**Saving:** When saving, `diff_json()` compares the current config against `Config::default()` and writes only the fields that differ. This keeps the on-disk config minimal and human-readable. Default values are never persisted.

---

## Environment Variable Overrides

Environment variables use the prefix `KLYNTBOT_` with double underscores (`__`) for nesting. They are applied after the config file is loaded, so they always take priority.

### Complete List of Supported Environment Variables

| Environment Variable | Type | Config Path |
|---|---|---|
| `KLYNTBOT_AGENTS__DEFAULTS__MODEL` | String | `agents.defaults.model` |
| `KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE` | String | `agents.defaults.workspace` |
| `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE` | f32 | `agents.defaults.temperature` |
| `KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS` | u32 | `agents.defaults.maxTokens` |
| `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY` | Secret | `providers.anthropic.apiKey` |
| `KLYNTBOT_PROVIDERS__OPENAI__API_KEY` | Secret | `providers.openai.apiKey` |
| `KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY` | Secret | `providers.openrouter.apiKey` |
| `KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY` | Secret | `providers.deepseek.apiKey` |
| `KLYNTBOT_PROVIDERS__GEMINI__API_KEY` | Secret | `providers.gemini.apiKey` |
| `KLYNTBOT_PROVIDERS__GROQ__API_KEY` | Secret | `providers.groq.apiKey` |
| `KLYNTBOT_PROVIDERS__VLLM__API_KEY` | Secret | `providers.vllm.apiKey` |
| `KLYNTBOT_PROVIDERS__ZHIPU__API_KEY` | Secret | `providers.zhipu.apiKey` |
| `KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY` | Secret | `providers.dashscope.apiKey` |
| `KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY` | Secret | `providers.moonshot.apiKey` |
| `KLYNTBOT_PROVIDERS__MINIMAX__API_KEY` | Secret | `providers.minimax.apiKey` |
| `KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY` | Secret | `providers.aihubmix.apiKey` |
| `KLYNTBOT_DATA_DIR` | String | `dataDir` |
| `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN` | Secret | `channels.telegram.token` |
| `KLYNTBOT_CHANNELS__DISCORD__TOKEN` | Secret | `channels.discord.token` |
| `KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN` | Secret | `channels.slack.botToken` |
| `KLYNTBOT_CHANNELS__SLACK__APP_TOKEN` | Secret | `channels.slack.appToken` |
| `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY` | Secret | `tools.web.braveApiKey` |

---

## Secret Handling

The `Secret<T>` wrapper type is used for sensitive fields (API keys, tokens, passwords).

- **Serialization:** `Secret<T>` is `#[serde(transparent)]` -- it serializes/deserializes as the inner value. API keys are stored as **plaintext** in the JSON config file.
- **Debug/Display:** Both `Debug` and `Display` emit `[REDACTED]` instead of the actual value, preventing accidental exposure in logs.
- **Access:** Use `.expose()` to get a reference to the inner value, or `.into_inner()` to consume the wrapper.
- **Warning:** Config values are stored in plaintext at rest in `~/.klyntbot/config.json`. Protect the file with appropriate filesystem permissions.

---

## Root Config Fields

These fields sit at the top level of the config JSON object.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `timezone` | String | Auto-detected system timezone, fallback `"UTC"` | IANA timezone identifier |
| `dataDir` | String? | `null` (resolves to `~/.klyntbot`) | Data directory for SQLite + LanceDB storage files |
| `setupCompleted` | bool | `false` | Whether the first-run setup wizard has been completed |

All other top-level fields are section objects documented below.

---

## agents

JSON path: `agents`

### agents (top-level)

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `defaults` | object | See below | Default agent parameters |
| `monthlyBudgetUsd` | f64? | `null` | Monthly LLM cost budget in USD. Warnings emitted at 80% and 100% |
| `skillsDir` | String? | `null` (resolves to `~/.klyntbot/.agents/skills/`) | Directory for runtime-loaded external skills |

### agents.defaults

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `workspace` | String | `"~/.klyntbot/workspace"` | Workspace directory path (supports `~` expansion) |
| `model` | String | `"anthropic/claude-opus-4-5"` | Default LLM model identifier |
| `provider` | String? | `null` | Explicit active provider name (e.g., `"anthropic"`, `"deepseek"`). Takes priority over model-name auto-detection when set |
| `maxTokens` | u32 | `8192` | Maximum output tokens per LLM call |
| `temperature` | f32 | `0.7` | Sampling temperature |
| `maxToolIterations` | u32 | `20` | Maximum ReAct loop iterations (tool call rounds) |
| `maxConcurrentSubagents` | usize | `3` | Maximum number of concurrent sub-agent executions |

---

## providers

JSON path: `providers`

Contains 12 provider sub-objects, each with identical structure. The active provider is determined by: (1) explicit `agents.defaults.provider` field, then (2) auto-detection from the first provider with a non-empty API key, in the order listed below.

### Provider List (detection priority order)

1. `anthropic`
2. `openai`
3. `openrouter`
4. `deepseek`
5. `gemini`
6. `groq`
7. `vllm`
8. `zhipu`
9. `dashscope`
10. `moonshot`
11. `minimax`
12. `aihubmix`

### Per-Provider Fields

Each provider object (`providers.anthropic`, `providers.openai`, etc.) has this structure:

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `apiKey` | Secret\<String\> | `""` | API key for authentication |
| `apiBase` | String? | `null` | Custom API base URL override |
| `extraHeaders` | Map\<String, String\>? | `null` | Additional HTTP headers to include in requests |
| `native` | bool | `false` | Use native API format (e.g., Anthropic Messages API instead of OpenAI-compatible) |
| `cacheSystemPrompt` | bool | `false` | Enable prompt caching for system prompts (Anthropic-specific) |
| `extendedThinking` | object? | `null` | Extended thinking / chain-of-thought configuration |
| `apiVersion` | String? | `null` | API version header override (Anthropic-specific, e.g., `"2023-06-01"`) |

### providers.\*.extendedThinking

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | *(required)* | Whether extended thinking is active |
| `budgetTokens` | usize | `10000` | Token budget allocated for thinking |
| `useFor` | String[] | `[]` | Task types that should use extended thinking (e.g., `["planning", "debugging"]`) |

---

## providerManager

JSON path: `providerManager`

Routing configuration for primary/fallback/classifier provider selection.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `primary` | String? | `null` | Name of the primary provider (e.g., `"anthropic"`) |
| `fallback` | String? | `null` | Name of the fallback provider (e.g., `"openai"`) |
| `classifierModel` | String? | `null` | Model name for the complexity classifier |

---

## channels

JSON path: `channels`

Contains 7 channel sub-objects. All channels are disabled by default.

### channels.telegram

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Telegram channel |
| `token` | Secret\<String\> | `""` | Telegram bot token |
| `allowFrom` | String[] | `[]` | List of allowed user IDs or usernames |
| `proxy` | String? | `null` | SOCKS5/HTTP proxy URL (e.g., `"socks5://localhost:1080"`) |

### channels.discord

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Discord channel |
| `token` | Secret\<String\> | `""` | Discord bot token |
| `allowFrom` | String[] | `[]` | List of allowed user IDs |
| `gatewayUrl` | String | `"wss://gateway.discord.gg/?v=10&encoding=json"` | Discord gateway WebSocket URL |
| `intents` | u32 | `46593` | Discord gateway intents bitmask (GUILD_MESSAGES, DIRECT_MESSAGES, MESSAGE_CONTENT, etc.) |

### channels.slack

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Slack channel |
| `botToken` | Secret\<String\> | `""` | Slack bot token (`xoxb-...`) |
| `appToken` | Secret\<String\> | `""` | Slack app-level token (`xapp-...`) for Socket Mode |
| `allowFrom` | String[] | `[]` | List of allowed user IDs |
| `mode` | String | `"socket"` | Connection mode (`"socket"` for Socket Mode) |
| `groupPolicy` | String | `"none"` | Group/channel message policy |
| `groupAllowFrom` | String[] | `[]` | Allowed groups/channels |
| `dm` | object | See below | DM-specific configuration |

#### channels.slack.dm

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Slack DM handling |
| `allowFrom` | String[] | `[]` | Allowed user IDs for DMs |

### channels.email

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable email channel |
| `imapHost` | String | `""` | IMAP server hostname |
| `imapPort` | u16 | `993` | IMAP server port |
| `imapUsername` | String | `""` | IMAP login username |
| `imapPassword` | Secret\<String\> | `""` | IMAP login password |
| `imapMailbox` | String | `"INBOX"` | IMAP mailbox to monitor |
| `imapUseSsl` | bool | `true` | Use SSL for IMAP connection |
| `smtpHost` | String | `""` | SMTP server hostname |
| `smtpPort` | u16 | `587` | SMTP server port |
| `smtpUsername` | String | `""` | SMTP login username |
| `smtpPassword` | Secret\<String\> | `""` | SMTP login password |
| `smtpUseTls` | bool | `true` | Use TLS for SMTP connection |
| `smtpUseSsl` | bool | `false` | Use SSL for SMTP connection |
| `fromAddress` | String | `""` | Sender email address for outgoing messages |
| `allowFrom` | String[] | `[]` | Allowed sender email addresses |
| `consentGranted` | bool | `false` | Whether user consented to email processing |
| `autoReplyEnabled` | bool | `true` | Enable automatic email replies |
| `maxBodyChars` | u32 | `12000` | Maximum email body characters to process |
| `markSeen` | bool | `true` | Mark processed emails as seen in IMAP |
| `pollIntervalSeconds` | u32 | `30` | How often to poll for new emails (seconds) |
| `subjectPrefix` | String | `"Re: "` | Prefix for reply subject lines |

### channels.feishu

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Feishu/Lark channel |
| `appId` | String | `""` | Feishu app ID |
| `appSecret` | Secret\<String\> | `""` | Feishu app secret |
| `encryptKey` | Secret\<String\> | `""` | Message encryption key |
| `verificationToken` | String | `""` | Webhook verification token |
| `allowFrom` | String[] | `[]` | Allowed user IDs |

### channels.dingtalk

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable DingTalk channel |
| `clientId` | String | `""` | DingTalk client ID |
| `clientSecret` | Secret\<String\> | `""` | DingTalk client secret |
| `allowFrom` | String[] | `[]` | Allowed user IDs |

### channels.mochat

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable Mochat channel |
| `baseUrl` | String | `"https://mochat.io"` | Mochat API base URL |
| `socketUrl` | String | `""` | WebSocket URL for real-time messaging |
| `clawToken` | Secret\<String\> | `""` | Claw authentication token |
| `agentUserId` | String | `""` | Agent's user ID in the Mochat system |
| `sessions` | String[] | `[]` | Session IDs to monitor |
| `panels` | String[] | `[]` | Panel IDs to monitor |
| `allowFrom` | String[] | `[]` | Allowed user IDs |

---

## tools

JSON path: `tools`

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `restrictToWorkspace` | bool | `false` | Restrict filesystem tools to the workspace directory |
| `web` | object | See below | Web search tool configuration |
| `browser` | object | See below | Browser automation tool configuration |
| `permissions` | object? | `null` | Per-channel tool permission levels. When absent, all tools are allowed on all channels |

### tools.web

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `braveApiKey` | Secret\<String\> | `""` | Brave Search API key |
| `maxResults` | u8 | `5` | Maximum search results to return |

### tools.browser

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable browser automation tool |
| `trustLevel` | String | `"autonomous"` | Write-action guard level: `"strict"` (ask before every write), `"autonomous"` (ask for dangerous actions only), `"full"` (no confirmation) |
| `sessionTimeoutSecs` | u64 | `300` | Browser session timeout in seconds |

### tools.permissions

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `defaultLevel` | String | `"standard"` | Default permission level for channels not explicitly listed. Values: `"readOnly"`, `"standard"`, `"elevated"`, `"admin"` |
| `channels` | Map\<String, String\> | `{}` | Per-channel permission level overrides. Keys are channel names (e.g., `"telegram"`, `"discord"`, `"cli"`) |

---

## gateway

JSON path: `gateway`

HTTP server configuration for the gateway API.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `host` | String | `"127.0.0.1"` | Listen address |
| `port` | u16 | `18790` | Listen port |

---

## todo

JSON path: `todo`

Task management system configuration.

### todo.notifications

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `targets` | String[] | `["os_native"]` | Notification delivery targets |
| `focusReminders` | bool | `true` | Enable focus mode reminders |
| `dailyDigest` | bool | `true` | Enable daily task digest |
| `dailyDigestTime` | String | `"09:00"` | Time for daily digest (HH:MM format) |

### todo.focus

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `maxSlots` | usize | `3` | Maximum number of concurrent focus slots |
| `deadlineHours` | u64 | `18` | Default deadline for focus tasks (hours) |

### todo.enrichment

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable automatic metadata enrichment on task creation |
| `autoApplyThreshold` | f64 | `0.85` | Confidence threshold for auto-applying suggestions without confirmation |
| `useLlm` | bool | `false` | Use LLM for enrichment instead of keyword matching |

### todo.search

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable semantic search for tasks |
| `semanticThreshold` | f64 | `0.5` | Cosine similarity threshold for semantic search results (0.0-1.0) |
| `embeddingModel` | String | `"paraphrase-multilingual-MiniLM-L12-v2"` | Embedding model name |
| `rrfK` | u32 | `60` | RRF k parameter for hybrid search |

### todo.dailyPlanning

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable daily planning feature |
| `planningTime` | String | `"08:00"` | Time to trigger daily planning (HH:MM format) |

---

## finance

JSON path: `finance`

Financial tracking and FIRE planning system.

### finance (top-level)

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable/disable the finance system |
| `defaultCurrency` | String | `"USD"` | Default ISO 4217 currency code |
| `proactivityLevel` | String | `"full"` | Proactivity level: `"full"`, `"moderate"`, or `"reactive"` |
| `exchangeRates` | Map\<String, f64\>? | `null` | Manual exchange rates mapping currency codes to VND equivalent (e.g., `{"USD": 25500}`) |

### finance.inflation

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `rate` | f64 | `3.3` | Annual inflation rate as a percentage |
| `source` | String | `"manual"` | Source of inflation data: `"manual"` or `"api"` |

### finance.expectedReturns

Expected annual return rates by asset class (as percentages).

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `stocks` | f64 | `10.0` | Expected annual stock return (%) |
| `crypto` | f64 | `15.0` | Expected annual crypto return (%) |
| `realEstate` | f64 | `8.0` | Expected annual real estate return (%) |
| `bonds` | f64 | `5.0` | Expected annual bond return (%) |

### finance.budgeting

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `defaultMethod` | String | `"standard"` | Budgeting method: `"standard"` or `"six_jar"` |
| `alertThreshold` | u8 | `80` | Percentage threshold for budget alerts (0-100) |
| `sixJarRatios` | object | See below | Six Jar allocation ratios |

#### finance.budgeting.sixJarRatios

Allocation percentages (should sum to 100).

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `essentials` | u8 | `55` | Essentials allocation (%) |
| `savings` | u8 | `10` | Savings allocation (%) |
| `investment` | u8 | `10` | Investment allocation (%) |
| `education` | u8 | `10` | Education allocation (%) |
| `entertainment` | u8 | `10` | Entertainment allocation (%) |
| `charity` | u8 | `5` | Charity allocation (%) |

### finance.priceRefresh

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable automatic price refresh |
| `intervalHours` | u32 | `4` | How often to refresh prices (hours) |
| `cacheTtlMinutes` | u32 | `15` | Price cache TTL (minutes) |

### finance.scheduling

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `dailyReviewTime` | String | `"21:00"` | Time for daily financial review (HH:MM) |
| `weeklyReportDay` | String | `"monday"` | Day of week for weekly report |
| `budgetCheckTime` | String | `"09:00"` | Time for daily budget check (HH:MM) |
| `timezone` | String? | `null` | IANA timezone override (falls back to system timezone) |

### finance.categories

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `autoCategorize` | bool | `true` | Enable automatic transaction categorization |
| `confidenceThreshold` | f64 | `0.8` | Minimum confidence to auto-apply a category (0.0-1.0) |

### finance.fire

FIRE (Financial Independence, Retire Early) configuration.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable FIRE tracking |
| `currentAge` | u32? | `null` | Current age |
| `targetRetirementAge` | u32? | `null` | Target retirement age |
| `annualExpenses` | i64? | `null` | Annual expenses in cents |
| `safeWithdrawalRate` | f64 | `4.0` | Safe withdrawal rate as percentage |
| `fireType` | String | `"regular"` | FIRE type: `"lean"`, `"regular"`, `"fat"`, or `"coast"` |
| `targetNumber` | i64? | `null` | Target FIRE number in cents (auto-calculated or manual override) |
| `monthlySavingsRate` | i64? | `null` | Monthly savings rate in cents |
| `currentNetWorth` | i64? | `null` | Snapshot of current net worth in cents |

---

## confidence

JSON path: `confidence`

LLM-driven confidence evaluation for tool usage decisions.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable/disable confidence evaluation |
| `threshold` | f32 | `0.7` | Threshold below which `ask_user` is triggered |
| `toolOverrides` | Map\<String, f32\> | `{}` | Per-tool confidence threshold overrides. Tools not listed fall back to the global `threshold` |

---

## project

JSON path: `project`

Project management configuration.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable project management features |

---

## conversation

JSON path: `conversation`

Conversation memory, embedding, and session management.

### conversation.embedding

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable automatic conversation embedding |
| `excludeChannels` | String[] | `[]` | Channels to exclude from conversation embedding |
| `excludeRoles` | String[] | `["system", "tool"]` | Message roles to exclude from embedding |

### conversation.search

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable conversation search |
| `semanticThreshold` | f64 | `0.5` | Semantic similarity threshold for search results (0.0-1.0) |
| `maxResults` | usize | `20` | Maximum number of search results to return |

### conversation.session

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `historyLimit` | usize | `50` | Maximum number of history messages to load |
| `maxCacheSize` | usize | `1000` | Maximum sessions in the in-memory cache |
| `ttlDays` | u32 | `30` | Days before an inactive session is considered stale and deleted |
| `cleanupIntervalHours` | u32 | `1` | How often the cleanup service runs (hours) |

### conversation.memory

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `decayHalfLifeDays` | u32 | `138` | Half-life in days for time-decay scoring (0.995^138 ~ 0.5) |
| `maxAgeDays` | u32 | `90` | Maximum age in days before an embedding is pruned |
| `consolidationEnabled` | bool | `false` | Enable background consolidation of old embeddings |
| `maintenanceIntervalHours` | u32 | `24` | How often the memory maintenance service runs (hours) |

---

## learning

JSON path: `learning`

Adaptive confidence threshold learning system.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable/disable the learning system |
| `analysisIntervalSecs` | u64 | `3600` | How often the background analysis loop runs (seconds) |
| `minThreshold` | f32 | `0.4` | Lower bound for adaptive threshold |
| `maxThreshold` | f32 | `0.9` | Upper bound for adaptive threshold |
| `minOutcomesForAdaptation` | usize | `50` | Minimum outcomes required before threshold adaptation kicks in |

---

## productivity

JSON path: `productivity`

Activity tracking, focus sessions, and productivity nudges.

### productivity (top-level)

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable productivity tracking system |

### productivity.tracking

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `pollIntervalSecs` | u64 | `5` | How often to poll for activity data (seconds) |
| `idleThresholdSecs` | u64 | `120` | Seconds of inactivity before marking as idle |
| `batchWriteIntervalSecs` | u64 | `30` | How often to flush activity data to storage (seconds) |
| `rawRetentionDays` | u64 | `7` | Days to keep raw activity data |
| `bucketRetentionDays` | u64 | `365` | Days to keep bucketed/aggregated activity data |

### productivity.focus

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `defaultDurationMins` | u64 | `45` | Default focus session duration (minutes) |
| `breakIntervalMins` | u64 | `90` | Interval between break reminders (minutes) |
| `breakDurationMins` | u64 | `10` | Suggested break duration (minutes) |
| `maxDailyFocusHours` | u64 | `8` | Maximum daily focus hours |
| `softBlockEnabled` | bool | `true` | Enable soft-blocking of distracting apps during focus |
| `softBlockCooldownSecs` | u64 | `60` | Cooldown between soft-block prompts (seconds) |
| `softBlockTempPassMins` | u64 | `5` | Temporary pass duration for soft-blocked apps (minutes) |
| `softBlockLlmEnabled` | bool | `true` | Use LLM to evaluate soft-block decisions |
| `softBlockLlmTimeoutMs` | u64 | `3000` | LLM timeout for soft-block evaluation (ms) |
| `learnedRuleThreshold` | u64 | `3` | Number of consistent decisions before learning a rule |
| `autoDetectEnabled` | bool | `true` | Enable automatic focus session detection |
| `autoDetectMinMins` | u64 | `15` | Minimum duration to auto-detect a focus session (minutes) |
| `autoDetectProductiveThreshold` | f64 | `0.8` | Productivity score threshold for auto-detection |
| `autoDetectMaxSwitches` | u64 | `2` | Maximum app switches allowed during auto-detected focus |
| `cooldownGraceSecs` | u64 | `120` | Grace period after focus session ends (seconds) |

### productivity.nudges

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `breakReminders` | bool | `true` | Enable break reminders |
| `focusSuggestions` | bool | `true` | Enable focus session suggestions |
| `dailySummary` | bool | `true` | Enable daily productivity summary |
| `burnoutAlerts` | bool | `true` | Enable burnout risk alerts |
| `cooldownMins` | u64 | `15` | Minimum time between nudges (minutes) |
| `quietHoursStart` | String? | `null` | Start of quiet hours (HH:MM format) |
| `quietHoursEnd` | String? | `null` | End of quiet hours (HH:MM format) |

### productivity.privacy

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `excludedApps` | String[] | `[]` | Application names to exclude from tracking |
| `excludeWindowTitles` | bool | `false` | Exclude window titles from activity data |
| `excludedUrlPatterns` | String[] | `[]` | URL patterns to exclude from browser tracking |

---

## orchestrator

JSON path: `orchestrator`

Intent pipeline orchestrator configuration.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `heuristicConfidenceThreshold` | f32 | `0.85` | Confidence threshold above which heuristic classification is accepted (0.0-1.0) |
| `llmClassifierTimeout` | u64 | `2000` | Timeout in milliseconds for the LLM classifier call |
| `llmClassifierModel` | String? | `null` | Override model for the LLM classifier (uses default agent model if null) |
| `maxEscalations` | u32 | `1` | Maximum escalations per request (Direct to Reactive) |
| `maxFabricationRetries` | u32 | `2` | Maximum fabrication retries before accepting fabricated content |
| `satisfactionWindowMinutes` | u64 | `15` | Reaction satisfaction window (minutes) |

---

## cognitive

JSON path: `cognitive`

Cognitive memory system configuration including fact extraction, consolidation, reflection, and FSRS-based ranking.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `model` | String? | `null` | Model for cognitive LLM calls. Falls back to `agents.defaults.model` |
| `provider` | String? | `null` | Provider name override. Falls back to `agents.defaults.provider` |
| `temperature` | f32? | `null` | Temperature for cognitive calls (recommended: low, e.g., 0.2) |
| `maxTokens` | u32? | `null` | Max tokens per cognitive call (default behavior: 1024) |
| `reflectionMaxTokens` | u32? | `null` | Max tokens for reflection calls (default behavior: 2048) |
| `reflectionSchedule` | String? | `null` | Cron expression for weekly reflection (e.g., `"0 9 * * 1"` for Monday 9am) |
| `dynamicFactsEnabled` | bool | `true` | Enable dynamic fact retrieval using vector search |
| `staticFactLimit` | usize | `10` | Max static facts (identity baseline) per prompt |
| `dynamicFactLimit` | usize | `15` | Max dynamic facts (query-relevant) per prompt |
| `vectorTopK` | usize | `30` | Number of candidate facts to fetch from vector search before FSRS re-ranking |
| `minSimilarity` | f64 | `0.55` | Minimum cosine similarity threshold for vector search results |
| `accumulatePromoteThreshold` | usize | `5` | Minimum accumulated event occurrences before promoting to extraction |
| `accumulateMinDays` | usize | `3` | Minimum distinct days of accumulated events before promoting |
| `maxStability` | f64 | `30.0` | Maximum FSRS stability value to prevent ranking domination |
| `relevanceWeightSemantic` | f64 | `0.30` | Relevance weight for semantic similarity (5 weights should sum to 1.0) |
| `relevanceWeightRetrievability` | f64 | `0.20` | Relevance weight for FSRS retrievability |
| `relevanceWeightImportance` | f64 | `0.15` | Relevance weight for fact importance |
| `relevanceWeightFrequency` | f64 | `0.10` | Relevance weight for access frequency |
| `relevanceWeightSituation` | f64 | `0.25` | Relevance weight for situational boost |

---

## workContext

JSON path: `workContext`

Work context inference engine that automatically clusters tasks and conversations into work contexts.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable work context inference |
| `inferenceIntervalMins` | u64 | `5` | How often to run inference (minutes) |
| `assignmentThreshold` | f64 | `0.55` | Similarity threshold for assigning messages to contexts |
| `mergeThreshold` | f64 | `0.85` | Similarity threshold for merging contexts |
| `maxDormancyDays` | f64 | `7.0` | Days of inactivity before a context is archived |
| `maxActiveContexts` | usize | `50` | Maximum number of active work contexts |
| `semanticWeight` | f64 | `0.50` | Weight for semantic similarity in scoring |
| `temporalWeight` | f64 | `0.25` | Weight for temporal proximity in scoring |
| `resourceWeight` | f64 | `0.25` | Weight for shared resource overlap in scoring |

---

## mcp

JSON path: `mcp`

Model Context Protocol configuration for both client connections (to external MCP servers) and server settings (exposing klyntbot as an MCP server).

### mcp (top-level)

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Master enable/disable for MCP |
| `servers` | McpServerDef[] | `[]` | List of MCP server connections |
| `server` | object | See below | Settings for klyntbot's own MCP server |

### mcp.servers[\*] (MCP server definition)

Each entry in the `servers` array defines an external MCP server connection.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `name` | String | *(required)* | Unique server name (used in tool namespacing: `mcp_{name}_{tool}`) |
| `transport` | String | *(required)* | Transport type: `"stdio"` or `"http"` |
| `enabled` | bool | `true` | Whether this server connection is active |
| `startupTimeoutSec` | u64 | `10` | Startup timeout for connection + tool discovery (seconds) |
| `toolTimeoutSec` | u64 | `120` | Per-tool-call timeout (seconds) |
| `enabledTools` | String[]? | `null` | Allowlist of tool names (original MCP names). When set, only these tools are registered |
| `disabledTools` | String[]? | `null` | Denylist of tool names. Excluded from registration. Allowlist takes precedence if both are set |
| `oauth` | object? | `null` | OAuth credentials for this server |

**Stdio transport fields** (when `transport` is `"stdio"`):

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `command` | String | *(required)* | Command to spawn the MCP server subprocess |
| `args` | String[] | `[]` | Command arguments |
| `env` | Map\<String, String\> | `{}` | Environment variables for the subprocess |

**HTTP transport fields** (when `transport` is `"http"`):

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `url` | String | *(required)* | HTTP endpoint URL |
| `headers` | Map\<String, String\> | `{}` | HTTP headers for requests |

### mcp.servers[\*].oauth

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `provider` | String | *(required)* | Provider identifier (e.g., `"linear"`, `"github"`) |
| `accessToken` | Secret\<String\> | *(required)* | OAuth access token |
| `refreshToken` | Secret\<String\>? | `null` | OAuth refresh token |
| `expiresAt` | String? | `null` | ISO-8601 expiry timestamp |
| `envVar` | String | *(required)* | Environment variable name to inject into the MCP subprocess |

### mcp.server (klyntbot MCP server settings)

Settings for exposing klyntbot itself as an MCP server.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable klyntbot's MCP server |
| `port` | u16 | `3100` | Listen port |
| `host` | String | `"127.0.0.1"` | Listen address |

---

## packs

JSON path: `packs`

Feature packs control which skill groups and config sections are active.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | String[] | `["task-management", "productivity", "ai-intelligence", "developer-tools"]` | List of enabled pack IDs |
| `enabledSkills` | String[] | `[]` | Skill names derived from enabled packs (computed by wizard, saved to config) |

---

## plugins

JSON path: `plugins`

WASM plugin system configuration.

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable the plugin system |
| `registryUrl` | String | `"https://plugins.klyntbot.io/index.json"` | Plugin registry URL |
| `sandboxMemoryMb` | u32 | `64` | WASM sandbox memory limit (MB) |
| `allowNetworkByDefault` | bool | `false` | Whether plugins can access the network by default |

---

## capture

JSON path: `capture`

External capture sources for ingesting data from the shell, filesystem, and HTTP API.

### capture.shellHook

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable shell command capture |
| `excludePatterns` | String[] | `["export *=*", "ssh-keygen*", "gpg *", "pass *", "aws configure*"]` | Glob patterns for commands to exclude (security-sensitive) |

### capture.fileWatcher

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable filesystem watching |
| `directories` | String[] | `[]` | Directories to watch |
| `ignorePatterns` | String[] | `["node_modules", ".git", "target", "build", "dist", "__pycache__", ".next", ".cache", ".DS_Store"]` | Path patterns to ignore |
| `debounceMs` | u64 | `500` | Debounce interval for file change events (ms) |

### capture.ingestionApi

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable the ingestion HTTP API |
| `port` | u16 | `3456` | Listen port for the ingestion API |
| `token` | String? | `null` | Optional bearer token for API authentication |

---

## content

JSON path: `content`

Content registry for multi-source documentation and skills loading.

### content (top-level)

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `sources` | ContentSourceConfig[] | `[]` | List of content sources |
| `trustPolicy` | String | `"official,maintainer"` | Trust policy for content sources (comma-separated: `"official"`, `"community"`, `"maintainer"`) |
| `refreshIntervalSecs` | u64 | `86400` | How often to refresh remote content sources (seconds, default: 24 hours) |
| `contentDir` | String | `""` | Directory for content cache and local storage |

### content.sources[\*]

| JSON Field | Type | Default | Description |
|---|---|---|---|
| `name` | String | *(required)* | Unique name for this source |
| `url` | String? | `null` | Remote URL to fetch content from (for remote sources) |
| `path` | String? | `null` | Local filesystem path (for local sources) |

---

## Minimal Config Examples

### Bare minimum (just one provider)

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-api03-..."
    }
  }
}
```

All other fields use defaults. The model defaults to `anthropic/claude-opus-4-5`.

### With a different model

```json
{
  "agents": {
    "defaults": {
      "model": "openai/gpt-4o",
      "provider": "openai"
    }
  },
  "providers": {
    "openai": {
      "apiKey": "sk-..."
    }
  }
}
```

### With Telegram channel

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-api03-..."
    }
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "123456:ABC-DEF...",
      "allowFrom": ["your_username"]
    }
  }
}
```

### With MCP server connections

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-api03-..."
    }
  },
  "mcp": {
    "servers": [
      {
        "name": "linear",
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@anthropic/linear-mcp-server"],
        "env": {"LINEAR_API_KEY": "lin_api_..."},
        "enabled": true
      },
      {
        "name": "notion",
        "transport": "http",
        "url": "https://mcp.notion.so/v1",
        "headers": {"Authorization": "Bearer ntn_..."},
        "enabled": true
      }
    ]
  }
}
```

### With Anthropic native mode and extended thinking

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-api03-...",
      "native": true,
      "cacheSystemPrompt": true,
      "extendedThinking": {
        "enabled": true,
        "budgetTokens": 20000,
        "useFor": ["planning", "debugging"]
      }
    }
  }
}
```

### Full example (multi-provider, multi-channel)

```json
{
  "timezone": "America/New_York",
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "maxTokens": 8192,
      "temperature": 0.7,
      "maxToolIterations": 20
    },
    "monthlyBudgetUsd": 50.0
  },
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-api03-...",
      "native": true,
      "cacheSystemPrompt": true
    },
    "openai": {
      "apiKey": "sk-..."
    }
  },
  "providerManager": {
    "primary": "anthropic",
    "fallback": "openai"
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "123456:ABC-DEF...",
      "allowFrom": ["your_username"]
    },
    "discord": {
      "enabled": true,
      "token": "your-discord-bot-token",
      "allowFrom": ["123456789"]
    },
    "slack": {
      "enabled": true,
      "botToken": "xoxb-...",
      "appToken": "xapp-...",
      "allowFrom": ["U01234567"]
    }
  },
  "tools": {
    "web": {
      "braveApiKey": "BSA..."
    },
    "browser": {
      "enabled": true,
      "trustLevel": "autonomous"
    }
  },
  "mcp": {
    "servers": [
      {
        "name": "linear",
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@anthropic/linear-mcp-server"],
        "env": {"LINEAR_API_KEY": "lin_api_..."}
      }
    ]
  },
  "todo": {
    "notifications": {
      "dailyDigestTime": "08:30"
    }
  },
  "finance": {
    "defaultCurrency": "USD",
    "fire": {
      "enabled": true,
      "currentAge": 30,
      "targetRetirementAge": 45
    }
  }
}
```
