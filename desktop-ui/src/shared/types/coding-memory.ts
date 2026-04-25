export interface CodingMemoryStatusResponse {
  daemonAlive: boolean;
  bufferedEventCount: number;
  unprocessedEventCount: number;
  socketPath: string;
}

export interface CliHealthRow {
  cli: string;
  enabled: boolean;
  lastEventAt: string | null;
  eventCount24h: number;
}

export interface SessionReplayEntry {
  id: string;
  source: string;
  sessionId: string;
  kind: string;
  occurredAt: string;
  payload: string;
}

export interface RecallInvocationRow {
  id: string;
  occurredAt: string;
  sessionId: string | null;
  turnId: string | null;
  repoId: string | null;
  layer: string;
  query: string;
  coverageScore: number | null;
  skillUsed: string | null;
  latencyMs: number;
  resultIds: string[];
  renderedTokens: number | null;
  metadata: unknown;
}

export interface DiagnoseResult {
  ok: boolean;
  message: string;
}
