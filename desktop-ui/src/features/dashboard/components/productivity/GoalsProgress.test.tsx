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
    productivityGoalsQuery: vi.fn().mockResolvedValue([
      {
        id: 1,
        goalType: "daily",
        metric: "productive_hours",
        targetValue: 6,
        currentValue: 3,
        met: false,
        projectId: null,
      },
      {
        id: 2,
        goalType: "weekly",
        metric: "focus_sessions",
        targetValue: 10,
        currentValue: 12,
        met: true,
        projectId: null,
      },
    ]),
    productivityGoalDelete: vi.fn().mockResolvedValue(undefined),
  };
});

import { GoalsProgress } from "./GoalsProgress";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("GoalsProgress", () => {
  it("renders both goals with correct status pills", async () => {
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("MET")).toBeTruthy());
    expect(screen.getByText("IN PROGRESS")).toBeTruthy();
    expect(screen.getByText(/productive hours/)).toBeTruthy();
    expect(screen.getByText(/focus sessions/)).toBeTruthy();
  });

  it("opens AddGoalDialog when plus button clicked", async () => {
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("Goals")).toBeTruthy());
    const plusBtn = screen.getByLabelText("Add goal");
    fireEvent.click(plusBtn);
    expect(screen.getByText("Add Goal")).toBeTruthy();
  });

  it("calls productivityGoalDelete when trash button clicked and confirmed", async () => {
    const { productivityGoalDelete } = await import("@/api/endpoints/dashboard");
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("MET")).toBeTruthy());
    const deleteButtons = screen.getAllByLabelText("Delete goal");
    fireEvent.click(deleteButtons[0]);
    await waitFor(() => expect(productivityGoalDelete).toHaveBeenCalledWith(1));
    confirmSpy.mockRestore();
  });

  it("does not delete when user cancels confirmation", async () => {
    const { productivityGoalDelete } = await import("@/api/endpoints/dashboard");
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("MET")).toBeTruthy());
    const deleteButtons = screen.getAllByLabelText("Delete goal");
    fireEvent.click(deleteButtons[0]);
    expect(productivityGoalDelete).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
