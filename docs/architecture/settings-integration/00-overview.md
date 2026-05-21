# Settings Integration — Scan Results & Master Index

> **Status:** Task 1 (scan) complete. Task 2 (integration plan) pending a strategic-direction decision (see §4).
> Generated 2026-05-21 from a 4-agent parallel codebase scan.

## 0. Report index

| # | Report | Scope |
|---|--------|-------|
| 01 | [`01-frontend-inventory.md`](./01-frontend-inventory.md) | Every FE settings section, control, backing hook, wired-vs-mocked status |
| 02 | [`02-backend-config-schema.md`](./02-backend-config-schema.md) | Field-level catalog of all 37 `crates/config/src/schema/*` modules (~310 fields) |
| 03 | [`03-backend-hardcoded-scan.md`](./03-backend-hardcoded-scan.md) | Values hardcoded *outside* the config crate that should arguably be settings |
| 04 | [`04-ipc-and-personalization.md`](./04-ipc-and-personalization.md) | Existing config IPC commands, the config write path, personalization surfaces (souls/personas/skills/themes/hotkeys/secrets/MCP) |

## 1. The central finding (verified directly, not just agent-reported)

The frontend and backend **do not share a settings vocabulary**:

- The `desktop-ui` settings feature is a **port from a different product** (a Codex-style coding IDE). Evidence: it uses `@tanstack/react-query` + `useTauriQuery`/`useTauriMutation` (CLAUDE.md says KlyntBot has *no* such wrapper and uses direct `invoke()`), and its `AppSettings` shape is full of coding-IDE concepts — `codexBin`, `codexArgs`, `remoteBackends`, `globalWorktreesFolder`, `collaborationModesEnabled`, `reviewDeliveryMode`. See `desktop-ui/src/features/settings/hooks/useAppSettings.ts`.
- The settings UI calls `getAppSettings()` / `updateAppSettings()` (`api/endpoints/settings.ts`) → Tauri commands `get_app_settings` / `update_app_settings` that **do not exist in the Rust backend** (confirmed: `grep` finds zero definitions; `useAppSettings` `catch`es the failure and falls back to `buildDefaultSettings()`). **Every settings save is a silent no-op that resets on reload.**
- The backend's *real*, working config bridge — `config_get_section` / `config_update_section` (deep-merge + schema-validate + hot-reload, `crates/desktop/src/commands/settings.rs` → `app-core/.../handlers/settings/config.rs`) — has **essentially no FE callers** (only `features/models/hooks/useProviders.ts`).

So this is **schema reconciliation**, not wiring. Three populations of settings exist, only partially overlapping:

```
   FE AppSettings (coding-IDE dialect)        BE config schema (37 modules, ~310 fields)
   ┌───────────────────────────────┐         ┌────────────────────────────────────────┐
   │ codexBin, remoteBackends,      │         │ cognitive (~70), channels (~45),         │
   │ worktrees, collaboration,      │  small  │ providers (~40), productivity (~30),     │
   │ composer fences, review mode,  │ overlap │ launcher (~28), voice (~28), learning,   │
   │ shortcuts, theme, fonts...     │◄──────► │ language, notes, todo, mcp, gateway...   │
   └───────────────────────────────┘         └────────────────────────────────────────┘
        ▲ no BE backing today                      ▲ no FE surface today
                         + a 3rd population: hardcoded values (report 03)
```

## 2. Task 1 master list — everything that should be settings-managed

### 2a. Already-modeled backend config (report 02) — needs a UI

37 schema modules, ~310 leaf fields. Proposed user-facing domains:

1. **Models & Providers** — provider API keys (13 × `Secret<String>`), default model/temperature/maxTokens/maxToolIterations (hot-reloadable), monthly budget.
2. **Memory & Intelligence** — `cognitive` (~70 fields, KCA pipeline).
3. **Voice** — STT/TTS engines, personas, silence/conversation tuning (~28).
4. **Channels & Integrations** — Telegram/Discord/Slack/Email listeners + tokens (~45), MCP servers.
5. **Productivity & Focus** — tracking, nudges, lifecycle, todo scheduling (~30).
6. **Language & Learning** — FSRS, flashcards, pronunciation.
7. **Launcher** — ~17 source toggles (~28 fields).
8. **Privacy & Security** — workspace sandboxing, approval policy, capture.
9. **App & UI** — shortcuts, user name, packs, notifications, theme/fonts/scale.
10. **Advanced / Developer** — gateway port, autotuner, cron schedules, content registry.

### 2b. Hardcoded values that should become settings (report 03)

Top candidates (full table in report 03): approval timeout (600s), memory retrieval limit (30), subagent turn cap (500), voice idle-unload TTL (300s) & silence duration (1500ms), coaching rate limits, brain-voice pulse cap, launcher/tray window sizes, long-running tool timeout (600s), agent max iterations (10), task warning hours `[6,3,1]`, session compaction thresholds (200/100), max tool result bytes (50k), scheduler grace (3600s), bootstrap token cap (8000).

### 2c. Personalization surfaces with no settings UI (report 04)

- **Souls** — `~/.klyntbot/KLYNTBOT.md` + `KLYNTBOT-coding.md`: no read/write IPC at all.
- **Personas / squads** — DB rows; blocked by the `agent_read_file` filename allowlist.
- **Skills / profiles** — CRUD commands exist (`agent_*`) but limited UI.
- **Themes** — selection lives only in the (non-persisted) `AppSettings.theme`.
- **Global hotkeys** — `shortcuts_get`/`shortcuts_update` exist & work, but no settings component calls them (the FE `SettingsShortcutsSection` edits non-persisted *editor* shortcuts).
- **API keys / secrets** — `config_update_section` can write them, but no UI panel exists.
- **MCP servers** — full command set exists & typed, but no FE component calls it.

### 2d. FE sections today (report 01)

14 sections (Projects, Environments, Display, Composer, Dictation, Shortcuts, Open-in, Git, Server, Agents, Codex, Features, Hooks, About). Most call `invoke()`, but the central `AppSettings` get/set those depend on is unbacked (§1). HooksSection is an explicit stub. Many of these sections (Codex, Environments, worktrees) are **coding-IDE concepts that may not apply to KlyntBot at all.**

## 3. The reusable seams (what already works)

- `config_get_section(section)` / `config_update_section(section, patch)` — generic, deep-merge, schema-validated, hot-reloads model/temp/tokens. **This is the spine the new UI should ride.**
- `shortcuts_get` / `shortcuts_update` — OS hotkey registration with rollback.
- `mcp_*` — full MCP server CRUD with live agent reconnect.
- `agent_*` — skill/profile CRUD with hot-reload.
- Hot-reload watcher (`infrastructure/config_watcher.rs`, 30s poll) + immediate `HotConfig` update on write.

## 4. Decision required before the Task 2 plan

The migration plan's shape depends on the intended end state for the FE/BE dialect mismatch. See the question posed to the user — options are: (A) rebuild the settings UI natively over the real config schema, (B) build a backend `AppSettings` adapter to keep the existing FE shape, or (C) a hybrid that keeps the FE shell but re-backs each section onto `config_*_section` and expands to the full schema.
