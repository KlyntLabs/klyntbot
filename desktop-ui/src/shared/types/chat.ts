import type { DelegationInfo, PlanData } from "./common";
import type { Task } from "./tasks";

// ── Message Segments ────────────────────────────────────────

export type MessageSegment =
  | { type: "text"; content: string }
  | {
      type: "tool";
      name: string;
      action?: string;
      success: boolean;
      durationMs: number;
      result?: string;
      estimatedTokens?: number;
      agent?: string;
    };

// ── Chat Messages ───────────────────────────────────────────

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "interaction";
  content: string;
  timestamp?: string;
  segments?: MessageSegment[];
  transparency?: TransparencyData;
}

// ── Chat Thread ─────────────────────────────────────────────

export interface ChatThread {
  sessionKey: string;
  title: string;
  messageCount: number;
  updatedAt: string;
  contextType?: string;
  entityKind?: string;
  entityId?: string;
  areaId?: string;
  areaName?: string;
  projectId?: string;
  projectName?: string;
  squadId?: string;
}

// ── Session Context ─────────────────────────────────────────

export interface SessionContext {
  entityKind?: string;
  entityId?: string;
  contextType?: string;
  isEphemeral?: boolean;
}

// ── Agent Streaming Events ──────────────────────────────────

export interface ContentChunkPayload {
  sessionKey: string;
  data: string;
}

export interface ToolStartPayload {
  sessionKey: string;
  name: string;
  action?: string;
  agent?: string;
}

export interface ToolEndPayload {
  sessionKey: string;
  name: string;
  action?: string;
  success: boolean;
  durationMs: number;
  result?: string;
  estimatedTokens?: number;
  agent?: string;
}

export interface AgentDonePayload {
  sessionKey: string;
  content: string;
}

export interface AgentErrorPayload {
  sessionKey: string;
  message: string;
}

export interface ClassificationCompletePayload {
  sessionKey: string;
  strategy: string;
  confidence: number;
  source: string;
}

export interface ExecutionStartedPayload {
  sessionKey: string;
  engine: string;
  maxIterations: number;
}

// ── Transparency Events ──────────────────────────────────────

export interface IterationStartPayload {
  sessionKey: string;
  iteration: number;
  maxIterations: number;
}

export interface UsageReportPayload {
  sessionKey: string;
  promptTokens: number;
  completionTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  estimatedCostUsd: number;
  model: string;
  responseTimeMs: number;
}

export interface MemoryAccessPayload {
  sessionKey: string;
  action: string;
  query?: string;
  resultsCount: number;
}

export interface SkillLoadedPayload {
  sessionKey: string;
  name: string;
  trigger: string;
  agent?: string;
}

export interface LearningEventPayload {
  sessionKey: string;
  eventType: string;
  detail: string;
}

export interface SubagentSpawnedPayload {
  sessionKey: string;
  label: string;
  profile: string;
}

export interface AgentSelectedPayload {
  sessionKey: string;
  name: string;
  description: string;
}

// ── Plan Events ─────────────────────────────────────────────

export interface PlanGeneratedPayload {
  sessionKey: string;
  steps: string[];
  rawPlan: string;
}

export interface PlanStepCompletedPayload {
  sessionKey: string;
  stepIndex: number;
  description: string;
  toolName: string;
}

// ── Persona Events ──────────────────────────────────────────

export interface PersonaPerspectivePayload {
  sessionKey: string;
  personaId: string;
  personaName: string;
  personaIcon: string;
  personaRole: string;
  content: string;
  challenge?: string;
}

export interface PersonaSegment {
  personaId: string;
  personaName: string;
  personaIcon?: string;
  personaRole?: string;
  content: string;
  challenge?: string;
}

// ── Debate Events ──────────────────────────────────────────

export type DebatePhase = "opening" | "discussion" | "targeted" | "final";

export interface DebateRoundStartedPayload {
  sessionKey: string;
  round: number;
  totalRounds: number;
  phase: DebatePhase;
}

export interface DebateRoundCompletedPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
}

export interface ConsensusReachedPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
  summary: string;
}

export interface DebateJudgeDecisionPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
  decision: "continue" | "final_round" | "stop";
  speakingOrder: string[];
  reasoning: string;
}

export interface JudgeDecisionEntry {
  round: number;
  consensusScore: number;
  decision: "continue" | "final_round" | "stop";
  speakingOrder: string[];
  reasoning: string;
}

export interface MemoryPromotedPayload {
  sessionKey: string;
  factId: string;
  fromScope: string;
  toScope: string;
  subject: string;
  predicate: string;
}

export interface DebateRound {
  round: number;
  phase: DebatePhase;
  personaMessages: PersonaSegment[];
  consensusScore: number | null;
}

// ── Delegation Events ────────────────────────────────────────

export interface DelegationStartedPayload {
  sessionKey: string;
  fromAgent: string;
  toAgent: string;
  query: string;
  depth: number;
}

export interface DelegationCompletedPayload {
  sessionKey: string;
  fromAgent: string;
  toAgent: string;
  success: boolean;
  durationMs: number;
}

// ── Transparency Data (per-message) ──────────────────────────

export interface TransparencyData {
  usage?: {
    promptTokens: number;
    completionTokens: number;
    cacheReadTokens: number;
    cacheWriteTokens: number;
  };
  cost?: { estimatedUsd: number; model: string };
  timing?: { totalMs: number; classificationMs?: number; contextAssemblyMs?: number };
  tools?: {
    name: string;
    action?: string;
    success: boolean;
    durationMs: number;
    estimatedTokens?: number;
    agent?: string;
  }[];
  toolTokensTotal?: number;
  memoryAccesses?: { action: string; query?: string; resultsCount: number }[];
  skills?: { name: string; trigger: string; agent?: string }[];
  execution?: { engine: string; iterations: number; maxIterations: number; escalations: number };
  classification?: { strategy: string; confidence: number; source: string };
  agentSelected?: { name: string; description: string };
  subagents?: { label: string; profile: string }[];
  learning?: { eventType: string; detail: string }[];
  delegations?: DelegationInfo[];
  plan?: PlanData;
}

// ── Interaction (ask_user) ───────────────────────────────────

export interface ActiveInteraction {
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequestPayload {
  sessionKey: string;
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequest {
  title: string;
  questions: Question[];
}

export interface Question {
  id: string;
  title: string;
  text: string;
  answer_type: AnswerType;
}

export type AnswerType =
  | { type: "single_select"; options: AnswerOption[] }
  | { type: "multi_select"; options: AnswerOption[] }
  | { type: "yes_no"; default?: boolean }
  | { type: "free_text"; placeholder?: string };

export interface AnswerOption {
  value: string;
  label: string;
  description?: string;
}

export type AnswerValue =
  | { type: "selected"; value: string }
  | { type: "multi_selected"; values: string[] }
  | { type: "yes_no"; answer: boolean }
  | { type: "text"; content: string }
  | { type: "skipped" };

export interface Answer {
  question_id: string;
  value: AnswerValue;
}

export type FormResponse = { Completed: Answer[] } | "Cancelled";

// ── Agent Status ─────────────────────────────────────────────

export interface AgentStatus {
  status: string;
  activeTaskCount: number;
  focusTask: Task | null;
}
