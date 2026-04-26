# klyntbot Chat MVP — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the "New chat" button in the new desktop-ui's sidebar to klyntbot's existing Rust chat backend (sessions, threads, streaming), so users can hold a real conversation in the new UI.

**Architecture:** Conditional swap in `buildPrimaryNodes` — when `appView === "chat"`, substitute a new `<ChatPanel>` for `messagesNode + composerNode`. Port `chatStreamStore` and chat session hooks from `desktop-ui.bak/` (pure-TS port, framework-agnostic). Build a thin React surface (`ChatPanel`, `MessageBubble`, `ChatInput`) in the new UI's idioms (plain CSS, direct `invoke()`, no `useQuery`). Sidebar lists real threads from `chat_threads`.

**Tech Stack:** React 19, Tauri 2, `@tauri-apps/api/{core,event}` directly, plain CSS with ds-tokens, `react-markdown` (already installed), `vitest` + `@testing-library/react` (existing).

**Spec:** `docs/superpowers/specs/2026-04-26-klyntbot-chat-mvp-design.md`

**Constraint discovered during planning:** the new desktop-ui currently aliases `@tauri-apps/api/core` and `@tauri-apps/api/event` to mock modules in `vite.config.ts`. The chat work requires real Tauri APIs, so Task 2 removes those two aliases. Browser-only dev mode is explicitly out of scope (per user direction); we build for `cargo tauri dev`.

**Working directory:** `/Users/maixuantung/Dev/raki/klyntbot`

**Branch:** `feature/intergrate-chat` (already checked out)

---

## Task 1: Update stale CLAUDE.md desktop-ui section

**Files:**
- Modify: `CLAUDE.md` (the "Desktop UI (desktop-ui/)" section — current text references Tailwind, Biome, `bun run lint:fix`, `useQuery`/`ipc()`, none of which exist in the new UI)

- [ ] **Step 1: Read current section**

Run: `grep -n "Desktop UI" /Users/maixuantung/Dev/raki/klyntbot/CLAUDE.md`

Identify line range of "## Desktop UI (desktop-ui/)" and "## Desktop App (Tauri 2)". Read everything between them.

- [ ] **Step 2: Replace the desktop-ui section**

Replace the entire "## Desktop UI (desktop-ui/)" section (everything before "## Desktop App (Tauri 2)") with this:

```markdown
## Desktop UI (desktop-ui/)

```bash
cd desktop-ui && bun install        # Install deps (always bun, never npm)
cd desktop-ui && bun run dev:vite   # Vite dev server (port 1420)
cd desktop-ui && bun run build      # Production build (tsc + vite build)
cd desktop-ui && bun run lint       # ESLint check
cd desktop-ui && bun run typecheck  # tsc --noEmit
cd desktop-ui && bun run test       # Vitest (run once)
cd desktop-ui && bun run test:watch # Vitest (watch mode)
```

**Path aliases** (`vite.config.ts` + `tsconfig.json`):
- `@/*` → `src/*`
- `@app/*` → `src/features/app/*`
- `@settings/*` → `src/features/settings/*`
- `@threads/*` → `src/features/threads/*`
- `@services/*` → `src/services/*`
- `@utils/*` → `src/utils/*`

Always use these in imports, never relative `../../` paths. Note: there is **no** `@shared` or `@features` alias — those were the old UI's conventions.

**Styling:** Plain CSS. No Tailwind. All styles in `src/styles/*.css`, imported through `src/styles/index.css`. Design tokens in `src/styles/ds-tokens.css`; themes in `src/styles/themes.{dark,light,dim,system}.css`. Class naming is BEM-ish (e.g. `sidebar-chat__nav-item`). When adding a new feature with its own CSS file, add an `@import` line to `src/styles/index.css`.

**Tauri IPC:** Direct `invoke()` from `@/api/client` (which re-exports `@tauri-apps/api/core`). There is no `useQuery` / `useMutation` / `ipc()` wrapper — call `invoke()` from a `useEffect` and manage state with `useState`. For Tauri events, import `listen` from `@tauri-apps/api/event` directly, or use the per-event hubs in `src/services/events.ts`. Endpoint definitions live under `src/api/endpoints/`.

**Markdown rendering:** Reuse `Markdown` from `@/features/messages/components/Markdown` rather than importing `react-markdown` directly.

**Testing:** Vitest + `@testing-library/react`. Test files colocated as `Component.test.tsx`. Mock Tauri APIs per-test via `vi.mock("@tauri-apps/api/core", ...)`.

**Linter:** ESLint via `bun run lint`. No Biome.
```

Write: use Edit tool to replace the existing section.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): update desktop-ui section for new UI stack

The new desktop-ui replaces the bak: no Tailwind, no Biome, no
useQuery/ipc abstractions. Plain CSS + ds-tokens, ESLint, direct
invoke() from @/api/client.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Remove Tauri-mock aliases for core and event

**Files:**
- Modify: `desktop-ui/vite.config.ts` (remove two regex alias entries around lines 105–122)
- Delete: `desktop-ui/src/services/__mocks__/tauri-api-core.ts`
- Delete: `desktop-ui/src/services/__mocks__/tauri-api-event.ts`

**Why:** The current mocks intercept every `invoke()` and `listen()` call site-wide, returning null/no-op even inside the real Tauri webview. Chat needs real Tauri.

- [ ] **Step 1: Verify the typecheck baseline**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean (or current pre-existing errors — note the count).

- [ ] **Step 2: Edit vite.config.ts**

Remove these two alias entries (and their surrounding `{}, ` separators):

```ts
{
  find: /^@tauri-apps\/api\/core$/,
  replacement: fileURLToPath(
    new URL(
      "./src/services/__mocks__/tauri-api-core.ts",
      import.meta.url,
    ),
  ),
},
{
  find: /^@tauri-apps\/api\/event$/,
  replacement: fileURLToPath(
    new URL(
      "./src/services/__mocks__/tauri-api-event.ts",
      import.meta.url,
    ),
  ),
},
```

Leave the other Tauri shims (`app`, `dpi`, `menu`, `webview`, `window`, plugin shims) in place for now.

- [ ] **Step 3: Delete the mock files**

```bash
rm desktop-ui/src/services/__mocks__/tauri-api-core.ts
rm desktop-ui/src/services/__mocks__/tauri-api-event.ts
```

- [ ] **Step 4: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: same as baseline. Real `@tauri-apps/api/core` exports `invoke`, `convertFileSrc`, `isTauri`, `Channel`, `addPluginListener`, `checkPermissions`, `requestPermissions` — all the names existing call sites use.

If typecheck reports new errors, the most likely cause is `isTauri()` (function in real package) being used as a constant somewhere. Add explicit calls (`isTauri()` not `isTauri`) where needed.

- [ ] **Step 5: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: passes. Fix any unused-import warnings introduced by the deletion.

- [ ] **Step 6: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: existing tests still pass. Tests that mocked `@tauri-apps/api/core` via `vi.mock` continue to work (vi.mock takes precedence over Vite aliases anyway). Tests that relied on the alias-mock auto-stubbing may break — fix per-test by adding `vi.mock(...)` calls.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/vite.config.ts desktop-ui/src/services/__mocks__/
git commit -m "build(desktop-ui): remove Tauri core/event mock aliases

The mocks intercepted invoke/listen even in real Tauri webview,
making backend integration impossible. Remove for chat work; the
remaining @tauri-apps shims (window, dialog, etc.) stay in place.
Browser-only dev is no longer supported for these calls.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Create chat feature scaffold and port types

**Files:**
- Create: `desktop-ui/src/features/chat/types.ts`
- Create: `desktop-ui/src/features/chat/index.ts` (barrel — empty for now)

- [ ] **Step 1: Create the directory**

```bash
mkdir -p desktop-ui/src/features/chat/{store,hooks,components}
```

- [ ] **Step 2: Write `types.ts`**

Copy the entirety of `desktop-ui.bak/src/shared/types/chat.ts` into `desktop-ui/src/features/chat/types.ts`, with these edits:

1. Remove the top two imports (`import type { DelegationInfo, PlanData } from "./common";` and `import type { Task } from "./tasks";`).
2. Inline minimal stand-in types instead — chat UI only references `DelegationInfo` and `PlanData` inside `TransparencyData` (which v1 doesn't render but the store still populates). Add at the top of the file:

```ts
// Inlined from .bak shared/types — only the fields the store actually populates.
export interface DelegationInfo {
  fromAgent: string;
  toAgent: string;
  query: string;
  durationMs: number;
  success: boolean;
}

export interface PlanData {
  steps: string[];
  rawPlan: string;
  completedSteps: { stepIndex: number; description: string; toolName: string }[];
}
```

3. Remove the `AgentStatus` interface and its `Task` import dependency at the bottom of the file (lines 374-379 in the bak source). Not used by chat.

- [ ] **Step 3: Write `index.ts` barrel**

```ts
// Public exports for the chat feature. Filled in as components land.
export {};
```

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/
git commit -m "feat(desktop-ui/chat): scaffold chat feature with ported types

Ported from desktop-ui.bak/src/shared/types/chat.ts; inlined the two
cross-domain types (DelegationInfo, PlanData) and dropped AgentStatus.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Port chatStreamStore

**Files:**
- Create: `desktop-ui/src/features/chat/store/chatStreamStore.ts`

**Source:** `desktop-ui.bak/src/shared/stores/chatStreamStore.ts` (~907 lines, port verbatim with import edits).

- [ ] **Step 1: Copy file**

```bash
cp desktop-ui.bak/src/shared/stores/chatStreamStore.ts desktop-ui/src/features/chat/store/chatStreamStore.ts
```

- [ ] **Step 2: Edit the imports at the top of the file**

Replace the top imports:

```ts
import { DEV_SSE_BASE, isTauri, qualifiedToolName } from "@shared/lib/utils";
import type {
  ActiveInteraction,
  // ... long list
} from "@shared/types";
```

With:

```ts
import { isTauri } from "@tauri-apps/api/core";
import type {
  ActiveInteraction,
  // ... same long list
} from "../types";

const DEV_SSE_BASE = "http://127.0.0.1:3456";

function qualifiedToolName(name: string, action?: string): string {
  return action ? `${name}:${action}` : name;
}
```

Note: `isTauri` from real Tauri is a **function**, not a constant. Search the file for usages and change `if (!isTauri)` → `if (!isTauri())` and `if (isTauri)` → `if (isTauri())`. There should be 2-3 occurrences.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes. If errors mention missing types, ensure `types.ts` re-exports them (Task 3 should already cover this).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/chat/store/chatStreamStore.ts
git commit -m "feat(desktop-ui/chat): port chatStreamStore from bak

Pure-TS singleton managing per-session streaming state and Tauri
event subscriptions. Imports rewritten for new UI's paths;
isTauri changed from constant to function call.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Port useAgentStream

**Files:**
- Create: `desktop-ui/src/features/chat/hooks/useAgentStream.ts`

**Source:** `desktop-ui.bak/src/features/chat/hooks/useAgentStream.ts` (131 lines).

- [ ] **Step 1: Copy file**

```bash
cp desktop-ui.bak/src/features/chat/hooks/useAgentStream.ts desktop-ui/src/features/chat/hooks/useAgentStream.ts
```

- [ ] **Step 2: Edit imports**

Replace top imports:

```ts
import { chatStreamStore } from "@shared/stores/chatStreamStore";
import type {
  ActiveInteraction,
  // ...
} from "@shared/types";
```

With:

```ts
import { chatStreamStore } from "../store/chatStreamStore";
import type {
  ActiveInteraction,
  // ... same list
} from "../types";
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useAgentStream.ts
git commit -m "feat(desktop-ui/chat): port useAgentStream hook

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Port useChatSession (with `useQuery` swap)

**Files:**
- Create: `desktop-ui/src/features/chat/hooks/useChatSession.ts`

**Source:** `desktop-ui.bak/src/features/chat/hooks/useChatSession.ts` (152 lines). Two structural edits — `ipc()` → `invoke()`, and `useQuery()` → manual fetch.

- [ ] **Step 1: Copy file**

```bash
cp desktop-ui.bak/src/features/chat/hooks/useChatSession.ts desktop-ui/src/features/chat/hooks/useChatSession.ts
```

- [ ] **Step 2: Replace imports**

Replace:

```ts
import { useQuery } from "@shared/hooks/useQuery";
import { parseApiError } from "@shared/lib/utils";
import type {
  ActiveInteraction,
  // ...
} from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAgentStream } from "./useAgentStream";
import { ipc } from "./useIpc";
```

With:

```ts
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ActiveInteraction,
  ChatMessage,
  DebateRound,
  JudgeDecisionEntry,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "../types";
import { useAgentStream } from "./useAgentStream";

function parseApiError(e: unknown): { message: string } {
  if (e instanceof Error) return { message: e.message };
  if (typeof e === "object" && e !== null && "message" in e) {
    return { message: String((e as { message: unknown }).message) };
  }
  return { message: String(e) };
}
```

- [ ] **Step 3: Replace the `useQuery` block (top of `useChatSession`)**

Replace:

```ts
const { data: messages, refetch } = useQuery<ChatMessage[]>(
  "chat_messages",
  sessionKey ? { sessionKey } : null,
  [],
  {
    invalidateOn: ["chat:message_added"],
    invalidateFilter: (payload) =>
      (payload as { sessionKey?: string })?.sessionKey === sessionKey,
  },
);
```

With:

```ts
const [messages, setMessages] = useState<ChatMessage[]>([]);
const refetch = useCallback(async () => {
  if (!sessionKey) return;
  try {
    const result = await invoke<ChatMessage[]>("chat_messages", { sessionKey });
    setMessages(result);
  } catch {
    // Errors surface via the streaming store's error field; messages just stay
    // at the previous value rather than wiping the conversation.
  }
}, [sessionKey]);

useEffect(() => {
  refetch();
}, [refetch]);

useEffect(() => {
  if (!sessionKey) return;
  const unsub = listen<{ sessionKey?: string }>("chat:message_added", (event) => {
    if (event.payload?.sessionKey === sessionKey) {
      refetch();
    }
  });
  return () => {
    unsub.then((fn) => fn()).catch(() => {});
  };
}, [sessionKey, refetch]);
```

- [ ] **Step 4: Replace `ipc<ChatMessage>("chat_send", payload)` with `invoke`**

Find the line:

```ts
await ipc<ChatMessage>("chat_send", payload);
```

Replace with:

```ts
await invoke<ChatMessage>("chat_send", payload);
```

- [ ] **Step 5: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useChatSession.ts
git commit -m "feat(desktop-ui/chat): port useChatSession hook

ipc()→invoke(), useQuery()→manual fetch+listen pattern. Inlines
parseApiError to avoid porting the bak utils.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Build useChatThreads (TDD)

**Files:**
- Create: `desktop-ui/src/features/chat/hooks/useChatThreads.ts`
- Create: `desktop-ui/src/features/chat/hooks/useChatThreads.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// desktop-ui/src/features/chat/hooks/useChatThreads.test.ts
import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChatThreads } from "./useChatThreads";

const invokeMock = vi.fn();
const listeners = new Map<string, (event: { payload: unknown }) => void>();
const listenMock = vi.fn(async (event: string, cb: (e: { payload: unknown }) => void) => {
  listeners.set(event, cb);
  return () => listeners.delete(event);
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...(args as Parameters<typeof listenMock>)),
}));

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockClear();
  listeners.clear();
});

afterEach(() => {
  listeners.clear();
});

describe("useChatThreads", () => {
  it("fetches threads on mount", async () => {
    invokeMock.mockResolvedValueOnce([
      { sessionKey: "chat:1", title: "First", messageCount: 1, updatedAt: "2026-04-26" },
    ]);
    const { result } = renderHook(() => useChatThreads());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    expect(invokeMock).toHaveBeenCalledWith("chat_threads");
  });

  it("refetches on chat:thread_created event", async () => {
    invokeMock.mockResolvedValueOnce([]);
    const { result } = renderHook(() => useChatThreads());
    await waitFor(() => expect(result.current.threads).toEqual([]));

    invokeMock.mockResolvedValueOnce([
      { sessionKey: "chat:2", title: "New", messageCount: 1, updatedAt: "2026-04-26" },
    ]);

    await act(async () => {
      const cb = listeners.get("chat:thread_created");
      cb?.({ payload: {} });
    });

    await waitFor(() => expect(result.current.threads).toHaveLength(1));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test -- useChatThreads`
Expected: FAIL — "Cannot find module './useChatThreads'".

- [ ] **Step 3: Implement the hook**

```ts
// desktop-ui/src/features/chat/hooks/useChatThreads.ts
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ChatThread } from "../types";

export interface UseChatThreadsResult {
  threads: ChatThread[];
  refetch: () => Promise<void>;
  error: string | null;
}

export function useChatThreads(): UseChatThreadsResult {
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    try {
      const result = await invoke<ChatThread[]>("chat_threads");
      setThreads(result);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    refetch();
  }, [refetch]);

  useEffect(() => {
    const unsubCreated = listen("chat:thread_created", () => refetch());
    const unsubUpdated = listen("chat:thread_updated", () => refetch());
    return () => {
      unsubCreated.then((fn) => fn()).catch(() => {});
      unsubUpdated.then((fn) => fn()).catch(() => {});
    };
  }, [refetch]);

  return { threads, refetch, error };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd desktop-ui && bun run test -- useChatThreads`
Expected: PASS (both cases).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useChatThreads.ts desktop-ui/src/features/chat/hooks/useChatThreads.test.ts
git commit -m "feat(desktop-ui/chat): add useChatThreads hook

Loads chat_threads on mount; refetches on chat:thread_created and
chat:thread_updated Tauri events.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Build minimal ChatInput

**Files:**
- Create: `desktop-ui/src/features/chat/components/ChatInput.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/chat/components/ChatInput.tsx
import { useEffect, useRef, type KeyboardEvent } from "react";
import SendIcon from "lucide-react/dist/esm/icons/send";

type ChatInputProps = {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  isStreaming: boolean;
  placeholder?: string;
};

export function ChatInput({
  value,
  onChange,
  onSend,
  isStreaming,
  placeholder = "Message Klynt…",
}: ChatInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  // Auto-resize on content change.
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [value]);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!isStreaming && value.trim()) onSend();
    }
  };

  const disabled = isStreaming || !value.trim();

  return (
    <div className="chat-input">
      <textarea
        ref={textareaRef}
        className="chat-input__textarea"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        rows={1}
        disabled={isStreaming}
      />
      <button
        type="button"
        className="chat-input__send"
        onClick={onSend}
        disabled={disabled}
        aria-label="Send message"
      >
        <SendIcon aria-hidden />
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/components/ChatInput.tsx
git commit -m "feat(desktop-ui/chat): add minimal ChatInput component

Auto-resizing textarea, Enter to send, Shift+Enter for newline,
disabled while streaming.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Build MessageBubble

**Files:**
- Create: `desktop-ui/src/features/chat/components/MessageBubble.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/chat/components/MessageBubble.tsx
import { Markdown } from "@/features/messages/components/Markdown";
import type { ChatMessage } from "../types";

type MessageBubbleProps = {
  message: ChatMessage;
};

export function MessageBubble({ message }: MessageBubbleProps) {
  const role = message.role;
  return (
    <div
      className={`chat-bubble chat-bubble--${role}`}
      data-role={role}
    >
      {role === "user" ? (
        <div className="chat-bubble__user-text">{message.content}</div>
      ) : (
        <Markdown value={message.content} className="chat-bubble__markdown" />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/components/MessageBubble.tsx
git commit -m "feat(desktop-ui/chat): add MessageBubble component

User messages render as plain text; assistant messages route
through the existing Markdown component.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Build ChatPanel + smoke test

**Files:**
- Create: `desktop-ui/src/features/chat/components/ChatPanel.tsx`
- Create: `desktop-ui/src/features/chat/components/ChatPanel.test.tsx`
- Create: `desktop-ui/src/styles/chat.css`
- Modify: `desktop-ui/src/styles/index.css` (add `@import "./chat.css";`)

- [ ] **Step 1: Implement ChatPanel**

```tsx
// desktop-ui/src/features/chat/components/ChatPanel.tsx
import { useEffect, useRef } from "react";
import { useChatSession } from "../hooks/useChatSession";
import type { ChatMessage, MessageSegment } from "../types";
import { ChatInput } from "./ChatInput";
import { MessageBubble } from "./MessageBubble";

type ChatPanelProps = {
  sessionKey: string;
  onThreadsChanged: () => void;
};

function segmentsToContent(segments: MessageSegment[]): string {
  return segments
    .filter((s): s is { type: "text"; content: string } => s.type === "text")
    .map((s) => s.content)
    .join("");
}

export function ChatPanel({ sessionKey, onThreadsChanged }: ChatPanelProps) {
  const chat = useChatSession(sessionKey, onThreadsChanged);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // Auto-scroll to bottom on new messages or streaming segments.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [chat.messages, chat.segments]);

  const showEmpty = chat.messages.length === 0 && !chat.isStreaming && chat.segments.length === 0;
  const streamingText = segmentsToContent(chat.segments);

  return (
    <div className="chat-panel">
      <header className="chat-panel__header">
        <span className="chat-panel__title">Chat</span>
      </header>

      <div ref={scrollRef} className="chat-panel__scroll">
        <div className="chat-panel__list">
          {showEmpty && (
            <div className="chat-panel__empty">
              <p>Start a conversation</p>
              <p className="chat-panel__empty-hint">
                Ask Klynt anything about your tasks, projects, or schedule.
              </p>
            </div>
          )}

          {chat.messages.map((m: ChatMessage) => (
            <MessageBubble key={m.id} message={m} />
          ))}

          {chat.isStreaming && streamingText && (
            <MessageBubble
              message={{
                id: "streaming",
                role: "assistant",
                content: streamingText,
              }}
            />
          )}
        </div>
      </div>

      {chat.error && (
        <div className="chat-panel__error" role="alert">
          {chat.error}
        </div>
      )}

      <ChatInput
        value={chat.input}
        onChange={chat.setInput}
        onSend={() => chat.send()}
        isStreaming={chat.isStreaming}
      />
    </div>
  );
}
```

- [ ] **Step 2: Write smoke test**

```tsx
// desktop-ui/src/features/chat/components/ChatPanel.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../hooks/useChatSession", () => ({
  useChatSession: vi.fn(),
}));
vi.mock("@/features/messages/components/Markdown", () => ({
  Markdown: ({ value }: { value: string }) => <div data-testid="md">{value}</div>,
}));

import { useChatSession } from "../hooks/useChatSession";
import { ChatPanel } from "./ChatPanel";

const baseSession = {
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

describe("ChatPanel", () => {
  it("renders empty state when no messages", () => {
    vi.mocked(useChatSession).mockReturnValue(baseSession);
    render(<ChatPanel sessionKey="chat:test" onThreadsChanged={() => {}} />);
    expect(screen.getByText(/start a conversation/i)).toBeInTheDocument();
  });

  it("renders messages when present", () => {
    vi.mocked(useChatSession).mockReturnValue({
      ...baseSession,
      messages: [
        { id: "1", role: "user", content: "hello" },
        { id: "2", role: "assistant", content: "hi there" },
      ],
    });
    render(<ChatPanel sessionKey="chat:test" onThreadsChanged={() => {}} />);
    expect(screen.getByText("hello")).toBeInTheDocument();
    expect(screen.getByText("hi there")).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Write `chat.css`**

```css
/* desktop-ui/src/styles/chat.css */
.chat-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: var(--color-surface-base, #0c0c0e);
  color: var(--color-text-primary, #e7e7ea);
}

.chat-panel__header {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.06));
  font-size: var(--fs-md, 13.5px);
  font-weight: 500;
}

.chat-panel__scroll {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.chat-panel__list {
  max-width: 760px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.chat-panel__empty {
  text-align: center;
  padding: 80px 0;
  color: var(--color-text-muted, rgba(231, 231, 234, 0.5));
}

.chat-panel__empty-hint {
  font-size: var(--fs-xs, 11.5px);
  margin-top: 4px;
  color: var(--color-text-dim, rgba(231, 231, 234, 0.35));
}

.chat-panel__error {
  margin: 0 24px 8px;
  padding: 8px 12px;
  background: rgba(220, 80, 80, 0.08);
  color: rgb(240, 130, 130);
  border-radius: 8px;
  font-size: var(--fs-xs, 11.5px);
}

.chat-bubble {
  display: flex;
  flex-direction: column;
  font-size: var(--fs-base, 12.5px);
  line-height: 1.55;
}

.chat-bubble--user {
  align-items: flex-end;
}

.chat-bubble--user .chat-bubble__user-text {
  background: var(--color-surface-raised, rgba(255, 255, 255, 0.06));
  padding: 10px 14px;
  border-radius: 14px 14px 4px 14px;
  max-width: 80%;
  white-space: pre-wrap;
}

.chat-bubble--assistant {
  align-items: flex-start;
}

.chat-bubble__markdown {
  max-width: 100%;
}

.chat-input {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 12px 24px 16px;
  border-top: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.06));
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
}

.chat-input__textarea {
  flex: 1;
  resize: none;
  background: var(--color-surface-raised, rgba(255, 255, 255, 0.04));
  color: inherit;
  border: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.08));
  border-radius: 12px;
  padding: 10px 14px;
  font-size: var(--fs-base, 12.5px);
  font-family: inherit;
  line-height: 1.45;
  max-height: 240px;
  min-height: 40px;
  outline: none;
}

.chat-input__textarea:focus {
  border-color: var(--color-border-strong, rgba(255, 255, 255, 0.2));
}

.chat-input__send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  border: none;
  background: var(--color-accent, #4d6cff);
  color: white;
  cursor: pointer;
}

.chat-input__send:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
```

If any of those CSS variables don't exist, that's fine — the fallbacks render correctly. Verify visually in Task 15.

- [ ] **Step 4: Add `@import "./chat.css";` to `src/styles/index.css`**

Add after `@import "./composer.css";`:

```css
@import "./chat.css";
```

- [ ] **Step 5: Run tests and typecheck**

Run: `cd desktop-ui && bun run typecheck && bun run test -- ChatPanel`
Expected: typecheck passes; both ChatPanel tests pass.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/chat/components/ChatPanel.tsx desktop-ui/src/features/chat/components/ChatPanel.test.tsx desktop-ui/src/styles/chat.css desktop-ui/src/styles/index.css
git commit -m "feat(desktop-ui/chat): add ChatPanel with smoke tests

Header + scrollable message list + input. Streaming text segments
render as a synthetic 'streaming' assistant bubble below persisted
messages. Auto-scroll on new content. Inline error banner.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Modify SidebarChatLayout for thread list + onNewChat

**Files:**
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx`
- Modify: `desktop-ui/src/styles/sidebar-chat.css` (add styles for thread rows)

- [ ] **Step 1: Update SidebarChatLayout signature and render**

Replace the entire file with:

```tsx
import { memo } from "react";
import {
  Clock,
  FolderPlus,
  LayoutGrid,
  Search,
  Settings,
  SquarePen,
} from "lucide-react";
import type { ChatThread } from "@/features/chat/types";

type SidebarChatLayoutProps = {
  onSelectHome: () => void;
  onOpenSettings: () => void;
  onNewChat: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
};

type NavItem = {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
};

export const SidebarChatLayout = memo(function SidebarChatLayout({
  onSelectHome: _onSelectHome,
  onOpenSettings,
  onNewChat,
  threads,
  selectedSessionKey,
  onSelectThread,
}: SidebarChatLayoutProps) {
  const navItems: NavItem[] = [
    { id: "new-chat", label: "New chat", icon: <SquarePen aria-hidden />, onClick: onNewChat },
    { id: "search", label: "Search", icon: <Search aria-hidden /> },
    { id: "plugins", label: "Plugins", icon: <LayoutGrid aria-hidden /> },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    { id: "project", label: "Project", icon: <FolderPlus aria-hidden /> },
  ];

  return (
    <aside className="sidebar-chat">
      <div className="sidebar-chat__drag-strip" />
      <div className="sidebar-chat__topbar" aria-hidden />

      <nav className="sidebar-chat__nav" aria-label="Primary">
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className="sidebar-chat__nav-item"
            onClick={item.onClick}
          >
            <span className="sidebar-chat__nav-icon">{item.icon}</span>
            <span className="sidebar-chat__nav-label">{item.label}</span>
          </button>
        ))}
      </nav>

      <div className="sidebar-chat__chats">
        <div className="sidebar-chat__section-title">Chats</div>
        {threads.length === 0 ? (
          <div className="sidebar-chat__chats-empty">No chats</div>
        ) : (
          <ul className="sidebar-chat__thread-list">
            {threads.map((t) => (
              <li key={t.sessionKey}>
                <button
                  type="button"
                  className={
                    "sidebar-chat__thread-item" +
                    (t.sessionKey === selectedSessionKey
                      ? " sidebar-chat__thread-item--active"
                      : "")
                  }
                  onClick={() => onSelectThread(t.sessionKey)}
                  title={t.title}
                >
                  {t.title || "Untitled"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="sidebar-chat__spacer" />

      <div className="sidebar-chat__footer">
        <button
          type="button"
          className="sidebar-chat__settings"
          onClick={onOpenSettings}
        >
          <Settings aria-hidden />
          <span>Settings</span>
        </button>
        <button type="button" className="sidebar-chat__upgrade">
          Upgrade
        </button>
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
```

The `onSelectHome` prop is preserved (still passed in by the layout) but unused in v1; renamed to `_onSelectHome` to satisfy `noUnusedParameters`.

- [ ] **Step 2: Add thread-list styles**

Append to `desktop-ui/src/styles/sidebar-chat.css`:

```css
.sidebar-chat__thread-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sidebar-chat__thread-item {
  display: block;
  width: 100%;
  text-align: left;
  background: transparent;
  border: none;
  color: inherit;
  font-size: var(--fs-xs, 11.5px);
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-chat__thread-item:hover {
  background: var(--color-surface-raised, rgba(255, 255, 255, 0.05));
}

.sidebar-chat__thread-item--active {
  background: var(--color-surface-raised, rgba(255, 255, 255, 0.08));
  font-weight: 500;
}
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: errors at the call site in `buildPrimaryNodes.tsx` because we added required props. Those will be fixed in Task 12.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/app/components/SidebarChatLayout.tsx desktop-ui/src/styles/sidebar-chat.css
git commit -m "feat(desktop-ui/app): wire SidebarChatLayout to chat threads + new chat

Adds onNewChat, threads, selectedSessionKey, onSelectThread props.
Replaces 'No chats' placeholder with rendered thread list.
Call site updates land in the layout-types task.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Add chatViewProps to layout types and buildPrimaryNodes

**Files:**
- Modify: `desktop-ui/src/features/layout/hooks/layoutNodes/types.ts`
- Modify: `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx`

- [ ] **Step 1: Extend `LayoutPrimarySurface`**

In `types.ts`, replace the `LayoutPrimarySurface` block (lines 38-51) with:

```ts
export type ChatViewProps = {
  active: boolean;
  sessionKey: string | null;
  onThreadsChanged: () => void;
};

export type SidebarChatProps = {
  onSelectHome: () => void;
  onOpenSettings: () => void;
  onNewChat: () => void;
  threads: import("@/features/chat/types").ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
};

export type LayoutPrimarySurface = {
  sidebarProps: SidebarChatProps;
  messagesProps: ComponentProps<typeof Messages>;
  composerProps: ComponentProps<typeof Composer> | null;
  approvalToastsProps: ComponentProps<typeof ApprovalToasts>;
  updateToastProps: ComponentProps<typeof UpdateToast>;
  errorToastsProps: ComponentProps<typeof ErrorToasts>;
  homeProps: ComponentProps<typeof Home>;
  mainHeaderProps: ComponentProps<typeof MainHeader> | null;
  desktopTopbarProps: {
    showBackToChat: boolean;
    onExitDiff: () => void;
  };
  chatViewProps: ChatViewProps;
};
```

Note: `sidebarProps` was previously typed as `ComponentProps<typeof Sidebar>` (the *old* sidebar). Since we're using `SidebarChatLayout` exclusively in `buildPrimaryNodes`, retype it explicitly. If other code references `sidebarProps` expecting the old shape, those call sites need to be updated too — surface them in Task 13.

Remove the now-unused `import type { Sidebar }` import at the top of `types.ts` if grep shows no other reference.

- [ ] **Step 2: Update `buildPrimaryNodes.tsx`**

Add the chat panel import at the top:

```ts
import { ChatPanel } from "@/features/chat/components/ChatPanel";
```

Replace the body of `buildPrimaryNodes` so the messages and composer slots become conditional:

```tsx
export function buildPrimaryNodes(
  options: PrimaryLayoutNodesOptions,
): PrimaryLayoutNodes {
  const { chatViewProps } = options;
  const chatActive = chatViewProps.active && chatViewProps.sessionKey !== null;

  const sidebarNode = (
    <SidebarChatLayout
      onSelectHome={options.sidebarProps.onSelectHome}
      onOpenSettings={options.sidebarProps.onOpenSettings}
      onNewChat={options.sidebarProps.onNewChat}
      threads={options.sidebarProps.threads}
      selectedSessionKey={options.sidebarProps.selectedSessionKey}
      onSelectThread={options.sidebarProps.onSelectThread}
    />
  );

  const messagesNode = chatActive ? (
    <ChatPanel
      sessionKey={chatViewProps.sessionKey as string}
      onThreadsChanged={chatViewProps.onThreadsChanged}
    />
  ) : (
    <Messages {...options.messagesProps} />
  );

  const composerNode = chatActive
    ? null
    : options.composerProps
      ? <Composer {...options.composerProps} />
      : null;

  // ...keep the rest of the function (approvalToastsNode, updateToastNode, etc.) unchanged
```

Keep the trailing `return { sidebarNode, messagesNode, composerNode, ... }` block as-is.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: errors only in `useMainAppLayoutSurfaces.ts` (where `LayoutPrimarySurface` is built without the new fields). Those are fixed in Task 13.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/layout/hooks/layoutNodes/types.ts desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx
git commit -m "feat(desktop-ui/layout): conditional ChatPanel swap for messages+composer

Adds chatViewProps to LayoutPrimarySurface; when active, ChatPanel
replaces the messagesNode and composerNode is null. Sidebar props
retyped to SidebarChatLayout's required fields.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Wire MainApp state and propagate through layout surfaces

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`
- Modify: `desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts`

- [ ] **Step 1: Add chat state in `MainApp.tsx`**

Find the imports block at the top of `MainApp.tsx` and add:

```ts
import { useChatThreads } from "@/features/chat/hooks/useChatThreads";
```

Inside the `MainApp` component body, near the other top-level `useState` declarations (search for an early `useState`), add:

```ts
const [appView, setAppView] = useState<"home" | "chat">("home");
const [selectedSessionKey, setSelectedSessionKey] = useState<string | null>(null);
const { threads: chatThreads, refetch: refetchChatThreads } = useChatThreads();

const onNewChat = useCallback(() => {
  setSelectedSessionKey(`chat:${crypto.randomUUID()}`);
  setAppView("chat");
}, []);

const onSelectThread = useCallback((sessionKey: string) => {
  setSelectedSessionKey(sessionKey);
  setAppView("chat");
}, []);
```

`useState` and `useCallback` are likely already imported; if not, add them to the existing `react` import.

- [ ] **Step 2: Pass new state into `useMainAppLayoutSurfaces`**

Find the `useMainAppLayoutSurfaces({ ... })` call (around line 1564) and add the new fields to its argument object:

```ts
const layoutSurfaces = useMainAppLayoutSurfaces({
  // ...all existing fields...
  chatView: {
    appView,
    selectedSessionKey,
    onNewChat,
    onSelectThread,
    chatThreads,
    refetchChatThreads,
  },
});
```

- [ ] **Step 3: Update `useMainAppLayoutSurfaces.ts`**

Open the file and find where the function builds the `primary` surface. The hook returns an object containing `primary: LayoutPrimarySurface`. Add a new `chatView` parameter to the hook's options type, then thread it through.

At the top, define:

```ts
import type { ChatThread } from "@/features/chat/types";

type ChatViewInput = {
  appView: "home" | "chat";
  selectedSessionKey: string | null;
  onNewChat: () => void;
  onSelectThread: (sessionKey: string) => void;
  chatThreads: ChatThread[];
  refetchChatThreads: () => Promise<void>;
};
```

Add `chatView: ChatViewInput` to the hook's options interface. In the function body, when building `primary`, set:

```ts
primary: {
  sidebarProps: {
    onSelectHome: existingOnSelectHome,
    onOpenSettings: existingOnOpenSettings,
    onNewChat: chatView.onNewChat,
    threads: chatView.chatThreads,
    selectedSessionKey: chatView.selectedSessionKey,
    onSelectThread: chatView.onSelectThread,
  },
  // ...other existing primary fields unchanged...
  chatViewProps: {
    active: chatView.appView === "chat",
    sessionKey: chatView.selectedSessionKey,
    onThreadsChanged: chatView.refetchChatThreads,
  },
},
```

The exact substitution depends on how `sidebarProps` is currently built — it may currently be a `ComponentProps<typeof Sidebar>` shaped object (old sidebar). Replace whatever's there with the six fields above. Use Read to look at the existing `primary.sidebarProps` construction and adapt.

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: passes. If `sidebarProps` was used elsewhere with the old shape, adjust those call sites.

- [ ] **Step 5: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: passes.

- [ ] **Step 6: Run all tests**

Run: `cd desktop-ui && bun run test`
Expected: passes (or the same baseline failures as before this work).

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/app/components/MainApp.tsx desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts
git commit -m "feat(desktop-ui/app): wire appView+selectedSessionKey into layout

MainApp owns the chat view state and threads query; flows through
useMainAppLayoutSurfaces to buildPrimaryNodes which conditionally
renders ChatPanel.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 14: Manual verification

**Files:** none (verification only)

- [ ] **Step 1: Start the desktop dev environment**

In one terminal:

```bash
cd desktop-ui && bun run dev:vite
```

In another:

```bash
cargo tauri dev
```

- [ ] **Step 2: Walk the verification checklist**

Verify each item from the spec's success criteria:

1. App boots; home view renders.
2. Click "New chat" in the sidebar → ChatPanel appears, empty state visible.
3. Type "hello" → press Enter → user message appears immediately.
4. Streaming response renders incrementally (text grows in place).
5. After completion, the assistant message persists (still visible after a few seconds).
6. Sidebar "Chats" section now shows the new thread (refresh trigger via `chat:thread_created` event).
7. Click "New chat" again → fresh empty panel; previous thread still in sidebar.
8. Click the previous thread in the sidebar → its messages reload.
9. Send another message in the previous thread → continues correctly.
10. Trigger an error: stop the Tauri backend mid-send (close the Rust process). Confirm a red banner appears in the panel.
11. Restart and reload the app. The chat threads still appear in the sidebar; cold start lands on home.

- [ ] **Step 3: Capture issues**

If anything fails, file findings as TODO comments referencing the exact step number, then fix in subsequent tasks (or report back). Do not mark the plan complete with failures.

- [ ] **Step 4: Final commit (if any fix-ups happened)**

```bash
git status
# If clean — done. Otherwise commit fixes with focused messages.
```

---

## Self-review notes

- **Spec coverage:** Tasks 1-13 implement every section of the spec. Task 1 covers the CLAUDE.md update. Task 2 covers the unmocking constraint surfaced after spec writing. Tasks 3-10 build the chat surface. Tasks 11-13 wire it. Task 14 covers the manual verification checklist.
- **Type consistency:** `ChatThread`, `ChatMessage`, `MessageSegment`, `ActiveInteraction` flow consistently from `features/chat/types.ts` through hooks, components, and layout types. `ChatViewProps` and `SidebarChatProps` defined once in `layoutNodes/types.ts`.
- **TDD scope:** `useChatThreads` and `ChatPanel` get tests. Ported code (`chatStreamStore`, `useAgentStream`, `useChatSession`) is not unit-tested per the spec's "trust by parity" call. `ChatInput` and `MessageBubble` are too thin to need their own tests; covered by `ChatPanel`'s smoke tests.
- **Frequent commits:** every task ends in a commit. 13 functional commits + 1 verification commit if needed.
- **No placeholders:** every step contains either exact code or an exact command. Where a step says "find" or "search", it gives the exact pattern to look for.
