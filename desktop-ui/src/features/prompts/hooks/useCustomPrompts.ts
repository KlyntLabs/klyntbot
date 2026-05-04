import {
  createPrompt as createPromptService,
  deletePrompt as deletePromptService,
  getGlobalPromptsDir as getGlobalPromptsDirService,
  getPromptsList,
  getWorkspacePromptsDir as getWorkspacePromptsDirService,
  movePrompt as movePromptService,
  updatePrompt as updatePromptService,
} from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import type { CustomPromptOption, WorkspaceInfo } from "@/types";

export function useCustomPrompts(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<CustomPromptOption[]>({
    queryKey: qk.prompts.list(workspaceId),
    staleTime: 60_000,
    queryFn: async () => {
      if (!activeWorkspace) return [];
      const list = await getPromptsList(activeWorkspace.id);
      return list.filter((p) => Boolean(p.name));
    },
    fallback: [],
    enabled: activeWorkspace !== null,
  });

  const create = useTauriMutation<
    void,
    {
      scope: "workspace" | "global";
      name: string;
      description?: string | null;
      argumentHint?: string | null;
      content: string;
    }
  >({
    mutationFn: async (data) => {
      if (!activeWorkspace) throw new Error("no workspace");
      await createPromptService(activeWorkspace.id, data);
    },
    invalidates: [qk.prompts.list(workspaceId)],
  });

  const update = useTauriMutation<
    void,
    {
      path: string;
      name: string;
      description?: string | null;
      argumentHint?: string | null;
      content: string;
    }
  >({
    mutationFn: async (data) => {
      if (!activeWorkspace) throw new Error("no workspace");
      await updatePromptService(activeWorkspace.id, data);
    },
    invalidates: [qk.prompts.list(workspaceId)],
  });

  const remove = useTauriMutation<void, { path: string }>({
    mutationFn: async (data) => {
      if (!activeWorkspace) throw new Error("no workspace");
      await deletePromptService(activeWorkspace.id, data.path);
    },
    invalidates: [qk.prompts.list(workspaceId)],
  });

  const move = useTauriMutation<void, { path: string; scope: "workspace" | "global" }>({
    mutationFn: async (data) => {
      if (!activeWorkspace) throw new Error("no workspace");
      await movePromptService(activeWorkspace.id, data);
    },
    invalidates: [qk.prompts.list(workspaceId)],
  });

  const refreshPrompts = useCallback(async () => {
    await query.refetch();
  }, [query]);

  const getWorkspacePromptsDir = useCallback(async () => {
    if (!activeWorkspace) return null;
    return await getWorkspacePromptsDirService(activeWorkspace.id);
  }, [activeWorkspace]);

  const getGlobalPromptsDir = useCallback(async () => {
    if (!activeWorkspace) return null;
    return await getGlobalPromptsDirService(activeWorkspace.id);
  }, [activeWorkspace]);

  return {
    prompts: query.data,
    refreshPrompts,
    createPrompt: create.mutate,
    updatePrompt: update.mutate,
    deletePrompt: (path: string) => remove.mutate({ path }),
    movePrompt: move.mutate,
    getWorkspacePromptsDir,
    getGlobalPromptsDir,
  };
}
