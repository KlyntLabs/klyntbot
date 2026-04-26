// @vitest-environment jsdom

import { getGlobalPromptsDir } from "@services/tauri";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";
import { useCustomPrompts } from "./useCustomPrompts";

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);

vi.mock("../../../services/tauri", () => ({
  createPrompt: vi.fn(),
  deletePrompt: vi.fn(),
  getPromptsList: vi.fn(),
  getGlobalPromptsDir: vi.fn(),
  getWorkspacePromptsDir: vi.fn(),
  movePrompt: vi.fn(),
  updatePrompt: vi.fn(),
}));

const getGlobalPromptsDirMock = vi.mocked(getGlobalPromptsDir);

const workspace: WorkspaceInfo = {
  id: "ws-1",
  name: "Workspace",
  path: "/tmp/workspace",
  connected: false,
  settings: { sidebarCollapsed: false },
};

describe("useCustomPrompts", () => {
  it("returns null when no workspace is selected", async () => {
    const { result } = renderHook(() => useCustomPrompts(null), { wrapper });

    let path: string | null = "unset";
    await act(async () => {
      path = await result.current.getGlobalPromptsDir();
    });

    expect(path).toBeNull();
    expect(getGlobalPromptsDirMock).not.toHaveBeenCalled();
  });

  it("requests the global prompts dir when a workspace is selected", async () => {
    getGlobalPromptsDirMock.mockResolvedValue("/tmp/.codex/prompts");
    const { result } = renderHook(() => useCustomPrompts(workspace), { wrapper });

    let path: string | null = null;
    await act(async () => {
      path = await result.current.getGlobalPromptsDir();
    });

    expect(getGlobalPromptsDirMock).toHaveBeenCalledWith("ws-1");
    expect(path).toBe("/tmp/.codex/prompts");
  });
});
