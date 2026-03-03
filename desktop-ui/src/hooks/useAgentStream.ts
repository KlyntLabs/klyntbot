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
  TransparencyData,
  ClassificationCompletePayload,
  ExecutionStartedPayload,
  IterationStartPayload,
  UsageReportPayload,
  MemoryAccessPayload,
  SkillLoadedPayload,
  LearningEventPayload,
  SubagentSpawnedPayload,
} from '../lib/types';

interface AgentStream {
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  transparency: TransparencyData | null;
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
  const [transparency, setTransparency] = useState<TransparencyData | null>(null);

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
    setTransparency(null);
  }, [cancelRaf]);

  // Reset when session changes
  useEffect(() => {
    resetStream();
  }, [sessionKey, resetStream]);

  /** Guard: true if this payload belongs to our active session. */
  const isOurSession = (payload: { sessionKey: string }) =>
    sessionKeyRef.current !== '' && payload.sessionKey === sessionKeyRef.current;

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
    if (!isOurSession(payload)) return;
    streamTextRef.current += payload.data;
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(flushText);
    }
  });

  useEvent<ToolStartPayload>('agent:tool_start', (payload) => {
    if (!isOurSession(payload)) return;
    // Flush text before the tool segment and reset for the next text segment
    flushText();
    streamTextRef.current = '';
    setActiveTools((prev) => [...prev, payload.name]);
  });

  useEvent<ToolEndPayload>('agent:tool_end', (payload) => {
    if (!isOurSession(payload)) return;
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
    setTransparency((prev) => ({
      ...prev,
      tools: [...(prev?.tools ?? []), { name: payload.name, success: payload.success, durationMs: payload.durationMs }],
    }));
  });

  useEvent<InteractionRequestPayload>('agent:interaction_request', (payload) => {
    if (!isOurSession(payload)) return;
    setActiveInteraction({ requestId: payload.requestId, request: payload.request });
  });

  useEvent<AgentDonePayload>('agent:done', (payload) => {
    if (!isOurSession(payload)) return;
    setActiveInteraction(null);
    // Flush any trailing text and keep segments visible until persisted messages arrive
    flushText();
    streamTextRef.current = '';
    setIsStreaming(false);
    onDoneRef.current?.();
  });

  useEvent<AgentErrorPayload>('agent:error', (payload) => {
    if (!isOurSession(payload)) return;
    resetStream();
    setError(payload.message);
  });

  useEvent<ClassificationCompletePayload>('agent:classification_complete', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      classification: { strategy: payload.strategy, confidence: payload.confidence, source: payload.source },
    }));
  });

  useEvent<ExecutionStartedPayload>('agent:execution_started', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      execution: { engine: payload.engine, iterations: 0, maxIterations: payload.maxIterations, escalations: 0 },
    }));
  });

  useEvent<IterationStartPayload>('agent:iteration_start', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      execution: prev?.execution
        ? { ...prev.execution, iterations: payload.iteration }
        : { engine: 'unknown', iterations: payload.iteration, maxIterations: payload.maxIterations, escalations: 0 },
    }));
  });

  useEvent<UsageReportPayload>('agent:usage_report', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      usage: {
        promptTokens: payload.promptTokens,
        completionTokens: payload.completionTokens,
        cacheReadTokens: payload.cacheReadTokens,
        cacheWriteTokens: payload.cacheWriteTokens,
      },
      cost: { estimatedUsd: payload.estimatedCostUsd, model: payload.model },
      timing: { ...prev?.timing, totalMs: payload.responseTimeMs },
    }));
  });

  useEvent<MemoryAccessPayload>('agent:memory_access', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      memoryAccesses: [...(prev?.memoryAccesses ?? []), { action: payload.action, query: payload.query, resultsCount: payload.resultsCount }],
    }));
  });

  useEvent<SkillLoadedPayload>('agent:skill_loaded', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      skills: [...(prev?.skills ?? []), { name: payload.name, trigger: payload.trigger }],
    }));
  });

  useEvent<LearningEventPayload>('agent:learning_event', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      learning: [...(prev?.learning ?? []), { eventType: payload.eventType, detail: payload.detail }],
    }));
  });

  useEvent<SubagentSpawnedPayload>('agent:subagent_spawned', (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      subagents: [...(prev?.subagents ?? []), { label: payload.label, profile: payload.profile }],
    }));
  });

  const clearInteraction = useCallback(() => setActiveInteraction(null), []);
  const clearSegments = useCallback(() => {
    streamTextRef.current = '';
    cancelRaf();
    setSegments([]);
  }, [cancelRaf]);

  const clearTransparency = useCallback(() => setTransparency(null), []);

  // Cleanup rAF on unmount
  useEffect(() => cancelRaf, [cancelRaf]);

  return { segments, isStreaming, activeTools, error, activeInteraction, transparency, startStreaming, failStreaming, clearInteraction, clearSegments, clearTransparency };
}
