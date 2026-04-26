// @vitest-environment jsdom

import { connectWorkspace, getConfigModel, getModelList } from "@services/tauri";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";
import { useSettingsDefaultModels } from "./useSettingsDefaultModels";

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);

vi.mock("@services/tauri", () => ({
  connectWorkspace: vi.fn(),
  getConfigModel: vi.fn(),
  getModelList: vi.fn(),
}));

const connectWorkspaceMock = vi.mocked(connectWorkspace);
const getConfigModelMock = vi.mocked(getConfigModel);
const getModelListMock = vi.mocked(getModelList);

function workspace(id: string, connected = true): WorkspaceInfo {
  return {
    id,
    name: `Workspace ${id}`,
    path: `/tmp/${id}`,
    connected,
    settings: { sidebarCollapsed: false },
  };
}

function modelListResponse(model: string) {
  return {
    result: {
      data: [
        {
          id: model,
          model,
          displayName: model,
          description: "",
          supportedReasoningEfforts: [],
          defaultReasoningEffort: null,
          isDefault: false,
        },
      ],
    },
  };
}

describe("useSettingsDefaultModels", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("returns empty state when no projects are provided", async () => {
    const { result } = renderHook(
      ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
      { wrapper, initialProps: { projects: [] } },
    );

    await waitFor(() => {
      expect(result.current.models).toEqual([]);
      expect(result.current.connectedWorkspaceCount).toBe(0);
    });
    expect(getModelListMock).not.toHaveBeenCalled();
  });

  it("aggregates models across all workspaces and dedupes by id", async () => {
    connectWorkspaceMock.mockResolvedValue(undefined);
    getConfigModelMock.mockResolvedValue(null);
    getModelListMock.mockResolvedValue(modelListResponse("gpt-5.1"));

    const { result } = renderHook(
      ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
      {
        wrapper,
        initialProps: { projects: [workspace("w1", true), workspace("w2", true)] },
      },
    );

    await waitFor(() => {
      expect(connectWorkspaceMock).toHaveBeenCalledWith("w1");
      expect(connectWorkspaceMock).toHaveBeenCalledWith("w2");
      expect(getModelListMock).toHaveBeenCalledWith("w1");
      expect(getModelListMock).toHaveBeenCalledWith("w2");
      // Same model reported by both workspaces — deduped to one entry.
      expect(result.current.models).toHaveLength(1);
      expect(result.current.models[0]?.model).toBe("gpt-5.1");
      expect(result.current.connectedWorkspaceCount).toBe(2);
    });
  });

  it("includes the codex config model with CONFIG_MODEL_DESCRIPTION", async () => {
    connectWorkspaceMock.mockResolvedValueOnce(undefined);
    getConfigModelMock.mockResolvedValueOnce("gpt-5-codex");
    getModelListMock.mockResolvedValueOnce({ result: { data: [] } });

    const { result } = renderHook(
      ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
      { wrapper, initialProps: { projects: [workspace("w1", true)] } },
    );

    await waitFor(() => {
      const codex = result.current.models.find((m) => m.model === "gpt-5-codex");
      expect(codex).toBeDefined();
      expect(codex?.description).toBe("Configured in CODEX_HOME/config.toml");
      expect(codex?.isDefault).toBe(true);
    });
  });

  it("records the last error when connectWorkspace fails", async () => {
    connectWorkspaceMock.mockRejectedValueOnce(new Error("connect failed"));

    const { result } = renderHook(
      ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
      { wrapper, initialProps: { projects: [workspace("w1", true)] } },
    );

    await waitFor(() => {
      expect(result.current.error).toContain("connect failed");
      expect(result.current.connectedWorkspaceCount).toBe(0);
      expect(getModelListMock).not.toHaveBeenCalled();
    });
  });
});
