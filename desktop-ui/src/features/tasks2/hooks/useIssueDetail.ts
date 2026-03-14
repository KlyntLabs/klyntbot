import { useCallback, useState } from "react";
import type { MockDetailTask, MockSuggestion, TaskState } from "../mock-data/issue-detail";
import {
  mockActivityEntries,
  mockDetailTask,
  mockFocusSession,
  mockSubIssues,
  mockSuggestions,
  mockTaskMemory,
} from "../mock-data/issue-detail";

export function deriveTaskState(task: MockDetailTask): TaskState {
  if (task.completed) return "completed";
  if (task.focusedAt) return "focused";
  if (task.totalTrackedSecs > 0) return "has-history";
  return "new";
}

export function useIssueDetail(_issueId: string) {
  const [task, setTask] = useState<MockDetailTask>(mockDetailTask);
  const [suggestions, setSuggestions] = useState<MockSuggestion[]>(mockSuggestions);
  const taskState = deriveTaskState(task);

  const updateTask = useCallback((field: string, value: unknown) => {
    setTask((prev) => ({ ...prev, [field]: value }));
  }, []);

  const dismissSuggestion = useCallback((id: string) => {
    setSuggestions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, status: "dismissed" as const } : s)),
    );
  }, []);

  const applySuggestion = useCallback((id: string) => {
    setSuggestions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, status: "applied" as const } : s)),
    );
  }, []);

  return {
    task,
    taskState,
    activity: mockActivityEntries,
    suggestions,
    focusSession: taskState === "focused" ? mockFocusSession : null,
    subIssues: mockSubIssues,
    taskMemory: mockTaskMemory,
    updateTask,
    dismissSuggestion,
    applySuggestion,
  };
}
