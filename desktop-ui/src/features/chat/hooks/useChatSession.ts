import { useQuery } from "@shared/hooks/useQuery";
import { parseApiError } from "@shared/lib/utils";
import type {
  ActiveInteraction,
  ChatMessage,
  DebateRound,
  JudgeDecisionEntry,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAgentStream } from "./useAgentStream";
import { ipc } from "./useIpc";

interface ChatSession {
  messages: ChatMessage[];
  segments: MessageSegment[];
  transparency: TransparencyData | null;
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  activeDelegateAgent: string | null;
  statusPhase: string | null;
  personaMessages: PersonaSegment[];
  debateRounds: DebateRound[];
  totalDebateRounds: number | null;
  squadMode: "quick" | "debate" | null;
  judgeDecisions: JudgeDecisionEntry[];
  consensusReached: boolean;
  consensusSummary: string | null;
  input: string;
  setInput: (value: string) => void;
  send: (extraPayload?: Record<string, unknown>) => Promise<void>;
  clearInteraction: () => void;
}

interface UseChatSessionOptions {
  squadId?: string;
  squadMode?: "quick" | "debate";
}

/**
 * Encapsulates chat session state: message fetching, optimistic pending message,
 * streaming, and send logic. Used by SidebarChat and Chat.
 *
 * @param onDone Optional callback fired when the agent finishes — use for
 *               refreshing thread lists or other side-effects.
 * @param options Optional configuration including squadId for squad chat mode.
 */
export function useChatSession(
  sessionKey: string,
  onDone?: () => void,
  options?: UseChatSessionOptions,
): ChatSession {
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    "chat_messages",
    sessionKey ? { sessionKey } : null,
    [],
  );
  const [input, setInput] = useState("");
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  const stream = useAgentStream(sessionKey, () => {
    setPendingUserMsg(null);
    refetch();
    onDone?.();
  });

  // Clear streaming segments only when a NEW assistant message arrives from refetch.
  // Tracks assistant count to avoid clearing before the refetch completes — the old
  // approach fired on isStreaming→false before the new message existed, causing a flash.
  const { isStreaming, clearSegments, clearTransparency, clearPersonaMessages, segments } = stream;
  const hasSegmentsRef = useRef(false);
  hasSegmentsRef.current = segments.length > 0;
  const assistantCountRef = useRef(0);

  useEffect(() => {
    const count = messages.filter((m) => m.role === "assistant").length;
    if (!isStreaming && hasSegmentsRef.current && count > assistantCountRef.current) {
      clearSegments();
      clearTransparency();
      clearPersonaMessages();
    }
    assistantCountRef.current = count;
  }, [messages, isStreaming, clearSegments, clearTransparency, clearPersonaMessages]);

  const displayMessages = useMemo(() => {
    if (!pendingUserMsg) return messages;
    return [...messages, { id: "pending", role: "user" as const, content: pendingUserMsg }];
  }, [messages, pendingUserMsg]);

  const squadId = options?.squadId;
  const send = useCallback(
    async (extraPayload?: Record<string, unknown>) => {
      if (!input.trim() || stream.isStreaming) return;
      const text = input;
      setInput("");

      setPendingUserMsg(text);
      stream.startStreaming();

      try {
        const payload: Record<string, unknown> = {
          content: text,
          sessionKey,
          ...extraPayload,
        };
        if (squadId) {
          payload.context = {
            ...(payload.context as Record<string, unknown> | undefined),
            squadId,
            squadMode: options?.squadMode,
          };
        }
        await ipc<ChatMessage>("chat_send", payload);
      } catch (e: unknown) {
        stream.failStreaming(parseApiError(e).message);
      }
    },
    [input, sessionKey, stream, squadId, options?.squadMode],
  );

  return {
    messages: displayMessages,
    segments: stream.segments,
    transparency: stream.transparency,
    isStreaming: stream.isStreaming,
    activeTools: stream.activeTools,
    error: stream.error,
    activeInteraction: stream.activeInteraction,
    activeDelegateAgent: stream.activeDelegateAgent,
    statusPhase: stream.statusPhase,
    personaMessages: stream.personaMessages,
    debateRounds: stream.debateRounds,
    totalDebateRounds: stream.totalDebateRounds,
    squadMode: stream.squadMode,
    judgeDecisions: stream.judgeDecisions,
    consensusReached: stream.consensusReached,
    consensusSummary: stream.consensusSummary,
    input,
    setInput,
    send,
    clearInteraction: stream.clearInteraction,
  };
}
