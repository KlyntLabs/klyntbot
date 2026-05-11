import { renderHook } from "@testing-library/react";
import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useThreadWatchdog } from "./useThreadWatchdog";

describe("useThreadWatchdog", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("does not fire while not processing", () => {
    const onFire = vi.fn();
    renderHook(() =>
      useThreadWatchdog({ threadId: "t1", isProcessing: false, onFire }),
    );
    act(() => { vi.advanceTimersByTime(100_000); });
    expect(onFire).not.toHaveBeenCalled();
  });

  it("fires after 90s while processing", () => {
    const onFire = vi.fn();
    renderHook(() =>
      useThreadWatchdog({ threadId: "t1", isProcessing: true, onFire }),
    );
    act(() => { vi.advanceTimersByTime(89_999); });
    expect(onFire).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(2); });
    expect(onFire).toHaveBeenCalledWith("t1");
  });

  it("clears on isProcessing flip to false", () => {
    const onFire = vi.fn();
    const { rerender } = renderHook(
      ({ p }: { p: boolean }) =>
        useThreadWatchdog({ threadId: "t1", isProcessing: p, onFire }),
      { initialProps: { p: true } },
    );
    act(() => { vi.advanceTimersByTime(50_000); });
    rerender({ p: false });
    act(() => { vi.advanceTimersByTime(100_000); });
    expect(onFire).not.toHaveBeenCalled();
  });
});
