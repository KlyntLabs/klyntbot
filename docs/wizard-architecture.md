# Wizard Upgrade — Technical Architecture Document

## 1. Analysis of Existing Architecture

### Current Wizard (`crates/cli/src/wizard.rs`)
The existing wizard is a **monolithic 501-line function** with the following structure:
- **4 hardcoded steps**: Provider selection → API key → Channel toggle → Workspace setup
- **Synchronous I/O** via `std::io::stdin().read_line()` — blocking, no async
- **No state persistence** — if the wizard crashes at step 3, you restart from step 1
- **No validation feedback loop** — accepts API keys without testing them
- **No OAuth support** — all channels require manual token copy-paste
- **Templates baked in** as `const &str` blocks (AGENTS, SOUL, USER, TOOLS, IDENTITY, MEMORY)

### Config System (`crates/config`)
- `Config` struct with `#[serde(rename_all = "camelCase")]`
- `save()` uses `diff_json()` to write only non-default fields (smart minimal config)
- `load()` falls back to `Config::default()` if no file exists
- `load_with_env_overrides()` overlays `KLYNTBOT_*` env vars
- Secret wrapping via `Secret<String>` for API keys
- Config lives at `~/.klyntbot/config.json`

### Dependency Layer Constraints
```
Layer 0: common    — types, errors, terminal utilities
Layer 1: config    — schema + loader
Layer 6: cli       — wizard lives here, depends on everything below
```
The wizard is in the CLI crate (Layer 6), which already depends on `config`, `common`, `channels`, `bus`, `providers`, `agent`, etc. This means it can use all lower-layer APIs.

### Channel Authentication Patterns
Each channel has different auth:

| Channel   | Auth Method              | Config Fields                              |
|-----------|--------------------------|--------------------------------------------|
| Telegram  | Bot token (copy-paste)   | `token`, `allow_from`                      |
| Discord   | Bot token (copy-paste)   | `token`, `allow_from`                      |
| Slack     | Socket mode (2 tokens)   | `bot_token`, `app_token`, `allow_from`     |
| WhatsApp  | QR code (bridge server)  | `bridge_url`                               |
| Email     | IMAP/SMTP credentials    | `imap_*`, `smtp_*`, `from_address`         |
| QQ        | App credentials          | `app_id`, `secret`, `allow_from`           |
| Feishu    | App + encrypt key        | `app_id`, `app_secret`, `encrypt_key`      |
| DingTalk  | OAuth client credentials | `client_id`, `client_secret`               |

Slack and Discord (via developer portal) are candidates for OAuth browser flow. Others are token-based.

---

## 2. Proposed Architecture

### 2.1 Decision: Extend CLI Crate (No New Crate)

**Recommendation: Keep the wizard in `crates/cli`**, organized as a new `wizard/` module directory.

Rationale:
- The wizard is a CLI concern — it's interactive terminal UI
- It already has access to all needed deps (config, channels, providers, common)
- A new crate would add a dependency layer without benefit
- The only new dep needed is a small HTTP server for OAuth callbacks (add `axum` to cli's Cargo.toml)

### 2.2 Module Structure

```
crates/cli/src/
├── wizard/
│   ├── mod.rs              # Public API: run_wizard(), WizardState
│   ├── state.rs            # WizardState, persistence, navigation
│   ├── module_trait.rs     # WizardModule trait definition
│   ├── steps/
│   │   ├── mod.rs
│   │   ├── provider.rs     # Step 1: LLM provider selection + API key
│   │   ├── model.rs        # Step 2: Model selection + validation
│   │   ├── channels.rs     # Step 3: Channel setup (interactive per-channel)
│   │   ├── workspace.rs    # Step 4: Workspace creation + templates
│   │   └── daemon.rs       # Step 5: Service installation (new)
│   ├── oauth.rs            # Embedded HTTP server for OAuth callbacks
│   ├── channel_flows/
│   │   ├── mod.rs
│   │   ├── telegram.rs     # Telegram BotFather guided setup
│   │   ├── discord.rs      # Discord OAuth browser flow
│   │   ├── slack.rs        # Slack OAuth browser flow
│   │   ├── email.rs        # IMAP/SMTP credential wizard
│   │   └── generic.rs      # Generic token-paste flow (QQ, Feishu, DingTalk)
│   └── daemon/
│       ├── mod.rs
│       ├── systemd.rs      # Linux systemd unit generation
│       └── launchd.rs      # macOS launchd plist generation
├── wizard.rs               # DELETED (replaced by wizard/ module)
└── ...existing files...
```

### 2.3 WizardModule Trait

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
    /// Returns Ok(StepResult::Next) to advance, Ok(StepResult::Back) to go back,
    /// Ok(StepResult::Skip) to skip, or Err on fatal failure.
    async fn execute(&self, state: &mut WizardState) -> Result<StepResult>;

    /// Validate the state after execution. Called before advancing.
    /// Return Ok(()) if valid, Err with user-facing message if not.
    fn validate(&self, state: &WizardState) -> Result<()> { Ok(()) }

    /// Undo any side effects from execute (for back-navigation)
    async fn rollback(&self, state: &mut WizardState) -> Result<()> { Ok(()) }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    Next,       // Advance to next step
    Back,       // Go back to previous step
    Skip,       // Skip this step (only if is_optional)
    Restart,    // Restart wizard from beginning
    Quit,       // Quit without saving
}
```

### 2.4 WizardState

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

    /// OAuth tokens received during flows (transient, moved into config)
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

impl WizardState {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            current_step: 0,
            selected_channels: Vec::new(),
            pending_oauth_tokens: HashMap::new(),
            install_daemon: false,
            daemon_type: detect_daemon_type(),
            step_metadata: HashMap::new(),
        }
    }

    /// Save checkpoint to ~/.klyntbot/.wizard-state.json
    pub fn save_checkpoint(&self) -> Result<()> { /* ... */ }

    /// Load checkpoint if it exists
    pub fn load_checkpoint() -> Option<Self> { /* ... */ }

    /// Remove checkpoint file
    pub fn clear_checkpoint() -> Result<()> { /* ... */ }
}

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

### 2.5 Wizard Runner (Navigation Engine)

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
        Box::new(WorkspaceStep::new()),
        Box::new(DaemonStep::new()),
    ];

    // Filter to applicable steps
    let applicable: Vec<_> = steps.iter()
        .filter(|s| s.should_show(&state))
        .collect();

    let total = applicable.len();

    // Navigation loop
    while state.current_step < total {
        let step = &applicable[state.current_step];

        // Print step header
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

---

## 3. OAuth Integration Design

### 3.1 Embedded HTTP Server

Use `axum` (tokio ecosystem, minimal deps) for the callback server.

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

        // Wait for the callback (with timeout)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 min timeout
            result_rx,
        ).await??;

        let _ = shutdown_tx.send(());
        Ok(result)
    }
}
```

### 3.2 Browser Launch (Cross-Platform)

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

### 3.3 Token Storage

Tokens stored as `Secret<String>` in `config.json` (existing pattern). File permissions set to 600 on unix:

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let path = config::config_path()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
}
```

---

## 4. Daemon Setup Design

### 4.1 Systemd (Linux)

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
    let unit_content = generate_systemd_unit(binary_path);
    let unit_path = dirs::home_dir()
        .unwrap()
        .join(".config/systemd/user/klyntbot.service");

    std::fs::create_dir_all(unit_path.parent().unwrap())?;
    std::fs::write(&unit_path, unit_content)?;

    // User-level, no sudo required
    std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    std::process::Command::new("systemctl")
        .args(["--user", "enable", "klyntbot"])
        .status()?;

    Ok(())
}
```

### 4.2 Launchd (macOS)

```rust
// wizard/daemon/launchd.rs

pub fn generate_launchd_plist(binary_path: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.klyntbot.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary_path}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/klyntbot.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/klyntbot.stderr.log</string>
</dict>
</plist>"#)
}

pub fn install_launchd(binary_path: &str) -> Result<()> {
    let plist_content = generate_launchd_plist(binary_path);
    let plist_path = dirs::home_dir()
        .unwrap()
        .join("Library/LaunchAgents/io.klyntbot.agent.plist");

    std::fs::write(&plist_path, plist_content)?;

    std::process::Command::new("launchctl")
        .args(["load", &plist_path.display().to_string()])
        .status()?;

    Ok(())
}
```

---

## 5. New Dependencies

Add to `crates/cli/Cargo.toml`:

```toml
# OAuth callback server (wizard only)
axum = { version = "0.8", features = ["tokio"], optional = true }

[features]
default = ["wizard"]
wizard = ["axum"]
```

Feature-gated so headless/server-only builds can exclude it.

---

## 6. Integration Points

| Existing Code | How Wizard Integrates |
|---|---|
| `config::save()` | Called once at wizard completion with `state.config` |
| `config::load()` | Used to detect existing config for upgrade/re-run |
| `Config::set_provider_key()` | Called by provider step |
| `Secret::new()` | All API keys/tokens wrapped |
| `common::utils::terminal::*` | All UI rendering (boxes, colors, spinners, separators) |
| `channels::ChannelManager::initialize_channels()` | Used for connection testing after channel setup |
| `config::config_dir()` | Workspace/directory creation |
| `ChannelCommands::Login` | Replaced by interactive channel_flows |

---

## 7. Implementation Roadmap

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
12. Slack guided setup (token paste)
13. Email IMAP/SMTP wizard
14. Generic token flow (QQ, Feishu, DingTalk)

### Phase 3: OAuth Flows
15. Add `axum` dependency (feature-gated)
16. Implement OAuth callback server
17. Implement `open_browser()` cross-platform
18. Discord OAuth flow
19. Slack OAuth flow

### Phase 4: Daemon Setup
20. Systemd unit generator + installer
21. Launchd plist generator + installer
22. OS detection and daemon step integration

### Phase 5: Polish
23. Connection testing during wizard (verify API keys work)
24. Progress bar / step indicator
25. Error recovery UX (retry on network failure)

---

## 8. Key Design Decisions

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
