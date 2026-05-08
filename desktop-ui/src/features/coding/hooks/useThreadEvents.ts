import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { MessagePart } from "../components/parts/types";
import { applyView, setTodos } from "../state/todoStore";
import { subscribeToThread } from "../state/ThreadEventBuffer";
import { listen } from "@tauri-apps/api/event";

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
  | { kind: "todos_updated"; thread_id: string; items: Array<{ id: string; title: string; status: string }> };

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
  | { kind: "streaming"; turnId: string; model: string }
  | { kind: "tool_executing"; callId: string; tool: string };

type State = {
  items: MessageDto[];
  turnState: TurnState;
};

const initialState: State = {
  items: [],
  turnState: { kind: "idle" },
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
        turnState: { kind: "streaming", turnId: event.turn_id, model: event.model },
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
        turnState: { kind: "tool_executing", callId: event.call_id, tool: event.tool },
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
      return { ...state, turnState: { kind: "idle", lastFinishReason: event.finish_reason } };
    case "todos_updated":
      // Handled by the dedicated todo-refetch effect below,
      // which fetches full items rather than partial thread-event data.
      return state;
    default:
      return state;
  }
}

/// Subscribe to `agent:thread_event` and apply the reducer. Returns the
/// latest items + turnState. Filters by `threadId` so multiple coding tabs
/// don't bleed into one another.
export function useThreadEvents(threadId: string | null) {
  const [state, setState] = useState<State>(initialState);

  useEffect(() => {
    setState(initialState);
    if (!threadId) return;
    let cancelled = false;

    // Seed persisted history from DB (pre-buffer events).
    invoke<{ items?: MessageDto[] }>("coding_thread_resume", {
      threadId,
      includeItems: true,
    })
      .then((thread) => {
        if (cancelled) return;
        const items = thread?.items ?? [];
        if (items.length === 0) return;
        setState((prev) => ({ ...prev, items }));
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

  useEffect(() => {
    if (!threadId) return;

    let refreshing = false;

    const eventNames = ["coding:todos_updated", "coding:plan_entered", "coding:plan_updated", "coding:plan_exited"];
    const handlers = eventNames.map((name) =>
      listen(name, (e) => {
        const payload = e.payload as any;
        const matches = name === "coding:todos_updated" ? payload?.thread_id === threadId : payload === threadId;
        if (matches) refresh();
      }),
    );

    async function refresh() {
      if (refreshing) return;
      refreshing = true;
      try {
        const view = await invoke("coding_todo_get", { threadId });
        applyView(threadId, view as any);
      } finally {
        refreshing = false;
      }
    }

    return () => {
      handlers.forEach((h) => h.then((fn) => fn()));
    };
  }, [threadId]);

  /// Optimistically render a user message before the backend's echo arrives.
  /// Avoids the listener-registration race where the FE-published `ItemStarted`
  /// for the user's own turn would otherwise be dropped.
  const pushUserMessage = useCallback((text: string) => {
    setState((prev) => ({
      ...prev,
      items: [
        ...prev.items,
        {
          id: `local-user-${Date.now()}`,
          session_id: "",
          role: "user",
          parts: [{ kind: "text", text }],
          model: null,
          turn_id: null,
          created_at: Date.now(),
          finish_reason: null,
        },
      ],
    }));
  }, []);

  return { ...state, pushUserMessage };
}
