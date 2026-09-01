import { useEffect, useRef } from "react";
import type { WorkspaceInfo } from "@/types";

export const REMOTE_WORKSPACE_REFRESH_INTERVAL_MS = 15_000;

type WorkspaceRefreshOptions = {
  workspaces: WorkspaceInfo[];
  refreshWorkspaces: () => Promise<WorkspaceInfo[] | undefined>;
  listThreadsForWorkspaces: (
    workspaces: WorkspaceInfo[],
    options?: { preserveState?: boolean },
  ) => Promise<void>;
  backendMode?: string;
  pollIntervalMs?: number;
};

export function useWorkspaceRefreshOnFocus({
  workspaces,
  refreshWorkspaces,
  listThreadsForWorkspaces,
  backendMode = "local",
  pollIntervalMs = REMOTE_WORKSPACE_REFRESH_INTERVAL_MS,
}: WorkspaceRefreshOptions) {
  const optionsRef = useRef({
    workspaces,
    refreshWorkspaces,
    listThreadsForWorkspaces,
    backendMode,
    pollIntervalMs,
  });
  useEffect(() => {
    optionsRef.current = {
      workspaces,
      refreshWorkspaces,
      listThreadsForWorkspaces,
      backendMode,
      pollIntervalMs,
    };
  });

  const timersRef = useRef<{
    debounceTimer: ReturnType<typeof setTimeout> | null;
    pollTimer: ReturnType<typeof setInterval> | null;
    refreshInFlight: boolean;
    updatePolling: (() => void) | null;
  }>({
    debounceTimer: null,
    pollTimer: null,
    refreshInFlight: false,
    updatePolling: null,
  });

  useEffect(() => {
    const runRefreshCycle = () => {
      if (timersRef.current.refreshInFlight) {
        return;
      }
      timersRef.current.refreshInFlight = true;
      const {
        workspaces: ws,
        refreshWorkspaces: refresh,
        listThreadsForWorkspaces: listThreads,
      } = optionsRef.current;
      void (async () => {
        let latestWorkspaces = ws;
        try {
          const entries = await refresh();
          if (entries) {
            latestWorkspaces = entries;
          }
        } catch {
          // Silent: refresh errors show in debug panel.
        }
        const connected = latestWorkspaces.filter((entry) => entry.connected);
        if (connected.length > 0) {
          await listThreads(connected, { preserveState: true });
        }
      })().finally(() => {
        timersRef.current.refreshInFlight = false;
      });
    };

    const updatePolling = () => {
      if (timersRef.current.pollTimer) {
        clearInterval(timersRef.current.pollTimer);
        timersRef.current.pollTimer = null;
      }
      const { backendMode: currentBackendMode, pollIntervalMs: intervalMs } = optionsRef.current;
      if (currentBackendMode !== "remote" || document.visibilityState !== "visible") {
        return;
      }
      timersRef.current.pollTimer = setInterval(() => {
        runRefreshCycle();
      }, intervalMs);
    };

    const scheduleRefresh = () => {
      if (timersRef.current.debounceTimer) {
        clearTimeout(timersRef.current.debounceTimer);
      }
      timersRef.current.debounceTimer = setTimeout(() => {
        runRefreshCycle();
      }, 500);
    };

    const handleFocus = () => {
      scheduleRefresh();
      updatePolling();
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        scheduleRefresh();
      }
      updatePolling();
    };

    timersRef.current.updatePolling = updatePolling;

    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    updatePolling();
    return () => {
      window.removeEventListener("focus", handleFocus);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      if (timersRef.current.debounceTimer) {
        clearTimeout(timersRef.current.debounceTimer);
      }
      if (timersRef.current.pollTimer) {
        clearInterval(timersRef.current.pollTimer);
      }
      timersRef.current.updatePolling = null;
    };
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: backendMode/pollIntervalMs are inputs whose changes must reconfigure polling
  useEffect(() => {
    timersRef.current.updatePolling?.();
  }, [backendMode, pollIntervalMs]);
}
