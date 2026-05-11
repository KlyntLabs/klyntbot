import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { fetchCodingTodos } from "@/api/endpoints/coding";
import type { MessagePart } from "../components/parts/types";
import { subscribeToThread } from "../state/ThreadEventBuffer";
import { applyView } from "../state/todoStore";

const OPTIMISTIC_USER_PREFIX = "local-user-";

function makeOptimisticUserMessage(threadId: string, text: string): MessageDto {
  return {
    id: `${OPTIMISTIC_USER_PREFIX}${Date.now()}`,
    session_id: threadId,
    role: "user",
    parts: [{ kind: "text", text }],
    model: null,
    turn_id: null,
    created_at: Date.now(),
    finish_reason: null,
  };
}

/// Mirrors `desktop_shared::coding::events::ThreadEvent`. Kept loose (string
/// kind discriminator) so additions on the Rust side don't immediately fail
/// the build — unknown kinds are ignored by the reducer.
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

export type TurnState =
  | { kind: "idle"; lastFinishReason?: unknown }
  | { kind: "streaming"; turnId: string; model: string; startedAt: number }
  | { kind: "tool_executing"; callId: string; tool: string; startedAt: number };

type State = {
  items: MessageDto[];
  turnState: TurnState;
  // Wall-clock ms when the current turn began. Mirrored on `turnState` for
  // streaming/tool variants, hoisted here so consumers don't have to discriminate.
  // Cleared back to null when the turn completes.
  processingStartedAt: number | null;
  // Duration of the most recently completed turn, surfaced by the "Done in
  // …" indicator after streaming finishes.
  lastDurationMs: number | null;
};

const initialState: State = {
  items: [],
  turnState: { kind: "idle" },
  processingStartedAt: null,
  lastDurationMs: null,
};

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

/// Pure reducer — exhaustive over `ThreadEvent.kind`. Unknown kinds are no-ops.
export function applyThreadEvent(state: State, event: ThreadEvent): State {
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
        // Clear the previous turn's duration the moment a new turn begins,
        // otherwise the "Done in …" indicator would briefly flash next to
        // the live working spinner.
        lastDurationMs: null,
      };
    case "item_started": {
      // Dedupe: backend may echo a user message that the FE already pushed
      // optimistically (same id), or content (text) we just rendered.
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
      // Merge instead of replace: the BE bridge sends a "completed" item with
      // empty parts when the body was streamed via deltas. Replacing would
      // wipe the assistant text we just rendered.
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
          // Preserve the original turn start so the timer keeps ticking
          // through tool execution rather than resetting to 0:00 each call.
          startedAt: state.processingStartedAt ?? Date.now(),
        },
      };
    case "tool_call_completed":
      // Returning to "streaming" (kind, model unknown to this event) would
      // require synthesizing data we don't have. Leave `turnState` alone —
      // the next `item_delta` keeps the spinner alive, and `turn_completed`
      // clears it. `processingStartedAt` is unchanged so the timer continues.
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
      // Handled by the dedicated todo-refetch effect below,
      // which fetches full items rather than partial thread-event data.
      return state;
    default:
      return state;
  }
}

/// `seedThreadPrompt` is consumed exactly once per threadId change: it injects
/// the user's first message into initial state, sidestepping the SSE race
/// between subscription registration and the backend's `ItemStarted` echo.
export function useThreadEvents(threadId: string | null, seedThreadPrompt?: string | null) {
  const [state, setState] = useState<State>(initialState);
  const seedAppliedRef = useRef<Record<string, boolean>>({});

  // Inject optimistic user message when seedThreadPrompt arrives.
  // Runs independently of the subscription effect so a late-arriving
  // draftPrompt (common after thread-start + setState batching races)
  // still gets rendered.
  useEffect(() => {
    if (!seedThreadPrompt || !threadId) return;
    if (seedAppliedRef.current[threadId]) return;
    seedAppliedRef.current[threadId] = true;
    setState((prev) => {
      const exists = prev.items.some(
        (m) =>
          m.id.startsWith(OPTIMISTIC_USER_PREFIX) &&
          m.parts.some((p) => p.kind === "text" && p.text === seedThreadPrompt),
      );
      if (exists) return prev;
      return { ...prev, items: [...prev.items, makeOptimisticUserMessage(threadId, seedThreadPrompt)] };
    });
  }, [threadId, seedThreadPrompt]);

  useEffect(() => {
    if (!threadId) {
      setState(initialState);
      return;
    }
    let cancelled = false;

    invoke<{ items?: MessageDto[] }>("coding_thread_resume", {
      threadId,
      includeItems: true,
    })
      .then((thread: { items?: MessageDto[] }) => {
        if (cancelled) return;
        const items = thread?.items ?? [];
        if (items.length === 0) return;
        setState((prev) => {
          const hasOptimistic = prev.items.some((it) => it.id.startsWith(OPTIMISTIC_USER_PREFIX));
          if (!hasOptimistic) return { ...prev, items };
          const serverIds = new Set(items.map((it) => it.id));
          const localOnly = prev.items.filter(
            (it) => it.id.startsWith(OPTIMISTIC_USER_PREFIX) && !serverIds.has(it.id),
          );
          return { ...prev, items: [...items, ...localOnly] };
        });
      })
      .catch(() => {});

    // Subscribe via the global buffer — drains buffered events first,
    // then receives live ones. Replaces the per-mount listen() call.
    const unsubscribe = subscribeToThread(threadId, (evt) => {
      if (cancelled) return;
      setState((prev) => applyThreadEvent(prev, evt));
    });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [threadId]);

  // Clean up seed tracking for unmounted threads to avoid unbounded growth.
  useEffect(() => {
    if (!threadId) return;
    return () => {
      delete seedAppliedRef.current[threadId];
    };
  }, [threadId]);

  useEffect(() => {
    if (!threadId) return;

    let refreshing = false;
    let cancelled = false;

    const eventNames = [
      "coding:todos_updated",
      "coding:plan_entered",
      "coding:plan_updated",
      "coding:plan_exited",
    ];
    const handlers = eventNames.map((name) =>
      listen(name, (e) => {
        if (cancelled) return;
        const payload = e.payload as any;
        const matches =
          name === "coding:todos_updated" ? payload?.thread_id === threadId : payload === threadId;
        if (matches) refresh();
      }),
    );

    async function refresh() {
      if (refreshing) return;
      refreshing = true;
      try {
        const view = await fetchCodingTodos(threadId!);
        applyView(threadId!, view as any);
      } finally {
        refreshing = false;
      }
    }

    return () => {
      cancelled = true;
      // listen() returns a Promise<UnlistenFn>; settle each before invoking
      // so listeners that register after unmount still get torn down.
      for (const h of handlers) h.then((fn) => fn());
    };
  }, [threadId]);

  return state;
}
