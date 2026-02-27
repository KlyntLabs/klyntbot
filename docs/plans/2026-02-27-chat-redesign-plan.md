# Chat UI Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decompose the 1574-line Chat.tsx monolith into a chat module with URL-based session routing, a dedicated sessions management page, and a tool activity panel in the sidebar.

**Architecture:** ChatProvider context wraps useAgent + URL params + tool activity state. Chat module lives in `app/chat/` with components extracted from Chat.tsx. New `/sessions` page replaces the sidebar session list. Tool activity panel shows all 11 tool categories as chips with inactive/active/used visual states.

**Tech Stack:** React 19, React Router 7, TypeScript, Tailwind CSS 4, Lucide icons, Motion (framer-motion)

---

### Task 1: Add types for tool activity tracking

**Files:**
- Modify: `crates/dashboard/frontend/src/lib/types.ts` (append after line 437)

**Step 1: Add the new types**

```typescript
// ── Tool Activity (chat sidebar) ─────────────────────────────────────────────

export type ToolCategory =
  | 'Tasks'
  | 'Plans'
  | 'Calendar'
  | 'Finance'
  | 'Skills'
  | 'Cron'
  | 'Projects'
  | 'Web'
  | 'Files'
  | 'Message'
  | 'Spawn';

export interface ToolActivityEntry {
  category: ToolCategory;
  toolName: string;
  args?: Record<string, unknown>;
  timestamp: number;
  status: 'active' | 'completed' | 'failed';
}

/** Maps raw tool names from WebSocket events to display categories */
export const TOOL_CATEGORY_MAP: Record<string, ToolCategory> = {
  todo: 'Tasks',
  plan: 'Plans',
  calendar: 'Calendar',
  finance: 'Finance',
  skill: 'Skills',
  cron: 'Cron',
  project: 'Projects',
  web_search: 'Web',
  web_fetch: 'Web',
  file_read: 'Files',
  file_write: 'Files',
  file_list: 'Files',
  file_append: 'Files',
  message: 'Message',
  ask_user: 'Message',
  spawn: 'Spawn',
};
```

**Step 2: Run tests to verify nothing is broken**

Run: `cd crates/dashboard/frontend && npx vitest run --reporter=verbose 2>&1 | tail -30`
Expected: All existing tests PASS (type additions are additive)

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/lib/types.ts
git commit -m "feat(dashboard): add ToolCategory and ToolActivityEntry types"
```

---

### Task 2: Extract shared utilities from Chat.tsx

These utility functions are used by multiple components in the chat module. Extract them into a shared file before splitting components.

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/utils.ts`

**Step 1: Create the utils file**

Extract these functions from `Chat.tsx` lines 38-124 (renderMarkdown, strategyLabel, formatRelativeTime, priorityDisplay, phaseLabel, formatTime, formatDuration):

```typescript
import { marked } from 'marked';
import type { ThinkingState } from '../../lib/hooks/useAgent';

// Configure marked for safe rendering
marked.setOptions({
  gfm: true,
  breaks: true,
});

/** Render markdown to sanitized HTML */
export function renderMarkdown(content: string): string {
  const html = marked.parse(content, { async: false }) as string;
  return html
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '')
    .replace(/on\w+="[^"]*"/gi, '')
    .replace(/on\w+='[^']*'/gi, '');
}

/** Parse Rust debug format strategy into a clean label */
export function strategyLabel(raw: string): string {
  if (raw.startsWith('Direct')) return 'Direct';
  if (raw.startsWith('Reactive')) return 'Reactive';
  if (raw.startsWith('Planned')) return 'Planned';
  return raw;
}

/** Format relative time like "in 2h", "tomorrow", "in 3d" */
export function formatRelativeTime(dateStr: string): string {
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
export function priorityDisplay(p: number | null): { label: string; color: string } {
  switch (p) {
    case 1: return { label: 'P1', color: '#ef4444' };
    case 2: return { label: 'P2', color: '#f59e0b' };
    case 3: return { label: 'P3', color: 'var(--codex-accent)' };
    case 4: return { label: 'P4', color: 'var(--codex-fg-subtle)' };
    default: return { label: '--', color: 'var(--codex-fg-subtle)' };
  }
}

/** Map thinking phase to a human-readable label */
export function phaseLabel(phase: ThinkingState['phase']): string {
  switch (phase) {
    case 'classifying':
      return 'Classifying';
    case 'buildingContext':
      return 'Building context';
    case 'thinking':
      return 'Thinking';
    case 'idle':
      return 'Idle';
  }
}

/** Format a Date to a short time string like "10:32 AM" */
export function formatTime(d: Date): string {
  return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

/** Compute a human-readable duration between two ISO date strings */
export function formatDuration(createdAt: string, updatedAt: string): string {
  const start = new Date(createdAt).getTime();
  const end = new Date(updatedAt).getTime();
  const diffMs = Math.max(0, end - start);
  const totalMinutes = Math.floor(diffMs / 60000);
  if (totalMinutes < 1) return '<1m';
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes}m`;
}

export type SuggestionCard = {
  id: string;
  icon: React.ComponentType<{ className?: string; strokeWidth?: number; style?: React.CSSProperties }>;
  title: string;
  description: string;
};
```

**Step 2: Run TypeScript check**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit 2>&1 | tail -20`
Expected: No errors (file isn't imported yet, but should compile cleanly)

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/utils.ts
git commit -m "refactor(dashboard): extract chat utility functions"
```

---

### Task 3: Extract SidebarSection and SidebarRow components

These are reusable sidebar primitives used by the chat sidebar and potentially other sidebars.

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/SidebarSection.tsx`

**Step 1: Create the shared sidebar components**

Extract `SidebarSection` (lines 1512-1558) and `SidebarRow` (lines 1561-1574) from Chat.tsx:

```typescript
import { ChevronDown, ChevronRight } from 'lucide-react';

export function SidebarSection({
  title,
  open,
  onToggle,
  noBorder,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  noBorder?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      className={noBorder ? '' : 'border-b'}
      style={{ borderColor: 'var(--codex-border-subtle)' }}
    >
      <button
        onClick={onToggle}
        className="w-full px-4 py-3 flex items-center justify-between transition-colors"
        style={{
          backgroundColor: 'transparent',
          color: 'var(--codex-fg-subtle)',
        }}
        onMouseEnter={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-muted)')
        }
        onMouseLeave={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-subtle)')
        }
      >
        <span
          className="text-[10px] uppercase tracking-wider"
          style={{ fontWeight: 500 }}
        >
          {title}
        </span>
        {open ? (
          <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} />
        ) : (
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        )}
      </button>
      {open && children}
    </div>
  );
}

export function SidebarRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex justify-between items-center">
      <span style={{ color: 'var(--codex-fg-subtle)' }}>{label}</span>
      <span style={{ color: 'var(--codex-fg)' }}>{children}</span>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/sidebar/SidebarSection.tsx
git commit -m "refactor(dashboard): extract SidebarSection and SidebarRow"
```

---

### Task 4: Extract MessageBubble component

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/MessageBubble.tsx`

**Step 1: Create the component**

Extract `MessageBubble` (lines 907-993) and `StatusDot` (lines 127-151) from Chat.tsx. MessageBubble uses renderMarkdown and formatTime from utils:

```typescript
import { motion } from 'motion/react';
import { Terminal, Wifi, WifiOff } from 'lucide-react';
import type { ChatMessage } from '../../../lib/types';
import type { ConnectionStatus } from '../../../lib/ws';
import { renderMarkdown, formatTime } from '../utils';

/** Connection status indicator component */
export function StatusDot({ status }: { status: ConnectionStatus }) {
  const color =
    status === 'connected'
      ? '#10b981'
      : status === 'connecting' || status === 'reconnecting'
        ? '#f59e0b'
        : '#ef4444';

  const Icon =
    status === 'connected' || status === 'connecting' || status === 'reconnecting'
      ? Wifi
      : WifiOff;

  return (
    <div className="flex items-center gap-1.5" title={`WebSocket: ${status}`}>
      <Icon className="w-3 h-3" strokeWidth={1.5} style={{ color }} />
      <span
        className="text-[10px] uppercase tracking-wide"
        style={{ color, fontFamily: 'var(--font-mono)' }}
      >
        {status}
      </span>
    </div>
  );
}

export function MessageBubble({ msg }: { msg: ChatMessage }) {
  return (
    <motion.div
      key={msg.id}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex flex-col"
    >
      {msg.role === 'user' && (
        <div className="flex justify-end">
          <div
            className="max-w-[85%] px-4 py-3 rounded-lg"
            style={{
              backgroundColor: 'var(--codex-bg-user)',
              color: 'var(--codex-fg)',
            }}
          >
            <div
              className="text-[14px] leading-relaxed whitespace-pre-wrap"
              style={{ color: 'var(--codex-fg)' }}
            >
              {msg.content}
            </div>
            <div
              className="text-[11px] mt-2"
              style={{
                color: 'var(--codex-fg-subtle)',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {formatTime(msg.timestamp)}
            </div>
          </div>
        </div>
      )}

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

      {msg.role === 'system' && (
        <div className="flex justify-center py-2">
          <div
            className="flex items-center gap-2 px-3 py-1.5 rounded-md text-[12px]"
            style={{
              backgroundColor: 'var(--codex-bg-secondary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            <Terminal className="w-3 h-3" strokeWidth={1.5} />
            {msg.content}
          </div>
        </div>
      )}
    </motion.div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/MessageBubble.tsx
git commit -m "refactor(dashboard): extract MessageBubble and StatusDot components"
```

---

### Task 5: Extract ThinkingIndicator component

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/ThinkingIndicator.tsx`

**Step 1: Create the component**

Extract `ThinkingIndicator` (lines 998-1176) from Chat.tsx:

```typescript
import { Loader2, Check, X } from 'lucide-react';
import type { ThinkingState } from '../../../lib/hooks/useAgent';
import { phaseLabel, strategyLabel } from '../utils';

export function ThinkingIndicator({ thinking }: { thinking: ThinkingState }) {
  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-border)',
      }}
    >
      {/* Phase header */}
      <div
        className="px-3 py-2 flex items-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderBottom: '1px solid var(--codex-border)',
        }}
      >
        <Loader2
          className="w-3.5 h-3.5 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[12px]"
          style={{
            color: 'var(--codex-fg)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {phaseLabel(thinking.phase)}
        </span>

        {/* Strategy badge */}
        {thinking.strategy && (
          <div
            className="flex items-center gap-1 px-2 py-0.5 rounded ml-auto"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
            }}
          >
            <span
              className="text-[10px]"
              style={{
                color: '#888',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {strategyLabel(thinking.strategy)}
            </span>
            {thinking.confidence != null && (
              <>
                <span className="text-[10px]" style={{ color: '#666' }}>
                  &middot;
                </span>
                <span
                  className="text-[10px]"
                  style={{
                    color:
                      thinking.confidence > 0.8
                        ? 'var(--codex-accent)'
                        : thinking.confidence > 0.5
                          ? '#e5a00d'
                          : '#ef4444',
                    fontFamily: 'var(--font-mono)',
                    fontWeight: 500,
                  }}
                >
                  {Math.round(thinking.confidence * 100)}%
                </span>
              </>
            )}
          </div>
        )}

        {/* Iteration counter */}
        {thinking.iteration != null && thinking.maxIterations != null && (
          <span
            className="text-[10px] ml-2"
            style={{
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            {thinking.iteration}/{thinking.maxIterations}
          </span>
        )}
      </div>

      {/* Tool calls */}
      {thinking.toolCalls.length > 0 && (
        <div className="px-3 py-2 space-y-1.5">
          {thinking.toolCalls.map((tc, idx) => (
            <div
              key={`${tc.name}-${idx}`}
              className="flex items-center gap-2"
            >
              {tc.completed ? (
                tc.success ? (
                  <Check className="w-3 h-3" strokeWidth={2} style={{ color: '#10b981' }} />
                ) : (
                  <X className="w-3 h-3" strokeWidth={2} style={{ color: '#ef4444' }} />
                )
              ) : (
                <Loader2
                  className="w-3 h-3 animate-spin"
                  strokeWidth={1.5}
                  style={{ color: 'var(--codex-accent)' }}
                />
              )}
              <span
                className="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wide"
                style={{
                  backgroundColor: tc.completed
                    ? tc.success
                      ? 'rgba(16, 185, 129, 0.1)'
                      : 'rgba(239, 68, 68, 0.1)'
                    : 'var(--codex-accent-dim)',
                  color: tc.completed
                    ? tc.success
                      ? '#10b981'
                      : '#ef4444'
                    : 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)',
                  fontWeight: 500,
                }}
              >
                {tc.name}
              </span>
              {tc.durationMs != null && (
                <span
                  className="text-[10px]"
                  style={{
                    color: '#888',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  {tc.durationMs}ms
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Pulsing dots when no tool calls yet */}
      {thinking.toolCalls.length === 0 && (
        <div className="px-3 py-3 flex gap-1.5">
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{ backgroundColor: 'var(--codex-accent)' }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.2s',
            }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.4s',
            }}
          />
        </div>
      )}
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/ThinkingIndicator.tsx
git commit -m "refactor(dashboard): extract ThinkingIndicator component"
```

---

### Task 6: Extract InteractionPanel and InteractionField components

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/InteractionPanel.tsx`

**Step 1: Create the component**

Extract `InteractionPanel` (lines 1234-1331) and `InteractionField` (lines 1333-1507) from Chat.tsx:

```typescript
import { useState, useCallback } from 'react';
import { MessageSquare, Check } from 'lucide-react';
import type { PendingInteraction } from '../../../lib/hooks/useAgent';
import type { InteractionQuestion } from '../../../lib/types';

export function InteractionPanel({
  interaction,
  onRespond,
}: {
  interaction: PendingInteraction;
  onRespond: (requestId: string, response: Record<string, unknown>) => void;
}) {
  const [answers, setAnswers] = useState<Record<number, unknown>>({});

  const handleSubmit = useCallback(() => {
    const formAnswers = interaction.questions.map((q, idx) => {
      const raw = answers[idx];
      let value: Record<string, unknown>;

      switch (q.answer_type.type) {
        case 'single_select':
          value = { type: 'selected', value: (raw as string) ?? '' };
          break;
        case 'multi_select':
          value = { type: 'multi_selected', values: (raw as string[]) ?? [] };
          break;
        case 'yes_no':
          value = { type: 'yes_no', answer: (raw as boolean) ?? q.answer_type.default ?? false };
          break;
        case 'free_text':
          value = { type: 'text', content: (raw as string) ?? '' };
          break;
        default:
          value = { type: 'skipped' };
      }

      return { question_id: q.id, value };
    });

    onRespond(interaction.requestId, { Completed: formAnswers });
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
          onMouseEnter={(e) => { e.currentTarget.style.opacity = '0.9'; }}
          onMouseLeave={(e) => { e.currentTarget.style.opacity = '1'; }}
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
  const at = question.answer_type;

  switch (at.type) {
    case 'free_text':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.text}
          </label>
          <input
            type="text"
            value={(value as string) ?? ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={at.placeholder ?? ''}
            className="w-full px-3 py-2 rounded-md text-[13px] outline-none"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg)',
            }}
          />
        </div>
      );

    case 'yes_no':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.text}
          </label>
          <div className="flex gap-2">
            {(['Yes', 'No'] as const).map((opt) => {
              const selected =
                (opt === 'Yes' && value === true) ||
                (opt === 'No' && value === false);
              return (
                <button
                  key={opt}
                  onClick={() => onChange(opt === 'Yes')}
                  className="flex-1 py-1.5 rounded-md text-[12px] transition-colors"
                  style={{
                    backgroundColor: selected ? 'var(--codex-accent)' : 'var(--codex-bg-tertiary)',
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

    case 'single_select':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.text}
          </label>
          <div className="space-y-1">
            {at.options.map((opt) => (
              <button
                key={opt.value}
                onClick={() => onChange(opt.value)}
                className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors"
                style={{
                  backgroundColor: value === opt.value ? 'var(--codex-accent-dim)' : 'var(--codex-bg-tertiary)',
                  color: value === opt.value ? 'var(--codex-accent)' : 'var(--codex-fg)',
                  border: `1px solid ${value === opt.value ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                }}
              >
                <span>{opt.label}</span>
                {opt.description && (
                  <span className="block text-[11px] mt-0.5" style={{ color: 'var(--codex-fg-subtle)', opacity: 0.7 }}>
                    {opt.description}
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>
      );

    case 'multi_select':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.text}
          </label>
          <div className="space-y-1">
            {at.options.map((opt) => {
              const selected = Array.isArray(value) && (value as string[]).includes(opt.value);
              return (
                <button
                  key={opt.value}
                  onClick={() => {
                    const current = (Array.isArray(value) ? value : []) as string[];
                    onChange(
                      selected
                        ? current.filter((v) => v !== opt.value)
                        : [...current, opt.value],
                    );
                  }}
                  className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors flex items-center gap-2"
                  style={{
                    backgroundColor: selected ? 'var(--codex-accent-dim)' : 'var(--codex-bg-tertiary)',
                    color: selected ? 'var(--codex-accent)' : 'var(--codex-fg)',
                    border: `1px solid ${selected ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                  }}
                >
                  <div
                    className="w-3 h-3 rounded-sm border flex items-center justify-center"
                    style={{
                      borderColor: selected ? 'var(--codex-accent)' : 'var(--codex-border)',
                      backgroundColor: selected ? 'var(--codex-accent)' : 'transparent',
                    }}
                  >
                    {selected && <Check className="w-2 h-2" strokeWidth={3} style={{ color: '#000' }} />}
                  </div>
                  <span>{opt.label}</span>
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
git add crates/dashboard/frontend/src/app/chat/components/InteractionPanel.tsx
git commit -m "refactor(dashboard): extract InteractionPanel and InteractionField"
```

---

### Task 7: Extract MessageInput component

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/MessageInput.tsx`

**Step 1: Create the component**

Extract the message input area (Chat.tsx lines 433-539) into its own component. It needs sendMessage, cancel, newSession/startNewSession, and isStreaming from context:

```typescript
import { useState, useCallback, useRef } from 'react';
import { Send, X, Plus } from 'lucide-react';

interface MessageInputProps {
  onSend: (text: string) => void;
  onCancel: () => void;
  onNewSession: () => void;
  isStreaming: boolean;
}

export function MessageInput({ onSend, onCancel, onNewSession, isStreaming }: MessageInputProps) {
  const [message, setMessage] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = useCallback(() => {
    const text = message.trim();
    if (!text) return;
    setMessage('');
    onSend(text);
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [message, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleTextareaChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setMessage(e.target.value);
      e.target.style.height = 'auto';
      e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
    },
    [],
  );

  return (
    <div
      className="p-4 border-t"
      style={{
        borderColor: 'var(--codex-border-subtle)',
        backgroundColor: 'var(--codex-bg)',
      }}
    >
      <div className="px-4">
        {/* Cancel button when streaming */}
        {isStreaming && (
          <div className="flex justify-center mb-2">
            <button
              onClick={onCancel}
              className="flex items-center gap-1.5 px-3 py-1 rounded-md text-[12px] border transition-colors"
              style={{
                borderColor: 'var(--codex-border)',
                color: 'var(--codex-fg-subtle)',
                backgroundColor: 'var(--codex-bg-secondary)',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = '#ef4444';
                e.currentTarget.style.color = '#ef4444';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = 'var(--codex-border)';
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
              }}
            >
              <X className="w-3 h-3" strokeWidth={1.5} />
              Cancel
            </button>
          </div>
        )}

        <div
          className="flex gap-2 items-end px-3 py-2.5 rounded-lg border"
          style={{
            backgroundColor: 'var(--codex-bg-tertiary)',
            borderColor: 'var(--codex-border)',
          }}
        >
          <button
            onClick={onNewSession}
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

          <div className="w-px h-5" style={{ backgroundColor: 'var(--codex-border)' }} />

          <textarea
            ref={textareaRef}
            value={message}
            onChange={handleTextareaChange}
            onKeyDown={handleKeyDown}
            placeholder="Message klyntbot..."
            rows={1}
            disabled={isStreaming}
            className="flex-1 bg-transparent outline-none resize-none text-[14px]"
            style={{
              color: 'var(--codex-fg)',
              fontFamily: 'var(--font-ui)',
              maxHeight: '200px',
              opacity: isStreaming ? 0.5 : 1,
            }}
          />

          <button
            onClick={handleSend}
            disabled={!message.trim() || isStreaming}
            className="p-1.5 rounded transition-colors"
            style={{
              color: message.trim() && !isStreaming ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
              opacity: isStreaming ? 0.5 : 1,
            }}
            onMouseEnter={(e) => {
              if (message.trim() && !isStreaming)
                e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent';
            }}
          >
            <Send className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/MessageInput.tsx
git commit -m "refactor(dashboard): extract MessageInput component"
```

---

### Task 8: Extract MessageArea component

Combines the empty state (suggestion cards) and message list with auto-scroll.

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/components/MessageArea.tsx`

**Step 1: Create the component**

```typescript
import { useRef, useEffect, useCallback } from 'react';
import { Code, FileCode, Lightbulb, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { ChatMessage } from '../../../lib/types';
import type { ThinkingState, PendingInteraction } from '../../../lib/hooks/useAgent';
import { MessageBubble } from './MessageBubble';
import { ThinkingIndicator } from './ThinkingIndicator';
import { InteractionPanel } from './InteractionPanel';
import type { ConnectionStatus } from '../../../lib/ws';
import { StatusDot } from './MessageBubble';

const suggestions = [
  { id: '1', icon: Code, title: 'Build a classic Snake game', description: 'Create a retro snake game with canvas rendering' },
  { id: '2', icon: FileCode, title: 'Refactor legacy code', description: 'Improve code quality and add type safety' },
  { id: '3', icon: Lightbulb, title: 'Optimize performance', description: 'Analyze and improve application speed' },
  { id: '4', icon: Sparkles, title: 'Add new feature', description: 'Implement a feature with best practices' },
];

interface MessageAreaProps {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  pendingInteraction: PendingInteraction | null;
  onSendSuggestion: (text: string) => void;
  onRespondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
}

export function MessageArea({
  messages,
  thinking,
  isStreaming,
  status,
  pendingInteraction,
  onSendSuggestion,
  onRespondToInteraction,
}: MessageAreaProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const hasMessages = messages.length > 0;

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, thinking]);

  const handleSuggestionClick = useCallback(
    (description: string) => {
      onSendSuggestion(description);
    },
    [onSendSuggestion],
  );

  return (
    <div className="flex-1 flex flex-col">
      {/* Connection status bar */}
      {status !== 'connected' && (
        <div
          className="px-4 py-1.5 flex items-center justify-center gap-2 border-b"
          style={{
            backgroundColor: 'var(--codex-bg-secondary)',
            borderColor: 'var(--codex-border-subtle)',
          }}
        >
          <StatusDot status={status} />
          {status === 'reconnecting' && (
            <span className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              Reconnecting...
            </span>
          )}
        </div>
      )}

      {/* Chat Messages */}
      <div className="flex-1 overflow-y-auto px-6 py-8">
        {!hasMessages ? (
          <div className="h-full flex flex-col items-center justify-center max-w-3xl mx-auto">
            <div className="mb-12 text-center">
              <h1 className="text-2xl mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
                How can I help you today?
              </h1>
              <p className="text-sm" style={{ color: 'var(--codex-fg-subtle)' }}>
                Choose a suggestion below or describe your task
              </p>
            </div>

            <div className="grid grid-cols-2 gap-3 w-full max-w-2xl">
              {suggestions.map((suggestion) => (
                <button
                  key={suggestion.id}
                  className="p-4 rounded-lg border text-left transition-all group"
                  style={{
                    backgroundColor: 'var(--codex-bg-tertiary)',
                    borderColor: 'var(--codex-border)',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.borderColor = 'var(--codex-accent)';
                    e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.borderColor = 'var(--codex-border)';
                    e.currentTarget.style.backgroundColor = 'var(--codex-bg-tertiary)';
                  }}
                  onClick={() => handleSuggestionClick(suggestion.description)}
                >
                  <suggestion.icon
                    className="w-5 h-5 mb-3"
                    strokeWidth={1.5}
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  />
                  <div className="text-sm mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
                    {suggestion.title}
                  </div>
                  <div className="text-xs" style={{ color: 'var(--codex-fg-subtle)' }}>
                    {suggestion.description}
                  </div>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div className="max-w-3xl mx-auto space-y-6">
            {messages.map((msg) => (
              <MessageBubble key={msg.id} msg={msg} />
            ))}

            <AnimatePresence>
              {thinking && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -5 }}
                  className="flex gap-3"
                >
                  <div className="flex-1">
                    <ThinkingIndicator thinking={thinking} />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <AnimatePresence>
              {pendingInteraction && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -5 }}
                >
                  <InteractionPanel
                    interaction={pendingInteraction}
                    onRespond={onRespondToInteraction}
                  />
                </motion.div>
              )}
            </AnimatePresence>

            <div ref={messagesEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/components/MessageArea.tsx
git commit -m "refactor(dashboard): extract MessageArea component"
```

---

### Task 9: Create ToolActivityPanel (NEW component)

This is the new feature — a grid of tool category chips that show active/used/inactive states.

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/ToolActivityPanel.tsx`

**Step 1: Create the component**

```typescript
import { useState } from 'react';
import {
  CheckSquare,
  FileText,
  Calendar,
  DollarSign,
  Zap,
  Clock,
  FolderKanban,
  Globe,
  File,
  MessageSquare,
  GitBranch,
} from 'lucide-react';
import type { ToolCategory, ToolActivityEntry } from '../../../lib/types';

const TOOL_CATEGORIES: { category: ToolCategory; icon: typeof CheckSquare }[] = [
  { category: 'Tasks', icon: CheckSquare },
  { category: 'Plans', icon: FileText },
  { category: 'Calendar', icon: Calendar },
  { category: 'Finance', icon: DollarSign },
  { category: 'Skills', icon: Zap },
  { category: 'Cron', icon: Clock },
  { category: 'Projects', icon: FolderKanban },
  { category: 'Web', icon: Globe },
  { category: 'Files', icon: File },
  { category: 'Message', icon: MessageSquare },
  { category: 'Spawn', icon: GitBranch },
];

interface ToolActivityPanelProps {
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

function getToolState(
  category: ToolCategory,
  activeTools: Set<string>,
  toolHistory: ToolActivityEntry[],
): 'inactive' | 'active' | 'used' {
  if (activeTools.has(category)) return 'active';
  if (toolHistory.some((e) => e.category === category)) return 'used';
  return 'inactive';
}

function getLastOperation(category: ToolCategory, toolHistory: ToolActivityEntry[]): string | null {
  for (let i = toolHistory.length - 1; i >= 0; i--) {
    if (toolHistory[i].category === category) {
      const entry = toolHistory[i];
      const argsStr = entry.args
        ? Object.entries(entry.args)
            .map(([k, v]) => `${k}: ${JSON.stringify(v)}`)
            .join(', ')
        : '';
      const status = entry.status === 'failed' ? ' (failed)' : '';
      return `${entry.toolName}${argsStr ? ` — ${argsStr}` : ''}${status}`;
    }
  }
  return null;
}

export function ToolActivityPanel({ activeTools, toolHistory }: ToolActivityPanelProps) {
  const [hoveredCategory, setHoveredCategory] = useState<string | null>(null);

  return (
    <div className="px-4 py-3">
      <div
        className="text-[10px] uppercase tracking-wider mb-2.5"
        style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
      >
        Systems
      </div>
      <div className="flex flex-wrap gap-1.5">
        {TOOL_CATEGORIES.map(({ category, icon: Icon }) => {
          const state = getToolState(category, activeTools, toolHistory);
          const lastOp = getLastOperation(category, toolHistory);

          return (
            <div
              key={category}
              className="relative"
              onMouseEnter={() => setHoveredCategory(category)}
              onMouseLeave={() => setHoveredCategory(null)}
            >
              <div
                className={`flex items-center gap-1 px-2 py-1 rounded-md text-[10px] border transition-all ${
                  state === 'active' ? 'animate-pulse' : ''
                }`}
                style={{
                  opacity: state === 'inactive' ? 0.3 : 1,
                  borderColor:
                    state === 'active'
                      ? 'var(--codex-accent)'
                      : state === 'used'
                        ? 'var(--codex-border)'
                        : 'var(--codex-border-subtle)',
                  backgroundColor:
                    state === 'active'
                      ? 'var(--codex-accent-dim)'
                      : 'transparent',
                  color:
                    state === 'active'
                      ? 'var(--codex-accent)'
                      : state === 'used'
                        ? 'var(--codex-fg-muted)'
                        : 'var(--codex-fg-subtle)',
                }}
              >
                <Icon className="w-3 h-3" strokeWidth={1.5} />
                <span style={{ fontFamily: 'var(--font-mono)' }}>{category}</span>
              </div>

              {/* Tooltip */}
              {hoveredCategory === category && lastOp && (
                <div
                  className="absolute left-0 top-full mt-1 px-2 py-1 rounded text-[10px] whitespace-nowrap z-50"
                  style={{
                    backgroundColor: 'var(--codex-bg-tertiary)',
                    border: '1px solid var(--codex-border)',
                    color: 'var(--codex-fg-muted)',
                    fontFamily: 'var(--font-mono)',
                    maxWidth: '220px',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {lastOp}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/sidebar/ToolActivityPanel.tsx
git commit -m "feat(dashboard): add ToolActivityPanel component"
```

---

### Task 10: Extract sidebar section components (ToolCallList, QuickTasks, UpcomingEvents, ConnectionStatus)

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/ConnectionStatus.tsx`
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/ToolCallList.tsx`
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/QuickTasks.tsx`
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/UpcomingEvents.tsx`

**Step 1: Create ConnectionStatus.tsx**

```typescript
import { Loader2 } from 'lucide-react';
import type { ConnectionStatus as ConnectionStatusType } from '../../../lib/ws';
import { StatusDot } from '../components/MessageBubble';

interface ConnectionStatusBarProps {
  status: ConnectionStatusType;
  isStreaming: boolean;
}

export function ConnectionStatusBar({ status, isStreaming }: ConnectionStatusBarProps) {
  return (
    <div
      className="px-4 py-2.5 border-b flex items-center justify-between"
      style={{ borderColor: 'var(--codex-border-subtle)' }}
    >
      <StatusDot status={status} />
      {isStreaming && (
        <Loader2
          className="w-3 h-3 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
      )}
    </div>
  );
}
```

**Step 2: Create ToolCallList.tsx**

Extract `ToolCallItem` (lines 1180-1230) from Chat.tsx and the Tool Calls sidebar section:

```typescript
import { Loader2, Check, X } from 'lucide-react';
import type { ToolCallState, ThinkingState } from '../../../lib/hooks/useAgent';
import { SidebarSection } from './SidebarSection';

function ToolCallItem({ toolCall }: { toolCall: ToolCallState }) {
  return (
    <div
      className="flex items-center gap-2 p-1.5 rounded"
      style={{ backgroundColor: 'var(--codex-bg)' }}
    >
      {toolCall.completed ? (
        toolCall.success ? (
          <Check className="w-3 h-3 flex-shrink-0" strokeWidth={2} style={{ color: '#10b981' }} />
        ) : (
          <X className="w-3 h-3 flex-shrink-0" strokeWidth={2} style={{ color: '#ef4444' }} />
        )
      ) : (
        <Loader2
          className="w-3 h-3 flex-shrink-0 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
      )}
      <span
        className="text-[11px] truncate"
        style={{
          color: toolCall.completed
            ? toolCall.success ? '#10b981' : '#ef4444'
            : 'var(--codex-accent)',
          fontFamily: 'var(--font-mono)',
        }}
      >
        {toolCall.name}
      </span>
      {toolCall.durationMs != null && (
        <span
          className="text-[10px] ml-auto flex-shrink-0"
          style={{ color: '#888', fontFamily: 'var(--font-mono)' }}
        >
          {toolCall.durationMs}ms
        </span>
      )}
    </div>
  );
}

interface ToolCallListProps {
  thinking: ThinkingState | null;
}

export function ToolCallList({ thinking }: ToolCallListProps) {
  if (!thinking || thinking.toolCalls.length === 0) return null;

  return (
    <SidebarSection title="Tool Calls" open={true} onToggle={() => {}}>
      <div className="px-4 pb-4 space-y-2">
        {thinking.toolCalls.map((tc, idx) => (
          <ToolCallItem key={`${tc.name}-${idx}`} toolCall={tc} />
        ))}
      </div>
    </SidebarSection>
  );
}
```

**Step 3: Create QuickTasks.tsx**

```typescript
import { useState, useMemo } from 'react';
import { Circle } from 'lucide-react';
import { useApi } from '../../../lib/hooks/useApi';
import type { Task } from '../../../lib/types';
import { priorityDisplay } from '../utils';
import { SidebarSection } from './SidebarSection';

export function QuickTasks() {
  const [open, setOpen] = useState(true);
  const { data: tasks } = useApi<Task[]>('/api/tasks');

  const pendingTasks = useMemo(() => {
    if (!tasks) return [];
    return tasks
      .filter((t) => t.status === 'todo' || t.status === 'doing')
      .sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99))
      .slice(0, 5);
  }, [tasks]);

  return (
    <SidebarSection title="Quick Tasks" open={open} onToggle={() => setOpen(!open)}>
      <div className="px-4 pb-4 space-y-2">
        {pendingTasks.length === 0 && (
          <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
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
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
              <Circle
                className="w-3 h-3 flex-shrink-0"
                strokeWidth={1.5}
                style={{
                  color: task.status === 'doing' ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                }}
              />
              <span className="text-[12px] truncate flex-1" style={{ color: 'var(--codex-fg)' }}>
                {task.title}
              </span>
              <span
                className="text-[10px] flex-shrink-0"
                style={{ color: p.color, fontFamily: 'var(--font-mono)', fontWeight: 500 }}
              >
                {p.label}
              </span>
            </div>
          );
        })}
      </div>
    </SidebarSection>
  );
}
```

**Step 4: Create UpcomingEvents.tsx**

```typescript
import { useState } from 'react';
import { Calendar } from 'lucide-react';
import { useApi } from '../../../lib/hooks/useApi';
import type { CalendarEvent } from '../../../lib/types';
import { formatRelativeTime } from '../utils';
import { SidebarSection } from './SidebarSection';

export function UpcomingEvents() {
  const [open, setOpen] = useState(true);
  const { data: calendarEvents } = useApi<CalendarEvent[]>('/api/calendar/events', {
    params: { limit: 5 },
  });

  return (
    <SidebarSection title="Upcoming" open={open} onToggle={() => setOpen(!open)} noBorder>
      <div className="px-4 pb-4 space-y-2">
        {(!calendarEvents || calendarEvents.length === 0) && (
          <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            No upcoming events
          </div>
        )}
        {calendarEvents?.slice(0, 5).map((event) => (
          <div
            key={event.uid}
            className="flex items-center gap-2 p-1.5 rounded"
            style={{ backgroundColor: 'transparent' }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
          >
            <Calendar
              className="w-3 h-3 flex-shrink-0"
              strokeWidth={1.5}
              style={{ color: 'var(--codex-accent)' }}
            />
            <span className="text-[12px] truncate flex-1" style={{ color: 'var(--codex-fg)' }}>
              {event.summary}
            </span>
            <span
              className="text-[10px] flex-shrink-0"
              style={{ color: 'var(--codex-fg-subtle)', fontFamily: 'var(--font-mono)' }}
            >
              {formatRelativeTime(event.startAt)}
            </span>
          </div>
        ))}
      </div>
    </SidebarSection>
  );
}
```

**Step 5: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/sidebar/
git commit -m "refactor(dashboard): extract sidebar section components"
```

---

### Task 11: Create ChatSidebar container

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/sidebar/ChatSidebar.tsx`

**Step 1: Create the sidebar container**

Composes: ConnectionStatus, ToolActivityPanel, ToolCallList, QuickTasks, UpcomingEvents.

```typescript
import type { ThinkingState } from '../../../lib/hooks/useAgent';
import type { ConnectionStatus } from '../../../lib/ws';
import type { ToolActivityEntry } from '../../../lib/types';
import { ConnectionStatusBar } from './ConnectionStatus';
import { ToolActivityPanel } from './ToolActivityPanel';
import { ToolCallList } from './ToolCallList';
import { QuickTasks } from './QuickTasks';
import { UpcomingEvents } from './UpcomingEvents';

interface ChatSidebarProps {
  status: ConnectionStatus;
  isStreaming: boolean;
  thinking: ThinkingState | null;
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

export function ChatSidebar({
  status,
  isStreaming,
  thinking,
  activeTools,
  toolHistory,
}: ChatSidebarProps) {
  return (
    <aside
      className="w-[260px] border-l overflow-y-auto"
      style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)',
      }}
    >
      <ConnectionStatusBar status={status} isStreaming={isStreaming} />
      <ToolActivityPanel activeTools={activeTools} toolHistory={toolHistory} />
      <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }} />
      <ToolCallList thinking={thinking} />
      <QuickTasks />
      <UpcomingEvents />
    </aside>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/sidebar/ChatSidebar.tsx
git commit -m "feat(dashboard): create ChatSidebar container with tool activity"
```

---

### Task 12: Create ChatProvider context

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/ChatProvider.tsx`

**Step 1: Create the context provider**

This wraps useAgent + URL params + tool activity tracking:

```typescript
import { createContext, useContext, useCallback, useState, useEffect, useRef } from 'react';
import { useNavigate, useParams } from 'react-router';
import { useAgent } from '../../lib/hooks/useAgent';
import type { ThinkingState, PendingInteraction } from '../../lib/hooks/useAgent';
import type { ChatMessage, ToolActivityEntry, TOOL_CATEGORY_MAP } from '../../lib/types';
import { TOOL_CATEGORY_MAP as toolCategoryMap } from '../../lib/types';
import type { ConnectionStatus } from '../../lib/ws';

interface ChatContextValue {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  sessionKey: string | null;
  pendingInteraction: PendingInteraction | null;
  sendMessage: (text: string) => void;
  cancel: () => void;
  respondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
  startNewSession: () => void;
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

const ChatContext = createContext<ChatContextValue | null>(null);

export function useChatContext(): ChatContextValue {
  const ctx = useContext(ChatContext);
  if (!ctx) throw new Error('useChatContext must be used within ChatProvider');
  return ctx;
}

export function ChatProvider({ children }: { children: React.ReactNode }) {
  const navigate = useNavigate();
  const { sessionId } = useParams<{ sessionId?: string }>();
  const agent = useAgent();
  const [activeTools, setActiveTools] = useState<Set<string>>(new Set());
  const [toolHistory, setToolHistory] = useState<ToolActivityEntry[]>([]);
  const hasLoadedSession = useRef(false);

  // Load session from URL param on mount or when sessionId changes
  useEffect(() => {
    if (sessionId && sessionId !== agent.sessionKey) {
      agent.loadSession(sessionId);
      hasLoadedSession.current = true;
    } else if (!sessionId && agent.sessionKey && hasLoadedSession.current) {
      // Navigated to / — clear session
      agent.newSession();
      setActiveTools(new Set());
      setToolHistory([]);
    }
  }, [sessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Sync URL when session key changes (after first message)
  useEffect(() => {
    if (agent.sessionKey && !sessionId) {
      navigate(`/chat/${agent.sessionKey}`, { replace: true });
    }
  }, [agent.sessionKey, sessionId, navigate]);

  // Track tool activity from thinking state
  const prevToolCallsRef = useRef<number>(0);
  useEffect(() => {
    if (!agent.thinking) {
      // Streaming ended — mark all active tools as completed
      if (activeTools.size > 0) {
        setActiveTools(new Set());
      }
      prevToolCallsRef.current = 0;
      return;
    }

    const toolCalls = agent.thinking.toolCalls;
    // Process new tool calls since last check
    for (let i = prevToolCallsRef.current; i < toolCalls.length; i++) {
      const tc = toolCalls[i];
      const category = toolCategoryMap[tc.name] ?? 'System' as any;

      if (!tc.completed) {
        // Tool started
        setActiveTools((prev) => new Set(prev).add(category));
        setToolHistory((prev) => [
          ...prev,
          {
            category,
            toolName: tc.name,
            args: tc.args,
            timestamp: Date.now(),
            status: 'active',
          },
        ]);
      }
    }

    // Check for newly completed tools
    for (const tc of toolCalls) {
      if (tc.completed) {
        const category = toolCategoryMap[tc.name] ?? 'System' as any;
        setActiveTools((prev) => {
          // Only remove if no other active tool in same category
          const stillActive = toolCalls.some(
            (other) =>
              !other.completed &&
              (toolCategoryMap[other.name] ?? 'System') === category,
          );
          if (stillActive) return prev;
          const next = new Set(prev);
          next.delete(category);
          return next;
        });
        setToolHistory((prev) =>
          prev.map((entry) =>
            entry.toolName === tc.name && entry.status === 'active'
              ? { ...entry, status: tc.success ? 'completed' : 'failed' }
              : entry,
          ),
        );
      }
    }

    prevToolCallsRef.current = toolCalls.length;
  }, [agent.thinking]); // eslint-disable-line react-hooks/exhaustive-deps

  const sendMessage = useCallback(
    (text: string) => {
      agent.sendMessage(text);
    },
    [agent],
  );

  const startNewSession = useCallback(() => {
    agent.newSession();
    setActiveTools(new Set());
    setToolHistory([]);
    navigate('/');
  }, [agent, navigate]);

  const value: ChatContextValue = {
    messages: agent.messages,
    thinking: agent.thinking,
    isStreaming: agent.isStreaming,
    status: agent.status,
    sessionKey: agent.sessionKey,
    pendingInteraction: agent.pendingInteraction,
    sendMessage,
    cancel: agent.cancel,
    respondToInteraction: agent.respondToInteraction,
    startNewSession,
    activeTools,
    toolHistory,
  };

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/ChatProvider.tsx
git commit -m "feat(dashboard): create ChatProvider context with URL sync and tool tracking"
```

---

### Task 13: Create ChatLayout and ChatPage

**Files:**
- Create: `crates/dashboard/frontend/src/app/chat/ChatLayout.tsx`
- Create: `crates/dashboard/frontend/src/app/chat/ChatPage.tsx`

**Step 1: Create ChatLayout.tsx**

```typescript
import { useChatContext } from './ChatProvider';
import { MessageArea } from './components/MessageArea';
import { MessageInput } from './components/MessageInput';
import { ChatSidebar } from './sidebar/ChatSidebar';

export function ChatLayout() {
  const {
    messages,
    thinking,
    isStreaming,
    status,
    pendingInteraction,
    sendMessage,
    cancel,
    respondToInteraction,
    startNewSession,
    activeTools,
    toolHistory,
  } = useChatContext();

  return (
    <>
      <div className="flex-1 flex flex-col">
        <MessageArea
          messages={messages}
          thinking={thinking}
          isStreaming={isStreaming}
          status={status}
          pendingInteraction={pendingInteraction}
          onSendSuggestion={sendMessage}
          onRespondToInteraction={respondToInteraction}
        />
        <MessageInput
          onSend={sendMessage}
          onCancel={cancel}
          onNewSession={startNewSession}
          isStreaming={isStreaming}
        />
      </div>
      <ChatSidebar
        status={status}
        isStreaming={isStreaming}
        thinking={thinking}
        activeTools={activeTools}
        toolHistory={toolHistory}
      />
    </>
  );
}
```

**Step 2: Create ChatPage.tsx**

```typescript
import { ChatProvider } from './ChatProvider';
import { ChatLayout } from './ChatLayout';

export default function ChatPage() {
  return (
    <ChatProvider>
      <ChatLayout />
    </ChatProvider>
  );
}
```

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/app/chat/ChatLayout.tsx crates/dashboard/frontend/src/app/chat/ChatPage.tsx
git commit -m "feat(dashboard): create ChatPage and ChatLayout"
```

---

### Task 14: Create Sessions page

**Files:**
- Create: `crates/dashboard/frontend/src/app/pages/Sessions.tsx`

**Step 1: Create the page**

```typescript
import { useState, useMemo, useCallback } from 'react';
import { useNavigate } from 'react-router';
import { Search, Clock, Trash2, ArrowUpDown } from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { SessionListItem } from '../../lib/types';
import { formatDuration } from '../chat/utils';

type SortMode = 'recent' | 'oldest' | 'messages';

export default function Sessions() {
  const navigate = useNavigate();
  const { data: sessions, loading, refetch } = useApi<SessionListItem[]>('/api/sessions');
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortMode>('recent');
  const [deleting, setDeleting] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!sessions) return [];
    let list = sessions;
    if (search) {
      const q = search.toLowerCase();
      list = list.filter((s) => s.key.toLowerCase().includes(q));
    }
    return list.slice().sort((a, b) => {
      switch (sort) {
        case 'recent':
          return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
        case 'oldest':
          return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
        case 'messages':
          return b.messageCount - a.messageCount;
      }
    });
  }, [sessions, search, sort]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent, key: string) => {
      e.stopPropagation();
      setDeleting(key);
      try {
        await apiFetch(`/api/sessions/${key}`, { method: 'DELETE' });
        refetch();
      } finally {
        setDeleting(null);
      }
    },
    [refetch],
  );

  const formatDate = (iso: string) => {
    const d = new Date(iso);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHours = Math.floor(diffMin / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays}d ago`;
    return d.toLocaleDateString();
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div
        className="px-6 py-4 border-b flex items-center justify-between"
        style={{ borderColor: 'var(--codex-border-subtle)' }}
      >
        <h1
          className="text-lg"
          style={{ color: 'var(--codex-fg)', fontWeight: 400 }}
        >
          Sessions
        </h1>
        <div className="flex items-center gap-3">
          {/* Search */}
          <div
            className="flex items-center gap-2 px-3 py-1.5 rounded-md border"
            style={{
              borderColor: 'var(--codex-border)',
              backgroundColor: 'var(--codex-bg-tertiary)',
            }}
          >
            <Search className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search sessions..."
              className="bg-transparent outline-none text-[13px] w-48"
              style={{ color: 'var(--codex-fg)' }}
            />
          </div>
          {/* Sort */}
          <div className="flex items-center gap-1.5">
            <ArrowUpDown className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
            {(['recent', 'oldest', 'messages'] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => setSort(mode)}
                className="px-2 py-1 rounded text-[11px] transition-colors"
                style={{
                  backgroundColor: sort === mode ? 'var(--codex-bg-tertiary)' : 'transparent',
                  color: sort === mode ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
                  border: sort === mode ? '1px solid var(--codex-border)' : '1px solid transparent',
                }}
              >
                {mode === 'recent' ? 'Recent' : mode === 'oldest' ? 'Oldest' : 'Messages'}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-y-auto">
        {loading && (
          <div className="flex items-center justify-center py-12 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            Loading sessions...
          </div>
        )}
        {!loading && filtered.length === 0 && (
          <div className="flex items-center justify-center py-12 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            {search ? 'No sessions match your search' : 'No sessions yet'}
          </div>
        )}
        {filtered.map((session) => (
          <button
            key={session.key}
            onClick={() => navigate(`/chat/${session.key}`)}
            className="w-full px-6 py-3 flex items-center gap-4 border-b transition-colors text-left group"
            style={{
              borderColor: 'var(--codex-border-subtle)',
              backgroundColor: 'transparent',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
          >
            {/* Session key */}
            <span
              className="text-[12px] w-20 flex-shrink-0"
              style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}
            >
              #{session.key.slice(0, 8)}
            </span>

            {/* Message count */}
            <span
              className="text-[11px] w-16 flex-shrink-0"
              style={{ color: 'var(--codex-fg-subtle)', fontFamily: 'var(--font-mono)' }}
            >
              {session.messageCount} msgs
            </span>

            {/* Duration */}
            <div className="flex items-center gap-1 text-[11px] w-16 flex-shrink-0" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Clock className="w-3 h-3" strokeWidth={1.5} />
              {formatDuration(session.createdAt, session.updatedAt)}
            </div>

            {/* Created date */}
            <span className="text-[11px] flex-1" style={{ color: 'var(--codex-fg-subtle)' }}>
              {formatDate(session.createdAt)}
            </span>

            {/* Last active */}
            <span className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              Active {formatDate(session.updatedAt)}
            </span>

            {/* Delete */}
            <button
              onClick={(e) => handleDelete(e, session.key)}
              disabled={deleting === session.key}
              className="p-1.5 rounded opacity-0 group-hover:opacity-100 transition-opacity"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => { e.currentTarget.style.color = '#ef4444'; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; }}
              title="Delete session"
            >
              <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </button>
        ))}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/pages/Sessions.tsx
git commit -m "feat(dashboard): add Sessions management page"
```

---

### Task 15: Update routes and Layout

**Files:**
- Modify: `crates/dashboard/frontend/src/app/routes.tsx`
- Modify: `crates/dashboard/frontend/src/app/components/Layout.tsx:99-108`

**Step 1: Update routes.tsx**

Replace the file with updated routes adding `/chat/:sessionId` and `/sessions`:

In `routes.tsx`:
- Change import: `import Chat from './pages/Chat'` → `import ChatPage from './chat/ChatPage'`
- Add import: `import Sessions from './pages/Sessions'`
- Change chat index: `{ index: true, element: <Chat /> }` → `{ index: true, element: <ChatPage /> }`
- Add route after index: `{ path: 'chat/:sessionId', element: <ChatPage /> }`
- Add route before tasks: `{ path: 'sessions', element: <Sessions /> }`

**Step 2: Update Layout.tsx nav items**

In `Layout.tsx` at the `navItems` array (line 99-108), add Sessions between Chat and Tasks:

Change the navItems array to include:

```typescript
const navItems: NavItem[] = [
  { id: 'chat', icon: MessageSquare, label: 'Chat', path: '/' },
  { id: 'sessions', icon: History, label: 'Sessions', path: '/sessions' },
  { id: 'tasks', icon: CheckSquare, label: 'Tasks', path: '/tasks' },
  { id: 'projects', icon: FolderKanban, label: 'Projects', path: '/projects' },
  { id: 'plans', icon: FileText, label: 'Plans', path: '/plans' },
  { id: 'calendar', icon: Calendar, label: 'Calendar', path: '/calendar' },
  { id: 'cron', icon: Clock, label: 'Cron', path: '/cron' },
  { id: 'skills', icon: Zap, label: 'Skills', path: '/skills' },
  { id: 'finance', icon: DollarSign, label: 'Finance', path: '/finance' },
];
```

Also add `History` to the lucide-react import.

Also update `isActive` to handle `/chat/*` routes:

```typescript
const isActive = (path: string) => {
  if (path === '/') return location.pathname === '/' || location.pathname.startsWith('/chat/');
  return location.pathname.startsWith(path);
};
```

**Step 3: Run TypeScript check**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit 2>&1 | tail -30`
Expected: No errors

**Step 4: Commit**

```bash
git add crates/dashboard/frontend/src/app/routes.tsx crates/dashboard/frontend/src/app/components/Layout.tsx
git commit -m "feat(dashboard): add /chat/:sessionId and /sessions routes, Sessions nav item"
```

---

### Task 16: Remove sessionStorage from useAgent

**Files:**
- Modify: `crates/dashboard/frontend/src/lib/hooks/useAgent.ts`

**Step 1: Remove sessionStorage persistence**

In `useAgent.ts`:

1. Remove the initial state from sessionStorage (line 69-71):
   ```typescript
   // Before:
   const [sessionKey, setSessionKey] = useState<string | null>(() => {
     return sessionStorage.getItem('klyntbot-session-key');
   });
   // After:
   const [sessionKey, setSessionKey] = useState<string | null>(null);
   ```

2. Remove the sessionStorage sync effect (lines 80-86):
   ```typescript
   // Delete this entire useEffect:
   useEffect(() => {
     if (sessionKey) {
       sessionStorage.setItem('klyntbot-session-key', sessionKey);
     } else {
       sessionStorage.removeItem('klyntbot-session-key');
     }
   }, [sessionKey]);
   ```

3. Remove the session auto-load from sessionStorage (lines 304-310):
   ```typescript
   // Delete this entire useEffect:
   useEffect(() => {
     const savedKey = sessionStorage.getItem('klyntbot-session-key');
     if (savedKey) {
       loadSession(savedKey);
     }
     // eslint-disable-next-line react-hooks/exhaustive-deps
   }, []);
   ```

**Step 2: Run tests**

Run: `cd crates/dashboard/frontend && npx vitest run --reporter=verbose 2>&1 | tail -40`
Expected: All tests PASS. The useAgent tests don't depend on sessionStorage.

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/lib/hooks/useAgent.ts
git commit -m "refactor(dashboard): remove sessionStorage from useAgent (URL is source of truth)"
```

---

### Task 17: Delete old Chat.tsx

**Files:**
- Delete: `crates/dashboard/frontend/src/app/pages/Chat.tsx`

**Step 1: Delete the old monolith**

Run: `rm crates/dashboard/frontend/src/app/pages/Chat.tsx`

**Step 2: Run TypeScript check**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit 2>&1 | tail -30`
Expected: No errors — routes.tsx now imports from `./chat/ChatPage` instead

**Step 3: Run all tests**

Run: `cd crates/dashboard/frontend && npx vitest run --reporter=verbose 2>&1 | tail -40`
Expected: All tests pass

**Step 4: Commit**

```bash
git add -A crates/dashboard/frontend/src/app/pages/Chat.tsx
git commit -m "refactor(dashboard): remove Chat.tsx monolith (replaced by chat/ module)"
```

---

### Task 18: Update route tests

**Files:**
- Modify: `crates/dashboard/frontend/src/app/__tests__/routes.test.tsx`

**Step 1: Update the test to include new routes**

Update the `ALL_ROUTES` array to include the new routes:

```typescript
const ALL_ROUTES = [
  { path: '/',              label: 'Chat' },
  { path: '/chat/test-123', label: 'Chat Session' },
  { path: '/sessions',      label: 'Sessions' },
  { path: '/tasks',         label: 'Tasks' },
  { path: '/tasks/123',     label: 'Task Detail' },
  { path: '/plans',         label: 'Plans' },
  { path: '/calendar',      label: 'Calendar' },
  { path: '/cron',          label: 'Cron' },
  { path: '/skills',        label: 'Skills' },
  { path: '/finance',       label: 'Finance' },
  { path: '/settings',      label: 'Settings' },
  { path: '/setup',         label: 'Setup' },
];
```

**Step 2: Run route tests**

Run: `cd crates/dashboard/frontend && npx vitest run src/app/__tests__/routes.test.tsx --reporter=verbose 2>&1`
Expected: All tests PASS including new route entries

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/app/__tests__/routes.test.tsx
git commit -m "test(dashboard): update route tests for /chat/:sessionId and /sessions"
```

---

### Task 19: Run full test suite and fix any issues

**Step 1: Run all frontend tests**

Run: `cd crates/dashboard/frontend && npx vitest run --reporter=verbose 2>&1`
Expected: All tests pass

**Step 2: Run TypeScript check**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit 2>&1`
Expected: No errors

**Step 3: Run dev server smoke test**

Run: `cd crates/dashboard/frontend && npx vite build 2>&1 | tail -20`
Expected: Build succeeds with no errors

**Step 4: Fix any issues found**

If any test failures or type errors, fix them and commit:

```bash
git add -A crates/dashboard/frontend/
git commit -m "fix(dashboard): resolve issues from chat module refactor"
```

---

### Task 20: Final verification and summary commit

**Step 1: Verify file structure**

Run: `find crates/dashboard/frontend/src/app/chat -type f | sort`
Expected output:
```
crates/dashboard/frontend/src/app/chat/ChatLayout.tsx
crates/dashboard/frontend/src/app/chat/ChatPage.tsx
crates/dashboard/frontend/src/app/chat/ChatProvider.tsx
crates/dashboard/frontend/src/app/chat/components/InteractionPanel.tsx
crates/dashboard/frontend/src/app/chat/components/MessageArea.tsx
crates/dashboard/frontend/src/app/chat/components/MessageBubble.tsx
crates/dashboard/frontend/src/app/chat/components/MessageInput.tsx
crates/dashboard/frontend/src/app/chat/components/ThinkingIndicator.tsx
crates/dashboard/frontend/src/app/chat/sidebar/ChatSidebar.tsx
crates/dashboard/frontend/src/app/chat/sidebar/ConnectionStatus.tsx
crates/dashboard/frontend/src/app/chat/sidebar/QuickTasks.tsx
crates/dashboard/frontend/src/app/chat/sidebar/SidebarSection.tsx
crates/dashboard/frontend/src/app/chat/sidebar/ToolActivityPanel.tsx
crates/dashboard/frontend/src/app/chat/sidebar/ToolCallList.tsx
crates/dashboard/frontend/src/app/chat/sidebar/UpcomingEvents.tsx
crates/dashboard/frontend/src/app/chat/utils.ts
```

**Step 2: Verify old Chat.tsx is gone**

Run: `test -f crates/dashboard/frontend/src/app/pages/Chat.tsx && echo "STILL EXISTS" || echo "DELETED"`
Expected: `DELETED`

**Step 3: Run full test suite one final time**

Run: `cd crates/dashboard/frontend && npx vitest run 2>&1 | tail -10`
Expected: All tests pass
