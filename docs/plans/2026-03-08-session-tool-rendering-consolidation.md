# Session Tool Rendering Consolidation — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace session's heavy tool-call blocks with the chat's compact tool pills via an adapter pattern.

**Architecture:** A `contentBlocksToSegments()` adapter transforms session `ContentBlock[]` → `MessageSegment[]`. `SessionMessageItem` then renders via the chat's `SegmentedMessage` + `MarkdownContent`. Session-specific meta messages keep their own lightweight renderers.

**Tech Stack:** React, TypeScript, existing `SegmentedMessage` and `MarkdownContent` components from `desktop-ui/src/components/chat/`.

---

### Task 1: Create the ContentBlock → MessageSegment adapter

**Files:**
- Create: `desktop-ui/src/lib/adapters.ts`

**Step 1: Create the adapter file**

```typescript
// desktop-ui/src/lib/adapters.ts
import type { MessageSegment } from "./types";
import type { ContentBlock } from "./session-types";

/**
 * Transform session ContentBlock[] (tool_use/tool_result pairs) into
 * MessageSegment[] compatible with the chat's SegmentedMessage renderer.
 */
export function contentBlocksToSegments(blocks: ContentBlock[]): MessageSegment[] {
  const segments: MessageSegment[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];

    if (block.type === "text" && block.text) {
      segments.push({ type: "text", content: block.text });
    } else if (block.type === "tool_use") {
      // Look ahead for a paired tool_result
      const next = blocks[i + 1];
      const result =
        next?.type === "tool_result" && next.toolUseId === block.id ? next : undefined;
      if (result) i++; // consume the paired result

      const resultContent = result?.content;
      const resultStr =
        resultContent == null
          ? undefined
          : typeof resultContent === "string"
            ? resultContent
            : JSON.stringify(resultContent, null, 2);

      segments.push({
        type: "tool",
        name: block.name ?? "unknown",
        success: !result?.isError,
        durationMs: 0,
        result: resultStr,
      });
    }
    // Standalone tool_result blocks (no preceding tool_use) are skipped
  }

  return segments;
}
```

**Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -20`
Expected: No errors related to `adapters.ts`

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/adapters.ts
git commit -m "feat(desktop-ui): add ContentBlock → MessageSegment adapter"
```

---

### Task 2: Hide duration when 0 in SegmentedMessage

**Files:**
- Modify: `desktop-ui/src/components/chat/SegmentedMessage.tsx:76`

**Step 1: Update CompletedToolSegment to hide 0ms duration**

In `CompletedToolSegment`, change the duration span (line 76):

```tsx
// Before:
<span className="text-dim">{formatDuration(segment.durationMs)}</span>

// After:
{segment.durationMs > 0 && (
  <span className="text-dim">{formatDuration(segment.durationMs)}</span>
)}
```

Also update the expanded detail duration (line 86):

```tsx
// Before:
<span>{formatDuration(segment.durationMs)}</span>

// After:
{segment.durationMs > 0 && <span>{formatDuration(segment.durationMs)}</span>}
```

Also update the DelegateGroup duration (line 119):

```tsx
// Before:
<span className="text-dim">{formatDuration(delegate.durationMs)}</span>

// After:
{delegate.durationMs > 0 && (
  <span className="text-dim">{formatDuration(delegate.durationMs)}</span>
)}
```

**Step 2: Verify chat still works**

Run: `cd desktop-ui && bun run build`
Expected: Clean build

**Step 3: Commit**

```bash
git add desktop-ui/src/components/chat/SegmentedMessage.tsx
git commit -m "fix(desktop-ui): hide tool duration when 0ms"
```

---

### Task 3: Rewrite SessionMessageItem to use shared renderer

**Files:**
- Modify: `desktop-ui/src/components/sessions/SessionMessageItem.tsx`

**Step 1: Replace the full file contents**

```tsx
// desktop-ui/src/components/sessions/SessionMessageItem.tsx
import { Pin } from "lucide-react";
import { useMemo } from "react";
import type { SessionMessage } from "../../lib/session-types";
import { contentBlocksToSegments } from "../../lib/adapters";
import { MarkdownContent } from "../chat/MarkdownContent";
import { SegmentedMessage } from "../chat/SegmentedMessage";

interface SessionMessageItemProps {
  message: SessionMessage;
  onPin?: (messageUuid: string) => void;
}

export function SessionMessageItem({ message, onPin }: SessionMessageItemProps) {
  if (message.type === "progress") {
    return (
      <div className="flex justify-center py-0.5">
        <span className="glass-badge text-[10px] text-dim font-light px-2 py-0.5">
          {message.subtype ?? "working..."}
        </span>
      </div>
    );
  }

  if (message.type === "system") {
    return (
      <div className="flex justify-center py-1">
        <span className="text-[11px] text-dim font-light italic">{message.text ?? "system"}</span>
      </div>
    );
  }

  if (message.type === "queueOperation") {
    const queueText = typeof message.content === "string" ? message.content : message.text;
    if (message.operation === "enqueue" && queueText) {
      return (
        <div className="group flex justify-end py-1">
          <div className="max-w-[85%] px-4 py-2.5 text-[13px] font-light glass-bubble-user text-primary">
            <p className="whitespace-pre-wrap break-words">{queueText}</p>
          </div>
        </div>
      );
    }
    return null;
  }

  const isUser = message.type === "user";

  return (
    <div className={`group flex ${isUser ? "justify-end" : "justify-start"} py-1`}>
      <div
        className={`max-w-[85%] px-4 py-2.5 text-[13px] font-light ${
          isUser ? "glass-bubble-user text-primary" : "glass-bubble text-primary"
        }`}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap break-words">{message.text}</p>
        ) : (
          <AssistantContent message={message} />
        )}
      </div>
      {onPin && message.uuid && (
        <button
          type="button"
          onClick={() => onPin(message.uuid as string)}
          title="Pin message"
          className="self-start mt-2 ml-1 opacity-0 group-hover:opacity-100 transition-opacity w-6 h-6 rounded-md flex items-center justify-center text-dim hover:text-muted"
        >
          <Pin className="w-3 h-3" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}

function AssistantContent({ message }: { message: SessionMessage }) {
  const segments = useMemo(
    () => (Array.isArray(message.content) ? contentBlocksToSegments(message.content) : null),
    [message.content],
  );

  if (segments) {
    return <SegmentedMessage segments={segments} />;
  }

  const display = typeof message.content === "string" ? message.content : message.text;
  return display ? <MarkdownContent content={display} /> : null;
}
```

Key changes from the original:
- Removed `renderContentBlocks()` function and `CollapsibleToolBlock` import
- Added `contentBlocksToSegments` adapter + `SegmentedMessage` + `MarkdownContent` imports
- `AssistantContent` now converts `ContentBlock[]` → `MessageSegment[]` → `SegmentedMessage`
- Plain text content now uses `MarkdownContent` instead of bare `<p>` tags
- Added `useMemo` for adapter call

**Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run build`
Expected: Clean build

**Step 3: Commit**

```bash
git add desktop-ui/src/components/sessions/SessionMessageItem.tsx
git commit -m "refactor(desktop-ui): use shared SegmentedMessage for session tool rendering"
```

---

### Task 4: Adjust SessionMirrorView spacing

**Files:**
- Modify: `desktop-ui/src/components/sessions/SessionMirrorView.tsx:46`

**Step 1: Update spacing to match chat's MessageList**

Change `space-y-0.5` to `space-y-4`:

```tsx
// Before:
<div className="space-y-0.5">

// After:
<div className="space-y-4">
```

Note: Chat uses `space-y-6` but sessions have more compact meta messages (progress badges, system messages) so `space-y-4` is a better fit.

**Step 2: Verify visually**

Run: `cd desktop-ui && bun run dev`
Open the sessions page and confirm messages have proper spacing.

**Step 3: Commit**

```bash
git add desktop-ui/src/components/sessions/SessionMirrorView.tsx
git commit -m "style(desktop-ui): increase session message spacing"
```

---

### Task 5: Delete CollapsibleToolBlock

**Files:**
- Delete: `desktop-ui/src/components/sessions/CollapsibleToolBlock.tsx`

**Step 1: Verify no remaining imports**

Run: `cd desktop-ui && grep -r "CollapsibleToolBlock" src/`
Expected: No matches (SessionMessageItem no longer imports it)

**Step 2: Delete the file**

```bash
rm desktop-ui/src/components/sessions/CollapsibleToolBlock.tsx
```

**Step 3: Verify clean build**

Run: `cd desktop-ui && bun run build`
Expected: Clean build, no broken imports

**Step 4: Commit**

```bash
git add desktop-ui/src/components/sessions/CollapsibleToolBlock.tsx
git commit -m "refactor(desktop-ui): remove CollapsibleToolBlock, replaced by shared SegmentedMessage"
```

---

### Task 6: Lint & verify

**Step 1: Run Biome lint/format**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors (auto-fixes applied if any)

**Step 2: Run full build**

Run: `cd desktop-ui && bun run build`
Expected: Clean build

**Step 3: Final commit if lint changed anything**

```bash
git add -A desktop-ui/src/
git commit -m "style(desktop-ui): lint fixes"
```
