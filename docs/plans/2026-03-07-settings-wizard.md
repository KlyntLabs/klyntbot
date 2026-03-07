# Settings & Onboarding Wizard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all placeholder settings pages with real config UI, add a first-run onboarding wizard, and extend the finance config with FIRE support.

**Architecture:** Generic config read/write commands (`config_get_section`, `config_update_section`, `app_info`) in app-core, exposed via Tauri commands and dev server. Wizard is a frontend-only flow at `/setup/*` that calls these commands + existing entity CRUD. Shared form components between wizard and settings pages.

**Tech Stack:** Rust (config schema + handlers), TypeScript/React (wizard + settings UI), Tailwind v4 + CSS tokens, react-router hash routing.

---

### Task 1: Add FIRE Config Schema & `setup_completed` Flag

**Files:**
- Modify: `crates/config/src/schema/finance.rs` (append after line 307)
- Modify: `crates/config/src/schema/core.rs:84-150` (add field to Config struct)
- Modify: `crates/config/src/lib.rs` (add re-export)
- Modify: `crates/config/src/schema/mod.rs:63` (update default model test)

**Step 1: Add `FireConfig` struct to `finance.rs`**

Append to `crates/config/src/schema/finance.rs`:

```rust
/// FIRE (Financial Independence, Retire Early) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FireConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_age: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_retirement_age: Option<u32>,
    /// Annual expenses in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annual_expenses: Option<i64>,
    /// Safe withdrawal rate as percentage (default: 4.0).
    #[serde(default = "default_swr")]
    pub safe_withdrawal_rate: f64,
    /// FIRE type: "lean", "regular", "fat", or "coast".
    #[serde(default = "default_fire_type")]
    pub fire_type: String,
    /// Target FIRE number in cents (auto-calculated or manual override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_number: Option<i64>,
    /// Monthly savings rate in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly_savings_rate: Option<i64>,
    /// Snapshot of current net worth in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_net_worth: Option<i64>,
}

impl Default for FireConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            current_age: None,
            target_retirement_age: None,
            annual_expenses: None,
            safe_withdrawal_rate: default_swr(),
            fire_type: default_fire_type(),
            target_number: None,
            monthly_savings_rate: None,
            current_net_worth: None,
        }
    }
}

fn default_swr() -> f64 {
    4.0
}

fn default_fire_type() -> String {
    "regular".to_string()
}

/// Default finance transaction/budget categories.
pub fn default_finance_categories() -> Vec<FinanceDefaultCategory> {
    vec![
        // Income
        FinanceDefaultCategory::new("income", "Salary", "income"),
        FinanceDefaultCategory::new("income", "Freelance", "income"),
        FinanceDefaultCategory::new("income", "Investments", "income"),
        FinanceDefaultCategory::new("income", "Rental", "income"),
        FinanceDefaultCategory::new("income", "Side Business", "income"),
        FinanceDefaultCategory::new("income", "Gifts", "income"),
        FinanceDefaultCategory::new("income", "Refunds", "income"),
        // Essential
        FinanceDefaultCategory::new("essential", "Housing", "expense"),
        FinanceDefaultCategory::new("essential", "Utilities", "expense"),
        FinanceDefaultCategory::new("essential", "Groceries", "expense"),
        FinanceDefaultCategory::new("essential", "Transportation", "expense"),
        FinanceDefaultCategory::new("essential", "Insurance", "expense"),
        FinanceDefaultCategory::new("essential", "Healthcare", "expense"),
        FinanceDefaultCategory::new("essential", "Debt Payments", "expense"),
        // Lifestyle
        FinanceDefaultCategory::new("lifestyle", "Dining Out", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Entertainment", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Shopping", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Personal Care", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Subscriptions", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Travel", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Fitness", "expense"),
        // Savings
        FinanceDefaultCategory::new("savings", "Emergency Fund", "transfer"),
        FinanceDefaultCategory::new("savings", "Retirement", "transfer"),
        FinanceDefaultCategory::new("savings", "Investments", "transfer"),
        FinanceDefaultCategory::new("savings", "Education Fund", "transfer"),
        // Giving
        FinanceDefaultCategory::new("giving", "Charity", "expense"),
        FinanceDefaultCategory::new("giving", "Gifts", "expense"),
        FinanceDefaultCategory::new("giving", "Family Support", "expense"),
    ]
}

/// A default category entry used by the wizard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDefaultCategory {
    pub group: String,
    pub name: String,
    pub tx_type: String,
}

impl FinanceDefaultCategory {
    pub fn new(group: &str, name: &str, tx_type: &str) -> Self {
        Self {
            group: group.to_string(),
            name: name.to_string(),
            tx_type: tx_type.to_string(),
        }
    }
}
```

**Step 2: Add `fire` field to `FinanceConfig`**

In `crates/config/src/schema/finance.rs`, add to the `FinanceConfig` struct (after line 33):

```rust
    #[serde(default)]
    pub fire: FireConfig,
```

And update the `Default` impl to include `fire: Default::default()`.

**Step 3: Add `setup_completed` to `Config`**

In `crates/config/src/schema/core.rs`, add after the `mcp` field (line 149):

```rust
    /// Whether the first-run setup wizard has been completed.
    #[serde(default)]
    pub setup_completed: bool,
```

**Step 4: Update re-exports in `lib.rs`**

Add `FireConfig, FinanceDefaultCategory, default_finance_categories` to the `pub use schema::*` line in `crates/config/src/lib.rs`.

**Step 5: Run tests**

```bash
cargo nextest run -p config
```

Expected: All existing tests pass. New schema fields deserialize from existing JSON via `#[serde(default)]`.

**Step 6: Commit**

```bash
git add crates/config/
git commit -m "feat(config): add FIRE config schema and setup_completed flag"
```

---

### Task 2: Add Generic Config Commands to AppCore

**Files:**
- Modify: `crates/app-core/src/handlers/settings.rs` (append new methods)
- Modify: `crates/desktop-shared/src/commands.rs` (add response types)

**Step 1: Add `AppInfoResponse` type to `desktop-shared/src/commands.rs`**

Append:

```rust
// ── App Info ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoResponse {
    pub version: String,
    pub data_dir: String,
    pub setup_completed: bool,
}
```

**Step 2: Add config handlers to `app-core/src/handlers/settings.rs`**

Add a `deep_merge` helper and three new `AppCore` methods:

```rust
use serde_json::Value;
use desktop_shared::commands::AppInfoResponse;

/// Recursively merge `patch` into `base`. Objects merge keys;
/// arrays and scalars replace entirely; explicit `null` removes a key.
fn deep_merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if value.is_null() {
                    base_map.remove(&key);
                } else {
                    let entry = base_map.entry(key).or_insert(Value::Null);
                    deep_merge(entry, value);
                }
            }
        }
        (base, patch) => *base = patch,
    }
}
```

And inside `impl AppCore`:

```rust
    pub async fn app_info(&self) -> Result<AppInfoResponse, ApiError> {
        let cfg = self.config.read().await;
        Ok(AppInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: cfg.data_dir_path().to_string_lossy().to_string(),
            setup_completed: cfg.setup_completed,
        })
    }

    pub async fn config_get_section(&self, section: String) -> Result<Value, ApiError> {
        let cfg = self.config.read().await;
        let full = serde_json::to_value(&*cfg)
            .map_err(|e| ApiError::new("SERIALIZATION", e.to_string()))?;
        match full.get(&section) {
            Some(val) => Ok(val.clone()),
            None => Err(ApiError::new(
                "NOT_FOUND",
                format!("config section '{section}' not found"),
            )),
        }
    }

    pub async fn config_update_section(
        &self,
        section: String,
        patch: Value,
    ) -> Result<Value, ApiError> {
        let mut cfg = self.config.write().await;

        // Serialize current config to JSON, apply patch to the section, deserialize back
        let mut full = serde_json::to_value(&*cfg)
            .map_err(|e| ApiError::new("SERIALIZATION", e.to_string()))?;

        {
            let section_val = full.get_mut(&section).ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("config section '{section}' not found"))
            })?;
            deep_merge(section_val, patch);
        }

        // Deserialize the modified JSON back into Config
        let updated: config::Config = serde_json::from_value(full)
            .map_err(|e| ApiError::new("VALIDATION", format!("invalid config: {e}")))?;

        // Persist and update in-memory state
        config::save(&updated)
            .await
            .map_err(map_config_save_err)?;

        let result = serde_json::to_value(&updated)
            .map_err(|e| ApiError::new("SERIALIZATION", e.to_string()))?;
        let section_result = result.get(&section).cloned().unwrap_or(Value::Null);

        *cfg = updated;

        Ok(section_result)
    }

    pub async fn config_mark_setup_completed(&self) -> Result<(), ApiError> {
        let mut cfg = self.config.write().await;
        cfg.setup_completed = true;
        config::save(&cfg).await.map_err(map_config_save_err)?;
        Ok(())
    }
```

**Step 3: Run tests**

```bash
cargo nextest run -p app-core
cargo clippy --workspace --all-targets --all-features
```

Expected: Compiles with zero warnings.

**Step 4: Commit**

```bash
git add crates/app-core/ crates/desktop-shared/
git commit -m "feat(app-core): add generic config read/write and app_info handlers"
```

---

### Task 3: Wire Tauri Commands & Dev Server Routes

**Files:**
- Modify: `crates/desktop/src/commands/settings.rs` (add new commands)
- Modify: `crates/desktop/src/main.rs:120-283` (register new commands)
- Modify: `crates/desktop/src/dev_server.rs` (add dispatch routes)

**Step 1: Add Tauri commands to `settings.rs`**

Append to `crates/desktop/src/commands/settings.rs`:

```rust
use serde_json::Value;
use desktop_shared::commands::AppInfoResponse;

#[tauri::command]
pub async fn app_info(state: State<'_, Arc<AppCore>>) -> Result<AppInfoResponse, ApiError> {
    state.app_info().await
}

#[tauri::command]
pub async fn config_get_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
) -> Result<Value, ApiError> {
    state.config_get_section(section).await
}

#[tauri::command]
pub async fn config_update_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
    patch: Value,
) -> Result<Value, ApiError> {
    state.config_update_section(section, patch).await
}

#[tauri::command]
pub async fn config_mark_setup_completed(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    state.config_mark_setup_completed().await
}
```

**Step 2: Register commands in `main.rs`**

Add to the `invoke_handler` macro in `crates/desktop/src/main.rs` (after the Settings MCP section ~line 254):

```rust
            commands::settings::app_info,
            commands::settings::config_get_section,
            commands::settings::config_update_section,
            commands::settings::config_mark_setup_completed,
```

**Step 3: Add dev server dispatch routes**

Add to the match in `crates/desktop/src/dev_server.rs` (after the MCP settings section):

```rust
        // ── Settings (generic config) ────────────────────────
        "app_info" => r(core.app_info().await),
        "config_get_section" => {
            let section = match get_str(&body, "section") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.config_get_section(section).await)
        }
        "config_update_section" => {
            let section = match get_str(&body, "section") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let patch = body.get("patch").cloned().unwrap_or(serde_json::Value::Object(Default::default()));
            r(core.config_update_section(section, patch).await)
        }
        "config_mark_setup_completed" => r(core.config_mark_setup_completed().await),
```

**Step 4: Build and verify**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features
```

Expected: Zero warnings, clean build.

**Step 5: Commit**

```bash
git add crates/desktop/ crates/app-core/
git commit -m "feat(desktop): wire config commands to Tauri and dev server"
```

---

### Task 4: Frontend — Wizard Layout & Navigation Infrastructure

**Files:**
- Create: `desktop-ui/src/components/setup/SetupLayout.tsx`
- Create: `desktop-ui/src/components/setup/SetupProgress.tsx`
- Create: `desktop-ui/src/components/setup/useSetupNavigation.ts`
- Create: `desktop-ui/src/components/setup/steps.ts`
- Modify: `desktop-ui/src/App.tsx` (add wizard routes + first-run redirect)
- Modify: `desktop-ui/src/lib/types.ts` (add AppInfoResponse type)

**Step 1: Define wizard steps**

Create `desktop-ui/src/components/setup/steps.ts`:

```typescript
export interface SetupStep {
  id: string;
  path: string;
  label: string;
  required: boolean;
}

export const SETUP_STEPS: SetupStep[] = [
  { id: "welcome", path: "/setup/welcome", label: "Welcome", required: true },
  { id: "provider", path: "/setup/provider", label: "Provider & Model", required: true },
  { id: "channels", path: "/setup/channels", label: "Channels", required: false },
  { id: "areas", path: "/setup/areas", label: "Areas", required: false },
  { id: "productivity", path: "/setup/productivity", label: "Productivity", required: false },
  { id: "finance", path: "/setup/finance", label: "Finance", required: false },
  { id: "mcp", path: "/setup/mcp", label: "MCP Servers", required: false },
  { id: "complete", path: "/setup/complete", label: "Complete", required: true },
];
```

**Step 2: Create navigation hook**

Create `desktop-ui/src/components/setup/useSetupNavigation.ts`:

```typescript
import { useCallback } from "react";
import { useLocation, useNavigate } from "react-router";
import { SETUP_STEPS } from "./steps";

export function useSetupNavigation() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  const currentIndex = SETUP_STEPS.findIndex((s) => s.path === pathname);
  const currentStep = SETUP_STEPS[currentIndex];
  const isFirst = currentIndex <= 0;
  const isLast = currentIndex >= SETUP_STEPS.length - 1;

  const next = useCallback(() => {
    if (!isLast) navigate(SETUP_STEPS[currentIndex + 1].path);
  }, [currentIndex, isLast, navigate]);

  const back = useCallback(() => {
    if (!isFirst) navigate(SETUP_STEPS[currentIndex - 1].path);
  }, [currentIndex, isFirst, navigate]);

  const skip = useCallback(() => next(), [next]);

  return { currentStep, currentIndex, totalSteps: SETUP_STEPS.length, isFirst, isLast, next, back, skip };
}
```

**Step 3: Create SetupProgress component**

Create `desktop-ui/src/components/setup/SetupProgress.tsx` — a simple progress bar showing step N of M.

**Step 4: Create SetupLayout wrapper**

Create `desktop-ui/src/components/setup/SetupLayout.tsx` — centered card (max-w-xl), glassmorphism background, progress bar, back/skip/continue buttons.

**Step 5: Add AppInfoResponse type**

Add to `desktop-ui/src/lib/types.ts`:

```typescript
export interface AppInfoResponse {
  version: string;
  dataDir: string;
  setupCompleted: boolean;
}
```

**Step 6: Add wizard routes and first-run redirect to `App.tsx`**

Add lazy imports for setup pages and a `SetupRedirect` component that calls `app_info` and redirects to `/setup/welcome` if `!setupCompleted`. Add routes for `/setup/*`. The catch-all `*` route should use `SetupRedirect` instead of a plain `Navigate`.

**Step 7: Run dev**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

Expected: Clean build, no lint errors.

**Step 8: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): wizard layout, navigation, and first-run redirect"
```

---

### Task 5: Frontend — Wizard Step Pages (Welcome, Provider, Channels)

**Files:**
- Create: `desktop-ui/src/components/setup/pages/WelcomeStep.tsx`
- Create: `desktop-ui/src/components/setup/pages/ProviderStep.tsx`
- Create: `desktop-ui/src/components/setup/pages/ChannelsStep.tsx`

**Step 1: WelcomeStep**

Simple welcome screen with app name, brief description, and "Get Started" button that calls `next()`.

**Step 2: ProviderStep (required)**

- Dropdown to select provider (anthropic, openai, openrouter, deepseek, gemini, groq, etc.)
- API key input (masked, with show/hide toggle)
- Model name input (with sensible default based on provider)
- On Continue: calls `config_update_section("providers", { [provider]: { apiKey: key } })` and `config_update_section("agents", { defaults: { model, provider } })`

**Step 3: ChannelsStep (optional)**

- List of channels (Telegram, Discord, Slack, WhatsApp, Email) with enable toggles
- When enabled, show relevant credential fields (token, bot_token, etc.)
- On Continue: calls `config_update_section("channels", { telegram: { enabled, token, ... }, ... })`

**Step 4: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 5: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): wizard welcome, provider, and channels steps"
```

---

### Task 6: Frontend — Wizard Step Pages (Areas, Productivity)

**Files:**
- Create: `desktop-ui/src/components/setup/pages/AreasStep.tsx`
- Create: `desktop-ui/src/components/setup/pages/ProductivityStep.tsx`

**Step 1: AreasStep (optional)**

- Pre-populated with default areas: "Work", "Personal", "Health", "Learning"
- User can rename, remove, or add areas
- Each area has name + color picker
- On Continue: calls `area_create` for each area via existing command

**Step 2: ProductivityStep (optional)**

- Toggle: enable productivity tracking
- Default focus session duration (slider/input, default 45 min)
- Daily focus target (hours, default 8)
- Privacy: exclude apps list (text input, comma-separated)
- On Continue: calls `config_update_section("productivity", { ... })`

**Step 3: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): wizard areas and productivity steps"
```

---

### Task 7: Frontend — Wizard Finance Step (Multi Sub-Step)

**Files:**
- Create: `desktop-ui/src/components/setup/pages/FinanceStep.tsx`
- Create: `desktop-ui/src/components/setup/finance/FinanceBasicsForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/AccountsForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/IncomeForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/FireForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/InvestmentsForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/LiabilitiesForm.tsx`
- Create: `desktop-ui/src/components/setup/finance/GoalsForm.tsx`

**Step 1: FinanceStep parent with internal sub-stepper**

The `FinanceStep.tsx` manages which finance sub-step is active (basics → accounts → income → FIRE → investments → liabilities → goals). Each sub-step has its own Skip/Continue. The parent shows a mini progress indicator.

**Step 2: FinanceBasicsForm**

- Currency selector (common currencies: USD, EUR, GBP, VND, CNY, JPY, etc.)
- Proactivity level: radio group (full / moderate / reactive)
- On save: `config_update_section("finance", { defaultCurrency, proactivityLevel })`

**Step 3: AccountsForm**

- Dynamic list: add bank accounts with name, type (checking/savings/credit), balance (real), institution
- On save: calls `finance_account_create` for each account

**Step 4: IncomeForm**

- Budgeting method: radio (standard / six_jar)
- If six_jar: show editable ratio sliders
- Default categories: checkbox list from `default_finance_categories()` — user can add/remove/rename
- On save: `config_update_section("finance", { budgeting: { ... } })`

**Step 5: FireForm**

- Enable FIRE toggle
- Current age, target retirement age
- Annual expenses input
- Safe withdrawal rate (default 4%)
- FIRE type selector (lean/regular/fat/coast)
- Auto-calculated FIRE number display
- On save: `config_update_section("finance", { fire: { ... } })`

**Step 6: InvestmentsForm**

- Create portfolio (name, optional description)
- Add investments: asset type, symbol, quantity, cost basis
- On save: `finance_portfolio_create`, then `finance_investment_create` for each

**Step 7: LiabilitiesForm**

- Add liabilities: name, type (mortgage/student_loan/credit_card/other), principal, interest rate, monthly payment
- On save: `finance_liability_create` for each

**Step 8: GoalsForm**

- Add financial goals: name, type (savings/debt_payoff/investment), target amount, deadline, monthly contribution
- On save: `finance_goal_create` for each

**Step 9: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 10: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): wizard finance step with all sub-steps"
```

---

### Task 8: Frontend — Wizard MCP & Complete Steps

**Files:**
- Create: `desktop-ui/src/components/setup/pages/McpStep.tsx`
- Create: `desktop-ui/src/components/setup/pages/CompleteStep.tsx`

**Step 1: McpStep (optional)**

Reuse the existing `McpServersSettings` component (or its key parts — recommended server list with one-click install). Wrap in wizard layout.

**Step 2: CompleteStep**

- Success screen with checkmark
- Summary of what was configured (provider, N accounts, N areas, etc.)
- "Launch Klynt" button that calls `config_mark_setup_completed` then navigates to `/`

**Step 3: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): wizard MCP and completion steps"
```

---

### Task 9: Frontend — Update GeneralSettings (Remove Hardcoded Values)

**Files:**
- Modify: `desktop-ui/src/components/settings/pages/GeneralSettings.tsx`

**Step 1: Replace hardcoded values**

Replace hardcoded `"0.1.0"` and `"~/.klyntbot"` with data from `app_info` command:

```typescript
const { data: appInfo } = useQuery<AppInfoResponse>("app_info", undefined, {
  version: "...",
  dataDir: "...",
  setupCompleted: false,
});
```

Use `appInfo.version` and `appInfo.dataDir` in the template.

**Step 2: Add agent defaults section**

Add a card for agent defaults (model, temperature, max_tokens) loaded via `config_get_section("agents")` with editable fields that save via `config_update_section("agents", patch)`.

**Step 3: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "fix(desktop-ui): replace hardcoded values in GeneralSettings with real data"
```

---

### Task 10: Frontend — Replace Configuration & Personalization Placeholders

**Files:**
- Modify: `desktop-ui/src/components/settings/pages/ConfigurationSettings.tsx`
- Modify: `desktop-ui/src/components/settings/pages/PersonalizationSettings.tsx`

**Step 1: ConfigurationSettings — Channels**

Replace "coming soon" text with real channel configuration UI. Collapsible sections for each channel (Telegram, Discord, Slack, WhatsApp, Email, etc.) with enable toggle and credential fields. Data from `config_get_section("channels")`, saved via `config_update_section("channels", patch)`.

**Step 2: ConfigurationSettings — Tools**

Replace "coming soon" with real tools config: `restrict_to_workspace` toggle, browser settings (enabled, trust_level), web tools (Brave API key). Data from `config_get_section("tools")`.

**Step 3: ConfigurationSettings — Gateway**

Replace "coming soon" with host + port inputs. Data from `config_get_section("gateway")`.

**Step 4: PersonalizationSettings — Provider & Model**

Replace "coming soon" with provider selector, API key input (masked), model selection. Load all providers from `config_get_section("providers")` and agent defaults from `config_get_section("agents")`.

**Step 5: PersonalizationSettings — Learning**

Replace "coming soon" with learning config: enabled toggle, threshold sliders, analysis interval. Data from `config_get_section("learning")`.

**Step 6: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 7: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): replace placeholder settings with real config UI"
```

---

### Task 11: Frontend — ArchivedSettings & Shared Form Components

**Files:**
- Modify: `desktop-ui/src/components/settings/pages/ArchivedSettings.tsx`
- Create shared form components as needed (e.g., `desktop-ui/src/components/shared/SecretInput.tsx` for masked API key fields)

**Step 1: ArchivedSettings**

Wire to `chat_threads` with archive filter. Show list of archived threads with restore button. If no archived threads, keep the current empty state.

**Step 2: Extract shared components**

If wizard and settings pages share form patterns (SecretInput, collapsible section, toggle card), extract them into `desktop-ui/src/components/shared/`.

**Step 3: Lint and build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(desktop-ui): archived settings and shared form components"
```

---

### Task 12: Backend Tests & Final Verification

**Files:**
- Modify: `crates/config/src/schema/mod.rs` (add FIRE config tests)
- Modify: `crates/config/src/loader.rs` (add deep_merge-related tests if needed)

**Step 1: Add schema tests for FIRE config**

```rust
#[test]
fn test_fire_config_default() {
    let config = FireConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.safe_withdrawal_rate, 4.0);
    assert_eq!(config.fire_type, "regular");
    assert!(config.current_age.is_none());
}

#[test]
fn test_fire_config_serde_roundtrip() {
    let config = FireConfig {
        enabled: true,
        current_age: Some(30),
        target_retirement_age: Some(45),
        annual_expenses: Some(3_000_000),
        safe_withdrawal_rate: 3.5,
        fire_type: "lean".to_string(),
        target_number: Some(85_714_285),
        monthly_savings_rate: Some(500_000),
        current_net_worth: Some(10_000_000),
    };
    let json = serde_json::to_string(&config).unwrap();
    let loaded: FireConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.current_age, Some(30));
    assert_eq!(loaded.fire_type, "lean");
}

#[test]
fn test_setup_completed_default_false() {
    let config = Config::default();
    assert!(!config.setup_completed);
}

#[test]
fn test_default_finance_categories_count() {
    let cats = default_finance_categories();
    assert!(cats.len() >= 25);
    assert!(cats.iter().any(|c| c.name == "Salary"));
    assert!(cats.iter().any(|c| c.name == "Housing"));
}
```

**Step 2: Test deep_merge behavior**

Add a test in `app-core` or inline to verify:
- Object keys merge recursively
- Arrays replace entirely
- Null removes keys
- Scalars overwrite

**Step 3: Full test suite**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 4: Frontend build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 5: Commit**

```bash
git add .
git commit -m "test: add FIRE config, deep_merge, and setup wizard tests"
```

---

### Task Summary

| Task | Description | Type |
|------|-------------|------|
| 1 | FIRE config schema + `setup_completed` flag | Backend (config) |
| 2 | Generic config get/update handlers + `app_info` | Backend (app-core) |
| 3 | Wire Tauri commands & dev server routes | Backend (desktop) |
| 4 | Wizard layout, navigation, routes, first-run redirect | Frontend |
| 5 | Wizard steps: Welcome, Provider, Channels | Frontend |
| 6 | Wizard steps: Areas, Productivity | Frontend |
| 7 | Wizard finance multi-step (7 sub-forms) | Frontend |
| 8 | Wizard MCP + Complete steps | Frontend |
| 9 | Update GeneralSettings (remove hardcoded values) | Frontend |
| 10 | Replace Configuration & Personalization placeholders | Frontend |
| 11 | ArchivedSettings + shared form components | Frontend |
| 12 | Backend tests & final verification | Testing |
