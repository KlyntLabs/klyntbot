import { useState, useCallback, useEffect, useRef } from 'react';
import { useEvent } from './useEvent';
import { isTauri } from '../lib/utils';
import type {
  ContentChunkPayload,
  ToolStartPayload,
  ToolEndPayload,
  AgentDonePayload,
  AgentErrorPayload,
  InteractionRequest,
  InteractionRequestPayload,
} from '../lib/types';

interface AgentStream {
  streamingContent: string;
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: { requestId: string; request: InteractionRequest } | null;
  /** Call before sending a message to enter streaming mode. */
  startStreaming: () => void;
  /** Abort streaming and show an error. */
  failStreaming: (message: string) => void;
  /** Clear the active interaction (after submit/cancel). */
  clearInteraction: () => void;
}

/**
 * Listens to Tauri agent streaming events for a specific session.
 *
 * @param sessionKey  Session to filter events for (empty = inert).
 * @param onDone      Called when the agent finishes — use for refetching messages.
 */
export function useAgentStream(sessionKey: string, onDone?: () => void): AgentStream {
  const [streamingContent, setStreamingContent] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [activeTools, setActiveTools] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [activeInteraction, setActiveInteraction] = useState<{
    requestId: string;
    request: InteractionRequest;
  } | null>(null);

  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;
  const sessionKeyRef = useRef(sessionKey);
  sessionKeyRef.current = sessionKey;

  // Reset when session changes
  useEffect(() => {
    setStreamingContent('');
    setIsStreaming(false);
    setActiveTools([]);
    setError(null);
    setActiveInteraction(null);
  }, [sessionKey]);

  const startStreaming = useCallback(() => {
    setStreamingContent('');
    setIsStreaming(true);
    setActiveTools([]);
    setError(null);

    // Simulate in browser dev mode (no Tauri events available)
    if (!isTauri) {
      setTimeout(() => {
        setStreamingContent('Running in browser dev mode — streaming is simulated.');
      }, 500);
      setTimeout(() => {
        setStreamingContent('');
        setIsStreaming(false);
        onDoneRef.current?.();
      }, 2000);
    }
  }, []);

  const failStreaming = useCallback((message: string) => {
    setStreamingContent('');
    setIsStreaming(false);
    setActiveTools([]);
    setError(message);
  }, []);

  useEvent<ContentChunkPayload>('agent:content_chunk', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setStreamingContent((prev) => prev + payload.data);
  });

  useEvent<ToolStartPayload>('agent:tool_start', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveTools((prev) => [...prev, payload.name]);
  });

  useEvent<ToolEndPayload>('agent:tool_end', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveTools((prev) => prev.filter((t) => t !== payload.name));
  });

  useEvent<InteractionRequestPayload>('agent:interaction_request', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveInteraction({ requestId: payload.requestId, request: payload.request });
  });

  useEvent<AgentDonePayload>('agent:done', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveInteraction(null);
    setStreamingContent('');
    setIsStreaming(false);
    onDoneRef.current?.();
  });

  useEvent<AgentErrorPayload>('agent:error', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setStreamingContent('');
    setError(payload.message);
    setIsStreaming(false);
  });

  const clearInteraction = useCallback(() => setActiveInteraction(null), []);

  return { streamingContent, isStreaming, activeTools, error, activeInteraction, startStreaming, failStreaming, clearInteraction };
}
