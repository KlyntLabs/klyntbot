// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const mockTimeline: TimelineResponse = {
  entries: [
    {
      id: "task-1",
      source: "todo",
      entryType: "taskDue",
      title: "Write tests",
      description: null,
      startedAt: "2026-04-30T09:00:00Z",
      endedAt: null,
      durationSecs: 1800,
      entityId: "task-1",
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
    {
      id: "task-2",
      source: "todo",
      entryType: "taskDue",
      title: "Review PR",
      description: null,
      startedAt: "2026-04-30T11:00:00Z",
      endedAt: null,
      durationSecs: 900,
      entityId: "task-2",
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
  ],
  summary: {
    totalTrackedSecs: 2700,
    focusSecs: 0,
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
import { DashboardStateContext, useDashboardStateImpl } from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { DayView } from "./DayView";

const mockedTimelineQuery = vi.mocked(timelineQuery);

afterEach(() => cleanup());

function StateWrap({ children }: { children: ReactNode }) {
  const state = useDashboardStateImpl({ mode: "day", date: "2026-04-30" });
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

describe("DayView", () => {
  it("renders both task blocks from the mocked timeline response", async () => {
    mockedTimelineQuery.mockResolvedValue(mockTimeline);
    render(wrap(<DayView />));
    await waitFor(() => {
      expect(screen.getByText("Write tests")).toBeTruthy();
      expect(screen.getByText("Review PR")).toBeTruthy();
    });
  });
});
