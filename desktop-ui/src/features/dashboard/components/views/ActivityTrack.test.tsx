// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
    productivityTimelineQuery: vi.fn().mockResolvedValue([
      {
        startedAt: "2026-05-02T09:00:00",
        endedAt: "2026-05-02T10:00:00",
        durationSecs: 3600,
        appName: "VSCode",
        siteName: null,
        windowTitle: null,
        projectId: null,
        categoryId: "coding",
        focusSessionId: null,
        isIdle: false,
      },
    ]),
    productivityCategoriesQuery: vi.fn().mockResolvedValue([
      { id: "coding", categoryType: "productive", name: "Coding" },
    ]),
    productivityIntelligenceSessionsQuery: vi.fn().mockResolvedValue([]),
  };
});

import { ActivityTrack } from "./ActivityTrack";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("ActivityTrack", () => {
  it("renders one merged session block from a single timeline entry", async () => {
    const onSelectSession = vi.fn();
    render(
      wrap(
        <ActivityTrack
          date="2026-05-02"
          hourHeight={60}
          isToday={false}
          onSelectSession={onSelectSession}
          onSelectEntry={() => {}}
          selectedSession={null}
          selectedEntryId={null}
        />,
      ),
    );
    await waitFor(() => expect(screen.getByText(/VSCode/)).toBeTruthy());
  });

  it("calls onSelectSession when a block is clicked", async () => {
    const onSelectSession = vi.fn();
    render(
      wrap(
        <ActivityTrack
          date="2026-05-02"
          hourHeight={60}
          isToday={false}
          onSelectSession={onSelectSession}
          onSelectEntry={() => {}}
          selectedSession={null}
          selectedEntryId={null}
        />,
      ),
    );
    await waitFor(() => expect(screen.getByText(/VSCode/)).toBeTruthy());
    fireEvent.click(screen.getByText(/VSCode/).closest("button") as HTMLButtonElement);
    expect(onSelectSession).toHaveBeenCalled();
  });
});
