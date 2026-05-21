# Backend Hardcoded Values Scan

> **Purpose:** Inventory of values hardcoded outside `crates/config/` that are candidates for user-configurable settings.
> **Scope:** All crates except `crates/config/` and `desktop-ui/`. Search terms: `const`, `from_secs`, `from_millis`, `Duration::`, `max_`, `timeout`, `interval`, `_LIMIT`, `allowed_channels`, `approval_class`, model ID strings.
> **Recommendation legend:** `Expose` = strong case for user config; `Maybe` = situational; `Leave` = internal implementation detail.

---

## crates/agent

### agent/src/subagent_runtime.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEFAULT_TURN_CAP` | `agent/src/subagent_runtime.rs:20` | `500` | Maximum turns a subagent can take before being killed | **Expose** | `agents.defaults.subagent_max_turns` |

### agent/src/agent_profile/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEFAULT_MAX_ITERATIONS` | `agent/src/agent_profile/types.rs:6` | `10` | Default max tool iterations for agent profiles without explicit setting | **Expose** | `agents.defaults.max_iterations` |

### agent/src/agent_loop/mod.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `CORRECTION_WINDOW_MINUTES` | `agent/src/agent_loop/mod.rs:28` | `15` | Time window (minutes) in which user messages are treated as corrections to prior turn | **Maybe** | `agents.correction_window_minutes` |

### agent/src/execution/core.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `LONG_RUNNING_TOOL_TIMEOUT` | `agent/src/execution/core.rs:54` | `600s` | Timeout for tools marked as long-running (e.g., bash, subagent) | **Expose** | `agents.defaults.long_running_tool_timeout_secs` |
| `MAX_CONCURRENT_TOOLS` | `agent/src/execution/core.rs:60` | `10` | Semaphore capacity for parallel tool calls in one turn | **Maybe** | `agents.defaults.max_concurrent_tools` |
| `MAX_TOOL_RESULT_LENGTH` | `agent/src/execution/core.rs:65` | `50_000 bytes` | Hard truncation limit for tool output before sending to LLM | **Maybe** | `agents.defaults.max_tool_result_bytes` |

### agent/src/agent_loop/builder.rs (cognitive sub-model params)

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Graph-link `max_tokens` | `agent/src/agent_loop/builder.rs:489` | `2048` | Max tokens for graph-linker LLM call | **Leave** | — |
| Graph-link `temperature` | `agent/src/agent_loop/builder.rs:490` | `0.1` | Temperature for graph-linker LLM call | **Leave** | — |
| Critic `max_tokens` | `agent/src/agent_loop/builder.rs:504` | `1024` | Max tokens for extraction-critic LLM call | **Leave** | — |
| Critic `temperature` | `agent/src/agent_loop/builder.rs:505` | `0.0` | Temperature for extraction-critic LLM call | **Leave** | — |
| PPR cache TTL | `agent/src/agent_loop/builder.rs:750` | `300s` | Personalised PageRank graph cache TTL | **Leave** | — |
| Temporal pruner `max_tokens` | `agent/src/agent_loop/builder.rs:783` | `512` | Max tokens for temporal-pruner LLM call | **Leave** | — |
| Query-predictor `max_tokens` | `agent/src/agent_loop/builder.rs:1751` | `256` | Max tokens for query-predictor LLM call | **Leave** | — |
| Query-predictor `temperature` | `agent/src/agent_loop/builder.rs:1752` | `0.5` | Temperature for query-predictor LLM call | **Leave** | — |

### agent/src/context_sources/bootstrap.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_BOOTSTRAP_TOKENS_PER_FILE` | `agent/src/context_sources/bootstrap.rs:29` | `2000` | Token cap per bootstrap file injected into context | **Maybe** | `cognitive.bootstrap_max_tokens_per_file` |
| `MAX_BOOTSTRAP_TOKENS_TOTAL` | `agent/src/context_sources/bootstrap.rs:32` | `8000` | Total token budget for all bootstrap files combined | **Maybe** | `cognitive.bootstrap_max_tokens_total` |

### agent/src/adapters/semantic_fact_search.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_WIDEN_RESULTS` | `agent/src/adapters/semantic_fact_search.rs:53` | `6` | Result count when widening semantic search | **Leave** | — |
| `MIN_WIDEN_SCORE` | `agent/src/adapters/semantic_fact_search.rs:54` | `0.30` | Score threshold for widened results | **Leave** | — |

### agent/src/autotuner/hooks.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `ACTIVE_TRIALS_CACHE_TTL_SECS` | `agent/src/autotuner/hooks.rs:40` | `60s` | How long autotuner trial data is cached in memory | **Leave** | — |

### agent/src/adapters/note_tree_builder.rs / community_builder.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEBOUNCE_SECS` | `agent/src/adapters/note_tree_builder.rs:17` | `3s` | Debounce delay before rebuilding note tree on file change | **Leave** | — |
| `DEBOUNCE_DURATION` | `agent/src/adapters/community_builder.rs:23` | `10s` | Debounce before community rebuild triggers | **Leave** | — |

### agent/src/adapters/llm_rerank.rs / multi_query.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| LLM reranker `max_tokens` | `agent/src/adapters/llm_rerank.rs:119` | `100` | Output budget for reranker LLM call | **Leave** | — |
| Multi-query `temperature` | `agent/src/adapters/multi_query.rs:138` | `0.7` | Temperature for generating query variants | **Leave** | — |
| Query-rewriter `max_tokens` | `agent/src/adapters/query_rewriter.rs:533` | `50` | Output budget for query rewriter | **Leave** | — |
| Query-rewriter `temperature` | `agent/src/adapters/query_rewriter.rs:534` | `0.0` | Determinism for query rewriting | **Leave** | — |

### agent/src/adapters/productivity.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Productivity short summary `max_tokens` | `agent/src/adapters/productivity.rs:31` | `256` | Tokens for short productivity summary LLM call | **Leave** | — |
| Productivity medium summary `max_tokens` | `agent/src/adapters/productivity.rs:48` | `512` | Tokens for longer productivity summary LLM call | **Leave** | — |
| Productivity label `max_tokens` | `agent/src/adapters/productivity.rs:70` | `32` | Tokens for category label assignment | **Leave** | — |

### agent/src/learning/

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_THRESHOLD_HISTORY` | `agent/src/learning/types.rs:15` | `100` | In-memory history entries for adaptive thresholds | **Leave** | — |
| `MAX_THRESHOLD_STEP` | `agent/src/learning/adaptive.rs:11` | `0.05` | Max delta per learning adjustment step | **Leave** | — |
| `MIN_INTERACTIONS_FOR_ANALYSIS` | `agent/src/learning/pattern_analyzer.rs:12` | `10` | Min session count before learning analysis fires | **Maybe** | `learning.min_interactions_for_analysis` |
| `MIN_PATTERN_OCCURRENCES` | `agent/src/learning/pattern_analyzer.rs:15` | `5` | Min occurrences before a pattern is recognised | **Maybe** | `learning.min_pattern_occurrences` |

---

## crates/providers

### providers/src/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEFAULT_CONTEXT_WINDOW` | `providers/src/types.rs:652` | `128_000` | Fallback context window when provider doesn't specify | **Leave** | — |

### providers/src/adapters/anthropic_native.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `ANTHROPIC_CONTEXT_WINDOW` | `providers/src/adapters/anthropic_native.rs:22` | `200_000` | Context window reported for Anthropic native adapter | **Leave** | — |
| HTTP client timeout | `providers/src/adapters/anthropic_native.rs:148` | `120s` | Global reqwest client timeout for Anthropic API calls | **Maybe** | `agents.provider_http_timeout_secs` |
| Model list timeout | `providers/src/adapters/anthropic_native.rs:1035` | `5s` | Timeout for listing available models | **Leave** | — |
| Model info timeout | `providers/src/adapters/anthropic_native.rs:1080` | `10s` | Timeout for fetching model metadata | **Leave** | — |
| Default model | `providers/src/adapters/anthropic_native.rs:173` | `claude-sonnet-4-20250514` | Fallback model when none is configured | **Expose** | `agents.defaults.model` (already exists) |

### providers/src/adapters/openai_compat.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| HTTP client timeout | `providers/src/adapters/openai_compat.rs:40` | `120s` | Global reqwest client timeout for OpenAI API calls | **Maybe** | `agents.provider_http_timeout_secs` |
| Model list / info timeouts | `providers/src/adapters/openai_compat.rs:555,586` | `5s / 10s` | Admin timeouts; not user-tunable | **Leave** | — |

### providers/src/manager.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Retry backoff delays | `providers/src/manager.rs:179–181,291–292` | `500ms, 1s, 2s` | HTTP retry backoff schedule for rate-limited requests | **Maybe** | `agents.retry_backoff_ms` (array) |

### providers/src/registry.rs (default models per provider slot)

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| OpenRouter default model | `providers/src/registry.rs:87` | `anthropic/claude-sonnet-4` | Default model for OpenRouter slot | **Expose** | `agents.defaults.model` (via provider selection) |
| OpenAI default model | `providers/src/registry.rs:105` | `gpt-4o` | Default model for OpenAI slot | **Expose** | `agents.defaults.model` |
| Gemini default model | `providers/src/registry.rs:179` | `gemini-2.0-flash` | Default model for Gemini slot | **Expose** | `agents.defaults.model` |

---

## crates/approval

### approval/src/preview.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_DIFF_LINES` | `approval/src/preview.rs:63` | `200` | Max diff lines shown in approval preview | **Maybe** | `agents.approval_preview_max_diff_lines` |
| `MAX_BODY_CHARS` | `approval/src/preview.rs:64` | `500` | Max body characters in approval preview | **Leave** | — |
| `MAX_COMMAND_CHARS` | `approval/src/preview.rs:65` | `4_000` | Max command characters in approval preview | **Leave** | — |

---

## crates/channels

### channels/src/adapters/telegram_approval.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `APPROVAL_TIMEOUT` | `channels/src/adapters/telegram_approval.rs:23` | `600s` | How long Telegram waits for user to approve a tool call | **Expose** | `agents.approval_timeout_secs` |

### channels/src/adapters/telegram.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Long-poll timeout | `channels/src/adapters/telegram.rs:156` | `30s` | Telegram getUpdates long-poll timeout | **Leave** | — |
| Typing indicator interval | `channels/src/adapters/telegram.rs:547` | `4s` | How often Telegram typing action is re-sent | **Leave** | — |
| Error retry sleep | `channels/src/adapters/telegram.rs:179` | `5s` | Sleep after connection error before retry | **Leave** | — |

### channels/src/adapters/discord.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Typing indicator interval | `channels/src/adapters/discord.rs:146` | `8s` | How often Discord typing indicator is re-sent | **Leave** | — |

### channels/src/adapters/slack.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| WS heartbeat timeout | `channels/src/adapters/slack.rs:583` | `35s` | Slack WebSocket heartbeat timeout | **Leave** | — |

### channels/src/adapters/email.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Poll interval floor | `channels/src/adapters/email.rs:439` | `5s minimum` | Floor applied to configured `poll_interval_seconds`; config exists | **Leave** | — |

### channels/src/shared/interaction.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Interactive-input timeout | `channels/src/shared/interaction.rs:63` | `300s` | Max wait for a follow-up message in channel interaction | **Maybe** | `channels.interactive_timeout_secs` |

---

## crates/session

### session/src/manager.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `COMPACTION_THRESHOLD` | `session/src/manager.rs:203` | `200` | Message count that triggers DB compaction of a session | **Maybe** | `conversation.session.compaction_threshold` |
| `COMPACTION_KEEP` | `session/src/manager.rs:206` | `100` | Messages kept after DB compaction | **Maybe** | `conversation.session.compaction_keep` |
| `IN_MEMORY_TRIM_THRESHOLD` | `session/src/manager.rs:211` | `60` | Message count triggering in-memory trim | **Leave** | — |
| `IN_MEMORY_TRIM_KEEP` | `session/src/manager.rs:214` | `40` | Messages retained after in-memory trim | **Leave** | — |

---

## crates/context_engine

### context_engine/src/budget.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `LOW_BUDGET_THRESHOLD` | `context_engine/src/budget.rs:6` | `0.15` (15%) | Fraction of context budget at which "low budget" warning triggers | **Maybe** | `cognitive.context_low_budget_fraction` |

### context_engine/src/assembler/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEFAULT_MEMORY_RETRIEVAL_LIMIT` | `context_engine/src/assembler/types.rs:24` | `30` | Max memory facts retrieved per context assembly | **Expose** | `cognitive.memory_retrieval_limit` |

### context_engine/src/history_compressor/tiered.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MIN_COMPACTABLE_TOKENS` | `context_engine/src/history_compressor/tiered.rs:330` | `50` | Minimum tokens a tool block must have to be eligible for compaction | **Leave** | — |
| `MICROCOMPACT_SNIPPET_LEN` | `context_engine/src/history_compressor/tiered.rs:333` | `150` | Character length of micro-compact snippets | **Leave** | — |

### context_engine/src/book_index/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_nodes` | `context_engine/src/book_index/types.rs:149` | `50` | Max nodes in book index graph | **Leave** | — |
| `max_map_nodes` | `context_engine/src/book_index/types.rs:150` | `10` | Max map/summary nodes | **Leave** | — |
| `operator_timeout_ms` | `context_engine/src/book_index/types.rs:151` | `600ms` | Timeout for book-index operator | **Leave** | — |

### context_engine/src/insight_forge/mod.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_sub_queries` default | `context_engine/src/insight_forge/mod.rs:45` | `5` | Max sub-queries in InsightForge decomposition (also in config) | **Leave** (already configurable via `cognitive.insight_forge_max_sub_queries`) | — |
| `per_source_timeout_ms` default | `context_engine/src/insight_forge/mod.rs:48` | `800ms` | Per-source retrieval timeout (also in config) | **Leave** (already `cognitive.insight_forge_per_source_timeout_ms`) | — |
| `decomposer_timeout_ms` default | `context_engine/src/insight_forge/mod.rs:49` | `2000ms` | Decomposer LLM timeout | **Maybe** | `cognitive.insight_forge_decomposer_timeout_ms` |

### context_engine/src/enhancement/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Minimal budget `max_latency_ms` | `context_engine/src/enhancement/types.rs:86` | `100ms` | Retrieval budget for minimal-latency mode | **Leave** | — |
| Balanced budget `max_latency_ms` | `context_engine/src/enhancement/types.rs:95` | `500ms` | Retrieval budget for balanced mode | **Leave** | — |
| Thorough budget `max_latency_ms` | `context_engine/src/enhancement/types.rs:104` | `1000ms` | Retrieval budget for thorough mode | **Leave** | — |

---

## crates/scheduling

### scheduling/src/temporal/scheduler.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_SLEEP` | `scheduling/src/temporal/scheduler.rs:33` | `30s` | Maximum sleep between scheduler wake-ups | **Leave** | — |
| `DEFAULT_GRACE_SECS` | `scheduling/src/temporal/scheduler.rs:34` | `3600s` | Default misfire grace period for cron jobs | **Maybe** | `scheduling.default_grace_secs` |

### scheduling/src/types.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Default schedule tolerance | `scheduling/src/types.rs:349,387` | `7200s` | Tolerance window for recurrence schedule misfire | **Leave** | — |

---

## crates/cognitive

### cognitive/src/pipeline/consolidator.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `GROUPING_THRESHOLD` | `cognitive/src/pipeline/consolidator.rs:11` | `0.4` | Cosine-similarity threshold for grouping signals into clusters | **Leave** | — |
| Coaching confidence threshold | `cognitive/src/pipeline/consolidator.rs:91` | `0.7` | Min confidence to emit a coaching signal | **Leave** | — |
| Fact triple confidence threshold | `cognitive/src/pipeline/consolidator.rs:97` | `0.6` | Min confidence to emit a knowledge triple | **Leave** | — |
| Generic fact confidence threshold | `cognitive/src/pipeline/consolidator.rs:116` | `0.5` | Min confidence for loose semantic facts | **Leave** | — |

### cognitive/src/pipeline/recall_collector.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `BUFFER_FLUSH_SIZE` | `cognitive/src/pipeline/recall_collector.rs:26` | `20` | Recall collector flushes when buffer reaches this size | **Leave** | — |
| `OVERLAP_THRESHOLD` | `cognitive/src/pipeline/recall_collector.rs:28` | `0.5` | Dedup overlap threshold for recalled messages | **Leave** | — |

### cognitive/src/pipeline/chat_turn_collector.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MIN_MESSAGE_LEN` | `cognitive/src/pipeline/chat_turn_collector.rs:16` | `20` | Messages shorter than this are skipped for extraction | **Leave** | — |

### cognitive/src/repos/co_activation.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `STRENGTH_THRESHOLD` | `cognitive/src/repos/co_activation.rs:15` | `2.0` | Min co-activation strength to be considered a strong link | **Leave** | — |

### cognitive/src/services/session_memory.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Session memory `max_tokens` | `cognitive/src/services/session_memory.rs:200` | `800` | Output budget for session-level memory summarization | **Leave** | — |
| Session memory `temperature` | `cognitive/src/services/session_memory.rs:199` | `0.2` | Temperature for session memory summarization | **Leave** | — |

---

## crates/app-core

### app-core/src/desktop_approval_channel.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `APPROVAL_TIMEOUT` | `app-core/src/desktop_approval_channel.rs:19` | `600s` | How long desktop UI approval dialog waits | **Expose** | `agents.approval_timeout_secs` |

### app-core/src/brain_voice.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_pulses_per_hour` | `app-core/src/brain_voice.rs:66` | `2` | Max ambient-brain voice pulses allowed per hour | **Expose** | `voice.brain_ambient.max_pulses_per_hour` |
| `merge_window` | `app-core/src/brain_voice.rs:67` | `30s` | Window for coalescing back-to-back brain voice triggers | **Maybe** | `voice.brain_ambient.merge_window_secs` |
| `dampened_max_pulses` | `app-core/src/brain_voice.rs:69` | `1` | Max pulses per hour when user is in DND/dampened mode | **Expose** | `voice.brain_ambient.dampened_max_pulses_per_hour` |
| `dampened_merge_window` | `app-core/src/brain_voice.rs:70` | `60s` | Merge window while dampened | **Maybe** | `voice.brain_ambient.dampened_merge_window_secs` |

### app-core/src/init/cron.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Cron tolerance for episodic rollups | `app-core/src/init/cron.rs:1436,1444` | `14400s` | Misfire grace window for hourly/daily episodic jobs | **Leave** | — |
| Cron tolerance micro-reforge | `app-core/src/init/cron.rs:1452` | `300s` | Misfire grace for micro-reforge job | **Leave** | — |
| Weekly digest cron tolerance | `app-core/src/init/cron.rs:1460` | `86400s` | Misfire grace for weekly knowledge digest | **Leave** | — |
| Weekly report `max_tokens` | `app-core/src/init/cron.rs:1628` | `500` | Output budget for weekly report generation | **Leave** | — |

### app-core/src/init/storage.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| WAL checkpoint interval | `app-core/src/init/storage.rs:58` | `300s` | How often the WAL checkpoint runs | **Leave** | — |
| `MAX_CONV_ROWS` | `app-core/src/init/storage.rs:65` | `10_000` | Hard cap on conversation_messages rows before emergency prune | **Maybe** | `conversation.session.hard_message_cap` |

### app-core/src/init/temporal_scheduler.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `DEFAULT_MATERIALIZE_AHEAD` | `app-core/src/init/temporal_scheduler.rs:21` | `3` | Days ahead to materialise recurring schedule occurrences | **Leave** | — |

---

## crates/desktop

### desktop/src/lazy_window.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Launcher window size | `desktop/src/lazy_window.rs:71` | `660 × 580` | Launcher window inner size in logical pixels | **Expose** | `ui.launcher.width` / `ui.launcher.height` |
| Tray window size | `desktop/src/lazy_window.rs:98` | `320 × 600` | Tray popover window inner size | **Maybe** | `ui.tray.width` / `ui.tray.height` |
| Distraction-overlay size | `desktop/src/lazy_window.rs:121` | `340 × 300` | Distraction overlay window inner size | **Maybe** | `ui.distraction_overlay.width/height` |
| Voice-orb window size | `desktop/src/lazy_window.rs:138` | `200 × 200` | Voice orb floating window size | **Maybe** | `ui.voice_orb.size` |
| Settings window size | `desktop/src/lazy_window.rs:168` | `1200 × 800` | Settings window inner size | **Maybe** | `ui.settings.width/height` |

### desktop/src/shortcuts.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Launcher window fallback size | `desktop/src/shortcuts.rs:130` | `660 × 580` | Duplicate of lazy_window default; used when persisted size unavailable | **Expose** (same as above) | `ui.launcher.width/height` |

### desktop/src/focus_timer.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `WARNING_SECS` | `desktop/src/focus_timer.rs:284` | `30s` | Seconds before session end that focus-end warning fires | **Expose** | `productivity.focus.warning_secs` |
| `BREAK_PENDING_SECS` | `desktop/src/focus_timer.rs:285` | `5s` | Countdown from break-pending state before break starts | **Leave** | — |
| `SYNC_INTERVAL` | `desktop/src/focus_timer.rs:286` | `5s` | How often the focus timer syncs elapsed time to DB | **Leave** | — |

### desktop/src/tray_countdown.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `IDLE_MAX_SLEEP_SECS` | `desktop/src/tray_countdown.rs:126` | `3600s` | Max sleep between tray countdown ticks when idle | **Leave** | — |
| `FOCUS_MAX_SLEEP_SECS` | `desktop/src/tray_countdown.rs:129` | `60s` | Max sleep during active focus session | **Leave** | — |
| `VOICE_TICK_SECS` | `desktop/src/tray_countdown.rs:132` | `2s` | Tick interval while voice is active | **Leave** | — |
| `COUNTDOWN_TICK_SECS` | `desktop/src/tray_countdown.rs:134` | `1s` | Tick interval during countdown | **Leave** | — |

### desktop/src/app_core.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Config hot-reload interval | `desktop/src/app_core.rs:148` | `5s` | How often config file watcher checks for changes | **Leave** | — |

### desktop/src/commands/status_badge.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `WIDTH` / `HEIGHT` | `desktop/src/commands/status_badge.rs:13–14` | `280 × 40` | Status badge overlay window size | **Leave** | — |
| Default display duration | `desktop/src/commands/status_badge.rs:23` | `2000ms` | How long status badge shows if not specified | **Maybe** | `ui.status_badge.default_duration_ms` |

### desktop/src/oauth/flow.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| OAuth callback server timeout | `desktop/src/oauth/flow.rs:94` | `300s` | How long the local OAuth callback server listens | **Leave** | — |

### desktop/src/dev_server/streaming.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| SSE keepalive interval | `desktop/src/dev_server/streaming.rs:122,192,229,347` | `15s` | Dev server SSE heartbeat interval | **Leave** | — |
| `SSE_CHANNEL_TTL` | `desktop/src/dev_server/streaming.rs:18` | `300s` | TTL for idle SSE channel before cleanup | **Leave** | — |

---

## crates/voice-engine

### voice-engine/src/engines/qwen3_tts.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `IDLE_UNLOAD_SECS` (TTS) | `voice-engine/src/engines/qwen3_tts.rs:24` | `300s` | Idle time before TTS model is unloaded from memory | **Expose** | `voice.model_idle_unload_secs` |
| `MAX_CHUNK_CHARS` | `voice-engine/src/engines/qwen3_tts.rs:27` | `400` | Max characters per TTS synthesis chunk | **Leave** | — |

### voice-engine/src/engines/qwen3_asr.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `IDLE_UNLOAD_SECS` (ASR) | `voice-engine/src/engines/qwen3_asr.rs:17` | `300s` | Idle time before ASR model is unloaded from memory | **Expose** | `voice.model_idle_unload_secs` |

### voice-engine/src/capture.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `silence_duration` default | `voice-engine/src/capture.rs:41` | `1500ms` | Silence duration that ends a recording session | **Expose** | `voice.silence_duration_ms` |

### voice-engine/src/router.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `ROUTING_THRESHOLD` | `voice-engine/src/router.rs:5` | `0.4` | Score threshold for routing audio to voice vs text path | **Leave** | — |

### voice-engine/src/engine_manager.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `reset_timeout_secs` default | `voice-engine/src/engine_manager.rs:29` | `30s` | Engine reset timeout before forced kill | **Leave** | — |

---

## crates/notifications

### notifications/src/retry.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_attempts` default | `notifications/src/retry.rs:14` | `3` | Default notification retry attempts | **Maybe** | `notifications.retry.max_attempts` |
| `base_delay` default | `notifications/src/retry.rs:15` | `1s` | Base delay for exponential backoff in notifications | **Maybe** | `notifications.retry.base_delay_secs` |

---

## crates/feature-tasks

### feature-tasks/src/focus_alarms.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `WARNING_HOURS` | `feature-tasks/src/focus_alarms.rs:17` | `[6, 3, 1]` | Hours before task deadline that warning alarms fire | **Expose** | `todo.focus.warning_hours` |

### feature-tasks/src/focus_watcher.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Focus watcher poll interval | `feature-tasks/src/focus_watcher.rs:20` | `60s` | How often focus watcher checks for overdue tasks | **Leave** | — |

---

## crates/feature-coaching

### feature-coaching/src/router.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_per_hour` default | `feature-coaching/src/router.rs:50` | `3` | Max coaching messages that can be delivered per hour | **Expose** | `coaching.rate_limit.max_per_hour` |
| `max_per_day` default | `feature-coaching/src/router.rs:51` | `5` | Max coaching messages per day | **Expose** | `coaching.rate_limit.max_per_day` |

### feature-coaching/src/pattern_detector/mod.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_history` | `feature-coaching/src/pattern_detector/mod.rs:36` | `100` | In-memory history size for pattern detection | **Leave** | — |

---

## crates/feature-productivity

### feature-productivity/src/batch_writer.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_BUFFER_SIZE` | `feature-productivity/src/batch_writer.rs:15` | `1000` | Max activity tick events buffered before forced flush | **Leave** | — |

### feature-productivity/src/nudge.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `BURNOUT_COOLDOWN_MULTIPLIER` | `feature-productivity/src/nudge.rs:14` | `4` | Cooldown multiplier for burnout nudge after it fires | **Leave** | — |
| Nudge check interval | `feature-productivity/src/nudge.rs:54` | `60s` | How often nudge service evaluates conditions | **Leave** | — |

### feature-productivity/src/repos/frequency.rs (feature-launcher)

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `HALF_LIFE_HOURS` | `feature-launcher/src/repos/frequency.rs:5` | `72h` | Half-life for exponential frequency decay in launcher ranking | **Leave** | — |
| Usage history cutoff (long) | `feature-launcher/src/repos/frequency.rs:38` | `90 days` | How far back launcher usage history is retained | **Maybe** | `ui.launcher.usage_history_days` |

---

## crates/feature-insights

### feature-insights/src/cross_domain.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_temporal_days` default | `feature-insights/src/cross_domain.rs:82` | `7` | Max days apart for cross-domain signal correlation | **Leave** | — |

---

## crates/mcp

### mcp/src/server/security.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_INPUT_LENGTH` | `mcp/src/server/security.rs:9` | `50_000 bytes` | Max MCP tool input size before truncation | **Leave** | — |

### mcp/src/client/sanitize.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `MAX_TOOL_NAME_LEN` | `mcp/src/client/sanitize.rs:10` | `64 chars` | Max character length of an MCP tool name | **Leave** | — |

### mcp/src/client/manager.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Health-check interval | `mcp/src/client/manager.rs:440` | `30s` | How often MCP client manager polls server health | **Leave** | — |

---

## crates/storage

### storage/src/pool.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `max_connections` | `storage/src/pool.rs:35,75` | `5` | SQLite connection pool max size | **Leave** | — |
| `busy_timeout` | `storage/src/pool.rs:41,81` | `5000ms` | SQLite busy-timeout PRAGMA value | **Leave** | — |

---

## crates/feature-focus

### feature-focus/src/duration_parser.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| `TOMORROW_HOUR` | `feature-focus/src/duration_parser.rs:15` | `9` | Hour of day used as "tomorrow morning" default | **Expose** | `productivity.focus.default_morning_hour` |

---

## crates/common

### common/src/http.rs

| Value/Constant | File:Line | Current Value | What It Controls | Recommendation | Suggested Config Domain |
|---|---|---|---|---|---|
| Default HTTP client timeout | `common/src/http.rs:20` | `60s` | Default reqwest client timeout applied globally | **Leave** | — |
| Pool idle timeout | `common/src/http.rs:22` | `30s` | Idle connection pool TTL | **Leave** | — |

---

## Top 20 Highest-Value Items to Expose as Settings

Ranked by impact (user-visible behaviour × current pain of not being configurable):

| # | Constant | Location | Current Value | Why It Matters |
|---|---|---|---|---|
| 1 | `APPROVAL_TIMEOUT` (desktop + Telegram) | `app-core/src/desktop_approval_channel.rs:19`, `channels/src/adapters/telegram_approval.rs:23` | `600s` | Users on slow/mobile connections regularly time out; Telegram bots need longer windows |
| 2 | `DEFAULT_MEMORY_RETRIEVAL_LIMIT` | `context_engine/src/assembler/types.rs:24` | `30` | Direct control over how many memories surface per turn; power users want more |
| 3 | `DEFAULT_TURN_CAP` (subagent) | `agent/src/subagent_runtime.rs:20` | `500` | Long-running coding tasks hit the cap; no way to raise without recompiling |
| 4 | Voice model idle-unload TTL | `voice-engine/src/engines/qwen3_tts.rs:24`, `qwen3_asr.rs:17` | `300s` | Users with limited RAM want faster unload; users with fast GPUs want models to stay loaded |
| 5 | Voice silence duration | `voice-engine/src/capture.rs:41` | `1500ms` | Affects perceived voice latency; too long for fast speakers, too short for deliberate pauses |
| 6 | `max_per_hour` / `max_per_day` (coaching) | `feature-coaching/src/router.rs:50–51` | `3 / 5` | Users find coaching messages intrusive or insufficient; strongly personal preference |
| 7 | `max_pulses_per_hour` (brain voice) | `app-core/src/brain_voice.rs:66` | `2` | Controls how often the ambient assistant speaks unprompted; clearly user taste |
| 8 | Launcher / tray window sizes | `desktop/src/lazy_window.rs:71,98` | `660×580 / 320×600` | HiDPI / multi-monitor users want bigger; small-screen users want smaller |
| 9 | `LONG_RUNNING_TOOL_TIMEOUT` | `agent/src/execution/core.rs:54` | `600s` | Coding tasks hitting network-bound tools need longer; users on slow CI runners affected |
| 10 | `DEFAULT_MAX_ITERATIONS` (agent profile) | `agent/src/agent_profile/types.rs:6` | `10` | Determines how many tool-call rounds the agent takes before giving up; should be per-profile |
| 11 | `WARNING_HOURS` (task focus alarms) | `feature-tasks/src/focus_alarms.rs:17` | `[6, 3, 1]` | Advance-warning schedule is highly personal; some want daily, others hourly |
| 12 | `COMPACTION_THRESHOLD` / `COMPACTION_KEEP` | `session/src/manager.rs:203,206` | `200 / 100` | Affects how much history the agent can see in long sessions; power users want higher ceilings |
| 13 | `MAX_TOOL_RESULT_LENGTH` | `agent/src/execution/core.rs:65` | `50_000 bytes` | Truncation silently hides data; users running large-codebase coding tasks frequently hit it |
| 14 | `DEFAULT_GRACE_SECS` (scheduler misfire) | `scheduling/src/temporal/scheduler.rs:34` | `3600s` | Machines that sleep for hours cause cron misfires; users may want 0 (strict) or longer |
| 15 | `MAX_CONCURRENT_TOOLS` | `agent/src/execution/core.rs:60` | `10` | Advanced users orchestrating many tools in one turn occasionally hit the semaphore cap |
| 16 | `LOW_BUDGET_THRESHOLD` | `context_engine/src/budget.rs:6` | `15%` | Controls when "context nearly full" warnings appear; some users want earlier warnings |
| 17 | `MAX_BOOTSTRAP_TOKENS_TOTAL` | `agent/src/context_sources/bootstrap.rs:32` | `8000` | Users with large workspace files want more context injected at start |
| 18 | Focus timer `WARNING_SECS` | `desktop/src/focus_timer.rs:284` | `30s` | End-of-session warning timing is personal; some want 60s, others 0s |
| 19 | `CORRECTION_WINDOW_MINUTES` | `agent/src/agent_loop/mod.rs:28` | `15 min` | Determines how long the agent treats follow-up messages as corrections; affects memory recording |
| 20 | `TOMORROW_HOUR` (focus duration parser) | `feature-focus/src/duration_parser.rs:15` | `9am` | "Tomorrow morning" default is locale and lifestyle dependent |
