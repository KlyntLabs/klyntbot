# CLI

## Purpose

The `cli` crate (Layer 6) is the command-line interface for Klyntbot. It defines the top-level Clap-derived command structure, implements the gateway daemon startup sequence (`serve`), an interactive two-phase configuration wizard (`init`), a system status reporter (`status`), and a full plugin management suite (`plugin`). It also contains the heartbeat service that periodically wakes the agent to check a workspace file for pending tasks, and the `ask_user` prompt renderer -- a tabbed multi-question terminal UI consumed by the agent's `ask_user` tool at runtime.

## Key Types

### Command Structure

**`Cli`** -- the root Clap parser struct. Derives `Parser` with the binary name `klyntbot`. Has a single optional `command` field holding a `Commands` enum.

**`Commands`** -- the four subcommands:

| Variant | Flags | Purpose |
|---------|-------|---------|
| `Serve` | `--port` (default 18790), `--verbose` | Start the gateway daemon |
| `Init` | `--packs`, `--reset` | Run the configuration wizard |
| `Status` | `--verbose` | Display system/config status |
| `Plugin(PluginCommand)` | (nested subcommands) | Manage WASM plugins |

When no subcommand is given (`command` is `None`), the binary prints a brief status screen showing version, provider, and suggested commands.

**`PluginCommand`** -- seven nested subcommands for plugin lifecycle management:

| Variant | Args | Purpose |
|---------|------|---------|
| `Install` | `source` (path, `github:user/repo`, or registry ID) | Install a plugin |
| `List` | (none) | List installed plugins |
| `Remove` | `id` | Uninstall a plugin by ID |
| `Search` | `query` | Search the plugin registry |
| `Update` | `id` (optional -- omit for all) | Update to latest version |
| `New` | `name`, `--lang` (rust/typescript/python, default rust) | Scaffold a new plugin project |
| `Publish` | (none) | Guide user through registry publication |

### Wizard Types

**`WizardState`** -- mutable state threaded through all wizard modules. Holds the `Config` being built, step counters (`total_steps`, `current_step`), and whether this is a fresh install. Two constructors: `new()` loads existing config if present; `fresh()` always starts from `Config::default()` (used with `--reset`).

**`WizardModule`** (trait) -- pluggable wizard step interface. Methods: `name()`, `description()`, `is_required()`, `is_applicable(state)`, and `run(state) -> StepResult`. The `is_applicable` method allows modules to conditionally appear based on current config state.

**`StepResult`** -- navigation enum returned by each module: `Next`, `Back`, `Skip`, `Cancel`. The `WizardRunner` advances, reverses, or exits based on these signals.

**`WizardRunner`** -- orchestrates a `Vec<Box<dyn WizardModule>>`. Filters to applicable modules, assigns step numbers, and drives a forward/back loop through them. On completion, calls `config::save_sync()`.

**`DetectedState`** -- auto-detection results from three sources: existing config (`DetectSource::Config`), environment variables (`DetectSource::EnvVar`), and system probes (`DetectSource::Detected`). Fields include provider, API key, model, data directory writability, channel, channel token, and calendar provider.

**`Pack`** -- a feature pack definition with `id`, `name`, `description`, `tier` (`PackTier::Core`, `Recommended`, or `Optional`), and a list of `skills`.

**`PackRegistry`** -- static registry of all seven packs. Provides `all()`, `by_tier()`, `get(id)`, `skills_for_packs(enabled)`, and `default_selection()`.

**`HeartbeatService`** -- periodic agent wake-up that reads `HEARTBEAT.md` from the workspace and triggers the agent when actionable content is found.

### Prompt Types

The `wizard::prompts` module provides five reusable prompt components, each with an interactive (raw-mode TTY) path and a line-based fallback for non-interactive environments:

| Function | Interactive behavior | Fallback behavior |
|----------|---------------------|-------------------|
| `prompt_select` | Arrow keys + j/k to navigate, Enter to confirm | Numbered list, type a number |
| `prompt_multi_select` | Arrow keys, Space to toggle, `a` to toggle all, Enter to confirm | Comma-separated numbers |
| `prompt_multi_select_with_defaults` | Same as above, with pre-checked items | Same as above |
| `prompt_text` | Line input with optional default | Same |
| `prompt_secret` | Masked input (characters display as bullet symbols), backspace support | Plain line input |
| `prompt_yes_no` | Single keypress (y/n/Enter), no need to press Enter after y or n | y/n line input |
| `prompt_select_with_input` | Hybrid: select from list, TAB to expand an inline text field on certain options | Numbered list with optional text after number |

Shared infrastructure:
- **`RawModeGuard`** -- RAII guard that enables crossterm raw mode on creation and restores normal mode on drop, preventing terminal corruption on panics or early returns.
- **`is_interactive()`** -- returns true when both stdin and stdout are terminals, used to choose between interactive and fallback paths.
- **`read_key()`** -- reads a single key event, converting Ctrl+C into an `anyhow` error for clean cancellation.
- **`step_prefix()`** -- returns the branded vertical-bar prefix string used for consistent indentation throughout the wizard.

## How It Works

### Serve Command -- Startup Sequence

`handle_serve(port)` initializes the gateway daemon in a specific order, where each step depends on the previous:

1. **Config** -- `config::load_with_env_overrides()` loads `~/.klyntbot/config.json` and overlays `KLYNTBOT_` environment variables.
2. **Storage** -- `StoragePool::connect(data_dir)` opens or creates the SQLite database at `{data_dir}/data.db`, enables WAL mode, runs migrations. `Repos::from_pool()` creates the repository aggregate. `VectorStore::connect()` opens LanceDB at `{data_dir}/lancedb/`.
3. **Provider** -- `providers::create_provider(config)` auto-detects the LLM provider from model name keywords and API key presence, returning a `(Box<dyn LlmProvider>, resolved_model)` pair. The resolved model is written back into config.
4. **Message Bus** -- `MessageBus::new(100)` creates a bounded async channel pair (inbound + outbound) with capacity 100.
5. **Cron Service** -- `CronService::new(repos.cron)` creates the SQL-backed scheduler. `start()` loads persisted jobs from SQLite. A callback closure is set that dispatches cron ticks to domain handlers (focus checks, daily digests, overdue checks, weekly reports, calendar sync, daily planning, and four finance jobs). Built-in jobs are registered via the `ensure_job!` macro, which skips creation if a job with the same name already exists from a previous run.
6. **Notification Dispatcher** -- creates a `NotificationDispatcher` wired to the bus outbound sender, used for sending focus deadline reminders and digest notifications.
7. **Agent Loop** -- `AgentLoop::builder(bus, provider, config)` configures the agent with the SQLite pool, cron service reference, and notification handle, then `.build()`. The inbound receiver is taken out of the agent loop for separate ownership.
8. **Channel Manager** -- `ChannelManager::new(config, bus)` initializes all enabled chat platform adapters (Telegram, Discord, WhatsApp, Slack, QQ, Email).
9. **Heartbeat Service** -- `HeartbeatService::new(workspace, 1800, true)` creates a 30-minute periodic checker. A callback is wired to publish inbound messages through the bus.
10. **Launch** -- The agent loop and channel manager are spawned as separate tokio tasks. A summary of running services and enabled channels is printed. The main thread blocks on `Ctrl+C` or `SIGTERM`.
11. **Shutdown** -- Sets the agent's atomic shutdown flag, stops the cron and heartbeat services, waits up to 5 seconds for spawned tasks to finish, then aborts any remaining tasks.

### Init Command -- Two-Phase Wizard

The wizard runs in two phases. The `--packs` flag skips Phase 1; `--reset` wipes existing config to defaults first.

**Phase 1: Core Setup** (`core_setup::run_core_setup`):

1. **Auto-detection** -- `DetectedState::from_config()` scans the existing config for provider, API key, model, channel, channel token, and calendar provider. `overlay_env_vars()` checks `KLYNTBOT_*` environment variables (higher priority). `check_data_dir()` probes directory writability by creating and removing a marker file.
2. **Summary** -- renders what was auto-detected (provider, data directory, channel, calendar) with check marks for configured items and dots for missing ones.
3. **Provider selection** -- shows a 12-option select list (Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini, Groq, vLLM, Zhipu, DashScope, Moonshot, MiniMax, AIHubMix). If an API key already exists, offers to keep or replace it with masked display.
4. **Channel selection** (optional) -- "None (CLI only)" plus 6 channels. Selecting Telegram, Discord, or Slack prompts for the bot token via secret input.
5. **Calendar selection** (optional) -- "None" plus Apple, Google, or generic CalDAV. Each option has its own set of credential prompts (username, password/app-specific password, CalDAV URL, calendar name, and OAuth fields for Google).

**Phase 2: Pack Selection** (`pack_selection::run_pack_selection`):

1. **Build options** -- `build_pack_options()` maps the `PackRegistry` into a `Vec<PackOption>`, marking Core packs as locked and pre-checking packs that appear in the current `config.packs.enabled`.
2. **Interactive checklist** -- renders packs grouped by tier (Core, Recommended, Optional) with keyboard navigation (arrow keys/j/k), space to toggle, Enter to confirm. Core packs show a locked indicator and cannot be unchecked.
3. **Config mutations** -- `apply_pack_config()` maps each enabled pack to specific config section toggles:
   - `task-management`: enables `todo.enrichment`, `todo.search`
   - `productivity`: enables `todo.daily_planning`, `todo.notifications.daily_digest`
   - `ai-intelligence`: enables `conversation.embedding`, `conversation.search`, `learning`
   - `finance`: enables `finance.enabled`
   - `browser`: enables `tools.browser.enabled` (also offers to install `agent-browser` and Chromium)
   - `weather`, `skill-creator`: skill-only, no config mutations
4. **Skills aggregation** -- `PackRegistry::skills_for_packs()` collects all skill names from enabled packs into `config.packs.enabled_skills`.
5. **Browser pack setup** -- if the browser pack is enabled, checks for the `agent-browser` binary using `which`. If missing, offers installation via npm or brew. Then checks for Playwright's Chromium cache and runs `agent-browser install` if needed.

### Status Command

Two modes:
- **Brief** (`handle_brief_status`, invoked with no subcommand) -- shows version, readiness indicator based on API key presence, active provider/model, and suggested commands.
- **Verbose** (`handle_status --verbose`) -- adds storage path, workspace path, config path, and a channel-by-channel enabled/disabled table.

Provider resolution follows a priority order: explicit `agents.defaults.provider` field first, then API-key detection (checks Anthropic, OpenAI, OpenRouter, DeepSeek in order).

### Plugin Commands

All plugin commands receive the plugins directory (resolved from `config.data_dir_path().join("plugins")`) and the registry URL from config.

- **install** -- three source formats are recognized. Local paths (starting with `.` or `/`) expect a `klyntbot.plugin.json` manifest sibling to the wasm file. `github:user/repo` fetches the latest GitHub release and downloads `plugin.wasm` and `klyntbot.plugin.json` assets. Registry IDs (optionally with `@version`) look up the plugin in the configured registry JSON index and download the wasm binary. Permissions are displayed after manifest parsing.
- **list** -- calls `PluginManager::scan_manifests()` and prints a table of ID, version, and description.
- **remove** -- deletes the plugin's subdirectory under the plugins directory.
- **search** -- fetches the registry index and filters by case-insensitive substring match against ID, name, and description fields.
- **update** -- compares installed manifest versions against the registry's `latest_version` field. If newer, re-installs from registry. Can target a specific plugin or update all.
- **new** -- scaffolds a plugin project in Rust, TypeScript, or Python. Each scaffold creates the language-specific project files, a `klyntbot.plugin.json` manifest, and prints build/install instructions.
- **publish** -- a guided flow: checks for a manifest in the current directory, then prints instructions for creating a GitHub release and opening a PR to the plugin registry.

### AskUser Prompt Rendering

The `ask_user_prompt` module provides the terminal UI for the agent's `ask_user` tool, which presents multi-question forms to the user during agent execution. It handles `InteractionRequest` structs containing questions of four types:

- **SingleSelect** -- radio-button style, arrow keys to pick one option
- **MultiSelect** -- checkbox style, space to toggle, Enter to confirm
- **YesNo** -- toggle between Yes/No with arrow keys
- **FreeText** -- inline text input field

The UI is rendered as a tabbed box-drawing interface:
- A **tab bar** across the top shows all questions as named tabs, with a filled circle on answered tabs and an empty circle on unanswered ones. The active tab is highlighted.
- A **question pane** displays the current question's text and its interactive controls.
- A **hint bar** at the bottom shows available keyboard shortcuts.

Navigation: Left/Right arrows (or Tab/Shift+Tab) switch between question tabs. Up/Down arrows navigate within the current question. Enter selects an answer and auto-advances to the next unanswered question. The final "Submit" tab shows a summary of all answers; pressing Enter there returns the `FormResponse::Completed` with all answers.

The box is drawn with rounded Unicode corners (or ASCII fallback when colors are disabled), with ANSI-aware visible-length calculations to handle padding correctly despite escape sequences.

A **non-interactive fallback** (`fallback.rs`) handles piped/CI environments by rendering questions sequentially with numbered prompts and line-based input.

### Heartbeat Service

`HeartbeatService` runs on a configurable interval (default 30 minutes). Each tick:

1. Reads `HEARTBEAT.md` from the workspace directory.
2. Checks if the content is actionable via `is_heartbeat_empty()`, which skips empty lines, headers, HTML comments, and bare checkboxes (both empty and completed).
3. If actionable content exists, invokes the callback (wired to publish an inbound message through the bus), which triggers an agent turn with the `HEARTBEAT_PROMPT`.
4. Checks the agent's response for the `HEARTBEAT_OK` token to determine if action was taken.

The service exposes `trigger_now()` for manual heartbeat invocation and `stop()` for clean shutdown via task abort.

## Connections

**Depends on:**
- `config` (Layer 1) -- config loading, saving, schema types, `Secret`
- `common` (Layer 0) -- error types, terminal utilities (`colorize`, `BoxChars`, `draw_wizard_step_header`), `InteractionRequest`/`FormResponse` types
- `agent` (Layer 5) -- `AgentLoop`, `NotificationDispatcher`
- `bus` (Layer 1) -- `MessageBus`, `InboundMessage`, `OutboundMessage`
- `channels` (Layer 4) -- `ChannelManager`
- `scheduling` (Layer 2) -- `CronService`, `CronSchedule`, `CronJob`
- `providers` (Layer 2) -- `create_provider`, `ProviderRegistry`
- `storage` (Layer 1.5) -- `StoragePool`, `Repos`, `VectorStore`
- `plugin-runtime` -- `PluginManager`, `PluginManifest` (used by plugin commands)
- `crossterm` -- raw-mode terminal I/O for interactive prompts
- `clap` -- derive-based CLI parsing

**Depended on by:**
- `klyntbot` (Layer 7) -- the binary entry point calls into `cli::Commands` to dispatch subcommands
