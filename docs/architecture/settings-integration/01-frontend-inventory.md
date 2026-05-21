# Frontend Settings System Inventory

**Date:** 2026-05-21  
**Scope:** `desktop-ui/src/features/settings/**` + `desktop-ui/src/api/endpoints/settings.ts`

---

## 1. Navigation Structure

`SettingsNav` (`:desktop-ui/src/features/settings/components/SettingsNav.tsx`) renders a sidebar with items in this order:

| Order | Section ID | Label | Nav Icon |
|-------|-----------|-------|---------|
| 1 | `projects` | Projects | LayoutGrid |
| 2 | `environments` | Environments | Layers |
| 3 | `display` | Display & Sound | SlidersHorizontal |
| 4 | `composer` | Composer | FileText |
| 5 | `dictation` | Dictation | Mic |
| 6 | `shortcuts` | Shortcuts | Keyboard |
| 7 | `open-apps` | Open in | ExternalLink |
| 8 | `git` | Git | GitBranch |
| 9 | `server` | Server | ServerCog |
| 10 | `agents` | Agents | Bot |
| 11 | `codex` | Codex | TerminalSquare |
| 12 | `features` | Features | FlaskConical |
| 13 | `about` | About | Info |
| — | `hooks` | Hooks & Rules | *(not in nav, only in section containers)* |

`SettingsSectionContainers` (`:desktop-ui/src/features/settings/components/sections/SettingsSectionContainers.tsx:23-70`) dispatches to components by `activeSection`. All sections are mounted/unmounted (no `display:none` hiding).

The `hooks` section ID appears in `settingsTypes.ts:3` but **not in** `SettingsNav.tsx`. It is only reachable programmatically (via `initialSection` prop or direct route).

---

## 2. Section Catalog

### 2.1 SettingsProjectsSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsProjectsSection.tsx`  
**Hook:** `useSettingsProjectsSection` (`hooks/useSettingsProjectsSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| New group name input | text input | WIRED — calls `onCreateWorkspaceGroup` → `invoke("create_workspace_group")` |
| Add group button | button | WIRED |
| Group name rename (blur/Enter) | text input | WIRED — calls `onRenameWorkspaceGroup` → `invoke("rename_workspace_group")` |
| Copies folder Choose/Clear buttons | button | WIRED — calls `onUpdateAppSettings` → `invoke("update_app_settings")` |
| Move group up/down | buttons | WIRED — calls `onMoveWorkspaceGroup` |
| Delete group | button | WIRED — calls `onDeleteWorkspaceGroup` |
| Assign workspace to group (dropdown) | select | WIRED — calls `onAssignWorkspaceGroup` |
| Move workspace up/down | buttons | WIRED — calls `onMoveWorkspace` |
| Delete workspace | button | WIRED — calls `remove_workspace` IPC |

**Status: FULLY WIRED.** All mutations go through `onUpdateAppSettings` or dedicated workspace IPC commands.

---

### 2.2 SettingsEnvironmentsSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsEnvironmentsSection.tsx`  
**Hook:** `useSettingsEnvironmentsSection` (`hooks/useSettingsEnvironmentsSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Global worktrees root (text input + Browse) | text input + dialog | WIRED — saves via `onUpdateAppSettings` → `invoke("update_app_settings")` |
| Project selector (dropdown) | select | WIRED — local state switch, no backend call |
| Setup script (textarea) | textarea | WIRED — saves via `onUpdateWorkspaceSettings` → `invoke("update_workspace_settings")` |
| Worktrees folder (text input + Browse) | text input + dialog | WIRED — saves via `onUpdateWorkspaceSettings` |
| Copy / Reset / Save buttons | buttons | WIRED |

**Status: FULLY WIRED.**

---

### 2.3 SettingsDisplaySection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsDisplaySection.tsx`  
**Hook:** `useSettingsDisplaySection` (`hooks/useSettingsDisplaySection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Theme (dropdown: system/light/dark/dim) | select | WIRED → `update_app_settings` |
| Show remaining Codex limits (toggle) | toggle | WIRED → `update_app_settings` |
| Show file path in messages (toggle) | toggle | WIRED → `update_app_settings` |
| Split chat and diff center panes (toggle) | toggle | WIRED → `update_app_settings` |
| Auto-generate thread titles (toggle) | toggle | WIRED → `update_app_settings` |
| Unlimited chat history (toggle) | toggle | WIRED → `update_app_settings` |
| Scrollback preset (dropdown) | select | WIRED → `update_app_settings` |
| Max items per thread (text input) | text input | WIRED → `update_app_settings` |
| Reduce transparency (toggle) | toggle | **PARTIALLY WIRED** — stored in `localStorage` only (`useTransparencyPreference`, `features/layout/hooks/useTransparencyPreference.ts:4-11`), not persisted to backend |
| Interface scale (text input + Reset) | text input | WIRED → `update_app_settings` |
| UI font family (text input + Reset) | text input | WIRED → `update_app_settings` |
| Code font family (text input + Reset) | text input | WIRED → `update_app_settings` |
| Code font size (slider + Reset) | range | WIRED → `update_app_settings` |
| Notification sounds (toggle) | toggle | WIRED → `update_app_settings` |
| System notifications (toggle) | toggle | WIRED → `update_app_settings` |
| Sub-agent notifications (toggle) | toggle | WIRED → `update_app_settings` |
| Test sound (button) | button | WIRED — calls `playNotificationSound` (audio API, no backend IPC) |
| Test notification (button) | button | WIRED — calls `sendNotification` → `invoke("plugin:notification|send_notification")` |

**Status: MOSTLY WIRED. 1 gap: "Reduce transparency" uses `localStorage` only — not persisted to `AppSettings` in backend.**

---

### 2.4 SettingsComposerSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsComposerSection.tsx`  
No dedicated hook; props built directly in `useSettingsViewOrchestration`.

| Control | Type | Status |
|---------|------|--------|
| Follow-up behavior (Queue/Steer segmented radio) | radio group | WIRED → `update_app_settings` |
| Show follow-up hint (toggle) | toggle | WIRED → `update_app_settings` |
| Preset dropdown (default/helpful/smart) | select | WIRED — applies `COMPOSER_PRESET_CONFIGS` then → `update_app_settings` |
| Expand fences on Space (toggle) | toggle | WIRED → `update_app_settings` |
| Expand fences on Enter (toggle) | toggle | WIRED → `update_app_settings` |
| Support language tags (toggle) | toggle | WIRED → `update_app_settings` |
| Wrap selection in fences (toggle) | toggle | WIRED → `update_app_settings` |
| Copy blocks without fences (toggle) | toggle | WIRED → `update_app_settings` |
| Auto-wrap multi-line paste (toggle) | toggle | WIRED → `update_app_settings` |
| Auto-wrap code-like single lines (toggle) | toggle | WIRED → `update_app_settings` |
| Continue lists on Shift+Enter (toggle) | toggle | WIRED → `update_app_settings` |

**Status: FULLY WIRED.**

---

### 2.5 SettingsDictationSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsDictationSection.tsx`  
No dedicated hook; props built in `useSettingsViewOrchestration`.

| Control | Type | Status |
|---------|------|--------|
| Enable dictation (toggle) | toggle | WIRED → `update_app_settings` |
| Dictation model (dropdown: tiny/base/small/medium/large-v3) | select | WIRED → `update_app_settings`. Model list is **hardcoded** in `settingsViewConstants.ts:4-15` |
| Preferred dictation language (dropdown, 18 options) | select | WIRED → `update_app_settings`. Language list is **hardcoded** in component at lines `119-138` |
| Hold-to-dictate key (dropdown) | select | WIRED → `update_app_settings` |
| Download model (button) | button | WIRED — calls `onDownloadDictationModel` (wired through app layer to `invoke("download_dictation_model")`) |
| Cancel download (button) | button | WIRED — calls `onCancelDictationDownload` |
| Remove model (button) | button | WIRED — calls `onRemoveDictationModel` |

**Status: FULLY WIRED. Hardcoded constants are intentional configuration, not mocks.**

---

### 2.6 SettingsShortcutsSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsShortcutsSection.tsx`  
**Hook:** `useSettingsShortcutDrafts` (`hooks/useSettingsShortcutDrafts.ts`)

18 shortcut fields across 4 groups (File, Composer, Panels, Navigation). Each field captures key events and saves immediately.

| Control | Type | Status |
|---------|------|--------|
| All 18 shortcut key inputs | keyboard capture inputs | WIRED → `update_app_settings` |
| Clear button per shortcut | button | WIRED → `update_app_settings` (sets value to `null`) |
| Search filter input | text input | WIRED — local state filter only, no backend call |

Additionally, shortcut saves call `setMenuAccelerators` → `invoke("menu_set_accelerators")` to update macOS menu items.

**Status: FULLY WIRED.**

---

### 2.7 SettingsOpenAppsSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsOpenAppsSection.tsx`  
**Hook:** `useSettingsOpenAppDrafts` (`hooks/useSettingsOpenAppDrafts.ts`)

| Control | Type | Status |
|---------|------|--------|
| Label input (per app) | text input | WIRED → `update_app_settings` on blur |
| Type dropdown (App/Command/Finder) | select | WIRED → `update_app_settings` immediately |
| App name input | text input | WIRED → `update_app_settings` on blur |
| Command input | text input | WIRED → `update_app_settings` on blur |
| Args input | text input | WIRED → `update_app_settings` on blur |
| Default radio button | radio | WIRED → `update_app_settings` + `localStorage` write (`OPEN_APP_STORAGE_KEY`) |
| Move up/down | buttons | WIRED → `update_app_settings` |
| Delete app | button | WIRED → `update_app_settings` |
| Add app | button | WIRED → `update_app_settings` |

**Status: FULLY WIRED. `selectedOpenAppId` has dual-write to `localStorage` (fallback for legacy; backend is authoritative).**

---

### 2.8 SettingsGitSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsGitSection.tsx`  
**Hook:** `useSettingsGitSection` (`hooks/useSettingsGitSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Preload git diffs (toggle) | toggle | WIRED → `update_app_settings` |
| Ignore whitespace changes (toggle) | toggle | WIRED → `update_app_settings` |
| Commit message prompt (textarea) | textarea | WIRED → `update_app_settings` |
| Reset / Save buttons | buttons | WIRED → `update_app_settings` |
| Commit message model (dropdown) | select | WIRED → `update_app_settings`. Populated from live model list via `useSettingsDefaultModels` → `invoke("get_model_list")` |

**Status: FULLY WIRED.**

---

### 2.9 SettingsServerSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsServerSection.tsx`  
**Hook:** `useSettingsServerSection` (`hooks/useSettingsServerSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Backend mode dropdown (local/remote) | select | WIRED → `update_app_settings` |
| Keep daemon running after close (toggle) | toggle | WIRED → `update_app_settings` |
| Remote backend host input | text input | WIRED → `update_app_settings` |
| Remote backend token input (password) | text input | WIRED → `update_app_settings` |
| Saved remotes list (mobile) | list | WIRED → `update_app_settings` |
| Remote name input (mobile) | text input | WIRED → `update_app_settings` |
| Add remote modal (name, host, token) | modal + inputs | WIRED → `update_app_settings` + `invoke("list_workspaces")` connectivity test |
| Select active remote | button | WIRED → `update_app_settings` |
| Move remote up/down | buttons | WIRED → `update_app_settings` |
| Delete remote | button (with confirm modal) | WIRED → `update_app_settings` |
| Start daemon button | button | WIRED → `invoke("tailscale_daemon_start")` |
| Stop daemon button | button | WIRED → `invoke("tailscale_daemon_stop")` |
| Refresh daemon status | button | WIRED → `invoke("tailscale_daemon_status")` |
| Detect Tailscale | button | WIRED → `invoke("tailscale_status")` |
| Refresh daemon command | button | WIRED → `invoke("tailscale_daemon_command_preview")` |
| Use suggested host | button | WIRED → applies detected Tailscale host to active remote |
| Connect & test (mobile) | button | WIRED → `update_app_settings` + `invoke("list_workspaces")` |

**Status: FULLY WIRED.**

---

### 2.10 SettingsAgentsSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsAgentsSection.tsx`  
**Hook:** `useSettingsAgentsSection` (`hooks/useSettingsAgentsSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Refresh button | button | WIRED → `invoke("get_agents_settings")` query refetch |
| Open in Finder | button | WIRED → `revealItemInDir` (Tauri plugin, no IPC) |
| Enable Multi-Agent toggle | toggle | WIRED → `invoke("set_agents_core_settings")` |
| Max Threads stepper | stepper | WIRED → `invoke("set_agents_core_settings")` |
| Max Depth stepper | stepper | WIRED → `invoke("set_agents_core_settings")` |
| Create agent: name input | text input | WIRED → `invoke("create_agent")` |
| Create agent: description textarea | textarea | WIRED → `invoke("create_agent")` |
| Create agent: developer instructions textarea | textarea | WIRED → `invoke("create_agent")` |
| Create agent: model select | select | WIRED (models from `invoke("get_model_list")`); **fallback to hardcoded `FALLBACK_AGENT_MODELS`** at `SettingsAgentsSection.tsx:17-32` when models unavailable |
| Create agent: reasoning effort select | select | WIRED → `invoke("create_agent")` |
| Create agent: Generate button (AI) | button | WIRED → `invoke("generate_agent_description")` |
| Edit agent: name, description, instructions | inputs | WIRED → `invoke("update_agent")` |
| Edit agent: rename managed file checkbox | checkbox | WIRED → `invoke("update_agent")` with `renameManagedFile` flag |
| Delete agent (with confirm) | button | WIRED → `invoke("delete_agent")` |
| Edit File (config editor) | button | WIRED → `invoke("read_agent_config_toml")` / `invoke("write_agent_config_toml")` |

**Status: FULLY WIRED. `FALLBACK_AGENT_MODELS` at line 17 is a graceful degradation, not a permanent mock.**

---

### 2.11 SettingsCodexSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsCodexSection.tsx`  
**Hook:** `useSettingsCodexSection` (`hooks/useSettingsCodexSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Default Codex path input | text input | WIRED → `update_app_settings` |
| Browse codex binary | button | WIRED → `@tauri-apps/plugin-dialog` file picker |
| Use PATH button | button | WIRED (clears draft then → `update_app_settings`) |
| Default Codex args input | text input | WIRED → `update_app_settings` |
| Clear args button | button | WIRED |
| Save (codex settings) | button | WIRED → `update_app_settings` |
| Run doctor | button | WIRED → `invoke("codex_doctor")` |
| Update codex | button | WIRED → `invoke("codex_update")` |
| Default model select | select | WIRED → `update_app_settings`; populated from `invoke("get_model_list")` via `useSettingsDefaultModels` |
| Refresh models | button | WIRED → `invoke("get_model_list")` refetch |
| Reasoning effort select | select | WIRED → `update_app_settings` |
| Default access mode select | select | WIRED → `update_app_settings` |
| Review mode select | select | WIRED → `update_app_settings` |
| Global AGENTS.md editor (textarea, refresh, save) | file editor | WIRED → `invoke("workspace_meta_read")` / `invoke("workspace_meta_write")` |
| Global config.toml editor (textarea, refresh, save) | file editor | WIRED → `invoke("workspace_meta_read")` / `invoke("workspace_meta_write")` |

**Status: FULLY WIRED.**

---

### 2.12 SettingsFeaturesSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsFeaturesSection.tsx`  
**Hook:** `useSettingsFeaturesSection` (`hooks/useSettingsFeaturesSection.ts`)

| Control | Type | Status |
|---------|------|--------|
| Open config file (Finder) | button | WIRED → `revealItemInDir(configPath)` where configPath from `invoke("get_codex_config_path")` |
| Personality select (friendly/pragmatic) | select | WIRED → `update_app_settings` |
| Pause queued messages toggle | toggle | WIRED → `update_app_settings` |
| Stable feature flags (dynamic list) | toggles | WIRED → `invoke("set_codex_feature_flag")` or `update_app_settings` for mapped keys |
| Experimental feature flags (dynamic list) | toggles | WIRED → `invoke("set_codex_feature_flag")` |
| Feature list loaded from | query | WIRED → `invoke("experimental_feature_list")` paginated |

Hidden dynamic feature keys (`personality`, `collab`, `steer`) are filtered out at `useSettingsFeaturesSection.ts:17`.

**Status: FULLY WIRED.**

---

### 2.13 HooksSection
**File:** `desktop-ui/src/features/settings/components/sections/HooksSection.tsx`

| Control | Type | Status |
|---------|------|--------|
| *(no interactive controls)* | — | **FULLY MOCKED** |

The entire section is a static placeholder. Comment at line 2: `// TODO: Wire to unified hooks_list command once backend exposes it. Previously invoked coding_hooks_list (removed in unify-to-assistant).`

The component renders only a paragraph: `No ~/.klyntbot/hooks.toml found. Hooks are user-managed; create the file to enable.`

There is no `invoke()` call, no query, no form, no toggle. This section has zero backend persistence.

**Status: FULLY MOCKED / STUB. No controls to wire.**

---

### 2.14 SettingsAboutSection
**File:** `desktop-ui/src/features/settings/components/sections/SettingsAboutSection.tsx`  
No dedicated hook; props passed directly from orchestration.

| Control | Type | Status |
|---------|------|--------|
| Version / build type / branch / commit display | read-only text | WIRED — build info from Vite `define` constants (`__APP_VERSION__`, `__APP_GIT_BRANCH__`, `__APP_COMMIT_HASH__`, `__APP_BUILD_DATE__`); build type from `invoke("app_build_type")` |
| Automatic app update checks toggle | toggle | WIRED → `onToggleAutomaticAppUpdateChecks` → `update_app_settings` |
| Check for updates button | button | WIRED → `useUpdater` hook (Tauri updater plugin) |
| Download & Install button | button | WIRED → `useUpdater.startUpdate()` |

**Status: FULLY WIRED.**

---

## 3. Mock Data & Hardcoded Constants

| Constant / List | File | Line(s) | Nature |
|-----------------|------|---------|--------|
| `DICTATION_MODELS` (5 whisper model entries: tiny, base, small, medium, large-v3) | `settingsViewConstants.ts` | 4–15 | Intentional static config — models are fixed by upstream Whisper releases |
| `COMPOSER_PRESET_LABELS` (default/helpful/smart) | `settingsViewConstants.ts` | 31–35 | Intentional static config |
| `COMPOSER_PRESET_CONFIGS` (3 presets × 8 flags) | `settingsViewConstants.ts` | 37–68 | Intentional static config |
| `DEFAULT_REMOTE_HOST = "127.0.0.1:4732"` | `settingsViewConstants.ts` | 71 | Default value, not mock |
| `SETTINGS_SECTION_LABELS` (14 section name strings) | `settingsViewConstants.ts` | 73–88 | UI labels |
| `SHORTCUT_DRAFT_KEY_BY_SETTING` (18-entry mapping) | `settingsViewConstants.ts` | 90–109 | Static mapping |
| `FALLBACK_AGENT_MODELS` (single `gpt-5-codex` entry) | `SettingsAgentsSection.tsx` | 17–32 | Graceful degradation fallback, shown only when backend model list is unavailable |
| `FEATURE_DESCRIPTION_FALLBACKS` (43-key Record) | `SettingsFeaturesSection.tsx` | 12–43 | Client-side fallback descriptions for feature flags returned without `description`/`announcement` |
| `HIDDEN_DYNAMIC_FEATURE_KEYS` (`personality`, `collab`, `steer`) | `useSettingsFeaturesSection.ts` | 17 | Intentional filter — these keys are exposed via separate AppSettings fields |
| Dictation language list (18 languages, hardcoded `<option>` tags) | `SettingsDictationSection.tsx` | 119–138 | Static list, no backend enumeration endpoint |
| `buildDefaultSettings()` (40+ field object) | `useAppSettings.ts` | 131–217 | Fallback defaults for `AppSettings` when backend returns no/partial data — used as in-memory hydration base |
| `DEFAULT_REMOTE_BACKEND_HOST = "127.0.0.1:4732"` | `useAppSettings.ts` | 31 | Default value |

---

## 4. IPC Calls Present in the Settings System

All `invoke()` calls that settings sections use, sourced from `src/api/endpoints/`:

| Tauri Command | Calling Function | Used By |
|--------------|-----------------|---------|
| `get_app_settings` | `getAppSettings()` | `useAppSettings` — loads all settings on mount |
| `update_app_settings` | `updateAppSettings()` | Every section except HooksSection |
| `is_mobile_runtime` | `isMobileRuntime()` | `SettingsAboutSection`, `SettingsServerSection` |
| `get_config_model` | `getConfigModel()` | `useSettingsDefaultModels` |
| `menu_set_accelerators` | `setMenuAccelerators()` | `useSettingsShortcutDrafts` — updates macOS menu bar accelerators |
| `codex_doctor` | `runCodexDoctor()` | `useSettingsCodexSection` |
| `codex_update` | `runCodexUpdate()` | `useSettingsCodexSection` |
| `app_build_type` | `getAppBuildType()` | `SettingsAboutSection` |
| `tailscale_status` | `tailscaleStatus()` | `useSettingsServerSection` |
| `tailscale_daemon_command_preview` | `tailscaleDaemonCommandPreview()` | `useSettingsServerSection` |
| `tailscale_daemon_start` | `tailscaleDaemonStart()` | `useSettingsServerSection` |
| `tailscale_daemon_stop` | `tailscaleDaemonStop()` | `useSettingsServerSection` |
| `tailscale_daemon_status` | `tailscaleDaemonStatus()` | `useSettingsServerSection` |
| `set_codex_feature_flag` | `setCodexFeatureFlag()` | `useSettingsFeaturesSection` |
| `experimental_feature_list` | `getExperimentalFeatureList()` | `useSettingsFeaturesSection` |
| `get_agents_settings` | `getAgentsSettings()` | `useSettingsAgentsSection` |
| `set_agents_core_settings` | `setAgentsCoreSettings()` | `useSettingsAgentsSection` |
| `create_agent` | `createAgent()` | `useSettingsAgentsSection` |
| `update_agent` | `updateAgent()` | `useSettingsAgentsSection` |
| `delete_agent` | `deleteAgent()` | `useSettingsAgentsSection` |
| `read_agent_config_toml` | `readAgentConfigToml()` | `useSettingsAgentsSection` |
| `write_agent_config_toml` | `writeAgentConfigToml()` | `useSettingsAgentsSection` |
| `generate_agent_description` | `generateAgentDescription()` | `useSettingsAgentsSection` |
| `get_codex_config_path` | `getCodexConfigPath()` | `useSettingsFeaturesSection` |
| `workspace_meta_read` | `readGlobalAgentsMd()` / `readGlobalCodexConfigToml()` | `useGlobalAgentsMd`, `useGlobalCodexConfigToml` |
| `workspace_meta_write` | `writeGlobalAgentsMd()` / `writeGlobalCodexConfigToml()` | `useGlobalAgentsMd`, `useGlobalCodexConfigToml` |
| `connect_workspace` | `connectWorkspace()` | `useSettingsDefaultModels`, `useSettingsAgentsSection` |
| `list_workspaces` | `listWorkspaces()` | `useSettingsServerSection` (connection test) |
| `get_model_list` | `getModelList()` | `useSettingsDefaultModels` |
| `update_workspace_settings` | `updateWorkspaceSettings()` | `useSettingsEnvironmentsSection` |
| `remove_workspace` | `removeWorkspace()` | Workspace delete in projects section |

---

## 5. Gaps: Controls Without Backend Persistence

Only **one true gap** exists, plus **one fully mocked section**:

### Gap 1 — Reduce Transparency (Display section)

**Control:** "Reduce transparency" toggle in `SettingsDisplaySection.tsx:333-341`

**Problem:** The toggle calls `onToggleTransparency` which is wired to `setReduceTransparency` from `useTransparencyPreference` (`features/layout/hooks/useTransparencyPreference.ts:4-16`). That hook reads/writes **`localStorage` only** — not `AppSettings` in the backend.

If the user reinstalls the app, clears app data, or switches machines, the preference is lost. `AppSettings` has no `reduceTransparency` field. The backend is never informed of the user's preference.

### Gap 2 — HooksSection (fully mocked / stub)

**Component:** `HooksSection.tsx` (the `hooks` section)

**Problem:** The component is an inert placeholder (`HooksSection.tsx:1-12`). The comment at line 2 documents the issue: the previous `coding_hooks_list` IPC call was removed during the unify-to-assistant refactor (`28d4857d`) and no replacement backend endpoint exists yet. There are no controls, no form fields, no queries, and no mutations. It is unreachable via the nav sidebar (the Hooks & Rules item is absent from `SettingsNav.tsx`).

---

## 6. Summary Statistics

| Category | Count |
|----------|-------|
| Sections in nav | 13 |
| Sections reachable (including hooks) | 14 |
| Sections fully wired to backend | 12 |
| Sections partially wired (localStorage gap) | 1 (Display — reduce transparency) |
| Sections fully mocked / stub | 1 (Hooks) |
| Unique `invoke()` command names used | 31 |
| Interactive controls with no backend persistence | 1 (reduce transparency toggle) |
| Hardcoded lists that are intentional config | 6 |
| Hardcoded lists that are graceful fallbacks | 2 (FALLBACK_AGENT_MODELS, FEATURE_DESCRIPTION_FALLBACKS) |

The frontend settings system is **substantially wired** to the backend. The earlier characterization of it as "legacy and mocked" is **not accurate** for most sections. Nearly every section uses real `invoke()` calls. The only genuine gap is the `reduceTransparency` preference being localStorage-only, and the `HooksSection` being an explicit stub awaiting a backend endpoint.
