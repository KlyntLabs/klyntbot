import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type { StreamSnapshot } from "@/features/chat/types";
import { DEFAULT_STREAM_SNAPSHOT } from "@/features/chat/types";
import type { ConversationItem } from "@/types";
import type { ThreadAction, ThreadState } from "../hooks/useThreadsReducer";
import { initialState as threadInitialState, threadReducer } from "../hooks/useThreadsReducer";
import { CoalescerRegistry } from "../utils/coalesceDeltas";

// Preserve legacy v1 chat event bridge side-effects until full v2 migration.
// chatStreamStore registers Tauri listeners that populate streamSnapshots.
import "@/features/chat/store/chatStreamStore";

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

  // Convenience helpers (migrated from chatStreamStore)
  appendSystemItem: (sessionKey: string, kind: string, item: unknown) => void;
  appendErrorItem: (sessionKey: string, message: string) => void;
  upsertApproval: (sessionKey: string, item: ApprovalItem) => void;
  resolveApproval: (
    sessionKey: string,
    requestId: string,
    status: ApprovalItem["status"],
    decidedBy: ApprovalItem["decidedBy"],
  ) => void;
  upsertFileEdit: (sessionKey: string, item: DiffItem) => void;
  clearSegments: (sessionKey: string) => void;
}

const streamCoalescers = new CoalescerRegistry<StreamSnapshot>();

const createStreamSlice = (set: any, get: () => ChatStore): StreamSlice => ({
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

  appendSystemItem: (sessionKey: string, kind: string, item: unknown) => {
    const snap = get().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    get()._setStreamSnapshot(sessionKey, {
      ...snap,
      segments: [...snap.segments, { type: "system" as const, kind, item }],
    });
  },

  appendErrorItem: (sessionKey: string, message: string) => {
    const snap = get().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    get()._setStreamSnapshot(sessionKey, {
      ...snap,
      segments: [...snap.segments, { type: "error" as const, message }],
    });
  },

  upsertApproval: (sessionKey: string, item: ApprovalItem) => {
    const existing = get().streamApprovals[sessionKey] ?? [];
    const index = existing.findIndex((a) => a.requestId === item.requestId);
    const next =
      index >= 0
        ? [...existing.slice(0, index), item, ...existing.slice(index + 1)]
        : [...existing, item];
    get()._setStreamApprovals(sessionKey, next);
  },

  resolveApproval: (
    sessionKey: string,
    requestId: string,
    status: ApprovalItem["status"],
    decidedBy: ApprovalItem["decidedBy"],
  ) => {
    const existing = get().streamApprovals[sessionKey] ?? [];
    const index = existing.findIndex((a) => a.requestId === requestId);
    if (index < 0) return;
    const next = [...existing];
    next[index] = {
      ...next[index],
      status,
      decidedBy,
      decidedAt: new Date().toISOString(),
    };
    get()._setStreamApprovals(sessionKey, next);
  },

  upsertFileEdit: (sessionKey: string, item: DiffItem) => {
    const existing = get().streamFileEdits[sessionKey] ?? [];
    get()._setStreamFileEdits(sessionKey, [...existing, item]);
  },

  clearSegments: (sessionKey: string) => {
    const snap = get().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    get()._setStreamSnapshot(sessionKey, { ...snap, segments: [] });
  },
});

// ── Store ─────────────────────────────────────────────────────────────

export type ChatStore = ThreadsSlice & StreamSlice;

export const useChatStore = create<ChatStore>()(
  devtools(
    (set, get) => ({
      ...createThreadsSlice(set),
      ...createStreamSlice(set, get),
    }),
    {
      name: "ChatStore",
      enabled: import.meta.env?.DEV,
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
    planByThread: store.planByThread,
    lastAgentMessageByThread: store.lastAgentMessageByThread,
  };
}
