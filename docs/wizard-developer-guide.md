# Wizard Developer Guide

Technical documentation for developers extending or maintaining the klyntbot setup wizard.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Module Structure](#module-structure)
- [Core Framework](#core-framework)
  - [WizardModule Trait](#wizardmodule-trait)
  - [StepResult Enum](#stepresult-enum)
  - [WizardState](#wizardstate)
  - [Navigation Engine](#navigation-engine)
- [UI Components](#ui-components)
- [How to Add a New Wizard Step](#how-to-add-a-new-wizard-step)
- [How to Add a New Provider](#how-to-add-a-new-provider)
- [How to Add a New Channel](#how-to-add-a-new-channel)
- [OAuth Integration](#oauth-integration)
- [Daemon Setup](#daemon-setup)
- [Config System Integration](#config-system-integration)
- [Testing](#testing)
- [Implementation Roadmap](#implementation-roadmap)

---

## Architecture Overview

The wizard lives in the `cli` crate (Layer 6) as a `wizard/` module directory. It interacts with `config` (Layer 1) for persistence, `common` (Layer 0) for terminal UI, and optionally uses an embedded `axum` HTTP server for OAuth callback flows.

```
┌─────────────────────────────────────────────────────────────────┐
│  crates/cli/src/wizard/                                         │
│  ┌──────────────┐  ┌───────────────────────────────────────┐    │
│  │ mod.rs       │  │ steps/                                │    │
│  │  run_wizard()│  │  provider.rs  model.rs  channels.rs   │    │
│  │  navigation  │  │  workspace.rs  daemon.rs              │    │
│  │  engine      │  │                                       │    │
│  └──────┬───────┘  └────────────────┬──────────────────────┘    │
│         │                           │                           │
│  ┌──────┴───────┐  ┌───────────────┴──────────────────────┐    │
│  │ state.rs     │  │ channel_flows/                        │    │
│  │ WizardState  │  │  telegram.rs  discord.rs  slack.rs    │    │
│  │ checkpoint   │  │  email.rs  generic.rs                 │    │
│  └──────┬───────┘  └────────────────┬──────────────────────┘    │
│         │                           │                           │
│  ┌──────┴───────┐  ┌───────────────┴──────────────────────┐    │
│  │module_trait.rs│  │ oauth.rs          daemon/             │    │
│  │ WizardModule │  │  OAuthServer       systemd.rs         │    │
│  │ StepResult   │  │  callback          launchd.rs         │    │
│  └──────────────┘  └──────────────────────────────────────┘    │
│         │                           │                           │
│         ▼                           ▼                           │
│  ┌───────────────┐  ┌──────────────────────────┐               │
│  │ config::save  │  │ common::utils::terminal  │               │
│  │ config::Config│  │ (colors, boxes, spinners) │               │
│  └───────────────┘  └──────────────────────────┘               │
└─────────────────────────────────────────────────────────────────┘
```

Dependencies flow strictly upward per the workspace layout:

- **Layer 0**: `common` - Terminal utilities (`colorize`, `draw_box`, `Spinner`, status indicators)
- **Layer 1**: `config` - `Config` struct, `save()`, `config_path()`, `config_dir()`
- **Layer 6**: `cli` - Wizard implementation, with optional `axum` dependency for OAuth

### Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| New crate? | No, extend `cli` | Wizard is CLI-only; no other crate needs it |
| HTTP server | `axum` (feature-gated) | Tokio-native, minimal, same ecosystem |
| State persistence | JSON checkpoint file | Simple, human-readable, survives crashes |
| Navigation | Index-based with `StepResult` enum | Supports back/next/skip without complex state machine |
| OAuth port | `127.0.0.1:17891` | Localhost-only, arbitrary high port |
| Daemon type | User-level service (no sudo) | Safer, doesn't need root |
| Token storage | `Secret<String>` in `config.json` | Matches existing pattern |
| Channel flows | Separate files per channel | Clean separation, easy to add new channels |
| Error handling | `common::Result<T>` public API | Consistent with rest of codebase |

---

## Module Structure

```
crates/cli/src/
├── wizard/
│   ├── mod.rs              # Public API: run_wizard(), phase sequencing
│   ├── state.rs            # WizardState, checkpoint persistence, navigation
│   ├── module_trait.rs     # WizardModule trait definition, StepResult enum
│   ├── components.rs       # Reusable UI components (select, multi-select, progress)
│   ├── steps/
│   │   ├── mod.rs          # Re-exports all step modules
│   │   ├── provider.rs     # Phase 2: LLM provider selection + API key
│   │   ├── model.rs        # Phase 2 sub-step: Model selection + validation
│   │   ├── channels.rs     # Phase 3: Channel multi-select + per-channel flows
│   │   ├── tools.rs        # Phase 4: Tools & permissions configuration
│   │   ├── workspace.rs    # Workspace creation + template files
│   │   └── daemon.rs       # Phase 5: Service installation (systemd/launchd)
│   ├── validation.rs       # Phase 6: Connection testing and validation runner
│   ├── oauth.rs            # Embedded axum HTTP server for OAuth callbacks
│   ├── channel_flows/
│   │   ├── mod.rs          # Re-exports all channel flow modules
│   │   ├── telegram.rs     # Telegram BotFather guided setup
│   │   ├── discord.rs      # Discord Developer Portal guided setup
│   │   ├── slack.rs        # Slack manifest + Socket Mode token setup
│   │   ├── email.rs        # IMAP/SMTP credential wizard with provider presets
│   │   └── generic.rs      # Generic token-paste flow (QQ, Feishu, DingTalk, WhatsApp)
│   └── daemon/
│       ├── mod.rs          # DaemonType detection, install dispatch
│       ├── systemd.rs      # Linux systemd user unit generation + install
│       └── launchd.rs      # macOS launchd plist generation + install
├── lib.rs                  # CLI crate root, module declarations
├── commands.rs             # Clap command definitions (Commands::Init triggers wizard)
├── channels.rs             # Channel management commands
├── chat.rs                 # Chat/REPL commands
├── config_cmd.rs           # Config subcommands
├── cron.rs                 # Cron job management
├── interactive.rs          # REPL/interactive mode
├── mod.rs                  # Module re-exports
├── serve.rs                # Gateway daemon
├── skills.rs               # Skill management
└── status.rs               # Status display
```

The wizard is invoked from the CLI dispatch when `Commands::Init` is matched. The old monolithic `wizard.rs` file is replaced by the `wizard/` module directory.

---

## Core Framework

### WizardModule Trait

Every wizard step implements the `WizardModule` trait, defined in `wizard/module_trait.rs`:

```rust
use async_trait::async_trait;
use common::Result;
use super::state::WizardState;

/// A single step in the wizard pipeline
#[async_trait]
pub trait WizardModule: Send + Sync {
    /// Display name for the step header (e.g., "LLM Provider")
    fn name(&self) -> &str;

    /// Short description shown under the header
    fn description(&self) -> &str;

    /// Whether this step can be skipped
    fn is_optional(&self) -> bool { false }

    /// Whether this step should be shown based on current state
    /// (e.g., daemon step only on Linux/macOS, not in containers)
    fn should_show(&self, state: &WizardState) -> bool { true }

    /// Execute the step. Reads user input, modifies state.
    /// Returns Ok(StepResult) to control navigation.
    async fn execute(&self, state: &mut WizardState) -> Result<StepResult>;

    /// Validate the state after execution. Called before advancing.
    /// Return Ok(()) if valid, Err with user-facing message if not.
    fn validate(&self, state: &WizardState) -> Result<()> { Ok(()) }

    /// Undo any side effects from execute (for back-navigation)
    async fn rollback(&self, state: &mut WizardState) -> Result<()> { Ok(()) }
}
```

Trait method responsibilities:

| Method | Required | Purpose |
|--------|----------|---------|
| `name()` | Yes | Step header text: "Step N of M: {name}" |
| `description()` | Yes | Subtitle text in DIM below the header |
| `is_optional()` | No | If `true`, the step can be skipped via `StepResult::Skip` |
| `should_show()` | No | Dynamic filtering (e.g., hide daemon step in Docker) |
| `execute()` | Yes | Main logic: render UI, read input, modify `WizardState` |
| `validate()` | No | Post-execution check before advancing to next step |
| `rollback()` | No | Undo side effects when user navigates back |

### StepResult Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    Next,       // Advance to next step
    Back,       // Go back to previous step
    Skip,       // Skip this step (only if is_optional)
    Restart,    // Restart wizard from beginning
    Quit,       // Quit and save checkpoint
}
```

The navigation engine in `mod.rs` interprets each `StepResult` to control wizard flow:

- **Next**: Calls `validate()`, saves checkpoint, increments step index
- **Back**: Calls `rollback()` on the previous step, decrements step index
- **Skip**: Only accepted if `is_optional()` returns `true`; increments step index
- **Restart**: Clears checkpoint, resets to `WizardState::new()`
- **Quit**: Saves checkpoint, prints resume instructions, exits

### WizardState

Defined in `wizard/state.rs`, `WizardState` is the persistent data structure that flows through all wizard steps:

```rust
use config::Config;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Persistent wizard state — survives crashes via checkpoint file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardState {
    /// The config being built
    pub config: Config,

    /// Current step index (0-based)
    pub current_step: usize,

    /// Which channels the user wants to configure
    pub selected_channels: Vec<String>,

    /// OAuth tokens received during flows (transient, not serialized)
    #[serde(skip)]
    pub pending_oauth_tokens: HashMap<String, String>,

    /// Whether to install as daemon
    pub install_daemon: bool,

    /// Target daemon type (detected from OS)
    pub daemon_type: Option<DaemonType>,

    /// Metadata for each completed step (for display/summary)
    pub step_metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonType {
    Systemd,
    Launchd,
}
```

Key methods:

```rust
impl WizardState {
    /// Create new state with defaults and auto-detected daemon type
    pub fn new() -> Self;

    /// Save checkpoint to ~/.klyntbot/.wizard-state.json
    pub fn save_checkpoint(&self) -> Result<()>;

    /// Load checkpoint if it exists
    pub fn load_checkpoint() -> Option<Self>;

    /// Remove checkpoint file (called on successful completion)
    pub fn clear_checkpoint() -> Result<()>;
}
```

The checkpoint file (`~/.klyntbot/.wizard-state.json`) is a full JSON serialization of `WizardState`. It is:
- Written after each successful step advancement
- Loaded at wizard start to offer resume
- Deleted when the wizard completes all steps successfully

The `pending_oauth_tokens` field is marked `#[serde(skip)]` because OAuth tokens are transient — they're received during a session and immediately moved into `config` fields.

### Navigation Engine

The main wizard loop in `wizard/mod.rs`:

```rust
pub async fn run_wizard() -> Result<()> {
    // Check for existing checkpoint
    let mut state = if let Some(checkpoint) = WizardState::load_checkpoint() {
        if prompt_resume_checkpoint()? {
            checkpoint
        } else {
            WizardState::clear_checkpoint()?;
            WizardState::new()
        }
    } else {
        WizardState::new()
    };

    // Build step pipeline
    let steps: Vec<Box<dyn WizardModule>> = vec![
        Box::new(ProviderStep::new()),
        Box::new(ModelStep::new()),
        Box::new(ChannelStep::new()),
        Box::new(ToolsStep::new()),
        Box::new(WorkspaceStep::new()),
        Box::new(DaemonStep::new()),
        Box::new(ValidationStep::new()),
    ];

    // Filter to applicable steps
    let applicable: Vec<_> = steps.iter()
        .filter(|s| s.should_show(&state))
        .collect();
    let total = applicable.len();

    // Navigation loop
    while state.current_step < total {
        let step = &applicable[state.current_step];

        print_step_header(
            state.current_step + 1, total,
            step.name(), step.description(),
        );

        match step.execute(&mut state).await? {
            StepResult::Next => {
                if let Err(e) = step.validate(&state) {
                    println!("{}", colorize(&format!("Validation: {}", e), ERROR));
                    continue; // Re-run current step
                }
                state.current_step += 1;
                state.save_checkpoint()?;
            }
            StepResult::Back => {
                if state.current_step > 0 {
                    applicable[state.current_step - 1]
                        .rollback(&mut state).await?;
                    state.current_step -= 1;
                }
            }
            StepResult::Skip => {
                if step.is_optional() {
                    state.current_step += 1;
                    state.save_checkpoint()?;
                }
            }
            StepResult::Quit => {
                state.save_checkpoint()?;
                println!("Progress saved. Run 'klyntbot init' to resume.");
                return Ok(());
            }
            StepResult::Restart => {
                WizardState::clear_checkpoint()?;
                state = WizardState::new();
            }
        }
    }

    // All steps complete — save final config
    config::save(&state.config)?;
    WizardState::clear_checkpoint()?;
    print_completion(&state.config);
    Ok(())
}
```

The navigation engine handles all step transitions. Individual steps only need to return the appropriate `StepResult` — they don't manage their own indexing or persistence.

---

## UI Components

### Existing Terminal Utilities (from `common::utils::terminal`)

| Component       | Function                                   | Use Case                    |
|----------------|--------------------------------------------|-----------------------------|
| `draw_box()`    | `draw_box(content, header)`                | Welcome/completion screens  |
| `draw_table()`  | `draw_table(headers, rows)`                | Data display (channel summary) |
| `draw_code_block()` | `draw_code_block(code, lang)`          | Code snippets (Slack manifest) |
| `colorize()`    | `colorize(text, ANSI_CODE)`                | Colored text                |
| `Spinner`       | `Spinner::new(msg).start()/.stop()`        | Loading indicators          |
| `display_error()`| `display_error(title, problem, fixes, docs)`| Structured errors         |
| `MarkdownRenderer` | `MarkdownRenderer::render(md)`          | Markdown to terminal        |

### New UI Components (in `wizard/components.rs`)

These are wizard-specific reusable components:

```rust
// New functions needed:
pub fn draw_progress_bar(current: usize, total: usize, labels: &[&str]) -> String;
pub fn draw_single_select(items: &[SelectItem], selected: usize) -> String;
pub fn draw_multi_select(items: &[MultiSelectItem]) -> String;
pub fn draw_separator_double() -> String;  // ═══ style
pub fn draw_confirmation_card(title: &str, fields: &[(&str, &str)]) -> String;
pub fn read_password(prompt: &str) -> Result<String>;  // masked input
pub fn read_single_select(items: &[SelectItem]) -> Result<usize>;
pub fn read_multi_select(items: &[MultiSelectItem]) -> Result<Vec<usize>>;
pub fn terminal_width() -> usize;  // detect terminal width, default 80
```

### Progress Bar

Appears at the top of every step:

```
  ● Provider  ─  ○ Channels  ─  ○ Tools  ─  ○ Daemon  ─  ○ Validate  ─  ○ Done
  ━━━━━━━━━━━━━━━━━━━━                                               Step 2 of 7
```

- Completed steps: `●` in SUCCESS green
- Current step: `●` in TOOL cyan (bold)
- Future steps: `○` in DIM
- Bar uses `━` for completed, spaces for remaining
- Step counter right-aligned in DIM

### Single-Select List

```
  Select your LLM provider:

    ● Anthropic (Claude)       Recommended for best quality
      OpenAI (GPT)             Industry standard models
      DeepSeek                 Cost-effective alternative

  ↑/↓ navigate  ·  Enter select  ·  ? help
```

Uses arrow keys with `●` cursor. Falls back to numbered input for non-TTY.

### Multi-Select List

```
  Select channels to enable:

    [✓] Telegram           Simple setup: just a bot token
    [ ] Discord            Bot token + gateway connection

  ↑/↓ navigate  ·  Space toggle  ·  Enter confirm  ·  a all  ·  n none
```

Toggle with Space, `a` for all, `n` for none. Falls back to comma-separated numbers.

### Secure Input

```
  API Key: ●●●●●●●●●●●●●●●●sk-ant...3kf
```

Characters masked with `●`, last 6 shown for verification.

### Confirmation Card

```
  ┌─ Provider Configured ───────────────────────────────────────┐
  │                                                              │
  │  Provider:  Anthropic                                        │
  │  Model:     claude-sonnet-4-5                                │
  │  API Key:   sk-ant-●●●●●●●3kf                               │
  │                                                              │
  │  ✓ Configuration saved                                       │
  └──────────────────────────────────────────────────────────────┘
```

Uses `draw_box()` with a header and SUCCESS green checkmark.

### Color Constants

| Constant     | ANSI Code       | Usage                                    |
|-------------|-----------------|------------------------------------------|
| `RESET`      | `\x1b[0m`      | Reset all formatting                     |
| `BOLD`       | `\x1b[1m`      | Step titles, section headers             |
| `TOOL`       | `\x1b[36m`     | Selected values, highlighted items       |
| `SUCCESS`    | `\x1b[32m`     | Checkmarks, confirmed values             |
| `ERROR`      | `\x1b[31m`     | Error messages, failed validations       |
| `WARNING`    | `\x1b[33m`     | Caution notes, skipped steps             |
| `DIM`        | `\x1b[90m`     | Help text, defaults, descriptions        |
| `PROMPT`     | `\x1b[2;34m`   | Input prompts, navigation hints          |
| `UNDERLINE`  | `\x1b[4m`      | URLs, clickable references               |
| `SEPARATOR`  | `\x1b[2;90m`   | Box borders, horizontal rules            |

New constants for enhanced wizard:

```rust
pub const HIGHLIGHT: &str = "\x1b[36;1m";   // Bold cyan - selected item in lists
pub const BG_SELECT: &str = "\x1b[46;30m";  // Cyan bg, black text - cursor item
pub const BRAND: &str = "\x1b[35m";         // Magenta - klyntbot branding
```

### NO_COLOR and Accessibility

All components respect `NO_COLOR` and non-TTY environments:

- ANSI codes suppressed, Unicode falls back to ASCII
- Lists use numbered input: `[1] Anthropic (Claude) - Recommended`
- Progress bar becomes text: `[Step 2 of 7: Provider Setup]`
- Checkmarks become `[OK]`, errors `[FAIL]`, warnings `[WARN]`
- No cursor movement codes (screen reader compatible)
- All content is linear (no columns)

---

## How to Add a New Wizard Step

1. **Create the step file** in `wizard/steps/`:

```rust
// wizard/steps/my_step.rs
use async_trait::async_trait;
use common::Result;
use super::super::{WizardModule, StepResult, WizardState};

pub struct MyStep;

impl MyStep {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl WizardModule for MyStep {
    fn name(&self) -> &str { "My New Feature" }

    fn description(&self) -> &str { "Configure the new feature" }

    fn is_optional(&self) -> bool { true }

    fn should_show(&self, _state: &WizardState) -> bool {
        // Return false to hide this step based on state/platform
        true
    }

    async fn execute(&self, state: &mut WizardState) -> Result<StepResult> {
        // Your step logic here:
        // 1. Render UI using components from wizard/components.rs
        // 2. Read user input
        // 3. Modify state.config as needed
        // 4. Return StepResult::Next, Back, Skip, etc.

        Ok(StepResult::Next)
    }

    fn validate(&self, state: &WizardState) -> Result<()> {
        // Optional: validate the state after execution
        Ok(())
    }

    async fn rollback(&self, state: &mut WizardState) -> Result<()> {
        // Optional: undo side effects for back-navigation
        Ok(())
    }
}
```

2. **Register in `wizard/steps/mod.rs`**:

```rust
mod my_step;
pub use my_step::MyStep;
```

3. **Add to the step pipeline** in `wizard/mod.rs`:

```rust
let steps: Vec<Box<dyn WizardModule>> = vec![
    // ... existing steps ...
    Box::new(MyStep::new()),
    // ... remaining steps ...
];
```

4. **Update the progress bar labels** in `wizard/mod.rs` to include the new step name.

5. **Add tests** for the new step's logic (see [Testing](#testing)).

---

## How to Add a New Provider

1. **Add to the provider list** in `wizard/steps/provider.rs`:

```rust
const PROVIDERS: &[ProviderOption] = &[
    // ... existing providers ...
    ProviderOption {
        name: "NewProvider (ModelName)",
        key: "newprovider",
        description: "Brief description",
        api_url: "https://newprovider.com/api-keys",
        key_prefix: Some("np-"),  // For format validation
    },
];
```

2. **Add default model mapping** in `wizard/steps/model.rs`:

```rust
let default_model = match provider_key {
    // ... existing providers ...
    "newprovider" => "newprovider-default-model",
    _ => "claude-sonnet-4-5",
};
```

3. **Add validation rule** in `wizard/components.rs` or `validation.rs`:

```rust
// Key format validation
("newprovider", key) => key.starts_with("np-") && key.len() >= 20,
```

4. **Add config support** in `crates/config/src/schema.rs`:

```rust
// Add field to ProvidersConfig (if it doesn't already exist)
pub struct ProvidersConfig {
    // ... existing ...
    #[serde(default)]
    pub newprovider: ProviderConfig,
}
```

5. **Add `set_provider_key` match arm** in `Config`:

```rust
pub fn set_provider_key(&mut self, provider_name: &str, key: String) {
    match provider_name {
        // ... existing ...
        "newprovider" => self.providers.newprovider.api_key = Secret::new(key),
        _ => {}
    }
}
```

6. **Add environment variable support** in `loader.rs`:

```rust
if let Ok(key) = std::env::var("KLYNTBOT_PROVIDERS__NEWPROVIDER__API_KEY") {
    config.providers.newprovider.api_key = Secret::new(key);
}
```

---

## How to Add a New Channel

1. **Create a channel flow** in `wizard/channel_flows/`:

```rust
// wizard/channel_flows/newchannel.rs
use common::Result;
use config::Config;

pub async fn configure_newchannel(config: &mut Config) -> Result<()> {
    // 1. Print setup instructions
    println!("  --- NewChannel Setup ---");
    println!();
    println!("  1. Go to https://newchannel.example.com/developers");
    println!("  2. Create a bot application");
    println!("  3. Copy the bot token");

    // 2. Read credentials with validation
    let token = read_password("  Bot Token: ")?;
    validate_token_format(&token)?;

    // 3. Configure access control
    let allow_from = configure_allowlist()?;

    // 4. Save to config
    config.channels.newchannel.enabled = true;
    config.channels.newchannel.token = Secret::new(token);
    config.channels.newchannel.allow_from = allow_from;

    // 5. Show confirmation card
    draw_confirmation_card("NewChannel Configured", &[
        ("Token", &mask_secret(&token)),
        ("Access", &format!("{} users", allow_from.len())),
        ("Status", "Enabled"),
    ]);

    Ok(())
}
```

2. **Register in `wizard/channel_flows/mod.rs`**:

```rust
mod newchannel;
pub use newchannel::configure_newchannel;
```

3. **Add to the channel selection list** in `wizard/steps/channels.rs`:

```rust
const CHANNELS: &[ChannelOption] = &[
    // ... existing channels ...
    ChannelOption {
        name: "NewChannel",
        key: "newchannel",
        description: "Brief description of requirements",
    },
];
```

4. **Add dispatch** in the channel step's execution loop:

```rust
match channel.key {
    // ... existing ...
    "newchannel" => channel_flows::configure_newchannel(&mut state.config).await?,
    _ => channel_flows::generic::configure_generic(channel, &mut state.config).await?,
}
```

5. **Define the config struct** in `crates/config/src/schema.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewChannelConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub token: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,
}
```

6. **Add to `ChannelsConfig`**:

```rust
pub struct ChannelsConfig {
    // ... existing ...
    #[serde(default)]
    pub newchannel: NewChannelConfig,
}
```

7. **Implement the `Channel` trait** in `crates/channels/src/`:
   - `async fn start()` - Connect to the platform
   - `async fn stop()` - Disconnect gracefully
   - `async fn send()` - Send a message
   - `fn name()` - Return channel name
   - `fn is_allowed()` - Check allowlist

8. **Add validation** in `wizard/validation.rs` for connection testing.

---

## OAuth Integration

### Embedded HTTP Server

The OAuth callback server is in `wizard/oauth.rs`. It uses `axum` (feature-gated behind the `wizard` feature flag) to run a lightweight HTTP server on localhost.

```rust
// wizard/oauth.rs
use std::sync::Arc;
use tokio::sync::oneshot;

pub struct OAuthResult {
    pub code: String,
    pub state: String,
}

pub struct OAuthServer;

impl OAuthServer {
    /// Start server on localhost, wait for callback, return auth code
    pub async fn wait_for_callback(port: u16) -> Result<OAuthResult> {
        let (result_tx, result_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let result_tx = Arc::new(tokio::sync::Mutex::new(Some(result_tx)));

        let app = axum::Router::new()
            .route("/callback", axum::routing::get(move |params| {
                handle_callback(params, result_tx.clone())
            }));

        let listener = tokio::net::TcpListener::bind(
            format!("127.0.0.1:{}", port)
        ).await?;

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async { shutdown_rx.await.ok(); })
                .await
                .ok();
        });

        // Wait for the callback (with 5-minute timeout)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            result_rx,
        ).await??;

        let _ = shutdown_tx.send(());
        Ok(result)
    }
}
```

### Browser Launch

Cross-platform browser opening utility:

```rust
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;

    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;

    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/c", "start", url]).spawn()?;

    Ok(())
}
```

### Feature Gating

The OAuth server is optional via Cargo features:

```toml
# In crates/cli/Cargo.toml
[dependencies]
axum = { version = "0.8", features = ["tokio"], optional = true }

[features]
default = ["wizard"]
wizard = ["axum"]
```

Headless/server-only builds can exclude it with `--no-default-features`.

### Token Storage

After OAuth completes, tokens are stored as `Secret<String>` in the config. File permissions are set to 600 on Unix:

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let path = config::config_path()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
}
```

---

## Daemon Setup

### OS Detection

The daemon module auto-detects the platform:

```rust
fn detect_daemon_type() -> Option<DaemonType> {
    if cfg!(target_os = "macos") {
        Some(DaemonType::Launchd)
    } else if cfg!(target_os = "linux") {
        if std::path::Path::new("/run/systemd/system").exists() {
            Some(DaemonType::Systemd)
        } else {
            None
        }
    } else {
        None
    }
}
```

### Systemd (Linux)

Generates and installs a user-level systemd unit:

```rust
// wizard/daemon/systemd.rs
pub fn generate_systemd_unit(binary_path: &str) -> String {
    format!(r#"[Unit]
Description=Klyntbot AI Assistant
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={binary_path} serve
Restart=on-failure
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
"#)
}

pub fn install_systemd(binary_path: &str) -> Result<()> {
    let unit_path = dirs::home_dir().unwrap()
        .join(".config/systemd/user/klyntbot.service");
    std::fs::create_dir_all(unit_path.parent().unwrap())?;
    std::fs::write(&unit_path, generate_systemd_unit(binary_path))?;

    // User-level, no sudo required
    std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"]).status()?;
    std::process::Command::new("systemctl")
        .args(["--user", "enable", "klyntbot"]).status()?;
    Ok(())
}
```

### Launchd (macOS)

Generates and installs a launchd plist:

```rust
// wizard/daemon/launchd.rs
pub fn generate_launchd_plist(binary_path: &str) -> String {
    // Returns XML plist content with RunAtLoad, KeepAlive, log paths
}

pub fn install_launchd(binary_path: &str) -> Result<()> {
    let plist_path = dirs::home_dir().unwrap()
        .join("Library/LaunchAgents/io.klyntbot.agent.plist");
    std::fs::write(&plist_path, generate_launchd_plist(binary_path))?;
    std::process::Command::new("launchctl")
        .args(["load", &plist_path.display().to_string()]).status()?;
    Ok(())
}
```

---

## Config System Integration

### Minimal Diff Saving

The `config::save()` function compares the config against `Config::default()` and only persists fields that differ. This means:

- A freshly-wizarded config with only Anthropic set will contain ~3 fields
- Default values (temperature 0.7, maxTokens 8192, etc.) are omitted
- On load, missing fields are filled from defaults via `#[serde(default)]`

The wizard calls `config::save(&state.config)` exactly once, at the end of all steps.

### Secret Handling

API keys and tokens use `Secret<String>`:

```rust
// Creating
let secret = Secret::new("sk-ant-...".to_string());

// Accessing (explicit intent required)
let key: &str = secret.expose();

// Debug/Display output shows [REDACTED]
println!("{:?}", secret);  // [REDACTED]
```

### Config File Format

JSON with `camelCase` field names (enforced by `#[serde(rename_all = "camelCase")]`).

Rust field `max_tokens` maps to JSON field `maxTokens`.

### Integration Points

| Existing Code | How Wizard Integrates |
|---|---|
| `config::save()` | Called once at wizard completion with `state.config` |
| `config::load()` | Detects existing config for returning user flow |
| `Config::set_provider_key()` | Called by provider step |
| `Secret::new()` | All API keys/tokens wrapped |
| `common::utils::terminal::*` | All UI rendering (boxes, colors, spinners) |
| `channels::ChannelManager` | Used for connection testing in validation step |
| `config::config_dir()` | Workspace and directory creation |

---

## Testing

### Test Patterns

The wizard's components can be unit tested by isolating logic from I/O:

- **Unit tests**: Inline as `#[cfg(test)] mod tests` in each module
- **Integration tests**: In `tests/` using the facade crate (`use klyntbot::*`)
- **Mock provider**: In `tests/mock_provider.rs` for LLM API mocking

### Testing Config Round-Trips

```rust
#[test]
fn test_wizard_config_round_trip() {
    let mut config = Config::default();
    config.set_provider_key("anthropic", "sk-ant-test".to_string());
    config.agents.defaults.model = "claude-sonnet-4-5".to_string();

    let json = serde_json::to_string_pretty(&config).unwrap();
    let loaded: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.providers.anthropic.api_key.expose(), "sk-ant-test");
    assert_eq!(loaded.agents.defaults.model, "claude-sonnet-4-5");
}
```

### Testing WizardState Checkpoints

```rust
#[test]
fn test_wizard_state_checkpoint_round_trip() {
    let mut state = WizardState::new();
    state.current_step = 3;
    state.selected_channels = vec!["telegram".into(), "discord".into()];
    state.install_daemon = true;

    state.save_checkpoint().unwrap();
    let loaded = WizardState::load_checkpoint().unwrap();

    assert_eq!(loaded.current_step, 3);
    assert_eq!(loaded.selected_channels, vec!["telegram", "discord"]);
    assert!(loaded.install_daemon);

    WizardState::clear_checkpoint().unwrap();
    assert!(WizardState::load_checkpoint().is_none());
}
```

### Testing Template Files

```rust
#[test]
fn test_template_files_not_overwritten() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("AGENTS.md");

    // Write custom content
    std::fs::write(&path, "My custom agents").unwrap();

    // Wizard should NOT overwrite
    create_template_file(&path, AGENTS_TEMPLATE).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "My custom agents");
}
```

### Testing Daemon Generators

```rust
#[test]
fn test_systemd_unit_generation() {
    let unit = generate_systemd_unit("/usr/local/bin/klyntbot");
    assert!(unit.contains("ExecStart=/usr/local/bin/klyntbot serve"));
    assert!(unit.contains("[Service]"));
    assert!(unit.contains("Restart=on-failure"));
}

#[test]
fn test_launchd_plist_generation() {
    let plist = generate_launchd_plist("/usr/local/bin/klyntbot");
    assert!(plist.contains("io.klyntbot.agent"));
    assert!(plist.contains("<string>/usr/local/bin/klyntbot</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
}
```

### Testing Validation

```rust
#[test]
fn test_api_key_format_validation() {
    assert!(validate_key_format("anthropic", "sk-ant-valid-key-here-long-enough"));
    assert!(!validate_key_format("anthropic", "sk-short"));
    assert!(!validate_key_format("anthropic", "wrong-prefix-key"));

    assert!(validate_key_format("openai", "sk-valid-openai-key-here-long-enough"));
    assert!(validate_key_format("slack_bot", "xoxb-valid-slack-bot-token"));
    assert!(!validate_key_format("slack_bot", "xoxp-wrong-prefix"));
}
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Just CLI crate tests (includes wizard)
cargo test -p cli

# Specific wizard tests
cargo test test_wizard

# With output
cargo test -- --nocapture

# Linting (must pass with zero warnings)
cargo clippy --workspace --all-targets --all-features
```

---

## Implementation Roadmap

### Phase 1: Core Framework
1. Create `wizard/` module directory structure
2. Define `WizardModule` trait and `WizardState` struct
3. Implement navigation engine (back/next/skip/quit)
4. Implement state checkpointing (save/load/clear)
5. Port existing provider step to `WizardModule` impl
6. Port existing model step
7. Port existing workspace step
8. Delete old `wizard.rs`, update `lib.rs` exports

### Phase 2: Enhanced Channel Setup
9. Implement per-channel flow architecture
10. Telegram guided setup (BotFather walkthrough)
11. Discord guided setup (token paste + invite URL)
12. Slack guided setup (manifest + token paste)
13. Email IMAP/SMTP wizard with provider presets
14. Generic token flow (QQ, Feishu, DingTalk, WhatsApp)

### Phase 3: OAuth Flows
15. Add `axum` dependency (feature-gated)
16. Implement OAuth callback server
17. Implement `open_browser()` cross-platform
18. Discord OAuth flow
19. Slack OAuth flow

### Phase 4: Daemon & Tools Setup
20. Tools configuration step (file access, shell, web search)
21. Systemd unit generator + installer
22. Launchd plist generator + installer
23. OS detection and daemon step integration

### Phase 5: Validation & Polish
24. Connection testing during wizard (verify API keys, channels)
25. Progress bar and step indicator UI
26. Live test message (optional)
27. Error recovery UX (retry on network failure)
28. Comprehensive test suite
