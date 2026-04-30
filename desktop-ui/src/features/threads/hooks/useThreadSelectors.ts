import { useCallback, useMemo, useSyncExternalStore } from "react";
import type { ConversationItem } from "@/types";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import { getActiveItemsForThread, getActiveThreadIdForWorkspace } from "./threadSelectorsHelpers";
import type { ThreadState } from "./useThreadsReducer";

type UseThreadSelectorsOptions = {
  activeWorkspaceId: string | null;
  activeThreadIdByWorkspace: ThreadState["activeThreadIdByWorkspace"];
  itemsByThread: ThreadState["itemsByThread"];
  threadsByWorkspace: ThreadState["threadsByWorkspace"];
};

export function useThreadSelectors({
  activeWorkspaceId,
  activeThreadIdByWorkspace,
  itemsByThread,
  threadsByWorkspace,
}: UseThreadSelectorsOptions) {
  const activeThreadId = useMemo(
    () => getActiveThreadIdForWorkspace(activeWorkspaceId, activeThreadIdByWorkspace),
    [activeThreadIdByWorkspace, activeWorkspaceId],
  );

  const activeWorkspaceThreads = activeWorkspaceId
    ? threadsByWorkspace[activeWorkspaceId]
    : undefined;

  const getApprovals = useCallback(
    () => chatStreamStore.getApprovals(activeThreadId ?? ""),
    [activeThreadId],
  );
  const approvals = useSyncExternalStore(chatStreamStore.subscribe, getApprovals);

  const activeItems = useMemo<ConversationItem[]>(
    () =>
      getActiveItemsForThread({
        activeThreadId,
        itemsByThread,
        threads: activeWorkspaceThreads,
        approvals,
      }),
    [activeThreadId, activeWorkspaceThreads, itemsByThread, approvals],
  );

  return { activeThreadId, activeItems };
}
