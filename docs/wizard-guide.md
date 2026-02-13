# Klyntbot Setup Wizard Guide

Complete guide to configuring klyntbot using the interactive setup wizard.

## Table of Contents

- [Quick Start](#quick-start)
- [User Profiles](#user-profiles)
- [Wizard Walkthrough](#wizard-walkthrough)
  - [Phase 1: Welcome and Overview](#phase-1-welcome-and-overview)
  - [Phase 2: Provider Setup](#phase-2-provider-setup)
  - [Phase 3: Channel Configuration](#phase-3-channel-configuration)
  - [Phase 4: Tools and Permissions](#phase-4-tools-and-permissions)
  - [Phase 5: Background Service](#phase-5-background-service)
  - [Phase 6: Validation and Testing](#phase-6-validation-and-testing)
  - [Phase 7: Summary and Next Steps](#phase-7-summary-and-next-steps)
- [Provider Setup Guides](#provider-setup-guides)
- [Channel Setup Guides](#channel-setup-guides)
  - [Telegram](#telegram)
  - [Discord](#discord)
  - [Slack](#slack)
  - [WhatsApp](#whatsapp)
  - [Email](#email)
  - [QQ](#qq)
  - [Feishu / DingTalk](#feishu--dingtalk)
- [Configuration Reference](#configuration-reference)
- [Environment Variables](#environment-variables)
- [Troubleshooting](#troubleshooting)
- [FAQ](#faq)

---

## Quick Start

```bash
klyntbot init
```

The wizard guides you through up to 7 phases. Only the first phase (provider setup) is required; everything else is optional and skippable:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Welcome    │────>│   Provider   │────>│   Channels   │
│   & Overview │     │    Setup     │     │   (optional) │
└──────────────┘     └──────────────┘     └──────────────┘
                                                │
       ┌────────────────────────────────────────┘
       v
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│    Tools &   │────>│   Daemon /   │────>│  Validation  │
│  Permissions │     │   Service    │     │  & Testing   │
│  (optional)  │     │  (optional)  │     │              │
└──────────────┘     └──────────────┘     └──────────────┘
                                                │
                                                v
                                         ┌──────────────┐
                                         │   Summary    │
                                         │ & Next Steps │
                                         └──────────────┘
```

**Minimal path** (CLI-only user): Provider setup and workspace creation. Takes about 2 minutes.

**Full path** (multi-channel power user): All 7 phases. Takes about 5-10 minutes.

After completion:

```bash
klyntbot chat "Hello!"
```

---

## User Profiles

The wizard adapts to three common user types:

### CLI-Only Developer

- Wants a local AI assistant via terminal
- Configures one provider, skips channels and daemon
- Goal: `klyntbot chat` working in under 3 minutes

### Multi-Channel Power User

- Connects klyntbot to Telegram, Discord, Slack, etc.
- Configures channels with allowlists and connection testing
- Sets up the background daemon to keep channels connected
- Goal: All channels running via `klyntbot serve` in under 10 minutes

### Enterprise / Self-Hosted User

- Uses custom API endpoints, multiple providers for failover
- Configures strict tool permissions and command allowlists
- Installs as a system service (systemd/launchd)
- Goal: Hardened, production-ready configuration in under 15 minutes

---

## Wizard Walkthrough

### Navigation

The wizard supports these keys at every prompt:

| Key | Action |
|-----|--------|
| `Enter` | Submit / confirm / accept default |
| `b` | Go back to previous step |
| `?` | Show contextual help |
| `q` | Quit (progress is saved) |
| `Ctrl+C` | Abort with save/discard prompt |

For list prompts, arrow keys (`Up`/`Down` or `j`/`k`) navigate, `Space` toggles selection in multi-select, and `Enter` confirms.

### Progress Bar

A progress indicator appears at the top of each phase:

```
  ● Provider  ─  ○ Channels  ─  ○ Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━                                               Step 2 of 7
```

Completed steps show `●` in green. The current step shows `●` in cyan. Future steps show `○` in gray.

---

### Phase 1: Welcome and Overview

```
  ┌──────────────────────────────────────────────────────────┐
  │                                                          │
  │   █▄▀ █   █ █▄ █ ▀█▀ █▄▄ █▀█ ▀█▀                       │
  │   █ █ █▄▄ ▄█ █ ▀█  █  █▄█ █▄█  █                        │
  │                                                          │
  │   Your AI assistant framework                    v0.1.0  │
  │                                                          │
  └──────────────────────────────────────────────────────────┘

  Welcome! This wizard will set up klyntbot in a few steps.

  What we'll configure:
    1. LLM Provider       Your AI backend (required)
    2. Chat Channels      Telegram, Discord, Slack...
    3. Tools & Perms      File access, web search, shell
    4. Background Agent   Run as a system service
    5. Validation         Test everything works

  Estimated time: 2-5 minutes

  Press Enter to start, or q to quit...
```

**Returning users**: If `~/.klyntbot/config.json` already exists, the wizard detects it and shows your current configuration, offering three options:

```
  ┌─ Existing Configuration Found ──────────────────────────┐
  │                                                          │
  │  Provider:  Anthropic (claude-sonnet-4-5)                │
  │  Channels:  Telegram ✓  Discord ✓                        │
  │                                                          │
  └──────────────────────────────────────────────────────────┘

  What would you like to do?

    ● Reconfigure everything       Start fresh
      Update specific settings     Choose what to change
      Validate & test              Check everything works
```

**Resume support**: If you quit the wizard mid-way, your progress is saved to a checkpoint file. Next time you run `klyntbot init`, you'll be asked whether to resume where you left off.

---

### Phase 2: Provider Setup

**Step 2a: Select Provider**

```
  ════════════════════════════════════════════════════════════
  Step 2 of 7: Choose Your LLM Provider
  ════════════════════════════════════════════════════════════

  Select your provider:

    ● Anthropic (Claude)       Recommended for best quality
      OpenAI (GPT)             Industry standard models
      DeepSeek                 Cost-effective alternative
      Google (Gemini)          Multimodal capabilities
      OpenRouter               Access to many models
      ─────────────────────────────────────────────
      Other / Custom           Bring your own endpoint

  ↑/↓ navigate  ·  Enter select  ·  ? help
```

The wizard supports all 12 providers in the config schema. The top 5 are shown by default; selecting "Other / Custom" reveals additional providers (Groq, vLLM, Zhipu, Dashscope, Moonshot, Minimax, AiHubMix) and allows custom API base URLs.

**Step 2b: API Key and Model**

```
  Provider: Anthropic (Claude)

  Get your API key at: https://console.anthropic.com

  API Key: ●●●●●●●●●●●●●●●●●●●●●●●●sk-ant...3kf
  ✓ Key format looks valid (sk-ant prefix detected)

  Model [claude-sonnet-4-5]: _

  Available models:
    claude-sonnet-4-5      Balanced speed & quality (recommended)
    claude-opus-4-5        Highest quality, slower
    claude-haiku-4-5       Fastest, most affordable
```

The wizard validates API key format per provider (e.g., Anthropic keys start with `sk-ant-`, Slack bot tokens with `xoxb-`). After entering the key, a real API test call verifies the key works.

**Step 2c: API Validation**

```
  ⣾ Testing Anthropic API connection...
  ✓ API connection successful (1.2s)
```

If validation fails, you can retry, enter a different key, or skip validation (with a warning).

**Step 2d: Confirmation**

```
  ┌─ Provider Configured ───────────────────────────────────┐
  │                                                          │
  │  Provider:  Anthropic                                    │
  │  Model:     claude-sonnet-4-5                            │
  │  API Key:   sk-ant-●●●●●●●●●●3kf                        │
  │                                                          │
  │  ✓ Saved to config                                       │
  └──────────────────────────────────────────────────────────┘
```

After configuring the primary provider, you're asked: "Would you like to configure additional providers?" This allows multi-provider setups for failover or model variety.

**Default models per provider**:

| Provider   | Default Model          |
|-----------|------------------------|
| Anthropic  | `claude-sonnet-4-5`    |
| OpenAI     | `gpt-4o`               |
| DeepSeek   | `deepseek-chat`        |
| Gemini     | `gemini-2.0-flash`     |
| OpenRouter | `openrouter/auto`      |

---

### Phase 3: Channel Configuration

```
  ════════════════════════════════════════════════════════════
  Step 3 of 7: Enable Chat Channels (Optional)
  ════════════════════════════════════════════════════════════

  Channels let klyntbot respond on messaging platforms.
  You can always add more later with: klyntbot channels login <name>

  Would you like to set up channels now? [y/N]: _
```

If yes, you'll see a multi-select list of all available channels:

```
  Select channels to enable:

    [ ] Telegram           Simple setup: just a bot token
    [ ] Discord            Bot token + gateway connection
    [ ] Slack              Bot + App tokens (Socket Mode)
    [ ] WhatsApp           Requires separate bridge server
    [ ] Email              IMAP/SMTP credentials
    [ ] QQ                 App ID + secret key
    [ ] Feishu/Lark        Enterprise messaging
    [ ] DingTalk           Enterprise messaging

  ↑/↓ navigate  ·  Space toggle  ·  Enter confirm  ·  ? help
```

Use `Space` to toggle channels on/off, then `Enter` to confirm. The wizard then walks through each selected channel with a guided sub-wizard (see [Channel Setup Guides](#channel-setup-guides) for per-channel details).

Each channel setup includes:
1. Step-by-step instructions for obtaining credentials
2. Token/credential input with format validation
3. Access control (allowlist) configuration
4. Connection test to verify everything works

After all channels are configured, a summary table shows the results:

```
  ┌──────────┬───────────┬──────────────────────────────────┐
  │ Channel  │ Status    │ Details                          │
  ├──────────┼───────────┼──────────────────────────────────┤
  │ Telegram │ ✓ Enabled │ 2 allowed users                  │
  │ Discord  │ ✓ Enabled │ All servers                      │
  │ Email    │ ✓ Enabled │ user@gmail.com                   │
  └──────────┴───────────┴──────────────────────────────────┘

  3 channels enabled. Press Enter to continue...
```

---

### Phase 4: Tools and Permissions

```
  ════════════════════════════════════════════════════════════
  Step 4 of 7: Tools & Permissions (Optional)
  ════════════════════════════════════════════════════════════

  klyntbot can use tools to interact with your system.
  Default settings are safe for most users.

  Would you like to customize tool permissions? [y/N]: _
```

If yes, you can configure:

- **File access restriction**: Limit file operations to the workspace directory only
- **Shell command timeout**: How long commands can run (default: 60 seconds)
- **Command allowlist**: Restrict which shell commands the agent can execute
- **Brave Search API key**: Enable web search capabilities
- **Max search results**: How many results to return (default: 5)

```
  ┌─ Tools Configured ──────────────────────────────────────┐
  │                                                          │
  │  File access:    Workspace only (~/.klyntbot/workspace)  │
  │  Shell:          60s timeout, all commands               │
  │  Web search:     ✓ Brave API configured                  │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

---

### Phase 5: Background Service

```
  ════════════════════════════════════════════════════════════
  Step 5 of 7: Background Agent (Optional)
  ════════════════════════════════════════════════════════════

  Run klyntbot as a background service so it stays connected
  to your chat channels even when the terminal is closed.

  Install as system service? [y/N]: _
```

The wizard detects your platform and offers the appropriate service type:

**macOS** (launchd):
```
    ✓ Created ~/Library/LaunchAgents/io.klyntbot.agent.plist
    ✓ Service registered with launchd
    ✓ Service configured to start on login
```

**Linux** (systemd):
```
    ✓ Created ~/.config/systemd/user/klyntbot.service
    ✓ Reloaded systemd daemon
    ✓ Enabled klyntbot.service
```

Services are installed at the user level (no sudo required). You can configure the gateway port (default: 18790) and bind address (default: 0.0.0.0).

---

### Phase 6: Validation and Testing

The wizard runs automated checks on everything you configured:

```
  ════════════════════════════════════════════════════════════
  Step 6 of 7: Validation & Testing
  ════════════════════════════════════════════════════════════

  Running validation checks...

    ✓ Config file valid                              0.1s
    ✓ Workspace directories exist                    0.0s
    ✓ Anthropic API connection                       1.2s
    ✓ Telegram bot authenticated                     0.8s
    ✓ Discord bot ready                              0.6s
    ✓ Email IMAP connection                          1.5s
    ✓ Email SMTP connection                          0.9s
    ○ Brave Search API (not configured)              -
    ✓ Service installed and running                  0.2s

  8 passed  ·  0 failed  ·  1 skipped
```

If any checks fail, the wizard shows detailed error information with suggestions:

```
  ✗ Anthropic API connection failed
    Error: 401 Unauthorized
    The API key may be incorrect or expired.
    Fix: Run klyntbot config set providers.anthropic.apiKey <key>
         or re-run klyntbot init
```

You can choose to fix issues now, continue anyway, or quit.

Optionally, you can send a live test message through the LLM and channels to verify end-to-end functionality.

---

### Phase 7: Summary and Next Steps

```
  ┌─ Setup Complete! ───────────────────────────────────────┐
  │                                                          │
  │  Your AI assistant is ready!                             │
  │                                                          │
  └──────────────────────────────────────────────────────────┘

  Configuration Summary
  ─────────────────────

  Provider     Anthropic (claude-sonnet-4-5)
  Channels     Telegram ✓  Discord ✓  Email ✓
  Tools        File I/O ✓  Shell ✓  Web Search ✓
  Service      launchd (auto-start) ✓
  Config       ~/.klyntbot/config.json
  Workspace    ~/.klyntbot/workspace

  Quick Start
  ───────────

  Start chatting:
    $ klyntbot chat "Hello!"

  Start REPL:
    $ klyntbot chat

  Start server (if not using daemon):
    $ klyntbot serve

  Manage channels:
    $ klyntbot channels list

  Check status:
    $ klyntbot status --verbose
```

---

## Provider Setup Guides

The wizard supports 12 LLM providers. Here are guides for each:

### Anthropic (Claude)

1. Go to [console.anthropic.com](https://console.anthropic.com)
2. Sign in or create an account
3. Navigate to **API Keys**
4. Click **Create Key**
5. Copy the key (starts with `sk-ant-`)

**Models**: `claude-opus-4-5`, `claude-sonnet-4-5`, `claude-haiku-4-5`

### OpenAI (GPT)

1. Go to [platform.openai.com/api-keys](https://platform.openai.com/api-keys)
2. Sign in or create an account
3. Click **Create new secret key**
4. Copy the key (starts with `sk-`)

**Models**: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `o1`, `o3-mini`

### DeepSeek

1. Go to [platform.deepseek.com](https://platform.deepseek.com)
2. Generate an API key

**Models**: `deepseek-chat`, `deepseek-reasoner`

### Google (Gemini)

1. Go to [makersuite.google.com/app/apikey](https://makersuite.google.com/app/apikey)
2. Sign in and click **Create API key**

**Models**: `gemini-2.0-flash`, `gemini-2.0-pro`, `gemini-1.5-pro`

### OpenRouter

1. Go to [openrouter.ai/keys](https://openrouter.ai/keys)
2. Create and copy the key (starts with `sk-or-`)

**Default model**: `openrouter/auto` (auto-selects per request)

### Additional Providers

The wizard also supports: **Groq**, **vLLM**, **Zhipu**, **Dashscope**, **Moonshot**, **Minimax**, and **AiHubMix**. Select "Other / Custom" in the provider list to access them.

### Custom API Endpoints

For self-hosted or proxied providers, the wizard lets you set a custom API base URL and extra headers:

```json
{
  "providers": {
    "openai": {
      "apiKey": "sk-...",
      "apiBase": "https://your-custom-endpoint.com/v1",
      "extraHeaders": { "X-Custom-Header": "value" }
    }
  }
}
```

---

## Channel Setup Guides

### Telegram

The wizard guides you through BotFather setup:

1. Open Telegram and message [@BotFather](https://t.me/botfather)
2. Send `/newbot` and follow the prompts
3. Copy the bot token (format: `123456:ABC-DEF...`)
4. Paste in the wizard

The wizard validates the token format and optionally tests it with a `getMe` API call.

**Access control**: Choose "Anyone" or enter specific Telegram user IDs (comma-separated).

| Field       | Description                          | Example                        |
|------------|--------------------------------------|--------------------------------|
| `token`     | Bot API token from BotFather         | `123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11` |
| `allowFrom` | Allowed user/group IDs (empty = all) | `["12345678", "-100987654"]`   |
| `proxy`     | Optional SOCKS5 proxy                | `socks5://localhost:1080`      |

### Discord

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications)
2. Click **New Application**, give it a name
3. Navigate to **Bot** > **Reset Token**, copy it
4. Enable **MESSAGE CONTENT** intent under Privileged Gateway Intents
5. Under **OAuth2 > URL Generator**, select `bot` scope and appropriate permissions
6. Use the generated URL to invite the bot to your server
7. Paste the bot token in the wizard

The wizard validates the token and can generate the invite URL for you.

| Field        | Description                           | Default |
|-------------|---------------------------------------|---------|
| `token`      | Bot token from Developer Portal       | -       |
| `allowFrom`  | Allowed guild/channel IDs             | `[]`    |
| `gatewayUrl` | Discord Gateway WebSocket URL         | `wss://gateway.discord.gg/?v=10&encoding=json` |
| `intents`    | Gateway intents bitmask               | `37377` |

### Slack

The wizard provides a ready-to-use Slack app manifest:

```yaml
display_information:
  name: klyntbot
features:
  bot_user:
    display_name: klyntbot
oauth_config:
  scopes:
    bot:
      - chat:write
      - app_mentions:read
settings:
  socket_mode_enabled: true
```

1. Go to [api.slack.com/apps](https://api.slack.com/apps) > **Create New App** > **From Manifest**
2. Paste the manifest
3. Install to your workspace
4. Copy the **Bot User OAuth Token** (starts with `xoxb-`)
5. Enable **Socket Mode** and generate an **App-Level Token** (starts with `xapp-`)
6. Paste both tokens in the wizard

| Field           | Description                        | Default    |
|----------------|------------------------------------|------------|
| `botToken`      | Bot User OAuth Token (`xoxb-...`)  | -          |
| `appToken`      | App-Level Token (`xapp-...`)       | -          |
| `allowFrom`     | Allowed channel/user IDs           | `[]`       |
| `mode`          | Connection mode                    | `socket`   |
| `groupPolicy`   | How to handle group messages       | `none`     |
| `dm.enabled`    | Enable direct messages             | `false`    |

### WhatsApp

WhatsApp uses a bridge service for integration.

1. Set up a WhatsApp bridge server (e.g., whatsapp-web.js bridge)
2. Enter the bridge WebSocket URL in the wizard (default: `ws://localhost:3001`)
3. Configure allowed contacts

| Field       | Description                    | Default                 |
|------------|--------------------------------|-------------------------|
| `bridgeUrl` | WebSocket URL of the bridge    | `ws://localhost:3001`   |
| `allowFrom` | Allowed phone numbers/group IDs| `[]`                    |

### Email

The wizard supports quick setup for common providers:

**Gmail quick setup**:
1. Enable 2-factor authentication on your Google account
2. Go to [myaccount.google.com/apppasswords](https://myaccount.google.com/apppasswords)
3. Generate an app password for "Mail"
4. Enter your Gmail address and app password in the wizard

The wizard auto-detects IMAP/SMTP settings for Gmail, Outlook, and other major providers.

**Full IMAP/SMTP configuration**:

| Field                | Description                     | Default   |
|---------------------|---------------------------------|-----------|
| `imapHost`           | IMAP server hostname            | -         |
| `imapPort`           | IMAP server port                | `993`     |
| `imapUsername`        | IMAP login username             | -         |
| `imapPassword`        | IMAP login password             | -         |
| `imapMailbox`         | Mailbox to monitor              | `INBOX`   |
| `imapUseSsl`          | Use SSL for IMAP                | `true`    |
| `smtpHost`           | SMTP server hostname            | -         |
| `smtpPort`           | SMTP server port                | `587`     |
| `smtpUsername`        | SMTP login username             | -         |
| `smtpPassword`        | SMTP login password             | -         |
| `smtpUseTls`          | Use TLS for SMTP                | `true`    |
| `fromAddress`         | Sender email address            | -         |
| `allowFrom`           | Allowed sender addresses        | `[]`      |
| `consentGranted`      | User consent for email access   | `false`   |
| `autoReplyEnabled`    | Enable automatic replies        | `true`    |
| `maxBodyChars`        | Max email body length           | `12000`   |
| `markSeen`            | Mark processed emails as seen   | `true`    |
| `pollIntervalSeconds` | Check interval in seconds       | `30`      |
| `subjectPrefix`       | Reply subject prefix            | `Re: `    |

### QQ

1. Create a QQ Bot application
2. Enter your App ID and Secret in the wizard

| Field       | Description       |
|------------|-------------------|
| `appId`     | QQ Bot App ID     |
| `secret`    | QQ Bot Secret     |
| `allowFrom` | Allowed user IDs  |

### Feishu / DingTalk

These enterprise messaging platforms use a generic token-based setup in the wizard. Enter your App ID and Secret when prompted.

**Feishu**: `appId`, `appSecret`, optional `encryptKey` and `verificationToken`

**DingTalk**: `clientId`, `clientSecret`

---

## Configuration Reference

### Config File Location

```
~/.klyntbot/config.json
```

The configuration uses **camelCase** JSON format. Only non-default values are saved (the config file is minimal by design - the `save()` function diffs against defaults and only writes fields that differ).

### Directory Structure

```
~/.klyntbot/
├── config.json              # Main configuration file
├── .wizard-state.json       # Wizard checkpoint (temporary, during wizard)
├── sessions/                # Conversation session storage
├── cron/                    # Scheduled job definitions
├── media/                   # Media file storage
├── history/                 # Command history
└── workspace/
    ├── AGENTS.md            # Agent behavior configuration
    ├── SOUL.md              # Personality and communication style
    ├── USER.md              # User profile and preferences
    ├── TOOLS.md             # Tool permissions and restrictions
    ├── IDENTITY.md          # Bot identity and capabilities
    ├── memory/
    │   └── MEMORY.md        # Long-term memory storage
    └── skills/              # Custom skill definitions
```

### Full Config Schema

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
  "channels": {
    "telegram": { "enabled": false, "token": "", "allowFrom": [] },
    "discord": { "enabled": false, "token": "", "allowFrom": [] },
    "whatsapp": { "enabled": false, "bridgeUrl": "ws://localhost:3001", "allowFrom": [] },
    "slack": { "enabled": false, "botToken": "", "appToken": "", "allowFrom": [], "mode": "socket" },
    "email": { "enabled": false, "imapHost": "", "smtpHost": "", "allowFrom": [] },
    "qq": { "enabled": false, "appId": "", "secret": "", "allowFrom": [] },
    "feishu": { "enabled": false, "appId": "", "appSecret": "" },
    "dingtalk": { "enabled": false, "clientId": "", "clientSecret": "" },
    "mochat": { "enabled": false, "baseUrl": "https://mochat.io" }
  },
  "providers": {
    "anthropic": { "apiKey": "" },
    "openai": { "apiKey": "" },
    "openrouter": { "apiKey": "" },
    "deepseek": { "apiKey": "" },
    "gemini": { "apiKey": "" },
    "groq": { "apiKey": "" },
    "vllm": { "apiKey": "" },
    "zhipu": { "apiKey": "" },
    "dashscope": { "apiKey": "" },
    "moonshot": { "apiKey": "" },
    "minimax": { "apiKey": "" },
    "aihubmix": { "apiKey": "" }
  },
  "tools": {
    "web": { "braveApiKey": "", "maxResults": 5 },
    "exec": { "timeout": 60, "allowedCommands": [] },
    "restrictToWorkspace": false
  },
  "gateway": {
    "host": "0.0.0.0",
    "port": 18790
  }
}
```

### Agent Defaults

| Field              | Type     | Default                      | Description                     |
|-------------------|----------|------------------------------|---------------------------------|
| `workspace`        | string   | `~/.klyntbot/workspace`      | Path to workspace directory     |
| `model`            | string   | `anthropic/claude-opus-4-5`  | Default LLM model to use       |
| `maxTokens`        | integer  | `8192`                       | Maximum response tokens         |
| `temperature`      | float    | `0.7`                        | Response randomness (0.0-1.0)   |
| `maxToolIterations`| integer  | `20`                         | Max tool calls per turn         |

### Provider Configuration

Each provider accepts:

| Field          | Type   | Description                             |
|---------------|--------|-----------------------------------------|
| `apiKey`       | string | API key (stored securely)               |
| `apiBase`      | string | Custom API endpoint (optional)          |
| `extraHeaders` | object | Additional HTTP headers (optional)      |

### Tools Configuration

| Field                  | Type     | Default | Description                        |
|-----------------------|----------|---------|------------------------------------|
| `restrictToWorkspace`  | boolean  | `false` | Limit file operations to workspace |
| `exec.timeout`         | integer  | `60`    | Shell command timeout (seconds)    |
| `exec.allowedCommands` | array    | `[]`    | Allowed shell commands (empty = all)|
| `web.braveApiKey`      | string   | `""`    | Brave Search API key               |
| `web.maxResults`       | integer  | `5`     | Max web search results             |

### Gateway Configuration

| Field  | Type    | Default     | Description              |
|--------|---------|-------------|--------------------------|
| `host`  | string  | `0.0.0.0`  | Server bind address      |
| `port`  | integer | `18790`     | Server listen port       |

---

## Environment Variables

Any configuration value can be overridden via environment variables using the `KLYNTBOT_` prefix with `__` (double underscore) as a nesting separator.

### Common Overrides

```bash
# Agent defaults
KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o
KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE=/custom/workspace
KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE=0.5
KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS=4096

# Provider API keys
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...
KLYNTBOT_PROVIDERS__OPENAI__API_KEY=sk-...
KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY=sk-or-...
KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY=...
KLYNTBOT_PROVIDERS__GEMINI__API_KEY=...
KLYNTBOT_PROVIDERS__GROQ__API_KEY=...

# Channel tokens
KLYNTBOT_CHANNELS__TELEGRAM__TOKEN=123456:ABC...
KLYNTBOT_CHANNELS__DISCORD__TOKEN=...
KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN=xoxb-...
KLYNTBOT_CHANNELS__SLACK__APP_TOKEN=xapp-...

# Tools
KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY=...
KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

Environment variables take precedence over values in `config.json`.

---

## Troubleshooting

### Wizard doesn't start

**Symptom**: `klyntbot init` shows an error or exits immediately.

**Fix**: Ensure `$HOME` is set and you have write access to `~/.klyntbot/`:
```bash
mkdir -p ~/.klyntbot
klyntbot init
```

### API key validation fails

**Symptom**: "API key seems too short" or format validation error.

**Fix**: Ensure you're copying the full key. Key format expectations:

| Provider  | Prefix    | Minimum Length |
|----------|-----------|---------------|
| Anthropic | `sk-ant-` | 20+           |
| OpenAI    | `sk-`     | 20+           |
| Slack bot | `xoxb-`   | 20+           |
| Slack app | `xapp-`   | 20+           |
| Telegram  | `digits:` | 10+           |

### API connection test fails

**Symptom**: "Connection failed: timeout" or "401 Unauthorized" during validation.

**Possible causes**:
- API key is incorrect or expired
- Network/firewall blocking outbound HTTPS
- Custom API base URL is wrong

**Fix**: Retry with the correct key, or skip validation and fix later with `klyntbot config set`.

### Channel won't connect

**Symptom**: Channel is enabled but not responding.

1. Check channel status: `klyntbot channels list`
2. Test the connection: `klyntbot channels test <channel-name>`
3. Verify the daemon is running: `klyntbot status --verbose`
4. Check credentials haven't expired

### OAuth callback not received

**Symptom**: Browser opens but wizard times out waiting for authorization.

**Fix**: The callback server runs on `localhost:17891`. Ensure no firewall is blocking localhost. If the browser didn't open, manually visit the URL shown in the terminal. If all else fails, choose "Enter tokens manually" to paste them directly.

### Wizard progress lost

**Symptom**: Restarting the wizard goes back to the beginning.

**Fix**: The wizard saves checkpoints to `~/.klyntbot/.wizard-state.json`. If this file was deleted, progress is lost. The checkpoint is only cleaned up on successful completion.

### Config not saving

**Symptom**: Changes from the wizard don't appear in `klyntbot config show`.

**Note**: The config file uses minimal diff saving - only non-default values are stored. Default values like `temperature: 0.7` or `maxTokens: 8192` won't appear in the file but are still active. Use `klyntbot config show` to see the full effective config.

### Service won't start

**macOS**:
```bash
launchctl list | grep klyntbot            # Check if loaded
launchctl start io.klyntbot.agent         # Start manually
cat /tmp/klyntbot.stderr.log              # Check error logs
```

**Linux**:
```bash
systemctl --user status klyntbot          # Check status
journalctl --user -u klyntbot            # Check logs
systemctl --user restart klyntbot         # Restart
```

---

## FAQ

### Can I configure multiple providers?

Yes. The wizard asks "Would you like to configure additional providers?" after the primary setup. You can also manually add provider keys to the config. The active provider is auto-detected from the model name prefix.

### Can I run the wizard again?

Yes. Running `klyntbot init` again detects your existing config and lets you reconfigure, update specific settings, or just validate. Workspace template files are never overwritten. The old config is backed up to `config.json.bak`.

### Where are my API keys stored?

In `~/.klyntbot/config.json`, as plaintext (wrapped in `Secret<String>` in memory for log redaction). Set file permissions:

```bash
chmod 600 ~/.klyntbot/config.json
```

### Can I use klyntbot without any channels?

Yes. klyntbot works in CLI mode by default:
```bash
klyntbot chat "What is the weather?"
klyntbot chat  # Interactive REPL mode
```

### How do I add a custom API endpoint?

The wizard's "Other / Custom" option lets you set an API base URL during setup. Or edit the config directly:

```json
{
  "providers": {
    "openai": {
      "apiKey": "sk-...",
      "apiBase": "https://your-proxy.example.com/v1"
    }
  }
}
```

### What are skills?

Skills are reusable instruction sets extending the agent's capabilities. Built-in: summarize, skill-creator, github, tmux, weather, cron. List them with `klyntbot skills list`.

### How do allowlists work?

Each channel's `allowFrom` field controls access. Empty means anyone can interact. Values are platform-specific IDs:

- **Telegram**: User IDs or group chat IDs
- **Discord**: Guild IDs or channel IDs
- **Slack**: Channel IDs or user IDs
- **WhatsApp**: Phone numbers or group IDs
- **Email**: Email addresses

### How do I start klyntbot as a background service?

The wizard can install it for you (Phase 5). Or manually:

```bash
klyntbot serve                    # Foreground
klyntbot serve --port 8080        # Custom port
klyntbot serve --verbose          # Debug logging
```

### What's the workspace for?

The workspace (`~/.klyntbot/workspace/`) stores files that shape the agent:

- **AGENTS.md**: Core capabilities and constraints
- **SOUL.md**: Personality and communication style
- **USER.md**: Your profile and preferences
- **TOOLS.md**: Tool permissions
- **IDENTITY.md**: Bot name and description
- **memory/MEMORY.md**: Persistent memory across sessions

Edit these to customize behavior.

### What's the difference between the wizard and `klyntbot channels login`?

The wizard provides a guided, all-in-one setup. `klyntbot channels login <name>` sets up a single channel after initial setup. Both write to the same config file.
