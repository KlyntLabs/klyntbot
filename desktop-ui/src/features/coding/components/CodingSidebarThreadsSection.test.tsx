/** @vitest-environment jsdom */

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatThread } from "@/features/chat/types";
import type { WorkspaceInfo } from "@/types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

beforeEach(() => {
  if (typeof localStorage !== "undefined") localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

function thread(id: string, title: string): ChatThread {
  return { sessionKey: id, title, updatedAt: new Date().toISOString(), messageCount: 0 };
}

function workspace(id: string, name: string, path = `/Users/x/${name}`): WorkspaceInfo {
  return { id, name, path, connected: true, settings: {} as WorkspaceInfo["settings"] };
}

describe("CodingSidebarThreadsSection — project grouping", () => {
  it("groups threads under their workspace project headers", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Refactor auth"), thread("b", "Fix bug"), thread("c", "Notes")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId={null}
        onSelectThread={() => {}}
        workspaces={[workspace("ws1", "KlyntBot"), workspace("ws2", "CryptoGuard")]}
        workspaceIdByThread={
          new Map([
            ["a", "ws1"],
            ["b", "ws1"],
            ["c", "ws2"],
          ])
        }
      />,
    );
    expect(screen.getByText(/Projects/i)).toBeInTheDocument();
    expect(screen.getByText("KlyntBot")).toBeInTheDocument();
    expect(screen.getByText("CryptoGuard")).toBeInTheDocument();
    expect(screen.getByText("Refactor auth")).toBeInTheDocument();
    expect(screen.getByText("Fix bug")).toBeInTheDocument();
    expect(screen.getByText("Notes")).toBeInTheDocument();
  });

  it("hides a project's threads when collapsed", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Refactor auth")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId={null}
        onSelectThread={() => {}}
        workspaces={[workspace("ws1", "KlyntBot")]}
        workspaceIdByThread={new Map([["a", "ws1"]])}
      />,
    );
    expect(screen.getByText("Refactor auth")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /KlyntBot/ }));
    expect(screen.queryByText("Refactor auth")).not.toBeInTheDocument();
  });

  it("falls back to 'Other chats' when a thread has no known workspace", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "Orphan")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId={null}
        onSelectThread={() => {}}
        workspaces={[]}
        workspaceIdByThread={new Map()}
      />,
    );
    expect(screen.getByText("Other chats")).toBeInTheDocument();
    expect(screen.getByText("Orphan")).toBeInTheDocument();
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
        workspaces={[workspace("ws1", "KlyntBot")]}
        workspaceIdByThread={new Map([["a", "ws1"]])}
      />,
    );
    fireEvent.click(screen.getByText("Done one"));
    expect(onSelect).toHaveBeenCalledWith("a");
    expect(buf.getRecentlyCompleted().has("a")).toBe(false);
  });

  it("marks the active row with data-active='true' for the box highlight", async () => {
    const { CodingSidebarThreadsSection } = await import("./CodingSidebarThreadsSection");
    const { container } = render(
      <CodingSidebarThreadsSection
        sessions={[thread("a", "One"), thread("b", "Two")]}
        runningIds={new Set()}
        recentlyCompleted={new Map()}
        activeThreadId="a"
        onSelectThread={() => {}}
        workspaces={[workspace("ws1", "KlyntBot")]}
        workspaceIdByThread={
          new Map([
            ["a", "ws1"],
            ["b", "ws1"],
          ])
        }
      />,
    );
    const active = container.querySelectorAll('[data-active="true"]');
    expect(active).toHaveLength(1);
    expect(active[0].textContent).toContain("One");
  });
});
