import { enrichConversationItemsWithThreads } from "@utils/threadItems";
import type { ConversationItem, ThreadSummary } from "@/types";

export function getActiveThreadIdForWorkspace(
  activeWorkspaceId: string | null,
  activeThreadIdByWorkspace: Record<string, string | null | undefined>,
) {
  if (!activeWorkspaceId) {
    return null;
  }
  return activeThreadIdByWorkspace[activeWorkspaceId] ?? null;
}

export function getActiveItemsForThread({
  activeThreadId,
  itemsByThread,
  threads,
  approvals,
}: {
  activeThreadId: string | null;
  itemsByThread: Record<string, ConversationItem[]>;
  threads: ThreadSummary[] | undefined;
  approvals?: ConversationItem[];
}) {
  if (!activeThreadId) {
    return [];
  }
  const items = itemsByThread[activeThreadId] ?? [];
  const enriched = enrichConversationItemsWithThreads(items, threads ?? []);
  if (!approvals || approvals.length === 0) return enriched;
  return [...enriched, ...approvals];
}
