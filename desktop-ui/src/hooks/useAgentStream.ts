import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ActiveInteraction,
  AgentDonePayload,
  AgentErrorPayload,
  AgentSelectedPayload,
  ClassificationCompletePayload,
  ContentChunkPayload,
  DelegationCompletedPayload,
  DelegationStartedPayload,
  ExecutionStartedPayload,
  InteractionRequestPayload,
  IterationStartPayload,
  LearningEventPayload,
  MemoryAccessPayload,
  MessageSegment,
  SkillLoadedPayload,
  SubagentSpawnedPayload,
  ToolEndPayload,
  ToolStartPayload,
  TransparencyData,
  UsageReportPayload,
} from "../lib/types";
import { isTauri, qualifiedToolName } from "../lib/utils";
import { useEvent } from "./useEvent";

/** All SSE event names the dev-api server emits — used to bridge SSE → CustomEvent in browser mode. */
const SSE_AGENT_EVENTS = [
  "agent:content_chunk",
  "agent:tool_start",
  "agent:tool_end",
  "agent:done",
  "agent:error",
  "agent:classification_complete",
  "agent:execution_started",
  "agent:iteration_start",
  "agent:usage_report",
  "agent:memory_access",
  "agent:skill_loaded",
  "agent:learning_event",
  "agent:agent_selected",
  "agent:subagent_spawned",
  "agent:delegation_started",
  "agent:delegation_completed",
  "agent:interaction_request",
  "entity:updated",
] as const;

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
  /** The agent currently being delegated to (between delegation_started and delegation_completed). */
  activeDelegateAgent: string | null;
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
  const [activeDelegateAgent, setActiveDelegateAgent] = useState<string | null>(null);

  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;
  const sessionKeyRef = useRef(sessionKey);
  sessionKeyRef.current = sessionKey;

  // Text buffer for the current streaming text segment.
  // Accumulates chunks in a ref (no re-render per chunk) and flushes to
  // segment state at most once per animation frame or at segment boundaries.
  const streamTextRef = useRef("");
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
      if (last && last.type === "text") {
        return [...prev.slice(0, -1), { type: "text" as const, content: text }];
      }
      return [...prev, { type: "text" as const, content: text }];
    });
  }, [cancelRaf]);

  const resetStream = useCallback(() => {
    streamTextRef.current = "";
    cancelRaf();
    setSegments([]);
    setIsStreaming(false);
    setActiveTools([]);
    setError(null);
    setActiveInteraction(null);
    setTransparency(null);
    setActiveDelegateAgent(null);
  }, [cancelRaf]);

  // Reset when session changes
  useEffect(() => {
    resetStream();
  }, [resetStream]);

  /** Guard: true if this payload belongs to our active session. */
  const isOurSession = (payload: { sessionKey: string }) =>
    sessionKeyRef.current !== "" && payload.sessionKey === sessionKeyRef.current;

  // SSE EventSource ref for browser mode
  const eventSourceRef = useRef<EventSource | null>(null);

  const startStreaming = useCallback(() => {
    resetStream();
    setIsStreaming(true);

    // In browser dev mode, connect to SSE endpoint and dispatch as CustomEvents
    // so the existing useEvent listeners pick them up.
    if (!isTauri && sessionKeyRef.current) {
      // Close any previous connection
      eventSourceRef.current?.close();

      const es = new EventSource(`/api/events/${encodeURIComponent(sessionKeyRef.current)}`);
      eventSourceRef.current = es;

      for (const eventName of SSE_AGENT_EVENTS) {
        es.addEventListener(eventName, (e: MessageEvent) => {
          try {
            const payload = JSON.parse(e.data);
            window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
          } catch {
            // skip malformed events
          }
        });
      }

      // Close on terminal events (done/error will be handled by useEvent listeners)
      const closeOnTerminal = () => {
        es.close();
        eventSourceRef.current = null;
      };
      es.addEventListener("agent:done", closeOnTerminal);
      es.addEventListener("agent:error", closeOnTerminal);

      es.onerror = () => {
        es.close();
        eventSourceRef.current = null;
      };
    }
  }, [resetStream]);

  const failStreaming = useCallback(
    (message: string) => {
      resetStream();
      setError(message);
    },
    [resetStream],
  );

  useEvent<ContentChunkPayload>("agent:content_chunk", (payload) => {
    if (!isOurSession(payload)) return;
    streamTextRef.current += payload.data;
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(flushText);
    }
  });

  useEvent<ToolStartPayload>("agent:tool_start", (payload) => {
    if (!isOurSession(payload)) return;
    // Flush text before the tool segment and reset for the next text segment
    flushText();
    streamTextRef.current = "";
    setActiveTools((prev) => [...prev, qualifiedToolName(payload.name, payload.action)]);
  });

  useEvent<ToolEndPayload>("agent:tool_end", (payload) => {
    if (!isOurSession(payload)) return;
    const displayName = qualifiedToolName(payload.name, payload.action);
    // Remove only the first occurrence (handles concurrent calls of same tool)
    setActiveTools((prev) => {
      const idx = prev.indexOf(displayName);
      if (idx === -1) return prev;
      return [...prev.slice(0, idx), ...prev.slice(idx + 1)];
    });
    setSegments((prev) => [
      ...prev,
      {
        type: "tool",
        name: payload.name,
        action: payload.action,
        success: payload.success,
        durationMs: payload.durationMs,
        result: payload.result,
        estimatedTokens: payload.estimatedTokens,
        agent: payload.agent,
      },
    ]);
    setTransparency((prev) => ({
      ...prev,
      tools: [
        ...(prev?.tools ?? []),
        {
          name: payload.name,
          action: payload.action,
          success: payload.success,
          durationMs: payload.durationMs,
          estimatedTokens: payload.estimatedTokens,
          agent: payload.agent,
        },
      ],
    }));
  });

  useEvent<InteractionRequestPayload>("agent:interaction_request", (payload) => {
    if (!isOurSession(payload)) return;
    setActiveInteraction({ requestId: payload.requestId, request: payload.request });
  });

  useEvent<AgentDonePayload>("agent:done", (payload) => {
    if (!isOurSession(payload)) return;
    setActiveInteraction(null);
    // Flush any trailing text and keep segments visible until persisted messages arrive
    flushText();
    streamTextRef.current = "";
    setIsStreaming(false);
    onDoneRef.current?.();
  });

  useEvent<AgentErrorPayload>("agent:error", (payload) => {
    if (!isOurSession(payload)) return;
    resetStream();
    setError(payload.message);
  });

  useEvent<ClassificationCompletePayload>("agent:classification_complete", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      classification: {
        strategy: payload.strategy,
        confidence: payload.confidence,
        source: payload.source,
      },
    }));
  });

  useEvent<ExecutionStartedPayload>("agent:execution_started", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      execution: {
        engine: payload.engine,
        iterations: 0,
        maxIterations: payload.maxIterations,
        escalations: 0,
      },
    }));
  });

  useEvent<IterationStartPayload>("agent:iteration_start", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      execution: prev?.execution
        ? { ...prev.execution, iterations: payload.iteration }
        : {
            engine: "unknown",
            iterations: payload.iteration,
            maxIterations: payload.maxIterations,
            escalations: 0,
          },
    }));
  });

  useEvent<UsageReportPayload>("agent:usage_report", (payload) => {
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

  useEvent<MemoryAccessPayload>("agent:memory_access", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      memoryAccesses: [
        ...(prev?.memoryAccesses ?? []),
        { action: payload.action, query: payload.query, resultsCount: payload.resultsCount },
      ],
    }));
  });

  useEvent<SkillLoadedPayload>("agent:skill_loaded", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      skills: [
        ...(prev?.skills ?? []),
        { name: payload.name, trigger: payload.trigger, agent: payload.agent },
      ],
    }));
  });

  useEvent<LearningEventPayload>("agent:learning_event", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      learning: [
        ...(prev?.learning ?? []),
        { eventType: payload.eventType, detail: payload.detail },
      ],
    }));
  });

  useEvent<AgentSelectedPayload>("agent:agent_selected", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      agentSelected: { name: payload.name, description: payload.description },
    }));
  });

  useEvent<SubagentSpawnedPayload>("agent:subagent_spawned", (payload) => {
    if (!isOurSession(payload)) return;
    setTransparency((prev) => ({
      ...prev,
      subagents: [...(prev?.subagents ?? []), { label: payload.label, profile: payload.profile }],
    }));
  });

  useEvent<DelegationStartedPayload>("agent:delegation_started", (payload) => {
    if (!isOurSession(payload)) return;
    setActiveDelegateAgent(payload.toAgent);
    setTransparency((prev) => ({
      ...prev,
      delegations: [
        ...(prev?.delegations ?? []),
        {
          fromAgent: payload.fromAgent,
          toAgent: payload.toAgent,
          query: payload.query,
          depth: payload.depth,
          status: "active",
        },
      ],
    }));
  });

  useEvent<DelegationCompletedPayload>("agent:delegation_completed", (payload) => {
    if (!isOurSession(payload)) return;
    setActiveDelegateAgent(null);
    setTransparency((prev) => ({
      ...prev,
      delegations: (prev?.delegations ?? []).map((d) =>
        d.toAgent === payload.toAgent && d.status === "active"
          ? {
              ...d,
              status: payload.success ? ("completed" as const) : ("failed" as const),
              durationMs: payload.durationMs,
            }
          : d,
      ),
    }));
  });

  const clearInteraction = useCallback(() => setActiveInteraction(null), []);
  const clearSegments = useCallback(() => {
    streamTextRef.current = "";
    cancelRaf();
    setSegments([]);
  }, [cancelRaf]);

  const clearTransparency = useCallback(() => setTransparency(null), []);

  // Cleanup rAF and SSE on unmount
  useEffect(() => {
    return () => {
      cancelRaf();
      eventSourceRef.current?.close();
      eventSourceRef.current = null;
    };
  }, [cancelRaf]);

  return {
    segments,
    isStreaming,
    activeTools,
    error,
    activeInteraction,
    transparency,
    activeDelegateAgent,
    startStreaming,
    failStreaming,
    clearInteraction,
    clearSegments,
    clearTransparency,
  };
}
