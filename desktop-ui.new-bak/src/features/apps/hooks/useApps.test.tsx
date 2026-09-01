// @vitest-environment jsdom
import { getAppsList } from "@services/tauri";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";
import { useApps } from "./useApps";

vi.mock("../../../services/tauri", () => ({
  getAppsList: vi.fn(),
}));

const getAppsListMock = vi.mocked(getAppsList);

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);

const workspace: WorkspaceInfo = {
  id: "workspace-1",
  name: "Klynt",
  path: "/tmp/codex",
  connected: true,
  settings: { sidebarCollapsed: false },
};

describe("useApps", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("returns empty when no workspace is active", async () => {
    const { result } = renderHook(() => useApps({ activeWorkspace: null, activeThreadId: null }), {
      wrapper,
    });

    await waitFor(() => {
      expect(result.current.apps).toEqual([]);
    });
    expect(getAppsListMock).not.toHaveBeenCalled();
  });

  it("fetches apps for the active workspace", async () => {
    getAppsListMock.mockResolvedValue([{ id: "app-a", name: "App A", isAccessible: true }]);

    const { result } = renderHook(
      () => useApps({ activeWorkspace: workspace, activeThreadId: "thread-1" }),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.apps).toEqual([
        expect.objectContaining({ id: "app-a", name: "App A" }),
      ]);
    });
    expect(getAppsListMock).toHaveBeenCalledWith("workspace-1", null, 100, "thread-1");
  });

  it("refetches when the active thread changes", async () => {
    getAppsListMock.mockImplementation(async (_ws, _cursor, _limit, threadId) =>
      threadId === "thread-1"
        ? [{ id: "app-a", name: "App A", isAccessible: true }]
        : [{ id: "app-b", name: "App B", isAccessible: true }],
    );

    const { result, rerender } = renderHook(
      ({ activeThreadId }) => useApps({ activeWorkspace: workspace, activeThreadId }),
      { wrapper, initialProps: { activeThreadId: "thread-1" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.apps[0]?.id).toBe("app-a");
    });

    rerender({ activeThreadId: "thread-2" });

    await waitFor(() => {
      expect(result.current.apps[0]?.id).toBe("app-b");
      expect(getAppsListMock).toHaveBeenCalledWith("workspace-1", null, 100, "thread-2");
    });
  });

  it("refetches when the active workspace changes", async () => {
    getAppsListMock.mockImplementation(async (workspaceId) =>
      workspaceId === "workspace-1"
        ? [{ id: "ws1-app", name: "WS1 App", isAccessible: true }]
        : [{ id: "ws2-app", name: "WS2 App", isAccessible: true }],
    );

    const { result, rerender } = renderHook(
      ({ activeWorkspace }) => useApps({ activeWorkspace, activeThreadId: null }),
      { wrapper, initialProps: { activeWorkspace: workspace } },
    );

    await waitFor(() => {
      expect(result.current.apps[0]?.id).toBe("ws1-app");
    });

    const ws2: WorkspaceInfo = { ...workspace, id: "workspace-2", name: "Workspace 2" };
    rerender({ activeWorkspace: ws2 });

    await waitFor(() => {
      expect(result.current.apps[0]?.id).toBe("ws2-app");
    });
  });

  it("filters out entries missing id or name", async () => {
    getAppsListMock.mockResolvedValue([
      { id: "ok", name: "OK", isAccessible: true },
      { id: "", name: "no-id", isAccessible: true },
      { id: "no-name", name: "", isAccessible: true },
    ]);

    const { result } = renderHook(
      () => useApps({ activeWorkspace: workspace, activeThreadId: null }),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.apps).toHaveLength(1);
      expect(result.current.apps[0]?.id).toBe("ok");
    });
  });
});
