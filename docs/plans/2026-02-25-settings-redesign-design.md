# Settings Page Redesign

**Date**: 2026-02-25
**Status**: Approved

## Problem

The current settings page has poor information density and UX:
- One bordered card per field (~50 cards) wastes vertical space
- Raw text inputs for fields that should be selects (model, provider, timezone)
- 14 nav sections — some with just 1 field
- 260px right sidebar shows only static info ("Status: Ready")
- No visual hierarchy — every field looks equally important
- ~2025 lines in a single monolithic component

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Design direction | VS Code / Linear — dense inline rows | Power-user efficient, information-dense |
| Section grouping | 8 consolidated groups | Reduces nav decision fatigue |
| Provider/Model UX | Smart select with known model lists | Eliminates "what do I type?" confusion |
| Right sidebar | Remove entirely | Reclaim 260px; save indicator inline |
| Sub-section behavior | First expanded, rest collapsed | Clean first impression, discoverable |
| Row density | 44px default, ~60px for obscure fields with descriptions | Dense where possible, self-documenting where needed |

## Layout

```
[48px app nav rail]  [200px settings nav]  [flex-1 content area]
```

- No right sidebar. Save status is a subtle inline indicator near the page title.
- 8 nav items in the left settings nav.
- Content area: page title + sub-sections with chevron toggles.
- Sub-sections contain grouped inline rows (label left, control right).

## Inline Row Design

Two row variants:

1. **Simple row (44px)**: Label left-aligned, control right-aligned, 1px bottom border.
2. **Descriptive row (~60px)**: Same as above + muted description line under label. Used for obscure fields (RRF K, Decay Half-Life, etc.).

No individual cards per field. Rows grouped inside a sub-section container with a collapsible header.

## Control Types

| Control | Used for |
|---------|----------|
| Toggle | Booleans (enabled/disabled) |
| Select dropdown | Provider, model, timezone, trust level, creation mode, currencies |
| Inline number (`w-20`) | Max tokens, thresholds, intervals |
| Slider + value | Temperature, confidence threshold |
| Secret input | API keys (masked + eye toggle) |
| Tag input | Allow-from lists, skills (chips with X, + to add) |
| Compact text | API base URL, custom model names |

## Smart Model Select

Provider select filters the model dropdown. Known models hardcoded per provider:
- Anthropic: claude-opus-4-0520, claude-sonnet-4-20250514, claude-haiku-4-5-20251001
- OpenAI: gpt-4o, gpt-4o-mini, gpt-4-turbo, o1, o1-mini, o3-mini
- DeepSeek: deepseek-chat, deepseek-reasoner
- etc.

"Custom..." option at the bottom opens a text input for arbitrary model names.

## Section Map (8 groups)

### 1. General (4 fields)
- Timezone (select), Data Directory (read-only), Gateway Host (text), Gateway Port (number)

### 2. AI & Models (3 sub-sections)
- **Providers** (expanded): Tab strip per provider → API Key, API Base, Native Mode, Cache System Prompt, Extended Thinking + Budget, API Version
- **Agent Defaults** (collapsed): Provider (select), Model (smart select), Temperature (slider), Max Tokens, Max Tool Iterations, Max Concurrent Subagents
- **Routing** (collapsed): Primary Provider (select), Fallback (select), Classifier Model

### 3. Channels (tab strip)
- Per channel: Enabled, Token, Allow From (tag input), Proxy (conditional)

### 4. Tools (3 sub-sections)
- **Web Search** (expanded): Brave API Key, Max Results
- **Browser** (collapsed): Enabled, Trust Level (select), Session Timeout
- **Permissions** (collapsed): Restrict to Workspace, Default Level (select)

### 5. Tasks (5 sub-sections)
- **General** (expanded): Creation Mode (select), Projects enabled
- **Enrichment** (collapsed): Enabled, Auto Apply Threshold, Use LLM
- **Search** (collapsed): Semantic enabled, Threshold, Embedding Model, RRF K
- **Notifications** (collapsed): Targets (tag input), Focus Reminders, Digest, Digest Time
- **Focus & Planning** (collapsed): Max Slots, Deadline Hours, Planning enabled, Planning Time

### 6. AI Behavior (4 sub-sections)
- **Conversation** (expanded): Embedding enabled, Exclude Channels/Roles, Search enabled, Threshold, Max Results
- **Session** (collapsed): History Limit, TTL Days, Cleanup Interval
- **Memory** (collapsed): Decay Half-Life, Max Age, Consolidation, Maintenance Interval
- **Learning & Confidence** (collapsed): Learning enabled, Analysis Interval, Thresholds, Confidence enabled/threshold (slider), Tool Overrides

### 7. Finance (6 sub-sections)
- **General** (expanded): Enabled, Display Currency (select), Proactivity (select)
- **Budgeting** (collapsed): Default Method, Alert Threshold, Six Jar Ratios
- **Investment Returns** (collapsed): Stocks/Crypto/Real Estate/Bonds
- **Inflation** (collapsed): Annual Rate, Source
- **Auto-Categorization** (collapsed): Enabled, Confidence Threshold
- **Scheduling** (collapsed): Daily Review Time, Budget Check Time, Weekly Report Day

### 8. Extensions (3 sub-sections)
- **Packs** (expanded): Enabled packs (read-only badges)
- **Skills** (collapsed): Enabled skills (tag input)
- **Plugins** (collapsed): Enabled, Registry URL, Sandbox Memory, Allow Network

## Component Architecture

Extract from monolithic 2025-line component into:

- `SettingRow` — single inline row (label, optional description, control slot)
- `SettingSection` — collapsible sub-section with chevron header
- `SettingToggle` — row with toggle control
- `SettingSelect` — row with select dropdown
- `SettingNumber` — row with inline number input
- `SettingSlider` — row with range slider + value display
- `SettingSecret` — row with masked input + eye toggle
- `SettingTagInput` — row with chip list + add button
- `SettingText` — row with text input
- `ModelSelect` — smart provider-aware model dropdown
- `ProviderTabs` — tab strip for provider/channel selection
- `SaveIndicator` — inline "Saved" / "Saving..." fade indicator

All defined as module-level components (not inline in the page component).

## Migration

- All existing PATCH API calls and debounce logic preserved
- All fields map to the same config paths — no backend changes needed
- Settings.tsx rewritten from scratch using new components
- Existing `useApi`, `apiFetch`, `patchSection`, `debouncedPatch` patterns reused
