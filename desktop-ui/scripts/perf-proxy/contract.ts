export const SCHEMA_VERSION = 1;
export const MEASUREMENT_CONTRACT_VERSION = 1;
export const R_REPETITIONS = 5;
export const K_CAPTURES = 10;

export const MARGIN_POLICY = {
  rafP95: { multiplier: 3, floorMs: 2 },
  screenshotP50: { multiplier: 3, floorRatio: 0.15 },
} as const;

export type Identity = {
  schemaVersion: number;
  measurementContractVersion: number;
  environmentKey: string;
};

export type RowEnvironment = {
  webkitVersion: string;
  headless: boolean;
  viewport: { width: number; height: number };
  dpr: number;
  userAgent: string;
};

export type RowResult = {
  row: string;
  scenario: string;
  theme: string;
  n: number;
  renderPath: string;
  raf: {
    sampleCount: number;
    p50Ms: number;
    p95Ms: number;
    maxMs: number;
  };
  screenshot: {
    captureCount: number;
    p50Ms: number;
    options: Record<string, unknown>;
  };
  environment: RowEnvironment;
  durationMs: number;
};

export type MetricStat = {
  median: number;
  spread: number;
  margin: number;
};

export type KeyedDims = {
  runnerClass: string;
  platform: string;
  arch: string;
  osMajor: string;
  webkitVersion: string;
  headless: boolean;
  viewport: { width: number; height: number };
  dpr: number;
};

export type Diagnostics = {
  cpuModel: string;
  osRelease: string;
  userAgent: string;
};

export type OsFacts = {
  platform: string;
  arch: string;
  release: string;
  hostname: string;
  cpus: Array<{ model: string }>;
};

export type Baseline = {
  identity: Identity;
  recordedAt: string;
  sourceRevision: string;
  repetitions: number;
  rows: Record<
    string,
    {
      raf: { p95: MetricStat };
      screenshot: { p50: MetricStat };
    }
  >;
  environment: { keyed: KeyedDims; diagnostic: Diagnostics };
};

export type Outcome = "HEALTHY" | "DEGRADED" | "COULD_NOT_MEASURE";

export type Subcode =
  | "NO_BASELINE"
  | "PRECONDITION"
  | "UNEXPECTED_COMMAND"
  | "LAUNCH"
  | "TIMEOUT"
  | "IDENTITY"
  | "ROW_MISSING"
  | "LATEST_MALFORMED"
  | "CHILD_FAILED"
  | "PORT_BUSY"
  | "WRITE_FAILED";

export type RowDelta = {
  row: string;
  metric: string;
  baseline: number;
  current: number;
  delta: number;
  margin: number;
  exceeded: boolean;
};

export type LatestRun = {
  identity: Identity;
  environment: { keyed: KeyedDims; diagnostic: Diagnostics };
  rows: RowResult[];
  outcome: Outcome;
  subcode?: Subcode;
  comparison?: RowDelta[];
  recordedAt: string;
};
