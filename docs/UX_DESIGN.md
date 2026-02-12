# klyntbot CLI UX Design

> A professional, lightning-fast, and delightful command-line interface for your AI assistant

## Design Principles

### 1. **Speed First**
- Instant feedback (<100ms to first prompt)
- Streaming responses character-by-character as LLM generates
- No unnecessary loading screens or delays for local operations
- Async operations never block the UI

### 2. **Clarity Over Decoration**
- Every output should be immediately understandable
- Minimal use of color (only for meaning, not decoration)
- Clear visual hierarchy using whitespace and subtle formatting
- Error messages that explain *what went wrong* and *how to fix it*

### 3. **Minimalism**
- Less is more — no visual clutter
- Clean ASCII art where necessary, but never excessive
- Subtle separators and spacing instead of heavy boxes
- Let content breathe

### 4. **Professional Aesthetic**
- Suitable for open-source projects and production use
- Consistent styling across all commands
- No emoji by default (can be enabled via config)
- High contrast, readable typography

### 5. **Accessibility**
- Color-blind friendly palette
- Plain text fallback mode (--no-color, NO_COLOR env)
- Screen reader compatible output
- All features accessible via keyboard

---

## Command Structure

Improved from nanobot with more intuitive naming and better organization:

```bash
# Core Commands
klyntbot                          # Show brief status + available commands
klyntbot chat                     # Interactive REPL mode
klyntbot chat "your message"      # Single message mode (print response and exit)
klyntbot serve                    # Start gateway daemon (all enabled channels)
klyntbot init                     # Interactive setup wizard
klyntbot status                   # Detailed system status
klyntbot version                  # Version information

# Channel Management
klyntbot channels list            # List all channels and their status
klyntbot channels login <name>    # Channel-specific login (e.g., WhatsApp QR)
klyntbot channels test <name>     # Test channel connection

# Cron/Scheduled Tasks
klyntbot cron list                # List scheduled jobs
klyntbot cron add                 # Interactive job creation wizard
klyntbot cron remove <id>         # Remove a scheduled job
klyntbot cron run <id>            # Manually trigger a job now
klyntbot cron enable <id>         # Enable a disabled job
klyntbot cron disable <id>        # Disable a job without removing it

# Configuration
klyntbot config show              # Display current configuration
klyntbot config get <key>         # Get a specific config value
klyntbot config set <key> <value> # Set a configuration value
klyntbot config edit              # Open config file in $EDITOR
klyntbot config reset             # Reset to default configuration
klyntbot config validate          # Validate configuration file

# Skills Management
klyntbot skills list              # List available skills
klyntbot skills info <name>       # Show skill details
klyntbot skills enable <name>     # Enable a skill
klyntbot skills disable <name>    # Disable a skill
klyntbot skills path              # Show skills directory path

# Workspace
klyntbot workspace path           # Show workspace directory
klyntbot workspace open           # Open workspace in file manager
klyntbot workspace clean          # Clean temporary files

# Diagnostics
klyntbot doctor                   # Check system health and dependencies
klyntbot logs [--tail N]          # View recent logs
```

---

## Interactive REPL Design

### Prompt Style

```
klyntbot> your message here
```

- Simple, unobtrusive prompt
- Uses a subtle color (dim blue/cyan) that doesn't dominate
- No fancy decorations or excessive styling

### Streaming Response Display

```
klyntbot> What is 2+2?

⣾ thinking...

┌─ klyntbot ────────────────────────────────────
│
│ The answer is 4.
│
└───────────────────────────────────────────────

klyntbot>
```

**Key features:**
- Braille spinner (⣾⣽⣻⢿⡿⣟⣯⣷) during thinking — minimal, professional
- Response appears character-by-character as it streams from the LLM
- Clean borders using box-drawing characters
- Soft visual separation without being heavy

### Tool Execution Indicators

When the agent executes tools:

```
⚡ read_file("config.json")
⚡ exec("ls -la")
⚡ web_search("rust async patterns")
```

- Lightning bolt (⚡) prefix for tool execution
- Dim color so it doesn't distract from response content
- Shows what the agent is doing without overwhelming the user

### Multi-line Input

**Option 1: Paste mode**
```
klyntbot> /paste
[paste mode: Ctrl+D or /end to submit]
... multiple lines ...
... of input ...
/end

⣾ thinking...
```

**Option 2: Backslash continuation**
```
klyntbot> This is a long message \
... that spans multiple \
... lines

⣾ thinking...
```

### Command History

- Up/Down arrows navigate history
- History persists across sessions (~/.klyntbot/history)
- Ctrl+R for reverse search through history
- History ignores exit commands and empty lines

### Special Commands

```
exit, quit, /exit, /quit    Exit the REPL
Ctrl+D                      Exit the REPL
/clear                      Clear screen and reset conversation context
/paste                      Enter multi-line paste mode
/help                       Show help
/history                    Show command history
```

### Keyboard Shortcuts

- `Ctrl+C`: Cancel current operation (if generating) or exit (if idle)
- `Ctrl+D`: Exit REPL
- `Ctrl+L`: Clear screen (without resetting context)
- `Ctrl+R`: Reverse history search
- `Up/Down`: Navigate history
- `Home/End`: Jump to start/end of line
- `Ctrl+A/E`: Jump to start/end of line (alternative)

---

## Color Scheme

Minimal, high-contrast palette optimized for both light and dark terminals:

| Element | Color | ANSI Code | Rationale |
|---------|-------|-----------|-----------|
| User input | Default | - | Let terminal theme control |
| Bot response | Default | - | Maximum readability |
| Prompt | Dim Blue | `\x1b[2;34m` | Subtle, doesn't distract |
| Headers | Dim White | `\x1b[2;37m` | Organize sections |
| Tool calls | Cyan | `\x1b[36m` | Informative but not loud |
| Success | Green | `\x1b[32m` | Positive feedback |
| Error | Red | `\x1b[31m` | Immediate attention |
| Warning | Yellow | `\x1b[33m` | Caution |
| Dim text | Gray | `\x1b[90m` | De-emphasize |
| Separators | Dim Gray | `\x1b[2;90m` | Visual structure |

**Fallback modes:**
- `--no-color`: Strip all ANSI codes
- `NO_COLOR=1`: Honor standard environment variable
- Auto-detect non-TTY and disable colors

---

## Onboarding Wizard (`klyntbot init`)

A step-by-step wizard that's clear, efficient, and doesn't overwhelm:

```
╭─ Welcome to klyntbot ────────────────────────╮
│                                               │
│  Let's set up your AI assistant.              │
│  This will take about 2 minutes.              │
│                                               │
╰───────────────────────────────────────────────╯

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 1 of 4: Choose Your LLM Provider

Select your provider (↑↓ to move, Enter to select):

  > Anthropic (Claude)      Recommended for best quality
    OpenAI (GPT)            Industry standard models
    DeepSeek                Cost-effective alternative
    Google (Gemini)         Multimodal capabilities
    OpenRouter              Access to many models
    Local (vLLM)            Run your own models
    Other...

[?] Provider: ▏

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 2 of 4: API Configuration

Anthropic API Key:
  Get yours at: https://console.anthropic.com

[?] API Key: ▏••••••••••••••••

Model [claude-sonnet-4-20250514]: ▏

✓ Configuration validated successfully

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 3 of 4: Enable Channels (Optional)

Would you like to enable chat channels? [y/N]: ▏

[If yes]
  Select channels to enable (Space to toggle, Enter to continue):

  [ ] Telegram        Easy setup, just need bot token
  [ ] Discord         Great for communities
  [ ] WhatsApp        Requires QR code scan
  [ ] Slack           Socket mode, no webhooks needed
  [ ] Email           IMAP/SMTP polling
  [ ] More...

  [Configure selected channels with minimal prompts]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 4 of 4: Workspace Setup

Creating workspace at ~/.klyntbot/workspace/...

  ✓ Created configuration directory
  ✓ Created workspace templates (AGENTS.md, SOUL.md, USER.md)
  ✓ Initialized memory directory
  ✓ Set up skills directory

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

╭─ Setup Complete! ─────────────────────────────╮
│                                               │
│  Your AI assistant is ready to use.           │
│                                               │
│  Try it out:                                   │
│    klyntbot chat                              │
│    klyntbot chat "Hello!"                     │
│                                               │
│  Get help:                                     │
│    klyntbot --help                            │
│    klyntbot status                            │
│                                               │
╰───────────────────────────────────────────────╯
```

**Design notes:**
- Progress clearly shown (Step X of Y)
- Each step is self-contained and clear
- Sensible defaults offered
- Validation happens immediately
- Success feedback at each step
- Final summary shows next steps

---

## Status Display (`klyntbot status`)

Clear, scannable status overview:

```
klyntbot v0.1.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Provider
  anthropic/claude-sonnet-4-20250514

Workspace
  ~/.klyntbot/workspace

Configuration
  ~/.klyntbot/config.json

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Channels                                   Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
telegram                                   ✓ @mybot
discord                                    ✓ MyBot#1234
slack                                      ○ disabled
whatsapp                                   ○ disabled
email                                      ○ disabled

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Services                                   Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
gateway                                    ✓ running (PID 12345)
cron                                       ✓ 3 jobs scheduled
heartbeat                                  ✓ next in 14m

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Skills                                     Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
github                                     ✓ available
weather                                    ✓ available
tmux                                       ✗ missing: tmux binary

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

**Compact alternative** (`klyntbot status --compact`):

```
klyntbot v0.1.0
Provider: anthropic/claude-sonnet-4-20250514
Channels: telegram (✓), discord (✓), slack (○), whatsapp (○)
Services: gateway (✓), cron (✓ 3 jobs), heartbeat (✓)
Skills: 15 available, 2 unavailable
```

---

## Detailed Status (`klyntbot status --verbose`)

```
klyntbot v0.1.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

System Information
  OS: macOS 14.2.1 (arm64)
  Rust: 1.76.0
  Config: ~/.klyntbot/config.json (valid)
  Workspace: ~/.klyntbot/workspace (12 sessions, 45MB)

Provider Configuration
  Active: anthropic
  Model: claude-sonnet-4-20250514
  API Key: sk-ant-***************xyz (valid)
  Base URL: https://api.anthropic.com/v1
  Max Tokens: 8192
  Temperature: 0.7

Channels (3 enabled, 2 disabled)
  telegram
    Status: Connected
    Bot: @mybot_assistant
    Users: 2 allowed
    Last message: 5 minutes ago

  discord
    Status: Connected
    Bot: MyBot#1234
    Gateway: wss://gateway.discord.gg
    Intents: GUILDS, MESSAGES, DIRECT_MESSAGES, MESSAGE_CONTENT
    Last message: 32 minutes ago

  slack
    Status: Disabled
    Reason: Not configured

  whatsapp
    Status: Disabled
    Reason: Bridge not running

  email
    Status: Disabled
    Reason: No IMAP credentials

Cron Jobs (3 total, 3 active)
  daily-standup (cron: 0 9 * * *)
    Next run: 2026-02-12 09:00:00
    Last run: 2026-02-11 09:00:00 (success)

  hourly-check (every: 3600s)
    Next run: 2026-02-11 19:42:00
    Last run: 2026-02-11 18:42:00 (success)

  reminder-task (at: 2026-02-15 14:00:00)
    Next run: 2026-02-15 14:00:00
    Status: Pending

Skills (15 available, 2 unavailable)
  ✓ github - GitHub repository operations
  ✓ weather - Weather forecasts and current conditions
  ✓ summarize - Summarize long documents
  ✗ tmux - tmux session management (missing: tmux binary)
  ✗ docker - Docker container operations (missing: docker binary)

Memory
  Sessions: 12 active
  Total size: 45MB
  Oldest: 2026-01-15
  Newest: 2026-02-11

Performance
  Uptime: 3 days, 14 hours
  API calls: 1,247 (avg 285ms)
  Errors: 3 (0.24%)
```

---

## Error Display

Errors should be **immediately clear** and **actionable**:

### Connection Error
```
Error: Failed to connect to Telegram

Cause:
  Invalid bot token

How to fix:
  1. Get a new token from @BotFather on Telegram
  2. Update your configuration:
     klyntbot config set channels.telegram.token YOUR_TOKEN
  3. Restart the service:
     klyntbot serve

Documentation:
  https://klyntbot.dev/docs/channels/telegram
```

### Configuration Error
```
Error: Invalid configuration

Problem:
  ~/.klyntbot/config.json:12:5
  Missing required field: providers.anthropic.api_key

How to fix:
  1. Get an API key from https://console.anthropic.com
  2. Add it to your config:
     klyntbot config set providers.anthropic.api_key YOUR_KEY

Or run the setup wizard again:
  klyntbot init
```

### Validation Error
```
Error: Model not available

Problem:
  Model 'claude-opus-5' is not supported by provider 'anthropic'

Available models:
  - claude-opus-4-5
  - claude-sonnet-4-5
  - claude-haiku-4-5

How to fix:
  klyntbot config set agents.defaults.model claude-opus-4-5
```

### Permission Error
```
Error: Permission denied

Problem:
  Cannot write to ~/.klyntbot/config.json

How to fix:
  Check file permissions:
    ls -la ~/.klyntbot/config.json

  Fix permissions:
    chmod 644 ~/.klyntbot/config.json

  Or recreate configuration:
    klyntbot init
```

---

## Markdown Rendering in Terminal

Rich markdown support in terminal output:

### Text Formatting
- **Bold**: ANSI bold (`\x1b[1m`)
- *Italic*: ANSI italic (`\x1b[3m`) with fallback to underline
- `Code`: Background color with monospace font
- ~~Strikethrough~~: Crossed out text

### Code Blocks
```
┌─ python ──────────────────────────────────────
│ def fibonacci(n):
│     if n <= 1:
│         return n
│     return fibonacci(n-1) + fibonacci(n-2)
└───────────────────────────────────────────────
```

**Features:**
- Syntax highlighting using a fast highlighter
- Language label in header
- Subtle box drawing
- Copy-friendly (no line numbers by default)

### Lists
Proper indentation and bullets:

```
● First item
  - Nested item 1
  - Nested item 2
● Second item
  1. Numbered sub-item
  2. Another numbered item
● Third item
```

### Links
```
Read more: https://klyntbot.dev/docs
Documentation: [User Guide](https://klyntbot.dev/guide)
```

- URLs shown in full
- Clickable in modern terminals
- Underlined for visibility

### Tables
```
┌─────────────┬──────────┬──────────────────────┐
│ Provider    │ Status   │ Model                │
├─────────────┼──────────┼──────────────────────┤
│ anthropic   │ ✓ active │ claude-sonnet-4-5    │
│ openai      │ ○ setup  │ -                    │
│ deepseek    │ ○ setup  │ -                    │
└─────────────┴──────────┴──────────────────────┘
```

**Features:**
- Box-drawing characters for clean borders
- Aligned columns
- Responsive width (fits terminal)

### Blockquotes
```
│ This is a quote from the documentation.
│ It spans multiple lines and is visually
│ distinct from regular text.
```

---

## Progress Indicators

### Thinking/Processing
```
⣾ thinking...
⣽ processing...
⣻ analyzing...
⢿ searching...
```

**Braille spinners** (8 frames):
- Minimal visual footprint
- Professional appearance
- Fast animation (100ms per frame)
- Automatically stops when response starts

### Long Operations
```
Setting up workspace...
  ✓ Created config directory
  ✓ Initialized templates
  ⣾ Installing skills...
```

- Checkmarks (✓) for completed steps
- Spinner for current step
- Clear progress through list

### Download Progress
```
Downloading model...
[████████████░░░░░░░░] 65% (1.2GB / 1.8GB) 12.5 MB/s
```

- Simple progress bar
- Percentage and absolute values
- Transfer speed

---

## Plain Text Fallback Mode

When `--no-color` flag is used or `NO_COLOR` environment variable is set:

```
klyntbot v0.1.0
================================================

Provider: anthropic/claude-sonnet-4-5
Workspace: ~/.klyntbot/workspace

Channels:
- telegram: connected (@mybot)
- discord: connected (MyBot#1234)
- slack: disabled
- whatsapp: disabled

Services:
- gateway: running (PID 12345)
- cron: 3 jobs scheduled
- heartbeat: active (next in 14m)
```

**Features:**
- All formatting stripped
- ASCII borders instead of box-drawing
- Symbols replaced with text (✓ → OK, ○ → disabled)
- Still readable and well-structured
- Screen reader friendly

---

## Accessibility Features

### Color Blindness Support
- All status indicators work without color:
  - `✓` = success (green)
  - `○` = disabled (gray)
  - `✗` = error (red)
  - `!` = warning (yellow)
- Symbols carry meaning independent of color
- High contrast ratios (WCAG AAA compliant)

### Screen Reader Support
- Plain text fallback available via `--no-color`
- No Unicode decoration in fallback mode
- Semantic structure (headings, lists, tables)
- Clear labels for all interactive elements

### Keyboard Navigation
- All features accessible via keyboard
- No mouse required
- Vi-style keybindings optional (j/k for history)
- Clear shortcuts displayed in help

---

## Performance UX

### Startup Time
- Target: <50ms to first prompt in interactive mode
- Config loading: async, parallel
- Skills: lazy-loaded on demand
- No blocking operations on startup

### First Response
- Character-by-character streaming from LLM
- First character visible <500ms after LLM starts
- No buffering or waiting for complete response
- Instant feedback that system is working

### Tool Execution
- Show tool calls immediately as they execute
- Don't wait for completion to show next tool
- Parallel tool execution where possible
- Cancel support for long-running operations

### Memory Footprint
- Target: <50MB RAM for idle process
- Lazy loading of heavy dependencies
- Efficient session management
- Auto-cleanup of old data

---

## Configuration UX

### Show Configuration
```bash
$ klyntbot config show

Provider:
  anthropic/claude-sonnet-4-20250514

Channels:
  telegram: enabled
  discord: enabled
  slack: disabled

Tools:
  web_search: enabled (Brave API)
  exec: enabled (timeout: 60s)
  restrict_to_workspace: false

Workspace:
  ~/.klyntbot/workspace
```

### Get/Set Values
```bash
$ klyntbot config get agents.defaults.model
claude-sonnet-4-20250514

$ klyntbot config set agents.defaults.model claude-opus-4-5
✓ Updated agents.defaults.model

$ klyntbot config set channels.telegram.enabled true
✓ Updated channels.telegram.enabled

$ klyntbot config set channels.telegram.token "123:ABC..."
✓ Updated channels.telegram.token
```

### Validate Configuration
```bash
$ klyntbot config validate

Checking configuration...

✓ Config file exists and is valid JSON
✓ All required fields present
✓ Provider configuration valid
✗ Channel configuration has errors:
  - telegram.token: Invalid format (must start with digits)
! Warning: No API key configured for backup provider

1 error, 1 warning

Fix errors:
  klyntbot config set channels.telegram.token VALID_TOKEN
```

### Edit Interactively
```bash
$ klyntbot config edit

# Opens ~/.klyntbot/config.json in $EDITOR
# After saving:

✓ Configuration updated
  Validating...
✓ Configuration is valid

Restart required for changes to take effect:
  klyntbot serve
```

---

## Skills Management UX

### List Skills
```bash
$ klyntbot skills list

Available Skills (12):
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
github                    ✓ GitHub repository operations
weather                   ✓ Weather forecasts
summarize                 ✓ Summarize long documents
cron                      ✓ Manage scheduled tasks
tmux                      ✗ tmux session management
                            Missing: tmux binary
docker                    ✗ Docker container ops
                            Missing: docker binary

Skills Directory:
  ~/.klyntbot/workspace/skills

Add custom skills:
  klyntbot skills info custom-skill-template
```

### Skill Details
```bash
$ klyntbot skills info github

Skill: github
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Description:
  GitHub repository operations including searching code,
  reading files, creating issues, and more.

Status: ✓ Available

Requirements:
  - GITHUB_TOKEN environment variable

Location:
  ~/.klyntbot/workspace/skills/github/SKILL.md

Usage:
  This skill is automatically loaded when available.
  Set GITHUB_TOKEN to enable:
    export GITHUB_TOKEN=ghp_your_token_here

Documentation:
  https://klyntbot.dev/docs/skills/github
```

---

## Logging and Diagnostics

### View Logs
```bash
$ klyntbot logs

2026-02-11 18:30:15 [INFO] Gateway started on port 18790
2026-02-11 18:30:16 [INFO] Telegram channel connected (@mybot)
2026-02-11 18:30:16 [INFO] Discord channel connected (MyBot#1234)
2026-02-11 18:30:17 [INFO] Cron service started (3 jobs)
2026-02-11 18:30:45 [INFO] Message received from telegram:123456789
2026-02-11 18:30:46 [INFO] Agent loop started (session: telegram:123456789)
2026-02-11 18:30:47 [DEBUG] Tool call: web_search("rust async")
2026-02-11 18:30:48 [INFO] Response sent (278 chars)
```

### Tail Logs
```bash
$ klyntbot logs --tail 20 --follow

[Following logs, Ctrl+C to exit]
2026-02-11 18:45:23 [INFO] Heartbeat check
2026-02-11 18:45:24 [INFO] Memory updated
...
```

### System Check
```bash
$ klyntbot doctor

Checking system health...

✓ Configuration file exists and is valid
✓ Workspace directory accessible
✓ API key configured (anthropic)
✓ Network connectivity
✓ Disk space sufficient (45GB available)
! Warning: No backup provider configured
✗ Error: tmux binary not found (required for tmux skill)

Recommendations:
  1. Configure a backup provider:
     klyntbot config set providers.openai.api_key YOUR_KEY

  2. Install tmux to enable tmux skill:
     brew install tmux  # macOS
     apt install tmux   # Linux

Overall health: Good (1 warning, 1 error)
```

---

## Help and Documentation

### Main Help
```bash
$ klyntbot --help

klyntbot v0.1.0
A fast, lightweight AI assistant for your terminal

USAGE:
  klyntbot <COMMAND>

COMMANDS:
  chat        Start interactive chat or send a single message
  serve       Start the gateway daemon (enables channels)
  init        Run the setup wizard
  status      Show system status
  config      Manage configuration
  channels    Manage chat channels
  cron        Manage scheduled tasks
  skills      Manage skills
  workspace   Manage workspace
  logs        View logs
  doctor      Check system health
  version     Show version information
  help        Show this help message

OPTIONS:
  -h, --help     Show help
  -v, --version  Show version
  --no-color     Disable colored output

EXAMPLES:
  klyntbot chat
  klyntbot chat "What's the weather?"
  klyntbot serve
  klyntbot status

Get help for a command:
  klyntbot <command> --help

Documentation:
  https://klyntbot.dev/docs

Report bugs:
  https://github.com/youruser/klyntbot/issues
```

### Command-specific Help
```bash
$ klyntbot chat --help

Start interactive chat or send a single message

USAGE:
  klyntbot chat [MESSAGE] [OPTIONS]

ARGUMENTS:
  [MESSAGE]  Optional message to send (single message mode)

OPTIONS:
  -s, --session <ID>     Session ID [default: cli:default]
  --no-markdown          Disable markdown rendering
  --no-stream            Wait for complete response before displaying
  --log-level <LEVEL>    Log level [default: info]
  -h, --help             Show help

INTERACTIVE MODE:
  If no message is provided, starts an interactive REPL.

  Commands:
    exit, quit     Exit the chat
    /clear         Clear conversation context
    /paste         Enter multi-line mode
    /help          Show help
    /history       Show command history

EXAMPLES:
  # Interactive mode
  klyntbot chat

  # Single message
  klyntbot chat "What is 2+2?"

  # Custom session
  klyntbot chat --session work:project-alpha
```

---

## Channel-Specific UX

### WhatsApp QR Login
```bash
$ klyntbot channels login whatsapp

WhatsApp Bridge Setup
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Starting bridge server...
  ✓ Bridge server started on ws://localhost:3001

Scan this QR code with WhatsApp:
  WhatsApp → Settings → Linked Devices → Link a Device

   ██████████████  ██████  ██  ██████████████
   ██          ██      ████████  ██          ██
   ██  ██████  ██  ██████  ██    ██  ██████  ██
   ██  ██████  ██  ██        ██  ██  ██████  ██
   ██  ██████  ██  ████████  ██  ██  ██████  ██
   ██          ██  ██  ██        ██          ██
   ██████████████  ██  ██  ██  ██████████████
                   ██████  ██
   ██████    ██████  ██  ████  ██  ██    ██
   ██    ████  ████      ██████      ██  ██  ██
   ████  ████    ██████    ██████████████  ██
   ██  ██████  ████  ██  ██  ██  ██████████
   ████  ██████  ██████  ██████████  ██  ████
                   ████  ██  ██████      ██████
   ██████████████  ██  ██  ██████  ██      ██
   ██          ██    ██  ████    ██████  ██  ██
   ██  ██████  ██  ██    ████      ████  ██████
   ██  ██████  ██      ██  ██████  ██████
   ██  ██████  ██  ████████  ████  ██████  ██
   ██          ██  ██  ██  ██████  ██    ██
   ██████████████  ██  ████  ██████    ██████

Waiting for scan...

  ✓ Connected! Device: iPhone (iOS 17.2)

WhatsApp is now connected.

Next steps:
  1. Enable WhatsApp channel:
     klyntbot config set channels.whatsapp.enabled true

  2. Restart the gateway:
     klyntbot serve

The bridge must stay running for WhatsApp to work.
To run it in the background:
  klyntbot channels login whatsapp --daemon
```

### Telegram Setup
```bash
$ klyntbot channels login telegram

Telegram Bot Setup
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Steps to create a Telegram bot:
  1. Open Telegram and search for @BotFather
  2. Send /newbot and follow the prompts
  3. Copy the bot token (format: 123456:ABC-DEF...)

[?] Bot Token: ▏

[Enter token]

  ✓ Token validated
  ✓ Bot info: @my_assistant_bot

[?] Allow all users? [Y/n]: ▏

[If no]
  [?] Allowed user IDs (comma-separated): ▏

  ✓ Configuration saved

Telegram is now configured.

Start the gateway to begin receiving messages:
  klyntbot serve

Find your user ID:
  Send a message to @my_assistant_bot
  Check logs: klyntbot logs
```

---

## Implementation Notes

### Terminal Detection
- Detect TTY: `isatty(fd)`
- Detect color support: Check `TERM`, `COLORTERM` environment variables
- Fallback chain: truecolor → 256color → 16color → no color
- Honor `NO_COLOR` standard

### Box Drawing Characters
- Use Unicode box-drawing: `┌─┐│└┘├┤┬┴┼`
- Fallback to ASCII: `+-+|++++` in plain text mode
- Test rendering before committing to design

### Spinner Implementation
- Braille patterns: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`
- Update every 80-100ms
- Clear completely when done (no artifacts)
- Position at start of line for clean transitions

### Input Handling
Use `crossterm` or `rustyline` for:
- Line editing
- History management
- Multi-line support
- Keyboard shortcuts

### Markdown Rendering
Use `termimad` or similar for:
- Syntax highlighting in code blocks
- Box-drawing for tables
- Link formatting
- Text styling

---

## Success Metrics

A successful CLI UX implementation should achieve:

1. **Speed**: <100ms startup, <500ms first LLM token
2. **Clarity**: User doesn't need to read docs for basic tasks
3. **Consistency**: Same patterns across all commands
4. **Accessibility**: Works without color, readable by screen readers
5. **Professional**: Looks polished, not gimmicky
6. **Delightful**: Small touches make it pleasant to use

---

## Future Enhancements

Consider for future versions:

- **Themes**: User-customizable color schemes
- **Vi mode**: Vi-style keybindings for power users
- **Shell integration**: Completion scripts for bash/zsh/fish
- **TUI mode**: Full-screen interface with panels
- **Voice input**: Speech-to-text for hands-free operation
- **Desktop notifications**: System notifications for background events
- **Rich media**: Image display in terminal (using protocols like iTerm2's or kitty's)

---

## Comparison with nanobot

| Aspect | nanobot | klyntbot | Improvement |
|--------|---------|----------|-------------|
| Main command | `nanobot agent` | `klyntbot chat` | More intuitive |
| Daemon | `nanobot gateway` | `klyntbot serve` | Clearer purpose |
| Setup | `nanobot onboard` | `klyntbot init` | Industry standard |
| Config | Edit file manually | `klyntbot config set` | User-friendly |
| Skills | No management | `klyntbot skills list/info` | Discoverable |
| Errors | Generic | Context + fix instructions | Actionable |
| Status | Basic | Rich, hierarchical | Informative |
| Startup | ~200ms | <50ms target | 4x faster |
| Colors | Abundant | Minimal, purposeful | Professional |
| Markdown | Via rich library | Optimized for terminal | Faster |

---

## Design Philosophy

> "Perfection is achieved, not when there is nothing more to add, but when there is nothing left to take away." — Antoine de Saint-Exupéry

klyntbot's CLI should feel:
- **Fast**: No waiting, instant feedback
- **Obvious**: Don't make users think
- **Quiet**: Only speak when necessary
- **Polished**: Attention to detail everywhere
- **Trustworthy**: Reliable and consistent

Every interaction should respect the user's time and intelligence.
