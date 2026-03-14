import { describe, expect, it } from "vitest";
import type { MockDetailTask } from "../../mock-data/issue-detail";
import { mockDetailTask } from "../../mock-data/issue-detail";
import { deriveTaskState } from "../useIssueDetail";

function makeTask(overrides: Partial<MockDetailTask>): MockDetailTask {
  return { ...mockDetailTask, ...overrides };
}

describe("deriveTaskState", () => {
  it("returns 'completed' when task.completed is true", () => {
    expect(deriveTaskState(makeTask({ completed: true }))).toBe("completed");
  });

  it("returns 'focused' when focusedAt is set", () => {
    expect(deriveTaskState(makeTask({ completed: false, focusedAt: "2026-03-13T11:35:00Z" }))).toBe(
      "focused",
    );
  });

  it("returns 'has-history' when tracked time exists but not focused", () => {
    expect(
      deriveTaskState(makeTask({ completed: false, focusedAt: null, totalTrackedSecs: 3600 })),
    ).toBe("has-history");
  });

  it("returns 'new' when no completion, focus, or tracking", () => {
    expect(
      deriveTaskState(makeTask({ completed: false, focusedAt: null, totalTrackedSecs: 0 })),
    ).toBe("new");
  });

  it("completed takes priority over focused", () => {
    expect(deriveTaskState(makeTask({ completed: true, focusedAt: "2026-03-13T11:35:00Z" }))).toBe(
      "completed",
    );
  });

  it("focused takes priority over has-history", () => {
    expect(
      deriveTaskState(
        makeTask({
          completed: false,
          focusedAt: "2026-03-13T11:35:00Z",
          totalTrackedSecs: 3600,
        }),
      ),
    ).toBe("focused");
  });
});
