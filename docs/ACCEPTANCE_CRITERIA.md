# Acceptance Criteria

> Comprehensive acceptance criteria for every gap identified in the Feature Gap Analysis.
> Cross-referenced against the **current** Rust source (as of 2026-02-12) to reflect actual status.

---

## Status Legend

| Status | Meaning |
|--------|---------|
| **RESOLVED** | Gap has been fixed in the current codebase |
| **OPEN** | Gap still exists and requires implementation |
| **PARTIAL** | Config/schema exists but runtime code doesn't use it yet |

---

## Phase 1: P0 Critical — Core Agent Loop

### GAP-2.1: Tool Context Wiring

**Status: RESOLVED**

**Evidence:** `agent_loop.rs:370-385` implements `update_tool_contexts()` which calls `set_context(channel, chat_id)` on MessageTool, SpawnTool, and CronTool before each message is processed (called at `agent_loop.rs:199`).

**Acceptance Criteria (Verification Only):**
- [ ] AC-2.1.1: `update_tool_contexts()` is called before every `process_message()` invocation
- [ ] AC-2.1.2: MessageTool receives current `(channel, chat_id)` and routes outbound messages correctly
- [ ] AC-2.1.3: SpawnTool receives current context so subagent results route back to the originating channel/chat
- [ ] AC-2.1.4: CronTool receives current context so scheduled jobs reference the correct origin

**Test Scenarios:**
1. Send a message via Telegram → agent uses MessageTool → reply arrives in same Telegram chat
2. Send a message via Discord → agent spawns subagent → subagent result routes back to same Discord channel
3. Send a message from CLI → agent uses CronTool → cron job stores correct CLI origin

**Edge Cases:**
- Rapid messages from different channels interleaving — context must not leak between concurrent messages
- Tool context set but message processing fails — context should not persist stale values

---

### GAP-2.2: Discord IDENTIFY Not Sent

**Status: RESOLVED**

**Evidence:** `discord.rs:136-159` sends IDENTIFY payload with token and intents after receiving HELLO opcode.

**Acceptance Criteria (Verification Only):**
- [ ] AC-2.2.1: After WebSocket HELLO (opcode 10), IDENTIFY (opcode 2) is sent with bot token and intents
- [ ] AC-2.2.2: Heartbeat loop starts using the interval from HELLO payload
- [ ] AC-2.2.3: Bot transitions to READY state and can receive MESSAGE_CREATE events

**Test Scenarios:**
1. Start Discord channel → connects to gateway → receives HELLO → sends IDENTIFY → receives READY
2. Network disconnect → reconnect → re-IDENTIFY → resume receiving messages

**Edge Cases:**
- Invalid token → IDENTIFY rejected → graceful error with clear message
- Gateway sends unexpected opcode before HELLO → handle gracefully

---

### GAP-2.3: Subagent Has No Tool Access

**Status: RESOLVED**

**Evidence:** `subagent.rs:142-180` creates a ToolRegistry with 6 tools (ReadFileTool, WriteFileTool, ListDirTool, ExecTool, WebSearchTool, WebFetchTool) and runs a 15-iteration agent loop.

**Acceptance Criteria (Verification Only):**
- [ ] AC-2.3.1: Subagent receives 6 tools: read_file, write_file, list_dir, exec, web_search, web_fetch
- [ ] AC-2.3.2: Subagent runs up to 15 tool iterations per task
- [ ] AC-2.3.3: Subagent does NOT have message, spawn, or cron tools (isolation guarantee)
- [ ] AC-2.3.4: Subagent results are announced back via system InboundMessage to `"channel:chat_id"` format

**Test Scenarios:**
1. SpawnTool creates subagent with task "read file X" → subagent uses ReadFileTool → result announced
2. Subagent reaches 15 iterations → stops gracefully with partial result
3. Subagent tool call fails → error captured in result, not propagated to parent crash

**Edge Cases:**
- Subagent workspace path doesn't exist → ReadFileTool returns clear error
- Subagent ExecTool runs command that exceeds timeout → killed gracefully

---

### GAP-2.4: SpawnTool and CronTool Reference None

**Status: RESOLVED**

**Evidence:** `agent_loop.rs:120` creates `SpawnTool::with_manager(subagent_manager.clone())` and `agent_loop.rs:122-127` creates `CronTool::with_service(cron_svc)` via `new_with_cron()` constructor.

**Acceptance Criteria (Verification Only):**
- [ ] AC-2.4.1: SpawnTool is initialized with a valid `SubagentManager` reference
- [ ] AC-2.4.2: CronTool is initialized with a valid `CronService` reference
- [ ] AC-2.4.3: Calling SpawnTool.execute() spawns a real subagent task (not a no-op)
- [ ] AC-2.4.4: Calling CronTool.execute() creates a real cron job (not a no-op)

**Test Scenarios:**
1. Agent receives "remind me in 5 minutes" → CronTool creates job → job fires after 5 min
2. Agent receives "research topic X in background" → SpawnTool creates subagent → subagent runs

**Edge Cases:**
- CronService not started yet when CronTool is called → should return error, not panic
- SubagentManager has reached max concurrent tasks → SpawnTool returns clear limit error

---

## Phase 2: P1 Major — Channel Gaps

### GAP-3.1: Telegram Channel Incomplete

**Sub-gap 3.1a: Continuous Typing Indicator — RESOLVED**
`telegram.rs:430-465` sends typing action every 4 seconds during processing.

**Sub-gap 3.1b: Proxy Support — RESOLVED**
`telegram.rs:38-49` configures reqwest client with proxy from `config.proxy`.

**Sub-gap 3.1c: HTML Fallback on Parse Error — RESOLVED**
`telegram.rs:694-714` catches MarkdownV2 parse errors and retries with HTML.

**Sub-gap 3.1d: /reset Clears Session — OPEN**

**Evidence:** `telegram.rs:401-409` shows `/reset` command only sends acknowledgment text with `TODO: Integrate with session manager for proper session reset`. Session is NOT actually cleared.

**Acceptance Criteria:**
- [ ] AC-3.1d.1: `/reset` command clears the current session's message history
- [ ] AC-3.1d.2: `/reset` saves the cleared session to disk via SessionManager
- [ ] AC-3.1d.3: `/reset` sends confirmation message: "Conversation history has been reset."
- [ ] AC-3.1d.4: Next message after `/reset` starts a fresh conversation (no prior context in LLM call)

**Implementation Notes:**
The Telegram channel needs access to SessionManager (or the MessageBus needs a reset mechanism). In Python nanobot, the Telegram handler holds a reference to `session_manager` and calls `session.clear()` + `session_manager.save(session)`.

**Test Scenarios:**
1. Send several messages → /reset → send new message → LLM response has no knowledge of prior messages
2. /reset in empty session → no error, confirmation sent
3. /reset → verify session file on disk is updated (empty history)

**Edge Cases:**
- /reset while agent is processing a message → should queue reset after current processing completes
- /reset in group chat → should only clear that specific chat's session
- Multiple rapid /reset commands → idempotent, no errors

---

### GAP-3.2: Discord Channel Gaps

**Sub-gap 3.2a: Attachment Download — RESOLVED**
`discord.rs:298-371` downloads attachments with 20MB limit to `~/.klyntbot/media/`.

**Sub-gap 3.2b: Typing Indicators — RESOLVED**
`discord.rs:225-254` sends typing indicator every 8 seconds.

**Sub-gap 3.2c: Shared WebSocket Write — RESOLVED**
`discord.rs:62` uses `Arc<Mutex<write>>` for shared WebSocket access.

**Sub-gap 3.2d: Gateway URL and Intents From Config — PARTIAL**

**Evidence:** Config schema (`schema.rs:155-179`) has `gateway_url` and `intents` fields on DiscordConfig with correct defaults. However, `discord.rs:23-24` still uses hardcoded constants `GATEWAY_URL` and `INTENTS` instead of reading from `self.config.gateway_url` and `self.config.intents`.

**Acceptance Criteria:**
- [ ] AC-3.2d.1: Discord channel reads `gateway_url` from `self.config.gateway_url` instead of hardcoded constant
- [ ] AC-3.2d.2: Discord channel reads `intents` from `self.config.intents` instead of hardcoded constant
- [ ] AC-3.2d.3: Hardcoded `GATEWAY_URL` and `INTENTS` constants are removed from `discord.rs`
- [ ] AC-3.2d.4: Default values in config schema match the current hardcoded values (wss://gateway.discord.gg/?v=10&encoding=json, 37377)

**Test Scenarios:**
1. Default config → Discord connects to default gateway URL with default intents (37377)
2. Custom gateway_url in config → Discord connects to custom URL
3. Custom intents in config → IDENTIFY payload uses custom intents value

**Edge Cases:**
- Empty gateway_url in config → should fall back to default, not empty string
- Intents = 0 → should warn but still connect (user may intentionally want minimal intents)

---

### GAP-3.3: Config Schema Missing Fields

**Sub-gap 3.3a: Missing Channel Configs — OPEN**

**Evidence:** `schema.rs:104-122` ChannelsConfig has telegram, discord, whatsapp, slack, email, qq. Missing: `feishu`, `dingtalk`, `mochat`. Python nanobot has FeishuConfig, DingTalkConfig, MochatConfig.

**Acceptance Criteria:**
- [ ] AC-3.3a.1: `ChannelsConfig` includes `feishu: FeishuConfig` field with `#[serde(default)]`
- [ ] AC-3.3a.2: `ChannelsConfig` includes `dingtalk: DingTalkConfig` field with `#[serde(default)]`
- [ ] AC-3.3a.3: `ChannelsConfig` includes `mochat: MochatConfig` field with `#[serde(default)]`
- [ ] AC-3.3a.4: Each config struct has `enabled: bool`, `token/webhook: String`, `allow_from: Vec<String>` matching Python schema
- [ ] AC-3.3a.5: Existing configs deserialize without error when new fields are absent (serde default)

**Test Scenarios:**
1. Config JSON without feishu/dingtalk/mochat → deserializes with defaults (enabled: false)
2. Config JSON with feishu config → FeishuConfig populated correctly
3. Round-trip serialization → all fields preserved

**Edge Cases:**
- Config with unknown channel name → serde should ignore (or deny_unknown_fields not set)

---

**Sub-gap 3.3b: Missing Provider Configs — OPEN**

**Evidence:** `schema.rs:414-435` ProvidersConfig has anthropic, openai, openrouter, deepseek, gemini, groq, vllm. Missing: `zhipu`, `dashscope`, `moonshot`, `minimax`, `aihubmix`. Python nanobot supports 12 providers total.

**Acceptance Criteria:**
- [ ] AC-3.3b.1: `ProvidersConfig` includes `zhipu: ProviderConfig`
- [ ] AC-3.3b.2: `ProvidersConfig` includes `dashscope: ProviderConfig`
- [ ] AC-3.3b.3: `ProvidersConfig` includes `moonshot: ProviderConfig`
- [ ] AC-3.3b.4: `ProvidersConfig` includes `minimax: ProviderConfig`
- [ ] AC-3.3b.5: `ProvidersConfig` includes `aihubmix: ProviderConfig`
- [ ] AC-3.3b.6: All new providers use `#[serde(default)]` for backward compatibility

**Test Scenarios:**
1. Config without new providers → defaults (empty api_key, no api_base)
2. Config with zhipu api_key → properly deserialized and accessible

---

**Sub-gap 3.3c: Missing ProviderConfig.extra_headers — OPEN**

**Evidence:** `schema.rs:440-446` ProviderConfig has `api_key` and `api_base`. Python nanobot's ProviderConfig also has `extra_headers: Dict[str, str]` for custom HTTP headers.

**Acceptance Criteria:**
- [ ] AC-3.3c.1: `ProviderConfig` includes `extra_headers: HashMap<String, String>` with `#[serde(default)]`
- [ ] AC-3.3c.2: LLM provider HTTP clients inject `extra_headers` into every API request
- [ ] AC-3.3c.3: Empty extra_headers → no extra headers added (no-op)

**Test Scenarios:**
1. Provider with extra_headers `{"X-Custom": "value"}` → header present in API requests
2. Provider without extra_headers → requests work normally
3. Round-trip serialization preserves extra_headers

**Edge Cases:**
- extra_headers with reserved header names (Authorization, Content-Type) → should they override or be rejected?

---

**Sub-gap 3.3d: Missing GatewayConfig — OPEN**

**Evidence:** `schema.rs:10-22` Config has agents, channels, providers, tools. Python nanobot has a `gateway` section with `host` and `port` for the HTTP gateway server.

**Acceptance Criteria:**
- [ ] AC-3.3d.1: `Config` struct includes `gateway: GatewayConfig` field with `#[serde(default)]`
- [ ] AC-3.3d.2: `GatewayConfig` has `host: String` (default "0.0.0.0") and `port: u16` (default 8080)
- [ ] AC-3.3d.3: Gateway server (when implemented) reads host/port from config

**Test Scenarios:**
1. Config without gateway section → defaults to 0.0.0.0:8080
2. Custom host/port → gateway binds to specified address

---

### GAP-3.4: CLI REPL Incomplete

**Status: RESOLVED**

**Evidence:** `main.rs:77-78` imports rustyline, `main.rs:127-240` implements full interactive REPL with readline, history, /help, /status, /model, /clear, /quit commands.

**Acceptance Criteria (Verification Only):**
- [ ] AC-3.4.1: `cargo run -- chat` launches interactive REPL with rustyline
- [ ] AC-3.4.2: Command history persists to `~/.klyntbot/history.txt`
- [ ] AC-3.4.3: /help, /status, /model, /clear, /quit commands work
- [ ] AC-3.4.4: Ctrl+C and Ctrl+D are handled gracefully (interrupt and exit)

---

### GAP-3.5: Heartbeat Callback Not Wired

**Status: RESOLVED**

**Evidence:** `main.rs:335-362` creates HeartbeatService, sets callback that publishes InboundMessage through the bus, starts the service, and stops it on shutdown.

**Acceptance Criteria (Verification Only):**
- [ ] AC-3.5.1: HeartbeatService reads `~/.klyntbot/HEARTBEAT.md` on each tick
- [ ] AC-3.5.2: If HEARTBEAT.md has actionable content, callback publishes system InboundMessage to bus
- [ ] AC-3.5.3: Agent processes heartbeat message through normal agent loop
- [ ] AC-3.5.4: If HEARTBEAT.md is empty/template-only, no message is published (HEARTBEAT_OK path)

---

## Phase 3: P1 Major — Agent Processing Gaps

### GAP-3.6: process_system_message Does Not Run Through LLM

**Status: OPEN**

**Evidence:** `agent_loop.rs:336-363` `process_system_message()` only saves the subagent result to the session as a system message and returns. It does NOT run the result through the LLM to generate a natural-language response to send back to the user. In Python nanobot (`loop.py:210-240`), `_process_system_message()` runs a full agent loop on the subagent result, producing a natural response that gets routed back to the originating channel.

**Acceptance Criteria:**
- [ ] AC-3.6.1: When a subagent completes and sends a system message, the result is processed through the LLM
- [ ] AC-3.6.2: The LLM generates a natural-language summary/response from the subagent result
- [ ] AC-3.6.3: The generated response is published as an OutboundMessage to the originating channel/chat_id
- [ ] AC-3.6.4: The subagent result AND the LLM response are both saved to the session
- [ ] AC-3.6.5: Tool context is set correctly for the originating channel before LLM processing

**Implementation Notes:**
The current code correctly parses `channel:chat_id` from `msg.chat_id` and saves to session. It needs to additionally:
1. Build context from session
2. Call the LLM with the subagent result
3. Execute any tool calls in the response (up to max iterations)
4. Publish the final assistant response to the originating channel

**Test Scenarios:**
1. Spawn subagent "research Rust async" → subagent completes → user receives natural summary in original chat
2. Subagent result is very long → LLM summarizes appropriately
3. LLM response from system message includes tool calls → tools execute correctly

**Edge Cases:**
- Subagent result arrives after user has switched to a different conversation → should still route to original chat
- Multiple subagent results arrive simultaneously → each processed independently
- LLM fails during system message processing → error logged, subagent result still saved to session
- System message with invalid `channel:chat_id` format → warning logged, no crash

---

## Phase 4: P1 Major — Config and Migration

### GAP-3.7: Config Loader Missing Nanobot Fallback

**Status: OPEN**

**Evidence:** `loader.rs:25-38` `load()` only checks `~/.klyntbot/config.json`. Python nanobot's loader also checks `~/.nanobot/config.json` for migration.

**Acceptance Criteria:**
- [ ] AC-3.7.1: If `~/.klyntbot/config.json` does not exist, check `~/.nanobot/config.json`
- [ ] AC-3.7.2: If nanobot config found, load it, apply key migrations (see AC-3.7.3), save to klyntbot path
- [ ] AC-3.7.3: Migration converts `tools.exec.restrictToWorkspace` → `tools.restrictToWorkspace` (matching Python's `_migrate_config`)
- [ ] AC-3.7.4: If neither config exists, return `Config::default()` (current behavior preserved)
- [ ] AC-3.7.5: Log info message when migrating from nanobot config

**Test Scenarios:**
1. Only `~/.nanobot/config.json` exists → loaded and migrated to `~/.klyntbot/config.json`
2. Both exist → `~/.klyntbot/config.json` takes precedence
3. Neither exists → default config returned
4. Nanobot config with legacy field names → migrated correctly

**Edge Cases:**
- Nanobot config is corrupted JSON → skip migration, use defaults, log warning
- Nanobot config has camelCase keys → convert to snake_case during load (as Python does with `convert_keys`)

---

### GAP-3.8: Config Loader Limited Env Var Overrides

**Status: OPEN**

**Evidence:** `loader.rs:60-89` `load_with_env_overrides()` only handles 6 specific env vars. Python nanobot uses Pydantic's `env_prefix = "NANOBOT_"` with `env_nested_delimiter = "__"` for automatic env var mapping of ANY config field.

**Acceptance Criteria:**
- [ ] AC-3.8.1: Add env var overrides for ALL provider API keys (including new providers: zhipu, dashscope, moonshot, minimax, aihubmix)
- [ ] AC-3.8.2: Add env var overrides for channel tokens: `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN`, `KLYNTBOT_CHANNELS__DISCORD__TOKEN`, etc.
- [ ] AC-3.8.3: Add env var override for `KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE`
- [ ] AC-3.8.4: Add env var override for `KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS`
- [ ] AC-3.8.5: Document the env var naming convention: `KLYNTBOT_` prefix, `__` for nesting, UPPER_SNAKE_CASE

**Test Scenarios:**
1. Set `KLYNTBOT_CHANNELS__TELEGRAM__TOKEN=bot123` → config.channels.telegram.token == "bot123"
2. Set `KLYNTBOT_PROVIDERS__ZHIPU__API_KEY=key` → config.providers.zhipu.api_key == "key"
3. Env var + config file → env var wins (override behavior)
4. Env var not set → config file value used

**Edge Cases:**
- Empty env var value → should it clear the field or be ignored?
- Boolean env vars (e.g., KLYNTBOT_CHANNELS__TELEGRAM__ENABLED=true) → parse correctly

---

## Phase 5: P1 Major — Email Channel

### GAP-3.9: Email Channel Config Fields Not Used at Runtime

**Status: PARTIAL**

**Evidence:** Config schema (`schema.rs:266-383`) has all fields: `consent_granted`, `auto_reply_enabled`, `max_body_chars`, `mark_seen`, `imap_mailbox`, `imap_use_ssl`. However, `email.rs` does not reference most of these:
- Body truncation is hardcoded to 4000 chars (`email.rs:236`) instead of `config.max_body_chars`
- INBOX is hardcoded in `session.select("INBOX")` (`email.rs:111`) instead of `config.imap_mailbox`
- `consent_granted` is not checked before starting the channel
- `auto_reply_enabled` is not checked before sending replies
- `mark_seen` is not checked before marking emails as seen
- `imap_use_ssl` is not checked (always uses TLS)

**Acceptance Criteria:**
- [ ] AC-3.9.1: `start()` checks `config.consent_granted` — if false, log warning and return without starting polling
- [ ] AC-3.9.2: Body truncation uses `config.max_body_chars` instead of hardcoded 4000
- [ ] AC-3.9.3: IMAP mailbox selection uses `config.imap_mailbox` instead of hardcoded "INBOX"
- [ ] AC-3.9.4: Email marking uses `config.mark_seen` — if false, do not add `\Seen` flag
- [ ] AC-3.9.5: Reply sending checks `config.auto_reply_enabled` — if false, do not send outbound emails
- [ ] AC-3.9.6: TLS connection respects `config.imap_use_ssl` — if false, use plain TCP (or STARTTLS)

**Test Scenarios:**
1. `consent_granted: false` → channel logs "Email channel disabled: consent not granted" and does not poll
2. `max_body_chars: 2000` → email body truncated at 2000 chars
3. `imap_mailbox: "Archive"` → IMAP SELECT uses "Archive" folder
4. `mark_seen: false` → emails fetched but \Seen flag NOT applied
5. `auto_reply_enabled: false` → outbound send() returns Ok(()) without actually sending

**Edge Cases:**
- `max_body_chars: 0` → should mean unlimited or use a sensible minimum
- `imap_mailbox` is empty string → fall back to "INBOX"
- `imap_use_ssl: false` on port 993 → may fail, should warn

---

## Phase 6: P2 Minor — Remaining Gaps

### GAP-4.1: Web Tools Config Not Used

**Status: OPEN**

**Evidence:** `schema.rs:464-469` has `WebToolsConfig` with `brave_api_key`. Need to verify that WebSearchTool reads this from config at runtime.

**Acceptance Criteria:**
- [ ] AC-4.1.1: WebSearchTool reads `brave_api_key` from config (not hardcoded or env-only)
- [ ] AC-4.1.2: If `brave_api_key` is empty, WebSearchTool returns descriptive error on execution
- [ ] AC-4.1.3: WebFetchTool respects `restrict_to_workspace` if applicable (URL allowlisting)

**Test Scenarios:**
1. Config has brave_api_key → web search works
2. Config has empty brave_api_key → tool returns "Brave API key not configured" error
3. Env var `KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY` overrides config value

---

### GAP-4.2: Shell/Exec Tool Config

**Status: OPEN**

**Evidence:** `schema.rs:474-489` has `ExecToolConfig` with `timeout` and `allowed_commands`. Need to verify ExecTool honors these at runtime.

**Acceptance Criteria:**
- [ ] AC-4.2.1: ExecTool kills processes after `config.tools.exec.timeout` seconds
- [ ] AC-4.2.2: If `allowed_commands` is non-empty, ExecTool only permits listed commands
- [ ] AC-4.2.3: If `restrict_to_workspace` is true, ExecTool cwd is limited to workspace directory
- [ ] AC-4.2.4: ExecTool timeout defaults to 60 seconds when not configured

**Test Scenarios:**
1. Command runs longer than timeout → killed, error returned with "timeout" message
2. `allowed_commands: ["ls", "cat"]` → running "rm" returns "command not allowed"
3. `restrict_to_workspace: true` → exec cwd forced to workspace path

**Edge Cases:**
- Command spawns child processes → all children killed on timeout (process group kill)
- Empty allowed_commands list → all commands allowed (permissive default)
- Timeout = 0 → should mean "no timeout" or be rejected as invalid

---

### GAP-4.3: Session LRU Eviction

**Status: OPEN**

**Evidence:** No LRU eviction logic found in session manager. Python nanobot limits in-memory sessions.

**Acceptance Criteria:**
- [ ] AC-4.3.1: SessionManager has a configurable max in-memory sessions limit (default: 100)
- [ ] AC-4.3.2: When limit reached, least-recently-used session is evicted from memory
- [ ] AC-4.3.3: Evicted sessions remain on disk and can be reloaded on next access
- [ ] AC-4.3.4: Accessing a session updates its LRU timestamp

**Test Scenarios:**
1. Create 101 sessions with limit 100 → first session evicted from memory
2. Access evicted session → reloaded from disk transparently
3. Rapid access to same session → no eviction, LRU timestamp updated

**Edge Cases:**
- Session being processed when eviction triggered → should not evict active sessions
- Disk read fails on reload → create fresh session, log warning

---

### GAP-4.4: Skills Availability in Tool Summary

**Status: OPEN**

**Evidence:** Skills exist in `src/skills/` directory (tmux, github, skill-creator) but it's unclear if these are surfaced in the system prompt or tool descriptions sent to the LLM.

**Acceptance Criteria:**
- [ ] AC-4.4.1: Available skills are listed in the system prompt or tool registry
- [ ] AC-4.4.2: LLM can discover and invoke skills through the tool calling mechanism
- [ ] AC-4.4.3: Skill availability is dynamic — only loaded skills appear in the prompt

**Test Scenarios:**
1. Skills directory has tmux skill → LLM sees tmux capability in context
2. No skills configured → no skills section in system prompt
3. New skill added to directory → appears on next session start

---

## Consolidated Gap Summary

### Already Resolved (No Action Needed)

| ID | Description | Evidence |
|----|-------------|----------|
| GAP-2.1 | Tool context wiring | `agent_loop.rs:370-385` |
| GAP-2.2 | Discord IDENTIFY | `discord.rs:136-159` |
| GAP-2.3 | Subagent tool access | `subagent.rs:142-180` |
| GAP-2.4 | SpawnTool/CronTool None | `agent_loop.rs:120-127` |
| GAP-3.1a | Telegram typing | `telegram.rs:430-465` |
| GAP-3.1b | Telegram proxy | `telegram.rs:38-49` |
| GAP-3.1c | Telegram HTML fallback | `telegram.rs:694-714` |
| GAP-3.2a | Discord attachments | `discord.rs:298-371` |
| GAP-3.2b | Discord typing | `discord.rs:225-254` |
| GAP-3.2c | Discord shared WS | `discord.rs:62` |
| GAP-3.4 | CLI REPL with rustyline | `main.rs:77-240` |
| GAP-3.5 | Heartbeat callback | `main.rs:335-362` |

### Open Gaps Requiring Implementation

| ID | Description | Priority | Complexity | Task Ref |
|----|-------------|----------|------------|----------|
| GAP-3.1d | Telegram /reset session clear | P1 | Low | #6 |
| GAP-3.2d | Discord gateway_url/intents from config | P1 | Low | #5 |
| GAP-3.3a | Missing channel configs (feishu/dingtalk/mochat) | P1 | Medium | #7 |
| GAP-3.3b | Missing provider configs (5 providers) | P1 | Low | #7 |
| GAP-3.3c | ProviderConfig.extra_headers | P1 | Low | #7 |
| GAP-3.3d | Missing GatewayConfig | P1 | Low | #7 |
| GAP-3.6 | process_system_message LLM processing | P1 | High | #3 |
| GAP-3.7 | Config loader nanobot fallback | P1 | Medium | #7 |
| GAP-3.8 | Config loader env var expansion | P1 | Medium | #7 |
| GAP-3.9 | Email config fields not used at runtime | P1 | Medium | #10 |
| GAP-4.1 | Web tools config not used | P2 | Low | #11 |
| GAP-4.2 | Exec tool config not used | P2 | Low | #11 |
| GAP-4.3 | Session LRU eviction | P2 | Medium | #12 |
| GAP-4.4 | Skills in tool summary | P2 | Medium | #12 |

### Implementation Priority Order

1. **GAP-3.2d** — Discord config usage (Low complexity, config already exists)
2. **GAP-3.1d** — Telegram /reset (Low complexity, clear pattern from Python)
3. **GAP-3.9** — Email config runtime usage (Medium, straightforward substitutions)
4. **GAP-3.3a-d** — Config schema additions (Medium, mostly boilerplate)
5. **GAP-3.7** — Config migration (Medium, file system operations)
6. **GAP-3.8** — Env var expansion (Medium, systematic)
7. **GAP-3.6** — System message LLM processing (High, core agent logic)
8. **GAP-4.1-4.2** — Tool config usage (Low, verification + fixes)
9. **GAP-4.3** — Session LRU (Medium, new data structure)
10. **GAP-4.4** — Skills availability (Medium, system prompt design)

---

## Cross-Cutting Concerns

### CC-1: Error Handling Consistency

All gap fixes must follow the existing error pattern:
- Use `crate::error::Result<T>` return type
- Map errors to domain-specific variants (ConfigError, ChannelError, etc.)
- Log errors with `tracing::{error, warn, info, debug}`
- Never panic in production paths

### CC-2: Backward Compatibility

All config schema changes must:
- Use `#[serde(default)]` on new fields
- Existing config files must deserialize without error
- New fields must have sensible defaults matching Python nanobot behavior

### CC-3: Test Coverage

Each gap fix must include:
- Unit test for the specific behavior
- Integration test if it involves cross-module interaction
- Edge case tests for the scenarios listed above

### CC-4: Concurrency Safety

All runtime changes must:
- Use `Arc<RwLock<_>>` for shared mutable state (following existing patterns)
- Not introduce deadlocks (acquire locks in consistent order)
- Handle the case where channels/sessions are accessed concurrently
