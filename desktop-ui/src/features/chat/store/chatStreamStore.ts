/**
 * Global chat stream store — thin shim over `useChatStore`.
 *
 * During the PR6 migration window this module keeps its public API so
 * non-React callers (approval queue, file-edit events) don't break.
 * React consumers should migrate to `useChatStore` directly (see
 * `useAgentStream.ts` for the reference implementation).
 */

import { isTauri } from "@tauri-apps/api/core";
import type { ConversationItem } from "@/types";
import {
  DEFAULT_STREAM_SNAPSHOT,
  type StreamSnapshot,
} from "@/features/chat/types";
import { useChatStore } from "@/features/threads/store/useChatStore";
import type {
  AgentCancelledPayload,
  AgentDonePayload,
  AgentErrorPayload,
  AgentSelectedPayload,
  ApprovalRequestedPayload,
  ApprovalResolvedPayload,
  ClassificationCompletePayload,
  ConsensusReachedPayload,
  ContentChunkPayload,
  DeadEndWarningSurfacedPayload,
  DebateJudgeDecisionPayload,
  DebateRoundCompletedPayload,
  DebateRoundStartedPayload,
  DelegationCompletedPayload,
  DelegationStartedPayload,
  ExecutionStartedPayload,
  InteractionRequestPayload,
  IterationStartPayload,
  LearningEventPayload,
  MemoryAccessPayload,
  PersonaPerspectivePayload,
  PersonaSegment,
  PlanGeneratedPayload,
  PlanStepCompletedPayload,
  RecallInjectedPayload,
  RetrievalEnhancedPayload,
  SandboxPolicyAppliedPayload,
  SkillLoadedPayload,
  SubagentSpawnedPayload,
  ToolEndPayload,
  ToolStartPayload,
  UsageReportPayload,
} from "../types";

const DEV_SSE_BASE = "http://127.0.0.1:3456";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;
type DiffItem = Extract<ConversationItem, { kind: "diff" }>;
const EMPTY_APPROVALS: ApprovalItem[] = [];

function qualifiedToolName(name: string, action?: string): string {
  return action ? `${name}:${action}` : name;
}

/** All SSE event names — used for the browser-mode EventSource bridge. */
const SSE_AGENT_EVENTS = [
  "agent:content_chunk",
  "agent:tool_start",
  "agent:tool_end",
  "agent:done",
  "agent:error",
  "agent:cancelled",
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
  "agent:persona_perspective",
  "agent:debate_round_started",
  "agent:debate_round_completed",
  "agent:debate_judge_decision",
  "agent:consensus_reached",
  "agent:pipeline_started",
  "agent:context_assembled",
  "agent:retrieval_enhanced",
  "agent:memory_promoted",
  "agent:approval_requested",
  "agent:approval_resolved",
  "agent:recall_injected",
  "agent:dead_end_warning_surfaced",
  "agent:sandbox_policy_applied",
  "entity:updated",
] as const;

interface PipelineStartedPayload {
  sessionKey: string;
}

interface ContextAssembledPayload {
  sessionKey: string;
  totalTokens: number;
  durationMs: number;
}

type Listener = () => void;

// ── Store ───────────────────────────────────────────────────────────────

class ChatStreamStore {
  private static MAX_IDLE_SESSIONS = 5;
  private static MAX_SEGMENTS = 200;
  private static MAX_TOOL_RESULT_LENGTH = 2000;

  private textBuffers = new Map<string, string>();
  private rafIds = new Map<string, number>();
  private eventSources = new Map<string, EventSource>();
  private onDoneCallbacks = new Map<string, Set<() => void>>();
  private listenersInitialized = false;
  private tauriUnlisteners: Array<() => void> = [];

  constructor() {
    this.initEventListeners();
  }

  // ── Public API (consumed by useAgentStream shim) ────────────────────

  /** Subscribe to store changes — passed to useSyncExternalStore. */
  subscribe = (listener: Listener): (() => void) => {
    return useChatStore.subscribe((state, prevState) => {
      if (
        state.streamSnapshots !== prevState.streamSnapshots ||
        state.streamApprovals !== prevState.streamApprovals ||
        state.streamFileEdits !== prevState.streamFileEdits
      ) {
        listener();
      }
    });
  };

  /** Get snapshot for a session — passed to useSyncExternalStore. */
  getSnapshot(sessionKey: string): StreamSnapshot {
    return useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
  }

  /** Reset state and begin streaming for a session. */
  startStream(sessionKey: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    useChatStore.getState()._setStreamSnapshot(sessionKey, {
      ...DEFAULT_STREAM_SNAPSHOT,
      isStreaming: true,
      statusPhase: "Thinking",
      needsRefetch: false,
    });

    if (!isTauri()) {
      this.connectEventSource(sessionKey);
    }

    this.evictIdleSessions();
  }

  /** Abort streaming with an error. */
  failStream(sessionKey: string, message: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...DEFAULT_STREAM_SNAPSHOT, error: message });
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
    const state = useChatStore.getState().streamSnapshots[sessionKey];
    if (state?.needsRefetch) {
      useChatStore.getState()._setStreamSnapshot(sessionKey, { ...state, needsRefetch: false });
    }
  }

  /** Get pending approvals for a session. */
  getApprovals(sessionKey: string): ApprovalItem[] {
    return useChatStore.getState().streamApprovals[sessionKey] ?? EMPTY_APPROVALS;
  }

  /** Get file edit diffs for a session. */
  getFileEdits(sessionKey: string): DiffItem[] {
    return useChatStore.getState().streamFileEdits[sessionKey] ?? [];
  }

  /** Insert a file edit diff for a session. */
  upsertFileEdit(sessionKey: string, item: DiffItem): void {
    const existing = useChatStore.getState().streamFileEdits[sessionKey] ?? [];
    useChatStore.getState()._setStreamFileEdits(sessionKey, [...existing, item]);
  }

  /** Insert or update an approval request for a session. */
  upsertApproval(sessionKey: string, item: ApprovalItem): void {
    const existing = useChatStore.getState().streamApprovals[sessionKey] ?? [];
    const index = existing.findIndex((a) => a.requestId === item.requestId);
    const next =
      index >= 0
        ? [...existing.slice(0, index), item, ...existing.slice(index + 1)]
        : [...existing, item];
    useChatStore.getState()._setStreamApprovals(sessionKey, next);
  }

  /** Resolve an approval request with a final status. */
  resolveApproval(
    sessionKey: string,
    requestId: string,
    status: ApprovalItem["status"],
    decidedBy: ApprovalItem["decidedBy"],
  ): void {
    const existing = useChatStore.getState().streamApprovals[sessionKey] ?? [];
    const index = existing.findIndex((a) => a.requestId === requestId);
    if (index < 0) return;
    const next = [...existing];
    next[index] = {
      ...next[index],
      status,
      decidedBy,
      decidedAt: new Date().toISOString(),
    };
    useChatStore.getState()._setStreamApprovals(sessionKey, next);
  }

  clearSegments(sessionKey: string): void {
    this.textBuffers.set(sessionKey, "");
    this.cancelRaf(sessionKey);
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, segments: [] });
  }

  clearTransparency(sessionKey: string): void {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, transparency: null });
  }

  clearInteraction(sessionKey: string): void {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, activeInteraction: null });
  }

  clearPersonaMessages(sessionKey: string): void {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...snap, personaMessages: [] });
  }

  /** Append a synthetic system segment to the stream. */
  appendSystemItem(sessionKey: string, kind: string, item: unknown): void {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, {
      ...snap,
      segments: [...snap.segments, { type: "system" as const, kind, item }],
    });
  }

  /** Append a synthetic error segment to the stream. */
  appendErrorItem(sessionKey: string, message: string): void {
    const snap = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(sessionKey, {
      ...snap,
      segments: [...snap.segments, { type: "error" as const, message }],
    });
  }

  // ── Internal helpers ────────────────────────────────────────────────

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

    const current = useChatStore.getState().streamSnapshots[sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    const last = current.segments[current.segments.length - 1];
    let newSegments: StreamSnapshot["segments"];
    if (last && last.type === "text") {
      newSegments = [...current.segments.slice(0, -1), { type: "text", content: text }];
    } else {
      newSegments = [...current.segments, { type: "text", content: text }];
    }
    if (newSegments.length > ChatStreamStore.MAX_SEGMENTS) {
      newSegments = newSegments.slice(-ChatStreamStore.MAX_SEGMENTS);
    }
    useChatStore.getState()._setStreamSnapshot(sessionKey, { ...current, segments: newSegments });
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

    if (isTauri()) {
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
    on<AgentCancelledPayload>("agent:cancelled", (p) => this.onCancelled(p));
    on<InteractionRequestPayload>("agent:interaction_request", (p) => this.onInteractionRequest(p));
    on<PipelineStartedPayload>("agent:pipeline_started", (p) => this.onPipelineStarted(p));
    on<ClassificationCompletePayload>("agent:classification_complete", (p) =>
      this.onClassificationComplete(p),
    );
    on<ContextAssembledPayload>("agent:context_assembled", (p) => this.onContextAssembled(p));
    on<RetrievalEnhancedPayload>("agent:retrieval_enhanced", (p) => this.onRetrievalEnhanced(p));
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
    on<PersonaPerspectivePayload>("agent:persona_perspective", (p) => this.onPersonaPerspective(p));
    on<DebateRoundStartedPayload>("agent:debate_round_started", (p) =>
      this.onDebateRoundStarted(p),
    );
    on<DebateRoundCompletedPayload>("agent:debate_round_completed", (p) =>
      this.onDebateRoundCompleted(p),
    );
    on<ConsensusReachedPayload>("agent:consensus_reached", (p) => this.onConsensusReached(p));
    on<DebateJudgeDecisionPayload>("agent:debate_judge_decision", (p) =>
      this.onDebateJudgeDecision(p),
    );
    on<{ path: string; op: string; bytes: number; diff: string }>(
      "agent:file_edit_with_symbols",
      (p) => this.onFileEditWithSymbols(p),
    );
    on<ApprovalRequestedPayload>("agent:approval_requested", (p) => this.onApprovalRequested(p));
    on<ApprovalResolvedPayload>("agent:approval_resolved", (p) => this.onApprovalResolved(p));
    on<RecallInjectedPayload>("agent:recall_injected", (p) => this.onRecallInjected(p));
    on<DeadEndWarningSurfacedPayload>("agent:dead_end_warning_surfaced", (p) =>
      this.onDeadEndWarningSurfaced(p),
    );
    on<SandboxPolicyAppliedPayload>("agent:sandbox_policy_applied", (p) =>
      this.onSandboxPolicyApplied(p),
    );
  }

  private initTauriListeners(): void {
    import("@tauri-apps/api/event").then(({ listen }) => {
      const register = <T>(event: string, handler: (payload: T) => void) => {
        const result = listen?.<T>(event, (e) => handler(e.payload));
        if (result && typeof result.then === "function") {
          result.then((off) => {
            if (off) this.tauriUnlisteners.push(off);
          });
        }
      };

      register<ContentChunkPayload>("agent:content_chunk", (p) => this.onContentChunk(p));
      register<ToolStartPayload>("agent:tool_start", (p) => this.onToolStart(p));
      register<ToolEndPayload>("agent:tool_end", (p) => this.onToolEnd(p));
      register<AgentDonePayload>("agent:done", (p) => this.onDone(p));
      register<AgentErrorPayload>("agent:error", (p) => this.onError(p));
      register<AgentCancelledPayload>("agent:cancelled", (p) => this.onCancelled(p));
      register<InteractionRequestPayload>("agent:interaction_request", (p) =>
        this.onInteractionRequest(p),
      );
      register<PipelineStartedPayload>("agent:pipeline_started", (p) => this.onPipelineStarted(p));
      register<ClassificationCompletePayload>("agent:classification_complete", (p) =>
        this.onClassificationComplete(p),
      );
      register<ContextAssembledPayload>("agent:context_assembled", (p) =>
        this.onContextAssembled(p),
      );
      register<RetrievalEnhancedPayload>("agent:retrieval_enhanced", (p) =>
        this.onRetrievalEnhanced(p),
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
      register<PersonaPerspectivePayload>("agent:persona_perspective", (p) =>
        this.onPersonaPerspective(p),
      );
      register<DebateRoundStartedPayload>("agent:debate_round_started", (p) =>
        this.onDebateRoundStarted(p),
      );
      register<DebateRoundCompletedPayload>("agent:debate_round_completed", (p) =>
        this.onDebateRoundCompleted(p),
      );
      register<ConsensusReachedPayload>("agent:consensus_reached", (p) =>
        this.onConsensusReached(p),
      );
      register<DebateJudgeDecisionPayload>("agent:debate_judge_decision", (p) =>
        this.onDebateJudgeDecision(p),
      );
      register<{ path: string; op: string; bytes: number; diff: string }>(
        "agent:file_edit_with_symbols",
        (p) => this.onFileEditWithSymbols(p),
      );
      register<ApprovalRequestedPayload>("agent:approval_requested", (p) =>
        this.onApprovalRequested(p),
      );
      register<ApprovalResolvedPayload>("agent:approval_resolved", (p) =>
        this.onApprovalResolved(p),
      );
      register<RecallInjectedPayload>("agent:recall_injected", (p) => this.onRecallInjected(p));
      register<DeadEndWarningSurfacedPayload>("agent:dead_end_warning_surfaced", (p) =>
        this.onDeadEndWarningSurfaced(p),
      );
      register<SandboxPolicyAppliedPayload>("agent:sandbox_policy_applied", (p) =>
        this.onSandboxPolicyApplied(p),
      );
    });
  }

  // ── Event handlers ──────────────────────────────────────────────────

  private isActive(sessionKey: string): boolean {
    const state = useChatStore.getState().streamSnapshots[sessionKey];
    return !!state?.isStreaming;
  }

  private onContentChunk(payload: ContentChunkPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const buf = (this.textBuffers.get(payload.sessionKey) || "") + payload.data;
    this.textBuffers.set(payload.sessionKey, buf);
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey];
    if (state && state.segments.length === 0 && state.activeTools.length === 0) {
      useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
        ...state,
        statusPhase: "Composing",
      });
    }
    this.scheduleFlush(payload.sessionKey);
  }

  private onToolStart(payload: ToolStartPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.flushText(payload.sessionKey);
    this.textBuffers.set(payload.sessionKey, "");
    const toolDisplay = qualifiedToolName(payload.name, payload.action);
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: `Using ${toolDisplay}`,
      activeTools: [...state.activeTools, toolDisplay],
    });
  }

  private onToolEnd(payload: ToolEndPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const displayName = qualifiedToolName(payload.name, payload.action);
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    const idx = state.activeTools.indexOf(displayName);
    const activeTools =
      idx === -1 ? state.activeTools : [...state.activeTools.slice(0, idx), ...state.activeTools.slice(idx + 1)];
    const newSegments = [
      ...state.segments,
      {
        type: "tool" as const,
        name: payload.name,
        action: payload.action,
        success: payload.success,
        durationMs: payload.durationMs,
        result:
          payload.result && payload.result.length > ChatStreamStore.MAX_TOOL_RESULT_LENGTH
            ? `${payload.result.slice(0, ChatStreamStore.MAX_TOOL_RESULT_LENGTH)}\n… (truncated)`
            : payload.result,
        estimatedTokens: payload.estimatedTokens,
        agent: payload.agent,
      },
    ];
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      activeTools,
      segments:
        newSegments.length > ChatStreamStore.MAX_SEGMENTS
          ? newSegments.slice(-ChatStreamStore.MAX_SEGMENTS)
          : newSegments,
      transparency: {
        ...state.transparency,
        tools: [
          ...(state.transparency?.tools ?? []),
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
    });
  }

  private onDone(payload: AgentDonePayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.flushText(payload.sessionKey);
    this.textBuffers.set(payload.sessionKey, "");

    const cbs = this.onDoneCallbacks.get(payload.sessionKey);
    const hasCallbacks = cbs && cbs.size > 0;

    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      isStreaming: false,
      statusPhase: null,
      activeInteraction: null,
      needsRefetch: !hasCallbacks,
    });

    if (cbs && hasCallbacks) {
      for (const cb of cbs) cb();
    }

    this.evictIdleSessions();
  }

  private evictIdleSessions(): void {
    const snapshots = useChatStore.getState().streamSnapshots;
    const keys = Object.keys(snapshots);
    if (keys.length <= ChatStreamStore.MAX_IDLE_SESSIONS) return;

    const idle: string[] = [];
    for (const [key, state] of Object.entries(snapshots)) {
      if (!state.isStreaming) idle.push(key);
    }
    if (idle.length <= ChatStreamStore.MAX_IDLE_SESSIONS) return;

    const toRemove = idle.length - ChatStreamStore.MAX_IDLE_SESSIONS;
    for (let i = 0; i < toRemove; i++) {
      this.deleteSessionState(idle[i]);
    }
  }

  private deleteSessionState(key: string): void {
    const { [key]: _, ...restSnapshots } = useChatStore.getState().streamSnapshots;
    const { [key]: __, ...restApprovals } = useChatStore.getState().streamApprovals;
    const { [key]: ___, ...restFileEdits } = useChatStore.getState().streamFileEdits;
    useChatStore.setState({
      streamSnapshots: restSnapshots,
      streamApprovals: restApprovals,
      streamFileEdits: restFileEdits,
    });
    this.textBuffers.delete(key);
    this.rafIds.delete(key);
    this.onDoneCallbacks.delete(key);
    const es = this.eventSources.get(key);
    if (es) {
      es.close();
      this.eventSources.delete(key);
    }
  }

  /** Dispose all resources — useful for HMR teardown. */
  dispose(): void {
    for (const off of this.tauriUnlisteners) {
      off();
    }
    this.tauriUnlisteners = [];
    for (const es of this.eventSources.values()) {
      es.close();
    }
    this.eventSources.clear();
    this.textBuffers.clear();
    this.onDoneCallbacks.clear();
    for (const id of this.rafIds.values()) {
      cancelAnimationFrame(id);
    }
    this.rafIds.clear();
    this.listenersInitialized = false;
  }

  private onError(payload: AgentErrorPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.textBuffers.set(payload.sessionKey, "");
    this.cancelRaf(payload.sessionKey);
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...DEFAULT_STREAM_SNAPSHOT,
      error: payload.message,
    });
  }

  private onCancelled(payload: AgentCancelledPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.textBuffers.set(payload.sessionKey, "");
    this.cancelRaf(payload.sessionKey);
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      isStreaming: false,
      cancelled: true,
      partialContent: payload.partialContent,
      partialReasoning: payload.partialReasoning,
    });
  }

  private onInteractionRequest(payload: InteractionRequestPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      activeInteraction: {
        requestId: payload.requestId,
        request: payload.request,
      },
    });
  }

  private onPipelineStarted(payload: PipelineStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: "Routing",
    });
  }

  private onClassificationComplete(payload: ClassificationCompletePayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: "Analyzing",
      transparency: {
        ...state.transparency,
        classification: {
          strategy: payload.strategy,
          confidence: payload.confidence,
          source: payload.source,
        },
      },
    });
  }

  private onContextAssembled(payload: ContextAssembledPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: "Recalling",
      transparency: {
        ...state.transparency,
        contextTokens: payload.totalTokens,
      },
    });
  }

  private onRetrievalEnhanced(payload: RetrievalEnhancedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        enhancement: {
          stages: payload.stages,
          totalLatencyMs: payload.totalLatencyMs,
          totalLlmCalls: payload.totalLlmCalls,
        },
      },
    });
  }

  private onExecutionStarted(payload: ExecutionStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: "Preparing",
      transparency: {
        ...state.transparency,
        execution: {
          engine: payload.engine,
          iterations: 0,
          maxIterations: payload.maxIterations,
          escalations: 0,
        },
      },
    });
  }

  private onIterationStart(payload: IterationStartPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        execution: state.transparency?.execution
          ? { ...state.transparency.execution, iterations: payload.iteration }
          : {
              engine: "unknown",
              iterations: payload.iteration,
              maxIterations: payload.maxIterations,
              escalations: 0,
            },
      },
    });
  }

  private onUsageReport(payload: UsageReportPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        usage: {
          promptTokens: payload.promptTokens,
          completionTokens: payload.completionTokens,
          cacheReadTokens: payload.cacheReadTokens,
          cacheWriteTokens: payload.cacheWriteTokens,
        },
        cost: { estimatedUsd: payload.estimatedCostUsd, model: payload.model },
        timing: { ...state.transparency?.timing, totalMs: payload.responseTimeMs },
      },
    });
  }

  private onMemoryAccess(payload: MemoryAccessPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        memoryAccesses: [
          ...(state.transparency?.memoryAccesses ?? []),
          {
            action: payload.action,
            query: payload.query,
            resultsCount: payload.resultsCount,
          },
        ],
      },
    });
  }

  private onSkillLoaded(payload: SkillLoadedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const skillLabel = payload.name.replace(/-management$/, "").replace(/-/g, " ");
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: `Loading ${skillLabel}`,
      transparency: {
        ...state.transparency,
        skills: [
          ...(state.transparency?.skills ?? []),
          { name: payload.name, trigger: payload.trigger, agent: payload.agent },
        ],
      },
    });
  }

  private onLearningEvent(payload: LearningEventPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        learning: [
          ...(state.transparency?.learning ?? []),
          { eventType: payload.eventType, detail: payload.detail },
        ],
      },
    });
  }

  private onAgentSelected(payload: AgentSelectedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      statusPhase: `Consulting ${payload.name}`,
      transparency: {
        ...state.transparency,
        agentSelected: { name: payload.name, description: payload.description },
      },
    });
  }

  private onSubagentSpawned(payload: SubagentSpawnedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        subagents: [
          ...(state.transparency?.subagents ?? []),
          { label: payload.label, profile: payload.profile },
        ],
      },
    });
  }

  private onDelegationStarted(payload: DelegationStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      activeDelegateAgent: payload.toAgent,
      transparency: {
        ...state.transparency,
        delegations: [
          ...(state.transparency?.delegations ?? []),
          {
            fromAgent: payload.fromAgent,
            toAgent: payload.toAgent,
            query: payload.query,
            depth: payload.depth,
            status: "active" as const,
          },
        ],
      },
    });
  }

  private onDelegationCompleted(payload: DelegationCompletedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      activeDelegateAgent: null,
      transparency: {
        ...state.transparency,
        delegations: (state.transparency?.delegations ?? []).map((d) =>
          d.toAgent === payload.toAgent && d.status === "active"
            ? {
                ...d,
                status: payload.success ? ("completed" as const) : ("failed" as const),
                durationMs: payload.durationMs,
              }
            : d,
        ),
      },
    });
  }

  private onPlanGenerated(payload: PlanGeneratedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        plan: { steps: payload.steps, completedSteps: [] },
      },
    });
  }

  private onPlanStepCompleted(payload: PlanStepCompletedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        plan: state.transparency?.plan
          ? {
              ...state.transparency.plan,
              completedSteps: [...state.transparency.plan.completedSteps, payload.stepIndex],
            }
          : undefined,
      },
    });
  }

  private onPersonaPerspective(payload: PersonaPerspectivePayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const segment: PersonaSegment = {
      personaId: payload.personaId,
      personaName: payload.personaName,
      personaIcon: payload.personaIcon,
      personaRole: payload.personaRole,
      content: payload.content,
      challenge: payload.challenge ?? undefined,
    };
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      personaMessages: [...state.personaMessages, segment],
      debateRounds:
        state.currentDebateRound !== null
          ? state.debateRounds.map((r) =>
              r.round === state.currentDebateRound
                ? { ...r, personaMessages: [...r.personaMessages, segment] }
                : r,
            )
          : state.debateRounds,
    });
  }

  private onDebateRoundStarted(payload: DebateRoundStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      currentDebateRound: payload.round,
      totalDebateRounds: payload.totalRounds,
      squadMode: "debate",
      debateRounds: [
        ...state.debateRounds,
        {
          round: payload.round,
          phase: payload.phase,
          personaMessages: [],
          consensusScore: null,
        },
      ],
    });
  }

  private onDebateRoundCompleted(payload: DebateRoundCompletedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      debateRounds: state.debateRounds.map((r) =>
        r.round === payload.round ? { ...r, consensusScore: payload.consensusScore } : r,
      ),
    });
  }

  private onConsensusReached(payload: ConsensusReachedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      consensusReached: true,
      consensusSummary: payload.summary,
    });
  }

  private onDebateJudgeDecision(payload: DebateJudgeDecisionPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      judgeDecisions: [
        ...state.judgeDecisions,
        {
          round: payload.round,
          consensusScore: payload.consensusScore,
          decision: payload.decision,
          speakingOrder: payload.speakingOrder,
          reasoning: payload.reasoning,
        },
      ],
    });
  }

  private onApprovalRequested(payload: ApprovalRequestedPayload): void {
    const item: ApprovalItem = {
      id: `approval-${payload.request_id}`,
      kind: "approval",
      requestId: payload.request_id,
      tool: payload.tool,
      args: payload.args,
      cwd: payload.cwd,
      sandboxSummary: payload.sandbox_summary,
      layer: payload.layer,
      layerReason: payload.layer_reason,
      mirrorHistory: payload.mirror_history
        ? {
            approvalCount: payload.mirror_history.approval_count,
            denialCount: payload.mirror_history.denial_count,
          }
        : undefined,
      status: "pending",
    };
    this.upsertApproval(payload.sessionKey, item);
  }

  private onApprovalResolved(payload: ApprovalResolvedPayload): void {
    const status: ApprovalItem["status"] =
      payload.decided_by === "auto_deny"
        ? "denied"
        : payload.decided_by === "timeout"
          ? "timed-out"
          : payload.decided_by === "cancelled"
            ? "cancelled"
            : "approved-once";
    this.resolveApproval(payload.sessionKey, payload.request_id, status, payload.decided_by);
  }

  private onRecallInjected(payload: RecallInjectedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.appendSystemItem(payload.sessionKey, "recall", {
      memory_ids: payload.memory_ids,
      coverage_score: payload.coverage_score,
      dead_end_warning: payload.dead_end_warning,
    });
  }

  private onDeadEndWarningSurfaced(payload: DeadEndWarningSurfacedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.appendSystemItem(payload.sessionKey, "dead_end_warning", {
      approach_summary: payload.approach_summary,
      prior_attempt_id: payload.prior_attempt_id,
      confidence: payload.confidence,
    });
  }

  private onSandboxPolicyApplied(payload: SandboxPolicyAppliedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    const state = useChatStore.getState().streamSnapshots[payload.sessionKey] ?? DEFAULT_STREAM_SNAPSHOT;
    useChatStore.getState()._setStreamSnapshot(payload.sessionKey, {
      ...state,
      transparency: {
        ...state.transparency,
        sandboxPolicy: {
          tool: payload.tool,
          policySummary: payload.policy_summary,
          fallbackUnsandboxed: payload.fallback_unsandboxed,
        },
      },
    });
  }

  private onFileEditWithSymbols(payload: {
    path: string;
    op: string;
    bytes: number;
    diff: string;
  }): void {
    const id = `diff-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    const item: DiffItem = {
      id,
      kind: "diff",
      title: payload.path.split("/").pop() ?? payload.path,
      diff: payload.diff,
      path: payload.path,
      op: payload.op as DiffItem["op"],
      bytes: payload.bytes,
    };
    const sessionKey = this.inferActiveSessionKey() ?? "global";
    this.upsertFileEdit(sessionKey, item);
  }

  private inferActiveSessionKey(): string | null {
    const snapshots = useChatStore.getState().streamSnapshots;
    for (const [key, state] of Object.entries(snapshots)) {
      if (state.isStreaming) return key;
    }
    return null;
  }
}

/** Singleton store instance. */
export const chatStreamStore = new ChatStreamStore();
