import { useState, useCallback, useEffect, useRef } from 'react';
import { useEvent } from './useEvent';
import { isTauri } from '../lib/utils';
import type {
  ActiveInteraction,
  ContentChunkPayload,
  ToolStartPayload,
  ToolEndPayload,
  AgentDonePayload,
  AgentErrorPayload,
  InteractionRequestPayload,
  MessageSegment,
} from '../lib/types';

interface AgentStream {
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  /** Call before sending a message to enter streaming mode. */
  startStreaming: () => void;
  /** Abort streaming and show an error. */
  failStreaming: (message: string) => void;
  /** Clear the active interaction (after submit/cancel). */
  clearInteraction: () => void;
  /** Clear accumulated segments (after persisted messages arrive). */
  clearSegments: () => void;
}

/**
 * Listens to Tauri agent streaming events for a specific session.
 *
 * @param sessionKey  Session to filter events for (empty = inert).
 * @param onDone      Called when the agent finishes — use for refetching messages.
 */
export function useAgentStream(sessionKey: string, onDone?: () => void): AgentStream {
  const [segments, setSegments] = useState<MessageSegment[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [activeTools, setActiveTools] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [activeInteraction, setActiveInteraction] = useState<ActiveInteraction | null>(null);

  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;
  const sessionKeyRef = useRef(sessionKey);
  sessionKeyRef.current = sessionKey;

  // Text buffer for the current streaming text segment.
  // Accumulates chunks in a ref (no re-render per chunk) and flushes to
  // segment state at most once per animation frame or at segment boundaries.
  const streamTextRef = useRef('');
  const rafRef = useRef<number>(0);

  const cancelRaf = useCallback(() => {
    if (rafRef.current) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = 0;
    }
  }, []);

  /** Flush buffered text into segments state. */
  const flushText = useCallback(() => {
    cancelRaf();
    setSegments((prev) => {
      const text = streamTextRef.current;
      if (!text) return prev;
      const last = prev[prev.length - 1];
      if (last && last.type === 'text') {
        return [...prev.slice(0, -1), { type: 'text' as const, content: text }];
      }
      return [...prev, { type: 'text' as const, content: text }];
    });
  }, [cancelRaf]);

  const resetStream = useCallback(() => {
    streamTextRef.current = '';
    cancelRaf();
    setSegments([]);
    setIsStreaming(false);
    setActiveTools([]);
    setError(null);
    setActiveInteraction(null);
  }, [cancelRaf]);

  // Reset when session changes
  useEffect(() => {
    resetStream();
  }, [sessionKey, resetStream]);

  const startStreaming = useCallback(() => {
    resetStream();
    setIsStreaming(true);

    // Simulate in browser dev mode (no Tauri events available)
    if (!isTauri) {
      setTimeout(() => {
        setSegments([{ type: 'text', content: 'Running in browser dev mode — streaming is simulated.' }]);
      }, 500);
      setTimeout(() => {
        resetStream();
        onDoneRef.current?.();
      }, 2000);
    }
  }, [resetStream]);

  const failStreaming = useCallback((message: string) => {
    resetStream();
    setError(message);
  }, [resetStream]);

  useEvent<ContentChunkPayload>('agent:content_chunk', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    streamTextRef.current += payload.data;
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(flushText);
    }
  });

  useEvent<ToolStartPayload>('agent:tool_start', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    // Flush text before the tool segment and reset for the next text segment
    flushText();
    streamTextRef.current = '';
    setActiveTools((prev) => [...prev, payload.name]);
  });

  useEvent<ToolEndPayload>('agent:tool_end', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    // Remove only the first occurrence (handles concurrent calls of same tool)
    setActiveTools((prev) => {
      const idx = prev.indexOf(payload.name);
      if (idx === -1) return prev;
      return [...prev.slice(0, idx), ...prev.slice(idx + 1)];
    });
    setSegments((prev) => [
      ...prev,
      {
        type: 'tool',
        name: payload.name,
        success: payload.success,
        durationMs: payload.durationMs,
        result: payload.result,
      },
    ]);
  });

  useEvent<InteractionRequestPayload>('agent:interaction_request', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveInteraction({ requestId: payload.requestId, request: payload.request });
  });

  useEvent<AgentDonePayload>('agent:done', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    setActiveInteraction(null);
    // Flush any trailing text and keep segments visible until persisted messages arrive
    flushText();
    streamTextRef.current = '';
    setIsStreaming(false);
    onDoneRef.current?.();
  });

  useEvent<AgentErrorPayload>('agent:error', (payload) => {
    if (!sessionKeyRef.current || payload.sessionKey !== sessionKeyRef.current) return;
    resetStream();
    setError(payload.message);
  });

  const clearInteraction = useCallback(() => setActiveInteraction(null), []);
  const clearSegments = useCallback(() => {
    streamTextRef.current = '';
    cancelRaf();
    setSegments([]);
  }, [cancelRaf]);

  // Cleanup rAF on unmount
  useEffect(() => cancelRaf, [cancelRaf]);

  return { segments, isStreaming, activeTools, error, activeInteraction, startStreaming, failStreaming, clearInteraction, clearSegments };
}
