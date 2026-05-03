import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { MessagePart } from "../components/parts/types";

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
  | { kind: "heartbeat"; subscription_id: string; server_time: number };

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

/// Pure reducer — exhaustive over `ThreadEvent.kind`. Unknown kinds are no-ops.
export function applyThreadEvent(state: State, event: ThreadEvent): State {
  switch (event.kind) {
    case "turn_started":
      return {
        ...state,
        turnState: { kind: "streaming", turnId: event.turn_id, model: event.model },
      };
    case "item_started":
      return { ...state, items: [...state.items, event.item] };
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
        }
        return { ...m, parts };
      });
      return { ...state, items };
    }
    case "item_completed":
      return {
        ...state,
        items: state.items.map((m) => (m.id === event.item.id ? event.item : m)),
      };
    case "tool_call_started":
      return {
        ...state,
        turnState: { kind: "tool_executing", callId: event.call_id, tool: event.tool },
      };
    case "tool_call_completed":
      return state;
    case "file_changed":
    case "command_executed":
    case "context_compressed":
    case "heartbeat":
      return state;
    case "turn_completed":
      return { ...state, turnState: { kind: "idle", lastFinishReason: event.finish_reason } };
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
    if (!threadId) return;
    let unlisten: (() => void) | null = null;
    listen<ThreadEvent>("agent:thread_event", (e) => {
      const evt = e.payload;
      // Filter by thread — only relevant events update state.
      const evThreadId = (evt as { thread_id?: string }).thread_id;
      if (evThreadId && evThreadId !== threadId) return;
      setState((prev) => applyThreadEvent(prev, evt));
    }).then((un) => {
      unlisten = un;
    });
    return () => {
      unlisten?.();
    };
  }, [threadId]);

  return state;
}
