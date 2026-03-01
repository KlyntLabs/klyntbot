import { createContext, useContext, useCallback, useState, useEffect, useRef } from 'react';
import { useNavigate, useParams } from 'react-router';
import { useAgent } from '../../lib/hooks/useAgent';
import type { ThinkingState, PendingInteraction } from '../../lib/hooks/useAgent';
import type { ChatMessage, ToolActivityEntry } from '../../lib/types';
import { TOOL_CATEGORY_MAP } from '../../lib/types';
import type { ConnectionStatus } from '../../lib/ws';

interface ChatContextValue {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  sessionKey: string | null;
  pendingInteraction: PendingInteraction | null;
  sendMessage: (text: string) => void;
  cancel: () => void;
  respondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
  startNewSession: () => void;
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

const ChatContext = createContext<ChatContextValue | null>(null);

export function useChatContext(): ChatContextValue {
  const ctx = useContext(ChatContext);
  if (!ctx) throw new Error('useChatContext must be used within ChatProvider');
  return ctx;
}

export function ChatProvider({ children }: { children: React.ReactNode }) {
  const navigate = useNavigate();
  const { sessionId } = useParams<{ sessionId?: string }>();
  const agent = useAgent();
  const [activeTools, setActiveTools] = useState<Set<string>>(new Set());
  const [toolHistory, setToolHistory] = useState<ToolActivityEntry[]>([]);
  const hasLoadedSession = useRef(false);

  // Load session from URL param on mount or when sessionId changes
  useEffect(() => {
    if (sessionId && sessionId !== agent.sessionKey) {
      agent.loadSession(sessionId);
      hasLoadedSession.current = true;
    } else if (!sessionId && agent.sessionKey && hasLoadedSession.current) {
      // Navigated to / — clear session
      agent.newSession();
      setActiveTools(new Set());
      setToolHistory([]);
    }
  }, [sessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Sync URL when session key changes (after first message)
  useEffect(() => {
    if (agent.sessionKey && !sessionId) {
      navigate(`/chat/${agent.sessionKey}`, { replace: true });
    }
  }, [agent.sessionKey, sessionId, navigate]);

  // Track tool activity from thinking state
  const prevToolCallsRef = useRef<number>(0);
  useEffect(() => {
    if (!agent.thinking) {
      // Streaming ended — mark all active tools as completed
      if (activeTools.size > 0) {
        setActiveTools(new Set());
      }
      prevToolCallsRef.current = 0;
      return;
    }

    const toolCalls = agent.thinking.toolCalls;
    // Process new tool calls since last check
    for (let i = prevToolCallsRef.current; i < toolCalls.length; i++) {
      const tc = toolCalls[i];
      const category = TOOL_CATEGORY_MAP[tc.name];

      if (category && !tc.completed) {
        // Tool started
        setActiveTools((prev) => new Set(prev).add(category));
        setToolHistory((prev) => [
          ...prev,
          {
            category,
            toolName: tc.name,
            args: tc.args,
            timestamp: Date.now(),
            status: 'active',
          },
        ]);
      }
    }

    // Check for newly completed tools
    for (const tc of toolCalls) {
      if (tc.completed) {
        const category = TOOL_CATEGORY_MAP[tc.name];
        if (!category) continue;

        setActiveTools((prev) => {
          // Only remove if no other active tool in same category
          const stillActive = toolCalls.some(
            (other) =>
              !other.completed &&
              TOOL_CATEGORY_MAP[other.name] === category,
          );
          if (stillActive) return prev;
          const next = new Set(prev);
          next.delete(category);
          return next;
        });
        setToolHistory((prev) =>
          prev.map((entry) =>
            entry.toolName === tc.name && entry.status === 'active'
              ? { ...entry, status: tc.success ? 'completed' : 'failed' }
              : entry,
          ),
        );
      }
    }

    prevToolCallsRef.current = toolCalls.length;
  }, [agent.thinking]); // eslint-disable-line react-hooks/exhaustive-deps

  const sendMessage = useCallback(
    (text: string) => {
      agent.sendMessage(text);
    },
    [agent],
  );

  const startNewSession = useCallback(() => {
    agent.newSession();
    setActiveTools(new Set());
    setToolHistory([]);
    navigate('/');
  }, [agent, navigate]);

  const value: ChatContextValue = {
    messages: agent.messages,
    thinking: agent.thinking,
    isStreaming: agent.isStreaming,
    status: agent.status,
    sessionKey: agent.sessionKey,
    pendingInteraction: agent.pendingInteraction,
    sendMessage,
    cancel: agent.cancel,
    respondToInteraction: agent.respondToInteraction,
    startNewSession,
    activeTools,
    toolHistory,
  };

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}
