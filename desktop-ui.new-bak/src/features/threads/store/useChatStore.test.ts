import { beforeEach, describe, expect, it } from "vitest";
import { useChatStore } from "./useChatStore";

describe("useChatStore", () => {
  beforeEach(() => {
    // Reset store to initial state between tests
    useChatStore.setState({
      activeThreadIdByWorkspace: {},
      itemsByThread: {},
      maxItemsPerThread: 50,
      threadsByWorkspace: {},
      hiddenThreadIdsByWorkspace: {},
      threadParentById: {},
      threadStatusById: {},
      threadResumeLoadingById: {},
      threadListLoadingByWorkspace: {},
      threadListPagingByWorkspace: {},
      threadListCursorByWorkspace: {},
      threadSortKeyByWorkspace: {},
      activeTurnIdByThread: {},
      turnGenerationByThread: {},
      turnDiffByThread: {},
      approvals: [],
      userInputRequests: [],
      tokenUsageByThread: {},
      planByThread: {},
      lastAgentMessageByThread: {},
      streamSnapshots: {},
      streamApprovals: {},
      streamFileEdits: {},
    });
  });

  describe("threads slice", () => {
    it("dispatches thread actions through the reducer", () => {
      useChatStore.getState().dispatchThreadAction({
        type: "ensureThread",
        workspaceId: "ws-1",
        threadId: "t1",
      });
      const state = useChatStore.getState();
      expect(state.threadsByWorkspace["ws-1"]).toHaveLength(1);
      expect(state.threadsByWorkspace["ws-1"]?.[0]?.id).toBe("t1");
    });

    it("increments turn generation per thread", () => {
      useChatStore.getState().dispatchThreadAction({
        type: "incrementTurnGeneration",
        threadId: "t1",
      });
      useChatStore.getState().dispatchThreadAction({
        type: "incrementTurnGeneration",
        threadId: "t1",
      });
      expect(useChatStore.getState().turnGenerationByThread.t1).toBe(2);
    });

    it("keeps turn generation independent per thread", () => {
      useChatStore.getState().dispatchThreadAction({
        type: "incrementTurnGeneration",
        threadId: "t1",
      });
      expect(useChatStore.getState().turnGenerationByThread.t1).toBe(1);
      expect(useChatStore.getState().turnGenerationByThread.t2).toBeUndefined();
    });
  });

  describe("stream slice", () => {
    it("sets and retrieves stream snapshots", () => {
      useChatStore.getState()._setStreamSnapshot("sess-1", {
        segments: [{ type: "text", content: "hello" }],
        isStreaming: true,
        activeTools: [],
        error: null,
        activeInteraction: null,
        transparency: null,
        activeDelegateAgent: null,
        personaMessages: [],
        debateRounds: [],
        currentDebateRound: null,
        totalDebateRounds: null,
        squadMode: null,
        consensusReached: false,
        consensusSummary: null,
        judgeDecisions: [],
        statusPhase: "Thinking",
        needsRefetch: false,
        cancelled: false,
        partialContent: "",
        partialReasoning: "",
      });
      const snap = useChatStore.getState().streamSnapshots["sess-1"];
      expect(snap?.isStreaming).toBe(true);
      expect(snap?.segments).toHaveLength(1);
    });

    it("stores approvals per session", () => {
      const approval = {
        id: "a1",
        kind: "approval" as const,
        requestId: "r1",
        tool: "test",
        args: {},
        cwd: "/",
        sandboxSummary: "",
        layer: "default_mode" as const,
        layerReason: "",
        status: "pending" as const,
      };
      useChatStore.getState()._setStreamApprovals("sess-1", [approval]);
      expect(useChatStore.getState().streamApprovals["sess-1"]).toHaveLength(1);
    });
  });

  describe("generation counter invariants", () => {
    it("generation starts at 0 for unknown threads", () => {
      expect(useChatStore.getState().turnGenerationByThread.unknown).toBeUndefined();
    });

    it("generation is monotonically increasing", () => {
      const store = useChatStore.getState();
      store.dispatchThreadAction({ type: "incrementTurnGeneration", threadId: "t1" });
      const g1 = useChatStore.getState().turnGenerationByThread.t1;
      store.dispatchThreadAction({ type: "incrementTurnGeneration", threadId: "t1" });
      const g2 = useChatStore.getState().turnGenerationByThread.t1;
      expect(g2).toBeGreaterThan(g1);
      expect(g2).toBe(2);
    });

    it("generation does not decrease on redundant actions", () => {
      const store = useChatStore.getState();
      store.dispatchThreadAction({ type: "incrementTurnGeneration", threadId: "t1" });
      store.dispatchThreadAction({ type: "incrementTurnGeneration", threadId: "t1" });
      store.dispatchThreadAction({ type: "incrementTurnGeneration", threadId: "t1" });
      expect(useChatStore.getState().turnGenerationByThread.t1).toBe(3);
    });
  });
});
