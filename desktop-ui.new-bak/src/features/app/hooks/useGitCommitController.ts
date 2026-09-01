import {
  commitGit,
  fetchGit,
  generateCommitMessage,
  pullGit,
  pushGit,
  stageGitAll,
  syncGit,
} from "@services/tauri";
import { shouldApplyCommitMessage } from "@utils/commitMessage";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { useGitStatus } from "@/features/git/hooks/useGitStatus";
import { useTauriMutation } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";

type GitStatusState = ReturnType<typeof useGitStatus>["status"];

type GitCommitControllerOptions = {
  activeWorkspace: WorkspaceInfo | null;
  activeWorkspaceId: string | null;
  commitMessageModelId: string | null;
  gitStatus: GitStatusState;
};

type GitCommitController = {
  commitMessage: string;
  commitMessageLoading: boolean;
  commitMessageError: string | null;
  commitLoading: boolean;
  pullLoading: boolean;
  fetchLoading: boolean;
  pushLoading: boolean;
  syncLoading: boolean;
  commitError: string | null;
  pullError: string | null;
  fetchError: string | null;
  pushError: string | null;
  syncError: string | null;
  hasWorktreeChanges: boolean;
  onCommitMessageChange: (value: string) => void;
  onGenerateCommitMessage: () => Promise<void>;
  onCommit: () => Promise<void>;
  onCommitAndPush: () => Promise<void>;
  onCommitAndSync: () => Promise<void>;
  onPull: () => Promise<void>;
  onFetch: () => Promise<void>;
  onPush: () => Promise<void>;
  onSync: () => Promise<void>;
};

function extractErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useGitCommitController({
  activeWorkspace,
  activeWorkspaceId,
  commitMessageModelId,
  gitStatus,
}: GitCommitControllerOptions): GitCommitController {
  const [commitMessage, setCommitMessage] = useState("");
  const [commitMessageLoading, setCommitMessageLoading] = useState(false);
  const [commitMessageError, setCommitMessageError] = useState<string | null>(null);

  const [isCommitAndPushRunning, setIsCommitAndPushRunning] = useState(false);
  const [isCommitAndSyncRunning, setIsCommitAndSyncRunning] = useState(false);

  const hasWorktreeChanges = useMemo(() => {
    const hasStagedChanges = gitStatus.stagedFiles.length > 0;
    const hasUnstagedChanges = gitStatus.unstagedFiles.length > 0;
    return hasStagedChanges || hasUnstagedChanges;
  }, [gitStatus.stagedFiles.length, gitStatus.unstagedFiles.length]);

  const ensureStagedForCommit = useCallback(async () => {
    const hasStagedChanges = gitStatus.stagedFiles.length > 0;
    const hasUnstagedChanges = gitStatus.unstagedFiles.length > 0;
    if (!activeWorkspace || hasStagedChanges || !hasUnstagedChanges) {
      return;
    }
    await stageGitAll(activeWorkspace.id);
  }, [activeWorkspace, gitStatus.stagedFiles.length, gitStatus.unstagedFiles.length]);

  const generateMsg = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      return await generateCommitMessage(activeWorkspace.id, commitMessageModelId);
    },
  });

  const commit = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      await ensureStagedForCommit();
      await commitGit(activeWorkspace.id, commitMessage.trim());
    },
  });

  const push = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      await pushGit(activeWorkspace.id);
    },
  });

  const pull = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      await pullGit(activeWorkspace.id);
    },
  });

  const fetch = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      await fetchGit(activeWorkspace.id);
    },
  });

  const sync = useTauriMutation({
    mutationFn: async () => {
      if (!activeWorkspace) throw new Error("no workspace");
      await syncGit(activeWorkspace.id);
    },
  });

  const handleCommitMessageChange = useCallback((value: string) => {
    setCommitMessage(value);
  }, []);

  const handleGenerateCommitMessage = useCallback(async () => {
    if (!activeWorkspace || commitMessageLoading) {
      return;
    }
    const workspaceId = activeWorkspace.id;
    setCommitMessageLoading(true);
    setCommitMessageError(null);
    try {
      const message = await generateMsg.mutate();
      if (!shouldApplyCommitMessage(activeWorkspaceId, workspaceId)) {
        return;
      }
      setCommitMessage(message);
    } catch (error) {
      if (!shouldApplyCommitMessage(activeWorkspaceId, workspaceId)) {
        return;
      }
      setCommitMessageError(extractErrorMessage(error));
    } finally {
      if (shouldApplyCommitMessage(activeWorkspaceId, workspaceId)) {
        setCommitMessageLoading(false);
      }
    }
  }, [activeWorkspace, activeWorkspaceId, commitMessageLoading, generateMsg]);

  useEffect(() => {
    setCommitMessage("");
    setCommitMessageError(null);
    setCommitMessageLoading(false);
  }, []);

  const handleCommit = useCallback(async () => {
    if (!activeWorkspace || commit.isLoading || !commitMessage.trim() || !hasWorktreeChanges) {
      return;
    }
    try {
      await commit.mutate();
      setCommitMessage("");
    } catch {
      // error is surfaced via commit.error
    }
  }, [activeWorkspace, commit, commitMessage, hasWorktreeChanges]);

  const handleCommitAndPush = useCallback(async () => {
    if (
      !activeWorkspace ||
      commit.isLoading ||
      push.isLoading ||
      !commitMessage.trim() ||
      !hasWorktreeChanges
    ) {
      return;
    }
    setIsCommitAndPushRunning(true);
    try {
      await commit.mutate();
      setCommitMessage("");
      await push.mutate();
    } catch {
      // error surfaced via mutation.error
    } finally {
      setIsCommitAndPushRunning(false);
    }
  }, [activeWorkspace, commit, push, commitMessage, hasWorktreeChanges]);

  const handleCommitAndSync = useCallback(async () => {
    if (
      !activeWorkspace ||
      commit.isLoading ||
      sync.isLoading ||
      !commitMessage.trim() ||
      !hasWorktreeChanges
    ) {
      return;
    }
    setIsCommitAndSyncRunning(true);
    try {
      await commit.mutate();
      setCommitMessage("");
      await sync.mutate();
    } catch {
      // error surfaced via mutation.error
    } finally {
      setIsCommitAndSyncRunning(false);
    }
  }, [activeWorkspace, commit, sync, commitMessage, hasWorktreeChanges]);

  const handlePull = useCallback(async () => {
    if (!activeWorkspace || pull.isLoading) {
      return;
    }
    try {
      await pull.mutate();
    } catch {
      // error surfaced via pull.error
    }
  }, [activeWorkspace, pull]);

  const handlePush = useCallback(async () => {
    if (!activeWorkspace || push.isLoading) {
      return;
    }
    try {
      await push.mutate();
    } catch {
      // error surfaced via push.error
    }
  }, [activeWorkspace, push]);

  const handleFetch = useCallback(async () => {
    if (!activeWorkspace || fetch.isLoading) {
      return;
    }
    try {
      await fetch.mutate();
    } catch {
      // error surfaced via fetch.error
    }
  }, [activeWorkspace, fetch]);

  const handleSync = useCallback(async () => {
    if (!activeWorkspace || sync.isLoading) {
      return;
    }
    try {
      await sync.mutate();
    } catch {
      // error surfaced via sync.error
    }
  }, [activeWorkspace, sync]);

  return {
    commitMessage,
    commitMessageLoading,
    commitMessageError,
    commitLoading: commit.isLoading || isCommitAndPushRunning || isCommitAndSyncRunning,
    pullLoading: pull.isLoading,
    fetchLoading: fetch.isLoading,
    pushLoading: push.isLoading || isCommitAndPushRunning,
    syncLoading: sync.isLoading || isCommitAndSyncRunning,
    commitError: commit.error ? extractErrorMessage(commit.error) : null,
    pullError: pull.error ? extractErrorMessage(pull.error) : null,
    fetchError: fetch.error ? extractErrorMessage(fetch.error) : null,
    pushError: push.error ? extractErrorMessage(push.error) : null,
    syncError: sync.error ? extractErrorMessage(sync.error) : null,
    hasWorktreeChanges,
    onCommitMessageChange: handleCommitMessageChange,
    onGenerateCommitMessage: handleGenerateCommitMessage,
    onCommit: handleCommit,
    onCommitAndPush: handleCommitAndPush,
    onCommitAndSync: handleCommitAndSync,
    onPull: handlePull,
    onFetch: handleFetch,
    onPush: handlePush,
    onSync: handleSync,
  };
}
