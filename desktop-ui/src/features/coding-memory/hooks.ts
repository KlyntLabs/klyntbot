import { useQuery } from "@shared/hooks";
import type {
  CliHealthRow,
  CodingMemoryStatusResponse,
  RecallInvocationRow,
  SessionReplayEntry,
} from "@shared/types";

export const useCodingMemoryStatus = () =>
  useQuery<CodingMemoryStatusResponse | null>("coding_memory_status", undefined, null);

export const useCliHealth = () =>
  useQuery<CliHealthRow[] | null>("coding_memory_cli_health", undefined, null);

export const useSessionReplay = (sessionId?: string, limit = 500, offset = 0) =>
  useQuery<SessionReplayEntry[] | null>("coding_memory_session_replay", {
    sessionId: sessionId ?? null,
    limit,
    offset,
  });

export const useMemoryBrowser = () =>
  useQuery<{ id: string; subject: string; predicate: string; object: string }[] | null>(
    "coding_memory_browser",
    undefined,
    null,
  );

export const useActivityTimeline = () =>
  useQuery<{ date: string; count: number }[] | null>("coding_memory_activity", undefined, null);

export interface CostBreakdownRow {
  date: string;
  model: string;
  source: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  inputCostUsd: number;
  outputCostUsd: number;
  totalCostUsd: number;
}
export interface CostBreakdown {
  rows: CostBreakdownRow[];
  totalCostUsd: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCachedTokens: number;
  totalRequests: number;
  bucketCount: number;
  modelCount: number;
  dayCount: number;
}

export const useCostBreakdown = (days = 30) =>
  useQuery<CostBreakdown | null>("coding_memory_cost", { days }, null);

export const useSensitivityInspector = () =>
  useQuery<
    { id: string; subject: string; predicate: string; object: string; sensitivity: string }[] | null
  >("coding_memory_sensitivity", undefined, null);

export function useRecallLog(layer?: string, limit = 50, offset = 0) {
  return useQuery<RecallInvocationRow[] | null>(
    "coding_memory_recall_log",
    {
      layer: layer ?? null,
      limit,
      offset,
    },
    null,
  );
}

export function useSessionRecallOverlay(sessionId: string, limit = 200, offset = 0) {
  return useQuery<RecallInvocationRow[] | null>(
    "coding_memory_session_replay_recall_overlay",
    {
      sessionId,
      limit,
      offset,
    },
    null,
  );
}
