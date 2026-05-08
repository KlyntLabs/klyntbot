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

type PlanModeState = {
  planSessionId: string;
  planFileSlug: string;
  planFilePath: string;
  proposedItemCount: number;
};

type TodoState = {
  items: TodoItem[];
  planModeState: PlanModeState | null;
};

const stores = new Map<string, TodoState>();
const listeners = new Map<string, Set<() => void>>();

function getStore(threadId: string): TodoState {
  if (!stores.has(threadId)) {
    stores.set(threadId, { items: [], planModeState: null });
  }
  return stores.get(threadId)!;
}

function emit(threadId: string) {
  listeners.get(threadId)?.forEach((fn) => fn());
}

export type { PlanModeState };

export function setTodos(threadId: string, items: TodoItem[]) {
  const prev = getStore(threadId);
  if (prev.items === items && prev.planModeState === null) return;
  stores.set(threadId, { ...prev, items });
  emit(threadId);
}

export function setPlanModeState(threadId: string, planModeState: PlanModeState | null) {
  const prev = getStore(threadId);
  if (prev.planModeState === planModeState) return;
  stores.set(threadId, { ...prev, planModeState });
  emit(threadId);
}

export function applyView(
  threadId: string,
  view: { agents: Record<string, TodoItem[]>; planModeState: PlanModeState | null }
) {
  const prev = getStore(threadId);
  const items = Object.values(view.agents).flat();
  const planModeState = view.planModeState ?? null;
  // Quick path: skip emit if counts and plan state haven't changed.
  if (prev.items.length === items.length && prev.planModeState === planModeState) {
    return;
  }
  stores.set(threadId, { items, planModeState });
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
      return () => {
        const set = listeners.get(threadId);
        if (set) set.delete(cb);
      };
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
