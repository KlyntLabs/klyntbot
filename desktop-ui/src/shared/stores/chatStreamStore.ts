/**
 * Global chat stream store — manages streaming state and EventSource connections
 * independently of React component lifecycle, so streams survive route changes.
 *
 * Components subscribe via `useSyncExternalStore` in useAgentStream.
 */

import { DEV_SSE_BASE, isTauri, qualifiedToolName } from "@shared/lib/utils";
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
  PlanGeneratedPayload,
  PlanStepCompletedPayload,
  SkillLoadedPayload,
  SubagentSpawnedPayload,
  ToolEndPayload,
  ToolStartPayload,
  TransparencyData,
  UsageReportPayload,
} from "@shared/types";

/** All SSE event names — used for the browser-mode EventSource bridge. */
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
  "agent:plan_generated",
  "agent:plan_step_completed",
  "agent:interaction_request",
  "entity:updated",
] as const;

// ── Stream state ────────────────────────────────────────────────────────

export interface StreamSnapshot {
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  transparency: TransparencyData | null;
  activeDelegateAgent: string | null;
  /** True when a stream finished while no component was subscribed to consume onDone. */
  needsRefetch: boolean;
}

const DEFAULT_SNAPSHOT: StreamSnapshot = Object.freeze({
  segments: [],
  isStreaming: false,
  activeTools: [],
  error: null,
  activeInteraction: null,
  transparency: null,
  activeDelegateAgent: null,
  needsRefetch: false,
});

type Listener = () => void;

// ── Store ───────────────────────────────────────────────────────────────

class ChatStreamStore {
  private states = new Map<string, StreamSnapshot>();
  private listeners = new Set<Listener>();
  private eventSources = new Map<string, EventSource>();
  private textBuffers = new Map<string, string>();
  private rafIds = new Map<string, number>();
  private onDoneCallbacks = new Map<string, Set<() => void>>();
  private listenersInitialized = false;

  constructor() {
    this.initEventListeners();
  }

  // ── Public API (consumed by useAgentStream) ─────────────────────────

  /** Subscribe to store changes — passed to useSyncExternalStore. */
  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  /** Get snapshot for a session — passed to useSyncExternalStore. */
  getSnapshot(sessionKey: string): StreamSnapshot {
    return this.states.get(sessionKey) ?? DEFAULT_SNAPSHOT;
  }

  /** Reset state and begin streaming for a session. */
  startStream(sessionKey: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    this.states.set(sessionKey, {
      ...DEFAULT_SNAPSHOT,
      isStreaming: true,
      needsRefetch: false,
    });
    this.notify();

    if (!isTauri) {
      this.connectEventSource(sessionKey);
    }
  }

  /** Abort streaming with an error. */
  failStream(sessionKey: string, message: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    this.states.set(sessionKey, { ...DEFAULT_SNAPSHOT, error: message });
    this.notify();
  }

  /** Register an onDone callback for a session (component-scoped). */
  registerOnDone(sessionKey: string, callback: () => void): () => void {
    let cbs = this.onDoneCallbacks.get(sessionKey);
    if (!cbs) {
      cbs = new Set();
      this.onDoneCallbacks.set(sessionKey, cbs);
    }
    cbs.add(callback);
    return () => cbs?.delete(callback);
  }

  /** Mark needsRefetch as consumed after a component processes it. */
  consumeRefetch(sessionKey: string): void {
    const state = this.states.get(sessionKey);
    if (state?.needsRefetch) {
      this.states.set(sessionKey, { ...state, needsRefetch: false });
      this.notify();
    }
  }

  clearSegments(sessionKey: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    this.updateState(sessionKey, (s) => ({ ...s, segments: [] }));
  }

  clearTransparency(sessionKey: string): void {
    this.updateState(sessionKey, (s) => ({ ...s, transparency: null }));
  }

  clearInteraction(sessionKey: string): void {
    this.updateState(sessionKey, (s) => ({ ...s, activeInteraction: null }));
  }

  // ── Internal helpers ────────────────────────────────────────────────

  private notify(): void {
    for (const l of this.listeners) l();
  }

  private updateState(sessionKey: string, updater: (s: StreamSnapshot) => StreamSnapshot): void {
    const current = this.states.get(sessionKey) ?? DEFAULT_SNAPSHOT;
    this.states.set(sessionKey, updater(current));
    this.notify();
  }

  private cancelRaf(sessionKey: string): void {
    const id = this.rafIds.get(sessionKey);
    if (id) {
      cancelAnimationFrame(id);
      this.rafIds.delete(sessionKey);
    }
  }

  private flushText(sessionKey: string): void {
    this.cancelRaf(sessionKey);
    const text = this.textBuffers.get(sessionKey) || "";
    if (!text) return;

    this.updateState(sessionKey, (s) => {
      const last = s.segments[s.segments.length - 1];
      if (last && last.type === "text") {
        return {
          ...s,
          segments: [...s.segments.slice(0, -1), { type: "text", content: text }],
        };
      }
      return { ...s, segments: [...s.segments, { type: "text", content: text }] };
    });
  }

  private scheduleFlush(sessionKey: string): void {
    if (!this.rafIds.has(sessionKey)) {
      this.rafIds.set(
        sessionKey,
        requestAnimationFrame(() => {
          this.rafIds.delete(sessionKey);
          this.flushText(sessionKey);
        }),
      );
    }
  }

  // ── EventSource (browser dev mode) ──────────────────────────────────

  private connectEventSource(sessionKey: string): void {
    this.eventSources.get(sessionKey)?.close();

    const es = new EventSource(`${DEV_SSE_BASE}/api/events/${encodeURIComponent(sessionKey)}`);
    this.eventSources.set(sessionKey, es);

    // Bridge SSE → CustomEvent so entity:updated reaches page components
    for (const eventName of SSE_AGENT_EVENTS) {
      es.addEventListener(eventName, (e: MessageEvent) => {
        try {
          const payload = JSON.parse(e.data);
          window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
        } catch {
          // skip malformed
        }
      });
    }

    const cleanup = () => {
      es.close();
      this.eventSources.delete(sessionKey);
    };
    es.addEventListener("agent:done", cleanup);
    es.addEventListener("agent:error", cleanup);
    es.onerror = cleanup;
  }

  // ── Event listener registration ─────────────────────────────────────

  private initEventListeners(): void {
    if (this.listenersInitialized) return;
    this.listenersInitialized = true;

    if (isTauri) {
      this.initTauriListeners();
    } else {
      this.initBrowserListeners();
    }
  }

  private initBrowserListeners(): void {
    const on = <T>(event: string, handler: (payload: T) => void) => {
      window.addEventListener(event, (e) => handler((e as CustomEvent).detail as T));
    };

    on<ContentChunkPayload>("agent:content_chunk", (p) => this.onContentChunk(p));
    on<ToolStartPayload>("agent:tool_start", (p) => this.onToolStart(p));
    on<ToolEndPayload>("agent:tool_end", (p) => this.onToolEnd(p));
    on<AgentDonePayload>("agent:done", (p) => this.onDone(p));
    on<AgentErrorPayload>("agent:error", (p) => this.onError(p));
    on<InteractionRequestPayload>("agent:interaction_request", (p) => this.onInteractionRequest(p));
    on<ClassificationCompletePayload>("agent:classification_complete", (p) =>
      this.onClassificationComplete(p),
    );
    on<ExecutionStartedPayload>("agent:execution_started", (p) => this.onExecutionStarted(p));
    on<IterationStartPayload>("agent:iteration_start", (p) => this.onIterationStart(p));
    on<UsageReportPayload>("agent:usage_report", (p) => this.onUsageReport(p));
    on<MemoryAccessPayload>("agent:memory_access", (p) => this.onMemoryAccess(p));
    on<SkillLoadedPayload>("agent:skill_loaded", (p) => this.onSkillLoaded(p));
    on<LearningEventPayload>("agent:learning_event", (p) => this.onLearningEvent(p));
    on<AgentSelectedPayload>("agent:agent_selected", (p) => this.onAgentSelected(p));
    on<SubagentSpawnedPayload>("agent:subagent_spawned", (p) => this.onSubagentSpawned(p));
    on<DelegationStartedPayload>("agent:delegation_started", (p) => this.onDelegationStarted(p));
    on<DelegationCompletedPayload>("agent:delegation_completed", (p) =>
      this.onDelegationCompleted(p),
    );
    on<PlanGeneratedPayload>("agent:plan_generated", (p) => this.onPlanGenerated(p));
    on<PlanStepCompletedPayload>("agent:plan_step_completed", (p) => this.onPlanStepCompleted(p));
  }

  private initTauriListeners(): void {
    import("@tauri-apps/api/event").then(({ listen }) => {
      const register = <T>(event: string, handler: (payload: T) => void) => {
        // Singleton listeners — intentionally never unlistened
        listen<T>(event, (e) => handler(e.payload));
      };

      register<ContentChunkPayload>("agent:content_chunk", (p) => this.onContentChunk(p));
      register<ToolStartPayload>("agent:tool_start", (p) => this.onToolStart(p));
      register<ToolEndPayload>("agent:tool_end", (p) => this.onToolEnd(p));
      register<AgentDonePayload>("agent:done", (p) => this.onDone(p));
      register<AgentErrorPayload>("agent:error", (p) => this.onError(p));
      register<InteractionRequestPayload>("agent:interaction_request", (p) =>
        this.onInteractionRequest(p),
      );
      register<ClassificationCompletePayload>("agent:classification_complete", (p) =>
        this.onClassificationComplete(p),
      );
      register<ExecutionStartedPayload>("agent:execution_started", (p) =>
        this.onExecutionStarted(p),
      );
      register<IterationStartPayload>("agent:iteration_start", (p) => this.onIterationStart(p));
      register<UsageReportPayload>("agent:usage_report", (p) => this.onUsageReport(p));
      register<MemoryAccessPayload>("agent:memory_access", (p) => this.onMemoryAccess(p));
      register<SkillLoadedPayload>("agent:skill_loaded", (p) => this.onSkillLoaded(p));
      register<LearningEventPayload>("agent:learning_event", (p) => this.onLearningEvent(p));
      register<AgentSelectedPayload>("agent:agent_selected", (p) => this.onAgentSelected(p));
      register<SubagentSpawnedPayload>("agent:subagent_spawned", (p) => this.onSubagentSpawned(p));
      register<DelegationStartedPayload>("agent:delegation_started", (p) =>
        this.onDelegationStarted(p),
      );
      register<DelegationCompletedPayload>("agent:delegation_completed", (p) =>
        this.onDelegationCompleted(p),
      );
      register<PlanGeneratedPayload>("agent:plan_generated", (p) => this.onPlanGenerated(p));
      register<PlanStepCompletedPayload>("agent:plan_step_completed", (p) =>
        this.onPlanStepCompleted(p),
      );
    });
  }

  // ── Event handlers ──────────────────────────────────────────────────

  private isActive(sessionKey: string): boolean {
    const state = this.states.get(sessionKey);
    return !!state?.isStreaming;
  }

  private onContentChunk(payload: ContentChunkPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const buf = (this.textBuffers.get(payload.sessionKey) || "") + payload.data;
    this.textBuffers.set(payload.sessionKey, buf);
    this.scheduleFlush(payload.sessionKey);
  }

  private onToolStart(payload: ToolStartPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.flushText(payload.sessionKey);
    this.textBuffers.set(payload.sessionKey, "");
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      activeTools: [...s.activeTools, qualifiedToolName(payload.name, payload.action)],
    }));
  }

  private onToolEnd(payload: ToolEndPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const displayName = qualifiedToolName(payload.name, payload.action);
    this.updateState(payload.sessionKey, (s) => {
      const idx = s.activeTools.indexOf(displayName);
      const activeTools =
        idx === -1
          ? s.activeTools
          : [...s.activeTools.slice(0, idx), ...s.activeTools.slice(idx + 1)];
      return {
        ...s,
        activeTools,
        segments: [
          ...s.segments,
          {
            type: "tool" as const,
            name: payload.name,
            action: payload.action,
            success: payload.success,
            durationMs: payload.durationMs,
            result: payload.result,
            estimatedTokens: payload.estimatedTokens,
            agent: payload.agent,
          },
        ],
        transparency: {
          ...s.transparency,
          tools: [
            ...(s.transparency?.tools ?? []),
            {
              name: payload.name,
              action: payload.action,
              success: payload.success,
              durationMs: payload.durationMs,
              estimatedTokens: payload.estimatedTokens,
              agent: payload.agent,
            },
          ],
        },
      };
    });
  }

  private onDone(payload: AgentDonePayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.flushText(payload.sessionKey);
    this.textBuffers.set(payload.sessionKey, "");

    // Fire onDone callbacks if any components are subscribed
    const cbs = this.onDoneCallbacks.get(payload.sessionKey);
    const hasCallbacks = cbs && cbs.size > 0;

    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      isStreaming: false,
      activeInteraction: null,
      // If no component is listening, mark for deferred refetch
      needsRefetch: !hasCallbacks,
    }));

    if (cbs && hasCallbacks) {
      for (const cb of cbs) cb();
    }
  }

  private onError(payload: AgentErrorPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.textBuffers.set(payload.sessionKey, "");
    this.cancelRaf(payload.sessionKey);
    this.states.set(payload.sessionKey, {
      ...DEFAULT_SNAPSHOT,
      error: payload.message,
    });
    this.notify();
  }

  private onInteractionRequest(payload: InteractionRequestPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      activeInteraction: {
        requestId: payload.requestId,
        request: payload.request,
      },
    }));
  }

  private onClassificationComplete(payload: ClassificationCompletePayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        classification: {
          strategy: payload.strategy,
          confidence: payload.confidence,
          source: payload.source,
        },
      },
    }));
  }

  private onExecutionStarted(payload: ExecutionStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        execution: {
          engine: payload.engine,
          iterations: 0,
          maxIterations: payload.maxIterations,
          escalations: 0,
        },
      },
    }));
  }

  private onIterationStart(payload: IterationStartPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        execution: s.transparency?.execution
          ? { ...s.transparency.execution, iterations: payload.iteration }
          : {
              engine: "unknown",
              iterations: payload.iteration,
              maxIterations: payload.maxIterations,
              escalations: 0,
            },
      },
    }));
  }

  private onUsageReport(payload: UsageReportPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        usage: {
          promptTokens: payload.promptTokens,
          completionTokens: payload.completionTokens,
          cacheReadTokens: payload.cacheReadTokens,
          cacheWriteTokens: payload.cacheWriteTokens,
        },
        cost: { estimatedUsd: payload.estimatedCostUsd, model: payload.model },
        timing: { ...s.transparency?.timing, totalMs: payload.responseTimeMs },
      },
    }));
  }

  private onMemoryAccess(payload: MemoryAccessPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        memoryAccesses: [
          ...(s.transparency?.memoryAccesses ?? []),
          {
            action: payload.action,
            query: payload.query,
            resultsCount: payload.resultsCount,
          },
        ],
      },
    }));
  }

  private onSkillLoaded(payload: SkillLoadedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        skills: [
          ...(s.transparency?.skills ?? []),
          { name: payload.name, trigger: payload.trigger, agent: payload.agent },
        ],
      },
    }));
  }

  private onLearningEvent(payload: LearningEventPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        learning: [
          ...(s.transparency?.learning ?? []),
          { eventType: payload.eventType, detail: payload.detail },
        ],
      },
    }));
  }

  private onAgentSelected(payload: AgentSelectedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        agentSelected: { name: payload.name, description: payload.description },
      },
    }));
  }

  private onSubagentSpawned(payload: SubagentSpawnedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        subagents: [
          ...(s.transparency?.subagents ?? []),
          { label: payload.label, profile: payload.profile },
        ],
      },
    }));
  }

  private onDelegationStarted(payload: DelegationStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      activeDelegateAgent: payload.toAgent,
      transparency: {
        ...s.transparency,
        delegations: [
          ...(s.transparency?.delegations ?? []),
          {
            fromAgent: payload.fromAgent,
            toAgent: payload.toAgent,
            query: payload.query,
            depth: payload.depth,
            status: "active" as const,
          },
        ],
      },
    }));
  }

  private onDelegationCompleted(payload: DelegationCompletedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      activeDelegateAgent: null,
      transparency: {
        ...s.transparency,
        delegations: (s.transparency?.delegations ?? []).map((d) =>
          d.toAgent === payload.toAgent && d.status === "active"
            ? {
                ...d,
                status: payload.success ? ("completed" as const) : ("failed" as const),
                durationMs: payload.durationMs,
              }
            : d,
        ),
      },
    }));
  }

  private onPlanGenerated(payload: PlanGeneratedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        plan: { steps: payload.steps, completedSteps: [] },
      },
    }));
  }

  private onPlanStepCompleted(payload: PlanStepCompletedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
      ...s,
      transparency: {
        ...s.transparency,
        plan: s.transparency?.plan
          ? {
              ...s.transparency.plan,
              completedSteps: [...s.transparency.plan.completedSteps, payload.stepIndex],
            }
          : undefined,
      },
    }));
  }
}

/** Singleton store instance. */
export const chatStreamStore = new ChatStreamStore();
