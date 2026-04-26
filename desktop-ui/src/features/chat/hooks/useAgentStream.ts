import { chatStreamStore } from "../store/chatStreamStore";
import type {
  ActiveInteraction,
  DebateRound,
  JudgeDecisionEntry,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "../types";
import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";

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
 * State and EventSource connections are managed by a global store so they
 * survive React component unmount/remount (e.g. during route navigation).
 *
 * @param sessionKey  Session to filter events for (empty = inert).
 * @param onDone      Called when the agent finishes — use for refetching messages.
 */
export function useAgentStream(sessionKey: string, onDone?: () => void): AgentStream {
  // Subscribe to the global store for this session's state
  const getSnapshot = useCallback(() => chatStreamStore.getSnapshot(sessionKey), [sessionKey]);

  const state = useSyncExternalStore(chatStreamStore.subscribe, getSnapshot);

  // Register onDone callback with the store
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    if (!sessionKey) return;
    return chatStreamStore.registerOnDone(sessionKey, () => onDoneRef.current?.());
  }, [sessionKey]);

  // Handle deferred refetch: if a stream finished while this component was
  // unmounted, the store sets needsRefetch=true. On mount, consume it and
  // fire onDone to trigger message refetch.
  useEffect(() => {
    if (state.needsRefetch && sessionKey) {
      chatStreamStore.consumeRefetch(sessionKey);
      onDoneRef.current?.();
    }
  }, [state.needsRefetch, sessionKey]);

  const startStreaming = useCallback(() => {
    chatStreamStore.startStream(sessionKey);
  }, [sessionKey]);

  const failStreaming = useCallback(
    (message: string) => {
      chatStreamStore.failStream(sessionKey, message);
    },
    [sessionKey],
  );

  const clearInteraction = useCallback(
    () => chatStreamStore.clearInteraction(sessionKey),
    [sessionKey],
  );

  const clearSegments = useCallback(() => chatStreamStore.clearSegments(sessionKey), [sessionKey]);

  const clearTransparency = useCallback(
    () => chatStreamStore.clearTransparency(sessionKey),
    [sessionKey],
  );

  const clearPersonaMessages = useCallback(
    () => chatStreamStore.clearPersonaMessages(sessionKey),
    [sessionKey],
  );

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
