// @vitest-environment jsdom
import { getGitStatus } from "@services/tauri";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceInfo } from "@/types";
import { useGitStatus } from "./useGitStatus";

vi.mock("../../../services/tauri", () => ({
  getGitStatus: vi.fn(),
}));

function withQuery({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: 0 } },
  });
  return createElement(QueryClientProvider, { client }, children);
}

const workspace: WorkspaceInfo = {
  id: "workspace-1",
  name: "Klynt",
  path: "/tmp/codex",
  connected: true,
  settings: { sidebarCollapsed: false },
};

const secondaryWorkspace: WorkspaceInfo = {
  id: "workspace-2",
  name: "Klynt Secondary",
  path: "/tmp/codex-secondary",
  connected: true,
  settings: { sidebarCollapsed: false },
};

const makeStatus = (branchName: string, additions = 0, deletions = 0) => ({
  branchName,
  files: [],
  stagedFiles: [],
  unstagedFiles: [],
  totalAdditions: additions,
  totalDeletions: deletions,
});

describe("useGitStatus", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetches status for the active workspace", async () => {
    const getGitStatusMock = vi.mocked(getGitStatus);
    getGitStatusMock.mockResolvedValue(makeStatus("main", 2, 1));

    const { result } = renderHook(
      ({ active }: { active: WorkspaceInfo | null }) => useGitStatus(active),
      { initialProps: { active: workspace }, wrapper: withQuery },
    );

    await waitFor(() => {
      expect(result.current.status.branchName).toBe("main");
      expect(result.current.status.totalAdditions).toBe(2);
    });
    expect(getGitStatusMock).toHaveBeenCalledWith("workspace-1");
  });

  it("refresh() triggers a refetch", async () => {
    const getGitStatusMock = vi.mocked(getGitStatus);
    getGitStatusMock
      .mockResolvedValueOnce(makeStatus("main", 1, 0))
      .mockResolvedValueOnce(makeStatus("manual", 5, 6));

    const { result } = renderHook(
      ({ active }: { active: WorkspaceInfo | null }) => useGitStatus(active),
      { initialProps: { active: workspace }, wrapper: withQuery },
    );

    await waitFor(() => expect(result.current.status.branchName).toBe("main"));

    await act(async () => {
      await result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.status.branchName).toBe("manual");
      expect(result.current.status.totalAdditions).toBe(5);
    });
  });

  it("refetches when the active workspace changes", async () => {
    const getGitStatusMock = vi.mocked(getGitStatus);
    getGitStatusMock.mockImplementation(async (id: string) =>
      id === "workspace-1" ? makeStatus("primary", 1, 1) : makeStatus("secondary", 4, 0),
    );

    const { result, rerender } = renderHook(
      ({ active }: { active: WorkspaceInfo | null }) => useGitStatus(active),
      { initialProps: { active: workspace }, wrapper: withQuery },
    );

    await waitFor(() => expect(result.current.status.branchName).toBe("primary"));

    rerender({ active: secondaryWorkspace });

    await waitFor(() => {
      expect(result.current.status.branchName).toBe("secondary");
      expect(getGitStatusMock).toHaveBeenCalledWith("workspace-2");
    });
  });

  it("returns empty status when no workspace is active (queryFn skipped)", async () => {
    const getGitStatusMock = vi.mocked(getGitStatus);

    const { result } = renderHook(
      ({ active }: { active: WorkspaceInfo | null }) => useGitStatus(active),
      { initialProps: { active: null }, wrapper: withQuery },
    );

    await waitFor(() => {
      expect(result.current.status.branchName).toBe("");
      expect(result.current.status.files).toEqual([]);
    });
    expect(getGitStatusMock).not.toHaveBeenCalled();
  });
});
