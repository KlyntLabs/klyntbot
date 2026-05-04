// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn(),
  };
});

import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { DashboardStateContext, useDashboardStateImpl } from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { WeekView } from "./WeekView";

const mockedTimelineQuery = vi.mocked(timelineQuery);

afterEach(() => cleanup());

function StateWrap({ children }: { children: ReactNode }) {
  const state = useDashboardStateImpl({ mode: "week", date: "2026-04-27" });
  return (
    <DashboardStateContext.Provider value={state}>
      <LayerContext.Provider
        value={{
          enabled: new Set(["tasks"]),
          enabledSources: ["todo"],
          toggle: () => {},
          reset: () => {},
        }}
      >
        {children}
      </LayerContext.Provider>
    </DashboardStateContext.Provider>
  );
}

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <StateWrap>{node}</StateWrap>
    </QueryClientProvider>
  );
}

describe("WeekView", () => {
  it("renders without crashing", async () => {
    mockedTimelineQuery.mockResolvedValue(EMPTY_TIMELINE_RESPONSE);
    render(wrap(<WeekView />));
    await waitFor(() => {
      expect(screen.getByText("Mon")).toBeTruthy();
    });
  });

  it("shows 7 day columns", async () => {
    mockedTimelineQuery.mockResolvedValue(EMPTY_TIMELINE_RESPONSE);
    render(wrap(<WeekView />));
    await waitFor(() => {
      const headers = screen.getAllByTestId("week-day-header");
      expect(headers.length).toBe(7);
    });
  });

  it("shows 24 hour rows", async () => {
    mockedTimelineQuery.mockResolvedValue(EMPTY_TIMELINE_RESPONSE);
    render(wrap(<WeekView />));
    await waitFor(() => {
      const hours = screen.getAllByTestId("hour-label");
      expect(hours.length).toBe(24);
    });
  });

  it("selecting a session block opens EntryDetail in SummaryPanel", async () => {
    mockedTimelineQuery.mockResolvedValue({
      entries: [
        {
          id: "a1",
          title: "VSCode",
          description: null,
          startedAt: "2026-04-29T10:00:00",
          endedAt: "2026-04-29T11:00:00",
          durationSecs: 3600,
          source: "productivity",
          entryType: "appUsage",
          color: "var(--timeline-app-productive)",
          metadata: null,
          entityId: null,
          entityRoute: null,
        },
      ],
      summary: {
        totalTrackedSecs: 3600,
        focusSecs: 0,
        tasksCompleted: 0,
        tasksCreated: 0,
        notesTouched: 0,
        transactionsCount: 0,
        topApps: [{ appName: "VSCode", durationSecs: 3600, percentage: 100 }],
        sourceBreakdown: [],
      },
    });
    render(wrap(<WeekView />));
    await waitFor(() => expect(screen.getAllByText("VSCode").length).toBeGreaterThan(0));
    const sessionBtn = screen
      .getAllByText("VSCode")
      .find((el) => el.closest("button")?.classList.contains("dashboard__week-session"))
      ?.closest("button");
    if (!sessionBtn) throw new Error("session button not found");
    fireEvent.click(sessionBtn);
    await waitFor(() => expect(screen.getByText("Details")).toBeTruthy());
  });

  it("clicking a day header switches to day mode", async () => {
    mockedTimelineQuery.mockResolvedValue(EMPTY_TIMELINE_RESPONSE);
    const setDateMock = vi.fn();
    const setModeMock = vi.fn();

    function MockStateWrap({ children }: { children: ReactNode }) {
      const state = useDashboardStateImpl({ mode: "week", date: "2026-04-27" });
      return (
        <DashboardStateContext.Provider
          value={{ ...state, setDate: setDateMock, setMode: setModeMock }}
        >
          <LayerContext.Provider
            value={{
              enabled: new Set(["tasks"]),
              enabledSources: ["todo"],
              toggle: () => {},
              reset: () => {},
            }}
          >
            {children}
          </LayerContext.Provider>
        </DashboardStateContext.Provider>
      );
    }

    render(
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: 0 } } })}>
        <MockStateWrap>
          <WeekView />
        </MockStateWrap>
      </QueryClientProvider>,
    );
    await waitFor(() => {
      const header = screen.getAllByTestId("week-day-header")[0];
      expect(header).toBeTruthy();
    });
    const header = screen.getAllByTestId("week-day-header")[0];
    header.click();
    expect(setDateMock).toHaveBeenCalled();
    expect(setModeMock).toHaveBeenCalledWith("day");
  });
});
