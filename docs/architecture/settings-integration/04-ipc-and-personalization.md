# IPC & Personalization Bridge — Settings System

> **Purpose:** Reference for building a unified settings UI.
> Lists every existing IPC seam for reading/writing config and personalization,
> the canonical config write path, and which surfaces have no settings UI today.

---

## 1. Existing Settings / Config Commands

All commands are registered via `#[klynt_command]` or `#[klynt_raw_command]` macros and are present in
`crates/desktop/src/specta_builder.rs` (lines 389–399, 398–399). They are fully typed in `desktop-ui/src/bindings.ts`.

### Config (generic section read/write)

| Command | File : Line | Reads / Writes | Frontend Caller |
|---|---|---|---|
| `config_get_section(section)` | `commands/settings.rs:42` → `handlers/settings/config.rs:42` | Reads any top-level `Config` field by camelCase key, returns raw JSON | `useProviders.ts:70–71` (sections `"providers"`, `"agents"`) |
| `config_update_section(section, patch)` | `commands/settings.rs:50` → `handlers/settings/config.rs:55` | Deep-merges patch into one Config section, persists, propagates HotConfig + voice | None directly — called only through `useProviders.ts` indirectly. MCP path uses dedicated commands. |
| `config_mark_setup_completed()` | `commands/settings.rs:58` → `handlers/settings/config.rs:96` | Sets `setup_completed = true`, saves, marks journey milestone | None observed in non-test code |
| `app_info()` | `commands/settings.rs:37` → `handlers/settings/config.rs:32` | Returns `version`, `data_dir`, `setup_completed` | None observed in non-test code |

### MCP client configuration

All four mutation commands call `config::save` then live-update the agent's MCP connections without restart.

| Command | File : Line | Reads / Writes |
|---|---|---|
| `mcp_get_config()` | `commands/settings.rs:12` → `handlers/settings/mcp.rs:107` | Returns `McpConfigResponse` (enabled flag + all servers) |
| `mcp_add_server(params)` | `commands/settings.rs:17` → `handlers/settings/mcp.rs:114` | Appends `McpServerDef` to `config.mcp.servers`, saves, calls `agent.reconnect_mcp_server` |
| `mcp_remove_server(params)` | `commands/settings.rs:22` → `handlers/settings/mcp.rs:163` | Removes by name, saves, calls `agent.disconnect_mcp_server` |
| `mcp_toggle_server(params)` | `commands/settings.rs:27` → `handlers/settings/mcp.rs:193` | Flips `server.enabled`, saves, connect/disconnect accordingly |
| `mcp_update_server(params)` | `commands/settings.rs:31` → `handlers/settings/mcp.rs:229` | Updates transport config, saves, reconnects |

**Frontend caller:** MCP commands are in `bindings.ts` (lines 2789–2821) but no frontend component currently calls them directly (no non-test `commands.mcpGet*` usage found).

### Global hotkeys / shortcuts

| Command | File : Line | Reads / Writes |
|---|---|---|
| `shortcuts_get()` | `commands/shortcuts.rs:10` | Returns `ShortcutsConfig { launcher, tray }` from live `RwLock<Config>` |
| `shortcuts_update(launcher, tray)` | `commands/shortcuts.rs:16` | Validates strings, registers OS shortcuts (phase 1: parse; phase 2: OS `on_shortcut`), persists to config, rolls back on failure |

**Config schema:** `crates/config/src/schema/shortcuts.rs:1`. Defaults: `launcher = "alt+space"`, `tray = "alt+shift+space"`.
**Frontend caller:** In `bindings.ts` (lines 2829–2837) but no active non-test caller found.

### macOS permissions (status-only, no config write)

| Command | File : Line |
|---|---|
| `permissions_check_accessibility()` | `commands/permissions.rs:8` |
| `permissions_open_accessibility()` | `commands/permissions.rs:14` |
| `permissions_request_accessibility_for_input()` | `commands/permissions.rs:22` |
| `permissions_check_screen_recording()` | `commands/permissions.rs:29` |
| `permissions_request_screen_recording()` | `commands/permissions.rs:35` |
| `permissions_open_screen_recording()` | `commands/permissions.rs:42` |

All use `#[klynt_raw_command]`. No config written; OS permission state only.

### Workspace files (auxiliary agent instructions)

| Command | File : Line | Reads / Writes |
|---|---|---|
| `workspace_list_files()` | `commands/workspace.rs:7` → `handlers/workspace.rs:23` | Returns `Vec<WorkspaceFile>` for the 5 editable files: `AGENTS.md`, `USER.md`, `TOOLS.md`, `RESPONSE.md`, `HEARTBEAT.md` |
| `workspace_read_file(filename)` | `commands/workspace.rs:12` → `handlers/workspace.rs:38` | Reads workspace file; falls back to embedded template |
| `workspace_write_file(filename, content)` | `commands/workspace.rs:17` → `handlers/workspace.rs:65` | Writes one of the 5 allowed filenames under `~/.klyntbot/workspace/` |

**Frontend caller:** In `bindings.ts` (lines 3492–3508) but no active non-test caller found.

### Skill / agent profiles

| Command | File : Line | Reads / Writes |
|---|---|---|
| `agent_list_profiles()` | `commands/agents.rs:7` → `handlers/agents.rs:36` | Lists built-in + custom skill profiles from `~/.klyntbot/workspace/skills/` |
| `agent_read_file(agentName, filename)` | `commands/agents.rs:12` → `handlers/agents.rs:178` | Reads `SKILL.md` or `references/*.md`; workspace override takes precedence over built-in |
| `agent_write_file(agentName, filename, content)` | `commands/agents.rs:17` → `handlers/agents.rs:239` | Writes file, then calls `agent.reload_agents()` for hot-reload |
| `agent_create_profile(name)` | `commands/agents.rs:28` → `handlers/agents.rs:275` | Creates new skill dir + `SKILL.md`, hot-reloads |
| `agent_create_skill(agentName, skillName)` | `commands/agents.rs:33` → `handlers/agents.rs:319` | Creates `references/{skillName}.md`, hot-reloads |
| `agent_delete_file(agentName, filename)` | `commands/agents.rs:38` → `handlers/agents.rs:369` | Removes workspace file, hot-reloads |

### AI tool integrations detection/install

| Command | File : Line |
|---|---|
| `ai_tools_detect()` | `commands/integrations.rs:8` → `handlers/integrations.rs` |
| `ai_tools_install(params)` | `commands/integrations.rs:13` → `handlers/integrations.rs` |

### Frontend-only settings (no Rust backend)

`desktop-ui/src/api/endpoints/settings.ts` calls `get_app_settings` (line 13) and `update_app_settings` (line 21) via raw `invoke()`. **Neither command exists in the Rust backend** (`specta_builder.rs`, `bindings.ts`, or any `commands/*.rs` file). These are dead-end stubs that will silently fail at runtime. The hook `useAppSettings.ts` catches the error and falls back to in-memory defaults. Settings managed this way include: `theme`, `uiScale`, `uiFontFamily`, `codeFontFamily`, `codeFontSize`, `notificationSoundsEnabled`, keyboard shortcuts for the editor UI (not OS global hotkeys), `personality`, and many more — all currently **not persisted to disk**.

---

## 2. Canonical Config Write Path

A setting change flows as follows, with file:line citations:

1. **Frontend** calls `invoke("config_update_section", { section, patch })` directly or through the `bindings.ts` wrapper.
2. **Tauri dispatch** routes to `commands/settings.rs:50` (`config_update_section`), which calls `state.config_update_section(section, patch).await`.
3. **`AppCore::config_update_section`** (`handlers/settings/config.rs:55–93`):
   a. Acquires a write lock on `self.config` (`Arc<RwLock<Config>>`).
   b. Serializes the full current config to `serde_json::Value`.
   c. Applies `deep_merge` (defined at `config.rs:12`) into the target section — objects merge recursively; scalars and arrays replace; `null` removes a key.
   d. Deserializes the merged JSON back into `config::Config` (validates the full schema).
   e. Calls `config::save(&updated)` (`crates/config/src/loader.rs:78`) which:
      - serializes the config to JSON,
      - diffs it against `Config::default()` to produce a minimal file (only non-default fields),
      - writes pretty-printed JSON to `~/.klyntbot/config.json` (or `KLYNTBOT_HOME`).
   f. Updates `self.config` with the new `Config`.
   g. Updates `self.hot_config` (`Arc<RwLock<HotConfig>>`) from `HotConfig::from(&*cfg)` so the agent pipeline sees the change immediately (no restart needed for: model, temperature, max_tokens, max_tool_iterations, safety_timeout_secs, monthly_budget_usd — `schema/hot.rs:14`).
   h. If `section == "voice"`, calls `self.propagate_voice_config` to update the live VoiceService.
4. **Background hot-reload watcher** (`infrastructure/config_watcher.rs:17`): polls the config file every 30 seconds for external changes (e.g. the user edited the file in a text editor). If mtime changed AND `HotConfigDiff::has_changes()`, it updates both `hot_config` and `app_config` RwLocks.

For **shortcuts** specifically, the path diverges: `commands/shortcuts.rs:16` (`shortcuts_update`) writes directly to `config.shortcuts` and calls `config::save`, bypassing the generic `config_update_section` handler, because it also registers the OS global shortcut via `tauri-plugin-global-shortcut` before saving.

For **MCP** mutations, each handler in `handlers/settings/mcp.rs` writes directly to `config.mcp`, calls `config::save`, then performs live agent connection changes — all outside `config_update_section`.

---

## 3. Personalization Inventory

### 3a. Souls (`KLYNTBOT.md`)

- **What:** LLM system prompt personality file. Controls tone, formatting rules, language, persona.
- **Where stored:** `~/.klyntbot/KLYNTBOT.md` (or `KLYNTBOT_HOME`). Created on first run from `DEFAULT_SOUL` constant at `crates/skill-system/src/soul.rs:14`.
- **How read:** `SoulContextSource::provide()` (`soul.rs:97`) reads the file on every agent turn using mtime caching to avoid redundant disk reads. Highest priority context source (priority 50, protected from eviction).
- **How edited:** Direct file edit (no IPC command). The `workspace_write_file` command covers only the 5 auxiliary workspace files — KLYNTBOT.md is excluded by the allowlist at `handlers/workspace.rs:13`.
- **Settings UI:** **None.** No IPC command exists to read or write `KLYNTBOT.md`. A new `soul_read()` / `soul_write()` command pair must be created.

> Note: CLAUDE.md mentions `KLYNTBOT-coding.md` for coding-mode soul but no such file or `CodingSoulContextSource` was found in the codebase. Only a single `SoulContextSource` reading `KLYNTBOT.md` exists. This may be planned but not yet implemented.

### 3b. Auxiliary Workspace Files (AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, HEARTBEAT.md)

- **Where stored:** `~/.klyntbot/workspace/*.md`.
- **How read:** `BootstrapSource` injects them as context. `workspace_read_file` provides read access.
- **How edited:** `workspace_write_file(filename, content)` — exists and registered.
- **Settings UI:** Commands exist and are typed in `bindings.ts` but **no Settings panel component calls them**. The seam is ready; a UI panel is missing.

### 3c. Skill / Agent Profiles

- **What:** Custom LLM skill definitions. Each skill is a directory under `~/.klyntbot/workspace/skills/{name}/` containing `SKILL.md` and `references/*.md`.
- **Where stored:** Workspace directory above. Built-in skills are embedded in the `agent` crate binary and can be overridden.
- **How edited:** Full CRUD via `agent_list_profiles`, `agent_read_file`, `agent_write_file`, `agent_create_profile`, `agent_create_skill`, `agent_delete_file`. Writes trigger `agent.reload_agents()` for immediate hot-reload.
- **Settings UI:** Commands exist and typed in bindings. The `SettingsAgentsSection.tsx` component exists at `desktop-ui/src/features/settings/components/sections/SettingsAgentsSection.tsx` and has a full form referencing `onReadAgentConfig` / `onWriteAgentConfig` props. **The hook wiring (`useSettingsAgentsSection.ts`) exists.** This surface is the most complete.

### 3d. Persona Skills (PERSONA.md files)

- **What:** Persona-specific `PERSONA.md` files with YAML frontmatter (`expertise_areas`, `tone`, `cognitive_bias`, etc.). Parsed by `skill-system/src/persona.rs`.
- **Where stored:** `~/.klyntbot/workspace/skills/{name}/PERSONA.md` or embedded in agent crate.
- **How edited:** Via the same `agent_read_file` / `agent_write_file` IPC (filename must match `SKILL.md` or `references/*.md` — `handlers/agents.rs:403`). PERSONA.md does not match this allowlist. **No IPC path for PERSONA.md files exists.**
- **Settings UI:** None.

### 3e. Themes

- **Where stored:** In-memory only (no disk persistence). The `theme` field is part of `AppSettings` which is currently fetched/saved via `get_app_settings` / `update_app_settings` — commands that do not exist in the Rust backend. Theme falls back to the in-memory default `"system"`.
- **How applied:** `useThemePreference(appSettings.theme)` at `features/layout/hooks/useThemePreference.ts:4` sets `document.documentElement.dataset.theme`. CSS theme files at `desktop-ui/src/styles/themes.{light,dark,dim,system}.css`.
- **Settings UI:** `SettingsDisplaySection.tsx` has a `<select>` for theme (line 170). It calls `onUpdateAppSettings` which ultimately calls the non-existent `update_app_settings`. **Theme is not persisted across restarts.**

### 3f. Global Hotkeys (Launcher / Tray windows)

- **Where stored:** `config.shortcuts` in `config.json` (`schema/shortcuts.rs:5`). Default: `launcher = "alt+space"`, `tray = "alt+shift+space"`.
- **How edited:** `shortcuts_update(launcher, tray)` — registers OS shortcuts, persists config, rolls back on failure.
- **Settings UI:** Commands exist and are typed in `bindings.ts`. `SettingsShortcutsSection.tsx` at `features/settings/components/sections/SettingsShortcutsSection.tsx` exists but manages **editor-UI shortcuts only** (new agent, branch switcher, etc.) stored in the non-persisted `AppSettings`. It does not call `shortcuts_get` or `shortcuts_update`. **No UI for launcher/tray OS hotkey editing currently exists.**

### 3g. API Keys / Secrets

- **Where stored:** `config.providers.{provider}.api_key` as `Secret<String>` (serializes transparently to JSON). Also stored per MCP server as OAuth credentials (`config.mcp.servers[].oauth.access_token`).
- **How edited:**
  - Provider API keys: via `config_update_section("providers", { anthropic: { apiKey: "..." } })`.
  - MCP OAuth tokens: via `mcp_oauth_start` / `mcp_oauth_disconnect` commands in `crates/desktop/src/oauth/commands.rs` (registered in specta_builder at line 311–312).
- **Settings UI:** `useProviders.ts` reads provider config via `config_get_section` but only to display which providers are configured — no UI for entering/changing API keys currently exists in the settings panels. **API key editing has no settings UI.**

### 3h. MCP Server Config

- **Where stored:** `config.mcp.servers` (array of `McpServerDef`). Persisted in `config.json`.
- **How edited:** Full CRUD via `mcp_add_server`, `mcp_remove_server`, `mcp_toggle_server`, `mcp_update_server`. Changes take effect immediately (live agent reconnection).
- **Exposed tools configuration:** `config.mcp.server.exposed_tools` — the list of tool names KlyntBot exposes when acting as an MCP server. Populated at startup from `AiFeatureRegistry` + `EXPLICIT_TOOL_ALLOWLIST` (`schema/mcp.rs:197`). No UI to edit this.
- **Settings UI:** Commands fully registered and typed but **no Settings component currently calls them.** MCP settings UI is a blank space.

---

## 4. Reusable Seams vs. Commands to Create

### Ready to call (backend exists, no UI)

| Surface | Commands to call |
|---|---|
| Workspace auxiliary files (AGENTS.md etc.) | `workspace_list_files`, `workspace_read_file`, `workspace_write_file` |
| MCP server CRUD | `mcp_get_config`, `mcp_add_server`, `mcp_remove_server`, `mcp_toggle_server`, `mcp_update_server` |
| OS global hotkeys (launcher/tray) | `shortcuts_get`, `shortcuts_update` |
| macOS permissions | `permissions_check_*`, `permissions_open_*`, `permissions_request_*` |
| Skill / agent profiles | `agent_list_profiles`, `agent_read_file`, `agent_write_file`, `agent_create_profile`, `agent_create_skill`, `agent_delete_file` |
| Generic config section read | `config_get_section(section)` — works for `"agents"`, `"providers"`, `"voice"`, `"mcp"`, any top-level Config key |
| Generic config section write | `config_update_section(section, patch)` — deep-merge semantics, immediate hot-reload for hot-reloadable fields |
| App info (version, data dir) | `app_info()` |
| Setup wizard completion | `config_mark_setup_completed()` |

### Commands that must be created

| Surface | Required new commands | Notes |
|---|---|---|
| Soul / `KLYNTBOT.md` | `soul_read()` → returns file content; `soul_write(content)` → writes file | Soul is outside the workspace allowlist; file path is `config.data_dir_path()/"KLYNTBOT.md"`, not the workspace. Should trigger `SoulContextSource::reload()` if held by AppCore. |
| API key entry | `config_update_section("providers", patch)` already works; the gap is purely UI | Optionally add a dedicated `providers_set_api_key(provider, key)` for cleaner ergonomics |
| Persona files (PERSONA.md) | Extend `agent_read_file`/`agent_write_file` allowlist (`handlers/agents.rs:403`) to include `PERSONA.md`, OR add `persona_read(agentName)` / `persona_write(agentName, content)` | Currently blocked by filename validation |
| Theme persistence | Fix `get_app_settings`/`update_app_settings`: either create real Rust commands or route `theme` through `config_update_section("user", { theme })` after adding `theme` field to `UserConfig` | All pure-frontend settings in `AppSettings` (shortcuts, fonts, UI scale) need the same fix |
| MCP exposed tools editor | `config_update_section("mcp", { server: { exposedTools: [...] } })` already works; purely UI gap | |

### Important gap: frontend settings not persisted

All fields currently in `useAppSettings.ts` → `AppSettings` (theme, fonts, UI scale, editor shortcuts, personality, notification prefs, etc.) are fetched from and saved to the non-existent `get_app_settings`/`update_app_settings` backend commands. The settings hook silently falls back to in-memory defaults on every app launch. A new settings UI must either:

- (a) Create real Rust IPC commands `get_app_settings` / `update_app_settings` that persist to a new `user_settings.json` sidecar file, OR
- (b) Map each `AppSettings` field to the appropriate `config.json` section and use `config_update_section`, extending `Config` schema as needed.

Option (b) is preferred for architectural consistency since `config.json` already has `user`, `voice`, `shortcuts` sections.

---

*Generated 2026-05-21. File references are relative to the workspace root `/Users/jayden/Projects/Klynt/bot`.*
