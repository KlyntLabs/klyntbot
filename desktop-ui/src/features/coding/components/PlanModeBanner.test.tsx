// @vitest-environment jsdom

import { describe, expect, test, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { PlanModeBanner } from "./PlanModeBanner";
import { applyView, cleanupTodos } from "../state/todoStore";

vi.mock("@/api/endpoints/coding", () => ({
  removePlanItems: vi.fn(async () => ({
    agents: {},
    planModeState: null,
  })),
  editPlanItems: vi.fn(async () => ({
    agents: {},
    planModeState: null,
  })),
  ratifyPlan: vi.fn(async () => ({
    agents: {},
    planModeState: null,
  })),
  cancelPlanMode: vi.fn(async () => ({
    agents: {},
    planModeState: null,
  })),
  openPlanFile: vi.fn(async () => {}),
}));

const THREAD_ID = "test-thread";

function seedStore(items: any[], planModeState: any) {
  applyView(THREAD_ID, { agents: { root: items }, planModeState });
}

beforeEach(() => {
  cleanupTodos(THREAD_ID);
});

afterEach(() => {
  cleanupTodos(THREAD_ID);
  vi.clearAllMocks();
});

describe("PlanModeBanner", () => {
  test("hidden when plan_mode_state is null", () => {
    seedStore([], null);
    const { container } = render(<PlanModeBanner threadId={THREAD_ID} />);
    expect(container.firstChild).toBeNull();
  });

  test("renders plan mode header and items", () => {
    seedStore(
      [
        { id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] },
        { id: "b", title: "Add migration", status: "pending", concurrency: "sequential", blockedBy: [] },
      ],
      {
        planSessionId: "sess-1",
        planFileSlug: "2025-01-15-feature",
        planFilePath: "/tmp/plans/2025-01-15-feature.md",
        proposedItemCount: 2,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);
    expect(screen.getByText(/Plan mode ·/)).toBeInTheDocument();
    expect(screen.getByText("Read schema")).toBeInTheDocument();
    expect(screen.getByText("Add migration")).toBeInTheDocument();
    expect(screen.getByText("Reviewing 2 proposed items")).toBeInTheDocument();
  });

  test("inline edit blur triggers editPlanItems", async () => {
    const { editPlanItems } = await import("@/api/endpoints/coding");
    seedStore(
      [{ id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] }],
      {
        planSessionId: "sess-1",
        planFileSlug: "plan",
        planFilePath: "/tmp/plan.md",
        proposedItemCount: 1,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);

    const title = screen.getByText("Read schema");
    fireEvent.click(title);

    const input = screen.getByDisplayValue("Read schema");
    fireEvent.change(input, { target: { value: "Read schema v2" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(editPlanItems).toHaveBeenCalledWith(
        THREAD_ID,
        "sess-1",
        expect.stringContaining("Read schema v2"),
      );
    });
  });

  test("remove button triggers removePlanItems", async () => {
    const { removePlanItems } = await import("@/api/endpoints/coding");
    seedStore(
      [{ id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] }],
      {
        planSessionId: "sess-1",
        planFileSlug: "plan",
        planFilePath: "/tmp/plan.md",
        proposedItemCount: 1,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);

    const removeBtn = screen.getByLabelText("Remove Read schema");
    fireEvent.click(removeBtn);

    await waitFor(() => {
      expect(removePlanItems).toHaveBeenCalledWith(THREAD_ID, "sess-1", ["a"]);
    });
  });

  test("ratify confirmation flow", async () => {
    const { ratifyPlan } = await import("@/api/endpoints/coding");
    seedStore(
      [{ id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] }],
      {
        planSessionId: "sess-1",
        planFileSlug: "plan",
        planFilePath: "/tmp/plan.md",
        proposedItemCount: 1,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);

    fireEvent.click(screen.getByText("Ratify & Execute"));
    expect(screen.getByText(/Ratify 1 item\?/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Confirm"));
    await waitFor(() => {
      expect(ratifyPlan).toHaveBeenCalledWith(THREAD_ID, "sess-1");
    });
  });

  test("cancel confirmation flow", async () => {
    const { cancelPlanMode } = await import("@/api/endpoints/coding");
    seedStore(
      [{ id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] }],
      {
        planSessionId: "sess-1",
        planFileSlug: "plan",
        planFilePath: "/tmp/plan.md",
        proposedItemCount: 1,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);

    fireEvent.click(screen.getByText("Cancel Plan"));
    expect(screen.getByText(/Cancel plan and discard/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Confirm"));
    await waitFor(() => {
      expect(cancelPlanMode).toHaveBeenCalledWith(THREAD_ID);
    });
  });

  test("back button dismisses confirmation", () => {
    seedStore(
      [{ id: "a", title: "Read schema", status: "pending", concurrency: "safe", blockedBy: [] }],
      {
        planSessionId: "sess-1",
        planFileSlug: "plan",
        planFilePath: "/tmp/plan.md",
        proposedItemCount: 1,
      },
    );
    render(<PlanModeBanner threadId={THREAD_ID} />);

    fireEvent.click(screen.getByText("Ratify & Execute"));
    expect(screen.getByText(/Ratify 1 item\?/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Back"));
    expect(screen.queryByText(/Ratify 1 item\?/)).not.toBeInTheDocument();
    expect(screen.getByText("Ratify & Execute")).toBeInTheDocument();
  });
});
