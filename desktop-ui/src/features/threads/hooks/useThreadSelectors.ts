import { useMemo } from "react";
import type { ConversationItem } from "@/types";
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

  const activeItems = useMemo<ConversationItem[]>(
    () =>
      getActiveItemsForThread({
        activeThreadId,
        itemsByThread,
        threads: activeWorkspaceThreads,
      }),
    [activeThreadId, activeWorkspaceThreads, itemsByThread],
  );

  return { activeThreadId, activeItems };
}
