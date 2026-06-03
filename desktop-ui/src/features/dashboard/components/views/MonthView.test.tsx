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
      title: "Focus session",
      description: null,
      startedAt: "2026-04-15T09:00:00Z",
      endedAt: "2026-04-15T10:00:00Z",
      durationSecs: 3600,
      entityId: null,
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
    {
      id: "app-1",
      source: "productivity",
      entryType: "appUsage",
      title: "Code",
      description: null,
      startedAt: "2026-04-15T10:00:00Z",
      endedAt: "2026-04-15T11:00:00Z",
      durationSecs: 3600,
      entityId: null,
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
  ],
  summary: {
    totalTrackedSecs: 7200,
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
  const { defaultDashboardMocks } = await import("../../__tests__/dashboardCommandMocks");
  return {
    ...actual,
    ...defaultDashboardMocks(),
    timelineQuery: vi.fn(),
    taskUpdate: vi.fn(),
  };
});

vi.mock("@/utils/dashboardDates", async () => {
  const actual =
    await vi.importActual<typeof import("@/utils/dashboardDates")>("@/utils/dashboardDates");
  return {
    ...actual,
    todayISO: () => "2026-04-15",
  };
});

import { timelineQuery } from "@/api/endpoints/dashboard";
import { DashboardStateContext, useDashboardStateImpl } from "../../hooks/useDashboardState";
import { LayerContext, SidebarContext } from "../../lib/layers";
import { MonthView } from "./MonthView";

const mockedTimelineQuery = vi.mocked(timelineQuery);

afterEach(() => cleanup());

function StateWrap({
  children,
  setDate,
  setMode,
}: {
  children: ReactNode;
  setDate?: (d: string) => void;
  setMode?: (m: "day" | "week" | "month" | "year") => void;
}) {
  const state = useDashboardStateImpl({ mode: "month", date: "2026-04-01" });
  return (
    <DashboardStateContext.Provider
      value={{
        ...state,
        setDate: setDate ?? state.setDate,
        setMode: setMode ?? state.setMode,
      }}
    >
      <LayerContext.Provider
        value={{
          enabled: new Set(["activity"]),
          enabledSources: ["productivity", "focus"],
          toggle: () => {},
          reset: () => {},
        }}
      >
        <SidebarContext.Provider value={{ sidebarOpen: false, toggleSidebar: () => {} }}>
          {children}
        </SidebarContext.Provider>
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

describe("MonthView", () => {
  it("renders without crashing", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<MonthView />));
    await waitFor(() => {
      expect(screen.getByRole("grid")).toBeTruthy();
    });
  });

  it("shows a 7×6 grid of day cells", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<MonthView />));
    await waitFor(() => {
      const grid = screen.getByRole("grid");
      expect(grid).toBeTruthy();
    });
    // 6 rows × 7 columns = 42 buttons
    const buttons = screen.getAllByRole("button");
    expect(buttons.length).toBe(42);
  });

  it("clicking a day cell calls setDate and setMode('day')", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    const setDate = vi.fn();
    const setMode = vi.fn();
    render(wrap(<MonthView />, { setDate, setMode }));
    await waitFor(() => {
      expect(screen.getAllByRole("button").length).toBe(42);
    });
    const buttons = screen.getAllByRole("button");
    fireEvent.click(buttons[0]);
    expect(setDate).toHaveBeenCalledTimes(1);
    expect(setMode).toHaveBeenCalledWith("day");
  });

  it("highlights today with --today modifier", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<MonthView />));
    await waitFor(() => {
      expect(screen.getAllByRole("button").length).toBe(42);
    });
    const todayCell = document.querySelector('[data-today="true"]');
    expect(todayCell).toBeTruthy();
    expect(todayCell?.textContent).toContain("15");
  });

  it("arrow key navigation moves focus", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<MonthView />));
    await waitFor(() => {
      expect(screen.getAllByRole("button").length).toBe(42);
    });
    const grid = screen.getByRole("grid");
    // Today (15th) gets --today, not --focused
    expect(document.querySelector('[data-focused="true"]')).toBeNull();
    fireEvent.keyDown(grid, { key: "ArrowRight" });
    await waitFor(() => {
      const focused = document.querySelector('[data-focused="true"]');
      expect(focused).toBeTruthy();
      expect(focused?.textContent).toContain("16");
    });
  });

  it("renders SummaryPanel when sidebar is open", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
    render(
      <QueryClientProvider client={client}>
        <DashboardStateContext.Provider
          value={{
            mode: "month",
            date: "2026-04-01",
            setDate: vi.fn(),
            setMode: vi.fn(),
            navigatePrev: vi.fn(),
            navigateNext: vi.fn(),
            navigateToday: vi.fn(),
          }}
        >
          <LayerContext.Provider
            value={{
              enabled: new Set(["activity"]),
              enabledSources: ["productivity", "focus"],
              toggle: () => {},
              reset: () => {},
            }}
          >
            <SidebarContext.Provider value={{ sidebarOpen: true, toggleSidebar: () => {} }}>
              <MonthView />
            </SidebarContext.Provider>
          </LayerContext.Provider>
        </DashboardStateContext.Provider>
      </QueryClientProvider>,
    );
    await waitFor(() => {
      expect(screen.getAllByRole("button").length).toBeGreaterThan(0);
    });
    // SummaryPanel renders DaySummary which shows tracked time
    expect(screen.getByText("tracked")).toBeTruthy();
  });
});
