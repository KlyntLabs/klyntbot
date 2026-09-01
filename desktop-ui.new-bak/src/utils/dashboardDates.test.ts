import { describe, expect, it } from "vitest";
import {
  formatFullDate,
  formatHumanDuration,
  formatMonthLabel,
  formatTime,
  LONG_MONTHS,
  minutesSinceMidnight,
  minutesToIso,
  monthEndISO,
  SHORT_MONTHS,
  shiftDate,
  shiftMonth,
  todayISO,
  toLocalISO,
  weekStartISO,
} from "./dashboardDates";

describe("toLocalISO", () => {
  it("formats a Date as YYYY-MM-DD in local timezone", () => {
    expect(toLocalISO(new Date(2026, 3, 30))).toBe("2026-04-30");
  });
});

describe("todayISO", () => {
  it("returns a YYYY-MM-DD string for today", () => {
    expect(todayISO()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});

describe("formatFullDate", () => {
  it("formats as 'Weekday, Month D, YYYY'", () => {
    expect(formatFullDate("2026-04-30")).toBe("Thursday, April 30, 2026");
  });
});

describe("formatMonthLabel", () => {
  it("formats 'YYYY-MM' as 'Month YYYY'", () => {
    expect(formatMonthLabel("2026-04")).toBe("April 2026");
  });
});

describe("weekStartISO", () => {
  it("returns the Monday of the containing week", () => {
    // 2026-04-30 is a Thursday → Monday is 2026-04-27
    expect(weekStartISO("2026-04-30")).toBe("2026-04-27");
  });
  it("handles Sundays correctly (week starts Mon)", () => {
    expect(weekStartISO("2026-05-03")).toBe("2026-04-27");
  });
});

describe("shiftDate", () => {
  it("shifts forward by N days", () => {
    expect(shiftDate("2026-04-30", 3)).toBe("2026-05-03");
  });
  it("shifts backward by N days", () => {
    expect(shiftDate("2026-04-30", -2)).toBe("2026-04-28");
  });
});

describe("shiftMonth", () => {
  it("shifts forward across year boundary", () => {
    expect(shiftMonth("2026-12", 1)).toBe("2027-01");
  });
});

describe("monthEndISO", () => {
  it("returns the last day of a month", () => {
    expect(monthEndISO("2026-02")).toBe("2026-02-28");
  });
});

describe("minutesSinceMidnight", () => {
  it("returns minutes for an ISO timestamp", () => {
    expect(minutesSinceMidnight("2026-04-30T09:30:00")).toBe(570);
  });
});

describe("minutesToIso", () => {
  it("composes a UTC ISO from date+minutes", () => {
    expect(minutesToIso("2026-04-30", 570)).toBe("2026-04-30T09:30:00Z");
  });
  it("clamps out-of-range minutes", () => {
    expect(minutesToIso("2026-04-30", 9999)).toBe("2026-04-30T24:00:00Z");
  });
});

describe("formatHumanDuration", () => {
  it("formats hours and minutes", () => {
    expect(formatHumanDuration(3900)).toBe("1h 5m");
  });
  it("formats minutes only when under an hour", () => {
    expect(formatHumanDuration(600)).toBe("10m");
  });
});

describe("LONG_MONTHS / SHORT_MONTHS", () => {
  it("LONG_MONTHS has 12 entries starting with January", () => {
    expect(LONG_MONTHS.length).toBe(12);
    expect(LONG_MONTHS[0]).toBe("January");
  });
  it("SHORT_MONTHS has 12 entries starting with Jan", () => {
    expect(SHORT_MONTHS.length).toBe(12);
    expect(SHORT_MONTHS[0]).toBe("Jan");
  });
});

describe("formatTime", () => {
  it("returns HH:MM in 24h format", () => {
    expect(formatTime("2026-04-30T09:30:00Z")).toMatch(/^\d{2}:\d{2}$/);
  });
});
