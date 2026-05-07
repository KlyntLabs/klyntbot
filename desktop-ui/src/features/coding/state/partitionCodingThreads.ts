import type { ChatThread } from "@/features/chat/types";

export interface CodingThreadGroups {
  running: ChatThread[];
  recent: ChatThread[];
  chats: ChatThread[];
}

/**
 * Partition the flat coding-sessions list into the three sidebar groups.
 *
 * Rules:
 * - A session id in `runningIds` always lands in `running`, even if it is
 *   also present in `recent` (running takes precedence — turn restarted).
 * - A session id in `recent` (and not in `runningIds`) lands in `recent`.
 * - Everything else lands in `chats`.
 * - Within each group, the input order from `sessions` is preserved.
 */
export function partitionCodingThreads(
  sessions: ChatThread[],
  runningIds: ReadonlySet<string>,
  recent: ReadonlyMap<string, number>,
): CodingThreadGroups {
  const running: ChatThread[] = [];
  const recentArr: ChatThread[] = [];
  const chats: ChatThread[] = [];
  for (const t of sessions) {
    if (runningIds.has(t.sessionKey)) {
      running.push(t);
    } else if (recent.has(t.sessionKey)) {
      recentArr.push(t);
    } else {
      chats.push(t);
    }
  }
  return { running, recent: recentArr, chats };
}
