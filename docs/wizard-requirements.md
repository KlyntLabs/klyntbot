# Enhanced Wizard Requirements Document

> **Author**: Business Analyst
> **Date**: 2026-02-13
> **Version**: 1.0
> **Status**: Draft for Review

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [User Personas](#3-user-personas)
4. [User Journey Maps](#4-user-journey-maps)
5. [Functional Requirements](#5-functional-requirements)
6. [MoSCoW Prioritization](#6-moscow-prioritization)
7. [Acceptance Criteria](#7-acceptance-criteria)
8. [Technical Constraints & Dependencies](#8-technical-constraints--dependencies)

---

## 1. Executive Summary

The current `klyntbot init` wizard is a minimal 4-step onboarding flow that configures a single LLM provider and creates workspace files. It leaves channel setup, tools configuration, skills discovery, daemon installation, and validation entirely to manual post-setup work. This document defines requirements for an enhanced wizard that transforms onboarding from a 2-minute skeleton into a comprehensive, guided, end-to-end setup experience.

---

## 2. Current State Analysis

### 2.1 Current Wizard Flow (`crates/cli/src/wizard.rs`)

| Step | What it does | What it lacks |
|------|-------------|---------------|
| **Step 1: Provider Selection** | Lists 5 providers (Anthropic, OpenAI, DeepSeek, Gemini, OpenRouter) with numeric selection | No support for 7 additional providers in config (Groq, vLLM, Zhipu, Dashscope, Moonshot, Minimax, AiHubMix). No custom API base URL entry. No multi-provider setup. |
| **Step 2: API Configuration** | Prompts for API key + model name | No API key validation (only checks `len < 10`). No test API call. Says "validated successfully" without actually validating. No support for `extra_headers` or `api_base` overrides. |
| **Step 3: Channels (Optional)** | Yes/no prompt; if yes, just prints `klyntbot channels login <channel>` | No actual channel configuration. No token entry. No OAuth flow. No channel testing. Doesn't even list available channels. |
| **Step 4: Workspace Setup** | Creates `~/.klyntbot/workspace/` directories and 5 template `.md` files + memory dir | Creates template files that are never actually used for guided personalization. Config dirs (sessions, cron, media, history) created but no explanation of what they're for. |

### 2.2 Specific Limitations

1. **No API validation**: The wizard claims "Configuration validated successfully" (`wizard.rs:247`) but performs zero validation beyond `key.len() < 10`.

2. **No channel configuration**: Step 3 is a dead end — answering "yes" only shows a CLI hint. Users must manually edit `config.json` or use `klyntbot channels login <channel>` (which itself only prints instructions, not interactive prompts per `channels.rs:59-154`).

3. **No tools configuration**: The `ToolsConfig` schema supports `restrict_to_workspace`, `exec.timeout`, `exec.allowed_commands`, and `web.brave_api_key` — none are configured by the wizard.

4. **No skills discovery**: 6 built-in skills exist (`summarize`, `skill-creator`, `github`, `tmux`, `weather`, `cron`) but the wizard doesn't mention them.

5. **No daemon/service setup**: `klyntbot serve` is the gateway daemon, but the wizard provides no guidance on running it as a background service (systemd, launchd, Docker).

6. **Missing providers**: Config schema supports 12 providers (`schema.rs:600-639`), but the wizard only lists 5 (`wizard.rs:18-49`).

7. **No re-run safety**: Running `klyntbot init` again overwrites the config. Template files check for existence, but the config save at line 103 blindly overwrites.

8. **No progress persistence**: If the user quits mid-wizard, all progress is lost.

9. **No existing config detection**: The wizard doesn't check for or migrate existing configuration.

---

## 3. User Personas

### 3.1 Persona A: CLI-Only Developer ("Alex")

- **Profile**: Software developer who wants a local AI assistant via terminal
- **Goals**: Quick setup with one provider, use `klyntbot chat` immediately
- **Technical level**: High — comfortable with CLI, API keys, config files
- **Channels needed**: None (CLI only via `klyntbot chat`)
- **Pain points with current wizard**: Step 3 channel prompt is confusing noise; no way to verify the API key actually works before finishing
- **Success metric**: Running `klyntbot chat "Hello"` successfully within 3 minutes of starting `init`

### 3.2 Persona B: Multi-Channel Power User ("Blake")

- **Profile**: Team lead who wants klyntbot accessible across Telegram, Discord, and Slack
- **Goals**: Full setup with multiple channels, allowlists, and scheduled tasks
- **Technical level**: Medium-high — can handle bot tokens but needs guidance for OAuth flows
- **Channels needed**: Telegram + Discord + Slack (3 channels with different auth models)
- **Pain points with current wizard**: Channels are completely unconfigured; must manually edit JSON or use the text-only `channels login` command for each
- **Success metric**: All three channels configured, tested, and running via `klyntbot serve`

### 3.3 Persona C: Enterprise/Self-Hosted User ("Casey")

- **Profile**: DevOps engineer deploying klyntbot on a company server
- **Goals**: Custom API base URLs, restricted tools, systemd service, multiple providers for failover
- **Technical level**: High — needs custom endpoints, security lockdown
- **Channels needed**: Slack + Email (enterprise channels)
- **Pain points with current wizard**: No `api_base` configuration, no tools restriction setup, no service installation, no multi-provider configuration
- **Success metric**: Hardened config with workspace restrictions, custom endpoints, and daemon running as a system service

---

## 4. User Journey Maps

### 4.1 Journey: Alex (CLI-Only, Fast Path)

```
[Welcome] → [Provider Selection] → [API Key + Quick Test] → [Workspace Setup] → [Verify: klyntbot chat "ping"] → [Done]
                                                                                         ↓
                                                                                  "Would you like to set up channels later? (y/N)"
                                                                                         ↓ N
                                                                                     [Summary + Next Steps]
```

**Key decisions**: Provider choice, model choice
**Duration target**: < 3 minutes
**Steps**: 4 (same as current, but with real validation)

### 4.2 Journey: Blake (Multi-Channel)

```
[Welcome] → [Provider Selection] → [API Key + Test] → [Channel Selection (multi-select)]
                                                                    ↓
                                              ┌─────────────┬───────┴───────┬──────────────┐
                                              ↓             ↓               ↓              ↓
                                         [Telegram]    [Discord]       [Slack]     [More channels...]
                                         Bot token     Bot token      Bot+App tokens
                                         AllowFrom     Invite URL     Scopes
                                         Test ping     Test ping      Test ping
                                              ↓             ↓               ↓
                                              └─────────────┴───────────────┘
                                                            ↓
                                                   [Tools Config (optional)]
                                                            ↓
                                                   [Daemon Setup (optional)]
                                                            ↓
                                                   [Skills Discovery]
                                                            ↓
                                                   [Summary + Test All]
```

**Key decisions**: Which channels, allowlist entries, whether to run as daemon
**Duration target**: < 10 minutes
**Steps**: 6-8 (depending on channel count)

### 4.3 Journey: Casey (Enterprise)

```
[Welcome] → [Detect existing config?] → [Provider(s) Selection (multi-provider)]
                                                    ↓
                                         [API Keys + Custom Base URLs + Extra Headers]
                                                    ↓
                                         [Test all providers]
                                                    ↓
                                         [Channel Selection: Slack + Email]
                                                    ↓
                                         [Slack: Bot Token + App Token + Scopes]
                                         [Email: IMAP + SMTP full config]
                                                    ↓
                                         [Tools Configuration]
                                         - restrict_to_workspace = true
                                         - exec.allowed_commands whitelist
                                         - exec.timeout
                                         - web.brave_api_key
                                                    ↓
                                         [Daemon/Service Setup]
                                         - systemd unit file generation
                                         - launchd plist generation
                                         - Docker compose snippet
                                                    ↓
                                         [Environment variable overrides cheatsheet]
                                                    ↓
                                         [Full validation + summary]
```

**Key decisions**: Multi-provider, custom endpoints, security restrictions, service type
**Duration target**: < 15 minutes
**Steps**: 8-10

---

## 5. Functional Requirements

### 5.1 FR-01: Existing Config Detection & Migration

**Description**: On startup, the wizard must detect existing configuration and offer to enhance/modify it rather than starting from scratch.

**Details**:
- Check `~/.klyntbot/config.json` exists
- If exists: show what's currently configured (provider, channels, tools) and ask: "Modify existing config" / "Start fresh" / "Cancel"
- If "Modify existing config": pre-fill all wizard prompts with existing values
- Preserve any manual edits to config that the wizard doesn't manage (e.g., `extra_headers`)

### 5.2 FR-02: Enhanced Provider Selection

**Description**: Support all 12 providers defined in `ProvidersConfig`, with optional multi-provider setup.

**Details**:
- Display all 12 providers: Anthropic, OpenAI, DeepSeek, Gemini, OpenRouter, Groq, vLLM, Zhipu, Dashscope, Moonshot, Minimax, AiHubMix
- Group into tiers: "Major" (top 5 as current) and "Additional" (show on request)
- After primary provider setup: "Would you like to configure additional providers? (y/N)"
- For each provider: API key + optional `api_base` URL + optional `extra_headers`

### 5.3 FR-03: API Key Validation

**Description**: Perform real API validation by making a lightweight test call after key entry.

**Details**:
- After API key entry, make a minimal API call (e.g., list models or a tiny completion)
- Show spinner during validation: "Validating API key..."
- On success: show model name, provider name, confirm access
- On failure: show error, offer retry, allow skipping validation (with warning)
- Timeout: 10 seconds per provider test
- Must work offline for self-hosted providers (vLLM) — skip test if custom `api_base` is localhost

### 5.4 FR-04: Interactive Channel Configuration

**Description**: Replace the current dead-end Step 3 with real, per-channel interactive setup.

**Details**:

**Channel Selection UI**:
- Multi-select prompt: "Which channels do you want to configure?"
- Show all 9 channels with brief descriptions: Telegram, Discord, WhatsApp, Slack, Email, QQ, Feishu, DingTalk, Mochat
- Show prerequisite info before each channel (e.g., "You'll need a Telegram bot token from @BotFather")

**Per-channel configuration** (details below for primary channels):

#### Telegram (`TelegramConfig`)
- Prompt: Bot token (with link to @BotFather)
- Prompt: Allowed users/groups (comma-separated, or empty for all)
- Optional: Proxy URL (socks5/http)
- Test: `getMe` API call to validate token → display bot username

#### Discord (`DiscordConfig`)
- Prompt: Bot token (with link to Discord Developer Portal)
- Prompt: Allowed servers/users (comma-separated)
- Auto: Generate OAuth2 invite URL with correct permissions (Read Messages, Send Messages, Read History)
- Display: Invite URL for user to open in browser
- Optional: Custom gateway URL (for self-hosted)
- Test: Validate token with Discord API

#### Slack (`SlackConfig`)
- Prompt: Bot Token (xoxb-...)
- Prompt: App Token (xapp-...)
- Prompt: Mode selection (socket / events — default socket)
- Prompt: Allowed channels/users
- Optional: Group policy, DM config
- Test: Validate tokens with `auth.test` API

#### WhatsApp (`WhatsAppConfig`)
- Prompt: Bridge URL (default: ws://localhost:3001)
- Prompt: Allowed contacts
- Note: Explain bridge requirement, link to bridge setup docs
- Test: Attempt WebSocket connection to bridge URL

#### Email (`EmailConfig`)
- Prompt: IMAP host, port (default 993), username, password, use SSL
- Prompt: SMTP host, port (default 587), username, password, use TLS
- Prompt: From address
- Prompt: Allowed senders
- Optional: Auto-reply enabled, poll interval, subject prefix
- Test: IMAP login + SMTP connection test

#### QQ (`QQConfig`)
- Prompt: App ID
- Prompt: Secret
- Prompt: Allowed groups
- Test: Validate credentials with QQ API

#### Feishu (`FeishuConfig`)
- Prompt: App ID, App Secret
- Optional: Encrypt key, verification token
- Prompt: Allowed users
- Test: Validate with Feishu API

#### DingTalk (`DingTalkConfig`)
- Prompt: Client ID, Client Secret
- Prompt: Allowed users
- Test: Validate with DingTalk API

#### Mochat (`MochatConfig`)
- Prompt: Claw token
- Prompt: Agent user ID
- Optional: Base URL, socket URL
- Prompt: Sessions, panels (comma-separated)
- Test: Validate with Mochat API

### 5.5 FR-05: Tools Configuration

**Description**: Configure tool restrictions and API keys for web tools.

**Details**:
- Prompt: "Restrict tools to workspace directory?" (y/N) → sets `tools.restrict_to_workspace`
- Prompt: "Set command execution timeout?" (default: 60s) → sets `tools.exec.timeout`
- Prompt: "Restrict allowed shell commands?" → if yes, enter comma-separated allowlist → sets `tools.exec.allowed_commands`
- Prompt: "Configure web search? (requires Brave API key)" → sets `tools.web.brave_api_key`
- Prompt: "Max web search results?" (default: 5) → sets `tools.web.max_results`

### 5.6 FR-06: Skills Discovery

**Description**: Show available built-in skills and their readiness status.

**Details**:
- Auto-discover skills in the workspace `skills/` directory
- For each skill: show name, description, availability, required binaries/env vars
- Highlight skills that are NOT available due to missing dependencies (e.g., `tmux` skill needs `tmux` binary)
- Offer to install missing dependencies (where possible, e.g., `brew install tmux`)
- Show skills that require additional env vars (e.g., `github` skill may need `GITHUB_TOKEN`)

### 5.7 FR-07: Daemon/Service Setup

**Description**: Offer to install klyntbot as a background service.

**Details**:
- Detect OS: macOS (launchd), Linux (systemd), other (Docker/manual)
- **macOS**: Generate `~/Library/LaunchAgents/com.klyntbot.agent.plist`, offer to load it
- **Linux**: Generate `~/.config/systemd/user/klyntbot.service`, offer to enable it
- **Docker**: Generate `docker-compose.yml` snippet and `Dockerfile`
- **Manual**: Show `klyntbot serve` command with recommended flags
- Show port selection (default: 18790 from `GatewayConfig`)
- Explain: daemon is needed for channels to work; CLI-only users don't need it

### 5.8 FR-08: Validation & Testing Pipeline

**Description**: At each configuration step, validate inputs and run connectivity tests.

**Details**:
- **Provider validation**: Real API call (FR-03)
- **Channel validation**: Per-channel connection test (FR-04)
- **Tools validation**: Verify workspace directory exists and is writable
- **Final validation**: Run `klyntbot status --verbose` equivalent and display results
- **Quick smoke test**: Send a test message through the agent loop to verify end-to-end

### 5.9 FR-09: Progress Persistence & Resume

**Description**: Save wizard progress so users can resume if interrupted.

**Details**:
- After each major step, save partial config to `~/.klyntbot/.wizard-progress.json`
- On next `klyntbot init`, if progress file exists: "Resume previous setup? (Y/n)"
- Clean up progress file on successful completion
- Progress file stores: current step, all entered values, completed channels list

### 5.10 FR-10: Summary & Next Steps

**Description**: Show a comprehensive summary of what was configured and actionable next steps.

**Details**:
- Show table of: Provider, Model, Channels (enabled/configured), Tools restrictions, Skills available
- Show next steps based on what was configured:
  - CLI-only: "Try: `klyntbot chat \"Hello!\"`"
  - Channels configured: "Start the daemon: `klyntbot serve`"
  - Channels need more setup: "Complete channel setup: `klyntbot channels login <name>`"
  - Skills missing deps: "Install missing dependencies for full skill support"
- Show config file path
- Show environment variable override examples

### 5.11 FR-11: Workspace Template Personalization

**Description**: Make the workspace template files interactive rather than static.

**Details**:
- **IDENTITY.md**: Ask for bot name (default: "klyntbot"), version, description
- **USER.md**: Ask for user name, role, preferred communication style, technical level
- **SOUL.md**: Offer personality presets (Professional, Casual, Technical, Friendly) or custom
- **AGENTS.md** and **TOOLS.md**: Pre-fill based on actual configured capabilities
- Skip files that already exist (current behavior — preserve this)

### 5.12 FR-12: Gateway Configuration

**Description**: Allow configuring the HTTP gateway settings.

**Details**:
- Prompt: Gateway host (default: 0.0.0.0)
- Prompt: Gateway port (default: 18790)
- Only show this step if channels or daemon setup is selected

---

## 6. MoSCoW Prioritization

### Must Have (P0) — Required for MVP

| ID | Feature | Rationale |
|----|---------|-----------|
| FR-01 | Existing config detection | Prevents data loss on re-run |
| FR-02 | Enhanced provider selection (all 12) | Config schema already supports them |
| FR-03 | API key validation (real test call) | Current "validated" message is misleading |
| FR-04 | Interactive channel config (Telegram, Discord, Slack) | These are the 3 most popular channels; current wizard skips them entirely |
| FR-08 | Validation & testing at each step | Core quality requirement |
| FR-10 | Summary & next steps | Users need to know what to do after wizard |

### Should Have (P1) — Important but Not Blocking

| ID | Feature | Rationale |
|----|---------|-----------|
| FR-04 | Interactive channel config (Email, WhatsApp, QQ) | Secondary channels |
| FR-05 | Tools configuration | Security-conscious users need this |
| FR-06 | Skills discovery | Improves discoverability |
| FR-07 | Daemon/service setup | Needed for channels but users can run `klyntbot serve` manually |
| FR-11 | Workspace template personalization | Improves user experience |

### Could Have (P2) — Nice to Have

| ID | Feature | Rationale |
|----|---------|-----------|
| FR-04 | Interactive channel config (Feishu, DingTalk, Mochat) | Niche channels |
| FR-09 | Progress persistence & resume | Helpful for complex setups |
| FR-12 | Gateway configuration | Most users use defaults |

### Won't Have (This Release)

| Feature | Rationale |
|---------|-----------|
| GUI/TUI wizard | Terminal UI library adds complexity; stick with stdin/stdout |
| OAuth browser flow automation | Too complex; provide instructions + test instead |
| Auto-detection of existing bot tokens from env | Security concern — don't auto-read env vars silently |
| Remote deployment wizard | Out of scope — deployment is a separate concern |

---

## 7. Acceptance Criteria

### AC-01: Existing Config Detection (FR-01)

- [ ] Running `klyntbot init` with existing `~/.klyntbot/config.json` shows "Existing configuration found"
- [ ] User can choose to modify, start fresh, or cancel
- [ ] "Modify" pre-fills all prompts with current values
- [ ] "Start fresh" backs up old config to `config.json.bak` before overwriting
- [ ] Manual config edits outside wizard scope are preserved

### AC-02: Enhanced Provider Selection (FR-02)

- [ ] All 12 providers from `ProvidersConfig` are selectable
- [ ] Providers display in two groups: "Recommended" (top 5) and "Additional" (remaining 7)
- [ ] User can type a number or provider name to select
- [ ] Default selection is Anthropic (index 1)
- [ ] Multi-provider setup available via follow-up prompt

### AC-03: API Key Validation (FR-03)

- [ ] After entering API key, a real HTTP request is made to the provider
- [ ] Spinner displays during validation (using `common::utils::terminal::Spinner`)
- [ ] Success shows provider name and confirms access
- [ ] Failure shows clear error message and allows retry
- [ ] User can skip validation with explicit warning
- [ ] Validation timeout is 10 seconds
- [ ] Localhost/custom `api_base` URLs skip validation gracefully

### AC-04: Channel Configuration (FR-04)

- [ ] Multi-select prompt allows choosing 0+ channels
- [ ] Each selected channel has a guided sub-wizard
- [ ] Telegram: prompts for token, allowlist, optional proxy; tests with `getMe`
- [ ] Discord: prompts for token, allowlist; generates invite URL; tests token
- [ ] Slack: prompts for bot + app tokens, mode; tests with `auth.test`
- [ ] Email: prompts for full IMAP/SMTP config; tests both connections
- [ ] WhatsApp: prompts for bridge URL; tests WebSocket connection
- [ ] QQ: prompts for app ID + secret; tests credentials
- [ ] Each channel test shows pass/fail with clear error messages
- [ ] Failed channel tests don't block wizard completion (warn and continue)
- [ ] Channel configs are written to `config.channels.<name>` with `enabled: true`

### AC-05: Tools Configuration (FR-05)

- [ ] Workspace restriction toggle sets `tools.restrict_to_workspace`
- [ ] Exec timeout is configurable with validation (1-600 seconds)
- [ ] Command allowlist accepts comma-separated list
- [ ] Brave API key prompt with optional validation
- [ ] Max results configurable (1-20)

### AC-06: Skills Discovery (FR-06)

- [ ] Lists all skills found in workspace `skills/` directory
- [ ] Shows availability status (available/unavailable) per skill
- [ ] Missing binary requirements highlighted (e.g., `tmux` not found)
- [ ] Missing environment variable requirements highlighted
- [ ] Purely informational — no config changes needed

### AC-07: Daemon Setup (FR-07)

- [ ] OS detection works on macOS and Linux
- [ ] macOS: generates valid launchd plist, offers `launchctl load`
- [ ] Linux: generates valid systemd user unit, offers `systemctl --user enable`
- [ ] Generates correct `klyntbot serve --port <port>` command
- [ ] Explains that daemon is required for channel integrations

### AC-08: Validation Pipeline (FR-08)

- [ ] Provider validation runs after API key entry (FR-03)
- [ ] Channel validation runs after each channel config (FR-04)
- [ ] Final summary shows all validation results in a table
- [ ] At least one provider must pass validation for wizard to complete
- [ ] Channel validation failures are warnings, not errors

### AC-09: Summary & Next Steps (FR-10)

- [ ] Displays table: Provider(s), Model, Channels, Tools restrictions, Skills count
- [ ] Context-aware next steps (different for CLI-only vs. channel users)
- [ ] Shows config file path
- [ ] Shows relevant `klyntbot` commands for the user's setup

---

## 8. Technical Constraints & Dependencies

### 8.1 Existing Architecture Constraints

1. **Config schema is the source of truth**: All wizard outputs must map to fields in `Config` (`config::schema`). No new config fields should be invented by the wizard.

2. **camelCase JSON serde**: All config serialization uses `#[serde(rename_all = "camelCase")]`. The wizard must produce configs that pass `serde_json` round-trip.

3. **Minimal config saves**: The `config::save()` function diffs against defaults and only writes changed fields. The wizard must work with this behavior — don't assume all fields are present in the file.

4. **Secret wrapping**: API keys use `Secret<String>`. Wizard must wrap keys in `Secret::new()` before setting on config.

5. **Terminal utilities**: Use `common::utils::terminal::*` for all UI rendering (box drawing, spinners, colors, status indicators). Don't introduce new UI libraries.

6. **Async runtime**: The wizard runs in a tokio async context. Channel tests and API validation need `async` support.

### 8.2 New Dependencies Needed

- **None for P0**: All validation can use `reqwest` (already a dependency for channels/providers)
- **P1/P2**: Service file generation is pure string formatting, no new deps needed

### 8.3 File Impact Analysis

| File | Changes |
|------|---------|
| `crates/cli/src/wizard.rs` | Major rewrite — new step functions, validation, channel sub-wizards |
| `crates/config/src/schema.rs` | No changes needed — schema already supports everything |
| `crates/config/src/loader.rs` | Minor — may need `backup()` function for FR-01 |
| `crates/cli/src/channels.rs` | Extract reusable channel test logic for wizard use |
| `crates/providers/src/` | May need lightweight `validate_key()` functions per provider |
| `crates/common/src/utils/terminal.rs` | May need multi-select prompt utility |

### 8.4 Testing Strategy

- **Unit tests**: Each wizard step function should be testable with mocked stdin
- **Integration tests**: End-to-end wizard flow with mock provider (existing `tests/mock_provider.rs`)
- **Manual testing**: Interactive wizard requires real terminal testing for UX polish
- **Regression**: Ensure existing `cargo test --workspace` (~330 tests) passes after changes
