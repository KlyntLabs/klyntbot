// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
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
    productivityActivityFeedQuery: vi.fn().mockResolvedValue([
      {
        startedAt: new Date(Date.now() - 5_000).toISOString(),
        appName: "VSCode",
        siteName: null,
        windowTitle: "main.ts — myproject",
        projectId: null,
        categoryId: "coding",
        isIdle: false,
      },
      {
        startedAt: new Date(Date.now() - 90_000).toISOString(),
        appName: "Slack",
        siteName: null,
        windowTitle: null,
        projectId: null,
        categoryId: "communication",
        isIdle: false,
      },
    ]),
  };
});

import { ActivityFeed } from "./ActivityFeed";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("ActivityFeed", () => {
  it("renders both rows with app names", async () => {
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    expect(screen.getByText("Slack")).toBeTruthy();
  });

  it("shows recent ('now' / 'Ns') tag for first row", async () => {
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    // 5s old → "5s" or "now"
    expect(screen.getByText(/now|5s/)).toBeTruthy();
  });

  it("shows empty-state when no events", async () => {
    const { productivityActivityFeedQuery } = await import("@/api/endpoints/dashboard");
    (productivityActivityFeedQuery as ReturnType<typeof vi.fn>).mockResolvedValueOnce([]);
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("No recent activity")).toBeTruthy());
  });
});
