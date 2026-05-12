import { useMemo } from "react";
import { useChatStore } from "@/features/threads/store/useChatStore";
import type { ConversationItem } from "@/types";
import { getActiveItemsForThread, getActiveThreadIdForWorkspace } from "./threadSelectorsHelpers";
import type { ThreadState } from "./useThreadsReducer";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;
const EMPTY_APPROVALS: ApprovalItem[] = [];

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

  const approvals = useChatStore(
    (store) => store.streamApprovals[activeThreadId ?? ""] ?? EMPTY_APPROVALS,
  );

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
