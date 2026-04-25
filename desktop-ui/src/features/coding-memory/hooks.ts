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
