import { useCallback, useEffect, useRef } from "react";
import { DEFAULT_STREAM_SNAPSHOT } from "@/features/chat/types";
import { useChatStore } from "@/features/threads/store/useChatStore";
import type {
  ActiveInteraction,
  DebateRound,
  JudgeDecisionEntry,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "../types";

export type { PersonaSegment };

interface AgentStream {
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  transparency: TransparencyData | null;
  personaMessages: PersonaSegment[];
  debateRounds: DebateRound[];
  totalDebateRounds: number | null;
  squadMode: "quick" | "debate" | null;
  judgeDecisions: JudgeDecisionEntry[];
  consensusReached: boolean;
  consensusSummary: string | null;
  /** Call before sending a message to enter streaming mode. */
  startStreaming: () => void;
  /** Abort streaming and show an error. */
  failStreaming: (message: string) => void;
  /** Clear the active interaction (after submit/cancel). */
  clearInteraction: () => void;
  /** Clear accumulated segments (after persisted messages arrive). */
  clearSegments: () => void;
  /** Clear accumulated transparency data. */
  clearTransparency: () => void;
  /** Clear accumulated persona messages. */
  clearPersonaMessages: () => void;
  /** The agent currently being delegated to (between delegation_started and delegation_completed). */
  activeDelegateAgent: string | null;
  /** Dynamic status phase (e.g. "Thinking", "Using tasks:search"). */
  statusPhase: string | null;
}

/**
 * Listens to agent streaming events for a specific session.
 *
 * State is managed by `useChatStore` so streams survive React component
 * unmount/remount (e.g. during route navigation).
 *
 * @param sessionKey  Session to filter events for (empty = inert).
 * @param onDone      Called when the agent finishes — use for refetching messages.
 */
export function useAgentStream(sessionKey: string, onDone?: () => void): AgentStream {
  const state = useChatStore(
    (store) => store.streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT,
  );

  // Register onDone callback with the store
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    if (!sessionKey) return;
    return useChatStore.subscribe((store, prevStore) => {
      const snap = store.streamSnapshots[sessionKey];
      const prevSnap = prevStore.streamSnapshots[sessionKey];
      if (!snap || snap === prevSnap) return;
      // Fire onDone when a stream transitions from active to inactive
      if (prevSnap?.isStreaming && !snap.isStreaming) {
        onDoneRef.current?.();
      }
    });
  }, [sessionKey]);

  const startStreaming = useCallback(() => {
    useChatStore.getState()._setStreamSnapshot(sessionKey, {
      ...DEFAULT_STREAM_SNAPSHOT,
      isStreaming: true,
      statusPhase: "Thinking",
      needsRefetch: false,
    });
  }, [sessionKey]);

  const failStreaming = useCallback(
    (message: string) => {
      useChatStore.getState()._setStreamSnapshot(sessionKey, {
        ...DEFAULT_STREAM_SNAPSHOT,
        error: message,
      });
    },
    [sessionKey],
  );

  const clearInteraction = useCallback(() => {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, activeInteraction: null });
  }, [sessionKey]);

  const clearSegments = useCallback(() => {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, segments: [] });
  }, [sessionKey]);

  const clearTransparency = useCallback(() => {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, transparency: null });
  }, [sessionKey]);

  const clearPersonaMessages = useCallback(() => {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, personaMessages: [] });
  }, [sessionKey]);

  return {
    segments: state.segments,
    isStreaming: state.isStreaming,
    activeTools: state.activeTools,
    error: state.error,
    activeInteraction: state.activeInteraction,
    transparency: state.transparency,
    activeDelegateAgent: state.activeDelegateAgent,
    personaMessages: state.personaMessages,
    debateRounds: state.debateRounds,
    totalDebateRounds: state.totalDebateRounds,
    squadMode: state.squadMode,
    judgeDecisions: state.judgeDecisions,
    consensusReached: state.consensusReached,
    consensusSummary: state.consensusSummary,
    statusPhase: state.statusPhase,
    startStreaming,
    failStreaming,
    clearInteraction,
    clearSegments,
    clearTransparency,
    clearPersonaMessages,
  };
}
