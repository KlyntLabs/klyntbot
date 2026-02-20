# Init Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the 11-step wizard with a 2-phase flow (smart core setup + unified pack selection) and add pack-based skill filtering.

**Architecture:** Add `PacksConfig` to the config schema. Create a `PackRegistry` in the CLI crate that defines 7 packs (each bundling config mutations + skill lists). Rewrite the wizard orchestrator as two direct functions. Update `SkillManager` to filter skills by enabled packs.

**Tech Stack:** Rust, serde, crossterm (existing terminal UI), clap (CLI flags)

---

### Task 1: Add PacksConfig to Config Schema

**Files:**
- Create: `crates/config/src/schema/packs.rs`
- Modify: `crates/config/src/schema/mod.rs:18-44`
- Modify: `crates/config/src/schema/core.rs:74-125`

**Step 1: Write the failing test**

Add to `crates/config/src/schema/packs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packs_config_default() {
        let config = PacksConfig::default();
        assert_eq!(
            config.enabled,
            vec![
                "task-management",
                "productivity",
                "ai-intelligence",
                "developer-tools",
            ]
        );
    }

    #[test]
    fn test_packs_config_serialization_camel_case() {
        let config = PacksConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"enabled\""));
    }

    #[test]
    fn test_packs_config_round_trip() {
        let config = PacksConfig {
            enabled: vec!["task-management".to_string(), "finance".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: PacksConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.enabled, vec!["task-management", "finance"]);
    }

    #[test]
    fn test_pack_tier_ordering() {
        assert!(PackTier::Core < PackTier::Recommended);
        assert!(PackTier::Recommended < PackTier::Optional);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p config -E 'test(packs_config)'`
Expected: FAIL — module `packs` not found

**Step 3: Write minimal implementation**

Create `crates/config/src/schema/packs.rs`:

```rust
//! Feature pack configuration.

use serde::{Deserialize, Serialize};

/// Which feature packs are enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacksConfig {
    /// List of enabled pack IDs.
    #[serde(default = "default_enabled_packs")]
    pub enabled: Vec<String>,
}

impl Default for PacksConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled_packs(),
        }
    }
}

fn default_enabled_packs() -> Vec<String> {
    vec![
        "task-management".to_string(),
        "productivity".to_string(),
        "ai-intelligence".to_string(),
        "developer-tools".to_string(),
    ]
}

/// Pack tier determines default selection state in the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackTier {
    Core,
    Recommended,
    Optional,
}
```

Then wire it into the schema:

In `crates/config/src/schema/mod.rs`, add after line 29 (`mod tools;`):
```rust
mod packs;
```
And after line 43 (`pub use self::tools::*;`):
```rust
pub use self::packs::*;
```

In `crates/config/src/schema/core.rs`, add import after line 18:
```rust
use super::packs::PacksConfig;
```
And add field to `Config` struct after line 124 (`pub database_url`):
```rust
    /// Feature packs (controls which skills and config sections are active).
    #[serde(default)]
    pub packs: PacksConfig,
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p config -E 'test(packs_config)' && cargo nextest run -p config -E 'test(pack_tier)'`
Expected: PASS

**Step 5: Verify existing tests still pass**

Run: `cargo nextest run -p config`
Expected: All existing config tests PASS (PacksConfig defaults to the 4 recommended packs, so `diff_json` will treat it as default and the minimal save test still produces `{}`)

**Step 6: Commit**

```bash
git add crates/config/src/schema/packs.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add PacksConfig schema for feature pack system"
```

---

### Task 2: Create Pack Registry

**Files:**
- Create: `crates/cli/src/wizard/packs/mod.rs`
- Create: `crates/cli/src/wizard/packs/registry.rs`

**Step 1: Write the failing test**

In `crates/cli/src/wizard/packs/registry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_packs_returns_7() {
        let packs = PackRegistry::all();
        assert_eq!(packs.len(), 7);
    }

    #[test]
    fn test_core_packs_not_empty() {
        let core = PackRegistry::by_tier(PackTier::Core);
        assert!(!core.is_empty());
        assert!(core.iter().all(|p| p.tier == PackTier::Core));
    }

    #[test]
    fn test_recommended_packs() {
        let rec = PackRegistry::by_tier(PackTier::Recommended);
        assert_eq!(rec.len(), 3); // productivity, ai-intelligence, developer-tools
    }

    #[test]
    fn test_optional_packs() {
        let opt = PackRegistry::by_tier(PackTier::Optional);
        assert_eq!(opt.len(), 3); // finance, weather, skill-creator
    }

    #[test]
    fn test_get_by_id() {
        let pack = PackRegistry::get("productivity");
        assert!(pack.is_some());
        assert_eq!(pack.unwrap().name, "Productivity");
    }

    #[test]
    fn test_get_unknown_returns_none() {
        assert!(PackRegistry::get("nonexistent").is_none());
    }

    #[test]
    fn test_task_management_skills() {
        let pack = PackRegistry::get("task-management").unwrap();
        assert!(pack.skills.contains(&"todo"));
        assert!(pack.skills.contains(&"todo-party"));
        assert!(pack.skills.contains(&"todo-yolo"));
    }

    #[test]
    fn test_skills_for_packs() {
        let enabled = vec!["task-management".to_string(), "developer-tools".to_string()];
        let skills = PackRegistry::skills_for_packs(&enabled);
        assert!(skills.contains(&"todo".to_string()));
        assert!(skills.contains(&"github".to_string()));
        assert!(skills.contains(&"tmux".to_string()));
        assert!(!skills.contains(&"weather".to_string()));
    }

    #[test]
    fn test_default_selection() {
        let selection = PackRegistry::default_selection();
        assert!(selection.contains(&"task-management".to_string()));
        assert!(selection.contains(&"productivity".to_string()));
        assert!(!selection.contains(&"finance".to_string()));
    }

    #[test]
    fn test_all_pack_ids_unique() {
        let packs = PackRegistry::all();
        let mut ids: Vec<&str> = packs.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), packs.len());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cli -E 'test(pack)'`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

Create `crates/cli/src/wizard/packs/mod.rs`:
```rust
//! Feature pack definitions and registry.

pub mod registry;
pub use registry::*;
```

Create `crates/cli/src/wizard/packs/registry.rs`:
```rust
//! Static registry of all feature packs.

use config::PackTier;

/// A feature pack bundles config settings and skills.
#[derive(Debug, Clone)]
pub struct Pack {
    /// Unique identifier (e.g., "productivity").
    pub id: &'static str,
    /// Display name (e.g., "Productivity").
    pub name: &'static str,
    /// Short description for the wizard UI.
    pub description: &'static str,
    /// Selection tier (Core/Recommended/Optional).
    pub tier: PackTier,
    /// Skill names included in this pack.
    pub skills: &'static [&'static str],
}

/// Static pack registry.
pub struct PackRegistry;

impl PackRegistry {
    /// All defined packs, ordered by tier then name.
    pub fn all() -> Vec<&'static Pack> {
        PACKS.iter().collect()
    }

    /// Packs matching a given tier.
    pub fn by_tier(tier: PackTier) -> Vec<&'static Pack> {
        PACKS.iter().filter(|p| p.tier == tier).collect()
    }

    /// Look up a pack by ID.
    pub fn get(id: &str) -> Option<&'static Pack> {
        PACKS.iter().find(|p| p.id == id)
    }

    /// Collect all skill names from a set of enabled pack IDs.
    pub fn skills_for_packs(enabled: &[String]) -> Vec<String> {
        let mut skills = Vec::new();
        for pack in PACKS.iter() {
            if enabled.iter().any(|e| e == pack.id) {
                for skill in pack.skills {
                    let s = skill.to_string();
                    if !skills.contains(&s) {
                        skills.push(s);
                    }
                }
            }
        }
        skills
    }

    /// Default selection: Core + Recommended packs.
    pub fn default_selection() -> Vec<String> {
        PACKS
            .iter()
            .filter(|p| p.tier == PackTier::Core || p.tier == PackTier::Recommended)
            .map(|p| p.id.to_string())
            .collect()
    }
}

static PACKS: &[Pack] = &[
    Pack {
        id: "task-management",
        name: "Task Management",
        description: "Tasks, focus mode, enrichment, semantic search",
        tier: PackTier::Core,
        skills: &["todo", "todo-party", "todo-yolo"],
    },
    Pack {
        id: "productivity",
        name: "Productivity",
        description: "Calendar sync, daily planning, cron, summarize",
        tier: PackTier::Recommended,
        skills: &["daily-planning", "cron", "summarize"],
    },
    Pack {
        id: "ai-intelligence",
        name: "AI Intelligence",
        description: "Conversation memory, learning system, embeddings",
        tier: PackTier::Recommended,
        skills: &[],
    },
    Pack {
        id: "developer-tools",
        name: "Developer Tools",
        description: "GitHub, tmux, shell execution, web search",
        tier: PackTier::Recommended,
        skills: &["github", "tmux"],
    },
    Pack {
        id: "finance",
        name: "Finance",
        description: "Budget tracking, expenses, investment projection",
        tier: PackTier::Optional,
        skills: &["finance"],
    },
    Pack {
        id: "weather",
        name: "Weather",
        description: "Weather queries and forecasts",
        tier: PackTier::Optional,
        skills: &["weather"],
    },
    Pack {
        id: "skill-creator",
        name: "Skill Creator",
        description: "Create custom skills",
        tier: PackTier::Optional,
        skills: &["skill-creator"],
    },
];
```

Add `pub mod packs;` to `crates/cli/src/wizard/mod.rs` (after line 33, `pub mod search;`).

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p cli -E 'test(pack)'`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/cli/src/wizard/packs/ crates/cli/src/wizard/mod.rs
git commit -m "feat(cli): add pack registry with 7 feature packs"
```

---

### Task 3: Create Auto-Detector

**Files:**
- Create: `crates/cli/src/wizard/detect.rs`

The auto-detector scans env vars, existing config, and system probes to pre-fill core setup fields.

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_state_default_has_no_sources() {
        let state = DetectedState::default();
        assert!(state.provider.is_none());
        assert!(state.api_key.is_none());
        assert!(state.database_url.is_none());
        assert!(state.channel.is_none());
    }

    #[test]
    fn test_detect_from_config_with_provider() {
        let mut config = config::Config::default();
        config.providers.anthropic.api_key = config::Secret::new("sk-ant-test".to_string());
        config.agents.defaults.provider = Some("anthropic".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(state.provider, Some(("anthropic".to_string(), DetectSource::Config)));
        assert_eq!(state.api_key.as_ref().map(|(k, _)| k.as_str()), Some("sk-ant-test"));
    }

    #[test]
    fn test_detect_from_config_with_channel() {
        let mut config = config::Config::default();
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = config::Secret::new("bot123".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(state.channel, Some(("telegram".to_string(), DetectSource::Config)));
        assert_eq!(state.channel_token.as_ref().map(|(t, _)| t.as_str()), Some("bot123"));
    }

    #[test]
    fn test_detect_from_config_with_database() {
        let mut config = config::Config::default();
        config.database_url = Some("postgres://custom/db".to_string());

        let state = DetectedState::from_config(&config);
        assert_eq!(
            state.database_url,
            Some(("postgres://custom/db".to_string(), DetectSource::Config))
        );
    }

    #[test]
    fn test_detect_source_display() {
        assert_eq!(format!("{}", DetectSource::Config), "from config");
        assert_eq!(format!("{}", DetectSource::EnvVar), "from env");
        assert_eq!(format!("{}", DetectSource::Detected), "detected");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cli -E 'test(detect)'`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

Create `crates/cli/src/wizard/detect.rs`:

```rust
//! Auto-detection of existing configuration from config file, env vars, and system probes.

use std::fmt;

/// How a value was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectSource {
    /// From existing ~/.klyntbot/config.json
    Config,
    /// From KLYNTBOT_* environment variable
    EnvVar,
    /// From system probe (e.g., pg_isready)
    Detected,
}

impl fmt::Display for DetectSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config => write!(f, "from config"),
            Self::EnvVar => write!(f, "from env"),
            Self::Detected => write!(f, "detected"),
        }
    }
}

/// Pre-detected values for the core setup screen.
#[derive(Debug, Default)]
pub struct DetectedState {
    /// (provider_name, source)
    pub provider: Option<(String, DetectSource)>,
    /// (api_key, source) — raw key value
    pub api_key: Option<(String, DetectSource)>,
    /// (model, source)
    pub model: Option<(String, DetectSource)>,
    /// (database_url, source)
    pub database_url: Option<(String, DetectSource)>,
    /// Whether postgres is running (from pg_isready probe)
    pub pg_running: bool,
    /// (channel_key, source) — e.g., "telegram"
    pub channel: Option<(String, DetectSource)>,
    /// (token, source)
    pub channel_token: Option<(String, DetectSource)>,
}

impl DetectedState {
    /// Build detected state from an existing config.
    pub fn from_config(config: &config::Config) -> Self {
        let mut state = Self::default();

        // Detect provider from config
        let active = config.active_provider_name();
        if active != "none" {
            state.provider = Some((active.to_string(), DetectSource::Config));

            // Get the API key for the active provider
            for (name, pc) in [
                ("anthropic", &config.providers.anthropic),
                ("openai", &config.providers.openai),
                ("openrouter", &config.providers.openrouter),
                ("deepseek", &config.providers.deepseek),
                ("gemini", &config.providers.gemini),
                ("groq", &config.providers.groq),
            ] {
                if name == active && !pc.api_key.is_empty() {
                    state.api_key = Some((pc.api_key.expose().clone(), DetectSource::Config));
                    break;
                }
            }
        }

        // Detect model
        if config.agents.defaults.model != "anthropic/claude-opus-4-5" {
            state.model = Some((config.agents.defaults.model.clone(), DetectSource::Config));
        }

        // Detect database URL
        if let Some(ref url) = config.database_url {
            state.database_url = Some((url.clone(), DetectSource::Config));
        }

        // Detect first enabled channel
        let channels: &[(&str, bool, &str)] = &[
            ("telegram", config.channels.telegram.enabled, config.channels.telegram.token.expose()),
            ("discord", config.channels.discord.enabled, config.channels.discord.token.expose()),
            ("slack", config.channels.slack.enabled, config.channels.slack.bot_token.expose()),
        ];
        for (name, enabled, token) in channels {
            if *enabled && !token.is_empty() {
                state.channel = Some((name.to_string(), DetectSource::Config));
                state.channel_token = Some((token.to_string(), DetectSource::Config));
                break;
            }
        }

        state
    }

    /// Overlay environment variable detections (higher priority than config).
    pub fn overlay_env_vars(&mut self) {
        // Provider API keys from env
        let env_providers = [
            ("KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY", "anthropic"),
            ("KLYNTBOT_PROVIDERS__OPENAI__API_KEY", "openai"),
            ("KLYNTBOT_PROVIDERS__OPENROUTER__API_KEY", "openrouter"),
            ("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY", "deepseek"),
            ("KLYNTBOT_PROVIDERS__GEMINI__API_KEY", "gemini"),
            ("KLYNTBOT_PROVIDERS__GROQ__API_KEY", "groq"),
        ];
        for (var, name) in env_providers {
            if let Ok(key) = std::env::var(var) {
                if !key.is_empty() {
                    self.provider = Some((name.to_string(), DetectSource::EnvVar));
                    self.api_key = Some((key, DetectSource::EnvVar));
                    break; // Use the first found
                }
            }
        }

        // Database URL from env
        if let Ok(url) = std::env::var("KLYNTBOT_DATABASE_URL") {
            if !url.is_empty() {
                self.database_url = Some((url, DetectSource::EnvVar));
            }
        }

        // Channel tokens from env
        let env_channels = [
            ("KLYNTBOT_CHANNELS__TELEGRAM__TOKEN", "telegram"),
            ("KLYNTBOT_CHANNELS__DISCORD__TOKEN", "discord"),
            ("KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN", "slack"),
        ];
        for (var, name) in env_channels {
            if let Ok(token) = std::env::var(var) {
                if !token.is_empty() {
                    self.channel = Some((name.to_string(), DetectSource::EnvVar));
                    self.channel_token = Some((token, DetectSource::EnvVar));
                    break;
                }
            }
        }
    }

    /// Probe system for running PostgreSQL (fills pg_running and default DB URL).
    pub fn probe_postgres(&mut self) {
        if self.database_url.is_some() {
            return; // Already have a URL, skip probe
        }

        // Check if pg_isready succeeds
        let pg_running = std::process::Command::new("pg_isready")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        self.pg_running = pg_running;
        if pg_running {
            self.database_url = Some((
                "postgresql://localhost/klyntbot".to_string(),
                DetectSource::Detected,
            ));
        }
    }
}
```

Add `pub mod detect;` to `crates/cli/src/wizard/mod.rs`.

**Step 4: Run tests**

Run: `cargo nextest run -p cli -E 'test(detect)'`
Expected: All PASS

**Step 5: Commit**

```bash
git add crates/cli/src/wizard/detect.rs crates/cli/src/wizard/mod.rs
git commit -m "feat(cli): add auto-detector for smart core setup pre-filling"
```

---

### Task 4: Create Core Setup Phase

**Files:**
- Create: `crates/cli/src/wizard/core_setup.rs`

This is the single-screen Phase 1: provider + database + channel. Reuses existing prompt utilities from `crates/cli/src/wizard/prompts/` and detection logic from `crates/cli/src/wizard/steps/database.rs` (PostgreSQL install/start helpers).

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_list_matches_config() {
        // All entries in PROVIDER_INFO should have a valid key
        for info in PROVIDER_INFO {
            assert!(!info.key.is_empty());
            assert!(!info.name.is_empty());
        }
    }

    #[test]
    fn test_channel_list() {
        assert!(CHANNEL_INFO.len() >= 4); // telegram, discord, slack, whatsapp minimum
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cli -E 'test(provider_list)' -E 'test(channel_list)'`
Expected: FAIL

**Step 3: Write implementation**

Create `crates/cli/src/wizard/core_setup.rs`. This is the largest new file — it renders the single-screen core setup with 3 sections (provider, database, channel), using the existing `prompts` module for input and the `detect` module for pre-filling. Key functions:

- `pub async fn run_core_setup(state: &mut WizardState) -> Result<StepResult>` — orchestrates the full Phase 1
- `fn render_summary(detected: &DetectedState)` — prints the auto-detected summary
- `fn prompt_provider(state: &mut WizardState, detected: &DetectedState) -> Result<()>` — provider + key + model
- `async fn prompt_database(state: &mut WizardState, detected: &DetectedState) -> Result<()>` — DB URL + connection test (reuses `steps::database` helpers for PostgreSQL install)
- `fn prompt_channel(state: &mut WizardState, detected: &DetectedState) -> Result<()>` — channel radio + token
- `async fn validate_core(state: &WizardState) -> Result<Vec<String>>` — test API key, test DB, validate token

The implementation should import and reuse:
- `crate::wizard::prompts::{prompt_text, prompt_secret, prompt_select, prompt_yes_no}`
- `crate::wizard::detect::{DetectedState, DetectSource}`
- `crate::wizard::provider::detection::PROVIDERS` (provider metadata)
- `crate::wizard::steps::database::{find_pg_binary, detect_status, ...}` (PostgreSQL helpers — make these `pub(crate)` if not already)
- `common::utils::terminal::*` (colorize, status icons, etc.)

Provider and channel metadata constants:

```rust
pub struct ProviderInfo {
    pub key: &'static str,
    pub name: &'static str,
}

pub static PROVIDER_INFO: &[ProviderInfo] = &[
    ProviderInfo { key: "anthropic", name: "Anthropic" },
    ProviderInfo { key: "openai", name: "OpenAI" },
    ProviderInfo { key: "openrouter", name: "OpenRouter" },
    ProviderInfo { key: "deepseek", name: "DeepSeek" },
    ProviderInfo { key: "gemini", name: "Google Gemini" },
    ProviderInfo { key: "groq", name: "Groq" },
    ProviderInfo { key: "vllm", name: "vLLM" },
    ProviderInfo { key: "zhipu", name: "Zhipu" },
    ProviderInfo { key: "dashscope", name: "DashScope" },
    ProviderInfo { key: "moonshot", name: "Moonshot" },
    ProviderInfo { key: "minimax", name: "MiniMax" },
    ProviderInfo { key: "aihubmix", name: "AIHubMix" },
];

pub struct ChannelInfo {
    pub key: &'static str,
    pub name: &'static str,
}

pub static CHANNEL_INFO: &[ChannelInfo] = &[
    ChannelInfo { key: "telegram", name: "Telegram" },
    ChannelInfo { key: "discord", name: "Discord" },
    ChannelInfo { key: "slack", name: "Slack" },
    ChannelInfo { key: "whatsapp", name: "WhatsApp" },
    ChannelInfo { key: "email", name: "Email" },
    ChannelInfo { key: "qq", name: "QQ" },
];
```

The `run_core_setup` flow:
1. Build `DetectedState` from config → overlay env vars → probe postgres
2. Render summary showing detected values
3. Prompt for any missing/editable fields (provider, key, DB URL, channel, token)
4. Validate (connection tests)
5. Save to `state.config`
6. Return `StepResult::Next`

Add `pub mod core_setup;` to `crates/cli/src/wizard/mod.rs`.

**Step 4: Run tests**

Run: `cargo nextest run -p cli -E 'test(provider_list)' -E 'test(channel_list)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cli/src/wizard/core_setup.rs crates/cli/src/wizard/mod.rs
git commit -m "feat(cli): add Phase 1 core setup (provider + database + channel)"
```

---

### Task 5: Create Pack Selection Phase

**Files:**
- Create: `crates/cli/src/wizard/pack_selection.rs`

This renders the Phase 2 checklist: shows all packs grouped by tier, lets user toggle with spacebar, confirms with enter. Uses the existing `prompts::prompt_multi_select_with_defaults` pattern.

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pack_options_returns_all_packs() {
        let options = build_pack_options(&[]);
        assert_eq!(options.len(), 7);
    }

    #[test]
    fn test_build_pack_options_default_selection() {
        let defaults = config::PacksConfig::default().enabled;
        let options = build_pack_options(&defaults);

        // Core packs should be marked as locked
        let task = options.iter().find(|o| o.id == "task-management").unwrap();
        assert!(task.locked);

        // Recommended should be pre-checked
        let prod = options.iter().find(|o| o.id == "productivity").unwrap();
        assert!(prod.checked);

        // Optional should not be checked
        let weather = options.iter().find(|o| o.id == "weather").unwrap();
        assert!(!weather.checked);
    }

    #[test]
    fn test_apply_pack_selection_enables_config_sections() {
        let mut config = config::Config::default();
        let selection = vec!["task-management".to_string(), "ai-intelligence".to_string()];

        apply_pack_config(&mut config, &selection);

        // AI Intelligence enables conversation + learning
        assert!(config.conversation.embedding.enabled);
        assert!(config.learning.enabled);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cli -E 'test(build_pack)' -E 'test(apply_pack)'`
Expected: FAIL

**Step 3: Write implementation**

Create `crates/cli/src/wizard/pack_selection.rs`:

Key functions:
- `pub fn run_pack_selection(state: &mut WizardState) -> Result<StepResult>` — renders checklist, collects selection, applies to config
- `pub fn build_pack_options(enabled: &[String]) -> Vec<PackOption>` — builds the UI list with checked/locked state
- `pub fn apply_pack_config(config: &mut Config, enabled_packs: &[String])` — applies config mutations for each enabled pack

`apply_pack_config` maps pack IDs to config mutations:
- `"task-management"` → `todo.enrichment.enabled = true`, `todo.search.enabled = true`
- `"productivity"` → `calendar` defaults, `todo.daily_planning.enabled = true`, `todo.notifications.daily_digest = true`
- `"ai-intelligence"` → `conversation.embedding.enabled = true`, `conversation.search.enabled = true`, `learning.enabled = true`
- `"developer-tools"` → `tools.restrict_to_workspace = true` (balanced preset)
- `"finance"` → `finance.budget.enabled = true` (if applicable)
- `"weather"` → no config changes (skill only)
- `"skill-creator"` → no config changes (skill only)

When a pack is *not* in the selection, disable its config sections:
- Missing `"ai-intelligence"` → `conversation.embedding.enabled = false`, `learning.enabled = false`
- Missing `"productivity"` → `todo.daily_planning.enabled = false`

Render the checklist using the existing terminal primitives:
- `[■]` for locked core packs (cannot toggle)
- `[✓]` for checked packs
- `[ ]` for unchecked packs
- Arrow keys to navigate, spacebar to toggle, enter to confirm
- Uses `crossterm` raw mode (same pattern as existing menus)

Add `pub mod pack_selection;` to `crates/cli/src/wizard/mod.rs`.

**Step 4: Run tests**

Run: `cargo nextest run -p cli -E 'test(build_pack)' -E 'test(apply_pack)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cli/src/wizard/pack_selection.rs crates/cli/src/wizard/mod.rs
git commit -m "feat(cli): add Phase 2 pack selection with config mutations"
```

---

### Task 6: Add CLI Flags for Init

**Files:**
- Modify: `crates/cli/src/commands.rs:45-46`
- Modify: `src/main.rs:34,76-82`

**Step 1: Write the failing test**

```rust
// In crates/cli/src/commands.rs tests:
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_init_packs_flag() {
        let cli = Cli::try_parse_from(["klyntbot", "init", "--packs"]).unwrap();
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(packs);
                assert!(!reset);
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn test_init_reset_flag() {
        let cli = Cli::try_parse_from(["klyntbot", "init", "--reset"]).unwrap();
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(!packs);
                assert!(reset);
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn test_init_no_flags() {
        let cli = Cli::try_parse_from(["klyntbot", "init"]).unwrap();
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(!packs);
                assert!(!reset);
            }
            _ => panic!("expected Init"),
        }
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cli -E 'test(init_packs)' -E 'test(init_reset)' -E 'test(init_no_flags)'`
Expected: FAIL — Init variant has no fields

**Step 3: Write implementation**

In `crates/cli/src/commands.rs`, change the `Init` variant (line 46):

```rust
    /// Initialize klyntbot configuration and workspace
    Init {
        /// Jump directly to pack selection
        #[arg(long)]
        packs: bool,

        /// Reset configuration to defaults before running wizard
        #[arg(long)]
        reset: bool,
    },
```

In `src/main.rs`, update the dispatch (line 34):

```rust
        Some(Commands::Init { packs, reset }) => handle_init(packs, reset).await,
```

And the handler (lines 76-83):

```rust
async fn handle_init(packs: bool, reset: bool) -> anyhow::Result<()> {
    use cli::run_wizard;

    run_wizard(packs, reset).await?;

    Ok(())
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p cli -E 'test(init_)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cli/src/commands.rs src/main.rs
git commit -m "feat(cli): add --packs and --reset flags to init command"
```

---

### Task 7: Rewrite Wizard Orchestrator

**Files:**
- Modify: `crates/cli/src/wizard/mod.rs` (major rewrite of `run_wizard`)

**Step 1: Write the failing test**

The existing tests in `crates/cli/src/wizard/mod.rs:339` (`mod tests;`) need to still compile. Check what's there:

Run: `cargo nextest run -p cli -E 'test(wizard)' --no-run`
Expected: Compilation succeeds (we haven't changed the orchestrator yet)

**Step 2: Rewrite `run_wizard` to accept flags and use 2-phase flow**

Update `run_wizard` signature and body in `crates/cli/src/wizard/mod.rs`:

```rust
/// Run the interactive onboarding wizard.
///
/// - `packs_only`: Skip core setup, jump directly to pack selection.
/// - `reset`: Wipe config to defaults before starting.
pub async fn run_wizard(packs_only: bool, reset: bool) -> Result<()> {
    print_wizard_header();

    let mut state = if reset {
        WizardState::fresh()
    } else {
        WizardState::new()
    };

    if !packs_only {
        // Phase 1: Smart Core Setup
        print_phase_header(1, 2, "Core Setup");
        match core_setup::run_core_setup(&mut state).await {
            Ok(StepResult::Cancel) => {
                println!("\n{} Setup cancelled.", status_warning());
                return Ok(());
            }
            Ok(_) => {
                config::save(&state.config).await?;
            }
            Err(ref e) if e.to_string().contains("Ctrl+C") => {
                println!("\n{} Setup cancelled.", status_warning());
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }

    // Phase 2: Pack Selection
    print_phase_header(if packs_only { 1 } else { 2 }, if packs_only { 1 } else { 2 }, "Feature Packs");
    match pack_selection::run_pack_selection(&mut state) {
        Ok(StepResult::Cancel) => {
            println!("\n{} Setup cancelled.", status_warning());
            return Ok(());
        }
        Ok(StepResult::Back) if !packs_only => {
            // Go back to Phase 1 — recurse without packs_only
            return Box::pin(run_wizard(false, false)).await;
        }
        Ok(_) => {
            config::save(&state.config).await?;
        }
        Err(ref e) if e.to_string().contains("Ctrl+C") => {
            println!("\n{} Setup cancelled.", status_warning());
            return Ok(());
        }
        Err(e) => return Err(e),
    }

    print_completion();
    Ok(())
}

fn print_phase_header(phase: usize, total: usize, name: &str) {
    println!();
    println!(
        "  {} Phase {} of {} — {}",
        colorize("●", BRAND),
        phase,
        total,
        colorize(name, BOLD),
    );
    println!();
}
```

Add `WizardState::fresh()` to `framework.rs`:

```rust
    /// Create a fresh wizard state with default config (ignoring existing).
    pub fn fresh() -> Self {
        Self {
            config: Config::default(),
            total_steps: 0,
            current_step: 0,
            is_fresh_install: true,
        }
    }
```

**Important:** Keep the old wizard modules (`provider/`, `channels/`, `steps/`, etc.) compiled but no longer called from `run_wizard`. They remain available for pack-specific credential prompts (e.g., calendar configuration when Productivity pack is selected). They will be cleaned up in a later task.

Remove the old adapter structs (`ToolsModule`, `WorkspaceModule`) and the 11-step dispatch logic from `run_wizard`. Keep the helper functions (`run_channels_step`, `run_calendar_step`, etc.) as they may be called from pack credential prompts.

**Step 3: Run compilation check**

Run: `cargo build -p cli`
Expected: Compiles (may have warnings for unused old modules — that's fine)

**Step 4: Run existing tests**

Run: `cargo nextest run -p cli`
Expected: PASS (old tests still work, new orchestrator compiles)

**Step 5: Commit**

```bash
git add crates/cli/src/wizard/mod.rs crates/cli/src/wizard/framework.rs
git commit -m "feat(cli): rewrite wizard orchestrator to 2-phase flow"
```

---

### Task 8: Update SkillManager for Pack Filtering

**Files:**
- Modify: `crates/agent/src/skills.rs:86-110`

**Step 1: Write the failing test**

Add to `crates/agent/src/skills.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ... existing tests ...

    #[test]
    fn test_filter_by_packs_includes_matching() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        let enabled_skills = vec!["todo".to_string(), "github".to_string()];
        mgr.filter_by_skills(&enabled_skills);

        assert!(mgr.get("todo").is_some());
        assert!(mgr.get("github").is_some());
        assert!(mgr.get("weather").is_none()); // Not in enabled list
    }

    #[test]
    fn test_filter_by_packs_empty_keeps_nothing() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        mgr.filter_by_skills(&[]);

        assert_eq!(mgr.all().len(), 0);
    }

    #[test]
    fn test_filter_by_packs_all_skills_keeps_all() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();
        let total = mgr.all().len();

        let all_names: Vec<String> = mgr.all().iter().map(|s| s.name.clone()).collect();
        mgr.filter_by_skills(&all_names);

        assert_eq!(mgr.all().len(), total);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(filter_by_packs)'`
Expected: FAIL — method `filter_by_skills` not found

**Step 3: Write implementation**

Add to `SkillManager` in `crates/agent/src/skills.rs` (after the `all()` method, ~line 302):

```rust
    /// Filter loaded skills to only those in the allowed list.
    ///
    /// Used after `load()` to restrict skills to those from enabled packs.
    /// Workspace-loaded skills (non-builtin) are always kept.
    pub fn filter_by_skills(&mut self, allowed_skill_names: &[String]) {
        self.skills.retain(|name, skill| {
            // Keep workspace skills (not builtin) regardless
            if !skill.path.to_string_lossy().starts_with("builtin::") {
                return true;
            }
            // Keep if name is in the allowed list
            allowed_skill_names.iter().any(|a| a == name)
        });
    }
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(filter_by_packs)' -E 'test(filter_by_skills)'`
Expected: PASS

**Step 5: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: All PASS

**Step 6: Commit**

```bash
git add crates/agent/src/skills.rs
git commit -m "feat(agent): add pack-based skill filtering to SkillManager"
```

---

### Task 9: Wire SkillManager Filtering Into Agent Startup

**Files:**
- Modify: wherever `SkillManager::load()` is called (likely `crates/agent/src/agent_loop.rs` or similar)

**Step 1: Find the caller**

Run: `cargo grep "skill_manager.load\|SkillManager::new" --include="*.rs"`

Read the file(s) that call `SkillManager::load()`. After the `load()` call, add pack filtering:

```rust
// After skill_manager.load(workspace_path).await?;
let enabled_skills = config::packs::PackRegistry::skills_for_packs(&config.packs.enabled);
// Wait — PackRegistry is in cli crate, not config. We need to use the pack data from config.

// Alternative: SkillManager gets a method that accepts the enabled pack IDs directly,
// and internally maps them to skill names using a shared mapping.
// But the mapping lives in cli::wizard::packs::PackRegistry...
//
// Better approach: the skills_for_packs logic is simple enough to duplicate,
// or we move PackRegistry to the config crate (since it's just data).
```

**Decision point:** The `PackRegistry` is currently in the `cli` crate (Layer 6), but the `SkillManager` is in `agent` (Layer 5). Since `agent` cannot depend on `cli`, we have two options:

**Option A:** Move `PackRegistry` to the `config` crate (Layer 1) — it's just static data with no UI dependencies. This is clean.

**Option B:** Store the list of enabled skill names directly in `PacksConfig` (computed during wizard and saved to config.json). The agent just reads `config.packs.enabled_skills` without needing to know about pack definitions.

**Go with Option B** — it's simpler and doesn't couple the agent to pack definitions. The wizard computes the skill list when packs are selected.

Add to `PacksConfig` in `crates/config/src/schema/packs.rs`:

```rust
pub struct PacksConfig {
    pub enabled: Vec<String>,
    /// Skill names derived from enabled packs (computed by wizard, saved to config).
    #[serde(default)]
    pub enabled_skills: Vec<String>,
}
```

Update `pack_selection.rs` to populate `config.packs.enabled_skills` when saving:

```rust
config.packs.enabled_skills = PackRegistry::skills_for_packs(&config.packs.enabled);
```

Then in the agent startup, after `skill_manager.load()`:

```rust
if !config.packs.enabled_skills.is_empty() {
    skill_manager.filter_by_skills(&config.packs.enabled_skills);
}
```

**Step 2: Implement and test**

Run: `cargo nextest run -p config -E 'test(packs)'`
Expected: PASS (update the default test to include empty `enabled_skills`)

**Step 3: Commit**

```bash
git add crates/config/src/schema/packs.rs crates/agent/src/skills.rs crates/cli/src/wizard/pack_selection.rs
git commit -m "feat: wire pack-based skill filtering into agent startup"
```

---

### Task 10: Integration Testing

**Files:**
- Modify: existing integration tests or create new ones

**Step 1: Verify full build compiles**

Run: `cargo build --workspace`
Expected: Compiles with 0 errors

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any new warnings)

**Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

**Step 4: Run formatting check**

Run: `cargo fmt --all --check`
Expected: Clean

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve clippy warnings and test failures from init redesign"
```

---

### Task 11: Clean Up Old Wizard Modules

**Files:**
- Modify: `crates/cli/src/wizard/mod.rs` — remove unused imports and dead code
- Potentially remove or deprecate: `search/`, `memory/`, `learning/`, `daemon/` wizard modules if they're no longer called

**Important:** Don't delete modules that are still needed for pack credential prompts (e.g., `calendar/` for CalDAV setup, `channels/` for OAuth flows, `steps/database.rs` for PostgreSQL helpers). Only remove modules that are fully absorbed into packs with no remaining callers.

**Step 1: Check for unused modules**

Run: `cargo clippy --workspace --all-targets 2>&1 | grep "unused"` to find dead code.

**Step 2: Remove unused code**

Remove modules flagged as unused. Keep:
- `prompts/` — still used by core_setup and pack_selection
- `ui/` — still used
- `framework.rs` — still used (WizardState, StepResult, print_step_header)
- `templates.rs` — may still be used by workspace setup
- `provider/detection.rs` — used by core_setup
- `steps/database.rs` — used by core_setup for PostgreSQL helpers
- `channels/` — may be used for pack credential prompts
- `calendar/` — may be used for pack credential prompts
- `oauth.rs` — used by channels

**Step 3: Run all tests again**

Run: `cargo nextest run --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: All PASS, 0 warnings

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor(cli): remove unused wizard modules after init redesign"
```

---

### Task 12: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

Update the CLI Subcommands section to document the new `--packs` and `--reset` flags:

```bash
klyntbot init                    # Interactive setup wizard (2-phase: core + packs)
klyntbot init --packs            # Jump to pack selection only
klyntbot init --reset            # Reset config to defaults, then run wizard
```

Add a new section about Feature Packs:

```markdown
## Feature Packs

Packs bundle config settings and skills into selectable units. Managed via `klyntbot init`.

| Pack | Tier | Skills |
|------|------|--------|
| Task Management | Core | todo, todo-party, todo-yolo |
| Productivity | Recommended | daily-planning, cron, summarize |
| AI Intelligence | Recommended | (engine features) |
| Developer Tools | Recommended | github, tmux |
| Finance | Optional | finance |
| Weather | Optional | weather |
| Skill Creator | Optional | skill-creator |

Pack selection is stored in `config.packs.enabled`. Skills from enabled packs are filtered by `SkillManager::filter_by_skills()`.
```

**Step 1: Make the edits**

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with feature packs and new init flags"
```
