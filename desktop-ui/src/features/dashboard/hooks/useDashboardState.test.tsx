// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useDashboardStateImpl } from "./useDashboardState";

describe("useDashboardStateImpl", () => {
  it("defaults to day mode and today's date", () => {
    const { result } = renderHook(() => useDashboardStateImpl());
    expect(result.current.mode).toBe("day");
    expect(result.current.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("navigatePrev moves day back one day", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "day", date: "2026-04-30" }));
    act(() => result.current.navigatePrev());
    expect(result.current.date).toBe("2026-04-29");
  });

  it("navigateNext moves week forward seven days", () => {
    const { result } = renderHook(() =>
      useDashboardStateImpl({ mode: "week", date: "2026-04-27" }),
    );
    act(() => result.current.navigateNext());
    expect(result.current.date).toBe("2026-05-04");
  });

  it("navigatePrev moves month back one month", () => {
    const { result } = renderHook(() =>
      useDashboardStateImpl({ mode: "month", date: "2026-04-30" }),
    );
    act(() => result.current.navigatePrev());
    expect(result.current.date.slice(0, 7)).toBe("2026-03");
  });

  it("setMode('year') keeps the year portion of the date", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "day", date: "2026-04-30" }));
    act(() => result.current.setMode("year"));
    expect(result.current.mode).toBe("year");
    expect(result.current.date).toBe("2026");
  });

  it("navigateNext in year mode moves forward one year", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "year", date: "2026" }));
    act(() => result.current.navigateNext());
    expect(result.current.date).toBe("2027");
  });

  it("navigateToday returns to today's date and day mode", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "year", date: "2020" }));
    act(() => result.current.navigateToday());
    expect(result.current.mode).toBe("day");
    expect(result.current.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
