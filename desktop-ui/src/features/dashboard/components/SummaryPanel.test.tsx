// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
  };
});

import type { TimelineEntry, TimelineSummary } from "@/bindings";
import type { SessionBlock } from "./views/ActivityTrack";
import { SummaryPanel } from "./SummaryPanel";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

const SUMMARY: TimelineSummary = {
  totalTrackedSecs: 3600,
  focusSecs: 1200,
  tasksCompleted: 0,
  tasksCreated: 0,
  notesTouched: 0,
  transactionsCount: 0,
  topApps: [{ appName: "VSCode", durationSecs: 1800, percentage: 50 }],
  sourceBreakdown: [],
};

describe("SummaryPanel", () => {
  it("renders nothing when summary is null and no selection", () => {
    const { container } = render(
      wrap(<SummaryPanel summary={null} selectedEntry={null} onClose={() => {}} />),
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders DaySummary fallback when summary present, no productivity", async () => {
    render(wrap(<SummaryPanel summary={SUMMARY} selectedEntry={null} onClose={() => {}} />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    expect(screen.getByText("tracked")).toBeTruthy();
  });

  it("renders EntryDetail when selectedEntry is set", () => {
    const entry: TimelineEntry = {
      id: "e1",
      title: "Test entry",
      description: "test desc",
      startedAt: "2026-05-02T10:00:00Z",
      endedAt: "2026-05-02T10:30:00Z",
      durationSecs: 1800,
      source: "task",
      entryType: "taskDue",
      color: "#4285F4",
      metadata: null,
      entityId: null,
      entityRoute: null,
    };
    const onClose = vi.fn();
    render(wrap(<SummaryPanel summary={null} selectedEntry={entry} onClose={onClose} />));
    expect(screen.getByText("Test entry")).toBeTruthy();
    expect(screen.getByText("test desc")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("Close details"));
    expect(onClose).toHaveBeenCalled();
  });

  it("renders SessionDetail when selectedSession is set", () => {
    const session: SessionBlock = {
      startMin: 540,
      endMin: 600,
      color: "#22C55E",
      label: "Coding session",
      duration: 3600,
      dominantCategory: "productive",
      appBreakdown: [{ app: "VSCode", dur: 3600, catType: "productive" }],
      duringFocus: true,
      intelligence: null,
    };
    render(
      wrap(
        <SummaryPanel
          summary={null}
          selectedEntry={null}
          selectedSession={session}
          onClose={() => {}}
        />,
      ),
    );
    expect(screen.getByText("Coding session")).toBeTruthy();
    expect(screen.getByText("Activity Session")).toBeTruthy();
  });
});
