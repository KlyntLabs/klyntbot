# Conversational Setup Wizard

**Date:** 2026-03-13
**Status:** Approved

## Overview

Replace the traditional 8-step setup wizard with a conversational onboarding flow. The AI introduces itself and learns about the user through a flowing dialogue — each prompt is a sentence with an inline blank that the user fills in. Completed answers lock into the narrative, building a readable summary of the user's configuration.

## Scope

**Included steps:**
1. Welcome/Name — `"Hello, I'm Klynt. Your name is {input}."`
2. Provider — `"I'll be powered by {input}."` (inline dropdown, default Anthropic)
3. API Key — `"My API key is {input}."` (masked input)
4. Areas — `"Your main areas of focus are {input}."`
5. Productivity — `"Your productivity style is {input}."` (inline dropdown)
6. Finance gate — `"Would you like to set up finance tracking? {input}"` (yes/no dropdown)
7. Finance setup — opens existing finance sub-forms panel (conditional on gate = yes)
8. Complete — `"Great, we're all set. Let's get started!"` + Launch button

**Excluded:** MCP server config (removed from onboarding), Channels config (deferred to Settings — mentioned in conversation as "You can connect Telegram, Discord, and more later from Settings").

## Backend Change: Add User Section to Config

The `Config` struct has no `user` section. A new `UserConfig` must be added:

```rust
// crates/config/src/schema/user.rs (new file)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    #[serde(default)]
    pub name: String,
}
```

Add `pub user: UserConfig` field to `Config` in `core.rs`. This is pre-release, so no migration needed — just add the field with `#[serde(default)]`.

**Gating prerequisite:** The Rust backend changes (`user.rs` + `core.rs` field + `mod.rs` export) MUST land before any frontend work that calls `config_update_section` with `section: "user"`. Without the backend field, the IPC call returns `NOT_FOUND`.

## Architecture: Declarative Conversation Schema

### ConversationNode Type

```ts
type InputType = "text" | "select" | "masked" | "confirm" | "tags" | "complex"

type TranscriptValues = Record<string, string | boolean | string[]>

type ConversationNode = {
  id: string                          // unique key, e.g. "user_name"
  prompt: string                      // sentence template with {input} placeholder
  inputType: InputType
  default?: string | boolean | string[]
  options?: { label: string; value: string }[]
  validate?: (value: string) => string | null
  saveKey: string                     // maps to IPC command / config field
  condition?: (values: TranscriptValues) => boolean
  // Custom save function — overrides default config_update_section behavior
  save?: (value: string | boolean | string[]) => Promise<void>
  // Custom resume loader — overrides default config_get_section behavior
  load?: () => Promise<string | boolean | string[] | null>
}
```

### Schema Definition

| id | prompt | inputType | notes |
|---|---|---|---|
| `user_name` | `"Hello, I'm Klynt. Your name is {input}."` | text | Saves to `user.name` config section |
| `provider` | `"I'll be powered by {input}."` | select | Options use lowercase `value` matching `ProvidersConfig` field names exactly: `anthropic`, `openai`, `openrouter`, `deepseek`, `gemini`, `groq`. Labels can be display names (e.g. "Anthropic"). Model defaults to `anthropic/claude-opus-4-5` — configurable later in Settings |
| `api_key` | `"My API key is {input}."` | masked | Saves to `providers.{selected_provider}.apiKey` |
| `areas` | `"Your main areas of focus are {input}."` | tags | Comma/Enter to add tags. Colors auto-assigned from `AREA_COLORS` constant (shared with existing `AreasStep`). Uses `area_create` IPC per entry |
| `productivity_gate` | `"Would you like to enable productivity tracking? {input}"` | confirm | Default: Yes |
| `finance_gate` | `"Would you like to set up finance tracking? {input}"` | confirm | — |
| `finance_setup` | *(opens finance panel)* | complex | condition: `values.finance_gate === true` |
| `complete` | `"Great, we're all set. Let's get started!"` | — | Calls `config_mark_setup_completed`, navigates to `/` |

**Decisions captured:**
- **No model selection during onboarding.** Provider is selected; model uses the default for that provider (`anthropic/claude-opus-4-5` for Anthropic). Users can change the model later in Settings.
- **Productivity is gate-only.** A yes/no confirm enables tracking with sensible defaults (`focus.defaultDurationMins: 45`, `focus.maxDailyFocusHours: 8`). Detailed sliders (duration, daily target, excluded apps) are available later in Settings.
- **Areas use a `tags` input type.** User types area names separated by comma or Enter. Each becomes a tag/pill. Colors are auto-assigned from `AREA_COLORS` palette. On save, each area calls `area_create` IPC individually.

Adding or removing a step = adding or removing one object in the array.

## ConversationRunner Engine

Single React component that interprets the schema and manages the flow.

### State

```ts
{
  transcript: Record<string, { node: ConversationNode; value: any; status: "completed" | "editing" }>
  activeIndex: number       // currently active node
  isAnimating: boolean      // typewriter in progress, blocks input
}
```

The `transcript` uses a plain `Record` (not `Map`) so condition functions can access values directly via `values[nodeId]`. A derived `TranscriptValues` (flat `Record<string, primitive>`) is passed to condition functions.

### Behavior

1. **Render loop** — iterates nodes `0..activeIndex`. Completed nodes render as solid text. Active node renders with a live input replacing `{input}`.

2. **On Enter:**
   - Validate input
   - Call save API immediately (IPC command mapped from `saveKey`)
   - Mark node as `completed` with brief highlight animation
   - Advance `activeIndex`
   - Typewriter-animate the next prompt
   - Place cursor in new `{input}`

3. **On click completed answer:**
   - Set that node's status to `"editing"`
   - Swap solid text back to input
   - Nodes below grey out (`opacity-40`) but stay visible
   - On re-Enter, save again and restore

4. **Condition evaluation** — when advancing, derive `TranscriptValues` from transcript and call `node.condition(values)`. If false, skip silently to next node.

5. **Finance panel** — when `finance_gate` is confirmed yes, a slide-down panel opens containing the finance sub-forms (all 7: basics, accounts, budgeting, FIRE, investments, liabilities, goals). `FinancePanel` is a NEW component that reimplements the sub-step navigation from `FinanceStep.tsx` (the `SUB_STEPS` array + sub-step state + `subSaveMap` pattern) but takes props instead of `useOutletContext`. The existing finance sub-form components (`FinanceBasicsForm`, `AccountsForm`, etc.) use `registerSave` and `onDirty` props which are context-free and can be reused directly. The panel exposes an `onComplete` callback — called when the user finishes the last sub-step or clicks a "Done" button. `ConversationRunner` listens for `onComplete` and advances `activeIndex` to the next node. The panel also has a "Skip" button that triggers `onComplete` without saving.

6. **Error handling** — if save API fails, input stays active with red inline message below the sentence (e.g. `"Couldn't validate that API key. Try again."`). No modals or retry loops.

## Visual Design

### Layout
- Full-screen centered container, `max-w-[640px]`, vertically centered initially, scrolls as conversation grows
- Clean `bg-surface-base` background — no cards, just text flowing down
- Thin progress line at top (`bg-accent`) fills as nodes complete

### Typography
- Prompt text: standard body size, `text-foreground`
- `{input}` blank: underlined, `text-accent`, slightly bolder weight
- Completed values: same weight as prompt, `text-foreground`, no underline — becomes part of sentence

### Animations (~200-300ms, all subtle)
- **Typewriter:** next prompt types out ~30ms/char. Input disabled during animation. Click or keypress anywhere skips to instant completion.
- **Lock-in:** on Enter, underline fades out, value briefly flashes `bg-accent/10`, settles to plain text.
- **Edit mode:** text cross-fades back to underlined input. Nodes below transition to `opacity-40`.
- **Finance panel:** slides down from gate question with `ease-out`.

### Input Types

**Text:** underlined blank inline in sentence. Cursor auto-focused.

**Select (dropdown):** appears as underlined text showing default value. On click/focus, minimal dropdown opens via portal below the text. Selection replaces text, same lock-in animation on Enter.

**Masked (API key):** displays as `••••••••` with small eye icon at end. Paste supported. Validates on Enter.

**Confirm (yes/no):** inline dropdown with Yes/No options, default pre-selected.

**Tags (areas):** underlined area expands into a tag input. User types a name, presses comma or Enter to create a pill/chip. Each pill is deletable with an ×. Colors auto-assigned from `AREA_COLORS` palette in order. Press Enter on an empty input to confirm and lock in. Display format when completed: pills inline in the sentence (e.g. `"Your main areas of focus are [Work] [Personal] [Health]."`).

**Complex (finance panel):** no inline input. Instead, the node triggers a slide-down panel below the conversation containing the full finance sub-forms. The panel has its own internal navigation (mini progress pills) and a "Done" / "Skip" button. On completion, the panel collapses and the conversation continues.

## API Integration

### Save-on-Enter Mapping

| Node | IPC Command | Payload |
|---|---|---|
| `user_name` | `config_update_section` | `{ section: "user", patch: { name: "Vu" } }` |
| `provider` | `config_update_section` | `{ section: "agents", patch: { defaults: { provider: "anthropic" } } }` |
| `api_key` | `config_update_section` | `{ section: "providers", patch: { [provider]: { apiKey: "sk-..." } } }` — provider name is read from the `provider` transcript value |
| `areas` | `area_create` (per entry) | `{ name: "Work", color: "#3b82f6" }` — called once per tag. Colors assigned from `AREA_COLORS` palette |
| `productivity_gate` | `config_update_section` | `{ section: "productivity", patch: { enabled: true/false } }` — Rust-side defaults already provide sensible values (45min focus, 8h daily). Only the `enabled` flag is patched; no need to send defaults explicitly. |
| `finance_gate` | *(no save — controls flow only)* | — |
| `finance_setup` | existing finance sub-form save handlers | Each sub-form calls its own IPC (unchanged from current `FinanceStep`) |
| `complete` | `config_mark_setup_completed` | — |

### Resume Support

On mount, the runner loads existing values per node:

| Node | Resume loader | Check |
|---|---|---|
| `user_name` | `config_get_section("user")` | `name` is non-empty |
| `provider` | `config_get_section("agents")` | `defaults.provider` is set |
| `api_key` | `config_get_section("providers")` | active provider has non-empty `apiKey` |
| `areas` | `area_list` IPC | returns non-empty array |
| `productivity_gate` | skip resume | Always shown — `ProductivityConfig` has Rust-side defaults (`enabled: true`), so checking the config would always appear pre-completed. This node is cheap to re-answer. |
| `finance_gate` | skip resume | Always shown — user re-decides each time. Known limitation: if finance was partially configured in a prior run, the user is re-asked. |

Nodes with existing values start as `completed` in the transcript with their loaded value. The runner advances to the first unfilled node. Users who close mid-setup pick up where they left off.

### Final Step

The `complete` node has no input — just a typewriter message and a "Launch Klynt" button that calls `config_mark_setup_completed` and navigates to `/` (which triggers `DashboardRedirect` → `app_info()` check → main app).

## Files to Create/Modify

### New files (Rust backend)
- `crates/config/src/schema/user.rs` — `UserConfig` struct with `name: String`

### New files (Frontend)
- `desktop-ui/src/features/setup/schema.ts` — `ConversationNode` type + full schema array
- `desktop-ui/src/features/setup/components/ConversationRunner.tsx` — main engine component
- `desktop-ui/src/features/setup/components/InlineInput.tsx` — text input rendered inside sentence
- `desktop-ui/src/features/setup/components/InlineSelect.tsx` — dropdown rendered inside sentence (portal for dropdown overlay)
- `desktop-ui/src/features/setup/components/InlineMasked.tsx` — masked API key input with eye toggle
- `desktop-ui/src/features/setup/components/InlineTags.tsx` — tag/pill input for areas
- `desktop-ui/src/features/setup/components/TypewriterText.tsx` — typewriter animation wrapper
- `desktop-ui/src/features/setup/components/FinancePanel.tsx` — slide-down panel wrapping existing finance sub-forms, exposes `onComplete` callback
- `desktop-ui/src/features/setup/hooks/useConversationRunner.ts` — state management hook
- `desktop-ui/src/features/setup/hooks/useSaveNode.ts` — API save logic per node (handles both `config_update_section` and custom save functions)
- `desktop-ui/src/features/setup/hooks/useTypewriter.ts` — typewriter animation hook
- `desktop-ui/src/features/setup/hooks/useResumeSetup.ts` — loads existing values on mount to resume mid-setup

### Modified files (Rust backend)
- `crates/config/src/schema/core.rs` — add `pub user: UserConfig` field with `#[serde(default)]`
- `crates/config/src/schema/mod.rs` — add `mod user; pub use user::*;`

### Modified files (Frontend)
- `desktop-ui/src/features/setup/pages/` — remove all step page files (WelcomeStep, ProviderStep, ChannelsStep, AreasStep, ProductivityStep, McpStep, CompleteStep, FinanceStep). `FinancePanel` replaces `FinanceStep` — the sub-step navigation is reimplemented in the new component using props instead of `useOutletContext`
- `desktop-ui/src/features/setup/components/SetupLayout.tsx` — simplify to just render ConversationRunner (remove progress bar, back/next buttons)
- `desktop-ui/src/features/setup/hooks/steps.ts` — remove old step definitions
- `desktop-ui/src/features/setup/index.ts` — update exports (remove deleted page exports, add new component exports)
- `desktop-ui/src/app/router.tsx` — simplify setup routes to single route rendering ConversationRunner

### Kept as-is
- `desktop-ui/src/features/setup/components/finance/` — all 7 finance sub-form components (`FinanceBasicsForm`, `AccountsForm`, `IncomeForm`, `FireForm`, `InvestmentsForm`, `LiabilitiesForm`, `GoalsForm`) reused inside `FinancePanel` — they take `registerSave` and `onDirty` props which are context-free
- Backend IPC commands (`config_update_section`, `config_get_section`, `area_create`, `area_list`, `config_mark_setup_completed`) — no changes needed
