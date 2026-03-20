# Implementation Gap Analysis

> Generated: 2026-03-19
> Scope: Backend (33 Rust crates) vs Frontend (desktop-ui React 19 + Tauri 2)

## Summary

- **0 High** priority gaps -- OKR, Project CRUD, and Note UI gaps resolved
- **5 Medium** priority gaps -- secondary features or partially integrated
- **6 Low** priority gaps -- nice-to-have, internal-only, or debug-only

---

## 1. Backend Commands Without Frontend Integration

### Fully Missing (no `useQuery`/`useMutation`/`ipc()` call anywhere in `desktop-ui/src/`)

| Command | Module | Description | Priority | Suggested Approach |
|---------|--------|-------------|----------|-------------------|
| `entity_search` | `entities.rs` | Search knowledge graph entities | Medium | Add global entity search in knowledge base |
| `entity_merge` | `entities.rs` | Merge duplicate entities | Low | Add merge action in entity detail |
| `entity_get_neighborhood` | `entities.rs` | Get entity relationship graph | Medium | Wire into knowledge graph visualization |
| `agent_list_profiles` | `agents.rs` | List custom agent profiles | Low | Add agent profiles section in settings |
| `agent_read_file` | `agents.rs` | Read agent profile file | Low | Wire into agent profile editor |
| `agent_write_file` | `agents.rs` | Write agent profile file | Low | Wire into agent profile editor |
| `agent_create_profile` | `agents.rs` | Create new agent profile | Low | Add creation dialog in settings |
| `agent_create_skill` | `agents.rs` | Create custom skill for agent | Low | Add skill creation in profile editor |
| `agent_delete_file` | `agents.rs` | Delete agent file | Low | Add delete in file browser |
| `workspace_list_files` | `workspace.rs` | List workspace files | Low | Show workspace browser in settings or debug |
| `workspace_read_file` | `workspace.rs` | Read workspace file | Low | Show file content in workspace browser |
| `workspace_write_file` | `workspace.rs` | Write workspace file | Low | Wire into workspace file editor |
| `task_reject_decomposition` | `tasks.rs` | Reject a task decomposition | Low | Already using `task_apply_decomposition`; add reject button |
| `cognitive_system_status` | `cognitive.rs` | Get cognitive system health | Low | Add to debug/system tab |
| `cognitive_fact_create` | `cognitive.rs` | Manually create a semantic fact | Low | Debug only -- already in MemoryTab |
| `cognitive_fact_update` | `cognitive.rs` | Update a semantic fact | Low | Debug only |
| `cognitive_rule_create` | `cognitive.rs` | Create procedural rule | Low | Debug only |
| `cognitive_rule_deactivate` | `cognitive.rs` | Deactivate a rule | Low | Debug only |
| `cognitive_inject_event` | `cognitive.rs` | Inject a domain event (debug) | Low | Debug only |
| `note_search_semantic` | `notes.rs` | Pure semantic note search | Low | `note_search_hybrid` is used instead -- OK |
| `note_insight_regenerate_tab` | `notes.rs` | Regenerate a single insight tab | Low | Add refresh button per insight tab |
| `note_insight_list_personas` | `notes.rs` | List insight personas | Low | Used via `usePersonas` -- partially covered |
| `flashcard_get` | `notes.rs` | Get single flashcard | Low | Used internally by review flow |
| `flashcard_update` | `notes.rs` | Update flashcard content | Low | Add edit capability in flashcard deck view |
| `flashcard_delete` | `notes.rs` | Delete flashcard | Low | Add delete button in flashcard view |
| `flashcard_get_all_due` | `notes.rs` | Get all due cards across decks | Low | Used in learn page -- may already work via `useReviewSession` |
| `productivity_projects_list` | `productivity.rs` | List productivity projects | Medium | Add project management in productivity settings |
| `productivity_project_upsert` | `productivity.rs` | Create/update productivity project | Medium | Add project form in productivity settings |
| `productivity_project_delete` | `productivity.rs` | Delete productivity project | Medium | Add delete action in productivity project list |

### Partially Integrated (called from frontend but only in debug/limited contexts)

| Command | Where Used | Gap |
|---------|-----------|-----|
| `cognitive_pipeline_log` | Debug PipelineTab only | No user-facing exposure |
| `cognitive_event_log` | Debug EventsTab only | No user-facing exposure |
| `update_squad` | Backend exists | Frontend uses `create_squad`/`delete_squad` but not `update_squad` |
| `get_squad` | Backend exists | Frontend lists squads but never fetches a single one by ID |

---

## 2. Dead Code / Unused Public Functions

| Item | Location | Reason Unused | Recommendation |
|------|----------|--------------|----------------|
| `note_search_semantic` | `commands/notes.rs` | `note_search_hybrid` supersedes it | **Keep** -- useful for MCP clients; mark as secondary |
| `workflow_get` | `commands/workflows.rs` | Frontend uses `workflow_list` + `workflow_get_effective` | **Keep** -- needed for future single-workflow edit |
| `cron_status` | `commands/cron.rs` | Frontend fetches `cron_list` but not `cron_status` | **Wire** into automations page status badge |
| `finance_exchange_rates` | `commands/finance.rs` | Backend implements rates; no UI to display them | **Wire** into finance overview currency section |
| `finance_report_income` | `commands/finance.rs` | Income report not displayed | **Wire** into cash flow page |
| `finance_report_trends` | `commands/finance.rs` | Trends report not displayed | **Wire** into finance charts |
| `distraction_allow_temp` / `distraction_allow_session` | `commands/distraction.rs` | Wired via `distraction_respond` dispatch | OK -- integrated |
| `productivity_auto_focus_start` / `_end` | `commands/productivity.rs` | Called from backend events only | OK -- `productivity_auto_focus_confirm` now exists |

---

## 3. Feature Packages Without UI

| Feature | Backend Status | Frontend Status | Priority | Effort Estimate |
|---------|---------------|----------------|----------|-----------------|
| **feature-tasks** | Complete (CRUD, focus, suggestions, decompose, forecast) | **Full UI** in `features/tasks/` | -- | -- |
| **feature-finance** | Complete (accounts, transactions, budgets, goals, liabilities, portfolios, investments, reports) | **Full UI** in `features/finance/` (overview, cash flow, investments, targets) | Low | Income report + trends charts missing |
| **feature-notes** | Complete (CRUD, notebooks, links, search, insights, personas, flashcards, versions, annotations, archive) | **Full UI** in `features/notes/` (editor, graph, insights, flashcards, language) + archive/tag filtering via `TagsExplorer` | -- | -- |
| **feature-productivity** | Complete (tracking, focus, goals, categories, calendar, insights, distraction, patterns, hourly breakdown) | **Full UI** in `features/productivity/` (day/week/month views, focus timer, goals, categories, insights) | Low | Project management section missing |
| **feature-coaching** | Complete (signal accumulation, pattern detection, intervention routing, feedback) | **Partial UI** -- debug CoachingTab + nudge banners in chat | Medium | Dedicated coaching dashboard with patterns/feedback history |
| **feature-insights** | Complete (insight review, versions, evolution, personas, scenarios, knowledge growth, flashcard gen) | **Full UI** integrated into notes insight panels | -- | -- |
| **feature-launcher** | Complete (search, dashboard, clipboard, scripts, system commands, app index) | **Full UI** in `features/launcher/` | -- | -- |
| **feature-learning** | Card generator + types only | **Basic UI** in `features/learn/` (review sessions, deck list) | Medium | Expand with progress tracking, spaced repetition stats |
| **activity-log** | Complete (ingestion, work context tool, timeline) | **Integrated** via productivity + work-contexts features | -- | -- |
| **plugin-runtime** | WASM plugin loading + FeaturePackage bridge | **No UI** | Medium | Add plugin management page in settings (install/enable/disable) |
| **OKR system** (objectives + key_results) | Complete backend (CRUD, metrics) | **Full UI** in `features/projects/components/okr/` (OkrTab, ObjectiveCard, KeyResultRow, create/edit modals) | -- | -- |

---

## 4. Unexposed Tools

### Tools registered in the agent vs MCP whitelist

| Tool Name | Registered in Agent | In MCP `default_exposed_tools()` | Accessible via Desktop Commands | Recommendation |
|-----------|-------------------|--------------------------------|-------------------------------|----------------|
| `tasks` | Yes | Yes | Yes | -- |
| `project` | Yes | Yes | Yes (via `project_*` commands) | -- |
| `area` | Yes | Yes | Yes (via `area_*` commands) | -- |
| `notes` | Yes | Yes | Yes (via `note_*` commands) | -- |
| `memory` | Yes | Yes | Yes (via `cognitive_*` commands) | -- |
| `okr` | Yes | Yes | Yes (via `features/projects/components/okr/`) | -- |
| `finance` | Yes | Yes | Yes | -- |
| `productivity` | Yes | Yes | Yes | -- |
| `work_context` | Yes | Yes | Yes | -- |
| `agent` | Yes | Yes | Yes (chat_send delegates) | -- |
| `annotate` | Yes | Yes | Yes (via `annotation_*` commands) | -- |
| `learning` | Yes | Yes | No desktop commands | Wire desktop commands |
| `cron` | Yes | Yes | Yes (via `cron_*` commands) | -- |
| `spawn` | Yes | **No** | No (internal) | Internal only -- OK |
| `delegate` | Yes | **No** | No (internal) | Internal only -- OK |
| `agent_task` | Yes | **No** | No (internal) | Internal only -- OK |
| `context_request` | Yes | **No** | No (internal) | Internal only -- OK |
| `message` | Yes | **No** | No (internal) | Internal only -- OK |
| `ask_user` | Yes | **No** | No (internal) | Internal only -- OK |
| `web_search` | Yes | **No** | No (internal) | Keep internal |
| `web_fetch` | Yes | **No** | No (internal) | Keep internal |
| `grep` | Yes | **No** | No (internal) | Keep internal |
| `glob` | Yes | **No** | No (internal) | Keep internal |
| `browser` | Yes | **No** | No (internal) | Keep internal |

**MCP whitelist is now complete.** `annotate`, `learning`, and `cron` added to `default_exposed_tools()`.

---

## 5. Unregistered Migrations

| Feature | Migration Defined? | Registered in `init/`? | Gap |
|---------|-------------------|----------------------|-----|
| **feature-notes** | `migrations_static()` v1 | `init/storage.rs` | None |
| **feature-tasks** | `migration_sql()` v1 | `init/storage.rs` | None |
| **feature-finance** | `migrations_static()` v1 | `init/storage.rs` | None |
| **feature-productivity** | `migrations_static()` v1 | `init/productivity.rs` | None |
| **feature-launcher** | `migrations_static()` v1 | `init/launcher.rs` | None |
| **activity-log** | `migrations_static()` v1 | `init/agent.rs` | None |
| **cognitive** | `cognitive_migrations()` v1-v2 | `agent/builder.rs` (agent build) | None |
| **feature-coaching** | No `FeatureMigration` defined | N/A | None -- uses cognitive tables |
| **feature-insights** | No `FeatureMigration` defined | N/A | None -- uses cognitive + notes tables |
| **feature-learning** | No `FeatureMigration` defined | N/A | None -- uses cognitive flashcard tables |
| **plugin-runtime** | Dynamic from plugin manifest | Runs at plugin load time | None |

**All migrations are properly registered.** No gaps found.

---

## 6. Config Fields Without Settings UI

Config sections are exposed via `config_get_section` / `config_update_section`. The settings UI only covers a subset.

| Config Section | Field(s) | Type | Has UI? | Priority |
|----------------|----------|------|---------|----------|
| `agents` | `defaults.model`, `defaults.provider`, `defaults.temperature`, `defaults.maxTokens` | strings/numbers | Yes (General + Personalization) | -- |
| `agents` | `defaults.maxToolIterations`, `defaults.workspace` | number, string | **No** | Low |
| `providers` | All provider API keys and bases | Secret<String> | Yes (Personalization) | -- |
| `channels` | telegram/discord/slack/email enabled + tokens | various | Yes (Configuration) | -- |
| `tools` | `restrictToWorkspace`, `web.braveApiKey` | bool, Secret | Yes (Configuration) | -- |
| `tools` | `permissions` (per-channel permission levels) | struct | **No** | Low |
| `gateway` | `host`, `port` | string, u16 | Yes (Configuration) | -- |
| `todo` | `focus.maxSlots`, `focus.deadlineHours` | numbers | Yes (Tasks & Notifications) | -- |
| `todo` | `notifications.*` (targets, focusReminders, dailyDigest, dailyDigestTime) | various | Yes (Tasks & Notifications) | -- |
| `todo` | `enrichment`, `search`, `dailyPlanning` | structs | Yes (Tasks & Notifications) | -- |
| `confidence` | `confirmThreshold`, `warnThreshold` | f64 | **No** | Low |
| `project` | Various | struct | **No** | Low |
| `conversation` | `maxHistoryMessages`, `embedding.*`, `search.*` | various | **No** | Low |
| `learning` | `enabled`, `analysisIntervalSecs`, thresholds | various | Yes (Personalization) | -- |
| `finance` | `defaultCurrency`, `fire.*` | various | Partial (setup wizard for FIRE) | Low |
| `notes` | Various | struct | **No** | Low |
| `productivity` | `enabled`, `tracking.*`, `focus.*` | various | **No** | Medium |
| `orchestrator` | Various | struct | **No** | Low |
| `providerManager` | `primary`, `fallback`, routing | struct | Yes (Personalization) | -- |
| `cognitive` | `provider`, `model`, `temperature`, `maxTokens`, `reflectionMaxTokens` | various | Yes (Personalization) | -- |
| `user` | `name` | string | Yes (setup wizard) | -- |
| `workContext` | Various | struct | **No** | Low |
| `capture` | `shellHook.*`, `fileWatcher.*`, `ingestionApi.*` | various | Partial (Integrations page for shell hook + token) | Low |
| `content` | Various | struct | **No** | Low |
| `mcp` | Server definitions, enabled, auth | struct | Yes (MCP Servers page) | -- |
| `skills` | Various | struct | **No** | Low |
| `integrations` | `aiTools.*` | struct | Partial (setup wizard AI tools step) | Low |
| `language` | `sourceLang`, `targetLang`, `proficiency` | strings | Partial (via `useLanguageConfig` query) | Low |
| `launcher` | `sources.*` (apps, clipboard, scripts, etc.) | struct | Yes (Launcher settings) | -- |
| `scenario` | Various | struct | **No** | Low |
| `shortcuts` | `launcher`, `tray`, `quickCapture` | strings | Yes (General settings) | -- |
| `plugins` | `enabled`, `directory`, `autoUpdate` | various | **No** | Medium |
| `packs` | Feature pack toggles | struct | **No** | Low |
| `timezone` | Auto-detected string | string | **No** | Low |

---

## 7. Priority Action Plan

### Medium Priority (secondary features or partial integration)

1. **Coaching Dashboard** -- Coaching has full backend (signals, patterns, feedback, interventions) but only appears in debug tab + chat nudge banners.
   - Effort: 2 days
   - Build a dedicated coaching insights page or integrate into productivity

2. **Productivity Projects** -- `productivity_projects_list`, `productivity_project_upsert`, `productivity_project_delete` are not called from any UI.
   - Effort: 1 day
   - Add project assignment in productivity category settings

3. **Plugin Management UI** -- `plugin-runtime` has WASM plugin support but no settings page for plugin management.
   - Effort: 1 day
   - Add plugin list with install/enable/disable in settings

4. **Project Context Panels** -- `project_conversations_list`, `project_memories_list`, `project_source_*` commands exist but no UI shows project conversations, memories, or sources.
   - Effort: 1-2 days
   - Add sidebar panels to project detail view

5. **Finance Reports** -- `finance_report_income`, `finance_report_trends`, `finance_exchange_rates` have no UI.
   - Effort: 1 day
   - Add income report + trends charts to finance pages

### Low Priority (nice-to-have or internal-only)

6. **Entity Graph Explorer** -- `entity_search`, `entity_merge`, `entity_get_neighborhood` exist but no dedicated entity management UI.
    - Effort: 2 days

7. **Agent Profiles UI** -- `agent_*` commands for managing custom agent profiles have no frontend.
    - Effort: 1 day

8. **Workspace File Browser** -- `workspace_*` commands have no frontend.
    - Effort: 1 day

9. **Cron Status Badge** -- `cron_status` not called; could show system health in automations page.
    - Effort: 0.5 days

10. **Various Config Sections** -- Many config sections (confidence, conversation, content, skills, packs, scenario, orchestrator) have no settings UI. These are advanced tuning parameters that most users will never need.
    - Effort: Low priority; expose on demand

11. **Flashcard Edit/Delete** -- `flashcard_update` and `flashcard_delete` not called from frontend.
    - Effort: 0.5 days
