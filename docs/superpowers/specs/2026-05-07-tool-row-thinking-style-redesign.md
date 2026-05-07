# Tool-Row Thinking-Style Redesign

**Status:** Draft
**Date:** 2026-05-07
**Owner:** desktop-ui / messages
**Affects:** `desktop-ui/src/features/messages/components/MessageRows.tsx`, `desktop-ui/src/styles/messages.css`, `desktop-ui/src/styles/ds-tokens.css`

## Summary

Replace the current "tool card" rendering of assistant tool calls with a thinking-block-style row: a flat neutral bar with a 2px family-coloured left border, a status-only leading icon slot, an `Action: argument` label, contextual meta on the right, and click-to-expand for the full result. Adds a live stdout tail for long-running shell commands and auto-expansion of failed calls.

The rebuild is a presentation-layer change. The wire model (`ConversationItem.kind: "tool"`, `outputDelta` streaming, `expandedItems` state) is already correct and stays untouched.

## Goals

1. Tool calls feel like a *first-class secondary stream* alongside thinking — same visual weight, same left-border anchor.
2. Each row tells the user *what kind of work* (family colour) and *what's happening* (status icon + meta) at a glance.
3. Errors and prompts (`AskUser`) cannot be missed.
4. Long-running shell commands surface progress without a click.
5. Implementation reuses existing data flow: `ToolRow` component, `expandedItems` set, `outputDelta` reducer, `DiffView`, `CommandOutput`.

## Non-goals

- No change to the wire protocol or `ConversationItem` shape.
- No change to the Rust agent runtime, tool registry, or event emission.
- No new event types beyond what `appServerEvents.ts` already handles.
- No keyboard-shortcut layer (defer; current click target is sufficient).

## Visual model

### Row anatomy

```
| <leading-icon>  Name: argument            meta · meta  ▸
└─ 2px family border    flat bar bg (--surface-raised)   chevron
```

- **Bar background:** `var(--surface-raised)` (already defined). Same colour for every row regardless of family.
- **Left border:** `2px solid var(--tool-family-{n})`. Family colour from new tokens (see Tokens).
- **Bar shape:** `border-radius: 0 6px 6px 0` (square left edge so the border looks continuous, rounded right).
- **Padding:** `8px 12px 8px 14px`. The 14px left padding matches `.reasoning-inline`.
- **Margin between rows:** `4px` vertical. Burst-grouped sub-rows: `2px`.

### Leading icon slot (P1 — status-only)

A fixed-width 14px slot, always reserved (no layout shift across state transitions).

| State | Slot content |
|---|---|
| `pending` / `idle` | empty |
| `running` | spinner (1.5px border, 0.7s linear, family-coloured) |
| `succeeded` (default) | empty (collapsed rows show no leading icon) |
| `failed` | `✕` glyph in `--status-error` |
| `awaiting-user` (AskUser) | empty (border pulses instead) |

### Family colour palette (new tokens)

Added to `ds-tokens.css`:

| Family | Token | Dark theme value | Used by `toolType` |
|---|---|---|---|
| Filesystem | `--tool-family-filesystem` | `#60a5fa` (blue-400) | `fileChange` (read/write/edit/apply_patch/notebook_edit) |
| Shell | `--tool-family-shell` | `#f59e0b` (amber-500) | `commandExecution` |
| Search | `--tool-family-search` | `#34d399` (emerald-400) | local code search — `commandExecution` whose tool is `grep` / `glob` / `rg` |
| Web | `--tool-family-web` | `#c084fc` (violet-400) | `webSearch`, `webFetch`, `mcpToolCall` to web tools |
| Domain | `--tool-family-domain` | `#a78bfa` (purple-400) | `mcpToolCall` to klyntbot domain tools (tasks, notes, finance, memory, learning, coaching, etc.) and `recall_*` |
| Agent | `--tool-family-agent` | `#f472b6` (pink-400) | `collabToolCall`, `collabAgentToolCall`, agent/spawn |
| MCP external | `--tool-family-mcp` | `#2dd4bf` (teal-400) | `mcpToolCall` to non-klyntbot servers (github, linear, context7, etc.) |
| System | `--tool-family-system` | `#64748b` (slate-500) | `hook`, `contextCompaction` |
| Approval | `--tool-family-approval` | `#fbbf24` (amber-300) | `ask_user` (special — pulses) |
| Error | `--status-error` (existing) | `#ef4444` | overrides family colour when `status === "failed"` |

Light theme variants follow the same hue with bumped saturation; specified in `themes.light.css`.

### Header label format

`{Name}: {arg} {meta}` — segments separated by visual spacing, not punctuation glyphs.

- **Name** (`.tool-row__name`) — `font-weight: 600`, family-tinted text colour (lightest family shade — e.g., `#bfdbfe` for filesystem).
- **Arg** (`.tool-row__arg`) — monospace, `var(--text-muted)`. Truncated with ellipsis when longer than the available space; full value in tooltip.
- **Meta** (`.tool-row__meta`) — small (11px), `var(--text-faint)`, right-aligned via `margin-left: auto` on a wrapper. Multiple meta fragments separated by `· ` (mid-dot).

## Per-toolType mapping

Single source of truth lives in a new helper `toolRowDescriptor(item)` next to `toolNameFromTitle()` in `messageRenderUtils.ts`. Returns `{ family, name, arg, meta[] }`.

| `toolType` (+ tool name where relevant) | family | name | arg | meta |
|---|---|---|---|---|
| `commandExecution` (default = bash) | shell | `Bash` | command | elapsed (running) → final duration (done); exit code on failure |
| `commandExecution` w/ tool ∈ {grep, glob, rg} | search | `Grep` / `Glob` | pattern | `{N} matches · {F} files` |
| `fileChange` op=read | filesystem | `Read` | path | `L{start}–L{end}` if ranged |
| `fileChange` op=write | filesystem | `Write` | path | `+{lines}` |
| `fileChange` op=edit / apply_patch / notebook_edit | filesystem | `Edit` / `Patch` | path | `+{add} −{rm}` |
| `webSearch` | web | `WebSearch` | query | `{N} results · {duration}` |
| `mcpToolCall` (klyntbot server) | domain | klyntbot tool name (e.g., `Tasks`, `Notes`, `Memory`) | action verb | primary param (truncated) |
| `mcpToolCall` (any other server) | mcp | server name (`github`, `linear`, …) | tool action | key params + result count |
| `collabToolCall` / `collabAgentToolCall` | agent | `Agent` | subagent_type | description (truncated) + elapsed |
| `imageView` | system | `Image` | path | dimensions |
| `contextCompaction` | system | `Context` | `compacted` | tokens before → after |
| `hook` | system | `Hook` | event type | decision + matched rule |
| `ask_user` (synthesised) | approval | `AskUser` | the question | `awaiting reply…` (pulsing) → user's answer |

`recall_*` and other coding-memory tools fall under `domain` (purple). Burst grouping (below) applies before colour resolution — a burst of three `Read`s renders as one filesystem row regardless of paths.

## Interaction states & rules

| # | State | Default | Border | Leading icon | Body | Notes |
|---|---|---|---|---|---|---|
| 1 | done · success | collapsed | family | empty | hidden | click to toggle |
| 2 | running | collapsed | family | spinner | hidden (or tail, see #3) | chevron hidden during exec |
| 3 | bash · long-running | collapsed | shell | spinner | 3-line tail strip | gated on `elapsed ≥ 1.2s` (existing `isLongRunning`) |
| 4 | expanded | user-toggled or auto (#6) | family | per state | full body, max-height 360px | body inherits family border on its left edge |
| 5 | done · failed | **auto-expanded** | error red | `✕` | full body | first render; user can collapse manually |
| 6 | ask_user | persistent open | pulsing approval | empty | the question itself | no chevron until answered |
| 7 | burst (≥3 same family + same name) | collapsed | family | empty | per-call sub-rows on expand | header arg = `N items`; meta = first 3 paths + `+M more` |

### Auto-expand rules

- `status === "failed"` → set `expandedItems` on first mount of that item id.
- `status === "awaiting-user"` → render in persistent-open mode (does not consult `expandedItems`).
- All other rows: respect `expandedItems` (defaults to false; user-driven toggle is sticky per `manuallyToggledExpandedRef`).

### Bash tail rule

- Already-streamed content lives in `item.output` (reduced from `outputDelta` events at `threadItemsSlice.ts:314`).
- Tail mode renders `item.output.split("\n").slice(-3).join("\n")` in a strip below the row.
- Strip auto-scrolls if the model is rapidly emitting (`scrollIntoView({ behavior: "instant", block: "end" })`).
- On completion: tail strip is removed unless the user expanded the row. Full output remains accessible via expand.

### Burst grouping rule

- During render, group consecutive `kind: "tool"` items where `family` and `name` match into a single virtual `BurstRow` if `count ≥ 3`.
- Header: `{Name}: {count} {plural}` (e.g., `Read: 5 files`). Meta: first 3 args, then `+{count - 3} more`.
- Expansion shows each underlying row as a normal sub-row, indented 12px, no own bar bg (transparent), inherits family border.
- A failed call inside a burst breaks the group: the failed call renders as its own row (auto-expanded), and grouping resumes after.

## Component changes

### `MessageRows.tsx`

- **`ToolRow`** is rewritten end-to-end. The new component file is split: `ToolRow.tsx` (header + state machinery), `ToolRowBody.tsx` (expanded content dispatcher), `BashTail.tsx` (live 3-line strip), `BurstRow.tsx` (grouping wrapper).
- The CSS classnames change from `.tool-inline*` to `.tool-row*` to avoid mixing old + new during the changeover.
- `ExploreRow` (which uses sibling card styling) is refactored to share the same `tool-row` shell so explore entries match.
- `Messages.tsx` switch case for `kind: "tool"` is updated; the burst grouper runs before dispatch.

### `messageRenderUtils.ts`

- New export `toolRowDescriptor(item: ConversationItem): { family, name, arg, meta }`.
- `toolNameFromTitle` becomes a private helper used by the new descriptor.
- New helper `groupBursts(items: ConversationItem[]): (ConversationItem | BurstGroup)[]` — pure, unit-tested.

### `useMessagesViewState.ts`

- New side-effect: when an item transitions to `status === "failed"`, add its id to `expandedItems` once (idempotent, gated on `manuallyToggledExpandedRef` to respect a user collapse).
- No change to the toggle mechanics or persistence.

### CSS — `messages.css`

- Old `.tool-inline*` block (lines ~532–900) deleted.
- New `.tool-row` block authored. Sections: base, family modifiers, leading-icon, bar bg states, body, tail, burst, askuser pulse.
- Reasoning block stays untouched.

### Tokens — `ds-tokens.css`

- New variables: `--tool-family-filesystem`, `--tool-family-shell`, `--tool-family-search`, `--tool-family-web`, `--tool-family-domain`, `--tool-family-agent`, `--tool-family-mcp`, `--tool-family-system`, `--tool-family-approval`.
- Each gets a dark value at the root and an override in `themes.light.css` / `themes.dim.css`.

## Testing

- **Unit (Vitest):**
  - `toolRowDescriptor` — table-driven over every `toolType` × tool-name combo in the catalog. Snapshot the `{ family, name, arg, meta }` output.
  - `groupBursts` — sequences with: (a) 3 consecutive matches, (b) 2 matches (no group), (c) 4 matches with a failure in the middle (group breaks), (d) different families adjacent.
  - Auto-expand-on-error effect: simulate a `status: "failed"` transition and assert `expandedItems` contains the id.
- **Component (Vitest + RTL):**
  - `ToolRow` renders correct family border colour for each toolType.
  - Spinner mounts on `running`, unmounts on `succeeded`.
  - Bash tail appears at `elapsed ≥ 1200ms` and disappears on completion (unless expanded).
  - Failed row renders body without a click.
  - AskUser does not toggle on click.
- **Visual (manual):** start dev server, generate one of each toolType in the local agent, eyeball collapse/expand transitions, error rendering, and bash tail.

## Migration

Pre-release codebase — no users to migrate. The CSS rename (`.tool-inline*` → `.tool-row*`) is a clean cut; no compatibility shim. Tests are updated in the same commit.

## Out of scope

- Burst grouping across non-adjacent calls.
- Family-coloured spinner per row (chosen single neutral spinner colour `var(--accent-primary)` for v1).
- Keyboard shortcuts to expand/collapse all.
- Persistence of expand state across thread reopen — current behaviour (component-local Set) is preserved.
- Image-bearing tool results (no schema today; design parked in computer-use spec).

## Open questions

None as of this draft. All major decisions locked during the brainstorming session 2026-05-07:
- Color grouping = Option A by domain family (six core families + system + approval)
- Bar treatment = V1 flat neutral bg with 2px family border
- Leading icon = P1 status-only
- Auto-expand on error = yes
- Burst grouping = ship in v1
- Bash tail threshold = 1.2s (reuse existing `isLongRunning`)
