# CLI & Wizard System

The `cli` crate (`crates/cli/`) provides the command-line interface, interactive REPL, setup wizard, and plugin management for klyntbot. It sits at Layer 6 in the workspace dependency graph, consuming nearly every other crate to wire together the full application.

---

## Section 1: Narrative Overview

### CLI Design

The CLI is built on Clap's derive API. The top-level `Cli` struct holds an `Option<Commands>` subcommand. When no subcommand is given, the binary defaults to a brief status display. Five subcommands are defined: `chat`, `serve`, `init`, `status`, and `plugin`.

File: `crates/cli/src/commands.rs` (lines 1-67)

### Chat Command

The `chat` subcommand operates in two modes:

**One-shot mode** (`klyntbot chat "Hello"`): Sends a single message, streams the response with a thinking trace (classification, context assembly, tool calls), and exits. The session key defaults to `cli:default` but can be overridden with `--session`.

**REPL mode** (`klyntbot chat`): Launches an interactive read-eval-print loop. The setup sequence is:

1. Load config with `KLYNTBOT_*` env var overrides
2. Create the LLM provider and resolve the effective model
3. Print a startup banner showing the active model
4. Connect to SQLite storage and LanceDB vector store
5. Build an `AgentLoop` via the builder pattern
6. Enter the rustyline editor loop

The REPL supports slash commands (`/help`, `/paste`, `/history`, `/status`, `/session`, `/clear`, `/exit`, `/quit`), plain-text exit aliases (`exit`, `quit`, `:q`), and multi-line paste mode (`/paste` ... `/end` or Ctrl+D).

File: `crates/cli/src/chat.rs` (lines 19-265)

**Streaming and cancellation**: Every message goes through `run_with_streaming()`, which calls `agent_loop.process_direct_streaming()` to get a `StreamingHandle`. The function then enters a `tokio::select!` loop across three branches:

- Agent events (`event_rx`): classification, context assembly, tool start/end, content chunks, done/error
- Spinner animation ticks (every 80ms while thinking is active)
- Interactive prompts (`interaction_rx`): when the agent's `ask_user` tool fires, the streaming pauses and the tabbed multi-question UI renders inline

A dedicated Ctrl+C handler spawns a task that cancels the `cancel_token`, aborting the agent. After the event loop finishes, a `StreamRenderer` finalizes the output and a separator line shows elapsed time, model name, and tool/iteration counts.

File: `crates/cli/src/chat.rs` (lines 268-441)

### Serve Command

`klyntbot serve` starts the gateway daemon, wiring together all background services. The initialization sequence is:

1. Load config, connect to SQLite + LanceDB, create repos
2. Create the LLM provider
3. Initialize message bus (capacity 100)
4. Start cron service with SQL-backed job persistence, register a callback that dispatches to domain-specific handlers (focus checks, daily digest, overdue checks, weekly reports, calendar sync, daily planning, finance cron jobs)
5. Create a `NotificationDispatcher` for proactive notifications
6. Register built-in cron jobs via an `ensure_job!` macro that skips already-persisted jobs
7. Build `AgentLoop` with cron service and notification handle injected
8. Start dashboard HTTP server with its own shutdown signal watchers
9. Start `ChannelManager` (Telegram, Discord, WhatsApp, Slack, QQ, Email)
10. Start `HeartbeatService` (30-minute interval, wired to publish bus messages)
11. Run agent loop in background
12. Print status summary (dashboard URL, enabled services and channels)
13. Wait for Ctrl+C or SIGTERM, then shut down gracefully (5-second timeout)

File: `crates/cli/src/serve.rs` (lines 1-657)

### Init Command

`klyntbot init` runs a two-phase interactive wizard:

**Phase 1: Core Setup** -- Auto-detects existing configuration from three sources (config file, `KLYNTBOT_*` environment variables, system probes like directory writability), displays a summary, then prompts for:
- LLM provider (12 providers: Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini, Groq, vLLM, Zhipu, DashScope, Moonshot, MiniMax, AIHubMix)
- API key (masked input, keeps existing if detected)
- Chat channel (optional: Telegram, Discord, Slack, WhatsApp, Email, QQ)
- Channel token (if a channel is selected)
- Calendar provider (optional: Apple Calendar, Google Calendar, Generic CalDAV)
- Calendar credentials (varies by provider)

File: `crates/cli/src/wizard/core_setup.rs` (lines 128-461)

**Phase 2: Pack Selection** -- Presents 7 feature packs grouped into 3 tiers in a crossterm raw-mode checklist UI. Users navigate with arrow keys or j/k, toggle with Space, confirm with Enter:

| Pack | Tier | Skills | Config Mutations |
|------|------|--------|-----------------|
| Task Management | Core (locked) | todo | enrichment, semantic search |
| Productivity | Recommended | daily-planning, cron, summarize | daily planning, daily digest |
| AI Intelligence | Recommended | (none) | conversation embedding, conversation search, learning |
| Browser Automation | Optional | browser | tools.browser.enabled |
| Finance | Optional | finance | finance.enabled |
| Skill Creator | Optional | skill-creator | (none) |
| Weather | Optional | weather | (none) |

When the browser pack is enabled, the wizard checks for the `agent-browser` binary and offers to install it (npm or brew on macOS). It then checks for Playwright's Chromium cache and installs if missing.

File: `crates/cli/src/wizard/pack_selection.rs` (lines 268-377)

Flags:
- `--packs`: Skip Phase 1, jump directly to pack selection
- `--reset`: Discard existing config and start from defaults

Navigation supports Back (returns to Phase 1 from Phase 2 via recursive call) and Cancel (Ctrl+C or Esc).

File: `crates/cli/src/wizard/mod.rs` (lines 41-96)

### Status Command

Two variants are available:

**Brief status** (no subcommand, `klyntbot` alone): Shows version, ready/warning status based on API key presence, active provider/model, and a list of available commands.

File: `crates/cli/src/status.rs` (lines 7-54)

**Detailed status** (`klyntbot status`): Shows version with a separator line, provider, storage location, workspace path, and config file path. With `--verbose`, adds a table of all 6 channels and their enabled/disabled status.

File: `crates/cli/src/status.rs` (lines 57-137)

### Plugin Command

`klyntbot plugin` is a nested subcommand tree for WASM plugin lifecycle management:

**install**: Accepts three source formats:
- Local path: `./path.wasm` or directory containing `plugin.wasm` + `klyntbot.plugin.json`
- GitHub release: `github:user/repo` -- fetches latest release assets
- Registry: `plugin-id` or `plugin-id@version` -- queries the configured registry URL

File: `crates/cli/src/plugin_cmd/install.rs` (lines 1-218)

**list**: Scans the plugins directory using `PluginManager::scan_manifests()`, prints a formatted table of ID, version, and description.

File: `crates/cli/src/plugin_cmd/list.rs` (lines 1-33)

**remove**: Deletes the plugin's directory by ID.

File: `crates/cli/src/plugin_cmd/remove.rs` (lines 1-17)

**search**: Fetches the registry index JSON, filters by case-insensitive substring match on ID, name, and description.

File: `crates/cli/src/plugin_cmd/search.rs` (lines 1-54)

**update**: Compares installed manifest versions against the registry's `latest_version` field. Re-installs from registry when a newer version exists.

File: `crates/cli/src/plugin_cmd/update.rs` (lines 1-90)

**new**: Scaffolds a plugin project directory with language-specific templates (Rust, TypeScript, Python). Each template includes source code, a build file, and a `klyntbot.plugin.json` manifest.

File: `crates/cli/src/plugin_cmd/new_plugin.rs` (lines 1-223)

**publish**: Reads the local manifest and prints instructions for publishing via a PR to the plugin registry repository.

File: `crates/cli/src/plugin_cmd/publish.rs` (lines 1-44)

### Interactive REPL

The REPL is built on `rustyline` with a custom `SlashCommandHelper` that implements four traits:

- `Completer`: Tab-completes slash commands when the line starts with `/`
- `Hinter`: Shows inline gray hints for partially-typed slash commands
- `Highlighter`: Renders slash commands in cyan, hints in dim gray
- `Validator`: No-op (all input is valid)

Eight slash commands are registered: `/help`, `/paste`, `/history`, `/status`, `/session`, `/clear`, `/exit`, `/quit`. Command history persists to `~/.klyntbot/history.txt` and supports Up/Down navigation.

File: `crates/cli/src/interactive.rs` (lines 1-159)

### Wizard Framework

The wizard system is built on a pluggable module pattern:

**WizardModule trait**: Each step implements `name()`, `description()`, `is_required()`, `is_applicable()`, and `run()`. The `run()` method receives mutable access to `WizardState` and returns a `StepResult` (Next, Back, Skip, Cancel).

**WizardRunner**: Orchestrates a sequence of modules with forward/back navigation. Filters modules by `is_applicable()`, maintains a step counter, and saves config on completion via `config::save_sync()`.

**WizardState**: Holds the `Config` being built, step metadata (total, current), and a `is_fresh_install` flag. Two constructors: `new()` loads existing config if present, `fresh()` always uses defaults.

File: `crates/cli/src/wizard/framework.rs` (lines 1-215)

### Prompt Library

Five reusable prompt types live in `crates/cli/src/wizard/prompts/`. Each has an interactive mode (crossterm raw mode with keyboard navigation) and a non-TTY fallback (line-based numbered input):

**TextPrompt** (`prompt_text`): Optional default value, optional required validation. Also: `prompt_optional` (returns `Option<String>`) and `prompt_list` (collects items until empty line).

File: `crates/cli/src/wizard/prompts/text.rs` (lines 14-96)

**SelectPrompt** (`prompt_select`): Arrow keys + j/k navigate, Enter confirms. Returns 0-based index. Also: `prompt_select_with_input` -- a hybrid where expandable options reveal an inline text field on Tab.

File: `crates/cli/src/wizard/prompts/select.rs` (lines 27-39, 238-248)

**MultiSelectPrompt** (`prompt_multi_select`): Space toggles, `a` toggles all, Enter confirms. Returns indices. Also: `prompt_multi_select_with_defaults` -- pre-checks items from a `defaults` slice.

File: `crates/cli/src/wizard/prompts/multi_select.rs` (lines 19-27, 278-291)

**YesNoPrompt** (`prompt_yes_no`): Single-keypress y/n/Enter in interactive mode, defaults supported.

File: `crates/cli/src/wizard/prompts/yes_no.rs` (lines 14-62)

**SecretPrompt** (`prompt_secret`): Masks input with bullet characters, supports backspace, validates minimum length. Also: `prompt_secret_with_existing` (shows masked preview, Enter to keep existing) and `mask_secret` (display-only masking with provider prefix detection).

File: `crates/cli/src/wizard/prompts/secret.rs` (lines 15-110, 153-250)

All prompts share a `RawModeGuard` (RAII drop guard for crossterm raw mode), `is_interactive()` check, `step_prefix()` for consistent vertical-line formatting, `erase_lines()` for re-rendering, and `read_key()` that converts Ctrl+C into an error.

File: `crates/cli/src/wizard/prompts/mod.rs` (lines 35-89)

### Pack Registry

The `PackRegistry` is a static registry of all feature packs. Each `Pack` has an ID, display name, description, tier (`PackTier::Core | Recommended | Optional`), and a list of skill names.

Seven packs are defined in a `static PACKS: &[Pack]` constant. The registry provides four class methods:
- `all()` -- all packs ordered by tier
- `by_tier(tier)` -- filter by tier
- `get(id)` -- lookup by ID
- `skills_for_packs(enabled)` -- collect deduplicated skill names across enabled packs
- `default_selection()` -- Core + Recommended packs

File: `crates/cli/src/wizard/packs/registry.rs` (lines 1-115)

`apply_pack_config()` maps each pack ID to specific config section mutations. Packs not in the selection have their corresponding features disabled. The function also computes `config.packs.enabled` and `config.packs.enabled_skills`.

File: `crates/cli/src/wizard/pack_selection.rs` (lines 74-103)

### Environment Detection

The `detect` module probes three sources to pre-fill the wizard:

- **Config**: Reads the existing `~/.klyntbot/config.json` for provider, API key, model, channel, and calendar settings
- **Environment variables**: Checks `KLYNTBOT_PROVIDERS__*__API_KEY` and `KLYNTBOT_CHANNELS__*__TOKEN` (higher priority than config)
- **System probes**: Tests data directory writability by writing and removing a marker file

Each detected value is tagged with a `DetectSource` enum (Config, EnvVar, Detected) for display attribution.

File: `crates/cli/src/wizard/detect.rs` (lines 1-181)

### Terminal UI

**Box Drawing**: The `ask_user_prompt/box_drawing.rs` module provides rounded-corner box primitives (`write_box_top`, `write_box_line`, `write_box_empty`, `write_box_sep`, `write_box_bottom`) with a `visible_len()` function that strips ANSI escape sequences for correct padding. Unicode characters (rounded corners) are used when colors are enabled; ASCII fallback (`+`, `-`, `|`) otherwise.

File: `crates/cli/src/wizard/ask_user_prompt/box_drawing.rs` (lines 1-147)

**Tabbed Multi-Question UI**: The `ask_user_prompt` module renders `InteractionRequest` objects as a tabbed form within a rounded box. Features include tab-based navigation (Tab/Shift+Tab/number keys), four question types (SingleSelect, MultiSelect, YesNo, FreeText), auto-advance to the next unanswered tab, and a Submit tab with answer review. The non-TTY fallback renders sequential numbered prompts.

File: `crates/cli/src/wizard/ask_user_prompt/mod.rs` (lines 1-48, 909-925)

---

## Section 2: API Reference

### Commands Enum

```
crates/cli/src/commands.rs
```

```rust
// Line 8-17
pub struct Cli {
    pub command: Option<Commands>,
}

// Line 20-67
pub enum Commands {
    Chat {
        message: Option<String>,          // omit for REPL
        #[arg(short, long, default_value = "cli:default")]
        session: String,
        #[arg(short = 'V', long)]
        verbose: bool,
    },
    Serve {
        #[arg(short, long, default_value = "18790")]
        port: u16,
        #[arg(short, long)]
        verbose: bool,
    },
    Init {
        #[arg(long)]
        packs: bool,
        #[arg(long)]
        reset: bool,
    },
    Status {
        #[arg(short, long)]
        verbose: bool,
    },
    #[command(subcommand)]
    Plugin(PluginCommand),
}
```

### handle_chat()

```
crates/cli/src/chat.rs, line 19
```

```rust
pub async fn handle_chat(message: Option<String>, session: String, verbose: bool) -> Result<()>
```

Wiring steps:
1. `config::load_with_env_overrides().await` -- load config
2. `providers::create_provider(&config)` -- resolve provider + model
3. `StoragePool::connect(&data_dir).await` -- open SQLite
4. `VectorStore::connect(&data_dir).await` -- open LanceDB
5. `AgentLoop::builder().with_bus().with_provider().with_config().with_pool().build().await` -- create agent
6. Dispatch to one-shot (`run_with_streaming`) or REPL (rustyline editor loop)

### handle_serve()

```
crates/cli/src/serve.rs, line 18
```

```rust
pub async fn handle_serve(port: u16) -> Result<()>
```

Wiring steps:
1. Load config, connect storage, create repos
2. Create LLM provider
3. Initialize `MessageBus` (capacity 100)
4. Start `CronService`, set callback, register built-in jobs
5. Create `NotificationDispatcher`
6. Build `AgentLoop` with cron + notification injection
7. Start `DashboardServer`
8. Start `ChannelManager`
9. Start `HeartbeatService` (1800s interval)
10. Run agent loop in background
11. Wait for shutdown signal, graceful teardown with 5s timeout

### handle_status()

```
crates/cli/src/status.rs, lines 7, 57
```

```rust
pub async fn handle_brief_status() -> Result<()>   // no subcommand
pub async fn handle_status(verbose: bool) -> Result<()>  // `klyntbot status`
```

`resolve_provider_and_model(config)` (line 144) resolves the display provider name and model. Priority: explicit `agents.defaults.provider`, then API-key presence detection (anthropic > openai > openrouter > deepseek).

### Wizard Framework Types

```
crates/cli/src/wizard/framework.rs
```

**StepResult** (line 16):
```rust
pub enum StepResult {
    Next,    // advance
    Back,    // return to previous
    Skip,    // skip optional step
    Cancel,  // abort wizard
}
```

**WizardState** (line 35):
```rust
pub struct WizardState {
    pub config: Config,
    pub total_steps: usize,
    pub current_step: usize,
    pub is_fresh_install: bool,
}
```

Constructors:
- `WizardState::new()` -- loads existing config or defaults (line 54)
- `WizardState::fresh()` -- always `Config::default()`, `is_fresh_install = true` (line 73)
- `step_header(title)` -- formats "Step N of M: Title" (line 83)

**WizardModule** trait (line 100):
```rust
pub trait WizardModule {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn is_required(&self) -> bool { true }
    fn is_applicable(&self, state: &WizardState) -> bool { true }
    fn run(&self, state: &mut WizardState) -> Result<StepResult>;
}
```

**WizardRunner** (line 136):
```rust
pub struct WizardRunner { modules: Vec<Box<dyn WizardModule>> }
```

Methods:
- `new()` -- empty runner (line 148)
- `add_module(impl WizardModule + 'static)` -- append step (line 155)
- `run() -> Result<bool>` -- drive wizard to completion, returns `true` on save (line 163)

### Pack Registry

```
crates/cli/src/wizard/packs/registry.rs
```

**Pack** (line 7):
```rust
pub struct Pack {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub tier: PackTier,            // from config crate: Core | Recommended | Optional
    pub skills: &'static [&'static str],
}
```

**PACKS** constant (line 65): 7 entries ordered Core, Recommended, Optional:

| ID | Tier | Skills |
|----|------|--------|
| `task-management` | Core | `["todo"]` |
| `productivity` | Recommended | `["daily-planning", "cron", "summarize"]` |
| `ai-intelligence` | Recommended | `[]` |
| `browser` | Optional | `["browser"]` |
| `finance` | Optional | `["finance"]` |
| `skill-creator` | Optional | `["skill-creator"]` |
| `weather` | Optional | `["weather"]` |

**PackRegistry** methods (line 21):
- `all() -> Vec<&'static Pack>` (line 25)
- `by_tier(tier: PackTier) -> Vec<&'static Pack>` (line 30)
- `get(id: &str) -> Option<&'static Pack>` (line 35)
- `skills_for_packs(enabled: &[String]) -> Vec<String>` (line 40)
- `default_selection() -> Vec<String>` (line 56)

### apply_pack_config()

```
crates/cli/src/wizard/pack_selection.rs, line 74
```

```rust
pub fn apply_pack_config(config: &mut Config, enabled_packs: &[String])
```

Pack-to-config mutation mapping:
- **task-management**: `todo.enrichment.enabled`, `todo.search.enabled`
- **productivity**: `todo.daily_planning.enabled`, `todo.notifications.daily_digest`
- **ai-intelligence**: `conversation.embedding.enabled`, `conversation.search.enabled`, `learning.enabled`
- **finance**: `finance.enabled`
- **browser**: `tools.browser.enabled`
- **weather, skill-creator**: No config mutations (skill-only packs)

Also updates `config.packs.enabled` and `config.packs.enabled_skills` (via `PackRegistry::skills_for_packs`).

### PackOption (UI model)

```
crates/cli/src/wizard/pack_selection.rs, line 17
```

```rust
pub struct PackOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: PackTier,
    pub checked: bool,
    pub locked: bool,   // Core packs: always true
}
```

`build_pack_options(currently_enabled: &[String]) -> Vec<PackOption>` (line 43): Builds the checklist from the registry. Core packs are locked + checked. Others are checked only if in `currently_enabled`.

### Prompt Types

```
crates/cli/src/wizard/prompts/
```

**Text prompts** (`text.rs`):
- `prompt_text(label, default: Option<&str>, required: bool) -> Result<String>` (line 14)
- `prompt_optional(label) -> Result<Option<String>>` (line 44)
- `prompt_list(header, existing: &[String]) -> Result<Vec<String>>` (line 61)

**Select prompts** (`select.rs`):
- `SelectOption<'a> { label: &'a str, description: &'a str }` (line 16)
- `prompt_select(header, options, default_idx) -> Result<usize>` (line 27)
- `SelectWithInputOption<'a> { label, description, expandable: bool, input_hint: Option<&'a str> }` (line 218)
- `SelectWithInputResult { index: usize, text: Option<String> }` (line 228)
- `prompt_select_with_input(header, options, default_idx) -> Result<SelectWithInputResult>` (line 238)

**Multi-select prompts** (`multi_select.rs`):
- `prompt_multi_select(header, options: &[SelectOption]) -> Result<Vec<usize>>` (line 19)
- `prompt_multi_select_with_defaults(header, options, defaults: &[bool]) -> Result<Vec<usize>>` (line 278)

**Yes/No prompt** (`yes_no.rs`):
- `prompt_yes_no(prompt, default: bool) -> Result<bool>` (line 14)

**Secret prompt** (`secret.rs`):
- `prompt_secret(label, min_length: usize) -> Result<String>` (line 15)
- `prompt_secret_with_existing(label, existing, min_length) -> Result<Option<String>>` (line 153)
- `mask_secret(s: &str) -> String` (line 116) -- prefix-aware masking

**Shared utilities** (`mod.rs`):
- `RawModeGuard` -- RAII struct, `enable() -> Result<Self>`, disables raw mode on drop (line 41)
- `is_interactive() -> bool` -- checks stdin + stdout are TTYs (line 57)
- `step_prefix() -> String` -- returns the branded vertical-line prefix (line 61)
- `erase_lines(n) -> Result<()>` -- moves cursor up N lines, clears each (line 67)
- `read_key() -> Result<KeyEvent>` -- reads one key, Ctrl+C becomes error (line 80)

### Plugin Command Types

```
crates/cli/src/plugin_cmd/mod.rs
```

**PluginCommand** enum (line 13):
```rust
pub enum PluginCommand {
    Install { source: String },
    List,
    Remove { id: String },
    Search { query: String },
    Update { id: Option<String> },
    New { name: String, #[arg(short, long, default_value = "rust")] lang: String },
    Publish,
}
```

**Dispatcher** (line 55):
```rust
pub async fn handle_plugin(cmd: PluginCommand, config: &config::Config) -> Result<()>
```

**Helper** (line 50):
```rust
pub fn plugins_dir(config: &config::Config) -> PathBuf  // {data_dir}/plugins
```

### Environment Detection Types

```
crates/cli/src/wizard/detect.rs
```

**DetectSource** (line 7):
```rust
pub enum DetectSource {
    Config,    // from ~/.klyntbot/config.json
    EnvVar,    // from KLYNTBOT_* environment variable
    Detected,  // from system probe
}
```

**DetectedState** (line 28):
```rust
pub struct DetectedState {
    pub provider: Option<(String, DetectSource)>,
    pub api_key: Option<(String, DetectSource)>,
    pub model: Option<(String, DetectSource)>,
    pub data_dir: String,
    pub data_dir_writable: bool,
    pub channel: Option<(String, DetectSource)>,
    pub channel_token: Option<(String, DetectSource)>,
    pub calendar: Option<(String, DetectSource)>,
}
```

Methods:
- `from_config(config: &Config) -> Self` (line 49) -- populate from config fields
- `overlay_env_vars(&mut self)` (line 129) -- override with env vars (higher priority)
- `check_data_dir(&mut self)` (line 167) -- probe directory writability

### Core Setup Metadata

```
crates/cli/src/wizard/core_setup.rs
```

**ProviderInfo** (line 18):
```rust
pub struct ProviderInfo { pub key: &'static str, pub name: &'static str }
pub static PROVIDER_INFO: &[ProviderInfo]  // 12 entries (line 23)
```

**ChannelInfo** (line 75):
```rust
pub struct ChannelInfo { pub key: &'static str, pub name: &'static str }
pub static CHANNEL_INFO: &[ChannelInfo]  // 6 entries (line 80)
```

**CalendarProviderInfo** (line 108):
```rust
pub struct CalendarProviderInfo { pub key: &'static str, pub name: &'static str }
pub static CALENDAR_PROVIDER_INFO: &[CalendarProviderInfo]  // 3 entries (line 113)
```

### Ask-User Prompt Types

```
crates/cli/src/wizard/ask_user_prompt/mod.rs
```

**PromptResult** (line 909):
```rust
pub struct PromptResult {
    pub response: FormResponse,
    pub summary_lines: u16,
}
```

**Entry point** (line 915):
```rust
pub fn prompt_multi_question(request: &InteractionRequest) -> Result<PromptResult>
```

Delegates to the tabbed interactive UI when `is_interactive()` returns true, otherwise falls back to `fallback::prompt_non_interactive()`.

### Box Drawing Types

```
crates/cli/src/wizard/ask_user_prompt/box_drawing.rs
```

**RoundedChars** (line 35): Holds the 8 box-drawing character slots (corners, edges, separators). Two static instances: `ROUNDED_UNICODE` (lines 46-55) and `ROUNDED_ASCII` (lines 57-66).

**Functions**:
- `visible_len(s) -> usize` (line 13) -- ANSI-stripped character count
- `rounded_chars() -> &'static RoundedChars` (line 68) -- picks unicode or ascii
- `write_box_top(out, title, inner_w) -> Result<usize>` (line 77)
- `write_box_line(out, content, inner_w) -> Result<usize>` (line 94)
- `write_box_empty(out, inner_w) -> Result<usize>` (line 111)
- `write_box_sep(out, inner_w) -> Result<usize>` (line 124)
- `write_box_bottom(out, inner_w) -> Result<usize>` (line 137)

### Interactive REPL Types

```
crates/cli/src/interactive.rs
```

**SLASH_COMMANDS** (line 11): `&[(&str, &str)]` -- 8 command/description pairs.

**SlashCommandHelper** (line 23):
```rust
pub struct SlashCommandHelper {
    commands: Vec<(&'static str, &'static str)>,
}
```

Implements: `Completer<Candidate = Pair>`, `Hinter<Hint = CommandHint>`, `Highlighter`, `Validator`, `Helper`.

**CommandHint** (line 71):
```rust
pub struct CommandHint { text: String }
```

Implements: `Hint` (with `display()` and `completion()`).
