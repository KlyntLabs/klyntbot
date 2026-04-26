// NOTE: This hook receives `workspaces` + `threadsByWorkspace` as props from
// MainApp.tsx, which still owns those slices via local state. When threads
// are migrated to TanStack Query (a follow-up plan after the chat feature
// migrates), this hook should be rewritten as a queryClient.getQueryCache()
// subscriber that listens for `qk.threads.list()` updates and calls
// setTraySessionUsage from the cache. Until then it stays as a prop-driven
// effect.
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef } from "react";
import { setTraySessionUsage } from "@services/tauri";
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
  const {
    sessionPercent,
    weeklyPercent,
    sessionResetLabel,
    weeklyResetLabel,
  } = getUsageLabels(
    accountRateLimits,
    showRemaining,
  );
  if (sessionPercent === null) {
    return null;
  }

  const usageLabel = showRemaining
    ? `${sessionPercent}% remaining`
    : `${sessionPercent}% used`;
  const weeklyUsageLabel =
    typeof weeklyPercent === "number"
      ? showRemaining
        ? `${weeklyPercent}% remaining`
        : `${weeklyPercent}% used`
      : null;

  return {
    sessionLabel:
      sessionResetLabel === null
        ? usageLabel
        : `${usageLabel} · ${sessionResetLabel}`,
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

    const scheduleSync = () => {
      timeoutId = window.setTimeout(() => {
        void setTraySessionUsage(usage)
          .then(() => {
            if (!cancelled) {
              lastSyncedUsageRef.current = serializedUsage;
            }
          })
          .catch(() => {
            if (!cancelled) {
              // Retry until the desktop bridge or tray is ready for the same usage payload.
              scheduleSync();
            }
          });
      }, SYNC_DEBOUNCE_MS);
    };

    scheduleSync();

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [usage]);
}
