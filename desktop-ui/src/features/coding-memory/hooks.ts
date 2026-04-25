import { useQuery } from "@shared/hooks";
import type { CliHealthRow, CodingMemoryStatusResponse, SessionReplayEntry } from "@shared/types";

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

export const useCostRollup = () =>
  useQuery<{ period: string; cost_usd: number }[] | null>("coding_memory_cost", undefined, null);

export const useSensitivityInspector = () =>
  useQuery<{ id: string; subject: string; predicate: string; object: string; sensitivity: string }[] | null>(
    "coding_memory_sensitivity",
    undefined,
    null,
  );
