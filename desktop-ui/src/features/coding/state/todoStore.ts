import { useSyncExternalStore } from "react";

export type TodoItem = {
  id: string;
  title: string;
  status: "pending" | "in_progress" | "done" | "blocked";
  concurrency: "safe" | "sequential" | "exclusive";
  blockedReason?: string;
  blockedBy: string[];
  delegatedTo?: string;
};

type TodoState = {
  items: TodoItem[];
  planMode: boolean;
  planSessionId?: string;
};

const stores = new Map<string, TodoState>();
const listeners = new Map<string, Set<() => void>>();

function getStore(threadId: string): TodoState {
  if (!stores.has(threadId)) {
    stores.set(threadId, { items: [], planMode: false });
  }
  return stores.get(threadId)!;
}

function emit(threadId: string) {
  listeners.get(threadId)?.forEach((fn) => fn());
}

export function setTodos(threadId: string, items: TodoItem[], planMode = false) {
  stores.set(threadId, { items, planMode });
  emit(threadId);
}

/** Remove store and listeners for a thread to prevent unbounded growth. */
export function cleanupTodos(threadId: string) {
  stores.delete(threadId);
  listeners.delete(threadId);
}

export function useTodos(threadId: string): TodoState {
  return useSyncExternalStore(
    (cb) => {
      if (!listeners.has(threadId)) listeners.set(threadId, new Set());
      listeners.get(threadId)!.add(cb);
      return () => listeners.get(threadId)!.delete(cb);
    },
    () => getStore(threadId),
    () => getStore(threadId),
  );
}

export function countPending(items: TodoItem[]): number {
  return items.filter((i) => i.status === "pending").length;
}

export function countBlocked(items: TodoItem[]): number {
  return items.filter((i) => i.status === "blocked").length;
}

export function countInProgress(items: TodoItem[]): number {
  return items.filter((i) => i.status === "in_progress").length;
}
