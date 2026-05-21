# Unified Settings System — Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **This is a master/program plan.** Phase 0 (Foundation) and Phase 1 (first vertical slice) are fully concrete and produce working, shippable software on their own. Phases 2+ are a *recipe + backlog*: each domain phase is generated into its own detailed plan from the §Recipe template at the time it is executed, so we never carry placeholder code. Read §0–§3 first; §4 onward is the rollout map.

**Goal:** Replace the non-functional, ported-from-another-product settings UI with a native, schema-driven settings system that reads and writes KlyntBot's real `config.json` (37 modules, ~310 fields) plus its personalization surfaces, using the canonical direct-`invoke()` pattern.

**Architecture:** Every settings domain is a thin React section that calls the *already-working* generic backend bridge `config_get_section(section)` / `config_update_section(section, patch)` (deep-merge + schema-validate + hot-reload). No new backend commands are needed for the 37 modeled domains — the `section` string *is* the contract. New backend commands are added only for surfaces that live outside `config.json` (soul, persona files) and for the orphaned `AppSettings` fields, which we relocate into a new `config.ui` section rather than the dead `get_app_settings`/`update_app_settings` stubs. The foreign `AppSettings` + `@tanstack/react-query` settings stack is decommissioned.

**Tech Stack:** Rust (Tauri 2, `app-core` handlers + `desktop` command shells), `crates/config` serde schema, React + TypeScript (`desktop-ui`), direct `invoke()` from `@/api/client`, Vitest + `@testing-library/react`, `cargo nextest`.

---

## 0. Context every implementer must load first

Read these before touching code:
- `docs/architecture/settings-integration/00-overview.md` — the central finding and master list.
- `docs/architecture/settings-integration/0[1-4]-*.md` — the four scan reports (FE inventory, BE schema, hardcoded scan, IPC/personalization).
- `desktop-ui/src/features/models/hooks/useProviders.ts` — **the reference implementation** of the canonical pattern: direct `invoke("config_get_section", { section })`, `useState`/`useEffect`, no react-query.
- `crates/desktop/src/commands/settings.rs` — command shells + the `dispatch_dev` dev-server mirror.
- `CLAUDE.md` §"Adding a Tauri command" and §"Coding patterns".

### Hard rules (from CLAUDE.md — non-negotiable)
1. **New components use Tailwind**, direct `invoke()` from `@/api/client`, `useState`+`useEffect`. **No `@tanstack/react-query`, no `useTauriQuery`, no `@services/tauri`** in new settings code.
2. **Adding a Tauri command is a 4-touch ritual:** (a) `#[klynt_command]` in `commands/*.rs`, (b) register path in `klynt_collect_commands![...]` in `specta_builder.rs`, (c) add a `dispatch_dev` arm in the same command file, (d) run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`. The `registration_drift` + `bindings_are_current` tests fail until all four are done.
3. **AppCore handler methods** are annotated `#[tracing::instrument(skip(self), err)]`; command shells are NOT.
4. **Tests use `StoragePool::connect_in_memory()`**; pre-release schema changes are made in place (no migration files).
5. **Zero clippy warnings.** Run `cargo clippy --workspace --all-targets --all-features` and `cargo fmt --all --check`.

### The wire contract (memorize)
```ts
// READ a whole section as raw JSON:
const voice = await invoke("config_get_section", { section: "voice" }) as VoiceConfig;
// WRITE a partial patch (deep-merged server-side; null deletes a key):
await invoke("config_update_section", { section: "voice", patch: { silenceDurationMs: 1200 } });
```
Section strings are the camelCase `Config` field names in `crates/config/src/schema/core.rs:101`:
`agents, channels, providers, tools, gateway, todo, confidence, project, conversation, learning, notes, productivity, providerManager, packs, cognitive, user, workContext, capture, content, mcp, integrations, language, launcher, scenario, shortcuts, autotuner, lifecycle, voice, languageLearning` (+ scalars `timezone`, `setupCompleted`).

---

## 1. File structure (what gets created / decommissioned)

**New shared FE foundation** (`desktop-ui/src/features/settings/`):
- `lib/useConfigSection.ts` — generic hook: load + patch one config section via `invoke`. The spine every domain rides.
- `lib/configSectionTypes.ts` — hand-mirrored TS types per section (camelCase), kept in sync with `bindings.ts` where generated types exist.
- `registry/settingsDomains.ts` — declarative registry: domain id → label → icon → section(s) → component. Drives nav + routing so adding a domain is one entry.
- `components/SettingsShell.tsx` — new nav + content frame replacing the ported `SettingsView`/`SettingsNav`.
- `components/fields/` — reusable Tailwind controls: `ToggleField`, `SelectField`, `TextField`, `NumberField`, `SecretField`, `SliderField` — each takes `{ value, onChange, label, description, dirty }`.

**New backend commands** (`crates/desktop/src/commands/personalization.rs` + `app-core` handlers):
- `soul_read()` / `soul_write(content)` — read/write `~/.klyntbot/KLYNTBOT.md`.
- (later) `persona_read(agentName)` / `persona_write(agentName, content)` OR allowlist extension in `handlers/agents.rs:403`.

**Schema additions** (`crates/config/src/schema/`):
- New `ui.rs` (`UiConfig`) section to home the orphaned `AppSettings` fields worth persisting (theme, uiScale, fonts, notification prefs, etc.). Wired into `Config` in `core.rs`.
- Promoted fields (report 03) added to their natural existing modules (e.g. `voice.silenceDurationMs`, `agents.defaults.approvalTimeoutSecs`).

**Decommissioned** (Phase D):
- `desktop-ui/src/features/settings/hooks/useAppSettings.ts` and the `AppSettings` type usage in settings.
- `desktop-ui/src/api/endpoints/settings.ts` `get_app_settings`/`update_app_settings` dead stubs.
- All `@tanstack/react-query`/`useTauriQuery` usage *within the settings feature* (broader UI migration is out of scope for this plan).
- Coding-IDE-only sections that don't map to any KlyntBot concept: `SettingsCodexSection`, `SettingsEnvironmentsSection`, worktree/remote-backend controls (confirm per-section in Phase D before deletion).

---

## 2. Phasing & dependency order

```
Phase 0  Foundation (useConfigSection + field components + registry + shell)   ← blocks everything
Phase 1  Vertical slice: Models & Providers (proves the recipe end-to-end)     ← depends on 0
Phase 2  App & UI domain  (creates config.ui, fixes theme/fonts persistence)   ← depends on 0; unblocks Phase D
Phase 3  Voice                                                                 ┐
Phase 4  Channels & Integrations + MCP UI                                      │ each depends on 0,
Phase 5  Productivity & Focus                                                  │ independent of each other
Phase 6  Memory & Intelligence (cognitive)                                     │ → parallelizable
Phase 7  Language & Learning                                                   │
Phase 8  Launcher                                                              │
Phase 9  Privacy & Security (approvals, capture, sandbox)                      ┘
Phase H  Hardcoded → config promotions (report 03), surfaced in owning domains ← per-field, after owning domain
Phase P  Personalization: Soul editor, Persona files, Workspace files, Hotkeys, Secrets ← Soul/Persona need new cmds
Phase D  Decommission AppSettings + react-query + dead coding-IDE sections     ← LAST (after 2 lands theme)
```

Critical path: **0 → 1 → 2 → D**. Everything else parallelizes after 0.

---

## 3. Phase 0 — Foundation (fully concrete, TDD)

### Task 0.1: `useConfigSection` generic hook

**Files:**
- Create: `desktop-ui/src/features/settings/lib/useConfigSection.ts`
- Test: `desktop-ui/src/features/settings/lib/useConfigSection.test.ts`

- [ ] **Step 1: Write the failing test**
```ts
import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

const invoke = vi.fn();
vi.mock("@/api/client", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { useConfigSection } from "./useConfigSection";

describe("useConfigSection", () => {
  beforeEach(() => invoke.mockReset());

  it("loads a section on mount", async () => {
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1500 });
    const { result } = renderHook(() => useConfigSection<{ silenceDurationMs: number }>("voice"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invoke).toHaveBeenCalledWith("config_get_section", { section: "voice" });
    expect(result.current.value).toEqual({ silenceDurationMs: 1500 });
  });

  it("patches and optimistically updates", async () => {
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1500 }); // initial load
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1200 }); // update echo
    const { result } = renderHook(() => useConfigSection<{ silenceDurationMs: number }>("voice"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => { await result.current.patch({ silenceDurationMs: 1200 }); });
    expect(invoke).toHaveBeenCalledWith("config_update_section", {
      section: "voice",
      patch: { silenceDurationMs: 1200 },
    });
    expect(result.current.value).toEqual({ silenceDurationMs: 1200 });
  });
});
```

- [ ] **Step 2: Run to verify it fails** — `cd desktop-ui && bun run test useConfigSection` → FAIL (module not found).

- [ ] **Step 3: Implement**
```ts
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function useConfigSection<T extends object>(section: string) {
  const [value, setValue] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = (await invoke("config_get_section", { section })) as T;
        if (!cancelled) setValue(data);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [section]);

  const patch = useCallback(
    async (p: Partial<T>) => {
      const updated = (await invoke("config_update_section", { section, patch: p })) as T;
      setValue(updated);
      return updated;
    },
    [section],
  );

  return { value, loading, error, patch, setValue };
}
```

- [ ] **Step 4: Run to verify it passes** — `cd desktop-ui && bun run test useConfigSection` → PASS.

- [ ] **Step 5: Commit** — `git add desktop-ui/src/features/settings/lib && git commit -m "feat(settings): add useConfigSection bridge hook"`

### Task 0.2: Reusable field components

**Files:**
- Create: `desktop-ui/src/features/settings/components/fields/{ToggleField,SelectField,TextField,NumberField,SecretField,SliderField}.tsx`
- Create: `desktop-ui/src/features/settings/components/fields/index.ts`
- Test: `desktop-ui/src/features/settings/components/fields/ToggleField.test.tsx` (one representative test; replicate per component)

- [ ] **Step 1: Write the failing test for ToggleField**
```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ToggleField } from "./ToggleField";

describe("ToggleField", () => {
  it("renders label and fires onChange", () => {
    const onChange = vi.fn();
    render(<ToggleField label="Enabled" value={false} onChange={onChange} />);
    expect(screen.getByText("Enabled")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
```

- [ ] **Step 2: Run to verify it fails** — `cd desktop-ui && bun run test ToggleField` → FAIL.

- [ ] **Step 3: Implement ToggleField (Tailwind, role="switch")**
```tsx
type Props = { label: string; description?: string; value: boolean; onChange: (v: boolean) => void; };
export function ToggleField({ label, description, value, onChange }: Props) {
  return (
    <label className="flex items-center justify-between gap-4 py-2">
      <span className="flex flex-col">
        <span className="text-[var(--fs-base)]">{label}</span>
        {description && <span className="text-[var(--fs-xs)] opacity-70">{description}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={value}
        onClick={() => onChange(!value)}
        className={`h-5 w-9 rounded-full transition-colors ${value ? "bg-accent" : "bg-neutral-500/40"}`}
      >
        <span className={`block h-4 w-4 rounded-full bg-white transition-transform ${value ? "translate-x-4" : "translate-x-0.5"}`} />
      </button>
    </label>
  );
}
```

- [ ] **Step 4: Run to verify it passes** → PASS. Then implement the other 5 field components following the same shape (use `var(--fs-*)` tokens, never hardcoded px — CLAUDE.md typography rule). `SecretField` masks input and shows a "configured" badge instead of the value.

- [ ] **Step 5: Commit** — `git commit -am "feat(settings): add reusable settings field components"`

### Task 0.3: Domain registry + shell

**Files:**
- Create: `desktop-ui/src/features/settings/registry/settingsDomains.ts`
- Create: `desktop-ui/src/features/settings/components/SettingsShell.tsx`
- Test: `desktop-ui/src/features/settings/components/SettingsShell.test.tsx`

- [ ] **Step 1: Write failing test** — render `SettingsShell` with a 1-entry stub registry, assert the nav item renders and clicking it mounts the domain component.
```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SettingsShell } from "./SettingsShell";

it("renders nav from registry and switches content", () => {
  const registry = [{ id: "demo", label: "Demo", Component: () => <div>DEMO BODY</div> }];
  render(<SettingsShell domains={registry} />);
  expect(screen.getByText("Demo")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Demo"));
  expect(screen.getByText("DEMO BODY")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run to verify it fails** → FAIL.

- [ ] **Step 3: Implement** the registry type (`{ id: string; label: string; icon?: ReactNode; Component: ComponentType }`) and a `SettingsShell` that takes `domains` (defaulting to the real registry), renders a left nav, tracks `activeId` with `useState`, and mounts `domains.find(d => d.id === activeId).Component`. Seed `settingsDomains.ts` with the empty real registry (entries added per phase).

- [ ] **Step 4: Run to verify it passes** → PASS.

- [ ] **Step 5: Commit** — `git commit -am "feat(settings): add domain registry and SettingsShell"`

### Task 0.4: Mount the shell behind a flag (no UI regression)

- [ ] **Step 1:** Add a temporary route/tab that renders `SettingsShell` alongside the existing `SettingsView` (do NOT delete the old one yet — that's Phase D). Verify the app builds: `cd desktop-ui && bun run typecheck && bun run build`.
- [ ] **Step 2: Commit** — `git commit -am "chore(settings): mount new SettingsShell behind dev tab"`

**Phase 0 exit criteria:** `bun run test`, `bun run typecheck`, `bun run lint` all green; the shell renders an empty nav; the old settings UI is untouched.

---

## 4. Phase 1 — Vertical slice: Models & Providers (fully concrete)

Proves the entire recipe end-to-end against a domain that already has a partial native reader (`useProviders.ts`). Sections involved: `providers` (API keys, ~13 `Secret<String>`), `agents.defaults` (model/temperature/maxTokens/maxToolIterations — **hot-reloadable**), `agents.monthlyBudgetUsd`, `providerManager`.

### Task 1.1: TS types for the providers + agents sections
- [ ] **Step 1:** In `lib/configSectionTypes.ts`, mirror the relevant fields (cross-check names against `crates/config/src/schema/providers.rs` and `agents.rs`, and `bindings.ts` if generated types exist):
```ts
export type ProvidersConfig = Record<string, { apiKey?: string | null; baseUrl?: string | null }>;
export type AgentsConfig = {
  defaults?: { provider?: string | null; model?: string | null; temperature?: number; maxTokens?: number; maxToolIterations?: number };
  monthlyBudgetUsd?: number | null;
};
```
- [ ] **Step 2: Commit** — `git commit -am "feat(settings): add providers/agents config types"`

### Task 1.2: `SettingsModelsSection` component (TDD)
**Files:** Create `components/sections/SettingsModelsSection.tsx` + `.test.tsx`.
- [ ] **Step 1: Write failing test** — mock `invoke`; assert it reads `agents` + `providers`, renders a model `SelectField`, a temperature `SliderField`, and a `SecretField` per provider; editing temperature calls `config_update_section` with `{ section: "agents", patch: { defaults: { temperature: <v> } } }`.
- [ ] **Step 2: Run → FAIL.**
- [ ] **Step 3: Implement** using `useConfigSection<AgentsConfig>("agents")` + `useConfigSection<ProvidersConfig>("providers")` and the field components. Save-on-blur via `patch`. Show a "applies immediately" hint on hot-reloadable fields (model/temperature/maxTokens/maxToolIterations).
- [ ] **Step 4: Run → PASS.**
- [ ] **Step 5: Commit.**

### Task 1.3: Register the domain
- [ ] Add `{ id: "models", label: "Models & Providers", Component: SettingsModelsSection }` to `settingsDomains.ts`. Verify it appears in the shell nav. Commit.

### Task 1.4: Manual + backend verification (per superpowers:verification-before-completion)
- [ ] Run the app (`cargo tauri dev` + `cd desktop-ui && bun run dev`), change temperature, confirm `~/.klyntbot/config.json` (or `$KLYNTBOT_HOME`) shows `agents.defaults.temperature`, and that a chat turn picks it up without restart (hot-reload). Record evidence (file diff + behavior) before claiming done.

**Phase 1 exit criteria:** API keys can be entered and persist; default model/temperature/budget changes survive restart and hot-reload mid-session; zero clippy/lint/type errors.

---

## 5. The per-domain Recipe (template for Phases 2–9)

> Each phase below is generated into its own dated plan (`docs/architecture/settings-integration/plans/`) from this recipe when work starts. Do **not** pre-write placeholder code for all domains.

For domain **D** mapping to section(s) `S`:
1. **Type task:** mirror `S`'s fields into `lib/configSectionTypes.ts` from `crates/config/src/schema/<S>.rs`. Cross-check camelCase names; capture nested structs.
2. **Component task (TDD):** `Settings<D>Section.tsx` + test. Use `useConfigSection<SConfig>(S)` and field components. Group fields into labeled subsections matching the schema's nested structs. Write the failing test first (assert read on mount + a representative `config_update_section` patch), then implement, then pass.
3. **Registry task:** add the `{ id, label, icon, Component }` entry.
4. **Restart-vs-hot copy:** for each field, label whether it hot-reloads (only `agents.defaults.{model,temperature,maxTokens,maxToolIterations}`, `agents.execution.safetyTimeoutSecs`, `agents.monthlyBudgetUsd` — `schema/hot.rs`) or needs restart ("takes effect after restart" hint). Everything else = restart.
5. **Secrets:** any `Secret<String>` field uses `SecretField` (masked + "configured" badge; never render the raw value).
6. **Feature flags:** booleans that enable/disable a subsystem get a `ToggleField` with a "requires restart" note (channel listeners, browser automation, file watcher, etc. are restart-only per report 02).
7. **Verify:** edit in-app, confirm `config.json` diff, confirm behavior. Evidence before "done".
8. **Commit per task.**

---

## 6. Phases 2–9 — Domain rollout backlog

Each row → one plan generated from §5. Field counts/notes from report 02.

| Phase | Domain | Section(s) | Highlights | New cmd? |
|---|---|---|---|---|
| 2 | **App & UI** | new `ui` + `user`, `shortcuts` | Create `UiConfig` (theme, uiScale, uiFontFamily, codeFontFamily, codeFontSize, notification toggles, threadTitleAutogen, autoUpdateChecks — relocated from dead `AppSettings`); user name; timezone. **Unblocks Phase D theme persistence.** | Schema add (`ui.rs`); no new IPC (uses `config_*_section`) |
| 3 | **Voice** | `voice` | ~28 fields: STT/TTS engine, personas, conversation tuning. Promotes hardcoded silence (1500ms) + idle-unload TTL (300s) here (Phase H). | No |
| 4 | **Channels & Integrations** | `channels`, `integrations`, `mcp` | ~45 channel fields + tokens (Secret), listener enable flags (restart), **MCP UI uses dedicated `mcp_*` commands** (not `config_*_section`) for live reconnect. | No (mcp_* already exist) |
| 5 | **Productivity & Focus** | `productivity`, `todo`, `lifecycle` | ~30 fields: tracking, nudges, scheduling. Promotes coaching rate limits, task warning hours (Phase H). | No |
| 6 | **Memory & Intelligence** | `cognitive`, `conversation`, `confidence` | ~70 cognitive fields, 12 nested structs — the richest domain; group aggressively into subsections. Promotes memory retrieval limit (Phase H). | No |
| 7 | **Language & Learning** | `language`, `languageLearning`, `learning` | FSRS, flashcards, pronunciation. | No |
| 8 | **Launcher** | `launcher` | ~17 source toggles + window sizing (promote launcher/tray sizes, Phase H). | No |
| 9 | **Privacy & Security** | `tools`, `capture`, `agents.execution`, `packs` | Approval policy, workspace sandbox, capture sources (shell hook/file watcher default-off), feature packs. Promotes approval timeout (600s) (Phase H). | No |

Also surface, where they fit: `gateway` (port — Advanced), `autotuner` (Advanced/Developer), `scenario`, `content`, `workContext`, `project`, `notes`, `providerManager` — fold into the nearest domain or an **Advanced / Developer** domain.

---

## 7. Phase H — Hardcoded → config promotions (report 03)

Each promotion is a paired BE+FE change executed *after* its owning domain phase exists. Pattern per value:

1. **Schema task (BE, TDD):** add the field to the owning module with the current value as `#[serde(default = ...)]`. Test (`cargo nextest`) that `Config::default()` yields the existing constant.
2. **Wire-through task (BE):** replace the hardcoded `const` read site with a read of the config field (inject config where the const lived). Test the consuming behavior honors the config value.
3. **Surface task (FE):** add the field to the owning domain section (recipe §5).
4. If hot-reload is desired, add it to `HotConfig` (`schema/hot.rs`) + the `HotConfig::from` mapping and assert via the config-reload test.

Promotion backlog (priority order, file:line in report 03): approval timeout (600s, owning: Privacy), memory retrieval limit (30, Memory), subagent turn cap (500, Models), voice idle-unload TTL (300s) + silence (1500ms, Voice), coaching rate limits (Productivity), brain-voice pulse cap (Voice), launcher/tray window sizes (Launcher), long-running tool timeout (600s, Privacy), agent max iterations (10, Models), task warning hours `[6,3,1]` (Productivity), session compaction thresholds (200/100, Memory), max tool result bytes (50k, Privacy), scheduler grace (3600s, Advanced), bootstrap token cap (8000, Memory).

> **Judgment gate:** report 03 marks each value Expose / Maybe / Leave-internal. Only promote Expose (and reviewed Maybe). Do not promote internal constants.

---

## 8. Phase P — Personalization surfaces

### Task P.1: Soul editor — NEW backend commands (TDD, full ritual)
**Files:** `crates/desktop/src/commands/personalization.rs` (new), `crates/app-core/src/handlers/personalization.rs` (new), `crates/desktop/src/specta_builder.rs` (register), FE `components/sections/SettingsSoulSection.tsx`.

- [ ] **Step 1 (BE test):** in app-core, write a test that `soul_write(content)` writes `data_dir/KLYNTBOT.md` and `soul_read()` returns it (use a temp `KLYNTBOT_HOME`).
- [ ] **Step 2:** Run `cargo nextest run -p app-core -E 'test(soul)'` → FAIL.
- [ ] **Step 3 (BE impl):** AppCore handler methods `soul_read`/`soul_write` (`#[tracing::instrument(skip(self), err)]`) reading/writing `config.data_dir_path()/"KLYNTBOT.md"` (path per report 04 §3a; default content from `crates/skill-system/src/soul.rs::DEFAULT_SOUL` when missing). Add `#[klynt_command]` shells in `personalization.rs`, register in `klynt_collect_commands!`, add `dispatch_dev` arms.
- [ ] **Step 4:** `cargo nextest` → PASS; `cargo tauri dev` once to regen `bindings.ts`; confirm `bindings_are_current` + `registration_drift` tests pass.
- [ ] **Step 5 (FE TDD):** `SettingsSoulSection` — load via `invoke("soul_read")`, edit in a textarea (reuse `Markdown` preview from `@/features/messages/components/Markdown`), save via `invoke("soul_write", { content })`. Test with mocked invoke.
- [ ] **Step 6:** Register domain "Persona & Soul". Commit per step.

### Task P.2: Persona files (PERSONA.md)
- [ ] Extend the `agent_read_file`/`agent_write_file` allowlist at `handlers/agents.rs:403` to include `PERSONA.md` (TDD: test that a write to `PERSONA.md` is now accepted and hot-reloads), OR add `persona_read/persona_write` if cleaner. Surface in the same domain as Soul.

### Task P.3: Workspace files panel (seam already exists)
- [ ] New section calling existing `workspace_list_files`/`workspace_read_file`/`workspace_write_file` (AGENTS.md, USER.md, TOOLS.md, RESPONSE.md, HEARTBEAT.md). No new backend.

### Task P.4: Global hotkeys (seam already exists)
- [ ] New "Shortcuts (Global)" section calling existing `shortcuts_get`/`shortcuts_update` (launcher/tray). Distinct from the editor-shortcut section. Validate + rollback handled server-side.

### Task P.5: MCP servers UI (seam already exists)
- [ ] Section calling `mcp_get_config`/`mcp_add_server`/`mcp_remove_server`/`mcp_toggle_server`/`mcp_update_server`. Lives under Channels & Integrations (Phase 4) or its own domain.

### Task P.6: Secrets / API keys
- [ ] Covered structurally by Phase 1 `SecretField` over `providers`; ensure MCP OAuth (`mcp_oauth_start`/`mcp_oauth_disconnect`) is surfaced in the MCP UI.

---

## 9. Phase D — Decommission the foreign stack (LAST)

Only after Phase 2 has relocated theme/fonts/UI prefs into `config.ui` and every still-relevant control has a native home.

- [ ] **Step 1:** Grep for every consumer of `useAppSettings` / `AppSettings` *outside* the settings feature (some non-settings code may read `appSettings.theme` etc. — e.g. `useThemePreference`). Re-point each to the new `config.ui` source via `useConfigSection<UiConfig>("ui")`. Run typecheck after each.
- [ ] **Step 2:** Delete `useAppSettings.ts`, the `get_app_settings`/`update_app_settings` stubs in `api/endpoints/settings.ts`, and the old `SettingsView`/`SettingsNav`/ported section files that are fully superseded.
- [ ] **Step 3:** Confirm per-section that coding-IDE-only sections (`SettingsCodexSection`, `SettingsEnvironmentsSection`, worktree/remote-backend controls) have no KlyntBot meaning before deleting. If any concept is actually wanted, file it as a new schema field instead.
- [ ] **Step 4:** Remove `@tanstack/react-query`/`useTauriQuery` imports from the settings feature. (Workspace-wide react-query removal is explicitly OUT of scope.)
- [ ] **Step 5:** Flip the shell from the dev tab to the primary settings entry point. Full regression: `bun run test && bun run typecheck && bun run lint`, `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets --all-features`.
- [ ] **Step 6: Commit** — `git commit -m "refactor(settings): decommission ported AppSettings stack"`

---

## 10. Self-review (run against the user's two tasks)

**Task 1 (scan everything configurable):** covered by reports 01–04 + this plan's §1/§6/§7/§8 — modeled config (37 modules), hardcoded values (report 03), and personalization (souls/personas/themes/hotkeys/secrets/MCP). ✅
**Task 2 (modular, scalable, intuitive, domain-separated, migrate one-by-one):** §1 file structure (modular field components + registry), §2 phasing, §5 repeatable recipe, §6 per-domain backlog, §9 clean decommission. ✅

**Open confirmations to resolve at execution time:**
- `KLYNTBOT-coding.md` / coding-mode soul: report 04 found no `CodingSoulContextSource` in code, but CLAUDE.md claims per-mode souls exist. Confirm before building the soul editor (one file vs two).
- Exact camelCase field names per module must be cross-checked against each `schema/*.rs` and `bindings.ts` during each domain's Type task — do not trust this plan's abbreviated type sketches verbatim.
- Whether `config.ui` is the right home for relocated `AppSettings` fields vs extending `config.user` — decide in Phase 2 (recommend new `ui` to keep `user` = identity/profile).
```
