// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { useRef } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { useThreadCodexOrchestration } from "./useThreadCodexOrchestration";

describe("useThreadCodexOrchestration", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("persists thread-scoped codex params", () => {
    const { result } = renderHook(() => {
      const activeWorkspaceIdForParamsRef = useRef<string | null>("ws-1");
      return useThreadCodexOrchestration({ activeWorkspaceIdForParamsRef });
    });

    act(() => {
      result.current.activeThreadIdRef.current = "thread-1";
      result.current.persistThreadCodexParams({
        modelId: "gpt-5",
        codexArgsOverride: "--profile dev",
      });
    });

    expect(result.current.getThreadCodexParams("ws-1", "thread-1")).toEqual(
      expect.objectContaining({
        modelId: "gpt-5",
        codexArgsOverride: "--profile dev",
      }),
    );
  });

  it("falls back to the no-thread scope when no active thread", () => {
    const { result } = renderHook(() => {
      const activeWorkspaceIdForParamsRef = useRef<string | null>("ws-1");
      return useThreadCodexOrchestration({ activeWorkspaceIdForParamsRef });
    });

    act(() => {
      result.current.activeThreadIdRef.current = null;
      result.current.persistThreadCodexParams({
        effort: "high",
      });
    });

    expect(result.current.getThreadCodexParams("ws-1", "__no_thread__")).toEqual(
      expect.objectContaining({
        effort: "high",
      }),
    );
    expect(result.current.getThreadCodexParams("ws-1", "thread-1")).toBeNull();
  });
});
