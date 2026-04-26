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

export interface MirrorAlertRow {
  id: string;
  kind: string;
  severity: string;
  headline: string;
  payload: string;
  createdAt: string;
  dismissed: boolean;
}

export interface EffectivenessTrendBucket {
  at: string;
  score: number;
}

export interface EffectivenessTrendsResponse {
  patternId: string;
  patternName: string;
  buckets: EffectivenessTrendBucket[];
}

export interface ReforgeCycleSummary {
  cycleId: string;
  ranAt: string;
  repos: string[];
  artifactsWritten: number;
}

export interface ReforgeCycleDiffResponse {
  beforeBody: string;
  afterBody: string;
  sectionLabels: string[];
}

export interface ProjectSkillRow {
  skillName: string;
  repoId: string;
  activeVersion: number;
  status: string;
  effectiveness: number;
}
