// NOTE: This hook receives `workspaces` + `threadsByWorkspace` as props from
// MainApp.tsx, which still owns those slices via local state. When threads
// are migrated to TanStack Query (a follow-up plan after the chat feature
// migrates), this hook should be rewritten as a queryClient.getQueryCache()
// subscriber that listens for `qk.threads.list()` updates and calls
// setTraySessionUsage from the cache. Until then it stays as a prop-driven
// effect.

import { setTraySessionUsage } from "@services/tauri";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef } from "react";
import type { RateLimitSnapshot, TraySessionUsage } from "@/types";
import { getUsageLabels } from "../utils/usageLabels";

const SYNC_DEBOUNCE_MS = 150;

type UseTraySessionUsageParams = {
  accountRateLimits: RateLimitSnapshot | null;
  showRemaining: boolean;
};

export function buildTraySessionUsage(
  accountRateLimits: RateLimitSnapshot | null,
  showRemaining: boolean,
): TraySessionUsage | null {
  const { sessionPercent, weeklyPercent, sessionResetLabel, weeklyResetLabel } = getUsageLabels(
    accountRateLimits,
    showRemaining,
  );
  if (sessionPercent === null) {
    return null;
  }

  const usageLabel = showRemaining ? `${sessionPercent}% remaining` : `${sessionPercent}% used`;
  const weeklyUsageLabel =
    typeof weeklyPercent === "number"
      ? showRemaining
        ? `${weeklyPercent}% remaining`
        : `${weeklyPercent}% used`
      : null;

  return {
    sessionLabel: sessionResetLabel === null ? usageLabel : `${usageLabel} · ${sessionResetLabel}`,
    weeklyLabel:
      weeklyUsageLabel === null
        ? null
        : weeklyResetLabel === null
          ? weeklyUsageLabel
          : `${weeklyUsageLabel} · ${weeklyResetLabel}`,
  };
}

export function useTraySessionUsage({
  accountRateLimits,
  showRemaining,
}: UseTraySessionUsageParams) {
  const usage = useMemo(
    () => buildTraySessionUsage(accountRateLimits, showRemaining),
    [accountRateLimits, showRemaining],
  );
  const lastSyncedUsageRef = useRef<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    const serializedUsage = JSON.stringify(usage);
    if (lastSyncedUsageRef.current === serializedUsage) {
      return;
    }

    let cancelled = false;
    let timeoutId: number | null = null;
    let retries = 0;
    const MAX_RETRIES = 3;

    const attemptSync = () => {
      timeoutId = null;
      void setTraySessionUsage(usage)
        .then(() => {
          if (!cancelled) lastSyncedUsageRef.current = serializedUsage;
        })
        .catch((error) => {
          if (cancelled) return;
          console.warn("[tray] setTraySessionUsage failed", error);
          if (retries < MAX_RETRIES) {
            retries++;
            timeoutId = window.setTimeout(attemptSync, SYNC_DEBOUNCE_MS);
          } else {
            // Exhausted retries — mark as attempted so we don't loop forever.
            lastSyncedUsageRef.current = serializedUsage;
          }
        });
    };

    timeoutId = window.setTimeout(attemptSync, SYNC_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [usage]);
}
