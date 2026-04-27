# Klyntbot chat surface integration — Design

**Status:** approved (brainstorming)
**Date:** 2026-04-27
**Owner:** desktop-ui / chat integration
**Successor to:** [`2026-04-26-klyntbot-chat-mvp-design.md`](./2026-04-26-klyntbot-chat-mvp-design.md)

## Goal

Render klyntbot chat sessions through the rich Codex-style `Messages` + `Composer` surface that already exists in `desktop-ui/`, so klyntbot conversations match the target Codex screenshot (split-pane layout, header pills, reasoning rows, collapsed tool-call groups, full composer with model / mode / access pills).

The rich UI is already implemented. The MVP spec from 2026-04-26 wedged klyntbot's data flow into a deliberately minimal stub (`ChatPanel`/`ChatInput`/`MessageBubble`) to ship quickly. This spec replaces that stub with the real rich UI.

## Non-goals (this iteration)

Per the user mandate "keep everything; missing can be added later":

- **No backend changes.** Klyntbot's chat backend (`chat_send`, `chat_messages`, the `agent:*` Tauri events, `chatStreamStore`) is untouched.
- **No rich-UI component changes.** `Messages.tsx`, `MessageRows.tsx`, `Composer.tsx`, and the `ConversationItem` union are not refactored or extended.
- **No deletion of the non-`chatActive` (Codex) flow.** It keeps rendering exactly as today.
- **No new klyntbot capabilities.** The following are deferred to follow-up work:
  - Image attachments end-to-end (UI + send payload)
  - Interactive model / mode / access pills (today they're static read-outs)
  - Voice dictation
  - Git review prompts
  - Composer message queue
  - Rendering of klyntbot-specific signals (`transparency.*`, `personaMessages`, `debateRounds`, `judgeDecisions`, `activeDelegateAgent`, `statusPhase`) — backend keeps emitting; UI surfaces them later when `ConversationItem` is extended.
  - Reasoning / `diff` / `review` / `explore` rows: klyntbot doesn't emit source data; rows simply never render.

## Background

### What exists today

- **Rich UI** (target):
  - `desktop-ui/src/features/messages/components/Messages.tsx` — consumes `ConversationItem[]` and renders the full Codex experience.
  - `desktop-ui/src/features/messages/components/MessageRows.tsx` — row components per `ConversationItem` variant: `message`, `userInput`, `reasoning`, `diff`, `review`, `explore`, `tool`.
  - `desktop-ui/src/features/composer/components/Composer.tsx` — full-featured composer with attachments, model picker, reasoning effort, collaboration mode, access mode, dictation, queue, review prompts, etc.
  - `ConversationItem` union: `desktop-ui/src/types.ts:100-142`.

- **Klyntbot stub** (to be replaced):
  - `desktop-ui/src/features/chat/components/ChatPanel.tsx` — minimal session renderer, ~80 lines.
  - `desktop-ui/src/features/chat/components/ChatInput.tsx` — minimal textarea.
  - `desktop-ui/src/features/chat/components/MessageBubble.tsx` — plain text bubbles.
  - `desktop-ui/src/features/chat/components/ChatPanel.test.tsx` — colocated test.

- **Klyntbot data flow** (reused as-is):
  - `desktop-ui/src/features/chat/hooks/useChatSession.ts` — session state + `send()` + `chat_messages` fetch + `chat:message_added` listener.
  - `desktop-ui/src/features/chat/hooks/useAgentStream.ts` — Tauri event subscriptions (`agent:content_chunk`, `agent:tool_start`, `agent:tool_end`, `agent:done`, `agent:error`, `agent:interaction_request`, plus 14 other transparency / persona / debate events).
  - `desktop-ui/src/features/chat/store/chatStreamStore.ts` — central store, exposes `StreamSnapshot { segments, isStreaming, activeTools, error, activeInteraction, transparency, personaMessages, debateRounds, ... }`.
  - `desktop-ui/src/features/chat/hooks/useChatThreads.ts` — sidebar threads list + refetch.

- **Layout switch** (`desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx:28-53`):
  ```tsx
  const chatActive = chatViewProps.active && chatViewProps.sessionKey !== null;
  const messagesNode = chatActive ? <ChatPanel ... /> : <Messages {...messagesProps} />;
  const composerNode = chatActive ? null : <Composer {...composerProps} />;
  ```
  When `chatActive`, klyntbot renders its stub and the composer slot is empty. When not, the rich Codex UI renders.

### Klyntbot stream segment shape

```ts
// desktop-ui/src/features/chat/types.ts
export type MessageSegment =
  | { type: "text"; content: string }
  | {
      type: "tool";
      name: string;
      action?: string;
      success: boolean;
      durationMs: number;
      result?: string;
      estimatedTokens?: number;
      agent?: string;
    };
```

That's it for the main stream. Everything else (transparency, persona, debate) lives on parallel `StreamSnapshot` fields.

## Stack constraints (from CLAUDE.md)

- Plain CSS in `src/styles/`. No Tailwind. BEM-ish class names.
- Typography uses `--fs-*` tokens (`src/styles/ds-tokens.css`); never hardcode `font-size: Npx`.
- Path aliases only: `@/*`, `@app/*`, `@settings/*`, `@threads/*`, `@services/*`, `@utils/*`. No `../../`.
- Tauri IPC via `invoke()` from `@/api/client`. No `useQuery` / `useMutation` wrappers.
- Linting: ESLint via `bun run lint`. Tests: Vitest via `bun run test`.

## Architecture

### Approach: prop-level switch (Approach 1.5)

Apply the `chatActive` decision at the **prop source**, not at the **component**. The rich `Messages` and `Composer` always render; their props come from one of two sources depending on whether a klyntbot session is active.

- The component-level switch in `buildPrimaryNodes.tsx` is removed. It always renders `<Messages {...messagesProps} />` and `<Composer {...composerProps} />`.
- A new adapter hook `useKlyntbotSurfaceProps(sessionKey)` produces `{ messagesProps, composerProps }` from `useChatSession()` + `useAgentStream()` + a thin model lookup.
- The site that assembles `LayoutNodesOptions.primary` (currently `useMainAppLayoutSurfaces.ts` and/or its wrapper in `useMainAppShellProps.tsx` — exact insertion point pinned during writing-plans) chooses between the existing Codex-derived props and the klyntbot adapter's output, keyed on `chatActive`.
- `ChatPanel.tsx`, `ChatInput.tsx`, `MessageBubble.tsx`, `ChatPanel.test.tsx` are deleted — they're stubs the rich UI fully replaces.

This is the lowest-blast-radius option that still gives klyntbot the screenshot's split-pane composer-at-bottom look (which the previous "ChatPanel hosts everything" approach can't deliver because composer stays in its own layout slot).

### Data flow

```
                   ┌──────────────────────────┐
                   │  chatViewProps.active    │
                   │  + .sessionKey           │
                   └────────────┬─────────────┘
                                │
                                ▼
              ┌─────────────────────────────────────┐
              │  layoutSurfaces.primary assembly    │
              │  (useMainAppLayoutSurfaces)         │
              │                                     │
              │   if chatActive:                    │
              │     useKlyntbotSurfaceProps(key)    │
              │       → { messagesProps,            │
              │           composerProps }           │
              │   else:                             │
              │     existing Codex assembly         │
              └────────────┬────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────────┐
              │  buildPrimaryNodes          │
              │  (no chatActive branch)     │
              │                             │
              │  <Messages  {...} />        │
              │  <Composer {...} />         │
              └─────────────────────────────┘
```

### File layout

```
desktop-ui/src/features/chat/                ← existing
├── store/chatStreamStore.ts                 (unchanged)
├── types.ts                                 (unchanged)
├── hooks/
│   ├── useAgentStream.ts                    (unchanged)
│   ├── useChatSession.ts                    (unchanged)
│   ├── useChatThreads.ts                    (unchanged)
│   ├── useKlyntbotSurfaceProps.ts           ← NEW
│   └── useKlyntbotSurfaceProps.test.ts      ← NEW
└── components/
    ├── ChatPanel.tsx                        ← DELETE
    ├── ChatPanel.test.tsx                   ← DELETE
    ├── ChatInput.tsx                        ← DELETE
    └── MessageBubble.tsx                    ← DELETE
```

```
desktop-ui/src/features/layout/hooks/layoutNodes/
└── buildPrimaryNodes.tsx                    ← MODIFY (drop chatActive branch)
```

```
desktop-ui/src/features/app/hooks/
└── useMainAppLayoutSurfaces.ts              ← MODIFY
                                                (consume klyntbot props
                                                 when chatActive)
                                                — exact entry point
                                                  may move to its caller
                                                  if cleaner; pinned in
                                                  writing-plans.
```

### `useKlyntbotSurfaceProps` adapter

**Signature** (proposed):
```ts
export function useKlyntbotSurfaceProps(
  sessionKey: string,
  onThreadsChanged: () => void,
): {
  messagesProps: MessagesProps;
  composerProps: ComposerProps;
};
```

Internally:
- Calls `useChatSession(sessionKey)` to get `messages`, `segments`, `isStreaming`, `activeInteraction`, `error`, `input`, `setInput`, `send`, `clearInteraction`.
- Maps `messages` + `segments` into `ConversationItem[]` (see Mapping section).
- Builds `messagesProps` and `composerProps` per the strategies below.
- On send completion (e.g., via `useEffect` watching `isStreaming` falling edge, or via `chat:message_added` Tauri event already subscribed elsewhere), invokes `onThreadsChanged` to refresh the sidebar.

**Verification** (resolved during writing-plans):
- Confirm that `useChatThreads` already auto-refetches on `chat:thread_created` / `chat:message_added`. If yes, `onThreadsChanged` may be redundant — the hook can drop the parameter.
- Confirm where `selectedSessionKey` (from `chatViewProps`) is available to the calling layout hook.

## Stream → ConversationItem mapping

| Klyntbot data | Maps to | Mapping detail |
|---|---|---|
| `messages: ChatMessage[]` (persisted via `chat_messages`) | `kind: "message"` | One row per message; `role` straight pass-through; `text` from `content`; `id` from message id. Pre-streaming history. |
| `segments[i] { type: "text", content }` | append into trailing assistant `kind: "message"` | Coalesce streaming chunks: if the last item is `kind: "message"` with role `"assistant"`, append `content` to its `text`; otherwise create a new assistant message item. |
| `segments[i] { type: "tool", name, action?, success, durationMs, result?, agent? }` | `kind: "tool"` | `toolType: name`, `title: name`, `detail: action ?? ""`, `output: result ?? undefined`, `durationMs`, `status: derived from success`. Exact status string ("completed" / "failed" / etc.) verified against `MessageRows.tsx` conventions during writing-plans. |
| `activeInteraction` (live, pending) | `messagesProps.userInputRequests[]` | Not a `ConversationItem` row. The rich UI renders pending interaction prompts via the dedicated `userInputRequests` prop; `onUserInputSubmit` invokes klyntbot's interaction-response path. The `kind: "userInput"` ConversationItem variant is reserved for *answered* interactions in history (its type fixes `status: "answered"`); klyntbot does not currently persist resolved interactions, so we do not emit those rows this iteration. |
| `error: string \| null` | error banner above messages | Not a `ConversationItem`. Surfaced via the rich UI's existing error affordance if available, otherwise as a small banner. Confirmed during writing-plans. |
| `transparency.*` | (deferred) | Backend continues emitting; not surfaced. |
| `personaMessages` / `debateRounds` / `judgeDecisions` | (deferred) | Same. |
| `activeDelegateAgent` / `statusPhase` | (deferred) | Same. |

The rich UI variants `reasoning`, `diff`, `review`, `explore` have no klyntbot source data and simply never render — no special handling required.

### Edge cases

- **Mid-stream tool then more text.** Tool segment closes the trailing assistant message; subsequent text segment opens a new assistant message after the tool row.
- **Empty assistant message at stream start.** When the first segment arrives, create the assistant message; show `isThinking: true` in `messagesProps` while `isStreaming` is true and no segments have arrived yet.
- **User message echo.** `useChatSession` already handles persisted user messages via `chat_messages` refetch on `chat:message_added`. The adapter doesn't synthesize user rows from `pendingUserMsg`; it lets the persistence layer drive that.

## Composer prop strategy

| Composer prop | Klyntbot value | Notes |
|---|---|---|
| `onSend(text, images, appMentions, submitIntent)` | `chat.send({ content: text })` | `images`, `appMentions`, `submitIntent` ignored this iteration. |
| `onStop` | no-op | No klyntbot stop API yet. |
| `canStop` | `false` | Hides the stop button. |
| `disabled` | `false` | Always enabled. |
| `appsEnabled` | `false` | No app mention support. |
| `isProcessing` | `chat.isStreaming` | Drives spinner / submit state. |
| `steerAvailable` | `false` | No mid-stream steering. |
| `followUpMessageBehavior` | sane default (TBD in writing-plans) | Probably the rich UI's existing default. |
| `composerFollowUpHintEnabled` | `false` | |
| `models` | `[{ id, displayName, model }]` (single item from `Config.agents.defaults.model`) | Pill renders, klyntbot's effective model shown. |
| `selectedModelId` | the single model's id | |
| `onSelectModel` | no-op | Pill is visually present but inert. |
| `reasoningOptions` | `[]` | |
| `selectedEffort` | `null` | |
| `onSelectEffort` | no-op | |
| `selectedServiceTier` | `null` | |
| `reasoningSupported` | `false` | Should hide the reasoning effort pill in the rich UI. |
| `accessMode` | `"current"` | |
| `onSelectAccessMode` | no-op | |
| `collaborationModes` | `[]` | If the rich UI doesn't auto-hide on empty, fall back to `[{ id: "default", label: "klyntbot" }]` with no-op selector — pinned in writing-plans after reading Composer's render code. |
| `selectedCollaborationModeId` | `null` (or `"default"`) | |
| `onSelectCollaborationMode` | no-op | |
| `skills` / `apps` / `prompts` / `files` | `[]` | Autocomplete shows nothing. |
| `attachedImages` | `[]` | |
| `onPickImages` / `onAttachImages` / `onRemoveImage` | no-ops | |
| `dictationEnabled` | `false` | |
| `queuedMessages` | `[]` | |
| `reviewPrompt` | `undefined` | |
| `contextUsage` | `undefined` | |
| `draftText` / `onDraftChange` | wired to `chat.input` / `chat.setInput` | Preserves existing draft semantics. |

**Composer render verification** (during writing-plans): some pills the rich UI shows unconditionally even when empty arrays are supplied. Reading `Composer.tsx` end-to-end will identify any pills that need a one-element fallback (collaborationMode in particular) versus those that auto-hide. All adjustments stay confined to `useKlyntbotSurfaceProps`; `Composer.tsx` is not edited.

## `messagesProps` non-row fields

| Prop | Source |
|---|---|
| `items` | mapping above |
| `threadId` | `sessionKey` |
| `workspaceId` | `null` (klyntbot has no workspace concept; pass-through if rich UI tolerates `null`) |
| `isThinking` | `chat.isStreaming && items` doesn't yet have a streaming assistant row |
| `isLoadingMessages` | initial fetch in `useChatSession` (true until first `chat_messages` resolves) |
| `processingStartedAt` | timestamp captured when `chat.isStreaming` flips to true |
| `lastDurationMs` | duration of last completed stream |
| `openTargets` | `[]` |
| `selectedOpenAppId` | `""` |
| `codeBlockCopyUseModifier` | from `appSettings` already in scope |
| `showMessageFilePath` | from `appSettings` |
| `userInputRequests` | `chat.activeInteraction ? [chat.activeInteraction] : []` (shape conversion if needed) |
| `onUserInputSubmit` | wired to klyntbot's interaction-response path |
| `onPlanAccept` / `onPlanSubmitChanges` / `onOpenThreadLink` / `onQuoteMessage` | no-ops or `undefined` where optional |

## Out of scope

Everything in the **Non-goals** list above. None of these are blocked by this design — each can land in a follow-up:

1. Image attachments on klyntbot send (backend + UI).
2. Interactive model picker (per-session model override).
3. Reasoning effort selector (klyntbot reasoning support).
4. Voice dictation in chat composer.
5. Git review prompts.
6. Message queue.
7. Rendering klyntbot-specific signals (`transparency`, `personaMessages`, `debateRounds`, etc.) — likely needs new `ConversationItem` variants.

## Testing

### Unit (Vitest)

`desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.ts`:

- Empty session → `messagesProps.items === []`, `composerProps.isProcessing === false`.
- Persisted history (mock `useChatSession.messages`) → one `kind: "message"` per persisted entry, in order, correct role.
- Mid-stream text segments → coalesce into a single trailing assistant `kind: "message"` whose `text` concatenates chunks.
- Tool segment between text segments → splits into `[message, tool, message]` rows; tool fields populated correctly.
- `activeInteraction` set → `messagesProps.userInputRequests` contains one shape-converted entry; cleared on submit.
- `error` set → confirms error surface (banner or row, per implementation).
- `composerProps.onSend("hi", [])` → calls `useChatSession.send({ content: "hi" })`.

Mocks: `useChatSession`, `useAgentStream`, klyntbot config lookup. No actual Tauri.

### Manual smoke

```bash
cd desktop-ui && bun run dev
# in another terminal:
cargo tauri dev
```

Verify in the running app:

1. Start a klyntbot chat session. Confirm split-pane layout (messages on top, composer at bottom) — same as the target Codex screenshot.
2. Send a message. Confirm:
   - Streaming text coalesces into one assistant message row (not many tiny rows).
   - Tool calls render as `tool` rows (collapsed).
   - The model pill shows klyntbot's configured model.
   - Send button works, disables during stream.
3. Confirm the **non-`chatActive` path** still renders as before (no regressions in the Codex flow). Switch out of chat (or open the app without a session) and verify the existing rich UI behavior is unchanged.
4. Trigger an error (e.g., disconnect provider). Confirm error surface renders.

### Static checks

```bash
cd desktop-ui && bun run lint && bun run typecheck && bun run test
```

Required to be clean before merge.

## Open questions / verification during writing-plans

1. **Exact insertion point for the prop swap.** `useMainAppLayoutSurfaces.ts` builds many of the props but it's also a complex hook with many inputs. Cleanest may be to override `messagesProps`/`composerProps` in `useMainAppShellProps.tsx` (the caller) after `useMainAppLayoutSurfaces` returns. Pin during planning.

2. **`onThreadsChanged` redundancy.** Verify whether `useChatThreads` already auto-refetches on backend events. If yes, drop the parameter from `useKlyntbotSurfaceProps`.

3. **Composer's empty-array handling.** Read `Composer.tsx` end-to-end to determine which pills auto-hide on empty arrays vs. need a one-item fallback. Decisions confined to the new hook.

4. **Error surface placement.** Verify the rich UI has an existing error display affordance, or whether we need a small banner above `<Messages>` (which would be a tiny new component, not a `Messages` edit).

5. **`processingStartedAt` capture.** Determine where `chat.isStreaming` flips from false→true so we can timestamp it. Likely in `useKlyntbotSurfaceProps` via a `useEffect`-tracked previous value.

6. **`userInputRequests` shape.** The rich UI's `RequestUserInputRequest` may differ from klyntbot's `activeInteraction`. Map fields or adapt — pinned in planning.

These are scoped, local decisions — none threaten the overall architecture.

## Risks

- **Rich UI assumes Codex-shaped props in places we haven't audited.** Mitigated by: unit-testing the adapter against the rich UI's prop types (Vitest will catch type mismatches), and the manual smoke test path.
- **Composer pills look "dead" to users.** Acceptable for v1 per the user mandate. A follow-up can wire them; this design intentionally takes the screenshot-fidelity path over the hide-everything path.
- **The non-`chatActive` (Codex) flow regresses.** Mitigated by: this design does not touch its prop assembly; it only adds a sibling branch.

## Acceptance

- Klyntbot chat sessions render through `<Messages>` + `<Composer>` with the screenshot's split-pane layout.
- Streaming text coalesces into one assistant message; tool calls render as `tool` rows.
- The non-`chatActive` flow renders exactly as before.
- All static checks pass.
