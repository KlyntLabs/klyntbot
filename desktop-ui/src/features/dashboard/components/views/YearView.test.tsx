// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
  const { defaultDashboardMocks } = await import("../../__tests__/dashboardCommandMocks");
  return {
    ...actual,
    ...defaultDashboardMocks(),
    timelineQuery: vi.fn(),
    taskUpdate: vi.fn(),
  };
});

vi.mock("../../lib/layers", async () => {
  const actual = await vi.importActual<typeof import("../../lib/layers")>("../../lib/layers");
  return {
    ...actual,
    useEnabledLayers: vi.fn(),
    useSidebarOpen: vi.fn().mockReturnValue({ sidebarOpen: false, toggleSidebar: () => {} }),
  };
});

import { timelineQuery } from "@/api/endpoints/dashboard";
import { DashboardStateContext } from "../../hooks/useDashboardState";
import { useEnabledLayers, useSidebarOpen } from "../../lib/layers";
import { YearView } from "./YearView";

const mockedTimelineQuery = vi.mocked(timelineQuery);

afterEach(() => cleanup());

beforeEach(() => {
  vi.mocked(useEnabledLayers).mockReturnValue({
    enabled: new Set(["activity"]),
    enabledSources: ["focus"],
    toggle: () => {},
    reset: () => {},
  });
  vi.mocked(useSidebarOpen).mockReturnValue({ sidebarOpen: false, toggleSidebar: () => {} });
});

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
      {children}
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

  it("respects enabledSources — disabling 'focus' removes tinting", async () => {
    // First render: focus enabled, focus entry tints the day
    mockedTimelineQuery.mockResolvedValue({
      entries: [
        {
          id: "f1",
          title: "Focus session",
          description: null,
          startedAt: "2026-05-02T10:00:00",
          endedAt: "2026-05-02T11:00:00",
          durationSecs: 3600,
          source: "focus",
          entryType: "focusSession",
          color: "var(--timeline-focus)",
          metadata: null,
          entityId: null,
          entityRoute: null,
        },
      ],
      summary: {
        totalTrackedSecs: 0,
        focusSecs: 3600,
        tasksCompleted: 0,
        tasksCreated: 0,
        notesTouched: 0,
        transactionsCount: 0,
        topApps: [],
        sourceBreakdown: [],
      },
    });

    const { unmount } = render(wrap(<YearView />));
    await waitFor(() => {
      const cell = screen.getByTitle(/2026-05-02:/);
      expect(cell.getAttribute("style") ?? "").toContain("timeline-focus");
    });

    // Second render: focus disabled — same date should now have transparent/muted background
    unmount();
    vi.mocked(useEnabledLayers).mockReturnValue({
      enabled: new Set(),
      enabledSources: [],
      toggle: vi.fn(),
      reset: vi.fn(),
    });
    render(wrap(<YearView />));
    await waitFor(() => {
      const cell = screen.getByTitle(/2026-05-02:/);
      expect(cell.getAttribute("style") ?? "").not.toContain("timeline-focus");
    });
  });
});
