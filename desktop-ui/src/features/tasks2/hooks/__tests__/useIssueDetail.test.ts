import { describe, expect, it } from "vitest";
import type { MockDetailTask } from "../../mock-data/issue-detail";
import { mockDetailTask } from "../../mock-data/issue-detail";
import { deriveTaskState } from "../useIssueDetail";

function makeTask(overrides: Partial<MockDetailTask>): MockDetailTask {
  return { ...mockDetailTask, ...overrides };
}

describe("deriveTaskState", () => {
  it("returns 'completed' when task.completed is true", () => {
    const task = makeTask({ completed: true });
    expect(deriveTaskState(task)).toBe("completed");
  });

  it("returns 'focused' when focusedAt is set", () => {
    const task = makeTask({ completed: false, focusedAt: "2026-03-13T11:35:00Z" });
    expect(deriveTaskState(task)).toBe("focused");
  });

  it("returns 'has-history' when tracked time exists but not focused", () => {
    const task = makeTask({ completed: false, focusedAt: null, totalTrackedSecs: 3600 });
    expect(deriveTaskState(task)).toBe("has-history");
  });

  it("returns 'new' when no completion, focus, or tracking", () => {
    const task = makeTask({ completed: false, focusedAt: null, totalTrackedSecs: 0 });
    expect(deriveTaskState(task)).toBe("new");
  });

  it("completed takes priority over focused", () => {
    const task = makeTask({ completed: true, focusedAt: "2026-03-13T11:35:00Z" });
    expect(deriveTaskState(task)).toBe("completed");
  });
});
