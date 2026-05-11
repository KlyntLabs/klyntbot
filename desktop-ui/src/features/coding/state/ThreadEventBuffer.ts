import { useSyncExternalStore } from "react";
import { useChatStore } from "@/features/threads/store/useChatStore";
import type { ThreadEvent } from "./codingEventReducer";

const RING_BUFFER_CAP = 500;
const RECENT_WINDOW_MS = 30 * 60 * 1000;
// If a thread has been "running" but no event arrives for this long,
// treat it as silently dead so the composer's isProcessing flag doesn't
// trap sends in queue/steer mode forever (e.g. after a backend crash or
// a turn that never emitted TurnCompleted).
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
    const store = useChatStore.getState();
    const next = new Set(store.codingRunningIds);
    if (next.delete(threadId)) {
      store.setCodingRunningIds(next);
    }
  }, RUNNING_HEARTBEAT_MS);
  runningHeartbeats.set(threadId, timer);
}

function maybePruneThreadBuffer(threadId: string): void {
  const store = useChatStore.getState();
  if (
    !threadSubscribers.has(threadId) &&
    !store.codingRunningIds.has(threadId) &&
    !recentlyCompleted.has(threadId)
  ) {
    eventsByThread.delete(threadId);
  }
}

function rebuildSnapshots(): void {
  const store = useChatStore.getState();
  const recent = new Map<string, number>();
  for (const [id, entry] of recentlyCompleted) recent.set(id, entry.finishedAt);
  store.setCodingRecentlyCompleted(recent);
}

function notify(): void {
  rebuildSnapshots();
}

function subscribeStore(listener: () => void): () => void {
  return useChatStore.subscribe((state, prevState) => {
    if (
      state.codingRunningIds !== prevState.codingRunningIds ||
      state.codingRecentlyCompleted !== prevState.codingRecentlyCompleted
    ) {
      listener();
    }
  });
}

export function getRunningIds(): ReadonlySet<string> {
  return useChatStore.getState().codingRunningIds;
}

export function getRecentlyCompleted(): ReadonlyMap<string, number> {
  return useChatStore.getState().codingRecentlyCompleted;
}

export function useRunningCodingIds(): ReadonlySet<string> {
  return useSyncExternalStore(subscribeStore, getRunningIds, getRunningIds);
}

export function useRecentlyCompletedCodingIds(): ReadonlyMap<string, number> {
  return useSyncExternalStore(subscribeStore, getRecentlyCompleted, getRecentlyCompleted);
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

  const store = useChatStore.getState();
  let stateChanged = false;

  if (event.kind === "turn_started") {
    if (!store.codingRunningIds.has(threadId)) {
      store.setCodingRunningIds(new Set([...store.codingRunningIds, threadId]));
      stateChanged = true;
    }
    armHeartbeat(threadId);
    const prev = recentlyCompleted.get(threadId);
    if (prev) {
      if (prev.timer) clearTimeout(prev.timer);
      recentlyCompleted.delete(threadId);
      stateChanged = true;
    }
  } else if (event.kind === "turn_completed") {
    clearHeartbeat(threadId);
    if (store.codingRunningIds.has(threadId)) {
      const next = new Set(store.codingRunningIds);
      next.delete(threadId);
      store.setCodingRunningIds(next);
      stateChanged = true;
    }
    const prev = recentlyCompleted.get(threadId);
    if (prev?.timer) clearTimeout(prev.timer);
    const timer = setTimeout(() => {
      recentlyCompleted.delete(threadId);
      maybePruneThreadBuffer(threadId);
      notify();
    }, RECENT_WINDOW_MS);
    recentlyCompleted.set(threadId, {
      finishedAt: (event as { completed_at?: number }).completed_at ?? Date.now(),
      timer,
    });
    stateChanged = true;
  }

  // Any event for a running thread proves the loop is alive — rearm the
  // heartbeat so a single quiet stretch doesn't kick a busy thread out of
  // the running set.
  if (store.codingRunningIds.has(threadId) && event.kind !== "turn_completed") {
    armHeartbeat(threadId);
  }

  // Apply to global Zustand store
  useChatStore.getState().applyCodingThreadEvent(threadId, event);

  // Fan-out to per-thread subscribers
  const subs = threadSubscribers.get(threadId);
  if (subs) {
    for (const cb of subs) cb(event);
  }

  if (stateChanged) {
    notify();
  }
}

export function markThreadOpened(threadId: string): void {
  const entry = recentlyCompleted.get(threadId);
  if (!entry) return;
  if (entry.timer) clearTimeout(entry.timer);
  recentlyCompleted.delete(threadId);
  maybePruneThreadBuffer(threadId);
  notify();
}

export function subscribeToThread(threadId: string, onEvent: Subscriber): () => void {
  // Drain buffered events synchronously
  const buf = eventsByThread.get(threadId);
  if (buf) {
    for (const e of buf) onEvent(e);
  }
  // Register for live events
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
    const store = useChatStore.getState();
    store.setCodingRunningIds(new Set());
    for (const t of runningHeartbeats.values()) clearTimeout(t);
    runningHeartbeats.clear();
    for (const entry of recentlyCompleted.values()) {
      if (entry.timer) clearTimeout(entry.timer);
    }
    recentlyCompleted.clear();
    threadSubscribers.clear();
    store.setCodingRecentlyCompleted(new Map());
  },
  applyEvent(event: ThreadEvent): void {
    applyEvent(event);
  },
};

import { listen } from "@tauri-apps/api/event";

let listenerAttached = false;
let unlistenFn: (() => void) | null = null;

/**
 * Attaches the single global `agent:thread_event` Tauri listener that
 * powers running-state, recently-completed, and per-thread subscribers.
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
