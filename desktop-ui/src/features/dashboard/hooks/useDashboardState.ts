import { createContext, useCallback, useContext, useState } from "react";
import { shiftDate, todayISO, toLocalISO } from "@/utils/dashboardDates";

export type DashboardViewMode = "day" | "week" | "month" | "year";

export interface DashboardState {
  mode: DashboardViewMode;
  date: string; // YYYY-MM-DD for day/week/month, YYYY for year
  setMode(m: DashboardViewMode): void;
  setDate(d: string): void;
  navigatePrev(): void;
  navigateNext(): void;
  navigateToday(): void;
}

interface InitArgs {
  mode?: DashboardViewMode;
  date?: string;
}

interface CoreState {
  mode: DashboardViewMode;
  date: string;
}

/**
 * Internal-only — the context-free hook. Used in tests and by `Dashboard.tsx`
 * (which then exposes the value via `DashboardStateContext`).
 *
 * State is held as a single `{mode, date}` object so transitions stay atomic.
 * This is required: `setMode` derives the new `date` from the prior `mode`,
 * and React Strict Mode double-invokes updater functions to detect impurity —
 * holding mode and date in separate `useState` slots and calling `setDate`
 * inside `setMode`'s updater would corrupt the date on every transition.
 */
export function useDashboardStateImpl(init?: InitArgs): DashboardState {
  const [{ mode, date }, setCore] = useState<CoreState>({
    mode: init?.mode ?? "day",
    date: init?.date ?? todayISO(),
  });

  const setMode = useCallback((next: DashboardViewMode) => {
    setCore((prev) => {
      if (prev.mode === next) return prev;
      return { mode: next, date: convertDateForMode(prev.date, prev.mode, next) };
    });
  }, []);

  const setDate = useCallback((next: string) => {
    setCore((prev) => (prev.date === next ? prev : { ...prev, date: next }));
  }, []);

  const navigatePrev = useCallback(() => {
    setCore((prev) => ({ ...prev, date: stepDate(prev.mode, prev.date, -1) }));
  }, []);

  const navigateNext = useCallback(() => {
    setCore((prev) => ({ ...prev, date: stepDate(prev.mode, prev.date, 1) }));
  }, []);

  const navigateToday = useCallback(() => {
    setCore({ mode: "day", date: todayISO() });
  }, []);

  return { mode, date, setMode, setDate, navigatePrev, navigateNext, navigateToday };
}

/**
 * Convert a date string to the canonical form for `next` mode. Idempotent —
 * safe to call repeatedly with the same arguments without corrupting the value.
 */
function convertDateForMode(
  date: string,
  prev: DashboardViewMode,
  next: DashboardViewMode,
): string {
  if (next === "year" && prev !== "year") {
    // Day/Week/Month → Year: strip to YYYY (only if not already 4-char).
    return /^\d{4}$/.test(date) ? date : date.slice(0, 4);
  }
  if (next !== "year" && prev === "year") {
    // Year → Day/Week/Month: expand YYYY to YYYY-01-01 (only if currently bare year).
    return /^\d{4}$/.test(date) ? `${date}-01-01` : date;
  }
  return date;
}

function stepDate(mode: DashboardViewMode, date: string, dir: 1 | -1): string {
  switch (mode) {
    case "day":
      return shiftDate(date, dir);
    case "week":
      return shiftDate(date, 7 * dir);
    case "month": {
      const [y, m] = date.split("-").map(Number);
      const d = new Date(y, m - 1 + dir, 1);
      return toLocalISO(d);
    }
    case "year": {
      const y = Number(date.slice(0, 4));
      if (Number.isNaN(y)) return String(new Date().getFullYear() + dir);
      return String(y + dir);
    }
  }
}

export const DashboardStateContext = createContext<DashboardState | null>(null);

export function useDashboardState(): DashboardState {
  const ctx = useContext(DashboardStateContext);
  if (!ctx)
    throw new Error("useDashboardState must be used inside <DashboardStateContext.Provider>");
  return ctx;
}
