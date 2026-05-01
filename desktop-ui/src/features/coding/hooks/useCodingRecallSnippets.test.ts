// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { useCodingRecallSnippets } from "./useCodingRecallSnippets";

const listeners: Record<string, (payload: unknown) => void> = {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, cb: (payload: unknown) => void) => {
    listeners[event] = cb;
    return () => {
      delete listeners[event];
    };
  }),
}));

describe("useCodingRecallSnippets", () => {
  test("listens to recall and dead-end events for matching thread", async () => {
    const { result } = renderHook(() => useCodingRecallSnippets("t1"));

    await act(async () => {
      listeners["agent:recall_injected"]?.({
        payload: {
          thread_id: "t1",
          memory_ids: ["m1"],
          coverage_score: 0.75,
          dead_end_warning: false,
          snippets: [{ kind: "edit", summary: "fix parser", source: "src/main.rs" }],
        },
      });
    });

    expect(result.current.recall).not.toBeNull();
    expect(result.current.recall?.coverage_score).toBe(0.75);

    await act(async () => {
      listeners["agent:dead_end_warning_surfaced"]?.({
        payload: {
          thread_id: "t1",
          approach_summary: "regex rewrite",
          prior_attempt_id: "a1",
          confidence: 0.9,
        },
      });
    });

    expect(result.current.deadEnd).not.toBeNull();
    expect(result.current.deadEnd?.confidence).toBe(0.9);
  });

  test("ignores events for other threads", async () => {
    const { result } = renderHook(() => useCodingRecallSnippets("t1"));

    await act(async () => {
      listeners["agent:recall_injected"]?.({
        payload: {
          thread_id: "t2",
          memory_ids: [],
          coverage_score: 0,
          dead_end_warning: false,
          snippets: [],
        },
      });
    });

    expect(result.current.recall).toBeNull();
  });
});
