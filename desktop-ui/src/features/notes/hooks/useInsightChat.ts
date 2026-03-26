import { useChatSession } from "@features/chat/hooks/useChatSession";
import type { ChatMessage, DebateRound, JudgeDecisionEntry, PersonaSegment } from "@shared/types";
import { useCallback, useRef } from "react";

export interface InsightChat {
  messages: ChatMessage[];
  isStreaming: boolean;
  error: string | null;
  input: string;
  setInput: (value: string) => void;
  send: () => Promise<void>;
  hasMessages: boolean;
  // Squad/debate features (from useChatSession)
  personaMessages: PersonaSegment[];
  debateRounds: DebateRound[];
  totalDebateRounds: number | null;
  judgeDecisions: JudgeDecisionEntry[];
  consensusReached: boolean;
  consensusSummary: string | null;
  squadMode: "quick" | "debate" | null;
  statusPhase: string | null;
  // Keep streamingContent for backwards compat with non-squad rendering
  streamingContent: string;
}

/**
 * Chat session for an insight tab, backed by the full chat system.
 *
 * Uses `useChatSession` (same as /chat page) so squad mode gets the full
 * debate engine — personas discuss with each other, a judge evaluates rounds,
 * and consensus is reached.
 *
 * On the first message, prepends the tab analysis content so the AI (or squad)
 * has full context about what they're discussing.
 */
export function useInsightChat(
  noteId: string | null,
  tabName: string,
  enabled: boolean,
  squadId?: string | null,
  tabContent?: string,
): InsightChat {
  const sessionKey = noteId && enabled ? `insight-chat:${noteId}:${tabName}` : "";
  const hasSentFirstRef = useRef(false);

  // Track whether this session has existing messages (context already injected)
  const session = useChatSession(sessionKey, undefined, {
    squadId: squadId || undefined,
    squadMode: squadId ? "debate" : undefined,
  });

  // Reset first-send tracker when session key changes
  const lastKeyRef = useRef(sessionKey);
  if (lastKeyRef.current !== sessionKey) {
    hasSentFirstRef.current = session.messages.length > 0;
    lastKeyRef.current = sessionKey;
  }

  // If session already has messages, context was already sent
  if (!hasSentFirstRef.current && session.messages.length > 0) {
    hasSentFirstRef.current = true;
  }

  const originalSend = session.send;
  const send = useCallback(async () => {
    if (!session.input.trim()) return;

    // On first message, prepend the tab content as context
    if (!hasSentFirstRef.current && tabContent) {
      hasSentFirstRef.current = true;
      const contextPrefix = `[Discussing the ${tabName} analysis of this note]\n\n---\n${tabContent}\n---\n\n`;
      await originalSend({
        content: contextPrefix + session.input.trim(),
      });
    } else {
      await originalSend();
    }
  }, [session.input, originalSend, tabContent, tabName]);

  // Compute streamingContent from segments for backwards compat
  const streamingContent = session.segments
    .filter((s): s is Extract<typeof s, { type: "text" }> => s.type === "text")
    .map((s) => s.content)
    .join("");

  return {
    messages: session.messages,
    isStreaming: session.isStreaming,
    error: session.error,
    input: session.input,
    setInput: session.setInput,
    send,
    hasMessages: session.messages.length > 0,
    personaMessages: session.personaMessages,
    debateRounds: session.debateRounds,
    totalDebateRounds: session.totalDebateRounds,
    judgeDecisions: session.judgeDecisions,
    consensusReached: session.consensusReached,
    consensusSummary: session.consensusSummary,
    squadMode: session.squadMode,
    statusPhase: session.statusPhase,
    streamingContent,
  };
}
