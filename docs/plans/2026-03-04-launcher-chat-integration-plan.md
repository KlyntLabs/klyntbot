# Launcher Chat Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add inline chat mode to the Klynt Launcher so users can get AI answers directly in the floating window, with option to expand to the main chat.

**Architecture:** The Launcher component gains a `mode` state (`'command' | 'chat'`). In chat mode, the command list is replaced by a compact message view, tool spinners, interaction cards, and an input bar. Uses the existing `useChatSession` hook with a launcher-specific ephemeral session key. Expand-to-main emits the existing `open-chat` Tauri event with the session key.

**Tech Stack:** React, TypeScript, Tailwind v4, Tauri events, existing chat hooks (`useChatSession`, `useAgentStream`)

---

### Task 1: Add Mode State & Transition Logic to Launcher

**Files:**
- Modify: `desktop-ui/src/components/views/Launcher.tsx`

**Step 1: Add mode state and session key to Launcher**

Add state variables and transition handlers to the existing `Launcher` component. The `initialQuery` captures what the user typed before entering chat mode so it can be sent as the first message.

```tsx
// Add to imports
import { useState, useMemo, useEffect, useCallback, useRef } from 'react';
import { emit } from '@tauri-apps/api/event';

// Add inside Launcher() after existing state:
const [mode, setMode] = useState<'command' | 'chat'>('command');
const [sessionKey, setSessionKey] = useState<string | null>(null);
const [initialQuery, setInitialQuery] = useState<string | null>(null);

const enterChat = useCallback((text: string) => {
  const key = `launcher-${Date.now()}`;
  setSessionKey(key);
  setInitialQuery(text);
  setMode('chat');
  setQuery('');
}, []);

const exitChat = useCallback(() => {
  setMode('command');
  setSessionKey(null);
  setInitialQuery(null);
}, []);

const expandToMain = useCallback(async () => {
  if (!sessionKey) return;
  if (isTauri) {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    await emit('open-chat', { sessionKey });
    const mainWindow = await WebviewWindow.getByLabel('main');
    if (mainWindow) {
      await mainWindow.show();
      await mainWindow.setFocus();
    }
    await getCurrentWindow().hide();
  }
  exitChat();
}, [sessionKey, exitChat]);
```

**Step 2: Wire Enter key to trigger chat mode from "Ask Klynt AI" item**

Update the `handleKeyDown` function and add an `onSelect` handler:

```tsx
// Replace existing handleKeyDown with:
const handleKeyDown = (e: React.KeyboardEvent) => {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    setSelectedIndex(i => Math.min(i + 1, filteredItems.length - (query.trim() ? 0 : 1)));
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    setSelectedIndex(i => Math.max(i - 1, 0));
  } else if (e.key === 'Enter') {
    e.preventDefault();
    // If AI query option is selected (index 0 when query has text)
    if (query.trim() && selectedIndex === 0) {
      enterChat(query.trim());
    }
  } else if (e.key === 'Tab' && query.trim()) {
    e.preventDefault();
    enterChat(query.trim());
  }
};
```

**Step 3: Update the Escape key handler to be mode-aware**

```tsx
// Replace existing useEffect for Escape with:
useEffect(() => {
  if (!isTauri) return;
  const handleGlobalKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      if (mode === 'chat') {
        exitChat();
      } else {
        getCurrentWindow().hide();
      }
    }
  };
  window.addEventListener('keydown', handleGlobalKeyDown);
  return () => window.removeEventListener('keydown', handleGlobalKeyDown);
}, [mode, exitChat]);
```

**Step 4: Add conditional rendering for mode**

Wrap the existing JSX in a mode check. The chat mode content will be added in Task 3.

```tsx
// In the return, wrap the existing content:
return (
  <div className="w-screen text-primary flex justify-center pt-4 px-4">
    <div className="w-full max-w-[660px] rounded-2xl overflow-hidden bg-surface-floating shadow-2xl shadow-black/50 border border-border-subtle">
      {mode === 'command' ? (
        <>
          {/* ...existing Header, Search, Results, Footer JSX... */}
        </>
      ) : (
        <LauncherChat
          sessionKey={sessionKey!}
          initialQuery={initialQuery}
          onBack={exitChat}
          onExpand={expandToMain}
        />
      )}
    </div>
  </div>
);
```

**Step 5: Also wire the onClick on the "Ask Klynt AI" button**

```tsx
// Add onClick to the AI query button (around line 106):
<button
  className={...}
  onMouseEnter={() => setSelectedIndex(0)}
  onClick={() => enterChat(query.trim())}
>
```

**Step 6: Verify**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds (LauncherChat component doesn't exist yet, so temporarily comment out the chat branch or use a placeholder div)

**Step 7: Commit**

```bash
git add desktop-ui/src/components/views/Launcher.tsx
git commit -m "feat(desktop): add mode state and transitions to launcher"
```

---

### Task 2: Create ToolSpinner Component

**Files:**
- Create: `desktop-ui/src/components/chat/ToolSpinner.tsx`

**Step 1: Create the ToolSpinner component**

A compact inline spinner for the launcher chat. Reuses the `toolColor` utility from `SegmentedMessage.tsx`.

```tsx
import { qualifiedToolName } from '../../lib/utils';

// Same color rotation as SegmentedMessage
const TOOL_COLORS = [
  { ring: 'border-brand/60', text: 'text-brand' },
  { ring: 'border-info/60', text: 'text-info' },
  { ring: 'border-purple/60', text: 'text-purple' },
  { ring: 'border-success/60', text: 'text-success' },
] as const;

function toolColor(name: string) {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0;
  return TOOL_COLORS[Math.abs(hash) % TOOL_COLORS.length];
}

interface ToolSpinnerProps {
  tools: string[];
}

export function ToolSpinner({ tools }: ToolSpinnerProps) {
  if (tools.length === 0) return null;

  return (
    <div className="flex flex-col gap-1 py-1">
      {tools.map((name) => {
        const color = toolColor(name);
        return (
          <div key={name} className="flex items-center gap-1.5 text-[11px] font-light">
            <div className={`w-3 h-3 rounded-full border-[1.5px] ${color.ring} border-t-transparent animate-spin`} />
            <span className={color.text}>{name}</span>
            <span className="text-dim">&hellip;</span>
          </div>
        );
      })}
    </div>
  );
}
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add desktop-ui/src/components/chat/ToolSpinner.tsx
git commit -m "feat(desktop): add compact ToolSpinner component for launcher chat"
```

---

### Task 3: Create LauncherChat Component

**Files:**
- Create: `desktop-ui/src/components/views/LauncherChat.tsx`

This is the main chat mode view. It uses `useChatSession` for all state management and renders a compact message list with the existing `MarkdownContent`, `InteractionCard`, and new `ToolSpinner`.

**Step 1: Create the LauncherChat component**

```tsx
import { useEffect, useRef, useCallback } from 'react';
import { ArrowLeft, Sparkles, Send, ArrowUpRight } from 'lucide-react';
import { useChatSession } from '../../hooks/useChatSession';
import { MarkdownContent } from '../chat/MarkdownContent';
import { InteractionCard } from '../chat/InteractionCard';
import { ToolSpinner } from '../chat/ToolSpinner';
import { isTauri } from '../../lib/utils';

interface LauncherChatProps {
  sessionKey: string;
  initialQuery: string | null;
  onBack: () => void;
  onExpand: () => void;
}

export function LauncherChat({ sessionKey, initialQuery, onBack, onExpand }: LauncherChatProps) {
  const chat = useChatSession(sessionKey);
  const endRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const sentInitial = useRef(false);

  // Send initial query once on mount
  useEffect(() => {
    if (initialQuery && !sentInitial.current) {
      sentInitial.current = true;
      chat.setInput(initialQuery);
      // Defer send to next tick so input state is set
      setTimeout(() => chat.send(), 0);
    }
  }, [initialQuery]); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-scroll on new content
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chat.messages.length, chat.segments.length, chat.isStreaming, chat.activeInteraction]);

  // Focus input after streaming completes
  useEffect(() => {
    if (!chat.isStreaming && !chat.activeInteraction) {
      inputRef.current?.focus();
    }
  }, [chat.isStreaming, chat.activeInteraction]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      chat.send();
    }
  }, [chat.send]); // eslint-disable-line react-hooks/exhaustive-deps

  // Handle ⌘/ to expand
  useEffect(() => {
    const handleExpand = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === '/') {
        e.preventDefault();
        onExpand();
      }
    };
    window.addEventListener('keydown', handleExpand);
    return () => window.removeEventListener('keydown', handleExpand);
  }, [onExpand]);

  return (
    <div className="flex flex-col h-full max-h-[568px]">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
        <button
          onClick={onBack}
          className="flex items-center gap-1.5 text-[12px] font-light text-muted hover:text-primary transition-colors"
        >
          <ArrowLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
          Back
        </button>
        <span className="text-[13px] font-light text-primary">Klynt AI</span>
        <button
          onClick={onExpand}
          className="flex items-center gap-1.5 text-[11px] font-light text-muted hover:text-primary transition-colors"
        >
          <ArrowUpRight className="w-3.5 h-3.5" strokeWidth={1.5} />
          Expand
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-4">
        {chat.messages.map((msg) => (
          <div key={msg.id}>
            {msg.role === 'user' ? (
              <div className="flex justify-end">
                <div className="max-w-[85%] rounded-xl px-4 py-2.5 bg-surface-raised">
                  <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
                    {msg.content}
                  </p>
                </div>
              </div>
            ) : msg.role === 'interaction' ? null : (
              <div className="max-w-full">
                <MarkdownContent content={msg.content} />
              </div>
            )}
          </div>
        ))}

        {/* Streaming content */}
        {chat.segments.length > 0 && (
          <div className="max-w-full">
            {chat.segments.map((seg, i) => (
              seg.type === 'text' ? (
                <div key={`text-${i}`} className={chat.isStreaming && i === chat.segments.length - 1 ? 'streaming-cursor' : ''}>
                  <MarkdownContent content={seg.content} />
                </div>
              ) : null // Tool segments are hidden in launcher (spinner shown instead)
            ))}
          </div>
        )}

        {/* Active tool spinners */}
        <ToolSpinner tools={chat.activeTools} />

        {/* Thinking indicator */}
        {chat.isStreaming && chat.segments.length === 0 && chat.activeTools.length === 0 && (
          <div className="flex gap-1">
            <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
            <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
            <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
          </div>
        )}

        {/* Error */}
        {chat.error && (
          <div className="rounded-xl px-4 py-3 bg-destructive/10 border border-destructive/20">
            <p className="text-[12px] font-light text-destructive">{chat.error}</p>
          </div>
        )}

        {/* Interaction card */}
        {chat.activeInteraction && (
          <InteractionCard
            sessionKey={sessionKey}
            requestId={chat.activeInteraction.requestId}
            request={chat.activeInteraction.request}
            onSubmitted={chat.clearInteraction}
          />
        )}

        <div ref={endRef} />
      </div>

      {/* Input */}
      <div className="px-4 pb-3">
        <div className="flex items-center gap-3 bg-surface-base rounded-xl px-4 py-2.5">
          <Sparkles className="w-[16px] h-[16px] text-brand shrink-0" strokeWidth={1.5} />
          <textarea
            ref={inputRef}
            value={chat.input}
            onChange={(e) => chat.setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Follow up..."
            rows={1}
            className="flex-1 bg-transparent text-primary text-[13px] placeholder:text-muted outline-none font-light resize-none max-h-[80px]"
          />
          <button
            onClick={() => chat.send()}
            disabled={!chat.input.trim() || chat.isStreaming}
            className="text-brand hover:text-brand/80 disabled:text-muted transition-colors shrink-0"
          >
            <Send className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Footer */}
      <div className="px-5 py-2.5 border-t border-border-subtle">
        <div className="flex items-center justify-between text-[11px] text-muted">
          <span className="flex items-center gap-1.5 font-light">
            <kbd className="px-1.5 py-0.5 bg-surface-highest rounded">Esc</kbd>
            Back to commands
          </span>
          <span className="flex items-center gap-1.5 font-light">
            <kbd className="px-1.5 py-0.5 bg-surface-highest rounded">⌘/</kbd>
            Open full chat
          </span>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add desktop-ui/src/components/views/LauncherChat.tsx
git commit -m "feat(desktop): add LauncherChat component with streaming, tools, interactions"
```

---

### Task 4: Wire LauncherChat into Launcher & Handle Initial Send

**Files:**
- Modify: `desktop-ui/src/components/views/Launcher.tsx`

**Step 1: Import and render LauncherChat**

```tsx
// Add import at top of Launcher.tsx:
import { LauncherChat } from './LauncherChat';

// Ensure the chat branch in the mode conditional renders LauncherChat:
{mode === 'chat' && sessionKey && (
  <LauncherChat
    sessionKey={sessionKey}
    initialQuery={initialQuery}
    onBack={exitChat}
    onExpand={expandToMain}
  />
)}
```

**Step 2: Verify full flow works**

Run: `cd desktop-ui && bun run dev`
Expected:
1. Launcher shows command mode
2. Type a query → "Ask Klynt AI" item appears
3. Press Enter → switches to chat mode
4. Message is sent and response streams in
5. Esc → returns to command mode
6. Verify ⌘/ would trigger expand (check console for emit)

**Step 3: Commit**

```bash
git add desktop-ui/src/components/views/Launcher.tsx
git commit -m "feat(desktop): wire LauncherChat into launcher with mode switching"
```

---

### Task 5: Update MainApp to Handle Session Key from Launcher

**Files:**
- Modify: `desktop-ui/src/components/views/MainApp.tsx`

**Step 1: Update the open-chat event handler to accept a sessionKey**

The current handler only opens chat generically. Update it to navigate to a specific session when provided.

```tsx
// Replace the existing open-chat listener (line 86-90) with:
useEvent<{ text?: string; sessionKey?: string }>('open-chat', (payload) => {
  setIsChatOpen(true);
  setActiveSidebar('Chat');
  if (payload?.sessionKey) {
    // SidebarChat will need to accept this — pass via state or ref
    setOpenSessionKey(payload.sessionKey);
  }
});
```

Add the state:
```tsx
const [openSessionKey, setOpenSessionKey] = useState<string | null>(null);
```

Pass to SidebarChat:
```tsx
<SidebarChat
  viewContext={sidebarViewContext}
  sessionKey={openSessionKey}
  onSessionKeyUsed={() => setOpenSessionKey(null)}
/>
```

**Note:** The SidebarChat component may need a small update to accept and navigate to a specific session key. Check the SidebarChat props and update accordingly — the pattern should be: if `sessionKey` prop is set, switch to that thread, then call `onSessionKeyUsed()` to clear it.

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add desktop-ui/src/components/views/MainApp.tsx
git commit -m "feat(desktop): handle sessionKey in open-chat event for launcher expand"
```

---

### Task 6: Update SidebarChat to Accept External Session Key

**Files:**
- Modify: `desktop-ui/src/components/chat/SidebarChat.tsx`

**Step 1: Read the current SidebarChat implementation**

Read `desktop-ui/src/components/chat/SidebarChat.tsx` to understand how it manages session selection. Add a prop for `sessionKey` and `onSessionKeyUsed`.

**Step 2: Add prop handling**

When `sessionKey` prop changes to a non-null value:
1. Set the active thread to that session key
2. Call `onSessionKeyUsed()` to clear the prop
3. The existing `useChatSession(activeSessionKey)` picks up the thread

This is a small change — likely just a `useEffect` watching the prop.

```tsx
// Add to SidebarChat props:
interface SidebarChatProps {
  viewContext: SessionContext;
  sessionKey?: string | null;
  onSessionKeyUsed?: () => void;
}

// Add useEffect inside SidebarChat:
useEffect(() => {
  if (sessionKey) {
    setActiveThread(sessionKey);
    onSessionKeyUsed?.();
  }
}, [sessionKey, onSessionKeyUsed]);
```

**Step 3: Verify end-to-end expand flow**

Run: `cd desktop-ui && bun run dev`
Expected:
1. Chat in launcher → click Expand
2. Launcher hides
3. Main window opens with chat sidebar showing the same conversation

**Step 4: Commit**

```bash
git add desktop-ui/src/components/chat/SidebarChat.tsx
git commit -m "feat(desktop): accept external sessionKey in SidebarChat for launcher expand"
```

---

### Task 7: Handle Dismiss-on-Blur with Session Persistence

**Files:**
- Modify: `desktop-ui/src/components/views/Launcher.tsx`

**Step 1: Ensure mode persists across hide/show**

The current `dismiss_on_blur` in Rust hides the window but doesn't reset React state. Verify this works correctly — the state should persist because the React component stays mounted (window is hidden, not destroyed).

If the window unmounts on hide (check by adding a console log in useEffect cleanup), we need to lift state to a context or use sessionStorage. Most likely it persists since Tauri just hides the webview.

**Step 2: Add a reset when launcher window is re-shown (if needed)**

Listen for the Tauri window focus event to refocus the input:

```tsx
// Add to Launcher:
useEffect(() => {
  if (!isTauri) return;
  let cleanup: (() => void) | undefined;

  getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused && mode === 'chat') {
      // Re-focus input when launcher regains focus
      // LauncherChat handles this internally via inputRef
    }
  }).then(unlisten => { cleanup = unlisten; });

  return () => cleanup?.();
}, [mode]);
```

**Step 3: Verify**

Run: `cd desktop-ui && bun run dev`
Expected:
1. Enter chat mode in launcher
2. Click outside (launcher hides)
3. Alt+Space (launcher reappears in chat mode with conversation intact)
4. Esc → returns to command mode

**Step 4: Commit**

```bash
git add desktop-ui/src/components/views/Launcher.tsx
git commit -m "feat(desktop): preserve chat session across launcher hide/show"
```

---

### Task 8: Final Polish & Edge Cases

**Files:**
- Modify: `desktop-ui/src/components/views/Launcher.tsx`
- Modify: `desktop-ui/src/components/views/LauncherChat.tsx`

**Step 1: Handle initial send correctly**

The `initialQuery` flow in LauncherChat uses a `setTimeout` to defer `send()`. Verify this works reliably with the `useChatSession` hook. If `chat.send()` doesn't pick up the input set via `setInput`, adjust to use `send` with `extraPayload` or pass content directly.

Alternative approach if `setInput` + deferred `send()` is unreliable:

```tsx
// In LauncherChat, instead of using setInput + send(), call the IPC directly:
useEffect(() => {
  if (initialQuery && !sentInitial.current) {
    sentInitial.current = true;
    chat.setInput(''); // Keep input clear
    // Manually trigger via the hook's send, with input pre-set
    chat.setInput(initialQuery);
    requestAnimationFrame(() => chat.send());
  }
}, [initialQuery]);
```

**Step 2: Textarea auto-height**

Add auto-height behavior to the LauncherChat input (match the main chat pattern):

```tsx
// In the textarea onChange:
onChange={(e) => {
  chat.setInput(e.target.value);
  e.target.style.height = 'auto';
  e.target.style.height = `${Math.min(e.target.scrollHeight, 80)}px`;
}}
```

**Step 3: Verify complete flow**

Run: `cd desktop-ui && bun run dev`
Test scenarios:
1. Type query → Enter → response streams → follow up → works
2. Tool calls show spinner → spinner disappears → text response shown
3. Interaction card appears → select option → submit → works
4. Esc → back to commands → re-type → new chat session
5. ⌘/ → main window opens with conversation
6. Click outside → re-open → conversation persists
7. Type query → Tab → enters chat mode

**Step 4: Commit**

```bash
git add desktop-ui/src/components/views/Launcher.tsx desktop-ui/src/components/views/LauncherChat.tsx
git commit -m "feat(desktop): polish launcher chat with auto-height input and edge cases"
```

---

### Task 9: Build Verification

**Step 1: Run full build**

```bash
cd desktop-ui && bun run build
```
Expected: No errors, no warnings

**Step 2: Run lint**

```bash
cd desktop-ui && bun run lint 2>&1 || true
```
Expected: No new lint errors

**Step 3: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(desktop): address lint and build issues in launcher chat"
```
