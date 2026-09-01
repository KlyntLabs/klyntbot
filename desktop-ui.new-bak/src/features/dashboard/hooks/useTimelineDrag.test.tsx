// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", () => ({
  taskUpdate: vi.fn(),
}));

import { taskUpdate } from "@/api/endpoints/dashboard";
import { useTimelineDrag } from "./useTimelineDrag";

const mockedTaskUpdate = vi.mocked(taskUpdate);

afterEach(() => {
  mockedTaskUpdate.mockReset();
});

function wrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useTimelineDrag", () => {
  it("calls taskUpdate with new scheduledStart/scheduledEnd after a move drag", async () => {
    mockedTaskUpdate.mockResolvedValue({} as never);
    const { result } = renderHook(
      () =>
        useTimelineDrag("2026-04-30", 1, [
          "dashboard",
          "timeline",
          "2026-04-30",
          "2026-04-30",
          "todo",
        ]),
      { wrapper: wrapper() },
    );

    // Simulate startMove at 09:00 (540 min) ending at 09:30 (570)
    const fakeMouseEvent = {
      preventDefault: () => {},
      stopPropagation: () => {},
      clientY: 540,
      nativeEvent: { offsetY: 0 },
    } as unknown as React.MouseEvent;

    act(() => {
      result.current.startMove(fakeMouseEvent, "task-1", 540, 570);
    });

    // Drag down 60px → 60 minutes (pxPerMin = 1)
    act(() => {
      result.current.onMouseMove({ clientY: 600 } as MouseEvent);
    });

    await act(async () => {
      await result.current.onMouseUp();
    });

    expect(mockedTaskUpdate).toHaveBeenCalledTimes(1);
    const call = mockedTaskUpdate.mock.calls[0][0];
    expect(call.id).toBe("task-1");
    expect(call.scheduledStart).toBe("2026-04-30T10:00:00Z");
    expect(call.scheduledEnd).toBe("2026-04-30T10:30:00Z");
  });

  it("does not call taskUpdate when drag returns to origin", async () => {
    mockedTaskUpdate.mockResolvedValue({} as never);
    const { result } = renderHook(
      () =>
        useTimelineDrag("2026-04-30", 1, [
          "dashboard",
          "timeline",
          "2026-04-30",
          "2026-04-30",
          "todo",
        ]),
      { wrapper: wrapper() },
    );

    act(() => {
      result.current.startMove(
        {
          preventDefault() {},
          stopPropagation() {},
          clientY: 540,
          nativeEvent: { offsetY: 0 },
        } as never,
        "t1",
        540,
        570,
      );
    });
    await act(async () => {
      await result.current.onMouseUp();
    });
    expect(mockedTaskUpdate).not.toHaveBeenCalled();
  });
});
