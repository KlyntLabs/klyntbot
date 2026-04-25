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

export interface DiagnoseResult {
  ok: boolean;
  message: string;
}
