import { useSyncExternalStore } from "react";

// TODO: switch to @/bindings once tauri-specta bindings are regenerated
export type BashJobView = {
  id: string;
  session_id: string;
  agent_id: string;
  description: string;
  command: string;
  cwd: string;
  status: string;
  started_at: string;
  finished_at: string | null;
  exit_code: number | null;
  failure_kind: string | null;
  failure_detail: string | null;
  failure_extracted: unknown | null;
  total_bytes_emitted: number;
  last_polled_at: string | null;
  last_seen_offset: number;
};

export type BashJobsPanelView = {
  jobs: BashJobView[];
};

const stores = new Map<string, BashJobView[]>();
const listeners = new Map<string, Set<() => void>>();

const EMPTY: readonly BashJobView[] = Object.freeze([]);

function getStore(threadId: string): readonly BashJobView[] {
  return stores.get(threadId) ?? EMPTY;
}

function emit(threadId: string) {
  // biome-ignore lint/suspicious/useIterableCallbackReturn: callback returns void
  listeners.get(threadId)?.forEach((cb) => cb());
}

export function isActiveJob(j: BashJobView): boolean {
  return j.status === "Running" || j.status === "Starting";
}

export function applyJobsView(threadId: string, jobs: BashJobView[]) {
  const prev = stores.get(threadId);
  if (prev && prev.length === jobs.length && prev === jobs) return;
  stores.set(threadId, jobs);
  emit(threadId);
}

export function applyJobUpdate(threadId: string, updated: BashJobView) {
  const prev = stores.get(threadId);
  if (!prev) {
    stores.set(threadId, [updated]);
    emit(threadId);
    return;
  }
  const idx = prev.findIndex((j) => j.id === updated.id);
  if (idx >= 0 && prev[idx] === updated) return;
  const next =
    idx >= 0 ? [...prev.slice(0, idx), updated, ...prev.slice(idx + 1)] : [updated, ...prev];
  stores.set(threadId, next);
  emit(threadId);
}

export function removeJob(threadId: string, jobId: string) {
  const prev = stores.get(threadId);
  if (!prev) return;
  const next = prev.filter((j) => j.id !== jobId);
  if (next.length === prev.length) return;
  stores.set(threadId, next);
  emit(threadId);
}

export function cleanupJobs(threadId: string) {
  stores.delete(threadId);
  listeners.delete(threadId);
}

export function useJobs(threadId: string): readonly BashJobView[] {
  return useSyncExternalStore(
    (cb) => {
      let set = listeners.get(threadId);
      if (!set) {
        set = new Set();
        listeners.set(threadId, set);
      }
      set.add(cb);
      return () => {
        set?.delete(cb);
        if (set && set.size === 0) {
          listeners.delete(threadId);
        }
      };
    },
    () => getStore(threadId),
    () => getStore(threadId),
  );
}
