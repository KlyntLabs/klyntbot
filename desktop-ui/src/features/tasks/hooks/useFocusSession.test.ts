import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useFocusSession } from "./useFocusSession";

afterEach(() => vi.useRealTimers());

describe("useFocusSession", () => {
  it("is inactive when focusedAt is null", () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.isActive).toBe(false);
    expect(result.current.elapsedSecs).toBe(0);
  });

  it("is active when focusedAt is set", () => {
    const focusedAt = new Date(Date.now() - 60_000).toISOString();
    const { result } = renderHook(() => useFocusSession(focusedAt));
    expect(result.current.isActive).toBe(true);
    expect(result.current.elapsedSecs).toBeGreaterThanOrEqual(59);
  });

  it("timer increments via setInterval", () => {
    vi.useFakeTimers();
    const focusedAt = new Date().toISOString();
    const { result } = renderHook(() => useFocusSession(focusedAt));
    act(() => vi.advanceTimersByTime(5000));
    expect(result.current.elapsedSecs).toBeGreaterThanOrEqual(5);
  });

  it("resets elapsed to 0 when focusedAt becomes null", () => {
    vi.useFakeTimers();
    const focusedAt = new Date().toISOString();
    const { result, rerender } = renderHook(({ f }: { f: string | null }) => useFocusSession(f), {
      initialProps: { f: focusedAt },
    });
    act(() => vi.advanceTimersByTime(3000));
    rerender({ f: null });
    expect(result.current.elapsedSecs).toBe(0);
    expect(result.current.isActive).toBe(false);
  });

  it("formatElapsed: mm:ss under one hour", () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.formatElapsed(90)).toBe("01:30");
    expect(result.current.formatElapsed(0)).toBe("00:00");
    expect(result.current.formatElapsed(3599)).toBe("59:59");
  });

  it("formatElapsed: h:mm:ss at one hour or more", () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.formatElapsed(3600)).toBe("1:00:00");
    expect(result.current.formatElapsed(3661)).toBe("1:01:01");
  });
});
