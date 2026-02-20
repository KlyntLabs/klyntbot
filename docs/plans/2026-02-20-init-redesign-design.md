# Init & Configuration Redesign

**Date:** 2026-02-20
**Status:** Approved

## Problem

The current `klyntbot init` wizard has 11 sequential steps with nested sub-action menus. Only 3 steps are required, but new users must navigate all 11 linearly. Skills are bundled at compile-time with no selection mechanism. This makes onboarding slow and overwhelming.

## Design: Two-Phase Wizard with Unified Packs

Replace the 11-step wizard with a 2-phase flow: smart core setup, then pack selection.

### Phase 1: Smart Core Setup

A single screen that auto-detects and pre-fills configuration.

**Auto-detection sources (in priority order):**
1. Existing `~/.klyntbot/config.json`
2. Environment variables (`KLYNTBOT_PROVIDERS__*__API_KEY`, `KLYNTBOT_DATABASE_URL`, channel tokens)
3. System probes (PostgreSQL via `pg_isready`, Homebrew/apt paths for Postgres binaries)

**Screen layout:**

```
╭─ klyntbot setup ─────────────────────────────────────╮
│                                                       │
│  ① LLM Provider                                      │
│     Provider:  [Anthropic ▾]                          │
│     API Key:   sk-ant-••••••••7x3m  ✓ (from env)     │
│     Model:     claude-opus-4-5                        │
│                                                       │
│  ② Database                                           │
│     URL:  postgres://localhost/klyntbot  ✓ (detected) │
│                                                       │
│  ③ Channel                                            │
│     ● Telegram   ○ Discord   ○ Slack   ○ WhatsApp     │
│     Token: [                                   ]      │
│                                                       │
│  [ Continue → ]                                       │
╰───────────────────────────────────────────────────────╯
```

**Behavior:**
- Detected values show `✓` with source label
- Missing values show empty fields for user input
- Provider is a dropdown covering all 12 supported providers
- Channel is a radio group (pick one primary channel)
- "Continue" validates: tests API key connectivity, tests DB connection, validates token format
- Validation failures highlight inline with error messages
- If PostgreSQL is not installed/running, offer automated install + start

**Edge cases:**
- Existing config: all pre-filled, user just confirms
- Env vars set: pre-fill from env
- Fresh install: all empty, user fills 3 things (key, DB URL, channel token)

### Phase 2: Unified Pack Selection

Packs bundle both config settings and related skills into a single selectable unit.

**Pack tiers:**
- **Core** — always enabled, cannot be unchecked
- **Recommended** — pre-checked, user can uncheck
- **Optional** — unchecked by default, user opts in

**Pack definitions:**

| Pack | Tier | Config Sections | Skills |
|------|------|----------------|--------|
| Task Management | Core | `todo.*` (enrichment, search, focus, notifications) | `todo`, `todo-party`, `todo-yolo` |
| Productivity | Recommended | `calendar.*`, `todo.dailyPlanning`, `todo.notifications` | `daily-planning`, `cron`, `summarize` |
| AI Intelligence | Recommended | `conversation.*`, `learning.*`, `todo.search.enabled` | *(engine features, no separate skills)* |
| Developer Tools | Recommended | `tools.exec.*`, `tools.web.*` | `github`, `tmux` |
| Finance | Optional | `finance.*` | `finance` |
| Weather | Optional | *(none)* | `weather` |
| Skill Creator | Optional | *(none)* | `skill-creator` |

**Screen layout:**

```
╭─ Feature Packs ──────────────────────────────────────╮
│                                                       │
│  Select which feature packs to install.               │
│  Use [space] to toggle, [enter] to confirm.           │
│                                                       │
│  [■] Task Management (core)                           │
│  [■] Productivity                          recommended │
│  [■] AI Intelligence                       recommended │
│  [■] Developer Tools                       recommended │
│  [ ] Finance                                           │
│  [ ] Weather                                           │
│  [ ] Skill Creator                                     │
│                                                       │
│  [ Install Selected → ]                               │
╰───────────────────────────────────────────────────────╯
```

**Behavior:**
- Selecting a pack applies its config defaults and registers its skills
- Unchecking removes config overrides (reverts to disabled defaults) and unregisters skills
- Packs that need credentials (e.g., Productivity with calendar) prompt inline when checked
- Tool permission preset (strict/balanced/permissive) is a field within Developer Tools pack

### Post-Init Management

```bash
klyntbot init              # Full wizard (re-detects, pre-fills from saved config)
klyntbot init --packs      # Jump directly to pack selection
klyntbot init --reset      # Wipe config, start fresh
```

### Architecture Changes

**New data structures:**

```rust
// Pack definition (in cli crate)
pub struct Pack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: PackTier,               // Core, Recommended, Optional
    pub config_sections: Vec<ConfigSection>,
    pub skills: Vec<String>,
    pub requires_credentials: Vec<CredentialPrompt>,
}

pub enum PackTier { Core, Recommended, Optional }

// Added to Config (in config crate)
pub struct PacksConfig {
    pub enabled: Vec<String>,
}
```

**What gets removed:**

| Current | Becomes |
|---------|---------|
| 11 WizardModule implementations | 2 functions: `run_core_setup()`, `run_pack_selection()` |
| WizardRunner with step navigation | Linear 2-phase flow |
| Sub-action menus per step | Inline fields |
| `steps/welcome.rs`, `steps/review.rs` | Auto-detect summary + phase confirm |
| `search/`, `memory/`, `learning/`, `calendar/`, `workspace/`, `daemon/` modules | Pack definitions + auto-defaults |
| `tools/mod.rs` wizard | Preset field in Developer Tools pack |

**What stays:**
- Full `Config` schema (all fields remain — packs toggle them)
- `Secret<T>` handling
- Environment variable overrides
- Minimal JSON diff on save
- `config_path()` / `config_dir()` / `load()` / `save()`
- `SkillManager` (gains `register_packs()` method)

**Framework simplification:**
- `WizardModule` trait and `WizardRunner` removed
- Replaced by direct functions: `run_core_setup(&mut WizardState)` and `run_pack_selection(&mut WizardState)`
- `WizardState` retained for holding mutable `Config` during setup

**Skill registration flow:**
- `SkillManager::register_packs(enabled_packs)` filters built-in skills to only load those from enabled packs
- Custom workspace skills still override/extend as before
- Skills not in any enabled pack are not injected into the system prompt
