// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import { invoke } from "@/api/client";
import { apiCache } from "./cache";

// Backend tracing provider id. The UI dispatches between two independent
// apps (KimiTracingApp / ClaudeCodeTracingApp) at the parent level
// (CodingMemoryPlugin), and each hard-codes its own provider id when
// calling these helpers — so a chip→provider mapper is no longer needed.
export type TracingProviderId = "kimi" | "claudeCode" | "klynt";

// ── Upstream-shaped types the ported components import ───────────────

export interface SessionMetadataInfo {
  session_id: string;
  title: string;
  title_generated: boolean;
  archived: boolean;
  archived_at: number | null;
  auto_archive_exempt: boolean;
  wire_mtime: number | null;
}

export interface SessionInfo {
  session_id: string;
  session_dir: string;
  work_dir: string | null;
  work_dir_hash: string;
  title: string;
  last_updated: number;
  has_wire: boolean;
  has_context: boolean;
  has_state: boolean;
  metadata: SessionMetadataInfo | null;
  wire_size: number;
  context_size: number;
  state_size: number;
  total_size: number;
  turns: number;
  imported?: boolean;
  subagent_count?: number;
}

export interface SessionSummary {
  turns: number;
  steps: number;
  tool_calls: number;
  errors: number;
  compactions: number;
  duration_sec: number;
  input_tokens: number;
  output_tokens: number;
  wire_size: number;
  context_size: number;
  state_size: number;
  total_size: number;
}

export interface WireEvent {
  index: number;
  timestamp: number;
  type: string;
  payload: Record<string, unknown>;
  meta?: boolean;
}

export interface WireResponse {
  total: number;
  events: WireEvent[];
}

export interface ContentPart {
  type: string;
  text?: string;
  think?: string;
  thinking?: string;
  encrypted?: string;
  image_url?: { url: string; id?: string };
  audio_url?: { url: string; id?: string };
  video_url?: { url: string; id?: string };
  [key: string]: unknown;
}

export interface ToolCallItem {
  id: string;
  type: string;
  function: { name: string; arguments: string };
  extras?: Record<string, unknown>;
}

export interface ContextMessage {
  index: number;
  role: string;
  content?: ContentPart[] | string;
  tool_calls?: ToolCallItem[];
  tool_call_id?: string;
  name?: string;
  partial?: boolean;
  token_count?: number;
  id?: number;
  [key: string]: unknown;
}

export interface ContextResponse {
  total: number;
  messages: ContextMessage[];
}

export interface AggregateStats {
  total_sessions: number;
  total_turns: number;
  total_tokens: { input: number; output: number };
  total_duration_sec: number;
  tool_usage: { name: string; count: number; error_count: number }[];
  daily_usage: { date: string; sessions: number; turns: number }[];
  per_project: { work_dir: string; sessions: number; turns: number }[];
}

export interface VisCapabilities {
  open_in_supported: boolean;
}

export type SubagentStatus =
  | "idle"
  | "running_foreground"
  | "running_background"
  | "completed"
  | "failed"
  | "killed";

export interface SubagentInfo {
  agent_id: string;
  subagent_type: string;
  status: SubagentStatus;
  description: string;
  created_at: number;
  updated_at: number;
  last_task_id: string | null;
  wire_size: number;
  context_size: number;
  launch_spec: Record<string, unknown>;
}

export function normalizeContent(
  content: ContentPart[] | string | undefined | null,
): ContentPart[] {
  if (!content) return [];
  if (typeof content === "string") return [{ type: "text", text: content }];
  if (Array.isArray(content)) return content;
  return [];
}

// ── listSessions ──────────────────────────────────────────────────────

type BackendSessionSummary = {
  sessionId: string;
  providerId: string;
  sourceDir: string;
  cwd: string | null;
  projectBasename: string | null;
  customTitle: string | null;
  startedAt: string;
  lastEventAt: string;
  sizeBytes: number;
  turnCount: number;
  stepCount: number;
  toolCallCount: number;
  errorCount: number;
  subagentCount: number;
  hasWire: boolean;
  hasContext: boolean;
  imported: boolean;
  workDirHash: string;
  hasState: boolean;
  wireSize: number;
  contextSize: number;
  stateSize: number;
  totalSize: number;
  metadata: {
    sessionId: string;
    title: string;
    titleGenerated: boolean;
    archived: boolean;
    archivedAt: number | null;
    autoArchiveExempt: boolean;
    wireMtime: number | null;
  } | null;
};

function reshapeSession(b: BackendSessionSummary): SessionInfo {
  return {
    session_id: b.sessionId,
    session_dir: b.sourceDir,
    work_dir: b.cwd,
    work_dir_hash: b.workDirHash,
    title: b.customTitle ?? b.metadata?.title ?? b.sessionId,
    last_updated: new Date(b.lastEventAt).getTime() / 1000,
    has_wire: b.hasWire,
    has_context: b.hasContext,
    has_state: b.hasState,
    metadata: b.metadata
      ? {
          session_id: b.metadata.sessionId,
          title: b.metadata.title,
          title_generated: b.metadata.titleGenerated,
          archived: b.metadata.archived,
          archived_at: b.metadata.archivedAt,
          auto_archive_exempt: b.metadata.autoArchiveExempt,
          wire_mtime: b.metadata.wireMtime,
        }
      : null,
    wire_size: b.wireSize,
    context_size: b.contextSize,
    state_size: b.stateSize,
    total_size: b.totalSize,
    turns: b.turnCount,
    imported: b.imported,
    subagent_count: b.subagentCount,
  };
}

export async function listSessions(
  providerId: string,
  forceRefresh = false,
): Promise<SessionInfo[]> {
  const key = `${providerId}:sessions`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(
    key,
    async () => {
      const rows = await invoke<BackendSessionSummary[]>("tracing_list_sessions", {
        providerId,
      });
      return rows.map(reshapeSession);
    },
    30_000,
  );
}

// ── getWireEvents ─────────────────────────────────────────────────────

const CONTENT_PART_MAP: Record<string, string> = {
  text: "TextPart",
  think: "ThinkPart",
};

function normalizeWireEvents(res: WireResponse): WireResponse {
  return {
    ...res,
    events: res.events.map((e) => {
      if (
        e.type === "ContentPart" &&
        typeof (e.payload as Record<string, unknown>).type === "string"
      ) {
        const mapped = CONTENT_PART_MAP[(e.payload as Record<string, unknown>).type as string];
        if (mapped) return { ...e, type: mapped };
      }
      if (
        e.type === "SubagentEvent" &&
        (e.payload as Record<string, unknown>).event &&
        typeof (e.payload as Record<string, unknown>).event === "object"
      ) {
        const inner = (e.payload as Record<string, unknown>).event as Record<string, unknown>;
        if (inner.type === "ContentPart" && inner.payload && typeof inner.payload === "object") {
          const innerPayload = inner.payload as Record<string, unknown>;
          const mapped = CONTENT_PART_MAP[innerPayload.type as string];
          if (mapped) {
            return {
              ...e,
              payload: { ...e.payload, event: { ...inner, type: mapped } },
            };
          }
        }
      }
      return e;
    }),
  };
}

type BackendTraceEvent = {
  seq: number;
  providerId: string;
  rawKind: string;
  payload: Record<string, unknown>;
  occurredAt: string;
  category: string;
  turnIndex: number | null;
  stepIndex: number | null;
  parentSubagentId: string | null;
  meta: boolean;
};

type BackendSessionDetail = {
  sessionId: string;
  providerId: string;
  scope: { kind: "main" } | { kind: "subagent"; agentId: string };
  stats: Record<string, unknown>;
  events: BackendTraceEvent[];
  truncated: boolean;
  totalEventCount: number;
};

function reshapeWire(detail: BackendSessionDetail): WireResponse {
  return {
    total: detail.totalEventCount,
    events: detail.events.map((ev) => ({
      index: ev.seq,
      timestamp: new Date(ev.occurredAt).getTime() / 1000,
      type: ev.rawKind,
      payload: ev.payload,
      meta: ev.meta,
    })),
  };
}

export function getWireEvents(
  providerId: string,
  sessionId: string,
  forceRefresh = false,
): Promise<WireResponse> {
  const key = `${providerId}:wire:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const detail = await invoke<BackendSessionDetail>("tracing_load_session", {
      providerId,
      sessionId,
      scope: { kind: "main" },
    });
    return normalizeWireEvents(reshapeWire(detail));
  });
}

// ── getContextMessages ────────────────────────────────────────────────

type BackendContextMessage = {
  index: number;
  role: string;
  content: unknown;
};

function reshapeContext(rows: BackendContextMessage[]): ContextResponse {
  return {
    total: rows.length,
    messages: rows.map((m) => ({
      index: m.index,
      role: m.role,
      content: m.content as ContextMessage["content"],
    })),
  };
}

export function getContextMessages(
  providerId: string,
  sessionId: string,
  forceRefresh = false,
): Promise<ContextResponse> {
  const key = `${providerId}:context:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendContextMessage[]>("tracing_load_context", {
      providerId,
      sessionId,
      scope: { kind: "main" },
    });
    return reshapeContext(rows);
  });
}

// ── getSessionState + getSessionSummary ───────────────────────────────

export function getSessionState(
  providerId: string,
  sessionId: string,
  forceRefresh = false,
): Promise<Record<string, unknown>> {
  const key = `${providerId}:state:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, () =>
    invoke<Record<string, unknown>>("tracing_load_state", {
      providerId,
      sessionId,
    }),
  );
}

export function getSessionSummary(
  providerId: string,
  sessionId: string,
  forceRefresh = false,
): Promise<SessionSummary> {
  const key = `${providerId}:summary:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const b = await invoke<BackendSessionSummary>("tracing_session_summary", {
      providerId,
      sessionId,
    });
    return {
      turns: b.turnCount,
      steps: b.stepCount,
      tool_calls: b.toolCallCount,
      errors: b.errorCount,
      compactions: 0,
      duration_sec: 0,
      input_tokens: 0,
      output_tokens: 0,
      wire_size: b.wireSize,
      context_size: b.contextSize,
      state_size: b.stateSize,
      total_size: b.totalSize,
    };
  });
}

// ── Subagent functions ────────────────────────────────────────────────

type BackendSubagentSummary = {
  agentId: string;
  subagentType: string;
  status: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
  eventCount: number;
};

function reshapeSubagent(b: BackendSubagentSummary): SubagentInfo {
  return {
    agent_id: b.agentId,
    subagent_type: b.subagentType,
    status: b.status as SubagentStatus,
    description: b.description ?? "",
    created_at: new Date(b.createdAt).getTime() / 1000,
    updated_at: new Date(b.updatedAt).getTime() / 1000,
    last_task_id: null,
    wire_size: 0,
    context_size: 0,
    launch_spec: {},
  };
}

export function getSubagents(
  providerId: string,
  sessionId: string,
  forceRefresh = false,
): Promise<SubagentInfo[]> {
  const key = `${providerId}:subagents:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendSubagentSummary[]>("tracing_list_subagents", {
      providerId,
      sessionId,
    });
    return rows.map(reshapeSubagent);
  });
}

export function getSubagentWireEvents(
  providerId: string,
  sessionId: string,
  agentId: string,
  forceRefresh = false,
): Promise<WireResponse> {
  const key = `${providerId}:subagent-wire:${sessionId}:${agentId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const detail = await invoke<BackendSessionDetail>("tracing_load_subagent_session", {
      providerId,
      sessionId,
      agentId,
    });
    return normalizeWireEvents(reshapeWire(detail));
  });
}

export function getSubagentContextMessages(
  providerId: string,
  sessionId: string,
  agentId: string,
  forceRefresh = false,
): Promise<ContextResponse> {
  const key = `${providerId}:subagent-context:${sessionId}:${agentId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendContextMessage[]>("tracing_load_subagent_context", {
      providerId,
      sessionId,
      agentId,
    });
    return reshapeContext(rows);
  });
}

// ── Stats, capabilities, import, open, delete ────────────────────────

type BackendStats = {
  perProject: {
    projectBasename: string;
    cwd: string;
    sessionCount: number;
    turnCount: number;
    toolCallCount: number;
    errorCount: number;
    totalInputTokens: number;
    totalOutputTokens: number;
    cacheReadTokens: number;
  }[];
  toolUsage: { tool: string; callCount: number; errorCount: number }[];
  errorsByTool: { tool: string; errorCount: number }[];
  tokenSeries: { day: string; inputTokens: number; outputTokens: number }[];
  subagentTypes: { subagentType: string; count: number }[];
  cacheHitPct: number;
};

export async function getAggregateStats(
  providerId: string,
  forceRefresh = false,
): Promise<AggregateStats> {
  const key = `${providerId}:aggregate-stats`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(
    key,
    async () => {
      const b = await invoke<BackendStats>("tracing_stats", { providerId });
      const totalSessions = b.perProject.reduce((s, p) => s + p.sessionCount, 0);
      const totalTurns = b.perProject.reduce((s, p) => s + p.turnCount, 0);
      const totalInput = b.perProject.reduce((s, p) => s + p.totalInputTokens, 0);
      const totalOutput = b.perProject.reduce((s, p) => s + p.totalOutputTokens, 0);
      return {
        total_sessions: totalSessions,
        total_turns: totalTurns,
        total_tokens: { input: totalInput, output: totalOutput },
        total_duration_sec: 0,
        tool_usage: b.toolUsage.map((t) => ({
          name: t.tool,
          count: t.callCount,
          error_count: t.errorCount,
        })),
        daily_usage: b.tokenSeries.map((d) => ({
          date: d.day,
          sessions: 0,
          turns: 0,
        })),
        per_project: b.perProject.map((p) => ({
          work_dir: p.cwd,
          sessions: p.sessionCount,
          turns: p.turnCount,
        })),
      };
    },
    60_000,
  );
}

export function getVisCapabilities(_forceRefresh = false): Promise<VisCapabilities> {
  return Promise.resolve({ open_in_supported: true });
}

export function getSessionDownloadUrl(_providerId: string, _sessionId: string): string {
  // Download not supported in the desktop port.
  return "";
}

export async function openInPath(
  providerId: string,
  _app: "finder",
  sessionId: string,
): Promise<void> {
  await invoke("tracing_open_dir", { providerId, sessionId });
}

export async function importSession(
  providerId: string,
  file: File,
): Promise<{ session_id: string; work_dir_hash: string }> {
  const arrayBuffer = await file.arrayBuffer();
  const bytes = Array.from(new Uint8Array(arrayBuffer));
  const result = await invoke<{ sessionId: string; workDirHash: string }>("tracing_import", {
    providerId,
    bytes,
    fileName: file.name,
  });
  apiCache.invalidate(`${providerId}:sessions`);
  return { session_id: result.sessionId, work_dir_hash: result.workDirHash };
}

export async function deleteSession(_sessionId: string): Promise<void> {
  throw new Error("Session deletion is not supported in the desktop port.");
}

export interface SessionTabsResponse {
  tabs: ("wire" | "tree" | "context" | "state" | "dual" | "agents")[];
}

export interface HeaderLayoutResponse {
  chips: (
    | "turns"
    | "steps"
    | "messages"
    | "toolCalls"
    | "errors"
    | "compactions"
    | "agents"
    | "duration"
    | "tokens"
    | "cacheHitPct"
    | "model"
  )[];
}

export async function fetchSupportedTabs(providerId: string): Promise<SessionTabsResponse> {
  const tabs = await invoke<SessionTabsResponse["tabs"]>("tracing_supported_tabs", { providerId });
  return { tabs };
}

export async function fetchHeaderLayout(providerId: string): Promise<HeaderLayoutResponse> {
  const chips = await invoke<HeaderLayoutResponse["chips"]>("tracing_header_layout", {
    providerId,
  });
  return { chips };
}
