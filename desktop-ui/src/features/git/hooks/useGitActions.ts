import {
  applyWorktreeChanges as applyWorktreeChangesService,
  createGitHubRepo as createGitHubRepoService,
  initGitRepo as initGitRepoService,
  revertGitAll,
  revertGitFile as revertGitFileService,
  stageGitAll as stageGitAllService,
  stageGitFile as stageGitFileService,
  unstageGitFile as unstageGitFileService,
} from "@services/tauri";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback, useRef, useState } from "react";
import { qk, useTauriMutation } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";

export type InitGitRepoOutcome = "initialized" | "cancelled" | "failed";

export function useGitActions(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? null;
  const workspaceIdRef = useRef<string | null>(workspaceId);
  const isWorktree = activeWorkspace?.kind === "worktree";

  const [worktreeApplyError, setWorktreeApplyError] = useState<string | null>(null);
  const [worktreeApplySuccess, setWorktreeApplySuccess] = useState(false);

  const invalidatesGit = workspaceId
    ? [
        qk.git.status(workspaceId),
        qk.git.diffs(workspaceId),
        qk.git.log(workspaceId),
        qk.git.branches(workspaceId),
      ]
    : [];

  const stageFileMut = useTauriMutation({
    mutationFn: ({ path }: { path: string }) => {
      if (!workspaceId) throw new Error("no workspace");
      return stageGitFileService(workspaceId, path);
    },
    invalidates: invalidatesGit,
  });

  const stageAllMut = useTauriMutation({
    mutationFn: () => {
      if (!workspaceId) throw new Error("no workspace");
      return stageGitAllService(workspaceId);
    },
    invalidates: invalidatesGit,
  });

  const unstageFileMut = useTauriMutation({
    mutationFn: ({ path }: { path: string }) => {
      if (!workspaceId) throw new Error("no workspace");
      return unstageGitFileService(workspaceId, path);
    },
    invalidates: invalidatesGit,
  });

  const revertFileMut = useTauriMutation({
    mutationFn: ({ path }: { path: string }) => {
      if (!workspaceId) throw new Error("no workspace");
      return revertGitFileService(workspaceId, path);
    },
    invalidates: invalidatesGit,
  });

  const revertAllMut = useTauriMutation({
    mutationFn: () => {
      if (!workspaceId) throw new Error("no workspace");
      return revertGitAll(workspaceId);
    },
    invalidates: invalidatesGit,
  });

  const applyWorktreeMut = useTauriMutation({
    mutationFn: () => {
      if (!workspaceId) throw new Error("no workspace");
      return applyWorktreeChangesService(workspaceId);
    },
    invalidates: invalidatesGit,
  });

  const initRepoMut = useTauriMutation({
    mutationFn: async ({ branch, force = false }: { branch: string; force?: boolean }) => {
      if (!workspaceId) throw new Error("no workspace");
      return await initGitRepoService(workspaceId, branch, force);
    },
    invalidates: invalidatesGit,
  });

  const createGitHubRepoMut = useTauriMutation({
    mutationFn: async ({
      repo,
      visibility,
      branch,
    }: {
      repo: string;
      visibility: "private" | "public";
      branch: string;
    }) => {
      if (!workspaceId) throw new Error("no workspace");
      return await createGitHubRepoService(workspaceId, repo, visibility, branch);
    },
    invalidates: invalidatesGit,
  });

  const stageGitFile = useCallback(
    (path: string) => {
      stageFileMut.mutate({ path });
    },
    [stageFileMut],
  );
  const stageGitAll = useCallback(() => {
    stageAllMut.mutate();
  }, [stageAllMut]);
  const unstageGitFile = useCallback(
    (path: string) => {
      unstageFileMut.mutate({ path });
    },
    [unstageFileMut],
  );
  const revertGitFile = useCallback(
    (path: string) => {
      revertFileMut.mutate({ path });
    },
    [revertFileMut],
  );

  const revertAllGitChanges = useCallback(async () => {
    if (!workspaceId) {
      return;
    }
    const confirmed = await ask(
      "Revert all changes in this repo?\n\nThis will discard all staged and unstaged changes, including untracked files.",
      { title: "Revert all changes", kind: "warning" },
    );
    if (!confirmed) {
      return;
    }
    await revertAllMut.mutate();
  }, [revertAllMut, workspaceId]);

  const applyWorktreeChanges = useCallback(async () => {
    if (!workspaceId || !isWorktree) {
      return;
    }
    setWorktreeApplyError(null);
    setWorktreeApplySuccess(false);
    try {
      await applyWorktreeMut.mutate();
      setWorktreeApplySuccess(true);
    } catch (error) {
      setWorktreeApplyError(error instanceof Error ? error.message : String(error));
    }
  }, [applyWorktreeMut, isWorktree, workspaceId]);

  const initGitRepo = useCallback(
    async (branch: string): Promise<InitGitRepoOutcome> => {
      if (!workspaceId) {
        return "failed";
      }
      const actionWorkspaceId = workspaceId;
      let shouldRefresh = false;
      let outcome: InitGitRepoOutcome = "failed";
      let commitError: string | null = null;
      try {
        const response = await initRepoMut.mutate({
          branch,
          force: false,
        });
        if (workspaceIdRef.current !== actionWorkspaceId) {
          return "cancelled";
        }

        if (response.status === "needs_confirmation") {
          const entryCount = response.entryCount ?? 0;
          const plural = entryCount === 1 ? "" : "s";
          const confirmed = await ask(
            `Initialize Git in this folder?\n\nThis will create a .git directory, set the initial branch to "${branch}", and create an initial commit.\n\nThis folder contains ${entryCount} existing item${plural}.`,
            {
              title: "Initialize Git",
              kind: "warning",
              okLabel: "Initialize",
              cancelLabel: "Cancel",
            },
          );
          if (!confirmed) {
            return "cancelled";
          }

          if (workspaceIdRef.current !== actionWorkspaceId) {
            return "cancelled";
          }

          const forced = await initRepoMut.mutate({
            branch,
            force: true,
          });
          shouldRefresh =
            forced.status === "initialized" || forced.status === "already_initialized";
          if (forced.status === "initialized") {
            commitError = forced.commitError ?? null;
          }
          outcome = shouldRefresh ? "initialized" : "failed";
        } else {
          shouldRefresh =
            response.status === "initialized" || response.status === "already_initialized";
          if (response.status === "initialized") {
            commitError = response.commitError ?? null;
          }
          outcome = shouldRefresh ? "initialized" : "failed";
        }

        if (commitError) {
          // eslint-disable-next-line no-console
          console.error(
            `Git was initialized, but the initial commit failed.\n\n${commitError}\n\nYou may need to set git user.name and user.email, then commit manually.`,
          );
        }
      } catch {
        outcome = "failed";
      }
      return outcome;
    },
    [initRepoMut, workspaceId],
  );

  const createGitHubRepo = useCallback(
    async (
      repo: string,
      visibility: "private" | "public",
      branch: string,
    ): Promise<{ ok: true } | { ok: false; error: string }> => {
      if (!workspaceId) {
        return { ok: false, error: "No active workspace." };
      }

      const actionWorkspaceId = workspaceId;
      try {
        const response = await createGitHubRepoMut.mutate({
          repo,
          visibility,
          branch,
        });
        if (workspaceIdRef.current !== actionWorkspaceId) {
          return { ok: false, error: "Workspace changed." };
        }

        if (response.status === "ok") {
          return { ok: true };
        }

        const pushError = response.pushError?.trim() ?? "";
        const defaultBranchError = response.defaultBranchError?.trim() ?? "";
        const parts = [];
        if (pushError) {
          parts.push(`Push failed:\n${pushError}`);
        }
        if (defaultBranchError) {
          parts.push(`Failed to set default branch:\n${defaultBranchError}`);
        }
        const errorMessage =
          parts.length > 0
            ? parts.join("\n\n")
            : "Remote repo was created, but setup was incomplete.";
        return { ok: false, error: errorMessage };
      } catch (error) {
        if (workspaceIdRef.current !== actionWorkspaceId) {
          return { ok: false, error: "Workspace changed." };
        }
        return {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
    [createGitHubRepoMut, workspaceId],
  );

  return {
    applyWorktreeChanges,
    createGitHubRepo,
    createGitHubRepoLoading: createGitHubRepoMut.isLoading,
    initGitRepo,
    initGitRepoLoading: initRepoMut.isLoading,
    revertAllGitChanges,
    revertGitFile,
    stageGitAll,
    stageGitFile,
    unstageGitFile,
    worktreeApplyError,
    worktreeApplyLoading: applyWorktreeMut.isLoading,
    worktreeApplySuccess,
  };
}
