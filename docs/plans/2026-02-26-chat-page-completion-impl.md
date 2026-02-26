# Chat Page Completion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete all Chat page features with real API integration — no mocks, no TODOs.

**Architecture:** Enhance existing `useAgent` hook with session loading + interaction handling. Update `Chat.tsx` with markdown rendering, live sidebar data, session management. Install `marked` for markdown. All changes follow existing single-file-per-page convention.

**Tech Stack:** React 19, TypeScript, motion/react, lucide-react, marked (new), existing `apiFetch`/`useApi` utilities.

---

### Task 1: Install markdown dependency

**Files:**
- Modify: `crates/dashboard/frontend/package.json`

**Step 1: Install marked**

Run: `cd crates/dashboard/frontend && npm install marked`

Marked is a lightweight CommonMark parser (no React wrapper needed). We'll use `dangerouslySetInnerHTML` with manual sanitization of script tags.

**Step 2: Verify installation**

Run: `cd crates/dashboard/frontend && node -e "const { marked } = require('marked'); console.log('ok')"`
Expected: `ok`

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/package.json crates/dashboard/frontend/package-lock.json
git commit -m "feat(dashboard): add marked dependency for chat markdown rendering"
```

---

### Task 2: Add session loading + interaction state to useAgent hook

**Files:**
- Modify: `crates/dashboard/frontend/src/lib/hooks/useAgent.ts`

**Step 1: Add new imports and types to useAgent.ts**

At top of file, add import for `apiFetch`:

```typescript
import { apiFetch } from '../api';
import type { SessionWithMessages, InteractionRequestEvent, InteractionQuestion } from '../types';
```

Add `PendingInteraction` interface after `ThinkingState`:

```typescript
export interface PendingInteraction {
  requestId: string;
  title: string;
  questions: InteractionQuestion[];
}
```

Update `UseAgentResult` to add new fields:

```typescript
export interface UseAgentResult {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  sessionKey: string | null;
  pendingInteraction: PendingInteraction | null;
  sendMessage: (text: string, sessionKey?: string) => void;
  cancel: () => void;
  loadSession: (key: string) => Promise<void>;
  newSession: () => void;
  deleteSession: (key: string) => Promise<void>;
  respondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
}
```

**Step 2: Add session key state + sessionStorage persistence**

Inside `useAgent()`, add:

```typescript
const [sessionKey, setSessionKey] = useState<string | null>(() => {
  return sessionStorage.getItem('klyntbot-session-key');
});
const [pendingInteraction, setPendingInteraction] = useState<PendingInteraction | null>(null);
```

Add a `useEffect` to persist sessionKey:

```typescript
useEffect(() => {
  if (sessionKey) {
    sessionStorage.setItem('klyntbot-session-key', sessionKey);
  } else {
    sessionStorage.removeItem('klyntbot-session-key');
  }
}, [sessionKey]);
```

**Step 3: Add interaction.request handler in the onMessage switch**

In the `switch (event.type)` block, before the `default:` case, add:

```typescript
case 'interaction.request': {
  setPendingInteraction({
    requestId: event.requestId as string,
    title: event.title as string,
    questions: event.questions as InteractionQuestion[],
  });
  break;
}
```

**Step 4: Track session key from chat.send**

In `sendMessage`, after `socketRef.current.sendChatMessage(sessionKey, text)`, capture the session key if provided. Update the `sendMessage` callback:

```typescript
const sendMessage = useCallback(
  (text: string, overrideSessionKey?: string) => {
    if (isStreamingRef.current) return;
    if (!socketRef.current) return;

    // Add user message optimistically
    setMessages((prev) => [
      ...prev,
      {
        id: generateId(),
        role: 'user',
        content: text,
        timestamp: new Date(),
      },
    ]);

    isStreamingRef.current = true;
    setIsStreaming(true);
    accumulatedContentRef.current = '';
    streamingMessageIdRef.current = null;

    const key = overrideSessionKey ?? sessionKey ?? undefined;
    socketRef.current.sendChatMessage(key, text);
  },
  [sessionKey],
);
```

**Step 5: Add loadSession method**

After `sendMessage`, add:

```typescript
const loadSession = useCallback(async (key: string) => {
  try {
    const data = await apiFetch<SessionWithMessages>(`/api/sessions/${key}`);
    const loaded: ChatMessage[] = data.messages.map((m) => ({
      id: m.id,
      role: m.role as ChatMessage['role'],
      content: m.content,
      timestamp: new Date(m.timestamp),
    }));
    setMessages(loaded);
    setSessionKey(key);
    setThinking(null);
    setPendingInteraction(null);
    isStreamingRef.current = false;
    setIsStreaming(false);
    streamingMessageIdRef.current = null;
    accumulatedContentRef.current = '';
  } catch (err) {
    setMessages((prev) => [
      ...prev,
      {
        id: generateId(),
        role: 'system',
        content: `Failed to load session: ${err instanceof Error ? err.message : 'Unknown error'}`,
        timestamp: new Date(),
      },
    ]);
  }
}, []);
```

**Step 6: Add newSession method**

```typescript
const newSession = useCallback(() => {
  setMessages([]);
  setSessionKey(null);
  setThinking(null);
  setPendingInteraction(null);
  isStreamingRef.current = false;
  setIsStreaming(false);
  streamingMessageIdRef.current = null;
  accumulatedContentRef.current = '';
}, []);
```

**Step 7: Add deleteSession method**

```typescript
const deleteSession = useCallback(async (key: string) => {
  await apiFetch(`/api/sessions/${key}`, { method: 'DELETE' });
  // If we deleted the active session, clear it
  if (key === sessionKey) {
    newSession();
  }
}, [sessionKey, newSession]);
```

**Step 8: Add respondToInteraction method**

```typescript
const respondToInteraction = useCallback(
  (requestId: string, response: Record<string, unknown>) => {
    socketRef.current?.sendInteractionResponse(requestId, response);
    setPendingInteraction(null);
  },
  [],
);
```

**Step 9: Update return statement**

```typescript
return {
  messages, thinking, isStreaming, status, sessionKey, pendingInteraction,
  sendMessage, cancel, loadSession, newSession, deleteSession, respondToInteraction,
};
```

**Step 10: Restore session on mount**

Add effect after the WebSocket setup effect to auto-load the persisted session:

```typescript
// Auto-load persisted session on mount
useEffect(() => {
  const savedKey = sessionStorage.getItem('klyntbot-session-key');
  if (savedKey) {
    loadSession(savedKey);
  }
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, []);
```

**Step 11: Commit**

```bash
git add crates/dashboard/frontend/src/lib/hooks/useAgent.ts
git commit -m "feat(dashboard): add session loading, interaction handling, and session management to useAgent"
```

---

### Task 3: Add markdown rendering + interaction panel + sidebar data to Chat.tsx

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Chat.tsx`

This is the largest task. It modifies Chat.tsx in several places.

**Step 1: Update imports**

Replace the imports section at the top of Chat.tsx:

```typescript
import { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import {
  Send,
  ChevronDown,
  ChevronRight,
  Terminal,
  Sparkles,
  Code,
  Lightbulb,
  FileCode,
  Loader2,
  Check,
  Slash,
  X,
  Wifi,
  WifiOff,
  Clock,
  Plus,
  Trash2,
  Calendar,
  ListTodo,
  Brain,
  MessageSquare,
  Circle,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { marked } from 'marked';
import { useAgent } from '../../lib/hooks/useAgent';
import type { ThinkingState, ToolCallState, PendingInteraction } from '../../lib/hooks/useAgent';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type {
  SessionListItem,
  ChatMessage,
  Task,
  CalendarEvent,
  InteractionQuestion,
} from '../../lib/types';
import type { ConnectionStatus } from '../../lib/ws';
```

**Step 2: Configure marked + add renderMarkdown helper**

After the imports, before `phaseLabel`, add:

```typescript
// Configure marked for safe rendering
marked.setOptions({
  gfm: true,
  breaks: true,
});

/** Render markdown to sanitized HTML */
function renderMarkdown(content: string): string {
  const html = marked.parse(content, { async: false }) as string;
  // Basic sanitization: strip script tags and event handlers
  return html
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '')
    .replace(/on\w+="[^"]*"/gi, '')
    .replace(/on\w+='[^']*'/gi, '');
}

/** Format relative time like "in 2h", "tomorrow", "in 3d" */
function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  const target = new Date(dateStr).getTime();
  const diffMs = target - now;
  if (diffMs < 0) return 'past';
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 60) return `in ${diffMin}m`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `in ${diffHours}h`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays === 1) return 'tomorrow';
  return `in ${diffDays}d`;
}

/** Priority label + color */
function priorityDisplay(p: number | null): { label: string; color: string } {
  switch (p) {
    case 1: return { label: 'P1', color: '#ef4444' };
    case 2: return { label: 'P2', color: '#f59e0b' };
    case 3: return { label: 'P3', color: 'var(--codex-accent)' };
    case 4: return { label: 'P4', color: 'var(--codex-fg-subtle)' };
    default: return { label: '--', color: 'var(--codex-fg-subtle)' };
  }
}
```

**Step 3: Update the Chat component destructuring**

Replace line 103-104:

```typescript
const {
  messages, thinking, isStreaming, status, sessionKey, pendingInteraction,
  sendMessage, cancel, loadSession, newSession, deleteSession, respondToInteraction,
} = useAgent();
```

**Step 4: Add sidebar data hooks**

After the sessions `useApi` call, add:

```typescript
// Tasks for sidebar
const { data: tasks } = useApi<Task[]>('/api/tasks');

// Calendar events for sidebar
const { data: calendarEvents } = useApi<CalendarEvent[]>('/api/calendar/events', {
  params: { limit: 5 },
});

// Filtered pending tasks (top 5)
const pendingTasks = useMemo(() => {
  if (!tasks) return [];
  return tasks
    .filter((t) => t.status === 'todo' || t.status === 'doing')
    .sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99))
    .slice(0, 5);
}, [tasks]);

// Session stats
const sessionStats = useMemo(() => {
  const userMsgs = messages.filter((m) => m.role === 'user').length;
  const assistantMsgs = messages.filter((m) => m.role === 'assistant').length;
  return { userMsgs, assistantMsgs, total: messages.length };
}, [messages]);
```

**Step 5: Remove the `selectedModel` state**

Delete this line:

```typescript
const [selectedModel] = useState('GPT-4');
```

**Step 6: Add handleDeleteSession callback**

After `handleSuggestionClick`, add:

```typescript
const handleDeleteSession = useCallback(
  async (e: React.MouseEvent, key: string) => {
    e.stopPropagation();
    await deleteSession(key);
    // Optimistically remove from session list
    if (sessions) {
      // refetch will happen naturally, but we can trigger it
    }
  },
  [deleteSession, sessions],
);
```

**Step 7: Replace the model selector in the input bar with New Chat button**

Replace lines 367-385 (the slash + model selector + divider) with:

```typescript
<button
  onClick={newSession}
  className="flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors"
  style={{ color: 'var(--codex-fg-subtle)' }}
  onMouseEnter={(e) => {
    e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
    e.currentTarget.style.color = 'var(--codex-fg)';
  }}
  onMouseLeave={(e) => {
    e.currentTarget.style.backgroundColor = 'transparent';
    e.currentTarget.style.color = 'var(--codex-fg-subtle)';
  }}
  title="New conversation"
>
  <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
  <span>New</span>
</button>

<div
  className="w-px h-5"
  style={{ backgroundColor: 'var(--codex-border)' }}
/>
```

**Step 8: Add InteractionPanel after thinking indicator**

Inside the messages area, after the `<AnimatePresence>` block for thinking (after line 318), add:

```typescript
{/* Interaction request panel */}
<AnimatePresence>
  {pendingInteraction && (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -5 }}
    >
      <InteractionPanel
        interaction={pendingInteraction}
        onRespond={respondToInteraction}
      />
    </motion.div>
  )}
</AnimatePresence>
```

**Step 9: Update MessageBubble to render markdown**

Replace the assistant content rendering (the `whitespace-pre-wrap` div inside the assistant branch) with:

```typescript
{msg.role === 'assistant' && (
  <div className="flex gap-3">
    <div className="flex-1">
      <div
        className="text-[14px] leading-relaxed prose-chat"
        style={{ color: 'var(--codex-fg)' }}
        dangerouslySetInnerHTML={{ __html: renderMarkdown(msg.content) }}
      />
      {msg.isStreaming && (
        <span
          className="inline-block w-[2px] h-[14px] ml-0.5 align-text-bottom animate-pulse"
          style={{ backgroundColor: 'var(--codex-accent)' }}
        />
      )}
      <div className="flex items-center gap-2 mt-2">
        <div
          className="text-[11px]"
          style={{
            color: 'var(--codex-fg-subtle)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {formatTime(msg.timestamp)}
        </div>
      </div>
    </div>
  </div>
)}
```

**Step 10: Replace Memory Context sidebar section**

Replace the placeholder `--` in Memory Context (lines 620-633) with:

```typescript
<SidebarSection
  title="Session Context"
  open={memoryOpen}
  onToggle={() => setMemoryOpen(!memoryOpen)}
>
  <div className="px-4 pb-4 space-y-3 text-[13px]">
    {sessionKey && (
      <SidebarRow label="Session">
        <span
          style={{
            color: 'var(--codex-fg)',
            fontFamily: 'var(--font-mono)',
            fontSize: '11px',
          }}
        >
          #{sessionKey.slice(0, 8)}
        </span>
      </SidebarRow>
    )}
    <SidebarRow label="You">
      <span style={{ color: 'var(--codex-fg)' }}>
        {sessionStats.userMsgs}
      </span>
    </SidebarRow>
    <SidebarRow label="Assistant">
      <span style={{ color: 'var(--codex-fg)' }}>
        {sessionStats.assistantMsgs}
      </span>
    </SidebarRow>
    <SidebarRow label="Total">
      <span style={{ color: 'var(--codex-fg)' }}>
        {sessionStats.total}
      </span>
    </SidebarRow>
  </div>
</SidebarSection>
```

**Step 11: Replace Quick Tasks sidebar section**

Replace the placeholder `--` in Quick Tasks (lines 635-649) with:

```typescript
<SidebarSection
  title="Quick Tasks"
  open={tasksOpen}
  onToggle={() => setTasksOpen(!tasksOpen)}
>
  <div className="px-4 pb-4 space-y-2">
    {pendingTasks.length === 0 && (
      <div
        className="text-[12px]"
        style={{ color: 'var(--codex-fg-subtle)' }}
      >
        No pending tasks
      </div>
    )}
    {pendingTasks.map((task) => {
      const p = priorityDisplay(task.priority);
      return (
        <div
          key={task.id}
          className="flex items-center gap-2 p-1.5 rounded cursor-default"
          style={{ backgroundColor: 'transparent' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = 'transparent';
          }}
        >
          <Circle
            className="w-3 h-3 flex-shrink-0"
            strokeWidth={1.5}
            style={{
              color: task.status === 'doing'
                ? 'var(--codex-accent)'
                : 'var(--codex-fg-subtle)',
            }}
          />
          <span
            className="text-[12px] truncate flex-1"
            style={{ color: 'var(--codex-fg)' }}
          >
            {task.title}
          </span>
          <span
            className="text-[10px] flex-shrink-0"
            style={{
              color: p.color,
              fontFamily: 'var(--font-mono)',
              fontWeight: 500,
            }}
          >
            {p.label}
          </span>
        </div>
      );
    })}
  </div>
</SidebarSection>
```

**Step 12: Replace Upcoming/Calendar sidebar section**

Replace the placeholder `--` in Upcoming (lines 651-666) with:

```typescript
<SidebarSection
  title="Upcoming"
  open={calendarOpen}
  onToggle={() => setCalendarOpen(!calendarOpen)}
  noBorder
>
  <div className="px-4 pb-4 space-y-2">
    {(!calendarEvents || calendarEvents.length === 0) && (
      <div
        className="text-[12px]"
        style={{ color: 'var(--codex-fg-subtle)' }}
      >
        No upcoming events
      </div>
    )}
    {calendarEvents?.slice(0, 5).map((event) => (
      <div
        key={event.uid}
        className="flex items-center gap-2 p-1.5 rounded"
        style={{ backgroundColor: 'transparent' }}
        onMouseEnter={(e) => {
          e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.backgroundColor = 'transparent';
        }}
      >
        <Calendar
          className="w-3 h-3 flex-shrink-0"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[12px] truncate flex-1"
          style={{ color: 'var(--codex-fg)' }}
        >
          {event.summary}
        </span>
        <span
          className="text-[10px] flex-shrink-0"
          style={{
            color: 'var(--codex-fg-subtle)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {formatRelativeTime(event.startAt)}
        </span>
      </div>
    ))}
  </div>
</SidebarSection>
```

**Step 13: Wire session click + delete in Recent Sessions**

Replace the session button (lines 578-614) with:

```typescript
{sortedSessions.slice(0, 10).map((session) => (
  <div
    key={session.key}
    className="flex items-center gap-1 group"
  >
    <button
      onClick={() => loadSession(session.key)}
      className="flex-1 text-left p-2 rounded transition-colors"
      style={{
        backgroundColor: session.key === sessionKey
          ? 'var(--codex-bg)'
          : 'transparent',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.backgroundColor =
          session.key === sessionKey ? 'var(--codex-bg)' : 'transparent';
      }}
    >
      <div className="flex items-center justify-between mb-1">
        <span
          className="text-[11px]"
          style={{
            color: 'var(--codex-fg)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          #{session.key.slice(0, 8)}
        </span>
        <span
          className="text-[10px]"
          style={{ color: 'var(--codex-fg-subtle)' }}
        >
          {session.messageCount} msgs
        </span>
      </div>
      <div className="flex items-center gap-1.5 text-[10px]" style={{ color: '#888' }}>
        <Clock className="w-2.5 h-2.5" strokeWidth={1.5} />
        {formatDuration(session.createdAt, session.updatedAt)}
      </div>
    </button>
    <button
      onClick={(e) => handleDeleteSession(e, session.key)}
      className="p-1 rounded opacity-0 group-hover:opacity-100 transition-opacity"
      style={{ color: 'var(--codex-fg-subtle)' }}
      onMouseEnter={(e) => {
        e.currentTarget.style.color = '#ef4444';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.color = 'var(--codex-fg-subtle)';
      }}
      title="Delete session"
    >
      <Trash2 className="w-3 h-3" strokeWidth={1.5} />
    </button>
  </div>
))}
```

**Step 14: Commit**

```bash
git add crates/dashboard/frontend/src/app/pages/Chat.tsx
git commit -m "feat(dashboard): complete Chat page with markdown, sidebar data, sessions, and interaction UI"
```

---

### Task 4: Add InteractionPanel component to Chat.tsx

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Chat.tsx`

Add before the `SidebarSection` component (around line 1000):

**Step 1: Write InteractionPanel component**

```typescript
/* ── Interaction request panel ──────────────────────────────────────────── */

function InteractionPanel({
  interaction,
  onRespond,
}: {
  interaction: PendingInteraction;
  onRespond: (requestId: string, response: Record<string, unknown>) => void;
}) {
  const [answers, setAnswers] = useState<Record<number, unknown>>({});

  const handleSubmit = useCallback(() => {
    const response: Record<string, unknown> = {};
    interaction.questions.forEach((q, idx) => {
      response[q.label] = answers[idx] ?? (q.type === 'yesNo' ? (q.default ?? false) : '');
    });
    onRespond(interaction.requestId, response);
  }, [interaction, answers, onRespond]);

  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-accent)',
      }}
    >
      <div
        className="px-4 py-3 flex items-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderBottom: '1px solid var(--codex-border)',
        }}
      >
        <MessageSquare
          className="w-4 h-4"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[13px]"
          style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
        >
          {interaction.title}
        </span>
      </div>

      <div className="px-4 py-3 space-y-4">
        {interaction.questions.map((q, idx) => (
          <InteractionField
            key={idx}
            question={q}
            value={answers[idx]}
            onChange={(val) => setAnswers((prev) => ({ ...prev, [idx]: val }))}
          />
        ))}

        <button
          onClick={handleSubmit}
          className="w-full py-2 rounded-md text-[13px] transition-colors"
          style={{
            backgroundColor: 'var(--codex-accent)',
            color: '#000',
            fontWeight: 500,
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.opacity = '0.9';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.opacity = '1';
          }}
        >
          Submit
        </button>
      </div>
    </div>
  );
}

function InteractionField({
  question,
  value,
  onChange,
}: {
  question: InteractionQuestion;
  value: unknown;
  onChange: (val: unknown) => void;
}) {
  switch (question.type) {
    case 'freeText':
      return (
        <div>
          <label
            className="block text-[12px] mb-1.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {question.label}
          </label>
          <input
            type="text"
            value={(value as string) ?? ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={question.placeholder ?? ''}
            className="w-full px-3 py-2 rounded-md text-[13px] outline-none"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg)',
            }}
          />
        </div>
      );

    case 'yesNo':
      return (
        <div>
          <label
            className="block text-[12px] mb-1.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {question.label}
          </label>
          <div className="flex gap-2">
            {['Yes', 'No'].map((opt) => {
              const selected =
                (opt === 'Yes' && value === true) ||
                (opt === 'No' && value === false);
              return (
                <button
                  key={opt}
                  onClick={() => onChange(opt === 'Yes')}
                  className="flex-1 py-1.5 rounded-md text-[12px] transition-colors"
                  style={{
                    backgroundColor: selected
                      ? 'var(--codex-accent)'
                      : 'var(--codex-bg-tertiary)',
                    color: selected ? '#000' : 'var(--codex-fg)',
                    border: `1px solid ${selected ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                    fontWeight: selected ? 500 : 400,
                  }}
                >
                  {opt}
                </button>
              );
            })}
          </div>
        </div>
      );

    case 'singleSelect':
      return (
        <div>
          <label
            className="block text-[12px] mb-1.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {question.label}
          </label>
          <div className="space-y-1">
            {question.options.map((opt) => (
              <button
                key={opt}
                onClick={() => onChange(opt)}
                className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors"
                style={{
                  backgroundColor:
                    value === opt
                      ? 'var(--codex-accent-dim)'
                      : 'var(--codex-bg-tertiary)',
                  color:
                    value === opt
                      ? 'var(--codex-accent)'
                      : 'var(--codex-fg)',
                  border: `1px solid ${value === opt ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                }}
              >
                {opt}
              </button>
            ))}
          </div>
        </div>
      );

    case 'multiSelect':
      return (
        <div>
          <label
            className="block text-[12px] mb-1.5"
            style={{ color: 'var(--codex-fg-subtle)' }}
          >
            {question.label}
          </label>
          <div className="space-y-1">
            {question.options.map((opt) => {
              const selected = Array.isArray(value) && (value as string[]).includes(opt);
              return (
                <button
                  key={opt}
                  onClick={() => {
                    const current = (Array.isArray(value) ? value : []) as string[];
                    onChange(
                      selected
                        ? current.filter((v) => v !== opt)
                        : [...current, opt],
                    );
                  }}
                  className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors flex items-center gap-2"
                  style={{
                    backgroundColor: selected
                      ? 'var(--codex-accent-dim)'
                      : 'var(--codex-bg-tertiary)',
                    color: selected
                      ? 'var(--codex-accent)'
                      : 'var(--codex-fg)',
                    border: `1px solid ${selected ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                  }}
                >
                  <div
                    className="w-3 h-3 rounded-sm border flex items-center justify-center"
                    style={{
                      borderColor: selected
                        ? 'var(--codex-accent)'
                        : 'var(--codex-border)',
                      backgroundColor: selected
                        ? 'var(--codex-accent)'
                        : 'transparent',
                    }}
                  >
                    {selected && (
                      <Check
                        className="w-2 h-2"
                        strokeWidth={3}
                        style={{ color: '#000' }}
                      />
                    )}
                  </div>
                  {opt}
                </button>
              );
            })}
          </div>
        </div>
      );
  }
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/pages/Chat.tsx
git commit -m "feat(dashboard): add InteractionPanel component for ask_user prompts"
```

---

### Task 5: Add markdown CSS styles

**Files:**
- Modify: `crates/dashboard/frontend/src/index.css` (or wherever global styles live)

**Step 1: Find the CSS file**

Check for the global CSS file location.

**Step 2: Add prose-chat styles**

Append these styles for markdown rendering in chat messages:

```css
/* ── Chat markdown styles ──────────────────────────────────────────────── */

.prose-chat {
  word-wrap: break-word;
  overflow-wrap: break-word;
}

.prose-chat p {
  margin: 0.5em 0;
}

.prose-chat p:first-child {
  margin-top: 0;
}

.prose-chat p:last-child {
  margin-bottom: 0;
}

.prose-chat code {
  font-family: var(--font-mono);
  font-size: 0.9em;
  padding: 0.15em 0.4em;
  border-radius: 4px;
  background-color: var(--codex-bg-tertiary);
  color: var(--codex-accent);
}

.prose-chat pre {
  margin: 0.75em 0;
  padding: 0.75em 1em;
  border-radius: 6px;
  background-color: var(--codex-bg-tertiary);
  border: 1px solid var(--codex-border);
  overflow-x: auto;
}

.prose-chat pre code {
  padding: 0;
  background: none;
  color: var(--codex-fg);
  font-size: 0.85em;
  line-height: 1.6;
}

.prose-chat ul, .prose-chat ol {
  margin: 0.5em 0;
  padding-left: 1.5em;
}

.prose-chat li {
  margin: 0.25em 0;
}

.prose-chat h1, .prose-chat h2, .prose-chat h3,
.prose-chat h4, .prose-chat h5, .prose-chat h6 {
  margin: 0.75em 0 0.25em;
  color: var(--codex-fg);
  font-weight: 500;
}

.prose-chat h1 { font-size: 1.3em; }
.prose-chat h2 { font-size: 1.15em; }
.prose-chat h3 { font-size: 1.05em; }

.prose-chat blockquote {
  margin: 0.5em 0;
  padding-left: 1em;
  border-left: 3px solid var(--codex-border);
  color: var(--codex-fg-subtle);
}

.prose-chat a {
  color: var(--codex-accent);
  text-decoration: underline;
  text-decoration-color: rgba(var(--codex-accent-rgb, 100, 200, 255), 0.3);
}

.prose-chat a:hover {
  text-decoration-color: var(--codex-accent);
}

.prose-chat table {
  border-collapse: collapse;
  margin: 0.5em 0;
  width: 100%;
}

.prose-chat th, .prose-chat td {
  border: 1px solid var(--codex-border);
  padding: 0.35em 0.75em;
  text-align: left;
  font-size: 0.9em;
}

.prose-chat th {
  background-color: var(--codex-bg-tertiary);
  font-weight: 500;
}

.prose-chat hr {
  border: none;
  border-top: 1px solid var(--codex-border);
  margin: 1em 0;
}
```

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/index.css
git commit -m "feat(dashboard): add markdown prose styles for chat messages"
```

---

### Task 6: Verify build passes

**Step 1: Run TypeScript check**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit`
Expected: No errors

**Step 2: Run build**

Run: `cd crates/dashboard/frontend && npm run build`
Expected: Build succeeds

**Step 3: Fix any type errors**

If there are type errors, fix them in the relevant files.

**Step 4: Final commit if needed**

```bash
git add -A crates/dashboard/frontend/
git commit -m "fix(dashboard): resolve build issues in chat page completion"
```

---

### Task 7: Manual browser testing

**Step 1: Start the dev server**

Run: `cd crates/dashboard/frontend && npm run dev`

**Step 2: Test each feature in browser**

Use Claude in Chrome browser automation to:

1. **Chat messaging** — send a message, verify it appears, verify streaming response
2. **Session switching** — click a session in sidebar, verify messages load
3. **New session** — click "New" button, verify messages clear
4. **Session delete** — hover session item, click trash icon, verify removal
5. **Markdown rendering** — send a message that would produce markdown, verify rendered HTML
6. **Interaction panel** — (requires agent to trigger ask_user)
7. **Quick Tasks sidebar** — verify tasks appear with priority badges
8. **Upcoming Calendar sidebar** — verify events appear with relative times
9. **Session Context sidebar** — verify message counts update live
