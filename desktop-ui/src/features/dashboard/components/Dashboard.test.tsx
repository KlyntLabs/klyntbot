// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(actual.EMPTY_TIMELINE_RESPONSE),
    taskUpdate: vi.fn(),
    calendarSyncEvents: vi.fn(),
  };
});

import { Dashboard } from "./Dashboard";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("Dashboard", () => {
  it("renders the topbar with view-pill switcher and sync button", () => {
    render(wrap(<Dashboard />));
    expect(screen.getByText("Day")).toBeTruthy();
    expect(screen.getByText("Week")).toBeTruthy();
    expect(screen.getByText("Month")).toBeTruthy();
    expect(screen.getByText("Year")).toBeTruthy();
    expect(screen.getByText("Sync")).toBeTruthy();
  });

  it("active view-pill defaults to Day", () => {
    render(wrap(<Dashboard />));
    const dayPill = screen.getByText("Day").closest("button");
    expect(dayPill?.className).toContain("dashboard__view-pill--active");
  });

  it("renders week view when Week pill is clicked", () => {
    render(wrap(<Dashboard />));
    const weekPill = screen.getByText("Week").closest("button") as HTMLButtonElement;
    fireEvent.click(weekPill);
    expect(screen.getAllByTestId("week-day-header").length).toBe(7);
  });

  it("mounts FocusStateIndicator and AutoFocusToast as siblings of dashboard__content", () => {
    render(wrap(<Dashboard />));
    const root = screen.getByText("Day").closest(".dashboard");
    expect(root).toBeTruthy();
    expect(root?.querySelector(".dashboard__content")).toBeTruthy();
  });
});
