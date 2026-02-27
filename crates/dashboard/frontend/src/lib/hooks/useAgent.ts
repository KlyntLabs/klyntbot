/**
 * WebSocket streaming hook for agent chat.
 *
 * Exposes: messages, thinking, isStreaming, status, sendMessage, cancel
 *
 * Handles the full streaming lifecycle:
 *   classificationComplete → contextAssembled → executionStarted →
 *   iterationStart → toolStart/toolEnd → contentChunk → done
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { AgentSocket, type ConnectionStatus } from '../ws';
import type { ChatMessage } from '../types';
import { apiFetch } from '../api';
import type { SessionWithMessages, InteractionQuestion } from '../types';

export interface ToolCallState {
  name: string;
  args?: Record<string, unknown>;
  durationMs?: number;
  success?: boolean;
  completed: boolean;
}

export interface ThinkingState {
  phase:
    | 'classifying'
    | 'buildingContext'
    | 'thinking'
    | 'idle';
  strategy?: string;
  confidence?: number;
  engine?: string;
  iteration?: number;
  maxIterations?: number;
  toolCalls: ToolCallState[];
}

export interface PendingInteraction {
  requestId: string;
  title: string;
  questions: InteractionQuestion[];
}

export interface UseAgentResult {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  sessionKey: string | null;
  pendingInteraction: PendingInteraction | null;
  sendMessage: (text: string, sessionKey?: string) => void;
  cancel: () => void;
  loadSession: (key: string) => Promise<void>;
  newSession: () => void;
  deleteSession: (key: string) => Promise<void>;
  respondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function useAgent(): UseAgentResult {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [thinking, setThinking] = useState<ThinkingState | null>(null);
  const [isStreaming, setIsStreaming] = useState(false);
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [sessionKey, setSessionKey] = useState<string | null>(null);
  const [pendingInteraction, setPendingInteraction] = useState<PendingInteraction | null>(null);

  const socketRef = useRef<AgentSocket | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  const accumulatedContentRef = useRef('');
  // Ref to track isStreaming without stale closure issues
  const isStreamingRef = useRef(false);

  useEffect(() => {
    const socket = new AgentSocket();
    socketRef.current = socket;

    const removeStatusHandler = socket.onStatusChange((newStatus) => {
      setStatus(newStatus);

      // If disconnected while streaming, add a system message
      if (
        newStatus === 'reconnecting' &&
        isStreamingRef.current
      ) {
        isStreamingRef.current = false;
        setIsStreaming(false);
        setThinking(null);
        setMessages((prev) => [
          ...prev,
          {
            id: generateId(),
            role: 'system',
            content: 'Connection lost — message may be incomplete',
            timestamp: new Date(),
          },
        ]);
      }
    });

    const removeMessageHandler = socket.onMessage((event) => {
      switch (event.type) {
        case 'classificationComplete': {
          setThinking({
            phase: 'classifying',
            strategy: event.strategy as string | undefined,
            confidence: event.confidence as number | undefined,
            toolCalls: [],
          });
          break;
        }

        case 'contextAssembled': {
          setThinking((prev) => ({
            ...(prev ?? { toolCalls: [] }),
            phase: 'buildingContext',
          }));
          break;
        }

        case 'executionStarted': {
          setThinking((prev) => ({
            ...(prev ?? { toolCalls: [] }),
            phase: 'thinking',
            engine: event.engine as string | undefined,
            maxIterations: event.maxIterations as number | undefined,
          }));
          break;
        }

        case 'iterationStart': {
          setThinking((prev) =>
            prev
              ? {
                  ...prev,
                  iteration: event.iteration as number,
                  maxIterations: event.max as number,
                }
              : null,
          );
          break;
        }

        case 'toolStart': {
          setThinking((prev) => {
            if (!prev) return prev;
            return {
              ...prev,
              toolCalls: [
                ...prev.toolCalls,
                {
                  name: event.name as string,
                  args: event.args as Record<string, unknown> | undefined,
                  completed: false,
                },
              ],
            };
          });
          break;
        }

        case 'toolEnd': {
          setThinking((prev) => {
            if (!prev) return prev;
            const toolCalls = [...prev.toolCalls];
            // Update the last tool call with this name
            for (let i = toolCalls.length - 1; i >= 0; i--) {
              if (toolCalls[i].name === (event.name as string) && !toolCalls[i].completed) {
                toolCalls[i] = {
                  ...toolCalls[i],
                  completed: true,
                  success: event.success as boolean,
                  durationMs: event.durationMs as number,
                };
                break;
              }
            }
            return { ...prev, toolCalls };
          });
          break;
        }

        case 'contentChunk': {
          const chunk = event.data as string;
          accumulatedContentRef.current += chunk;

          // Create or update the streaming message
          if (streamingMessageIdRef.current === null) {
            const id = generateId();
            streamingMessageIdRef.current = id;
            setMessages((prev) => [
              ...prev,
              {
                id,
                role: 'assistant',
                content: accumulatedContentRef.current,
                timestamp: new Date(),
                isStreaming: true,
              },
            ]);
          } else {
            const id = streamingMessageIdRef.current;
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === id
                  ? { ...msg, content: accumulatedContentRef.current }
                  : msg,
              ),
            );
          }
          break;
        }

        case 'done': {
          const finalContent = (event.content as string) ?? accumulatedContentRef.current;
          const id = streamingMessageIdRef.current;

          if (id) {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === id
                  ? { ...msg, content: finalContent, isStreaming: false }
                  : msg,
              ),
            );
          } else {
            setMessages((prev) => [
              ...prev,
              {
                id: generateId(),
                role: 'assistant',
                content: finalContent,
                timestamp: new Date(),
                isStreaming: false,
              },
            ]);
          }

          // Reset streaming state
          streamingMessageIdRef.current = null;
          accumulatedContentRef.current = '';
          isStreamingRef.current = false;
          setIsStreaming(false);
          setThinking(null);
          break;
        }

        case 'error': {
          streamingMessageIdRef.current = null;
          accumulatedContentRef.current = '';
          isStreamingRef.current = false;
          setIsStreaming(false);
          setThinking(null);
          setMessages((prev) => [
            ...prev,
            {
              id: generateId(),
              role: 'system',
              content: (event.message as string) ?? 'An error occurred',
              timestamp: new Date(),
            },
          ]);
          break;
        }

        case 'interaction.request': {
          setPendingInteraction({
            requestId: event.requestId as string,
            title: event.title as string,
            questions: event.questions as InteractionQuestion[],
          });
          break;
        }

        default:
          break;
      }
    });

    return () => {
      removeStatusHandler();
      removeMessageHandler();
      socket.disconnect();
      socketRef.current = null;
    };
    // Run only once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const sendMessage = useCallback(
    (text: string, overrideSessionKey?: string) => {
      if (isStreamingRef.current) return;
      if (!socketRef.current) return;

      // Resolve or generate a session key — backend requires a non-null string
      let key = overrideSessionKey ?? sessionKey;
      if (!key) {
        key = `dashboard-${Date.now()}-${Math.random().toString(36).slice(2)}`;
        setSessionKey(key);
      }

      setMessages((prev) => [
        ...prev,
        {
          id: generateId(),
          role: 'user',
          content: text,
          timestamp: new Date(),
        },
      ]);

      isStreamingRef.current = true;
      setIsStreaming(true);
      accumulatedContentRef.current = '';
      streamingMessageIdRef.current = null;

      socketRef.current.sendChatMessage(key, text);
    },
    [sessionKey],
  );

  const cancel = useCallback(() => {
    socketRef.current?.sendCancel();

    // Remove incomplete streaming message
    if (streamingMessageIdRef.current) {
      const id = streamingMessageIdRef.current;
      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === id ? { ...msg, isStreaming: false } : msg,
        ),
      );
      streamingMessageIdRef.current = null;
    }

    accumulatedContentRef.current = '';
    isStreamingRef.current = false;
    setIsStreaming(false);
    setThinking(null);

    setMessages((prev) => [
      ...prev,
      {
        id: generateId(),
        role: 'system',
        content: 'Response cancelled',
        timestamp: new Date(),
      },
    ]);
  }, []);

  const loadSession = useCallback(async (key: string) => {
    try {
      const data = await apiFetch<SessionWithMessages>(`/api/sessions/${key}`);
      const loaded: ChatMessage[] = data.messages.map((m) => ({
        id: m.id,
        role: m.role as ChatMessage['role'],
        content: m.content,
        timestamp: new Date(m.timestamp),
      }));
      setMessages(loaded);
      setSessionKey(key);
      setThinking(null);
      setPendingInteraction(null);
      isStreamingRef.current = false;
      setIsStreaming(false);
      streamingMessageIdRef.current = null;
      accumulatedContentRef.current = '';
    } catch (err) {
      setMessages((prev) => [
        ...prev,
        {
          id: generateId(),
          role: 'system',
          content: `Failed to load session: ${err instanceof Error ? err.message : 'Unknown error'}`,
          timestamp: new Date(),
        },
      ]);
    }
  }, []);

  const newSession = useCallback(() => {
    setMessages([]);
    setSessionKey(null);
    setThinking(null);
    setPendingInteraction(null);
    isStreamingRef.current = false;
    setIsStreaming(false);
    streamingMessageIdRef.current = null;
    accumulatedContentRef.current = '';
  }, []);

  const deleteSession = useCallback(async (key: string) => {
    await apiFetch(`/api/sessions/${key}`, { method: 'DELETE' });
    if (key === sessionKey) {
      newSession();
    }
  }, [sessionKey, newSession]);

  const respondToInteraction = useCallback(
    (requestId: string, response: Record<string, unknown>) => {
      socketRef.current?.sendInteractionResponse(requestId, response);
      setPendingInteraction(null);
    },
    [],
  );

  return {
    messages, thinking, isStreaming, status, sessionKey, pendingInteraction,
    sendMessage, cancel, loadSession, newSession, deleteSession, respondToInteraction,
  };
}
