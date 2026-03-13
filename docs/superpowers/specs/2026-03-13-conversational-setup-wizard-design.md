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

## Architecture: Declarative Conversation Schema

### ConversationNode Type

```ts
type InputType = "text" | "select" | "masked" | "confirm"

type ConversationNode = {
  id: string                          // unique key, e.g. "user_name"
  prompt: string                      // sentence template with {input} placeholder
  inputType: InputType
  default?: string | boolean
  options?: { label: string; value: string }[]
  validate?: (value: string) => string | null
  saveKey: string                     // maps to IPC command / config field
  condition?: (transcript: Record<string, any>) => boolean
}
```

### Schema Definition

| id | prompt | inputType | condition |
|---|---|---|---|
| `user_name` | `"Hello, I'm Klynt. Your name is {input}."` | text | — |
| `provider` | `"I'll be powered by {input}."` | select | — |
| `api_key` | `"My API key is {input}."` | masked | — |
| `areas` | `"Your main areas of focus are {input}."` | text | — |
| `productivity` | `"Your productivity style is {input}."` | select | — |
| `finance_gate` | `"Would you like to set up finance tracking? {input}"` | confirm | — |
| `finance_setup` | *(opens finance panel)* | complex | `finance_gate === true` |
| `complete` | `"Great, we're all set. Let's get started!"` | — | — |

Adding or removing a step = adding or removing one object in the array.

## ConversationRunner Engine

Single React component that interprets the schema and manages the flow.

### State

```ts
{
  transcript: Map<string, { node: ConversationNode; value: any; status: "completed" | "editing" }>
  activeIndex: number       // currently active node
  isAnimating: boolean      // typewriter in progress, blocks input
}
```

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

4. **Condition evaluation** — when advancing, check `node.condition(transcript)`. If false, skip silently to next node.

5. **Finance gate** — when confirmed yes, a slide-down panel opens with existing finance sub-forms. On close/complete, conversation resumes.

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

## API Integration

### Save-on-Enter Mapping

| Node | IPC Command | Payload |
|---|---|---|
| `user_name` | `update_config` | `{ user: { name: "Vu" } }` |
| `provider` | `update_config` | `{ agents: { defaults: { provider: "anthropic" } } }` |
| `api_key` | `update_config` | `{ providers: { anthropic: { apiKey: "sk-..." } } }` |
| `areas` | `save_areas` | area data |
| `productivity` | `update_config` | productivity settings |
| `finance_gate` | *(no save — controls flow only)* | — |
| `finance_setup` | existing finance save handlers | — |
| `complete` | `config_mark_setup_completed` | — |

### Resume Support

On mount, the runner checks config for already-saved values. Nodes with existing values start as `completed`. The runner advances to the first unfilled node. Users who close mid-setup pick up where they left off.

### Final Step

The `complete` node has no input — just a typewriter message and a "Launch Klynt" button that calls `config_mark_setup_completed` and navigates to the main app.

## Files to Create/Modify

### New files
- `desktop-ui/src/features/setup/schema.ts` — conversation node type + full schema array
- `desktop-ui/src/features/setup/components/ConversationRunner.tsx` — main engine component
- `desktop-ui/src/features/setup/components/InlineInput.tsx` — text input rendered inside sentence
- `desktop-ui/src/features/setup/components/InlineSelect.tsx` — dropdown rendered inside sentence
- `desktop-ui/src/features/setup/components/InlineMasked.tsx` — masked API key input
- `desktop-ui/src/features/setup/components/TypewriterText.tsx` — typewriter animation wrapper
- `desktop-ui/src/features/setup/components/FinancePanel.tsx` — slide-down panel wrapping existing finance forms
- `desktop-ui/src/features/setup/hooks/useConversationRunner.ts` — state management hook
- `desktop-ui/src/features/setup/hooks/useSaveNode.ts` — API save logic per node
- `desktop-ui/src/features/setup/hooks/useTypewriter.ts` — typewriter animation hook

### Modified files
- `desktop-ui/src/features/setup/pages/` — remove individual step pages (WelcomeStep, ProviderStep, etc.)
- `desktop-ui/src/features/setup/components/SetupLayout.tsx` — simplify to just render ConversationRunner (remove progress bar, back/next buttons)
- `desktop-ui/src/features/setup/hooks/steps.ts` — remove old step definitions
- `desktop-ui/src/app/router.tsx` — simplify setup routes to single route rendering ConversationRunner

### Kept as-is
- `desktop-ui/src/features/setup/components/finance/` — all finance sub-forms reused inside FinancePanel
- Backend IPC commands — no changes needed
- `config_mark_setup_completed` — called on final step as before
