/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { ChatThread } from "@/features/chat/types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => {
  vi.restoreAllMocks();
});

function thread(id: string, title: string): ChatThread {
  return { sessionKey: id, title, updatedAt: new Date().toISOString(), messageCount: 0 };
}

describe("CodingSidebarThreadsSection", () => {
  it("renders three sections when groups are populated", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Refactor"), thread("b", "Build login"), thread("c", "Fix bug"), thread("d", "Idle one")]}
        runningIds={new Set(["a", "b"])}
        recentlyCompleted={new Map([["c", Date.now()]])}
        activeThreadId="a"
        onSelectThread={() => {}}
      />,
    );
    expect(screen.getByText(/Running/i)).toBeInTheDocument();
    expect(screen.getByText(/Recently completed/i)).toBeInTheDocument();
    expect(screen.getByText(/^Chats$/)).toBeInTheDocument();
    expect(screen.getByText("Refactor")).toBeInTheDocument();
    expect(screen.getByText("Build login")).toBeInTheDocument();
    expect(screen.getByText("Fix bug")).toBeInTheDocument();
    expect(screen.getByText("Idle one")).toBeInTheDocument();
  });

  it("collapses empty group headers", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Idle")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId={null}
        onSelectThread={() => {}}
      />,
    );
    expect(screen.queryByText(/Running/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Recently completed/i)).not.toBeInTheDocument();
    expect(screen.getByText(/^Chats$/)).toBeInTheDocument();
  });

  it("clicking a recently-completed row calls markThreadOpened and onSelectThread", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    const buf = await import("@/features/coding/state/ThreadEventBuffer");
    buf.__testing.reset();
    buf.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "a",
      turn_id: "t",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(buf.getRecentlyCompleted().has("a")).toBe(true);

    const onSelect = vi.fn();
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Done one")]}
        runningIds={new Set()}
        recentlyCompleted={buf.getRecentlyCompleted()}
        activeThreadId={null}
        onSelectThread={onSelect}
      />,
    );
    fireEvent.click(screen.getByText("Done one"));
    expect(onSelect).toHaveBeenCalledWith("a");
    expect(buf.getRecentlyCompleted().has("a")).toBe(false);
  });
});
