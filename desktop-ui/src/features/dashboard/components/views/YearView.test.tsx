// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const mockTimeline: TimelineResponse = {
  entries: [
    {
      id: "focus-1",
      source: "focus",
      entryType: "focusSession",
      title: "Deep work",
      description: null,
      startedAt: "2026-01-15T09:00:00Z",
      endedAt: "2026-01-15T10:00:00Z",
      durationSecs: 3600,
      entityId: null,
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
  ],
  summary: {
    totalTrackedSecs: 3600,
    focusSecs: 3600,
    tasksCompleted: 0,
    tasksCreated: 0,
    notesTouched: 0,
    transactionsCount: 0,
    topApps: [],
    sourceBreakdown: [],
  },
};

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn(),
    taskUpdate: vi.fn(),
  };
});

import { timelineQuery } from "@/api/endpoints/dashboard";
import { DashboardStateContext } from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { YearView } from "./YearView";

const mockedTimelineQuery = vi.mocked(timelineQuery);

afterEach(() => cleanup());

function StateWrap({
  children,
  setDate = vi.fn(),
  setMode = vi.fn(),
}: {
  children: ReactNode;
  setDate?: (d: string) => void;
  setMode?: (m: "day" | "week" | "month" | "year") => void;
}) {
  return (
    <DashboardStateContext.Provider
      value={{
        mode: "year",
        date: "2026",
        setMode,
        setDate,
        navigatePrev: () => {},
        navigateNext: () => {},
        navigateToday: () => {},
      }}
    >
      <LayerContext.Provider
        value={{
          enabled: new Set(["activity"]),
          enabledSources: ["focus"],
          toggle: () => {},
          reset: () => {},
        }}
      >
        {children}
      </LayerContext.Provider>
    </DashboardStateContext.Provider>
  );
}

function wrap(
  node: ReactNode,
  opts?: {
    setDate?: (d: string) => void;
    setMode?: (m: "day" | "week" | "month" | "year") => void;
  },
) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <StateWrap setDate={opts?.setDate} setMode={opts?.setMode}>
        {node}
      </StateWrap>
    </QueryClientProvider>
  );
}

describe("YearView", () => {
  it("renders without crashing", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<YearView />));
    await waitFor(() => {
      expect(screen.getByText("Jan")).toBeTruthy();
    });
  });

  it("shows 12 month grids", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<YearView />));
    await waitFor(() => {
      const months = [
        "Jan",
        "Feb",
        "Mar",
        "Apr",
        "May",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Oct",
        "Nov",
        "Dec",
      ];
      for (const m of months) {
        expect(screen.getByText(m)).toBeTruthy();
      }
    });
  });

  it("clicking a day cell switches to day mode", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    const setDate = vi.fn();
    const setMode = vi.fn();
    render(wrap(<YearView />, { setDate, setMode }));

    await waitFor(() => {
      expect(screen.getByText("Jan")).toBeTruthy();
    });

    // Find a clickable day button in January (there should be at least one)
    const dayButton = screen.getByTitle(/2026-01-15:/);
    fireEvent.click(dayButton);

    expect(setDate).toHaveBeenCalledWith("2026-01-15");
    expect(setMode).toHaveBeenCalledWith("day");
  });
});
