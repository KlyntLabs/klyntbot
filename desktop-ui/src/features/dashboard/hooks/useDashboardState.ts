import { createContext, useCallback, useContext, useState } from "react";
import { shiftDate, todayISO, toLocalISO } from "@/utils/dashboardDates";

export type DashboardViewMode = "day" | "week" | "month" | "year";

export interface DashboardState {
  mode: DashboardViewMode;
  date: string; // YYYY-MM-DD for day/week, YYYY-MM-DD (week-Monday) for week, YYYY-MM-DD for month, YYYY for year
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

/**
 * Internal-only — the context-free hook. Used in tests and by `Dashboard.tsx`
 * (which then exposes the value via `DashboardStateContext`).
 */
export function useDashboardStateImpl(init?: InitArgs): DashboardState {
  const [mode, setModeRaw] = useState<DashboardViewMode>(init?.mode ?? "day");
  const [date, setDate] = useState<string>(init?.date ?? todayISO());

  const setMode = useCallback((next: DashboardViewMode) => {
    setModeRaw((prev) => {
      if (prev === next) return prev;
      // When entering year mode, collapse the date to the year; when leaving year mode, expand to Jan 1 of that year.
      setDate((d) => {
        if (next === "year" && prev !== "year") return d.slice(0, 4);
        if (next !== "year" && prev === "year") return `${d}-01-01`;
        return d;
      });
      return next;
    });
  }, []);

  const navigatePrev = useCallback(() => {
    setDate((d) => stepDate(mode, d, -1));
  }, [mode]);

  const navigateNext = useCallback(() => {
    setDate((d) => stepDate(mode, d, 1));
  }, [mode]);

  const navigateToday = useCallback(() => {
    setModeRaw("day");
    setDate(todayISO());
  }, []);

  return { mode, date, setMode, setDate, navigatePrev, navigateNext, navigateToday };
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
