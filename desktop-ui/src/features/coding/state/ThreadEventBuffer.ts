import { useSyncExternalStore } from "react";
import type { ThreadEvent } from "../hooks/useThreadEvents";

const RING_BUFFER_CAP = 500;
const RECENT_WINDOW_MS = 30 * 60 * 1000;

type RecentEntry = {
  finishedAt: number;
  timer: ReturnType<typeof setTimeout> | null;
};

type Subscriber = (event: ThreadEvent) => void;

const eventsByThread = new Map<string, ThreadEvent[]>();
const runningIds = new Set<string>();
const recentlyCompleted = new Map<string, RecentEntry>();
const threadSubscribers = new Map<string, Set<Subscriber>>();

const storeListeners = new Set<() => void>();
let runningSnapshot: ReadonlySet<string> = new Set();
let recentSnapshot: ReadonlyMap<string, number> = new Map();

function maybePruneThreadBuffer(threadId: string): void {
  if (
    !threadSubscribers.has(threadId) &&
    !runningIds.has(threadId) &&
    !recentlyCompleted.has(threadId)
  ) {
    eventsByThread.delete(threadId);
  }
}

function rebuildSnapshots(): void {
  runningSnapshot = new Set(runningIds);
  const m = new Map<string, number>();
  for (const [id, entry] of recentlyCompleted) m.set(id, entry.finishedAt);
  recentSnapshot = m;
}

function notify(): void {
  rebuildSnapshots();
  for (const l of storeListeners) l();
}

function subscribeStore(listener: () => void): () => void {
  storeListeners.add(listener);
  return () => storeListeners.delete(listener);
}

export function getRunningIds(): ReadonlySet<string> {
  return runningSnapshot;
}

export function getRecentlyCompleted(): ReadonlyMap<string, number> {
  return recentSnapshot;
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

  let stateChanged = false;

  if (event.kind === "turn_started") {
    if (!runningIds.has(threadId)) {
      runningIds.add(threadId);
      stateChanged = true;
    }
    const prev = recentlyCompleted.get(threadId);
    if (prev) {
      if (prev.timer) clearTimeout(prev.timer);
      recentlyCompleted.delete(threadId);
      stateChanged = true;
    }
  } else if (event.kind === "turn_completed") {
    if (runningIds.has(threadId)) {
      runningIds.delete(threadId);
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
    runningIds.clear();
    for (const entry of recentlyCompleted.values()) {
      if (entry.timer) clearTimeout(entry.timer);
    }
    recentlyCompleted.clear();
    threadSubscribers.clear();
    storeListeners.clear();
    rebuildSnapshots();
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
