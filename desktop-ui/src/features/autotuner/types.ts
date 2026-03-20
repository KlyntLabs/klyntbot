// ChampionSummary and ExperimentSummary fields are snake_case because the Rust
// autotuner crate serializes without rename_all = "camelCase". AutoTunerStatus
// uses camelCase because its wrapper struct does have that annotation.

export interface ChampionSummary {
  trial_id: string | null;
  description: string;
  impact: string;
  promoted_at: string;
  days_active: number;
}

export interface ExperimentSummary {
  id: string;
  variant_count: number;
  messages_scored: number;
  hypothesis: string;
  started_at: string;
}

export interface BrainGrowth {
  correctionsCaptured7d: number;
  trialsEvaluated7d: number;
  promotedThisWeek: number;
  status: string;
}

export interface MetricsHealth {
  correctionRateAvailable: boolean;
  tokenRateAvailable: boolean;
  stabilityAvailable: boolean;
}

export interface AutoTunerStatus {
  enabled: boolean;
  champion: ChampionSummary;
  activeExperiment: ExperimentSummary | null;
  paused: boolean;
  brainGrowth: BrainGrowth | null;
  metricsHealth: MetricsHealth | null;
  experimentPace: string | null;
}

export interface ParamChange {
  name: string;
  old_value: number;
  new_value: number;
}
