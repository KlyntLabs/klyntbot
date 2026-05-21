# Backend Config Schema Catalog

> **Purpose:** Field-level catalog of `crates/config/src/schema/**` for mapping to a unified settings UI.
> **Source of truth:** Rust source files cited below. All JSON key names are camelCase (global `serde(rename_all = "camelCase")`).
> **Config file path:** `~/.klyntbot/config.json` (or `KLYNTBOT_HOME/config.json`).

---

## 1. Root Composition — `Config` (`schema/core.rs:98`)

`Config` is the flat root struct. Every top-level key in `config.json` corresponds to one field:

| JSON Key | Type | Default | Notes |
|---|---|---|---|
| `agents` | `AgentsConfig` | see §2 | Agent model/budget/workspace |
| `channels` | `ChannelsConfig` | see §3 | Telegram/Discord/Slack/Email |
| `providers` | `ProvidersConfig` | see §4 | LLM API keys |
| `tools` | `ToolsConfig` | see §5 | Web search, browser, approval |
| `gateway` | `GatewayConfig` | see §6 | Internal HTTP server |
| `todo` | `TodoConfig` | see §7 | Task management |
| `confidence` | `ConfidenceConfig` | see §8 | Confidence gate thresholds |
| `project` | `ProjectConfig` | see §9 | Project mgmt feature flag |
| `conversation` | `ConversationConfig` | see §10 | Memory/embedding/session |
| `learning` | `LearningConfig` | see §11 | Flashcard/FSRS system |
| `notes` | `NotesConfig` | see §12 | Notes versioning |
| `productivity` | `ProductivityConfig` | see §13 | Focus/tracking/nudges |
| `providerManager` | `ProviderManagerConfig` | see §4.1 | Multi-provider routing |
| `timezone` | `String` | auto-detect (system) | IANA timezone |
| `dataDir` | `Option<String>` | `~/.klyntbot` | Data directory override |
| `packs` | `PacksConfig` | see §14 | Feature packs |
| `cognitive` | `CognitiveConfig` | see §15 | Memory/KCA pipeline |
| `user` | `UserConfig` | see §16 | User profile |
| `workContext` | `WorkContextConfig` | see §17 | Work context inference |
| `capture` | `CaptureConfig` | see §18 | Shell hook / file watcher |
| `content` | `ContentConfig` | see §19 | Docs registry |
| `mcp` | `McpConfig` | see §20 | MCP client + server |
| `integrations` | `IntegrationsConfig` | see §21 | AI tool integrations |
| `projectRoot` | `Option<String>` | `None` | Scanning root for `.agents/skills/` |
| `language` | `LanguageConfig` | see §22 | Native/target language pair |
| `launcher` | `LauncherConfig` | see §23 | Launcher search sources |
| `scenario` | `ScenarioConfig` | see §24 | What-if reasoning |
| `shortcuts` | `ShortcutsConfig` | see §25 | Global hotkeys |
| `setupCompleted` | `bool` | `false` | First-run wizard gate |
| `autotuner` | `AutoTunerConfig` | see §26 | Nightly self-optimization |
| `schemaVersion` | `u32` | `1` | Config migration version |
| `lifecycle` | `LifecycleConfig` | see §27 | Sleep/wake/presence |
| `voice` | `VoiceConfig` | see §28 | STT/TTS engine |
| `languageLearning` | `LanguageLearningConfig` | see §29 | Pronunciation feedback |
| `embedding` | `EmbeddingConfig` | see §30 | Embedding provider |
| `notifications` | `NotificationsConfig` | see §31 | Quiet hours / retry |

---

## 2. `agents` — `AgentsConfig` (`schema/agents.rs`)

### `AgentsConfig` (root)

| Field | JSON Key | Type | Default | Purpose |
|---|---|---|---|---|
| `defaults` | `defaults` | `AgentDefaults` | see below | Per-turn LLM parameters |
| `monthly_budget_usd` | `monthlyBudgetUsd` | `Option<f64>` | `null` | Optional USD spend cap; warnings at 80%/100% |
| `skills_dir` | `skillsDir` | `Option<String>` | `null` (→ `~/.klyntbot/.agents/skills/`) | External skill directory |
| `rewriter_model` | `rewriterModel` | `Option<String>` | `null` | Model for query rewriting |

### `AgentDefaults` (nested at `agents.defaults`)

| Field | JSON Key | Type | Default | Purpose |
|---|---|---|---|---|
| `workspace` | `workspace` | `String` | `"~/.klyntbot/workspace"` | File sandbox for tool execution |
| `model` | `model` | `String` | `"anthropic/claude-opus-4-5"` | Active LLM model identifier |
| `provider` | `provider` | `Option<String>` | `null` | Explicit provider name; overrides auto-detect |
| `max_tokens` | `maxTokens` | `u32` | `8192` | Per-turn token budget |
| `temperature` | `temperature` | `f32` | `0.7` | Sampling temperature |
| `max_tool_iterations` | `maxToolIterations` | `u32` | `20` | Max tool calls per agent turn |
| `max_concurrent_subagents` | `maxConcurrentSubagents` | `usize` | `3` | Parallel subagent cap |
| `execution` | `execution` | `ExecutionConfig` | see §5.1 | Pipeline safety/depth |

### `ExecutionConfig` (nested at `agents.defaults.execution`) (`schema/execution.rs`)

| Field | JSON Key | Type | Default | Purpose |
|---|---|---|---|---|
| `safety_timeout_secs` | `safetyTimeoutSecs` | `u64` | `600` | Wall-clock deadlock guard (never fires normally) |
| `adaptive_depth` | `adaptiveDepth` | `bool` | `true` | Mirror-driven depth suggestions |

---

## 3. `channels` — `ChannelsConfig` (`schema/channels.rs`)

### `TelegramConfig` (`channels.telegram`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `token` | `token` | `Secret<String>` | `""` | **Yes** |
| `allow_from` | `allowFrom` | `Vec<String>` | `[]` | — |
| `proxy` | `proxy` | `Option<String>` | `null` | — |

**Env override:** `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN`

### `DiscordConfig` (`channels.discord`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `token` | `token` | `Secret<String>` | `""` | **Yes** |
| `allow_from` | `allowFrom` | `Vec<String>` | `[]` | — |
| `gateway_url` | `gatewayUrl` | `String` | `"wss://gateway.discord.gg/?v=10&encoding=json"` | — |
| `intents` | `intents` | `u32` | `46593` | — |

**Env override:** `KLYNTBOT_CHANNELS__DISCORD__TOKEN`

### `SlackConfig` (`channels.slack`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `bot_token` | `botToken` | `Secret<String>` | `""` | **Yes** |
| `app_token` | `appToken` | `Secret<String>` | `""` | **Yes** |
| `allow_from` | `allowFrom` | `Vec<String>` | `[]` | — |
| `mode` | `mode` | `String` | `"socket"` | — |
| `group_policy` | `groupPolicy` | `String` | `"none"` | — |
| `group_allow_from` | `groupAllowFrom` | `Vec<String>` | `[]` | — |
| `dm` | `dm` | `SlackDmConfig` | see below | — |

**Env overrides:** `KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN`, `KLYNTBOT_CHANNELS__SLACK__APP_TOKEN`

#### `SlackDmConfig` (`channels.slack.dm`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `allow_from` | `allowFrom` | `Vec<String>` | `[]` |

### `EmailConfig` (`channels.email`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `imap_host` | `imapHost` | `String` | `""` | — |
| `imap_port` | `imapPort` | `u16` | `993` | — |
| `imap_username` | `imapUsername` | `String` | `""` | — |
| `imap_password` | `imapPassword` | `Secret<String>` | `""` | **Yes** |
| `imap_mailbox` | `imapMailbox` | `String` | `"INBOX"` | — |
| `imap_use_ssl` | `imapUseSsl` | `bool` | `true` | — |
| `smtp_host` | `smtpHost` | `String` | `""` | — |
| `smtp_port` | `smtpPort` | `u16` | `587` | — |
| `smtp_username` | `smtpUsername` | `String` | `""` | — |
| `smtp_password` | `smtpPassword` | `Secret<String>` | `""` | **Yes** |
| `smtp_use_tls` | `smtpUseTls` | `bool` | `true` | — |
| `smtp_use_ssl` | `smtpUseSsl` | `bool` | `false` | — |
| `from_address` | `fromAddress` | `String` | `""` | — |
| `allow_from` | `allowFrom` | `Vec<String>` | `[]` | — |
| `consent_granted` | `consentGranted` | `bool` | `false` | — |
| `auto_reply_enabled` | `autoReplyEnabled` | `bool` | `true` | — |
| `max_body_chars` | `maxBodyChars` | `u32` | `12000` | — |
| `mark_seen` | `markSeen` | `bool` | `true` | — |
| `poll_interval_seconds` | `pollIntervalSeconds` | `u32` | `30` | — |
| `subject_prefix` | `subjectPrefix` | `String` | `"Re: "` | — |

---

## 4. `providers` — `ProvidersConfig` (`schema/providers.rs`)

All 13 providers share the same `ProviderConfig` shape. Present: `anthropic`, `openai`, `openrouter`, `deepseek`, `gemini`, `groq`, `vllm`, `zhipu`, `dashscope`, `moonshot`, `minimax`, `aihubmix`, `mimo`.

### `ProviderConfig` (repeated per provider)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `api_key` | `apiKey` | `Secret<String>` | `""` | **Yes** |
| `api_base` | `apiBase` | `Option<String>` | `null` | — |
| `extra_headers` | `extraHeaders` | `Option<HashMap<String,String>>` | `null` | — |
| `native` | `native` | `bool` | `false` | — |
| `cache_system_prompt` | `cacheSystemPrompt` | `bool` | `false` | — |
| `extended_thinking` | `extendedThinking` | `Option<ExtendedThinkingConfig>` | `null` | — |
| `api_version` | `apiVersion` | `Option<String>` | `null` | — |

#### `ExtendedThinkingConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | — |
| `budget_tokens` | `budgetTokens` | `usize` | `10000` |
| `use_for` | `useFor` | `Vec<String>` | `[]` |

### `CacheConfig` (`providers.cache`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |

**Env overrides for API keys:**
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
- `MIMO_API_KEY`

**Env overrides for API base:**
- `KLYNTBOT_PROVIDERS__DEEPSEEK__API_BASE`
- `KLYNTBOT_PROVIDERS__OPENAI__API_BASE`
- `KLYNTBOT_PROVIDERS__OPENROUTER__API_BASE`

### 4.1 `providerManager` — `ProviderManagerConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `primary` | `primary` | `Option<String>` | `null` |
| `fallback` | `fallback` | `Option<String>` | `null` |
| `classifier_model` | `classifierModel` | `Option<String>` | `null` |

---

## 5. `tools` — `ToolsConfig` (`schema/tools.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `web` | `web` | `WebToolsConfig` | see below |
| `browser` | `browser` | `BrowserConfig` | see below |
| `restrict_to_workspace` | `restrictToWorkspace` | `bool` | `false` |
| `approval_policy` | `approvalPolicy` | `ApprovalPolicyConfig` | see below |

### `WebToolsConfig` (`tools.web`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `brave_api_key` | `braveApiKey` | `Secret<String>` | `""` | **Yes** |
| `max_results` | `maxResults` | `u8` | `5` | — |

**Env override:** `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY`

### `BrowserConfig` (`tools.browser`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `trust_level` | `trustLevel` | `TrustLevel` enum | `"autonomous"` |
| `session_timeout_secs` | `sessionTimeoutSecs` | `u64` | `300` |

`TrustLevel` values: `"strict"`, `"autonomous"` (default), `"full"`.

### `ApprovalPolicyConfig` (`tools.approvalPolicy`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `non_ui_channels` | `nonUiChannels` | `NonUiPolicy` enum | `deny` |

---

## 6. `gateway` — `GatewayConfig` (`schema/gateway.rs`)

Internal HTTP server (dev mode ingestion + SSE relay).

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `host` | `host` | `String` | `"127.0.0.1"` |
| `port` | `port` | `u16` | `18790` |

---

## 7. `todo` — `TodoConfig` (`schema/todo.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `notifications` | `notifications` | `TodoNotificationConfig` | see below |
| `focus` | `focus` | `TodoFocusConfig` | see below |
| `enrichment` | `enrichment` | `TodoEnrichmentConfig` | see below |
| `search` | `search` | `TodoSearchConfig` | see below |
| `daily_planning` | `dailyPlanning` | `DailyPlanningConfig` | see below |

### `TodoNotificationConfig` (`todo.notifications`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `targets` | `targets` | `Vec<String>` | `["os_native"]` |
| `focus_reminders` | `focusReminders` | `bool` | `true` |
| `daily_digest` | `dailyDigest` | `bool` | `true` |
| `daily_digest_time` | `dailyDigestTime` | `String` | `"09:00"` |

### `TodoFocusConfig` (`todo.focus`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `max_slots` | `maxSlots` | `usize` | `3` |
| `deadline_hours` | `deadlineHours` | `u64` | `18` |

### `TodoEnrichmentConfig` (`todo.enrichment`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `auto_apply_threshold` | `autoApplyThreshold` | `f64` | `0.85` |
| `use_llm` | `useLlm` | `bool` | `false` |

### `TodoSearchConfig` (`todo.search`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `semantic_threshold` | `semanticThreshold` | `f64` | `0.5` |
| `embedding_model` | `embeddingModel` | `String` | `"bge-small-en-v1.5-Q"` |
| `rrf_k` | `rrfK` | `u32` | `60` |

### `DailyPlanningConfig` (`todo.dailyPlanning`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `planning_time` | `planningTime` | `String` | `"08:00"` |

---

## 8. `confidence` — `ConfidenceConfig` (`schema/confidence.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `threshold` | `threshold` | `f32` | `0.7` |
| `enabled` | `enabled` | `bool` | `true` |
| `tool_overrides` | `toolOverrides` | `HashMap<String,f32>` | `{}` |

---

## 9. `project` — `ProjectConfig` (`schema/project.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |

---

## 10. `conversation` — `ConversationConfig` (`schema/conversation.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `embedding` | `embedding` | `ConversationEmbeddingConfig` | see below |
| `search` | `search` | `ConversationSearchConfig` | see below |
| `session` | `session` | `SessionConfig` | see below |
| `memory` | `memory` | `MemoryConfig` | see below |

### `ConversationEmbeddingConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `exclude_channels` | `excludeChannels` | `Vec<String>` | `[]` |
| `exclude_roles` | `excludeRoles` | `Vec<String>` | `["system","tool"]` |

### `ConversationSearchConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `semantic_threshold` | `semanticThreshold` | `f64` | `0.5` |
| `max_results` | `maxResults` | `usize` | `20` |

### `SessionConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `history_limit` | `historyLimit` | `usize` | `50` |
| `max_cache_size` | `maxCacheSize` | `usize` | `5` |
| `ttl_days` | `ttlDays` | `u32` | `30` |
| `cleanup_interval_hours` | `cleanupIntervalHours` | `u32` | `1` |

### `MemoryConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `decay_half_life_days` | `decayHalfLifeDays` | `u32` | `138` |
| `max_age_days` | `maxAgeDays` | `u32` | `90` |
| `consolidation_enabled` | `consolidationEnabled` | `bool` | `false` |
| `maintenance_interval_hours` | `maintenanceIntervalHours` | `u32` | `24` |

---

## 11. `learning` — `LearningConfig` (`schema/learning.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `analysis_interval_secs` | `analysisIntervalSecs` | `u64` | `3600` |
| `min_threshold` | `minThreshold` | `f32` | `0.4` |
| `max_threshold` | `maxThreshold` | `f32` | `0.9` |
| `min_outcomes_for_adaptation` | `minOutcomesForAdaptation` | `usize` | `50` |
| `active_recall` | `activeRecall` | `ActiveRecallConfig` | see below |

### `ActiveRecallConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `semantic_auto_accept_threshold` | `semanticAutoAcceptThreshold` | `f64` | `0.78` |
| `semantic_auto_fail_threshold` | `semanticAutoFailThreshold` | `f64` | `0.45` |
| `graph_propagation_strength` | `graphPropagationStrength` | `String` | `"gentle"` |
| `graph_propagation_daily_cap` | `graphPropagationDailyCap` | `usize` | `15` |
| `default_answer_mode` | `defaultAnswerMode` | `String` | `"auto"` |

---

## 12. `notes` — `NotesConfig` (`schema/notes.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `max_versions_per_note` | `maxVersionsPerNote` | `usize` | `50` |

---

## 13. `productivity` — `ProductivityConfig` (`schema/productivity.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `tracking` | `tracking` | `TrackingConfig` | see below |
| `focus` | `focus` | `FocusConfig` | see below |
| `focus_bubble` | `focusBubble` | `FocusBubbleConfig` | see below |
| `nudges` | `nudges` | `NudgeConfig` | see below |
| `privacy` | `privacy` | `PrivacyConfig` | see below |

### `TrackingConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `poll_interval_secs` | `pollIntervalSecs` | `u64` | `5` |
| `idle_threshold_secs` | `idleThresholdSecs` | `u64` | `120` |
| `batch_write_interval_secs` | `batchWriteIntervalSecs` | `u64` | `30` |
| `raw_retention_days` | `rawRetentionDays` | `u64` | `7` |
| `bucket_retention_days` | `bucketRetentionDays` | `u64` | `365` |

### `FocusConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `default_duration_mins` | `defaultDurationMins` | `u64` | `45` |
| `break_interval_mins` | `breakIntervalMins` | `u64` | `90` |
| `break_duration_mins` | `breakDurationMins` | `u64` | `10` |
| `max_daily_focus_hours` | `maxDailyFocusHours` | `u64` | `8` |
| `soft_block_enabled` | `softBlockEnabled` | `bool` | `true` |
| `soft_block_cooldown_secs` | `softBlockCooldownSecs` | `u64` | `60` |
| `soft_block_temp_pass_mins` | `softBlockTempPassMins` | `u64` | `5` |
| `soft_block_llm_enabled` | `softBlockLlmEnabled` | `bool` | `true` |
| `soft_block_llm_timeout_ms` | `softBlockLlmTimeoutMs` | `u64` | `3000` |
| `learned_rule_threshold` | `learnedRuleThreshold` | `u64` | `3` |
| `cooldown_grace_secs` | `cooldownGraceSecs` | `u64` | `30` |

### `FocusBubbleConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `auto_reply_enabled` | `autoReplyEnabled` | `bool` | `false` |
| `auto_reply_text` | `autoReplyText` | `String` | `"I'm in a deep focus session…"` |

### `NudgeConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `break_reminders` | `breakReminders` | `bool` | `true` |
| `focus_suggestions` | `focusSuggestions` | `bool` | `true` |
| `daily_summary` | `dailySummary` | `bool` | `true` |
| `burnout_alerts` | `burnoutAlerts` | `bool` | `true` |
| `cooldown_mins` | `cooldownMins` | `u64` | `15` |
| `quiet_hours_start` | `quietHoursStart` | `Option<String>` | `null` |
| `quiet_hours_end` | `quietHoursEnd` | `Option<String>` | `null` |

### `PrivacyConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `excluded_apps` | `excludedApps` | `Vec<String>` | `[]` |
| `exclude_window_titles` | `excludeWindowTitles` | `bool` | `false` |
| `excluded_url_patterns` | `excludedUrlPatterns` | `Vec<String>` | `[]` |

---

## 14. `packs` — `PacksConfig` (`schema/packs.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `Vec<String>` | `["task-management","productivity","ai-intelligence","developer-tools"]` |
| `enabled_skills` | `enabledSkills` | `Vec<String>` | `[]` |

`PackTier` enum (used for wizard UI only, not stored directly): `"core"`, `"recommended"`, `"optional"`.

---

## 15. `cognitive` — `CognitiveConfig` (`schema/cognitive.rs`)

The largest single module — the KCA memory pipeline. All sub-configs nest under `cognitive.*`.

### Core model/retrieval fields

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `model` | `model` | `Option<String>` | `null` (falls back to `agents.defaults.model`) |
| `provider` | `provider` | `Option<String>` | `null` |
| `temperature` | `temperature` | `Option<f32>` | `null` (→ 0.2) |
| `max_tokens` | `maxTokens` | `Option<u32>` | `null` (→ 1024) |
| `graph_linker_model` | `graphLinkerModel` | `Option<String>` | `null` |
| `critic_model` | `criticModel` | `Option<String>` | `null` |
| `temporal_prune_model` | `temporalPruneModel` | `Option<String>` | `null` |
| `dynamic_facts_enabled` | `dynamicFactsEnabled` | `bool` | `true` |
| `static_fact_limit` | `staticFactLimit` | `usize` | `10` |
| `dynamic_fact_limit` | `dynamicFactLimit` | `usize` | `15` |
| `vector_top_k` | `vectorTopK` | `usize` | `30` |
| `min_similarity` | `minSimilarity` | `f64` | `0.55` |
| `accumulate_promote_threshold` | `accumulatePromoteThreshold` | `usize` | `5` |
| `accumulate_min_days` | `accumulateMinDays` | `usize` | `3` |
| `episodic_importance_threshold` | `episodicImportanceThreshold` | `f64` | `0.7` |
| `openai_embedding_model` | `openaiEmbeddingModel` | `String` | `"text-embedding-3-small"` |
| `max_stability` | `maxStability` | `f64` | `30.0` |
| `confirm_threshold` | `confirmThreshold` | `f64` | `0.0` |
| `intelligence_mode` | `intelligenceMode` | `IntelligenceMode` enum | `"standard"` |

**Relevance ranking weights** (all `f64`, must sum to ~1.0):

| Field | JSON Key | Default |
|---|---|---|
| `relevance_weight_semantic` | `relevanceWeightSemantic` | `0.30` |
| `relevance_weight_retrievability` | `relevanceWeightRetrievability` | `0.20` |
| `relevance_weight_importance` | `relevanceWeightImportance` | `0.15` |
| `relevance_weight_frequency` | `relevanceWeightFrequency` | `0.10` |
| `relevance_weight_situation` | `relevanceWeightSituation` | `0.25` |
| `relevance_weight_temporal` | `relevanceWeightTemporal` | `0.05` |
| `relevance_weight_recall_support` | `relevanceWeightRecallSupport` | `0.08` |
| `relevance_weight_graph_path_boost` | `relevanceWeightGraphPathBoost` | `0.06` |

### `InsightForge` fields

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `insight_forge_enabled` | `insightForgeEnabled` | `bool` | `true` |
| `insight_forge_max_sub_queries` | `insightForgeMaxSubQueries` | `usize` | `5` |
| `insight_forge_per_source_limit` | `insightForgePerSourceLimit` | `usize` | `5` |
| `insight_forge_total_limit` | `insightForgeTotalLimit` | `usize` | `15` |
| `insight_forge_per_source_timeout_ms` | `insightForgePerSourceTimeoutMs` | `u64` | `800` |

### `BookIndexConfig` (`cognitive.bookIndex`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `entity_resolution` | `entityResolution` | `BookEntityResolutionConfig` | see below |
| `retrieval` | `retrieval` | `BookRetrievalCfg` | see below |

#### `BookEntityResolutionConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `top_k` | `topK` | `usize` | `10` |
| `gradient_threshold` | `gradientThreshold` | `f64` | `0.6` |
| `min_similarity` | `minSimilarity` | `f64` | `0.3` |
| `use_llm_disambiguation` | `useLlmDisambiguation` | `bool` | `false` |

#### `BookRetrievalCfg`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `max_nodes` | `maxNodes` | `usize` | `50` |
| `max_map_nodes` | `maxMapNodes` | `usize` | `10` |
| `operator_timeout_ms` | `operatorTimeoutMs` | `u64` | `600` |
| `pagerank_damping` | `pagerankDamping` | `f64` | `0.85` |
| `pagerank_iterations` | `pagerankIterations` | `u32` | `20` |

### `AtomExtractionConfig` (`cognitive.atomExtraction`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `max_tokens` | `maxTokens` | `u32` | `1024` |

### `QueryEnhancementConfig` (`cognitive.queryEnhancement`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `prf` | `prf` | `PrfEnhancementConfig` | see below |
| `multi_query` | `multiQuery` | `MultiQueryEnhancementConfig` | see below |
| `reranking` | `reranking` | `RerankingEnhancementConfig` | see below |
| `budget_overrides` | `budgetOverrides` | `BudgetOverrides` | all null |

#### `PrfEnhancementConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `initial_fetch_limit` | `initialFetchLimit` | `usize` | `3` |
| `min_score_threshold` | `minScoreThreshold` | `f64` | `0.6` |
| `max_expansion_terms` | `maxExpansionTerms` | `usize` | `5` |

#### `MultiQueryEnhancementConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `max_variants` | `maxVariants` | `usize` | `3` |
| `model` | `model` | `Option<String>` | `null` |

#### `RerankingEnhancementConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `llm_rerank_top_n` | `llmRerankTopN` | `usize` | `10` |
| `llm_rerank_model` | `llmRerankModel` | `Option<String>` | `null` |

### `MicroReforgeConfig` (`cognitive.microReforge`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `turn_threshold` | `turnThreshold` | `u32` | `10` |
| `minute_threshold` | `minuteThreshold` | `u32` | `30` |
| `min_confidence` | `minConfidence` | `f64` | `0.65` |
| `model` | `model` | `Option<String>` | `null` |

### `PredictiveCacheConfig` (`cognitive.predictiveCache`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `predictions_per_turn` | `predictionsPerTurn` | `u32` | `3` |
| `ttl_seconds` | `ttlSeconds` | `u32` | `300` |
| `min_hit_rate_for_keep_alive` | `minHitRateForKeepAlive` | `f64` | `0.20` |
| `model` | `model` | `Option<String>` | `null` |

### `HierarchicalConfig` (`cognitive.hierarchical`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `hourly_schedule` | `hourlySchedule` | `String` | `"0 5 * * * *"` |
| `daily_schedule` | `dailySchedule` | `String` | `"0 30 0 * * *"` |
| `weekly_schedule` | `weeklySchedule` | `String` | `"0 0 1 * * 1"` |
| `model` | `model` | `Option<String>` | `null` |

### `HistoryCompressionConfig` (`cognitive.historyCompression`) (`schema/history_compression.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `model` | `model` | `Option<String>` | `null` |
| `use_cognitive_scoring` | `useCognitiveScoring` | `bool` | `true` |
| `delta_only_on_resume` | `deltaOnlyOnResume` | `bool` | `true` |
| `tier0_messages` | `tier0Messages` | `TierZeroConfig` | `{normal:8,deepThink:12,ultra:16}` |
| `tier1_ratio` | `tier1Ratio` | `f32` | `0.35` |
| `tier2_ratio` | `tier2Ratio` | `f32` | `0.12` |
| `high_relevance_threshold` | `highRelevanceThreshold` | `f64` | `0.70` |
| `low_relevance_threshold` | `lowRelevanceThreshold` | `f64` | `0.40` |
| `tier1_demotion_threshold` | `tier1DemotionThreshold` | `usize` | `30` |

---

## 16. `user` — `UserConfig` (`schema/user.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `name` | `name` | `String` | `""` |

---

## 17. `workContext` — `WorkContextConfig` (`schema/work_context.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `inference_interval_mins` | `inferenceIntervalMins` | `u64` | `5` |
| `assignment_threshold` | `assignmentThreshold` | `f64` | `0.55` |
| `merge_threshold` | `mergeThreshold` | `f64` | `0.85` |
| `max_dormancy_days` | `maxDormancyDays` | `f64` | `7.0` |
| `max_active_contexts` | `maxActiveContexts` | `usize` | `50` |
| `semantic_weight` | `semanticWeight` | `f64` | `0.70` |
| `temporal_weight` | `temporalWeight` | `f64` | `0.15` |
| `resource_weight` | `resourceWeight` | `f64` | `0.15` |

---

## 18. `capture` — `CaptureConfig` (`schema/capture.rs`)

### `ShellHookConfig` (`capture.shellHook`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `exclude_patterns` | `excludePatterns` | `Vec<String>` | `["export *=*","ssh-keygen*",…]` |

### `FileWatcherConfig` (`capture.fileWatcher`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `directories` | `directories` | `Vec<String>` | `[]` |
| `ignore_patterns` | `ignorePatterns` | `Vec<String>` | `["node_modules",".git","target",…]` |
| `debounce_ms` | `debounceMs` | `u64` | `500` |

### `IngestionApiConfig` (`capture.ingestionApi`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `port` | `port` | `u16` | `3456` |
| `token` | `token` | `Option<String>` | `null` |

---

## 19. `content` — `ContentConfig` (`schema/content.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `sources` | `sources` | `Vec<ContentSourceConfig>` | `[]` |
| `trust_policy` | `trustPolicy` | `String` | `"official,maintainer"` |
| `refresh_interval_secs` | `refreshIntervalSecs` | `u64` | `86400` |
| `content_dir` | `contentDir` | `PathBuf` | `""` |

### `ContentSourceConfig` (element of `content.sources`)

| Field | JSON Key | Type |
|---|---|---|
| `name` | `name` | `String` |
| `url` | `url` | `Option<String>` |
| `path` | `path` | `Option<String>` |

---

## 20. `mcp` — `McpConfig` (`schema/mcp.rs`)

### `McpConfig` root

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `servers` | `servers` | `Vec<McpServerDef>` | `[]` |
| `server` | `server` | `McpServerSettings` | see below |
| `channel_allowlists` | `channelAllowlists` | `HashMap<String,Vec<String>>` | `{}` |

### `McpServerDef` (element of `mcp.servers`)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `name` | `name` | `String` | — | — |
| `transport` | (inline tag) | `McpTransport` enum | — | — |
| `enabled` | `enabled` | `bool` | `true` | — |
| `oauth` | `oauth` | `Option<McpOAuthCredentials>` | `null` | — |
| `startup_timeout_sec` | `startupTimeoutSec` | `u64` | `10` | — |
| `tool_timeout_sec` | `toolTimeoutSec` | `u64` | `120` | — |
| `enabled_tools` | `enabledTools` | `Option<Vec<String>>` | `null` | — |
| `disabled_tools` | `disabledTools` | `Option<Vec<String>>` | `null` | — |

`McpTransport` variants:
- `Stdio`: `command: String`, `args: Vec<String>`, `env: HashMap<String,String>`
- `Http`: `url: String`, `headers: HashMap<String,String>`

### `McpOAuthCredentials`

| Field | JSON Key | Type | Secret? |
|---|---|---|---|
| `provider` | `provider` | `String` | — |
| `access_token` | `accessToken` | `Secret<String>` | **Yes** |
| `refresh_token` | `refreshToken` | `Option<Secret<String>>` | **Yes** |
| `expires_at` | `expiresAt` | `Option<String>` | — |
| `env_var` | `envVar` | `String` | — |

### `McpServerSettings` (`mcp.server` — expose KlyntBot as MCP server)

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `port` | `port` | `u16` | `3100` | — |
| `host` | `host` | `String` | `"127.0.0.1"` | — |
| `exposed_tools` | `exposedTools` | `Vec<String>` | `[]` (auto-filled) | — |
| `auth` | `auth` | `McpAuthConfig` | see below | — |

### `McpAuthConfig`

| Field | JSON Key | Type | Default | Secret? |
|---|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` | — |
| `token` | `token` | `Option<Secret<String>>` | `null` | **Yes** |

---

## 21. `integrations` — `IntegrationsConfig` (`schema/integrations.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `ai_tools` | `aiTools` | `Vec<String>` | `[]` |

---

## 22. `language` — `LanguageConfig` (`schema/language.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `native_lang` | `nativeLang` | `Option<String>` | `null` |
| `source_lang` | `sourceLang` | `Option<String>` | `null` |
| `target_lang` | `targetLang` | `Option<String>` | `null` |
| `auto_detect` | `autoDetect` | `bool` | `true` |
| `proficiency_level` | `proficiencyLevel` | `Option<String>` | `null` |

---

## 23. `launcher` — `LauncherConfig` (`schema/launcher.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `sources` | `sources` | `LauncherSourcesConfig` | see below |

### `LauncherSourcesConfig` (all sub-fields under `launcher.sources`)

| Source | JSON Key | Struct | Default `enabled` |
|---|---|---|---|
| Apps | `apps` | `SourceToggle` | `true` |
| System Prefs | `systemPrefs` | `SourceToggle` | `true` |
| Brew | `brew` | `SourceToggle` | `true` |
| SSH Hosts | `sshHosts` | `SourceToggle` | `true` |
| Git Repos | `gitRepos` | `GitReposConfig` | `true` |
| Scripts | `scripts` | `ScriptsConfig` | `true` |
| Files | `files` | `FileSearchConfig` | `true` |
| Content Grep | `contentGrep` | `ContentGrepConfig` | `true` |
| Contacts | `contacts` | `SourceToggle` | `true` |
| Running Apps | `runningApps` | `SourceToggle` | `true` |
| Bookmarks | `bookmarks` | `BrowserSourceConfig` | `true` |
| Browser History | `browserHistory` | `BrowserHistoryConfig` | `true` |
| Tasks | `tasks` | `SourceToggle` | `true` |
| Notes | `notes` | `SourceToggle` | `true` |
| Clipboard | `clipboard` | `ClipboardSourceConfig` | `true` |
| Window Presets | `windowPresets` | `SourceToggle` | `true` |
| Calendar | `calendar` | `CalendarSourceConfig` | `true` |

**Extended config per source:**

- `gitRepos.scanDirs`: `Vec<String>`, default `["~/Projects","~/Developer"]`
- `scripts.dir`: `String`, default `"~/.klyntbot/scripts"`
- `files`: `refreshIntervalSecs=120`, `maxEntries=1000000`, `mdfindFallback=true`, `mdfindThreshold=20`, `rebuildIntervalMin=30`, `scanDirs`
- `bookmarks.browser` / `browserHistory.browser`: `String`, default `"chrome"`
- `browserHistory.maxDays`: `i64`, default `30`
- `contentGrep.defaultScope`: `String`, default `"."`
- `clipboard.maxEntries`: `i64`, default `1000`
- `calendar.lookbackDays=1`, `lookaheadDays=7`

---

## 24. `scenario` — `ScenarioConfig` (`schema/scenario.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `max_graph_depth` | `maxGraphDepth` | `u32` | `2` |

---

## 25. `shortcuts` — `ShortcutsConfig` (`schema/shortcuts.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `launcher` | `launcher` | `String` | `"alt+space"` |
| `tray` | `tray` | `String` | `"alt+shift+space"` |

---

## 26. `autotuner` — `AutoTunerConfig` (`schema/autotuner.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `schedule` | `schedule` | `String` | `"0 0 2 * * *"` |
| `min_messages_for_promotion` | `minMessagesForPromotion` | `u32` | `50` |
| `rollback_after_days` | `rollbackAfterDays` | `u8` | `3` |
| `min_correction_improvement` | `minCorrectionImprovement` | `f64` | `0.05` |
| `max_token_cost_increase` | `maxTokenCostIncrease` | `f64` | `0.08` |
| `max_response_time_increase` | `maxResponseTimeIncrease` | `f64` | `0.15` |
| `max_routing_stability_decrease` | `maxRoutingStabilityDecrease` | `f64` | `0.10` |
| `max_memory_relevance_decrease` | `maxMemoryRelevanceDecrease` | `f64` | `0.05` |
| `max_retrieval_precision_drop` | `maxRetrievalPrecisionDrop` | `f64` | `0.05` |
| `max_correction_rate_increase` | `maxCorrectionRateIncrease` | `f64` | `0.03` |
| `max_promotion_accuracy_drop` | `maxPromotionAccuracyDrop` | `f64` | `0.05` |

---

## 27. `lifecycle` — `LifecycleConfig` (`schema/lifecycle.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `idle_threshold_secs` | `idleThresholdSecs` | `u64` | `300` |
| `presence_threshold_secs` | `presenceThresholdSecs` | `u64` | `2` |
| `wake_grace_period_secs` | `wakeGracePeriodSecs` | `u64` | `60` |
| `active_poll_interval_secs` | `activePollIntervalSecs` | `u64` | `10` |
| `idle_poll_interval_secs` | `idlePollIntervalSecs` | `u64` | `30` |
| `wake_delivery` | `wakeDelivery` | `WakeDeliveryConfig` | see below |
| `disable_smart_scheduling` | `disableSmartScheduling` | `bool` | `false` |

### `WakeDeliveryConfig` (`lifecycle.wakeDelivery`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `min_absence_for_panel_secs` | `minAbsenceForPanelSecs` | `u64` | `1800` |
| `quiet_period_morning_secs` | `quietPeriodMorningSecs` | `u64` | `45` |
| `quiet_period_midday_secs` | `quietPeriodMiddaySecs` | `u64` | `15` |
| `quiet_period_evening_secs` | `quietPeriodEveningSecs` | `u64` | `60` |
| `quiet_period_default_secs` | `quietPeriodDefaultSecs` | `u64` | `30` |
| `catch_up_tier_stagger_secs` | `catchUpTierStaggerSecs` | `u64` | `120` |
| `idle_resume_prompt_threshold_secs` | `idleResumePromptThresholdSecs` | `u64` | `600` |
| `nudge_consolidation_threshold_secs` | `nudgeConsolidationThresholdSecs` | `u64` | `1800` |

---

## 28. `voice` — `VoiceConfig` (`schema/voice.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `input` | `input` | `VoiceInputConfig` | see below |
| `output` | `output` | `VoiceOutputConfig` | see below |
| `learning` | `learning` | `VoiceLearningConfig` | see below |
| `conversation` | `conversation` | `VoiceConversationConfig` | see below |

### `VoiceInputConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `hotkey` | `hotkey` | `String` | `"alt+shift+v"` |
| `silence_threshold_secs` | `silenceThresholdSecs` | `f32` | `1.5` |
| `privacy_mode` | `privacyMode` | `VoicePrivacyMode` enum | `"standard"` |
| `stt_engine` | `sttEngine` | `SttEngineKind` enum | `"qwen3"` |
| `vad_threshold` | `vadThreshold` | `f32` | `0.5` |
| `use_neural_vad` | `useNeuralVad` | `bool` | `false` |
| `deployment` | `deployment` | `EngineDeployment` tag enum | `{mode:"local"}` |
| `allowed_languages` | `allowedLanguages` | `Vec<String>` | `["en","zh","vi"]` |
| `selected_device` | `selectedDevice` | `Option<String>` | `null` |
| `noise_reduction` | `noiseReduction` | `bool` | `false` |

`EngineDeployment::Cloud` adds `apiUrl: String` and `apiKey: Secret<String>` (**Yes, secret**).

### `VoiceOutputConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `true` |
| `voice_preferences` | `voicePreferences` | `HashMap<String,String>` | `{}` |
| `speaking_rate` | `speakingRate` | `f32` | `1.0` |
| `speak_during_focus` | `speakDuringFocus` | `bool` | `false` |
| `tts_engine` | `ttsEngine` | `TtsEngineKind` enum | `"qwen3"` |
| `deployment` | `deployment` | `EngineDeployment` | `{mode:"local"}` |
| `selected_device` | `selectedDevice` | `Option<String>` | `null` |
| `default_persona` | `defaultPersona` | `String` | `"neutral"` |
| `personas` | `personas` | `HashMap<String,VoicePersona>` | 6 preset personas |

`TtsEngineKind` values: `"qwen3"`, `"qwen3Base"`, `"system"`.

`VoicePersona` variants:
- `Preset`: `{type:"preset", speaker:String, speed:f32, temperature:f32}`
- `Custom`: `{type:"custom", description:String, speed:f32, temperature:f32}`

### `VoiceLearningConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `target_language` | `targetLanguage` | `Option<String>` | `null` |
| `show_pronunciation_scores` | `showPronunciationScores` | `bool` | `true` |
| `auto_create_flashcards` | `autoCreateFlashcards` | `bool` | `true` |

### `VoiceConversationConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `warm_session_minutes` | `warmSessionMinutes` | `u32` | `15` |
| `warm_chat_minutes` | `warmChatMinutes` | `u32` | `5` |
| `silence_threshold_secs` | `silenceThresholdSecs` | `f32` | `1.5` |
| `auto_resume` | `autoResume` | `bool` | `true` |
| `adaptive_breath` | `adaptiveBreath` | `bool` | `true` |
| `streaming_tts` | `streamingTts` | `bool` | `true` |
| `conversation_silence_secs` | `conversationSilenceSecs` | `f32` | `0.8` |

---

## 29. `languageLearning` — `LanguageLearningConfig` (`schema/language_learning.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `feedback` | `feedback` | `FeedbackConfig` | see below |

### `FeedbackConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `default_level` | `defaultLevel` | `FeedbackLevel` enum | `"summary"` |
| `escalation_threshold` | `escalationThreshold` | `f32` | `0.3` |
| `min_encounters` | `minEncounters` | `u32` | `5` |

`FeedbackLevel` values: `"summary"`, `"overlay"`, `"silent"`.

---

## 30. `embedding` — `EmbeddingConfig` (`schema/core.rs:241`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `provider` | `provider` | `String` | `"local"` |
| `api_base` | `apiBase` | `Option<String>` | `null` |

---

## 31. `notifications` — `NotificationsConfig` (`schema/notifications.rs`)

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `quiet_hours` | `quietHours` | `QuietHoursConfig` | see below |
| `default_channels` | `defaultChannels` | `Vec<String>` | `["os_native","tray"]` |
| `default_misfire_policy` | `defaultMisfirePolicy` | `String` | `"skip_if_stale"` |
| `default_grace_window_secs` | `defaultGraceWindowSecs` | `i64` | `3600` |
| `retry` | `retry` | `RetryConfig` | see below |

### `QuietHoursConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `enabled` | `enabled` | `bool` | `false` |
| `start` | `start` | `String` | `"22:00"` |
| `end` | `end` | `String` | `"07:00"` |
| `override_for_urgent_tasks` | `overrideForUrgentTasks` | `bool` | `true` |

### `RetryConfig`

| Field | JSON Key | Type | Default |
|---|---|---|---|
| `max_attempts` | `maxAttempts` | `u32` | `3` |
| `base_delay_secs` | `baseDelaySecs` | `u64` | `1` |

---

## Environment Variable Override System (`env.rs`)

**Convention:** `KLYNTBOT_` prefix + double-underscores for nesting (`__`). Loaded via `dotenvy` (`.env` file in CWD), then shell environment.

**Explicitly wired env overrides** (see `env.rs:45–184`):

| Env Var | Config Path | Type |
|---|---|---|
| `KLYNTBOT_AGENTS__DEFAULTS__MODEL` | `agents.defaults.model` | `String` |
| `KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE` | `agents.defaults.workspace` | `String` |
| `KLYNTBOT_AGENTS__DEFAULTS__PROVIDER` | `agents.defaults.provider` | `Option<String>` |
| `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE` | `agents.defaults.temperature` | `f32` |
| `KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS` | `agents.defaults.maxTokens` | `u32` |
| `KLYNTBOT_COGNITIVE__PROVIDER` | `cognitive.provider` | `Option<String>` |
| `KLYNTBOT_COGNITIVE__MODEL` | `cognitive.model` | `Option<String>` |
| `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY` | `providers.anthropic.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__OPENAI__API_KEY` | `providers.openai.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY` | `providers.openrouter.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY` | `providers.deepseek.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__GEMINI__API_KEY` | `providers.gemini.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__GROQ__API_KEY` | `providers.groq.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__VLLM__API_KEY` | `providers.vllm.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__ZHIPU__API_KEY` | `providers.zhipu.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__DASHSCOPE__API_KEY` | `providers.dashscope.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__MOONSHOT__API_KEY` | `providers.moonshot.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__MINIMAX__API_KEY` | `providers.minimax.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__AIHUBMIX__API_KEY` | `providers.aihubmix.apiKey` | `Secret<String>` |
| `MIMO_API_KEY` | `providers.mimo.apiKey` | `Secret<String>` |
| `KLYNTBOT_PROVIDERS__DEEPSEEK__API_BASE` | `providers.deepseek.apiBase` | `Option<String>` |
| `KLYNTBOT_PROVIDERS__OPENAI__API_BASE` | `providers.openai.apiBase` | `Option<String>` |
| `KLYNTBOT_PROVIDERS__OPENROUTER__API_BASE` | `providers.openrouter.apiBase` | `Option<String>` |
| `KLYNTBOT_DATA_DIR` | `dataDir` | `Option<String>` |
| `KLYNTBOT_HOME` | `dataDir` (fallback) | `Option<String>` |
| `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN` | `channels.telegram.token` | `Secret<String>` |
| `KLYNTBOT_CHANNELS__DISCORD__TOKEN` | `channels.discord.token` | `Secret<String>` |
| `KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN` | `channels.slack.botToken` | `Secret<String>` |
| `KLYNTBOT_CHANNELS__SLACK__APP_TOKEN` | `channels.slack.appToken` | `Secret<String>` |
| `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY` | `tools.web.braveApiKey` | `Secret<String>` |

> Only these explicit env vars are wired. There is **no** generic deserializer-based env-override for all fields — only the above list in `env.rs`.

---

## Hot-Reload vs Restart (`schema/hot.rs`, `loader.rs`)

### Hot-Reloadable Fields (take effect within ~5s via file-watcher mtime check)

`HotConfig` (`hot.rs:14`) is extracted from `Config` on every `reload_if_changed` cycle. Only these six fields trigger a `HotConfigDiff`:

| HotConfig Field | Config Path | Change Detected By |
|---|---|---|
| `model` | `agents.defaults.model` | `model_changed` |
| `temperature` | `agents.defaults.temperature` | `temperature_changed` |
| `max_tokens` | `agents.defaults.maxTokens` | `max_tokens_changed` |
| `max_tool_iterations` | `agents.defaults.maxToolIterations` | `max_tool_iterations_changed` |
| `safety_timeout_secs` | `agents.defaults.execution.safetyTimeoutSecs` | `safety_timeout_changed` |
| `monthly_budget_usd` | `agents.monthlyBudgetUsd` | `budget_changed` |

**Mechanism:** `loader::reload_if_changed` (`loader.rs:223`) checks mtime every ~5s, parses config, compares `HotConfig::diff()`. Changes are applied to the running agent immediately without restart.

### Requires Restart

Everything **not** in `HotConfig`:
- Enabling/disabling channels (`channels.telegram.enabled`, etc.) — channel listeners are started at init
- Provider init (loading new API keys into provider registry — the registry is initialized once at startup)
- Feature packs (`packs.enabled`) — `FeaturePackage` initialization at startup
- MCP server connections (`mcp.servers`) — connection pool created at startup
- Gateway host/port (`gateway.*`) — bound socket at startup
- `capture.ingestionApi` port — bound at startup
- `voice.input/output.deployment` mode — engine init at startup
- `embedding.provider` — engine init at startup
- Session TTL / cleanup scheduler config — scheduler created at init
- Any structural changes to cognitive pipeline sub-configs

> **Source comment** (`hot.rs:1`): "The full `Config` still requires restart for structural changes (channels, provider init, feature enable/disable flags)."

---

## Secrets Catalog

All fields using `Secret<String>` (printed as `[REDACTED]` in Debug/Display):

| JSON Path | Purpose |
|---|---|
| `providers.anthropic.apiKey` | Anthropic API key |
| `providers.openai.apiKey` | OpenAI API key |
| `providers.openrouter.apiKey` | OpenRouter API key |
| `providers.deepseek.apiKey` | DeepSeek API key |
| `providers.gemini.apiKey` | Google Gemini API key |
| `providers.groq.apiKey` | Groq API key |
| `providers.vllm.apiKey` | vLLM API key |
| `providers.zhipu.apiKey` | Zhipu AI API key |
| `providers.dashscope.apiKey` | Alibaba DashScope API key |
| `providers.moonshot.apiKey` | Moonshot AI API key |
| `providers.minimax.apiKey` | MiniMax API key |
| `providers.aihubmix.apiKey` | AIHubMix API key |
| `providers.mimo.apiKey` | Mimo API key |
| `channels.telegram.token` | Telegram bot token |
| `channels.discord.token` | Discord bot token |
| `channels.slack.botToken` | Slack Bot OAuth token |
| `channels.slack.appToken` | Slack App-level token |
| `channels.email.imapPassword` | IMAP account password |
| `channels.email.smtpPassword` | SMTP account password |
| `tools.web.braveApiKey` | Brave Search API key |
| `mcp.servers[*].oauth.accessToken` | MCP OAuth access token |
| `mcp.servers[*].oauth.refreshToken` | MCP OAuth refresh token |
| `mcp.server.auth.token` | MCP server auth token |
| `voice.input.deployment.apiKey` | Cloud STT API key (Cloud mode only) |
| `voice.output.deployment.apiKey` | Cloud TTS API key (Cloud mode only) |

**Serialization behavior:** `Secret<T>` is `#[serde(transparent)]` — it serializes as the inner value. `Debug` and `Display` emit `[REDACTED]`. No automatic omission from JSON (secrets appear in `config.json`).

---

## Feature Enable/Disable Flags

Boolean fields whose `false` value disables an entire subsystem:

| JSON Path | Default | Subsystem |
|---|---|---|
| `channels.telegram.enabled` | `false` | Telegram listener |
| `channels.discord.enabled` | `false` | Discord listener |
| `channels.slack.enabled` | `false` | Slack listener |
| `channels.slack.dm.enabled` | `false` | Slack DM sub-channel |
| `channels.email.enabled` | `false` | Email IMAP/SMTP |
| `tools.browser.enabled` | `false` | Browser automation |
| `mcp.enabled` | `true` | All MCP client connections |
| `mcp.server.enabled` | `false` | MCP server (expose KlyntBot) |
| `mcp.server.auth.enabled` | `false` | MCP server authentication |
| `voice.enabled` | `true` | Voice system entirely |
| `voice.output.enabled` | `true` | TTS output |
| `launcher.enabled` | `true` | Launcher window |
| `productivity.enabled` | `true` | Productivity tracking |
| `learning.enabled` | `true` | FSRS learning system |
| `languageLearning.enabled` | `false` | Pronunciation feedback |
| `project.enabled` | `true` | Project management |
| `confidence.enabled` | `true` | Confidence gate |
| `conversation.embedding.enabled` | `true` | Auto conversation embedding |
| `conversation.search.enabled` | `true` | Conversation search |
| `todo.enrichment.enabled` | `true` | Task auto-enrichment |
| `todo.search.enabled` | `true` | Semantic task search |
| `todo.dailyPlanning.enabled` | `true` | Daily planning feature |
| `capture.shellHook.enabled` | `false` | Shell command capture |
| `capture.fileWatcher.enabled` | `false` | File watcher capture |
| `capture.ingestionApi.enabled` | `true` | Ingestion HTTP API |
| `workContext.enabled` | `true` | Work context inference |
| `cognitive.dynamicFactsEnabled` | `true` | Dynamic fact retrieval |
| `cognitive.insightForgeEnabled` | `true` | InsightForge multi-dim retrieval |
| `cognitive.bookIndex.enabled` | `true` | Book/knowledge index |
| `cognitive.atomExtraction.enabled` | `true` | Atom extraction from notes |
| `cognitive.queryEnhancement.enabled` | `true` | Query enhancement pipeline |
| `cognitive.microReforge.enabled` | `true` | Micro-Reforge timer |
| `cognitive.predictiveCache.enabled` | `true` | Predictive memory cache |
| `cognitive.hierarchical.enabled` | `true` | Hierarchical episodic compression |
| `notifications.quietHours.enabled` | `false` | Quiet hours |
| `productivity.focus.softBlockEnabled` | `true` | Soft-block distraction guard |

---

## Proposed UI Domain Groupings

Clustering all ~37 modules into ~10 user-facing settings domains:

### Domain 1 — Models & Providers
**Modules:** `agents`, `providers`, `providerManager`, `embedding`, `cognitive` (model/provider/temperature fields only)
**Fields:** Active model, temperature, max tokens, max iterations, per-provider API keys + bases, extended thinking, prompt caching, fallback routing, embedding provider
**Richest area** — ~45 fields spanning 13 providers

### Domain 2 — Memory & Intelligence
**Modules:** `cognitive` (retrieval/KCA/pipelines), `conversation`, `confidence`, `scenario`
**Fields:** Fact limits, relevance weights, InsightForge, query enhancement, memory decay, session TTL, history compression, confidence thresholds
**Richest area** — ~65 fields; most are internal tuning knobs

### Domain 3 — Voice
**Modules:** `voice`
**Fields:** Enabled, STT/TTS engine, deployment (local/cloud), hotkey, VAD, privacy mode, personas, speaking rate, conversation turn-taking, language filtering

### Domain 4 — Channels & Integrations
**Modules:** `channels`, `integrations`, `mcp`
**Fields:** Per-channel enable/token/allow_from, Slack group policy, Email IMAP/SMTP config, MCP server connections, MCP server exposure, AI tool integrations

### Domain 5 — Productivity & Focus
**Modules:** `productivity`, `lifecycle`, `notifications`, `todo` (dailyPlanning/notifications/focus)
**Fields:** Focus duration/breaks, soft-block, nudge types, quiet hours, wake delivery, tracking retention, activity privacy, to-do digest

### Domain 6 — Language & Learning
**Modules:** `language`, `languageLearning`, `learning`, `voice.learning`
**Fields:** Native/target language, proficiency level, FSRS thresholds, active recall modes, pronunciation feedback level

### Domain 7 — Launcher
**Modules:** `launcher`
**Fields:** Per-source enable toggles (17 sources), scan directories, browser choice, clipboard history depth, calendar lookahead

### Domain 8 — Privacy & Security
**Modules:** `tools` (restrictToWorkspace, approvalPolicy), `productivity.privacy`, `capture`, `channels.email.consentGranted`, `voice.input.privacyMode`
**Fields:** Workspace sandboxing, tool approval for headless channels, excluded apps/URLs, shell hook exclude patterns, voice privacy mode, email consent

### Domain 9 — App & UI
**Modules:** `shortcuts`, `user`, `packs`, `notifications` (channels/retry)
**Fields:** Global hotkeys (launcher, tray), user name, enabled packs/skills, notification channels, quiet hours, retry policy

### Domain 10 — Advanced / Developer
**Modules:** `gateway`, `autotuner`, `autotuner`, `cognitive` (schedule/cron fields), `work_context`, `content`, `agents.skillsDir`, `schemaVersion`
**Fields:** Internal HTTP port, nightly autotuner schedule and thresholds, cron schedules for episodic compression, content registry sources, external skill directory

---

## Summary Statistics

| Metric | Count |
|---|---|
| Schema modules | 37 (31 in `schema/`, plus `hot.rs` + `history_compression.rs` as public sub-crates) |
| Top-level Config fields | 36 |
| Approximate total leaf fields | ~310 |
| `Secret<String>` fields | 25 |
| Feature enable/disable booleans | 37 |
| Env-override-wired fields | 29 |
| Hot-reloadable fields | 6 |
| Fields requiring restart | ~304 |
| Proposed UI domains | 10 |

**Richest modules by field count (approximate):**
1. `cognitive` — ~70 fields across 12 nested structs
2. `channels` — ~45 fields (email alone has 20)
3. `providers` — ~40 fields (13 providers × 7 fields each)
4. `productivity` — ~30 fields
5. `launcher` — ~28 fields (17 source configs)
6. `voice` — ~28 fields
