import { useCallback, useMemo, useState } from "react";
import { useSearchParams } from "react-router";

export type PeriodMode = "day" | "week" | "month" | "year";

export interface PeriodState {
  mode: PeriodMode;
  period: string;
  label: string;
  dateFrom: string;
  dateTo: string;
  setMode: (mode: PeriodMode) => void;
  prev: () => void;
  next: () => void;
  selectDay: (date: string | null) => void;
  selectedDay: string | null;
}

function _todayISO(): string {
  const d = new Date();
  return toISO(d);
}

function toISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function defaultPeriodForMode(mode: PeriodMode): string {
  const today = new Date();
  const y = today.getFullYear();
  const m = String(today.getMonth() + 1).padStart(2, "0");
  const d = String(today.getDate()).padStart(2, "0");
  switch (mode) {
    case "year":
      return String(y);
    case "month":
      return `${y}-${m}`;
    case "week": {
      // Return ISO week start (Monday) as YYYY-MM-DD
      const monday = getMondayOfDate(today);
      return toISO(monday);
    }
    case "day":
      return `${y}-${m}-${d}`;
  }
}

function getMondayOfDate(d: Date): Date {
  const day = d.getDay(); // 0=Sun, 1=Mon, ...
  const diff = day === 0 ? -6 : 1 - day;
  const monday = new Date(d);
  monday.setDate(d.getDate() + diff);
  return monday;
}

function parseDateISO(iso: string): Date {
  // Parse as local date (not UTC) by splitting
  const [y, m, day] = iso.split("-").map(Number);
  return new Date(y, m - 1, day);
}

function computeRange(mode: PeriodMode, period: string): { dateFrom: string; dateTo: string } {
  switch (mode) {
    case "year": {
      const y = period; // "2026"
      return { dateFrom: `${y}-01-01`, dateTo: `${y}-12-31` };
    }
    case "month": {
      const [y, m] = period.split("-").map(Number);
      const firstDay = new Date(y, m - 1, 1);
      const lastDay = new Date(y, m, 0); // day 0 of next month = last day of this month
      return { dateFrom: toISO(firstDay), dateTo: toISO(lastDay) };
    }
    case "week": {
      const monday = parseDateISO(period);
      const sunday = new Date(monday);
      sunday.setDate(monday.getDate() + 6);
      return { dateFrom: toISO(monday), dateTo: toISO(sunday) };
    }
    case "day":
      return { dateFrom: period, dateTo: period };
  }
}

function computeLabel(mode: PeriodMode, period: string): string {
  switch (mode) {
    case "year":
      return period;
    case "month": {
      const [y, m] = period.split("-").map(Number);
      const d = new Date(y, m - 1, 1);
      return new Intl.DateTimeFormat("en-US", {
        month: "long",
        year: "numeric",
      }).format(d);
    }
    case "week": {
      const monday = parseDateISO(period);
      const sunday = new Date(monday);
      sunday.setDate(monday.getDate() + 6);
      const startFmt = new Intl.DateTimeFormat("en-US", {
        month: "short",
        day: "numeric",
      }).format(monday);
      const endFmt = new Intl.DateTimeFormat("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric",
      }).format(sunday);
      // "Mar 10 – Mar 16, 2026" → normalize to "Mar 10–16, 2026" if same month
      if (monday.getMonth() === sunday.getMonth()) {
        const month = new Intl.DateTimeFormat("en-US", {
          month: "short",
        }).format(monday);
        const year = sunday.getFullYear();
        return `${month} ${monday.getDate()}–${sunday.getDate()}, ${year}`;
      }
      return `${startFmt}–${endFmt}`;
    }
    case "day": {
      const d = parseDateISO(period);
      return new Intl.DateTimeFormat("en-US", {
        month: "long",
        day: "numeric",
        year: "numeric",
      }).format(d);
    }
  }
}

function shiftPeriod(mode: PeriodMode, period: string, delta: number): string {
  switch (mode) {
    case "year": {
      const y = Number(period) + delta;
      return String(y);
    }
    case "month": {
      const [y, m] = period.split("-").map(Number);
      const d = new Date(y, m - 1 + delta, 1);
      const ny = d.getFullYear();
      const nm = String(d.getMonth() + 1).padStart(2, "0");
      return `${ny}-${nm}`;
    }
    case "week": {
      const monday = parseDateISO(period);
      monday.setDate(monday.getDate() + delta * 7);
      return toISO(monday);
    }
    case "day": {
      const d = parseDateISO(period);
      d.setDate(d.getDate() + delta);
      return toISO(d);
    }
  }
}

function adaptPeriodToMode(
  currentPeriod: string,
  currentMode: PeriodMode,
  newMode: PeriodMode,
): string {
  // Use today as reference if it falls within the current period, otherwise
  // pick a safe midpoint to avoid edge-case jumps (e.g. month 1st on Sunday
  // pushing week mode into the previous month).
  const { dateFrom, dateTo } = computeRange(currentMode, currentPeriod);
  const today = new Date();
  const todayISO = toISO(today);

  let refDate: Date;
  if (todayISO >= dateFrom && todayISO <= dateTo) {
    refDate = today;
  } else {
    switch (currentMode) {
      case "year":
        refDate = new Date(Number(currentPeriod), 6, 1); // July 1 (mid-year)
        break;
      case "month": {
        const [y, m] = currentPeriod.split("-").map(Number);
        refDate = new Date(y, m - 1, 15); // 15th (mid-month)
        break;
      }
      case "week":
        // Wednesday (mid-week) — offset +3 from Monday
        refDate = parseDateISO(currentPeriod);
        refDate.setDate(refDate.getDate() + 3);
        break;
      case "day":
        refDate = parseDateISO(currentPeriod);
        break;
    }
  }

  const y = refDate.getFullYear();
  const mo = String(refDate.getMonth() + 1).padStart(2, "0");
  const d = String(refDate.getDate()).padStart(2, "0");
  switch (newMode) {
    case "year":
      return String(y);
    case "month":
      return `${y}-${mo}`;
    case "week":
      return toISO(getMondayOfDate(refDate));
    case "day":
      return `${y}-${mo}-${d}`;
  }
}

export function usePeriodState(): PeriodState {
  const [searchParams, setSearchParams] = useSearchParams();

  const mode = (searchParams.get("mode") as PeriodMode | null) ?? "month";
  const period = searchParams.get("period") ?? defaultPeriodForMode(mode);

  const [selectedDay, setSelectedDay] = useState<string | null>(null);

  const setMode = useCallback(
    (newMode: PeriodMode) => {
      const newPeriod = adaptPeriodToMode(period, mode, newMode);
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        next.set("mode", newMode);
        next.set("period", newPeriod);
        return next;
      });
      setSelectedDay(null);
    },
    [mode, period, setSearchParams],
  );

  const prev = useCallback(() => {
    const newPeriod = shiftPeriod(mode, period, -1);
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set("period", newPeriod);
      return next;
    });
    setSelectedDay(null);
  }, [mode, period, setSearchParams]);

  const next = useCallback(() => {
    const newPeriod = shiftPeriod(mode, period, 1);
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set("period", newPeriod);
      return next;
    });
    setSelectedDay(null);
  }, [mode, period, setSearchParams]);

  const selectDay = useCallback((date: string | null) => {
    setSelectedDay(date);
  }, []);

  const { dateFrom, dateTo } = useMemo(() => computeRange(mode, period), [mode, period]);

  const label = useMemo(() => computeLabel(mode, period), [mode, period]);

  return {
    mode,
    period,
    label,
    dateFrom,
    dateTo,
    setMode,
    prev,
    next,
    selectDay,
    selectedDay,
  };
}
