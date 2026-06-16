/// Extracted from `features/coding/state/ThreadEventBuffer.ts` and
/// `codingEventReducer.ts` during the unify-to-assistant refactor.
///
/// NOTE: This module is currently vestigial. The assistant uses the legacy
/// v1 streaming architecture (`chatStreamStore.ts`) rather than
/// `agent:thread_event`. The buffer is preserved in case the assistant
/// migrates to the v2 thread-event protocol in the future.
///
/// All Zustand store dependencies have been removed to avoid coupling to
/// the deleted coding-specific store slice.

import { listen } from "@tauri-apps/api/event";

// ---------------------------------------------------------------------------
// Types (from codingEventReducer.ts)
// ---------------------------------------------------------------------------

export type ThreadEvent =
  | { kind: "turn_started"; thread_id: string; turn_id: string; model: string; started_at: number }
  | { kind: "item_started"; thread_id: string; turn_id: string; item: MessageDto }
  | {
      kind: "item_delta";
      thread_id: string;
      turn_id: string;
      item_id: string;
      part_idx: number;
      delta: PartDelta;
    }
  | { kind: "item_completed"; thread_id: string; turn_id: string; item: MessageDto }
  | {
      kind: "tool_call_started";
      thread_id: string;
      turn_id: string;
      item_id: string;
      call_id: string;
      tool: string;
    }
  | {
      kind: "tool_call_completed";
      thread_id: string;
      turn_id: string;
      call_id: string;
      success: boolean;
      duration_ms: number;
    }
  | {
      kind: "file_changed";
      thread_id: string;
      turn_id: string;
      path: string;
      change: string;
      diff_unified: string;
    }
  | {
      kind: "command_executed";
      thread_id: string;
      turn_id: string;
      command: string[];
      exit_code: number | null;
    }
  | {
      kind: "context_compressed";
      thread_id: string;
      turn_id: string;
      before_tokens: number;
      after_tokens: number;
    }
  | {
      kind: "turn_completed";
      thread_id: string;
      turn_id: string;
      finish_reason: unknown;
      completed_at: number;
      duration_ms: number;
    }
  | { kind: "heartbeat"; subscription_id: string; server_time: number }
  | {
      kind: "todos_updated";
      thread_id: string;
      items: Array<{ id: string; title: string; status: string }>;
    };

export type PartDelta =
  | { type: "text"; append: string }
  | { type: "reasoning"; append: string; redacted: boolean }
  | { type: "tool_call_args"; json_patch: unknown }
  | { type: "command_stdout"; append: string }
  | { type: "command_stderr"; append: string }
  | { type: "file_change_progress"; bytes_written: number };

export type MessageDto = {
  id: string;
  session_id: string;
  role: string;
  parts: MessagePart[];
  model: string | null;
  turn_id: string | null;
  created_at: number;
  finish_reason: unknown;
};

export type MessagePart =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string; redacted: boolean }
  | { kind: "tool_call"; id: string; tool: string; args: unknown }
  | { kind: "tool_result"; id: string; result: unknown }
  | {
      kind: "command_execution";
      command: string[];
      cwd: string;
      exit_code: number | null;
      stdout: string;
      stderr: string;
    }
  | {
      kind: "file_change";
      path: string;
      before: string | null;
      after: string;
      diff_unified: string;
      applied: boolean;
    };

export type TurnState =
  | { kind: "idle"; lastFinishReason?: unknown }
  | { kind: "streaming"; turnId: string; model: string; startedAt: number }
  | { kind: "tool_executing"; callId: string; tool: string; startedAt: number };

export type ThreadState = {
  items: MessageDto[];
  turnState: TurnState;
  processingStartedAt: number | null;
  lastDurationMs: number | null;
};

export const initialThreadState: ThreadState = {
  items: [],
  turnState: { kind: "idle" },
  processingStartedAt: null,
  lastDurationMs: null,
};

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

function sameTextParts(a: MessagePart[], b: MessagePart[]): boolean {
  const at = a.find((p) => p.kind === "text");
  const bt = b.find((p) => p.kind === "text");
  return Boolean(at && bt && at.text === bt.text);
}

function appendPartToLatestAssistant(items: MessageDto[], part: MessagePart): MessageDto[] {
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].role === "assistant") {
      const updated = { ...items[i], parts: [...items[i].parts, part] };
      return [...items.slice(0, i), updated, ...items.slice(i + 1)];
    }
  }
  return items;
}

export function applyThreadEvent(state: ThreadState, event: ThreadEvent): ThreadState {
  switch (event.kind) {
    case "turn_started":
      return {
        ...state,
        turnState: {
          kind: "streaming",
          turnId: event.turn_id,
          model: event.model,
          startedAt: event.started_at,
        },
        processingStartedAt: event.started_at,
        lastDurationMs: null,
      };
    case "item_started": {
      const incoming = event.item;
      const exists = state.items.some(
        (m) =>
          m.id === incoming.id ||
          (m.role === "user" && incoming.role === "user" && sameTextParts(m.parts, incoming.parts)),
      );
      if (exists) return state;
      return { ...state, items: [...state.items, incoming] };
    }
    case "item_delta": {
      const items = state.items.map((m) => {
        if (m.id !== event.item_id) return m;
        const parts = [...m.parts];
        const idx = event.part_idx;
        if (event.delta.type === "text") {
          const existing = parts[idx];
          if (existing && existing.kind === "text") {
            parts[idx] = { ...existing, text: existing.text + event.delta.append };
          } else {
            parts[idx] = { kind: "text", text: event.delta.append };
          }
        } else if (event.delta.type === "reasoning") {
          const existing = parts[idx];
          if (existing && existing.kind === "reasoning") {
            parts[idx] = {
              ...existing,
              text: existing.text + event.delta.append,
              redacted: event.delta.redacted,
            };
          } else {
            parts[idx] = {
              kind: "reasoning",
              text: event.delta.append,
              redacted: event.delta.redacted,
            };
          }
        } else if (event.delta.type === "tool_call_args") {
          const existing = parts[idx];
          if (existing && existing.kind === "tool_call") {
            parts[idx] = { ...existing, args: event.delta.json_patch };
          }
        } else if (event.delta.type === "command_stdout") {
          const existing = parts[idx];
          if (existing && existing.kind === "command_execution") {
            parts[idx] = { ...existing, stdout: existing.stdout + event.delta.append };
          }
        } else if (event.delta.type === "command_stderr") {
          const existing = parts[idx];
          if (existing && existing.kind === "command_execution") {
            parts[idx] = { ...existing, stderr: existing.stderr + event.delta.append };
          }
        }
        return { ...m, parts };
      });
      return { ...state, items };
    }
    case "item_completed": {
      const items = state.items.map((m) => {
        if (m.id !== event.item.id) return m;
        const incoming = event.item;
        const parts = incoming.parts.length === 0 ? m.parts : incoming.parts;
        return { ...incoming, parts };
      });
      return { ...state, items };
    }
    case "tool_call_started":
      return {
        ...state,
        turnState: {
          kind: "tool_executing",
          callId: event.call_id,
          tool: event.tool,
          startedAt: state.processingStartedAt ?? Date.now(),
        },
      };
    case "tool_call_completed":
      return state;
    case "file_changed": {
      const items = appendPartToLatestAssistant(state.items, {
        kind: "file_change",
        path: event.path,
        before: null,
        after: "",
        diff_unified: event.diff_unified ?? "",
        applied: true,
      });
      return { ...state, items };
    }
    case "command_executed": {
      const items = appendPartToLatestAssistant(state.items, {
        kind: "command_execution",
        command: event.command,
        cwd: "",
        exit_code: event.exit_code,
        stdout: "",
        stderr: "",
      });
      return { ...state, items };
    }
    case "context_compressed":
    case "heartbeat":
      return state;
    case "turn_completed":
      return {
        ...state,
        turnState: { kind: "idle", lastFinishReason: event.finish_reason },
        processingStartedAt: null,
        lastDurationMs: event.duration_ms,
      };
    case "todos_updated":
      return state;
    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Buffer (from ThreadEventBuffer.ts) — store-independent
// ---------------------------------------------------------------------------

const RING_BUFFER_CAP = 500;
const RECENT_WINDOW_MS = 30 * 60 * 1000;
const RUNNING_HEARTBEAT_MS = 90 * 1000;

type RecentEntry = {
  finishedAt: number;
  timer: ReturnType<typeof setTimeout> | null;
};

type Subscriber = (event: ThreadEvent) => void;

const eventsByThread = new Map<string, ThreadEvent[]>();
const runningHeartbeats = new Map<string, ReturnType<typeof setTimeout>>();
const recentlyCompleted = new Map<string, RecentEntry>();
const threadSubscribers = new Map<string, Set<Subscriber>>();

/** Internal mutable state replaces the former Zustand coding slice. */
const runningIds = new Set<string>();

function clearHeartbeat(threadId: string): void {
  const t = runningHeartbeats.get(threadId);
  if (t) {
    clearTimeout(t);
    runningHeartbeats.delete(threadId);
  }
}

function armHeartbeat(threadId: string): void {
  clearHeartbeat(threadId);
  const timer = setTimeout(() => {
    runningHeartbeats.delete(threadId);
    runningIds.delete(threadId);
  }, RUNNING_HEARTBEAT_MS);
  runningHeartbeats.set(threadId, timer);
}

function maybePruneThreadBuffer(threadId: string): void {
  if (
    !threadSubscribers.has(threadId) &&
    !runningIds.has(threadId) &&
    !recentlyCompleted.has(threadId)
  ) {
    eventsByThread.delete(threadId);
  }
}

function pushToRingBuffer(threadId: string, event: ThreadEvent): void {
  let buf = eventsByThread.get(threadId);
  if (!buf) {
    buf = [];
    eventsByThread.set(threadId, buf);
  }
  buf.push(event);
  if (buf.length > RING_BUFFER_CAP) {
    buf.splice(0, buf.length - RING_BUFFER_CAP);
  }
}

function applyEvent(event: ThreadEvent): void {
  const threadId = (event as { thread_id?: string }).thread_id;
  if (!threadId) return;

  pushToRingBuffer(threadId, event);

  if (event.kind === "turn_started") {
    runningIds.add(threadId);
    armHeartbeat(threadId);
    const prev = recentlyCompleted.get(threadId);
    if (prev?.timer) clearTimeout(prev.timer);
    recentlyCompleted.delete(threadId);
  } else if (event.kind === "turn_completed") {
    clearHeartbeat(threadId);
    runningIds.delete(threadId);
    const prev = recentlyCompleted.get(threadId);
    if (prev?.timer) clearTimeout(prev.timer);
    const timer = setTimeout(() => {
      recentlyCompleted.delete(threadId);
      maybePruneThreadBuffer(threadId);
    }, RECENT_WINDOW_MS);
    recentlyCompleted.set(threadId, {
      finishedAt: (event as { completed_at?: number }).completed_at ?? Date.now(),
      timer,
    });
  }

  if (runningIds.has(threadId) && event.kind !== "turn_completed") {
    armHeartbeat(threadId);
  }

  const subs = threadSubscribers.get(threadId);
  if (subs) {
    for (const cb of subs) cb(event);
  }
}

export function markThreadOpened(threadId: string): void {
  const entry = recentlyCompleted.get(threadId);
  if (!entry) return;
  if (entry.timer) clearTimeout(entry.timer);
  recentlyCompleted.delete(threadId);
  maybePruneThreadBuffer(threadId);
}

export function subscribeToThread(threadId: string, onEvent: Subscriber): () => void {
  const buf = eventsByThread.get(threadId);
  if (buf) {
    for (const e of buf) onEvent(e);
  }
  let subs = threadSubscribers.get(threadId);
  if (!subs) {
    subs = new Set();
    threadSubscribers.set(threadId, subs);
  }
  subs.add(onEvent);
  return () => {
    const cur = threadSubscribers.get(threadId);
    if (!cur) return;
    cur.delete(onEvent);
    if (cur.size === 0) {
      threadSubscribers.delete(threadId);
      maybePruneThreadBuffer(threadId);
    }
  };
}

export const __testing = {
  reset(): void {
    eventsByThread.clear();
    runningIds.clear();
    for (const t of runningHeartbeats.values()) clearTimeout(t);
    runningHeartbeats.clear();
    for (const entry of recentlyCompleted.values()) {
      if (entry.timer) clearTimeout(entry.timer);
    }
    recentlyCompleted.clear();
    threadSubscribers.clear();
  },
  applyEvent(event: ThreadEvent): void {
    applyEvent(event);
  },
};

let listenerAttached = false;
let unlistenFn: (() => void) | null = null;

/**
 * Attaches the single global `agent:thread_event` Tauri listener.
 * Idempotent — calling twice is a no-op.
 *
 * Returns a cleanup function (mainly useful in tests / hot-reload).
 */
export function initThreadEventBuffer(): () => void {
  if (listenerAttached) return () => {};
  listenerAttached = true;
  let cancelled = false;
  listen<ThreadEvent>("agent:thread_event", (msg) => {
    applyEvent(msg.payload);
  }).then((un) => {
    if (cancelled) un();
    else unlistenFn = un;
  });
  return () => {
    cancelled = true;
    unlistenFn?.();
    unlistenFn = null;
    listenerAttached = false;
  };
}
