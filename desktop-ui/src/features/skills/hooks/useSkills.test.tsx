// @vitest-environment jsdom
import { getSkillsList } from "@services/tauri";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";
import { useSkills } from "./useSkills";

vi.mock("../../../services/tauri", () => ({
  getSkillsList: vi.fn(),
}));

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);

const workspace: WorkspaceInfo = {
  id: "workspace-1",
  name: "Workspace One",
  path: "/tmp/workspace-one",
  connected: true,
  settings: { sidebarCollapsed: false },
};

describe("useSkills", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("returns empty when no workspace is active", async () => {
    const { result } = renderHook(() => useSkills(null), { wrapper });

    await waitFor(() => {
      expect(result.current.skills).toEqual([]);
    });
    expect(getSkillsList).not.toHaveBeenCalled();
  });

  it("fetches skills for the active workspace", async () => {
    vi.mocked(getSkillsList).mockResolvedValueOnce([{ name: "first", path: "/skills/first" }]);

    const { result } = renderHook(() => useSkills(workspace), { wrapper });

    await waitFor(() => {
      expect(result.current.skills.map((s) => s.name)).toEqual(["first"]);
    });
    expect(getSkillsList).toHaveBeenCalledWith("workspace-1");
  });

  it("filters out entries missing a name", async () => {
    vi.mocked(getSkillsList).mockResolvedValueOnce([
      { name: "first", path: "/skills/first" },
      { name: "", path: "/skills/blank" },
    ]);

    const { result } = renderHook(() => useSkills(workspace), { wrapper });

    await waitFor(() => {
      expect(result.current.skills).toHaveLength(1);
      expect(result.current.skills[0]?.name).toBe("first");
    });
  });

  it("refetches when the active workspace changes", async () => {
    vi.mocked(getSkillsList).mockImplementation(async (id) =>
      id === "workspace-1"
        ? [{ name: "ws1-skill", path: "/skills/ws1" }]
        : [{ name: "ws2-skill", path: "/skills/ws2" }],
    );

    const { result, rerender } = renderHook(({ ws }) => useSkills(ws), {
      wrapper,
      initialProps: { ws: workspace },
    });

    await waitFor(() => {
      expect(result.current.skills[0]?.name).toBe("ws1-skill");
    });

    rerender({ ws: { ...workspace, id: "workspace-2" } });

    await waitFor(() => {
      expect(result.current.skills[0]?.name).toBe("ws2-skill");
      expect(getSkillsList).toHaveBeenCalledWith("workspace-2");
    });
  });
});
