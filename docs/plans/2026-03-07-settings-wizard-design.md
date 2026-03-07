# Settings & Onboarding Wizard Design

**Date:** 2026-03-07
**Status:** Approved

## Overview

Replace all placeholder "coming soon" settings pages with real implementations, and add a first-run onboarding wizard that guides new users through system configuration. Breaking changes are acceptable — optimize for long-term architecture.

## Architecture: Config-Centric Wizard

The wizard writes directly to the `Config` struct (via generic config commands) for config-level settings, and uses existing entity CRUD commands for database entities (accounts, areas, goals, etc.). The wizard is a React route (`/setup`) that the app redirects to on first launch.

### New Backend Commands (3 total)

```
config_get_section(section: String) → JSON value
config_update_section(section: String, patch: JSON) → JSON value
app_info() → { version, dataDir, setupCompleted }
```

- `config_get_section` reads any top-level config key (e.g., `"agents"`, `"providers"`, `"finance"`) and returns its current JSON value.
- `config_update_section` does a deep merge of the patch into the existing section, saves to disk, and returns the updated section. Frontend sends only changed fields.
- `app_info` returns app metadata including version (from `Cargo.toml`), resolved data directory path, and setup completion status.

### Deep Merge Strategy

```rust
fn deep_merge(base: &mut Value, patch: Value) {
    // Objects: recursively merge keys
    // Arrays: replace entirely (not append)
    // Scalars: overwrite
    // null value: remove key (explicit deletion)
}
```

## Schema Changes

### New field on `Config`

```rust
#[serde(default)]
pub setup_completed: bool,
```

### New FIRE config on `FinanceConfig`

```rust
#[serde(default)]
pub fire: FireConfig,
```

```rust
pub struct FireConfig {
    pub enabled: bool,                    // default false
    pub current_age: Option<u32>,
    pub target_retirement_age: Option<u32>,
    pub annual_expenses: Option<i64>,     // cents
    pub safe_withdrawal_rate: f64,        // default 4.0
    pub fire_type: String,                // "lean" | "regular" | "fat" | "coast"
    pub target_number: Option<i64>,       // auto-calculated or manual override
    pub monthly_savings_rate: Option<i64>, // cents
    pub current_net_worth: Option<i64>,   // snapshot, cents
}
```

### Default Finance Categories

Rich default list seeded on first finance setup:

- **Income:** Salary, Freelance, Investments, Rental, Side Business, Gifts, Refunds
- **Essential:** Housing, Utilities, Groceries, Transportation, Insurance, Healthcare, Debt Payments
- **Lifestyle:** Dining Out, Entertainment, Shopping, Personal Care, Subscriptions, Travel, Fitness
- **Savings:** Emergency Fund, Retirement, Investments, Education Fund
- **Giving:** Charity, Gifts, Family Support

## Wizard Flow

### Route Structure

```
/setup              → redirects to /setup/welcome
/setup/welcome      → Step 1: Welcome
/setup/provider     → Step 2: Provider & Model (required)
/setup/channels     → Step 3: Channels (optional)
/setup/areas        → Step 4: Areas & Projects (optional)
/setup/productivity → Step 5: Productivity (optional)
/setup/finance      → Step 6: Finance (optional, multi-sub-step)
/setup/mcp          → Step 7: MCP Servers (optional)
/setup/complete     → Done screen with "Launch Klynt" button
```

### Navigation Behavior

- Progress bar at the top showing current step out of total
- Back button always available (goes to previous step)
- Skip button on optional steps (advances without saving)
- Continue/Save & Continue submits immediately via Tauri commands, then advances
- Going back shows already-saved data (loaded from config/DB), re-submitting overwrites
- No final "confirm all" — every step is immediately persisted

### First-Run Detection

```
App launch → app_info() → if !setupCompleted → redirect /setup/welcome
                        → if setupCompleted  → normal app /
```

Sets `setup_completed: true` when user clicks "Launch Klynt" on the final screen.

### Finance Sub-Steps (Step 6)

```
6a. Basics       → currency, proactivity level
6b. Accounts     → add bank accounts with real balances
6c. Income       → monthly income, budgeting method, category budgets
6d. FIRE Profile → age, target retirement, expenses, SWR, FIRE type
6e. Investments  → create portfolio, add holdings
6f. Liabilities  → loans, mortgages, credit card debt
6g. Goals        → financial goals with deadlines
```

Each sub-step has its own Skip/Continue. Default categories shown as pre-populated checkboxes (add/remove/rename).

### Wizard Layout

- Centered card layout (max-width ~640px)
- Glassmorphism background consistent with app theme
- Step title + description at top
- Form fields below
- Navigation buttons at bottom (Back | Skip | Continue)

## Settings Pages (Post-Wizard)

Wizard step forms and settings page forms share the same components (e.g., `ProviderForm`, `FinanceBasicsForm`, `AccountsForm`). Wizard wraps them in stepper layout; settings wraps them in collapsible cards.

### GeneralSettings (update existing)

- System info: version and data dir from `app_info()` (no more hardcoded values)
- Agent defaults: model, provider, temperature, max_tokens, max_tool_iterations
- Timezone: editable dropdown
- Permissions card: unchanged

### ConfigurationSettings (replace placeholder)

Three collapsible sections:
- **Channels:** 9 channels, each with enable toggle + config fields. Detail fields shown only when enabled.
- **Tools:** restrict_to_workspace, browser config, web tools (Brave API key)
- **Gateway:** host + port

### PersonalizationSettings (replace placeholder)

- **Provider & Model:** Primary provider, API key (masked), model, extended thinking. Fallback provider.
- **Learning:** enabled toggle, analysis interval, threshold sliders
- **Cognitive:** model override, temperature, reflection schedule

### Finance in Settings

All finance wizard sub-steps available as tabs/collapsible sections. FIRE dashboard with calculated number and projected timeline. Category management.

### Git & Environments

Remain "Coming soon" — no config schema exists, genuinely future features.

### ArchivedSettings

Wire to existing `chat_threads` with archive filter. Add archive/restore using existing chat commands.

## Data Flow

### Config Operations

```
Frontend → config_get_section("finance") → handler → config.read() → return JSON
Frontend → config_update_section("finance", patch) → handler → deep_merge → config::save() → disk
```

### Entity Operations

Use existing CRUD commands unchanged: `finance_account_create`, `area_create`, `finance_goal_create`, etc.

### Security

- API keys displayed as masked input (••••••••sk-1234)
- Full key visible on explicit "Show" toggle
- Stored via `Secret<String>` in config

## What's NOT in Scope

- Provider API key validation (ping test) — future enhancement
- Git integration settings — no config schema
- Environment variable management — no config schema
- New Tauri commands for entity CRUD — already exist
