import {
  formatMonthLabel,
  shiftDate,
  shiftMonth,
  todayISO,
  toLocalISO,
  weekStartISO,
} from "@shared/lib/dates";
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

function defaultPeriodForMode(mode: PeriodMode): string {
  const today = todayISO();
  switch (mode) {
    case "year":
      return today.slice(0, 4);
    case "month":
      return today.slice(0, 7);
    case "week":
      return weekStartISO(today);
    case "day":
      return today;
  }
}

function parseDateISO(iso: string): Date {
  const [y, m, day] = iso.split("-").map(Number);
  return new Date(y, m - 1, day);
}

function computeRange(mode: PeriodMode, period: string): { dateFrom: string; dateTo: string } {
  switch (mode) {
    case "year": {
      const y = period;
      return { dateFrom: `${y}-01-01`, dateTo: `${y}-12-31` };
    }
    case "month": {
      const [y, m] = period.split("-").map(Number);
      const firstDay = new Date(y, m - 1, 1);
      const lastDay = new Date(y, m, 0);
      return { dateFrom: toLocalISO(firstDay), dateTo: toLocalISO(lastDay) };
    }
    case "week": {
      const monday = parseDateISO(period);
      const sunday = new Date(monday);
      sunday.setDate(monday.getDate() + 6);
      return { dateFrom: toLocalISO(monday), dateTo: toLocalISO(sunday) };
    }
    case "day":
      return { dateFrom: period, dateTo: period };
  }
}

function computeLabel(mode: PeriodMode, period: string): string {
  switch (mode) {
    case "year":
      return period;
    case "month":
      return formatMonthLabel(period);
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
    case "year":
      return String(Number(period) + delta);
    case "month":
      return shiftMonth(period, delta);
    case "week": {
      const monday = parseDateISO(period);
      monday.setDate(monday.getDate() + delta * 7);
      return toLocalISO(monday);
    }
    case "day":
      return shiftDate(period, delta);
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
  const todayIso = toLocalISO(today);

  let refDate: Date;
  if (todayIso >= dateFrom && todayIso <= dateTo) {
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
        refDate = parseDateISO(currentPeriod);
        refDate.setDate(refDate.getDate() + 3); // Wednesday
        break;
      case "day":
        refDate = parseDateISO(currentPeriod);
        break;
    }
  }

  const refIso = toLocalISO(refDate);
  switch (newMode) {
    case "year":
      return refIso.slice(0, 4);
    case "month":
      return refIso.slice(0, 7);
    case "week":
      return weekStartISO(refIso);
    case "day":
      return refIso;
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
