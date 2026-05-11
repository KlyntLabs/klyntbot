import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type { StreamSnapshot } from "@/features/chat/types";
import { DEFAULT_STREAM_SNAPSHOT } from "@/features/chat/types";
import type {
  CodingThreadState,
  ThreadEvent,
} from "@/features/coding/state/codingEventReducer";
import { applyThreadEvent, initialCodingState } from "@/features/coding/state/codingEventReducer";
import type { ConversationItem } from "@/types";
import { initialState as threadInitialState, threadReducer } from "../hooks/useThreadsReducer";
import type { ThreadAction, ThreadState } from "../hooks/useThreadsReducer";
import { CoalescerRegistry } from "../utils/coalesceDeltas";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;
type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

// ── Thread Slice ──────────────────────────────────────────────────────

interface ThreadsSlice extends ThreadState {
  dispatchThreadAction: (action: ThreadAction) => void;
}

const createThreadsSlice = (set: any): ThreadsSlice => ({
  ...threadInitialState,
  dispatchThreadAction: (action: ThreadAction) =>
    set((state: ChatStore) => threadReducer(state, action)),
});

// ── Stream Slice ──────────────────────────────────────────────────────

interface StreamSlice {
  streamSnapshots: Record<string, StreamSnapshot>;
  streamApprovals: Record<string, ApprovalItem[]>;
  streamFileEdits: Record<string, DiffItem[]>;

  _setStreamSnapshot: (sessionKey: string, snapshot: StreamSnapshot) => void;
  _setStreamApprovals: (sessionKey: string, approvals: ApprovalItem[]) => void;
  _setStreamFileEdits: (sessionKey: string, edits: DiffItem[]) => void;
}

const streamCoalescers = new CoalescerRegistry<StreamSnapshot>();

const createStreamSlice = (set: any): StreamSlice => ({
  streamSnapshots: {},
  streamApprovals: {},
  streamFileEdits: {},

  _setStreamSnapshot: (sessionKey: string, snapshot: StreamSnapshot) => {
    const coalescer = streamCoalescers.get(sessionKey, {
      flush: (snapshots) => {
        const latest = snapshots[snapshots.length - 1];
        set((state: ChatStore) => ({
          streamSnapshots: { ...state.streamSnapshots, [sessionKey]: latest },
        }));
      },
      maxWaitMs: 50,
    });
    coalescer.push(snapshot);
  },

  _setStreamApprovals: (sessionKey: string, approvals: ApprovalItem[]) =>
    set((state: ChatStore) => ({
      streamApprovals: { ...state.streamApprovals, [sessionKey]: approvals },
    })),

  _setStreamFileEdits: (sessionKey: string, edits: DiffItem[]) =>
    set((state: ChatStore) => ({
      streamFileEdits: { ...state.streamFileEdits, [sessionKey]: edits },
    })),
});

// ── Coding Slice ──────────────────────────────────────────────────────

interface CodingSlice {
  codingStateByThread: Record<string, CodingThreadState>;
  codingRunningIds: Set<string>;
  codingRecentlyCompleted: Map<string, number>;

  applyCodingThreadEvent: (threadId: string, event: ThreadEvent) => void;
  setCodingRunningIds: (ids: Set<string>) => void;
  setCodingRecentlyCompleted: (map: Map<string, number>) => void;
  resetCodingThreadState: (threadId: string) => void;
}

const codingCoalescers = new CoalescerRegistry<ThreadEvent>();

const createCodingSlice = (set: any): CodingSlice => ({
  codingStateByThread: {},
  codingRunningIds: new Set<string>(),
  codingRecentlyCompleted: new Map<string, number>(),

  applyCodingThreadEvent: (threadId: string, event: ThreadEvent) => {
    const coalescer = codingCoalescers.get(threadId, {
      flush: (events) => {
        set((state: ChatStore) => {
          let prev = state.codingStateByThread[threadId] ?? initialCodingState;
          let changed = false;
          for (const evt of events) {
            const next = applyThreadEvent(prev, evt);
            if (next !== prev) {
              prev = next;
              changed = true;
            }
          }
          if (!changed) return {};
          return {
            codingStateByThread: { ...state.codingStateByThread, [threadId]: prev },
          };
        });
      },
      maxWaitMs: 50,
    });
    coalescer.push(event);
  },

  setCodingRunningIds: (ids: Set<string>) =>
    set(() => ({ codingRunningIds: new Set(ids) })),

  setCodingRecentlyCompleted: (map: Map<string, number>) =>
    set(() => ({ codingRecentlyCompleted: new Map(map) })),

  resetCodingThreadState: (threadId: string) => {
    codingCoalescers.dispose(threadId);
    set((state: ChatStore) => {
      const { [threadId]: _, ...rest } = state.codingStateByThread;
      return { codingStateByThread: rest };
    });
  },

});

// ── Store ─────────────────────────────────────────────────────────────

export type ChatStore = ThreadsSlice & StreamSlice & CodingSlice;

export const useChatStore = create<ChatStore>()(
  devtools(
    (set, _get) => ({
      ...createThreadsSlice(set),
      ...createStreamSlice(set),
      ...createCodingSlice(set),
    }),
    {
      name: "ChatStore",
      enabled: typeof import.meta.env !== "undefined" && import.meta.env.DEV,
    },
  ),
);

// Helper selectors (stable references)
export function selectStreamSnapshot(store: ChatStore, sessionKey: string): StreamSnapshot {
  return store.streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
}

export function selectThreadState(store: ChatStore): ThreadState {
  return {
    activeThreadIdByWorkspace: store.activeThreadIdByWorkspace,
    itemsByThread: store.itemsByThread,
    maxItemsPerThread: store.maxItemsPerThread,
    threadsByWorkspace: store.threadsByWorkspace,
    hiddenThreadIdsByWorkspace: store.hiddenThreadIdsByWorkspace,
    threadParentById: store.threadParentById,
    threadStatusById: store.threadStatusById,
    threadResumeLoadingById: store.threadResumeLoadingById,
    threadListLoadingByWorkspace: store.threadListLoadingByWorkspace,
    threadListPagingByWorkspace: store.threadListPagingByWorkspace,
    threadListCursorByWorkspace: store.threadListCursorByWorkspace,
    threadSortKeyByWorkspace: store.threadSortKeyByWorkspace,
    activeTurnIdByThread: store.activeTurnIdByThread,
    turnGenerationByThread: store.turnGenerationByThread,
    turnDiffByThread: store.turnDiffByThread,
    approvals: store.approvals,
    userInputRequests: store.userInputRequests,
    tokenUsageByThread: store.tokenUsageByThread,
    rateLimitsByWorkspace: store.rateLimitsByWorkspace,
    accountByWorkspace: store.accountByWorkspace,
    planByThread: store.planByThread,
    lastAgentMessageByThread: store.lastAgentMessageByThread,
  };
}
