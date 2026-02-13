# Klyntbot Enhanced Wizard - UX Design Document

## Table of Contents

1. [Design Philosophy](#design-philosophy)
2. [Color Scheme & Typography](#color-scheme--typography)
3. [Reusable UI Components](#reusable-ui-components)
4. [Wizard Flow Overview](#wizard-flow-overview)
5. [Phase 1: Welcome & Overview](#phase-1-welcome--overview)
6. [Phase 2: Provider Setup](#phase-2-provider-setup)
7. [Phase 3: Channel Configuration](#phase-3-channel-configuration)
8. [Phase 4: Tools & Permissions](#phase-4-tools--permissions)
9. [Phase 5: Daemon/Service Installation](#phase-5-daemonservice-installation)
10. [Phase 6: Validation & Testing](#phase-6-validation--testing)
11. [Phase 7: Summary & Next Steps](#phase-7-summary--next-steps)
12. [Error & Validation Patterns](#error--validation-patterns)
13. [OAuth Browser Flow](#oauth-browser-flow)
14. [Keyboard & Navigation](#keyboard--navigation)
15. [Accessibility & Graceful Degradation](#accessibility--graceful-degradation)

---

## Design Philosophy

### Principles

1. **Progressive disclosure** - Show only what's needed at each step; advanced options are tucked behind "More options" prompts
2. **Confidence-building** - Every step ends with visible confirmation so users never wonder "did that work?"
3. **Skippable depth** - Channels, daemon, and tools are all optional; the minimal path is Provider + Workspace (2 steps)
4. **Recoverable** - Every input can be retried; back-navigation is supported via `b`; the wizard can be re-run safely
5. **Terminal-native** - No TUI framework dependency; built entirely on ANSI codes, Unicode box-drawing, and stdin/stdout
6. **Quiet by default** - Minimal output until the user asks for detail; help is one `?` keypress away

### Terminal Width

All layouts target **80-column terminals** as the minimum. Content auto-wraps at the detected terminal width. Box-drawing and progress bars scale proportionally.

---

## Color Scheme & Typography

### Color Palette (maps to existing terminal.rs constants)

| Role           | ANSI Code       | Constant     | Usage                                      |
|----------------|-----------------|--------------|---------------------------------------------|
| Primary text   | (default)       | -            | Body text, user input echo                   |
| Header/Title   | `\x1b[1m`      | `BOLD`       | Step titles, section headers                 |
| Accent         | `\x1b[36m`     | `TOOL`       | Selected values, highlighted items, branding |
| Success        | `\x1b[32m`     | `SUCCESS`    | Checkmarks, confirmed values                 |
| Error          | `\x1b[31m`     | `ERROR`      | Error messages, failed validations           |
| Warning        | `\x1b[33m`     | `WARNING`    | Caution notes, skipped steps                 |
| Dim/Help       | `\x1b[90m`     | `DIM`        | Help text, defaults, descriptions            |
| Prompt         | `\x1b[2;34m`   | `PROMPT`     | Input prompts, navigation hints              |
| Link           | `\x1b[4m`      | `UNDERLINE`  | URLs, clickable references                   |
| Separator      | `\x1b[2;90m`   | `SEPARATOR`  | Box borders, horizontal rules                |

### New Constants to Add

```rust
pub const HIGHLIGHT: &str = "\x1b[36;1m";   // Bold cyan - selected item in lists
pub const BG_SELECT: &str = "\x1b[46;30m";  // Cyan bg, black text - cursor item
pub const BRAND: &str = "\x1b[35m";         // Magenta - klyntbot branding
```

### Typography Hierarchy

```
BOLD          = Step titles:     "Step 2 of 7: Provider Setup"
TOOL (cyan)   = Active values:   "claude-sonnet-4-5"
DIM           = Descriptions:    "Recommended for best quality"
PROMPT        = Input cues:      "API Key: "
default       = Body text:       "Enter your API key below."
```

---

## Reusable UI Components

### 1. Progress Bar (Top-of-Screen)

Shows at the top of every step. Communicates where the user is and what's ahead.

```
  ● Provider  ─  ○ Channels  ─  ○ Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━                                               Step 2 of 7
```

Implementation: A function `draw_progress(current: usize, total: usize, labels: &[&str])`.
- Completed steps: `●` in SUCCESS green
- Current step: `●` in TOOL cyan (bold)
- Future steps: `○` in DIM
- The bar below uses `━` (heavy horizontal) for completed, `─` for remaining
- Step counter right-aligned: `Step N of M` in DIM

### 2. Single-Select List

Arrow-key driven with visual cursor. Fallback: numbered input.

```
  Select your LLM provider:

    ● Anthropic (Claude)       Recommended for best quality
      OpenAI (GPT)             Industry standard models
      DeepSeek                 Cost-effective alternative
      Google (Gemini)          Multimodal capabilities
      OpenRouter               Access to many models

  ↑/↓ navigate  ·  Enter select  ·  ? help
```

- `●` (filled circle) for focused item, rendered in `HIGHLIGHT` (bold cyan)
- Descriptions right-aligned in `DIM`
- Bottom hint bar in `DIM`
- Fallback for non-TTY: numbered list with `[1]` default

### 3. Multi-Select List (Checkbox)

For selecting multiple channels or tools.

```
  Select channels to enable:

    [✓] Telegram           Bot token required
    [✓] Discord            Bot token + gateway
    [ ] WhatsApp           Requires bridge server
    [ ] Slack              Bot + App tokens (OAuth)
    [ ] Email              IMAP/SMTP credentials
    [ ] QQ                 App ID + secret
    [ ] Feishu/Lark        App credentials
    [ ] DingTalk           Client credentials

  ↑/↓ navigate  ·  Space toggle  ·  Enter confirm  ·  a all  ·  n none
```

- `[✓]` in SUCCESS green for selected, `[ ]` in DIM for unselected
- Cursor indicator: `>` prefix or background highlight on focused row
- `a` toggles all, `n` toggles none
- Fallback: comma-separated numbers `1,2,5`

### 4. Secure Input (API Keys / Passwords)

```
  API Key: ●●●●●●●●●●●●●●●●sk-ant...3kf
```

- Characters masked with `●` as typed
- Last 6 characters shown for verification
- On validation failure, the full masked value persists so the user can see length

### 5. Inline Validation

```
  API Key: sk-short
  ✗ API key seems too short (10+ characters expected)

  API Key: sk-ant-api03-valid-key-here
  ✓ Key format looks valid
```

- Validation runs on Enter (not per-keystroke to avoid flicker)
- `✓` in SUCCESS with brief confirmation
- `✗` in ERROR with specific guidance
- Cursor returns to the same input for retry

### 6. Help Tooltip

Triggered by pressing `?` at any prompt.

```
  ┌─ Help ──────────────────────────────────────────────────┐
  │                                                          │
  │  Your Anthropic API key starts with 'sk-ant-' and can   │
  │  be found at: https://console.anthropic.com              │
  │                                                          │
  │  The key is stored in ~/.klyntbot/config.json and is     │
  │  never sent anywhere except the Anthropic API.           │
  │                                                          │
  │  Press any key to continue...                            │
  └──────────────────────────────────────────────────────────┘
```

- Uses existing `draw_box` with "Help" header
- Content is context-sensitive per field
- Dismisses on any keypress

### 7. Confirmation Card

Shown after completing a configuration section.

```
  ┌─ Provider Configured ───────────────────────────────────┐
  │                                                          │
  │  Provider:  Anthropic                                    │
  │  Model:     claude-sonnet-4-5                            │
  │  API Key:   sk-ant-●●●●●●●3kf                           │
  │                                                          │
  │  ✓ Configuration saved                                   │
  └──────────────────────────────────────────────────────────┘
```

- Green `✓` in the footer confirms save
- Key values shown with secrets partially masked
- Brief pause (~500ms) then auto-advance, or press Enter to continue

### 8. Section Separator

Between wizard phases, a clean break:

```

  ════════════════════════════════════════════════════════════
  Step 3 of 7: Channel Configuration
  ════════════════════════════════════════════════════════════

```

- Double-line (`═`) separators for phase transitions
- Single-line (`─`) separators for sub-steps within a phase
- Step title in BOLD, centered

---

## Wizard Flow Overview

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

### Minimal Path (speed run)
Provider -> Workspace -> Done. Everything else is optional.

### Skip Logic
- If user answers "no" to channels -> skip Phase 3 entirely
- If user answers "no" to daemon -> skip Phase 5
- Tools section defaults are safe; skip prompt appears if no Brave API key

---

## Phase 1: Welcome & Overview

### Screen: Welcome

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

### Interaction
- Enter -> advance to Phase 2
- `q` -> exit with "Run `klyntbot init` anytime to continue."
- The step list uses numbered bullets; optional items marked with `(optional)` in DIM
- If `~/.klyntbot/config.json` already exists, show: "Existing config found. This will update it (backup at config.json.bak)"

### Returning User Detection

```
  ┌─ Existing Configuration Found ──────────────────────────┐
  │                                                          │
  │  Provider:  Anthropic (claude-sonnet-4-5)                │
  │  Channels:  Telegram ✓  Discord ✓                        │
  │  Last run:  2 days ago                                   │
  │                                                          │
  └──────────────────────────────────────────────────────────┘

  What would you like to do?

    ● Reconfigure everything       Start fresh
      Update specific settings     Choose what to change
      Validate & test              Check everything works

  ↑/↓ navigate  ·  Enter select
```

---

## Phase 2: Provider Setup

### Screen 2a: Select Provider

```
  ● Provider  ─  ○ Channels  ─  ○ Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━                                                       Step 2 of 7

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

### Screen 2b: API Key Entry

```
  Provider: Anthropic (Claude)

  Get your API key at: https://console.anthropic.com
  (The link above is clickable in most terminals)

  API Key: ●●●●●●●●●●●●●●●●●●●●●●●●sk-ant...3kf
  ✓ Key format looks valid (sk-ant prefix detected)

  Model [claude-sonnet-4-5]: _

  Available models:
    claude-sonnet-4-5      Balanced speed & quality (recommended)
    claude-opus-4-5        Highest quality, slower
    claude-haiku-4-5       Fastest, most affordable

  Press Enter for default, or type a model name...
```

### Screen 2c: Provider Confirmation

```
  ┌─ Provider Configured ───────────────────────────────────┐
  │                                                          │
  │  Provider:  Anthropic                                    │
  │  Model:     claude-sonnet-4-5                            │
  │  API Key:   sk-ant-●●●●●●●●●●3kf                        │
  │                                                          │
  │  ✓ Saved to config                                       │
  └──────────────────────────────────────────────────────────┘

  Press Enter to continue...
```

### "Other / Custom" Provider Flow

```
  Custom Provider Setup

  API Base URL: https://my-company.com/v1
  API Key: ●●●●●●●●●●●●●●●●
  Model name: my-custom-model

  Which provider format does this API follow?

    ● OpenAI-compatible     /v1/chat/completions format
      Anthropic-compatible  /v1/messages format

  ↑/↓ navigate  ·  Enter select
```

---

## Phase 3: Channel Configuration

### Screen 3a: Channel Selection

```
  ● Provider  ─  ● Channels  ─  ○ Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━━━━━                                           Step 3 of 7

  ════════════════════════════════════════════════════════════
  Step 3 of 7: Enable Chat Channels (Optional)
  ════════════════════════════════════════════════════════════

  Channels let klyntbot respond on messaging platforms.
  You can always add more later with: klyntbot channels login <name>

  Would you like to set up channels now? [y/N]: _
```

If yes:

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

### Screen 3b: Telegram Setup (simplest example)

```
  ─── Telegram Setup ───────────────────────────────────────

  1. Open Telegram and message @BotFather
  2. Send /newbot and follow the prompts
  3. Copy the bot token you receive

  Bot Token: ●●●●●●●●●●●●●●●●●●●●●●:●●●●●●●●●●
  ✓ Token format looks valid

  Access Control
  ─────────────
  Who should be able to talk to the bot?

    ● Anyone                 No restrictions
      Only specific users    Enter Telegram user IDs

  ↑/↓ navigate  ·  Enter select
```

If "Only specific users":

```
  Enter Telegram user IDs (comma-separated):
  Allow from: 123456789, 987654321
  ✓ 2 users configured

  ┌─ Telegram Configured ───────────────────────────────────┐
  │                                                          │
  │  Token:       ●●●●●●●●●●:●●●●●●●●●●                    │
  │  Access:      2 specific users                           │
  │  Status:      ✓ Enabled                                  │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

### Screen 3c: Discord Setup

```
  ─── Discord Setup ────────────────────────────────────────

  1. Go to https://discord.com/developers/applications
  2. Create a New Application
  3. Go to Bot tab, click "Reset Token", copy it
  4. Enable MESSAGE CONTENT intent under Privileged Intents
  5. Go to OAuth2 > URL Generator, select "bot" scope
  6. Use the generated URL to invite bot to your server

  Bot Token: ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●
  ✓ Token format looks valid

  Access Control
  ─────────────
  Restrict to specific servers/guilds? [y/N]: _
```

### Screen 3d: Slack Setup (OAuth flow)

```
  ─── Slack Setup ──────────────────────────────────────────

  Slack uses Socket Mode for real-time messaging.

  1. Go to https://api.slack.com/apps
  2. Create a New App > From Manifest
  3. Paste this manifest:

  ┌─ manifest.yml ──────────────────────────────────────────┐
  │ display_information:                                     │
  │   name: klyntbot                                         │
  │ features:                                                │
  │   bot_user:                                              │
  │     display_name: klyntbot                               │
  │ oauth_config:                                            │
  │   scopes:                                                │
  │     bot:                                                 │
  │       - chat:write                                       │
  │       - app_mentions:read                                │
  │ settings:                                                │
  │   socket_mode_enabled: true                              │
  └──────────────────────────────────────────────────────────┘

  4. Install to workspace, copy Bot Token
  5. Go to Socket Mode, generate App-Level Token

  Bot Token (xoxb-...): ●●●●●●●●●●●●●●●●●●●●●●●●
  ✓ Token format looks valid (xoxb- prefix)

  App Token (xapp-...): ●●●●●●●●●●●●●●●●●●●●●●●●
  ✓ Token format looks valid (xapp- prefix)
```

### Screen 3e: Email Setup

```
  ─── Email Setup ──────────────────────────────────────────

  klyntbot monitors an email inbox and can reply automatically.

  Quick setup for common providers:

    ● Gmail              Uses app passwords
      Outlook/Hotmail    IMAP/SMTP settings
      Custom IMAP/SMTP   Enter server details

  ↑/↓ navigate  ·  Enter select
```

Gmail selected:

```
  Gmail Setup
  ───────────
  1. Enable 2-factor authentication on your Google account
  2. Go to https://myaccount.google.com/apppasswords
  3. Generate an app password for "Mail"

  Gmail address: user@gmail.com
  App password:  ●●●●●●●●●●●●●●●●
  ✓ Credentials format valid

  Auto-detected settings:
    IMAP: imap.gmail.com:993 (SSL)
    SMTP: smtp.gmail.com:587 (TLS)

  ┌─ Email Configured ──────────────────────────────────────┐
  │                                                          │
  │  Account:  user@gmail.com                                │
  │  IMAP:     imap.gmail.com:993 (SSL ✓)                   │
  │  SMTP:     smtp.gmail.com:587 (TLS ✓)                   │
  │  Status:   ✓ Enabled                                     │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

### Screen 3f: Channel Summary

After all selected channels are configured:

```
  ─── Channels Summary ─────────────────────────────────────

  ┌──────────┬───────────┬──────────────────────────────────┐
  │ Channel  │ Status    │ Details                          │
  ├──────────┼───────────┼──────────────────────────────────┤
  │ Telegram │ ✓ Enabled │ 2 allowed users                  │
  │ Discord  │ ✓ Enabled │ All servers                      │
  │ Slack    │ ✗ Skipped │                                  │
  │ WhatsApp │ ✗ Skipped │                                  │
  │ Email    │ ✓ Enabled │ user@gmail.com                   │
  │ QQ       │ ✗ Skipped │                                  │
  └──────────┴───────────┴──────────────────────────────────┘

  3 channels enabled. Press Enter to continue...
```

---

## Phase 4: Tools & Permissions

### Screen 4a: Tools Overview

```
  ● Provider  ─  ● Channels  ─  ● Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                             Step 4 of 7

  ════════════════════════════════════════════════════════════
  Step 4 of 7: Tools & Permissions (Optional)
  ════════════════════════════════════════════════════════════

  klyntbot can use tools to interact with your system.
  Default settings are safe for most users.

  Would you like to customize tool permissions? [y/N]: _
```

If yes:

```
  Tool Permissions
  ────────────────

  File Access
    Restrict to workspace only? [Y/n]: _
    Workspace: ~/.klyntbot/workspace

  Shell Execution
    Command timeout [60s]: _
    Restrict to specific commands? [y/N]: _

  Web Tools
    Brave Search API key (optional): _
    Get one at: https://api.search.brave.com/app/keys

  ┌─ Tools Configured ──────────────────────────────────────┐
  │                                                          │
  │  File access:    Workspace only (~/.klyntbot/workspace)  │
  │  Shell:          60s timeout, all commands               │
  │  Web search:     ✓ Brave API configured                  │
  │  Web fetch:      ✓ Enabled                               │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

---

## Phase 5: Daemon/Service Installation

### Screen 5a: Daemon Setup

```
  ● Provider  ─  ● Channels  ─  ● Tools  ─  ● Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                 Step 5 of 7

  ════════════════════════════════════════════════════════════
  Step 5 of 7: Background Agent (Optional)
  ════════════════════════════════════════════════════════════

  Run klyntbot as a background service so it stays connected
  to your chat channels even when the terminal is closed.

  Install as system service? [y/N]: _
```

If yes:

```
  Service Installation
  ────────────────────

  Detected platform: macOS

  Installation method:

    ● launchd (recommended)   Auto-starts on login
      Manual                  Run with: klyntbot serve

  ↑/↓ navigate  ·  Enter select
```

On macOS/launchd:

```
  Installing launchd service...

    ✓ Created ~/Library/LaunchAgents/com.klyntbot.agent.plist
    ✓ Service registered with launchd
    ✓ Service configured to start on login

  Gateway Configuration
  ─────────────────────
  HTTP API port [18790]: _
  Bind address [0.0.0.0]: _

  ┌─ Service Installed ─────────────────────────────────────┐
  │                                                          │
  │  Type:     launchd (auto-start on login)                 │
  │  Gateway:  http://0.0.0.0:18790                          │
  │  Control:  launchctl start com.klyntbot.agent            │
  │  Logs:     ~/Library/Logs/klyntbot.log                   │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

On Linux/systemd:

```
  Detected platform: Linux

  Installation method:

    ● systemd user service    Auto-starts on login
      systemd system service  Runs as system daemon (requires sudo)
      Manual                  Run with: klyntbot serve

  Installing systemd user service...

    ✓ Created ~/.config/systemd/user/klyntbot.service
    ✓ Reloaded systemd daemon
    ✓ Enabled klyntbot.service

  ┌─ Service Installed ─────────────────────────────────────┐
  │                                                          │
  │  Type:     systemd user service                          │
  │  Control:  systemctl --user start klyntbot               │
  │  Logs:     journalctl --user -u klyntbot                 │
  │  Status:   systemctl --user status klyntbot              │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

---

## Phase 6: Validation & Testing

### Screen 6a: Validation Runner

```
  ● Provider  ─  ● Channels  ─  ● Tools  ─  ● Daemon  ─  ● Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━          Step 6 of 7

  ════════════════════════════════════════════════════════════
  Step 6 of 7: Validation & Testing
  ════════════════════════════════════════════════════════════

  Running validation checks...

    ⣾ Testing Anthropic API connection...
```

Progress updates in-place (spinner on current check):

```
  Running validation checks...

    ✓ Config file valid                              0.1s
    ✓ Workspace directories exist                    0.0s
    ✓ Anthropic API connection                       1.2s
    ⣾ Telegram bot authentication...
```

### Screen 6b: Validation Results (all pass)

```
  Validation Results
  ──────────────────

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

  Press Enter to continue...
```

### Screen 6c: Validation Results (with failures)

```
  Validation Results
  ──────────────────

    ✓ Config file valid                              0.1s
    ✓ Workspace directories exist                    0.0s
    ✗ Anthropic API connection                       2.1s
    ✓ Telegram bot authenticated                     0.8s
    ! Email SMTP connection                          3.0s

  3 passed  ·  1 failed  ·  1 warning

  ─── Issues Found ─────────────────────────────────────────

  ✗ Anthropic API connection failed
    Error: 401 Unauthorized
    The API key may be incorrect or expired.
    Fix: Run klyntbot config set providers.anthropic.apiKey <key>
         or re-run klyntbot init

  ! Email SMTP: Connection timed out
    The SMTP server didn't respond within 3 seconds.
    This might be a firewall or network issue.
    Fix: Check smtp_host and smtp_port in config, or try later.

  Would you like to:

    ● Fix issues now          Re-enter failed credentials
      Continue anyway         Issues can be fixed later
      Quit                    Exit wizard

  ↑/↓ navigate  ·  Enter select
```

### Screen 6d: Live Test (optional)

```
  Would you like to send a test message? [y/N]: _
```

If yes:

```
  Test Message
  ────────────

  ⣾ Sending "Hello from klyntbot!" to Anthropic API...

  ─── Response ─────────────────────────────────────────────
  Hello! I'm klyntbot, your AI assistant. Everything seems
  to be working correctly. How can I help you today?
  ─────────────────────────────────────────────────────────

  ✓ LLM responded successfully (1.4s, 23 tokens)

  Send test to channels too? [y/N]: _
```

If testing channels:

```
  Channel Tests
  ─────────────

    ✓ Telegram: Message sent to bot chat               0.8s
    ✓ Discord: Message sent to first available channel  0.6s
    ✗ Email: Failed to send test email                  3.1s
      Error: Authentication failed
```

---

## Phase 7: Summary & Next Steps

### Screen 7a: Final Summary

```
  ● Provider  ─  ● Channels  ─  ● Tools  ─  ● Daemon  ─  ● Validate  ─  ● Done
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  ┌─ Setup Complete! ───────────────────────────────────────┐
  │                                                          │
  │   █▄▀ █   █ █▄ █ ▀█▀ █▄▄ █▀█ ▀█▀                       │
  │   █ █ █▄▄ ▄█ █ ▀█  █  █▄█ █▄█  █                        │
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
    $ klyntbot channels start telegram

  Check status:
    $ klyntbot status --verbose

  More help:
    $ klyntbot --help
```

---

## Error & Validation Patterns

### Pattern 1: Inline Field Error

```
  API Key: sk-short
  ✗ API key seems too short (10+ characters expected)
  API Key: _
```

- Error appears below the field in `ERROR` red
- Cursor returns to the field automatically
- Previous invalid value is cleared

### Pattern 2: Contextual Error with Suggestion

```
  API Key: bearer_token_12345
  ✗ This doesn't look like an Anthropic API key
    Anthropic keys start with "sk-ant-"
    Get yours at: https://console.anthropic.com
  API Key: _
```

- Shows what format is expected
- Provides the URL to get the correct value
- Re-prompts for input

### Pattern 3: Network Error with Retry

```
  ⣾ Testing API connection...
  ✗ Connection failed: timeout after 5s

  Possible causes:
    - Network connectivity issue
    - API endpoint unreachable
    - Firewall blocking outbound HTTPS

  Retry? [Y/n]: _
```

### Pattern 4: Partial Success Warning

```
  ! Telegram bot authenticated but has limited permissions
    Missing: can_read_all_group_messages
    The bot may not respond in group chats.
    Fix: Message @BotFather, send /setprivacy, select Disable

  Continue anyway? [Y/n]: _
```

### Pattern 5: Skipped Step Acknowledgment

```
  ○ Channels: Skipped (you can add them later)
    Run: klyntbot channels login <channel-name>
```

Light gray `○` indicator, brief note on how to revisit.

### General Validation Rules

| Field | Validation | Error Message |
|-------|-----------|---------------|
| Anthropic key | starts with `sk-ant-`, len >= 20 | "Anthropic keys start with 'sk-ant-'" |
| OpenAI key | starts with `sk-`, len >= 20 | "OpenAI keys start with 'sk-'" |
| DeepSeek key | len >= 10 | "Key seems too short" |
| Telegram token | matches `\d+:.+` | "Telegram tokens look like '123456:ABC...'" |
| Discord token | len >= 50 | "Discord tokens are typically 60+ characters" |
| Slack bot token | starts with `xoxb-` | "Slack bot tokens start with 'xoxb-'" |
| Slack app token | starts with `xapp-` | "Slack app tokens start with 'xapp-'" |
| Email address | contains `@` and `.` | "Please enter a valid email address" |
| Port number | 1-65535 | "Port must be between 1 and 65535" |
| URL | starts with `http://` or `https://` | "Please enter a valid URL" |

---

## OAuth Browser Flow

For channels that support OAuth (future: Slack, Discord OAuth2):

### Screen: OAuth Initiation

```
  ─── Slack OAuth Setup ────────────────────────────────────

  We'll open your browser to authorize klyntbot with Slack.

  ⣾ Starting local callback server on port 18791...
  ✓ Callback server ready

  Opening browser...
  If it doesn't open, visit this URL:
  https://slack.com/oauth/v2/authorize?client_id=...&scope=...

  ⣾ Waiting for authorization...
     (Press Ctrl+C to cancel)
```

### Screen: OAuth Callback Received

```
  ✓ Authorization received!

  ⣾ Exchanging code for tokens...
  ✓ Bot token obtained
  ✓ App token obtained

  ┌─ Slack Connected ───────────────────────────────────────┐
  │                                                          │
  │  Workspace:  My Company                                  │
  │  Bot user:   @klyntbot                                   │
  │  Channels:   Authorized for 3 channels                   │
  │                                                          │
  └──────────────────────────────────────────────────────────┘
```

### OAuth Error States

```
  ✗ Authorization was denied

  The user clicked "Deny" in the browser.
  Would you like to try again? [y/N]: _
```

```
  ✗ Authorization timed out (60s)

  The callback wasn't received. Possible causes:
    - Browser didn't open (copy the URL manually)
    - Firewall blocking localhost:18791
    - Authorization was abandoned

  Would you like to:
    ● Try again               Re-open the browser
      Enter tokens manually   Paste bot + app tokens
      Skip Slack              Configure later

  ↑/↓ navigate  ·  Enter select
```

---

## Keyboard & Navigation

### Global Keys (available at every prompt)

| Key | Action |
|-----|--------|
| `Enter` | Submit/confirm current selection |
| `?` | Show contextual help tooltip |
| `Ctrl+C` | Abort wizard (with confirmation) |
| `b` | Go back to previous step |
| `q` | Quit wizard |

### List Navigation Keys

| Key | Action |
|-----|--------|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Space` | Toggle selection (multi-select) |
| `Enter` | Confirm selection |
| `a` | Select all (multi-select) |
| `n` | Select none (multi-select) |

### Input Keys

| Key | Action |
|-----|--------|
| `Enter` | Submit value (or accept default) |
| `Ctrl+U` | Clear input line |
| `Backspace` | Delete character |

### Ctrl+C Confirmation

```
  Are you sure you want to quit?
  Progress will be saved. Run klyntbot init to continue.

    ● Save and quit       Configuration saved so far is kept
      Quit without saving Discard all changes
      Cancel              Return to wizard

  ↑/↓ navigate  ·  Enter select
```

---

## Accessibility & Graceful Degradation

### NO_COLOR Support

When `NO_COLOR` is set or stdout is not a TTY:
- All ANSI codes suppressed (existing `colors_enabled()` check)
- Box drawing falls back to ASCII (`+`, `-`, `|`)
- Lists use numbered fallback: `[1] Anthropic (Claude) - Recommended`
- Progress bar uses text: `[Step 2 of 7: Provider Setup]`
- Checkmarks become `[OK]`, errors become `[FAIL]`, warnings become `[WARN]`

### Non-Interactive Mode (piped input)

When stdin is not a TTY:
- Arrow-key navigation disabled
- Uses numbered/text input only
- No spinner animations
- All defaults are accepted unless overridden via input

### Screen Reader Compatibility

- No cursor movement escape codes that confuse screen readers
- Status indicators have text equivalents: `✓ Passed` not just `✓`
- No time-delayed auto-advance; always wait for Enter
- All content is linear (no columns or side-by-side layout)

### Narrow Terminal (< 60 cols)

- Box drawing simplified (no padding)
- Descriptions moved below item names instead of right-aligned
- Progress bar becomes just `[2/7]` text

```
  [2/7] Provider Setup

  Select provider:
    1. Anthropic (Claude)
       Recommended for best quality
    2. OpenAI (GPT)
       Industry standard models
    ...

  Choice [1]: _
```

---

## Implementation Notes

### New Modules to Create

1. **`crates/cli/src/wizard/mod.rs`** - Main wizard orchestrator, phase sequencing
2. **`crates/cli/src/wizard/components.rs`** - Reusable UI components (select, multi-select, progress, etc.)
3. **`crates/cli/src/wizard/providers.rs`** - Provider selection and API key flow
4. **`crates/cli/src/wizard/channels.rs`** - Channel configuration flows
5. **`crates/cli/src/wizard/tools.rs`** - Tool permission configuration
6. **`crates/cli/src/wizard/daemon.rs`** - Service installation
7. **`crates/cli/src/wizard/validation.rs`** - Connection testing and validation

### Terminal Utilities to Add (in `crates/common/src/utils/terminal.rs`)

```rust
// New functions needed:
pub fn draw_progress_bar(current: usize, total: usize, labels: &[&str]) -> String;
pub fn draw_single_select(items: &[SelectItem], selected: usize) -> String;
pub fn draw_multi_select(items: &[MultiSelectItem]) -> String;
pub fn draw_separator_double() -> String;  // ═══ style
pub fn read_password(prompt: &str) -> Result<String>;  // masked input
pub fn read_single_select(items: &[SelectItem]) -> Result<usize>;
pub fn read_multi_select(items: &[MultiSelectItem]) -> Result<Vec<usize>>;
pub fn terminal_width() -> usize;  // detect terminal width, default 80
```

### Dependencies to Consider

- **`crossterm`** or **`termion`** - For raw-mode input (arrow keys, no-echo password). Lightweight, no full TUI.
- OR: Pure ANSI with numbered fallback (zero new deps, but no arrow-key navigation)

### Recommended Approach

Use `crossterm` for raw-mode input handling. It's the smallest dependency that enables:
- Arrow key detection for list navigation
- Hidden input for passwords
- Terminal size detection
- Cursor control for in-place updates (spinner, progress)

This avoids pulling in a full TUI framework while enabling the interactive elements in this design.
