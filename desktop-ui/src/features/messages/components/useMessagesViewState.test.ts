import { renderHook, act } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { useMessagesViewState } from "./useMessagesViewState";

function makeFailedTool(id: string): ConversationItem {
  return {
    id,
    kind: "tool",
    toolType: "commandExecution",
    title: "Command: tsc",
    detail: "",
    status: "failed",
    output: "error TS2322",
  };
}

describe("useMessagesViewState — auto-expand on error", () => {
  it("auto-expands a failed tool on first appearance", () => {
    const initial: ConversationItem[] = [];
    const { result, rerender } = renderHook(
      ({ items }: { items: ConversationItem[] }) =>
        useMessagesViewState({
          items,
          threadId: "t1",
          isThinking: false,
          activeUserInputRequestId: null,
          hasVisibleUserInputRequest: false,
        }),
      { initialProps: { items: initial } },
    );

    expect(result.current.expandedItems.has("err-1")).toBe(false);
    rerender({ items: [makeFailedTool("err-1")] });
    expect(result.current.expandedItems.has("err-1")).toBe(true);
  });

  it("does not re-expand after a manual collapse", () => {
    const failed = makeFailedTool("err-2");
    const { result, rerender } = renderHook(
      ({ items }: { items: ConversationItem[] }) =>
        useMessagesViewState({
          items,
          threadId: "t1",
          isThinking: false,
          activeUserInputRequestId: null,
          hasVisibleUserInputRequest: false,
        }),
      { initialProps: { items: [failed] } },
    );

    expect(result.current.expandedItems.has("err-2")).toBe(true);
    act(() => result.current.toggleExpanded("err-2"));
    expect(result.current.expandedItems.has("err-2")).toBe(false);
    rerender({ items: [failed] });
    expect(result.current.expandedItems.has("err-2")).toBe(false);
  });
});
