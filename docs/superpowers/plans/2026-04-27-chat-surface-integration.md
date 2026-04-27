# Klyntbot Chat Surface Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render klyntbot chat sessions through the rich Codex-style `<Messages>` + `<Composer>` UI in `desktop-ui/`, replacing the minimal `ChatPanel` stub.

**Architecture:** Build a `useKlyntbotSurfaceProps` adapter hook that turns `useChatSession()` output into klyntbot-specific overrides for `MessagesProps` / `ComposerProps`. Inject the overrides in `MainApp.tsx` between `useMainAppLayoutSurfaces` and `useMainAppLayoutNodes`. Drop the `chatActive` component-level branch from `buildPrimaryNodes`. Add a small `ChatErrorBanner` rendered alongside the messages slot. Delete the four stub files.

**Tech Stack:** React 19 + TypeScript, Vitest + `@testing-library/react`, plain CSS in `src/styles/`, path aliases `@/*` `@app/*` `@settings/*` `@threads/*` `@services/*` `@utils/*`, `bun` for package management.

**Spec:** [`docs/superpowers/specs/2026-04-27-chat-surface-integration-design.md`](../specs/2026-04-27-chat-surface-integration-design.md)

---

## Pre-flight (read once before Task 1)

The engineer should skim these files to load the contracts in their head. No code changes:

- `desktop-ui/src/types.ts:100-142` — `ConversationItem` union (the row types we'll emit).
- `desktop-ui/src/types.ts:393-413` — `RequestUserInputRequest`, `RequestUserInputParams`, `RequestUserInputQuestion`, `RequestUserInputResponse`, `RequestUserInputAnswer`.
- `desktop-ui/src/features/chat/types.ts:1-385` — full klyntbot chat types (`ChatMessage`, `MessageSegment`, `ActiveInteraction`, `InteractionRequest`, `Question`, `AnswerType`, `FormResponse`).
- `desktop-ui/src/features/chat/hooks/useChatSession.ts` — the `ChatSession` interface (lines 23-44) is the data we're adapting from.
- `desktop-ui/src/features/messages/components/Messages.tsx:26-50` — `MessagesProps`.
- `desktop-ui/src/features/composer/components/Composer.tsx:38-139` — `ComposerProps`.
- `desktop-ui/src/features/composer/components/ComposerMetaBar.tsx:60-170` — pill rendering conditions (key: line 70 `collaborationModes.length > 0 && ...` hides the pill if empty).
- `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx` — current `chatActive` branch.
- `desktop-ui/src/features/app/components/MainApp.tsx:1721-1745` — call site of `useMainAppLayoutSurfaces` and `useMainAppLayoutNodes` (the injection seam).

## File structure

| Action | Path | Responsibility |
|---|---|---|
| Create | `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts` | Adapter: `ChatSession` → `MessagesProps` / `ComposerProps` overrides + error state |
| Create | `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx` | Unit tests for the adapter (Vitest + `renderHook`) |
| Create | `desktop-ui/src/features/chat/components/ChatErrorBanner.tsx` | ~30-line dismissible error banner |
| Create | `desktop-ui/src/features/chat/components/ChatErrorBanner.test.tsx` | Banner unit tests |
| Create | `desktop-ui/src/styles/chat-error-banner.css` | Banner styling, BEM-ish; imported into `index.css` |
| Modify | `desktop-ui/src/styles/index.css` | Add `@import "./chat-error-banner.css";` |
| Modify | `desktop-ui/src/features/app/components/MainApp.tsx:~1729-1748` | Call adapter, merge overrides into `layoutSurfaces`, render banner |
| Modify | `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx` | Drop `chatActive` branch; always render `<Messages>` + `<Composer>` |
| Delete | `desktop-ui/src/features/chat/components/ChatPanel.tsx` | Stub fully replaced by rich UI |
| Delete | `desktop-ui/src/features/chat/components/ChatPanel.test.tsx` | Stub test |
| Delete | `desktop-ui/src/features/chat/components/ChatInput.tsx` | Stub textarea |
| Delete | `desktop-ui/src/features/chat/components/MessageBubble.tsx` | Stub bubble |

---

## Task 1: Bootstrap adapter hook + test scaffolding

**Files:**
- Create: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Create: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useKlyntbotSurfaceProps } from "./useKlyntbotSurfaceProps";

vi.mock("./useChatSession", () => ({
  useChatSession: vi.fn(),
}));

import { useChatSession } from "./useChatSession";

const mockUseChatSession = vi.mocked(useChatSession);

type Session = ReturnType<typeof useChatSession>;

const baseSession: Session = {
  messages: [],
  segments: [],
  transparency: null,
  isStreaming: false,
  activeTools: [],
  error: null,
  activeInteraction: null,
  activeDelegateAgent: null,
  statusPhase: null,
  personaMessages: [],
  debateRounds: [],
  totalDebateRounds: null,
  squadMode: null,
  judgeDecisions: [],
  consensusReached: false,
  consensusSummary: null,
  input: "",
  setInput: vi.fn(),
  send: vi.fn(),
  clearInteraction: vi.fn(),
};

describe("useKlyntbotSurfaceProps", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseChatSession.mockReturnValue(baseSession);
  });

  it("returns null when sessionKey is null", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps(null));
    expect(result.current).toBeNull();
  });

  it("returns an override object when sessionKey is provided", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current).not.toBeNull();
    expect(result.current?.messagesProps).toBeDefined();
    expect(result.current?.composerProps).toBeDefined();
  });
});
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL with `Cannot find module './useKlyntbotSurfaceProps'`.

- [ ] **Step 3: Write the minimal implementation**

Create `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`:

```ts
import type { ComponentProps } from "react";
import type { Composer } from "@/features/composer/components/Composer";
import type { Messages } from "@/features/messages/components/Messages";
import { useChatSession } from "./useChatSession";

type MessagesProps = ComponentProps<typeof Messages>;
type ComposerProps = ComponentProps<typeof Composer>;

type MessagesOverride = Pick<
  MessagesProps,
  | "items"
  | "threadId"
  | "workspaceId"
  | "isThinking"
  | "isLoadingMessages"
  | "processingStartedAt"
  | "userInputRequests"
  | "onUserInputSubmit"
>;

type ComposerOverride = Pick<
  ComposerProps,
  | "onSend"
  | "onStop"
  | "canStop"
  | "isProcessing"
  | "models"
  | "selectedModelId"
  | "onSelectModel"
  | "collaborationModes"
  | "selectedCollaborationModeId"
  | "onSelectCollaborationMode"
  | "reasoningSupported"
  | "accessMode"
  | "onSelectAccessMode"
  | "attachedImages"
  | "onPickImages"
  | "onAttachImages"
  | "onRemoveImage"
  | "dictationEnabled"
  | "queuedMessages"
  | "draftText"
  | "onDraftChange"
  | "historyKey"
>;

export type KlyntbotSurfaceOverrides = {
  messagesProps: MessagesOverride;
  composerProps: ComposerOverride;
  error: string | null;
  onDismissError: () => void;
};

export function useKlyntbotSurfaceProps(
  sessionKey: string | null,
): KlyntbotSurfaceOverrides | null {
  const chat = useChatSession(sessionKey ?? "");

  if (!sessionKey) {
    return null;
  }

  return {
    messagesProps: {
      items: [],
      threadId: sessionKey,
      workspaceId: null,
      isThinking: chat.isStreaming,
      isLoadingMessages: false,
      processingStartedAt: null,
      userInputRequests: [],
      onUserInputSubmit: () => {},
    },
    composerProps: {
      onSend: () => {},
      onStop: () => {},
      canStop: false,
      isProcessing: chat.isStreaming,
      models: [],
      selectedModelId: null,
      onSelectModel: () => {},
      collaborationModes: [{ id: "default", label: "klyntbot" }],
      selectedCollaborationModeId: "default",
      onSelectCollaborationMode: () => {},
      reasoningSupported: false,
      accessMode: "current",
      onSelectAccessMode: () => {},
      attachedImages: [],
      onPickImages: () => {},
      onAttachImages: () => {},
      onRemoveImage: () => {},
      dictationEnabled: false,
      queuedMessages: [],
      draftText: chat.input,
      onDraftChange: chat.setInput,
      historyKey: sessionKey,
    },
    error: chat.error,
    onDismissError: () => {},
  };
}
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, both tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): scaffold useKlyntbotSurfaceProps adapter hook

Returns null when sessionKey is null; otherwise produces empty-shaped
overrides for messagesProps + composerProps + error state. Subsequent
tasks fill in the real mappings.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Map persisted ChatMessage[] → ConversationItem rows

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block (after the existing tests):

```tsx
  it("maps persisted user/assistant messages into kind: 'message' rows", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [
        { id: "m1", role: "user", content: "hello" },
        { id: "m2", role: "assistant", content: "hi there" },
      ],
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m1", kind: "message", role: "user", text: "hello" },
      { id: "m2", kind: "message", role: "assistant", text: "hi there" },
    ]);
  });

  it("coerces role: 'interaction' messages to assistant rows", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m3", role: "interaction", content: "Q: which file?" }],
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m3", kind: "message", role: "assistant", text: "Q: which file?" },
    ]);
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — `items` is still `[]`.

- [ ] **Step 3: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, add a top-level helper above the hook:

```ts
import type { ConversationItem } from "@/types";
import type { ChatMessage } from "../types";

function buildPersistedItems(messages: ChatMessage[]): ConversationItem[] {
  return messages.map((msg) => ({
    id: msg.id,
    kind: "message" as const,
    role: msg.role === "user" ? ("user" as const) : ("assistant" as const),
    text: msg.content,
  }));
}
```

Then replace the `items: []` line in the hook return:

```ts
items: buildPersistedItems(chat.messages),
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all four tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): map persisted klyntbot messages to ConversationItem

Each ChatMessage becomes one kind: "message" row. role: "interaction"
collapses to "assistant" since the rich UI's message variant only
admits user/assistant.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Coalesce streaming text segments into trailing assistant message

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("coalesces streaming text segments into a single trailing assistant message", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m1", role: "user", content: "ping" }],
      segments: [
        { type: "text", content: "po" },
        { type: "text", content: "ng" },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m1", kind: "message", role: "user", text: "ping" },
      { id: "stream-assistant", kind: "message", role: "assistant", text: "pong" },
    ]);
  });

  it("does not append a streaming row when there are no text segments", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m1", role: "user", content: "ping" }],
      segments: [],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toHaveLength(1);
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — coalescing not implemented.

- [ ] **Step 3: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, replace `buildPersistedItems` with a combined builder:

```ts
import type { ChatMessage, MessageSegment } from "../types";

function buildItems(
  messages: ChatMessage[],
  segments: MessageSegment[],
): ConversationItem[] {
  const items: ConversationItem[] = messages.map((msg) => ({
    id: msg.id,
    kind: "message" as const,
    role: msg.role === "user" ? ("user" as const) : ("assistant" as const),
    text: msg.content,
  }));

  const streamingText = segments
    .filter((s): s is Extract<MessageSegment, { type: "text" }> => s.type === "text")
    .map((s) => s.content)
    .join("");

  if (streamingText.length > 0) {
    items.push({
      id: "stream-assistant",
      kind: "message",
      role: "assistant",
      text: streamingText,
    });
  }

  return items;
}
```

Update the call inside the hook return:

```ts
items: buildItems(chat.messages, chat.segments),
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all six tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): coalesce streaming text into trailing assistant row

Concatenates all in-flight text segments into a synthetic
stream-assistant message item appended after persisted history.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Map tool segments to ConversationItem `kind: "tool"` rows

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("maps tool segments to kind: 'tool' rows interleaved with text", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      segments: [
        { type: "text", content: "checking..." },
        {
          type: "tool",
          name: "search",
          action: "find foo",
          success: true,
          durationMs: 120,
          result: "no matches",
        },
        { type: "text", content: "done" },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "stream-assistant-0", kind: "message", role: "assistant", text: "checking..." },
      {
        id: "stream-tool-1",
        kind: "tool",
        toolType: "search",
        title: "search",
        detail: "find foo",
        output: "no matches",
        durationMs: 120,
        status: "completed",
      },
      { id: "stream-assistant-2", kind: "message", role: "assistant", text: "done" },
    ]);
  });

  it("emits failed tool rows with status: 'failed'", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      segments: [
        {
          type: "tool",
          name: "search",
          success: false,
          durationMs: 50,
          result: "error",
        },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items[0]).toMatchObject({
      kind: "tool",
      status: "failed",
    });
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — tool segments not mapped.

- [ ] **Step 3: Write the minimal implementation**

Replace `buildItems` in `useKlyntbotSurfaceProps.ts` with a streaming-aware version:

```ts
function buildItems(
  messages: ChatMessage[],
  segments: MessageSegment[],
): ConversationItem[] {
  const items: ConversationItem[] = messages.map((msg) => ({
    id: msg.id,
    kind: "message" as const,
    role: msg.role === "user" ? ("user" as const) : ("assistant" as const),
    text: msg.content,
  }));

  let textBuffer = "";
  let textBufferIndex = -1;

  segments.forEach((seg, idx) => {
    if (seg.type === "text") {
      if (textBufferIndex === -1) textBufferIndex = idx;
      textBuffer += seg.content;
      return;
    }
    if (textBuffer.length > 0) {
      items.push({
        id: `stream-assistant-${textBufferIndex}`,
        kind: "message",
        role: "assistant",
        text: textBuffer,
      });
      textBuffer = "";
      textBufferIndex = -1;
    }
    items.push({
      id: `stream-tool-${idx}`,
      kind: "tool",
      toolType: seg.name,
      title: seg.name,
      detail: seg.action ?? "",
      output: seg.result,
      durationMs: seg.durationMs,
      status: seg.success ? "completed" : "failed",
    });
  });

  if (textBuffer.length > 0) {
    items.push({
      id: `stream-assistant-${textBufferIndex}`,
      kind: "message",
      role: "assistant",
      text: textBuffer,
    });
  }

  return items;
}
```

Update the existing test from Task 3 to match the new id format. In `useKlyntbotSurfaceProps.test.tsx`, change the test "coalesces streaming text segments into a single trailing assistant message" to use `id: "stream-assistant-0"` instead of `id: "stream-assistant"`:

```tsx
  it("coalesces streaming text segments into a single trailing assistant message", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m1", role: "user", content: "ping" }],
      segments: [
        { type: "text", content: "po" },
        { type: "text", content: "ng" },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m1", kind: "message", role: "user", text: "ping" },
      { id: "stream-assistant-0", kind: "message", role: "assistant", text: "pong" },
    ]);
  });
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all eight tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): map klyntbot tool segments to tool rows

Tool segments interleave correctly with text — text buffers flush
into an assistant message immediately before each tool row, and
again at the end of the stream.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Adapt `activeInteraction` → `userInputRequests`

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("maps activeInteraction into userInputRequests with shape conversion", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-7",
        request: {
          title: "Pick a target",
          questions: [
            {
              id: "q1",
              title: "Which file?",
              text: "Choose the file to operate on.",
              answer_type: { type: "free_text" },
            },
          ],
        },
      },
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.userInputRequests).toEqual([
      {
        workspace_id: "",
        request_id: "req-7",
        params: {
          thread_id: "session-1",
          turn_id: "",
          item_id: "req-7",
          questions: [
            {
              id: "q1",
              header: "Which file?",
              question: "Choose the file to operate on.",
            },
          ],
        },
      },
    ]);
  });

  it("returns empty userInputRequests when no active interaction", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.userInputRequests).toEqual([]);
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — `userInputRequests` is still empty array.

- [ ] **Step 3: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, add the imports and helper above the hook:

```ts
import type {
  RequestUserInputRequest,
  RequestUserInputResponse,
} from "@/types";
import type { ActiveInteraction } from "../types";

function buildUserInputRequests(
  active: ActiveInteraction | null,
  sessionKey: string,
): RequestUserInputRequest[] {
  if (!active) return [];
  return [
    {
      workspace_id: "",
      request_id: active.requestId,
      params: {
        thread_id: sessionKey,
        turn_id: "",
        item_id: active.requestId,
        questions: active.request.questions.map((q) => ({
          id: q.id,
          header: q.title,
          question: q.text,
        })),
      },
    },
  ];
}
```

Replace the `userInputRequests: []` line in the hook return:

```ts
userInputRequests: buildUserInputRequests(chat.activeInteraction, sessionKey),
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all ten tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): adapt klyntbot activeInteraction to userInputRequests

Maps the klyntbot ActiveInteraction shape to the rich UI's
RequestUserInputRequest shape so pending Q&A surfaces in the
existing userInputRequests prop pathway.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Wire `onUserInputSubmit` to klyntbot's interaction-response IPC

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

The rich UI calls `onUserInputSubmit(request, response)` when a user answers a question. Klyntbot's backend command is `chat_respond_interaction` (takes `{ sessionKey, requestId, response: FormResponse }` per `crates/desktop/src/commands/chat.rs`). We translate the rich UI's `RequestUserInputResponse` into klyntbot's `FormResponse`.

- [ ] **Step 1: Add the `@tauri-apps/api/core` mock at the top of the test file**

In `useKlyntbotSurfaceProps.test.tsx`, add (alongside the existing `vi.mock("./useChatSession", ...)`):

```tsx
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);
```

Reset the mock in `beforeEach`:

```tsx
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseChatSession.mockReturnValue(baseSession);
    mockInvoke.mockResolvedValue(undefined);
  });
```

- [ ] **Step 2: Write the failing test**

Append inside the `describe` block:

```tsx
  it("invokes chat_respond_interaction when onUserInputSubmit fires", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-99",
        request: {
          title: "t",
          questions: [
            { id: "q1", title: "T", text: "?", answer_type: { type: "free_text" } },
          ],
        },
      },
    });

    const { result } = renderHook(() => useKlyntbotSurfaceProps("sk"));
    const req = result.current!.messagesProps.userInputRequests![0];
    result.current!.messagesProps.onUserInputSubmit!(req, {
      answers: { q1: { answers: ["yes"] } },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_interaction", {
      sessionKey: "sk",
      requestId: "req-99",
      response: {
        Completed: [
          { question_id: "q1", value: { type: "text", content: "yes" } },
        ],
      },
    });
  });
```

- [ ] **Step 3: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — `onUserInputSubmit` is currently a no-op.

- [ ] **Step 4: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, add the import and helper:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Answer, FormResponse } from "../types";
```

Add a helper above the hook:

```ts
function toFormResponse(response: RequestUserInputResponse): FormResponse {
  const answers: Answer[] = Object.entries(response.answers).map(
    ([questionId, ans]) => ({
      question_id: questionId,
      value: { type: "text", content: ans.answers.join(", ") },
    }),
  );
  return { Completed: answers };
}
```

Replace `onUserInputSubmit: () => {}` in the hook return with:

```ts
onUserInputSubmit: (request, response) => {
  void invoke("chat_respond_interaction", {
    sessionKey,
    requestId: String(request.request_id),
    response: toFormResponse(response),
  });
},
```

- [ ] **Step 5: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all eleven tests green.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): wire onUserInputSubmit to chat_respond_interaction

Translates the rich UI's RequestUserInputResponse into klyntbot's
FormResponse Completed shape and invokes chat_respond_interaction.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Capture `processingStartedAt` on streaming false→true edge

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("sets processingStartedAt on the false→true streaming edge", () => {
    const beforeStart = Date.now() - 1;
    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: false });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(result.current?.messagesProps.processingStartedAt).toBeNull();

    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: true });
    rerender();
    const ts = result.current?.messagesProps.processingStartedAt;
    expect(typeof ts).toBe("number");
    expect(ts).toBeGreaterThanOrEqual(beforeStart);
  });

  it("clears processingStartedAt on streaming true→false edge", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: true });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(typeof result.current?.messagesProps.processingStartedAt).toBe("number");

    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: false });
    rerender();
    expect(result.current?.messagesProps.processingStartedAt).toBeNull();
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — `processingStartedAt` is hardcoded to `null`.

- [ ] **Step 3: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, add `useEffect`, `useRef`, `useState` imports:

```ts
import { useEffect, useRef, useState } from "react";
```

Inside the hook (before the early return), add:

```ts
const [processingStartedAt, setProcessingStartedAt] = useState<number | null>(
  chat.isStreaming ? Date.now() : null,
);
const prevIsStreamingRef = useRef(chat.isStreaming);
useEffect(() => {
  const wasStreaming = prevIsStreamingRef.current;
  if (!wasStreaming && chat.isStreaming) {
    setProcessingStartedAt(Date.now());
  } else if (wasStreaming && !chat.isStreaming) {
    setProcessingStartedAt(null);
  }
  prevIsStreamingRef.current = chat.isStreaming;
}, [chat.isStreaming]);
```

Replace the `processingStartedAt: null` line in the hook return:

```ts
processingStartedAt,
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all thirteen tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): capture processingStartedAt on streaming edges

Stamps Date.now() on the isStreaming false→true transition and
clears on the reverse edge. Drives the WorkingIndicator timer in
the rich UI.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Wire composer `onSend` and the static model pill

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

`useChatSession.send()` reads from `chat.input` (which is wired to `draftText` / `onDraftChange`), so calling `chat.send()` after the controlled-input round-trip is sufficient. The model pill needs a single-item `models` array for the dropdown to render meaningfully.

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("calls chat.send when composer onSend fires", () => {
    const sendSpy = vi.fn();
    mockUseChatSession.mockReturnValue({ ...baseSession, send: sendSpy });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    result.current!.composerProps.onSend("hello", []);
    expect(sendSpy).toHaveBeenCalledTimes(1);
  });

  it("supplies a single-item models[] so the model pill renders", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.composerProps.models).toEqual([
      { id: "klyntbot", displayName: "klyntbot", model: "klyntbot" },
    ]);
    expect(result.current?.composerProps.selectedModelId).toBe("klyntbot");
  });

  it("supplies a default collaboration mode so the pill stays visible", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.composerProps.collaborationModes).toEqual([
      { id: "default", label: "klyntbot" },
    ]);
    expect(result.current?.composerProps.selectedCollaborationModeId).toBe("default");
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — the `onSend` and `models[]` assertions fail (current scaffolding has a no-op send and empty models). The collaboration assertion passes since the scaffolding already supplied a default mode.

- [ ] **Step 3: Write the minimal implementation**

Update the composer return in `useKlyntbotSurfaceProps.ts`:

```ts
onSend: () => {
  void chat.send();
},
models: [{ id: "klyntbot", displayName: "klyntbot", model: "klyntbot" }],
selectedModelId: "klyntbot",
```

(`collaborationModes`, `selectedCollaborationModeId` stay as scaffolded.)

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all sixteen tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): wire composer send + static model pill

Composer's onSend triggers chat.send() — the controlled-input
contract via draftText/onDraftChange keeps chat.input in sync.
Model and collaboration pills render with single-item arrays so
they appear in the chat surface like the target screenshot.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Add error dismissal mechanism

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx`

`chatStreamStore.error` is the persistent source. We don't have a `clearError` method on the store, so the dismiss tracks dismissal via local state keyed on the error string — when a fresh error arrives, the dismissal resets.

- [ ] **Step 1: Write the failing test**

Append inside the `describe` block:

```tsx
  it("exposes chat.error and clears it on onDismissError", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, error: "oops" });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(result.current?.error).toBe("oops");

    result.current!.onDismissError();
    rerender();
    expect(result.current?.error).toBeNull();
  });

  it("re-shows error when a different error string arrives after dismissal", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, error: "first" });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    result.current!.onDismissError();
    rerender();
    expect(result.current?.error).toBeNull();

    mockUseChatSession.mockReturnValue({ ...baseSession, error: "second" });
    rerender();
    expect(result.current?.error).toBe("second");
  });
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: FAIL — `onDismissError` is a no-op.

- [ ] **Step 3: Write the minimal implementation**

In `useKlyntbotSurfaceProps.ts`, inside the hook (near the `processingStartedAt` block), add:

```ts
const [dismissedError, setDismissedError] = useState<string | null>(null);
useEffect(() => {
  if (dismissedError !== null && chat.error !== dismissedError) {
    setDismissedError(null);
  }
}, [chat.error, dismissedError]);
const visibleError = dismissedError === chat.error ? null : chat.error;
```

Replace the `error` and `onDismissError` lines in the hook return:

```ts
error: visibleError,
onDismissError: () => setDismissedError(chat.error),
```

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test useKlyntbotSurfaceProps -- --run
```

Expected: PASS, all eighteen tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.ts \
        desktop-ui/src/features/chat/hooks/useKlyntbotSurfaceProps.test.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): add error dismissal to klyntbot surface adapter

Tracks dismissed error string in local state; auto-resets when the
underlying chat.error changes so a new failure resurfaces. Avoids
needing a store-level clearError() API.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Create `<ChatErrorBanner>` component

**Files:**
- Create: `desktop-ui/src/features/chat/components/ChatErrorBanner.tsx`
- Create: `desktop-ui/src/features/chat/components/ChatErrorBanner.test.tsx`
- Create: `desktop-ui/src/styles/chat-error-banner.css`
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/chat/components/ChatErrorBanner.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { ChatErrorBanner } from "./ChatErrorBanner";

describe("ChatErrorBanner", () => {
  it("renders nothing when error is null", () => {
    const { container } = render(
      <ChatErrorBanner error={null} onDismiss={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the error string when set", () => {
    render(<ChatErrorBanner error="boom" onDismiss={() => {}} />);
    expect(screen.getByRole("alert")).toHaveTextContent("boom");
  });

  it("invokes onDismiss when the dismiss button is clicked", () => {
    const onDismiss = vi.fn();
    render(<ChatErrorBanner error="boom" onDismiss={onDismiss} />);
    fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run the test — expect FAIL**

```bash
cd desktop-ui && bun run test ChatErrorBanner -- --run
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the minimal implementation**

Create `desktop-ui/src/features/chat/components/ChatErrorBanner.tsx`:

```tsx
type Props = {
  error: string | null;
  onDismiss: () => void;
};

export function ChatErrorBanner({ error, onDismiss }: Props) {
  if (error === null) return null;
  return (
    <div className="chat-error-banner" role="alert">
      <span className="chat-error-banner__message">{error}</span>
      <button
        type="button"
        className="chat-error-banner__dismiss"
        aria-label="Dismiss error"
        onClick={onDismiss}
      >
        ×
      </button>
    </div>
  );
}
```

Create `desktop-ui/src/styles/chat-error-banner.css`:

```css
.chat-error-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--color-error-bg, rgba(220, 38, 38, 0.12));
  color: var(--color-error-fg, #b91c1c);
  border-bottom: 1px solid var(--color-error-border, rgba(220, 38, 38, 0.3));
  font-size: var(--fs-xs);
}

.chat-error-banner__message {
  flex: 1;
  white-space: pre-wrap;
}

.chat-error-banner__dismiss {
  background: transparent;
  border: none;
  cursor: pointer;
  color: inherit;
  font-size: var(--fs-md);
  line-height: 1;
  padding: 2px 6px;
}

.chat-error-banner__dismiss:hover {
  opacity: 0.7;
}
```

Append the import to `desktop-ui/src/styles/index.css`. Read the file first to find the right spot in the import chain, then add `@import "./chat-error-banner.css";` next to other component-specific imports.

- [ ] **Step 4: Run the test — expect PASS**

```bash
cd desktop-ui && bun run test ChatErrorBanner -- --run
```

Expected: PASS, all three tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/components/ChatErrorBanner.tsx \
        desktop-ui/src/features/chat/components/ChatErrorBanner.test.tsx \
        desktop-ui/src/styles/chat-error-banner.css \
        desktop-ui/src/styles/index.css
git commit -m "$(cat <<'EOF'
feat(desktop-ui): add ChatErrorBanner component

A dismissible banner for klyntbot stream errors, rendered above
the messages list. Uses --fs-xs typography token; plain CSS, BEM-ish
class names per project conventions.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Inject prop swap in `MainApp.tsx`

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx` (around lines 1729–1748)

This is the integration point: between `useMainAppLayoutSurfaces(...)` (which assembles the Codex-flow `layoutSurfaces`) and `useMainAppLayoutNodes(layoutSurfaces)` (which renders the React nodes), call `useKlyntbotSurfaceProps`, merge the overrides into a copy of `layoutSurfaces`, and pass that to `useMainAppLayoutNodes`. Also wrap `messagesNode` with `<ChatErrorBanner>` when in chat mode.

There is no clean unit-test seam for this wiring; correctness is verified end-to-end in Task 14. The change is mechanical but must be exact.

- [ ] **Step 1: Read the call site**

```bash
cd desktop-ui && rg -n "useMainAppLayoutSurfaces|useMainAppLayoutNodes" src/features/app/components/MainApp.tsx
```

Confirm `useMainAppLayoutSurfaces({...})` ends around line 1729 (closing `})`) and `useMainAppLayoutNodes(layoutSurfaces)` is called around line 1745.

- [ ] **Step 2: Add imports at the top of `MainApp.tsx`**

Add to the existing imports (alphabetize-merge with the chat block already present):

```ts
import { ChatErrorBanner } from "@/features/chat/components/ChatErrorBanner";
import { useKlyntbotSurfaceProps } from "@/features/chat/hooks/useKlyntbotSurfaceProps";
```

- [ ] **Step 3: Insert the override block between the two hook calls**

After the closing `});` of `useMainAppLayoutSurfaces({...})` (around line 1729), and before the destructuring of `useMainAppLayoutNodes(...)` (around line 1731), insert:

```tsx
  const klyntbotSurface = useKlyntbotSurfaceProps(
    appView === "chat" ? selectedSessionKey : null,
  );

  const finalLayoutSurfaces = klyntbotSurface
    ? {
        ...layoutSurfaces,
        primary: {
          ...layoutSurfaces.primary,
          messagesProps: {
            ...layoutSurfaces.primary.messagesProps,
            ...klyntbotSurface.messagesProps,
          },
          composerProps: layoutSurfaces.primary.composerProps
            ? {
                ...layoutSurfaces.primary.composerProps,
                ...klyntbotSurface.composerProps,
              }
            : layoutSurfaces.primary.composerProps,
        },
      }
    : layoutSurfaces;
```

Then change `useMainAppLayoutNodes(layoutSurfaces)` to `useMainAppLayoutNodes(finalLayoutSurfaces)`.

- [ ] **Step 4: Wrap `messagesNode` with `<ChatErrorBanner>` when in klyntbot chat**

Find the line:

```tsx
const mainMessagesNode =
  showWorkspaceHome && appView !== "chat" ? workspaceHomeNode : messagesNode;
```

Replace with:

```tsx
const chatMessagesNode = klyntbotSurface ? (
  <>
    <ChatErrorBanner
      error={klyntbotSurface.error}
      onDismiss={klyntbotSurface.onDismissError}
    />
    {messagesNode}
  </>
) : (
  messagesNode
);
const mainMessagesNode =
  showWorkspaceHome && appView !== "chat" ? workspaceHomeNode : chatMessagesNode;
```

- [ ] **Step 5: Verify typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS with no errors. If `ComponentProps<typeof Composer>` requires fields not in the override, the spread merge picks them up from `layoutSurfaces.primary.composerProps` — but if `layoutSurfaces.primary.composerProps` is `undefined` (which it can be), the conditional skip preserves the rich UI's existing null-handling. Re-read `buildPrimaryNodes.tsx:51-53` to confirm: `composerNode = ... options.composerProps ? <Composer ... /> : null` — composer can be absent.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "$(cat <<'EOF'
feat(desktop-ui): wire klyntbot surface overrides into MainApp layout

When in a klyntbot chat (appView === "chat" && selectedSessionKey),
useKlyntbotSurfaceProps produces messagesProps/composerProps overrides
that are merged on top of the Codex-flow surfaces before
useMainAppLayoutNodes renders. ChatErrorBanner wraps messagesNode in
chat mode.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Drop the `chatActive` branch in `buildPrimaryNodes.tsx`

**Files:**
- Modify: `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx`

After Task 11, `messagesProps` already contains klyntbot data when `chatActive`, so the component-level branch is redundant. Remove it and the now-unused `ChatPanel` import.

- [ ] **Step 1: Look for an existing test for this builder**

```bash
cd desktop-ui && fd buildPrimaryNodes
```

Expected: only `buildPrimaryNodes.tsx`. There is no test file currently. We will not add one — `buildPrimaryNodes` is a thin React-element factory and is exercised end-to-end via Task 14's smoke checks. Skip directly to Step 2.

- [ ] **Step 2: Replace the file contents**

Overwrite `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx` with:

```tsx
import { ApprovalToasts } from "@app/components/ApprovalToasts";
import { MainHeader } from "@app/components/MainHeader";
import { SidebarChatLayout } from "@app/components/SidebarChatLayout";
import ArrowLeft from "lucide-react/dist/esm/icons/arrow-left";
import { Composer } from "@/features/composer/components/Composer";
import { Home } from "@/features/home/components/Home";
import { Messages } from "@/features/messages/components/Messages";
import { ErrorToasts } from "@/features/notifications/components/ErrorToasts";
import { UpdateToast } from "@/features/update/components/UpdateToast";
import type { LayoutNodesResult, LayoutPrimarySurface } from "./types";

export type PrimaryLayoutNodesOptions = LayoutPrimarySurface;

type PrimaryLayoutNodes = Pick<
  LayoutNodesResult,
  | "sidebarNode"
  | "messagesNode"
  | "composerNode"
  | "approvalToastsNode"
  | "updateToastNode"
  | "errorToastsNode"
  | "homeNode"
  | "mainHeaderNode"
  | "desktopTopbarLeftNode"
>;

export function buildPrimaryNodes(options: PrimaryLayoutNodesOptions): PrimaryLayoutNodes {
  const sidebarNode = (
    <SidebarChatLayout
      onOpenSettings={options.sidebarProps.onOpenSettings}
      onNewChat={options.sidebarProps.onNewChat}
      threads={options.sidebarProps.threads}
      selectedSessionKey={options.sidebarProps.selectedSessionKey}
      onSelectThread={options.sidebarProps.onSelectThread}
    />
  );

  const messagesNode = <Messages {...options.messagesProps} />;

  const composerNode = options.composerProps ? (
    <Composer {...options.composerProps} />
  ) : null;

  const approvalToastsNode = <ApprovalToasts {...options.approvalToastsProps} />;
  const updateToastNode = <UpdateToast {...options.updateToastProps} />;
  const errorToastsNode = <ErrorToasts {...options.errorToastsProps} />;
  const homeNode = <Home {...options.homeProps} />;
  const mainHeaderNode = options.mainHeaderProps ? (
    <MainHeader {...options.mainHeaderProps} />
  ) : null;

  const desktopTopbarLeftNode = (
    <>
      {options.desktopTopbarProps.showBackToChat && (
        <button
          className="icon-button back-button"
          onClick={options.desktopTopbarProps.onExitDiff}
          aria-label="Back to chat"
        >
          <ArrowLeft aria-hidden />
        </button>
      )}
      {mainHeaderNode}
    </>
  );

  return {
    sidebarNode,
    messagesNode,
    composerNode,
    approvalToastsNode,
    updateToastNode,
    errorToastsNode,
    homeNode,
    mainHeaderNode,
    desktopTopbarLeftNode,
  };
}
```

The `ChatPanel` import and the `chatActive` computation are gone. `chatViewProps` on `LayoutPrimarySurface` is now dead data for this builder but is kept on the type for callers.

- [ ] **Step 3: Verify typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx
git commit -m "$(cat <<'EOF'
refactor(desktop-ui): drop chatActive branch in buildPrimaryNodes

Klyntbot data now reaches the rich UI through messagesProps /
composerProps overrides applied in MainApp.tsx, so the component-level
ChatPanel branch is no longer needed.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Delete the four stub files

**Files:**
- Delete: `desktop-ui/src/features/chat/components/ChatPanel.tsx`
- Delete: `desktop-ui/src/features/chat/components/ChatPanel.test.tsx`
- Delete: `desktop-ui/src/features/chat/components/ChatInput.tsx`
- Delete: `desktop-ui/src/features/chat/components/MessageBubble.tsx`

- [ ] **Step 1: Verify no remaining importers**

```bash
cd desktop-ui && rg -l "from.*chat/components/(ChatPanel|ChatInput|MessageBubble)"
```

Expected: empty output. If any matches remain, inspect and either remove the import (if unused) or fix the call site (if it is using one of these stubs and shouldn't be).

- [ ] **Step 2: Delete the files**

```bash
cd desktop-ui && rm -f \
  src/features/chat/components/ChatPanel.tsx \
  src/features/chat/components/ChatPanel.test.tsx \
  src/features/chat/components/ChatInput.tsx \
  src/features/chat/components/MessageBubble.tsx
```

- [ ] **Step 3: Verify the chat/components directory still exists with the new banner file only**

```bash
ls desktop-ui/src/features/chat/components/
```

Expected output: `ChatErrorBanner.test.tsx`, `ChatErrorBanner.tsx`.

- [ ] **Step 4: Run typecheck, lint, and full test suite**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test -- --run
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/chat/components/
git commit -m "$(cat <<'EOF'
chore(desktop-ui): delete chat stub components

ChatPanel, ChatInput, and MessageBubble are fully replaced by the
rich Messages + Composer surface driven via useKlyntbotSurfaceProps.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Final verification (manual smoke + static checks)

**Files:** none modified

- [ ] **Step 1: Run all static checks**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test -- --run
```

Expected: PASS across the board.

- [ ] **Step 2: Start the dev environment**

In one terminal:

```bash
cd desktop-ui && bun run dev
```

In another terminal (from repo root):

```bash
cargo tauri dev
```

- [ ] **Step 3: Manual smoke test — golden path**

In the running app:

1. Click **New chat** in the sidebar. Confirm a session opens with the rich UI: split-pane (messages on top, composer at bottom), composer pills visible (model "klyntbot", access "current", collaboration "klyntbot").
2. Type a message and press Enter. Confirm:
   - The textarea clears.
   - An assistant message row appears and text streams in (one row, not many small fragments).
   - If the agent invokes a tool, a collapsed tool row renders inline.
   - The `WorkingIndicator` timer increments while streaming.
3. After completion, confirm the streaming row is replaced by the persisted assistant message (no flash, no duplication).
4. Click another thread in the sidebar. Confirm the new session loads its history correctly.

- [ ] **Step 4: Manual smoke test — non-chat path**

1. Navigate out of chat (e.g., open the workspace home or switch `appView` away from `"chat"`). Confirm the existing Codex-flow rich UI renders unchanged.
2. Switch back to chat and confirm the klyntbot surface still works. No layout glitches.

- [ ] **Step 5: Manual smoke test — error path**

1. Force an error (disconnect provider, or temporarily mis-configure the model in `~/.klyntbot/config.json`).
2. Send a message. Confirm `<ChatErrorBanner>` appears above the messages list with the error string and a working dismiss button.
3. Click dismiss. Banner disappears. Confirm subsequent successful sends do not show the dismissed error.

- [ ] **Step 6: Final lint and test pass**

```bash
cd desktop-ui && bun run lint && bun run typecheck && bun run test -- --run
```

Expected: zero warnings, all tests pass.

- [ ] **Step 7: Commit any minor fixes from smoke testing**

If smoke testing surfaced a small fix (CSS spacing, prop wiring oversight), commit it as a follow-up:

```bash
git add desktop-ui/
git commit -m "$(cat <<'EOF'
fix(desktop-ui): smoke-test fix for klyntbot chat surface

<short description of the fix>

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If smoke testing was clean, no extra commit needed.

---

## Self-review checklist

**Spec coverage:**

| Spec section | Implemented in |
|---|---|
| Architecture: prop-level switch | Task 11 |
| `useKlyntbotSurfaceProps` adapter hook | Tasks 1–9 |
| Stream → ConversationItem mapping (text, tool, message, userInput) | Tasks 2–6 |
| Composer prop strategy (model, mode, access, hidden pills) | Task 8 + Task 1 scaffolding |
| `processingStartedAt` capture | Task 7 |
| Error surface (`ChatErrorBanner`) | Tasks 9–10, wired in Task 11 |
| Drop `chatActive` branch in `buildPrimaryNodes` | Task 12 |
| Delete stub files | Task 13 |
| Testing (Vitest + manual smoke) | All TDD tasks + Task 14 |

**Open spec questions resolved during this plan:**

1. Injection point: `MainApp.tsx` between `useMainAppLayoutSurfaces` and `useMainAppLayoutNodes` (Task 11). NOT `useMainAppShellProps` — by then nodes are already rendered.
2. `onThreadsChanged`: dropped — `useChatThreads` already auto-refetches on `chat:thread_created` / `chat:thread_updated` events (verified in `useChatThreads.ts:30-47`).
3. Composer empty-array handling: `collaborationModes` hides on empty (`ComposerMetaBar.tsx:70`) → must supply `[{ id: "default", label: "klyntbot" }]`. `models` shows "No models" placeholder option but we supply a real single-item array.
4. Error surface: new `<ChatErrorBanner>` (Task 10), wrapped over messagesNode in MainApp (Task 11). No `Messages.tsx` edits.
5. `processingStartedAt`: `useEffect` watching `isStreaming` previous-value via `useRef` (Task 7).
6. `RequestUserInputRequest` shape: documented and tested in Task 5; `Question.title` → `header`, `Question.text` → `question`, `requestId` doubles as `request_id` and `item_id`, `turn_id: ""`, `workspace_id: ""`.

**Type consistency check:**
- Hook return type `KlyntbotSurfaceOverrides` defined in Task 1; used unchanged in Task 11.
- `MessagesProps` / `ComposerProps` derived via `ComponentProps<typeof Messages | Composer>` so they auto-track the source-of-truth definitions.
- `ChatMessage`, `MessageSegment`, `ActiveInteraction`, `FormResponse`, `Answer` all imported from `@/features/chat/types`.
- `RequestUserInputRequest`, `RequestUserInputResponse`, `ConversationItem` all imported from `@/types`.

**No placeholders:**
- All steps include real, runnable code blocks.
- Commit messages are concrete.
- No "TBD" / "TODO" / "fill in details".
- File paths are absolute from repo root.

---
