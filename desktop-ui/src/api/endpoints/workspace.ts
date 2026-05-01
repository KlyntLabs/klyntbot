import type { WorkspaceInfo, WorkspaceSettings } from "@/types";
import { invoke, isMissingTauriInvokeError } from "../client";

export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  try {
    return await invoke<WorkspaceInfo[]>("list_workspaces");
  } catch (error) {
    if (isMissingTauriInvokeError(error)) {
      return [];
    }
    throw error;
  }
}

export async function addWorkspace(path: string): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("add_workspace", { path });
}

export async function addWorkspaceFromGitUrl(
  url: string,
  destinationPath: string,
  targetFolderName: string | null,
): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("add_workspace_from_git_url", {
    url,
    destinationPath,
    targetFolderName,
  });
}

export async function isWorkspacePathDir(path: string): Promise<boolean> {
  return invoke<boolean>("is_workspace_path_dir", { path });
}

export async function updateWorkspaceSettings(
  id: string,
  settings: WorkspaceSettings,
): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("update_workspace_settings", { id, settings });
}

export async function removeWorkspace(id: string): Promise<void> {
  return invoke("remove_workspace", { id });
}

export async function connectWorkspace(id: string): Promise<void> {
  return invoke("connect_workspace", { id });
}

export async function setWorkspaceRuntimeCodexArgs(
  workspaceId: string,
  codexArgs: string | null,
): Promise<{ appliedCodexArgs: string | null; respawned: boolean }> {
  return invoke("set_workspace_runtime_codex_args", {
    workspaceId,
    codexArgs,
  });
}

export async function openWorkspaceIn(
  path: string,
  options: {
    appName?: string | null;
    command?: string | null;
    args?: string[];
    line?: number | null;
    column?: number | null;
  },
): Promise<void> {
  return invoke("open_workspace_in", {
    path,
    app: options.appName ?? null,
    command: options.command ?? null,
    args: options.args ?? [],
    line: options.line ?? null,
    column: options.column ?? null,
  });
}
