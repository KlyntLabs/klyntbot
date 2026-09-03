import { useMutation } from "@shared/hooks/useMutation";
import { FlaskConical, Pause, Play, RotateCcw, Trophy } from "lucide-react";
import type { AutoTunerStatus, MetricsHealth } from "../types";
import { BrainHealthBadge } from "./BrainHealthBadge";

interface ChampionCardProps {
  status: AutoTunerStatus;
  onRefetch: () => void;
}

function MetricDot({ available, label }: { available: boolean; label: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <span className={`h-1.5 w-1.5 rounded-full ${available ? "bg-status-success" : "bg-dim"}`} />
      <span className="text-ui-xs font-light text-fg-dim">{label}</span>
    </span>
  );
}

export function ChampionCard({ status, onRefetch }: ChampionCardProps) {
  const { champion, activeExperiment, paused, brainGrowth, metricsHealth } = status;

  const { mutate: revert, loading: reverting } = useMutation("autotuner_revert");
  const { mutate: pause } = useMutation("autotuner_pause");
  const { mutate: resume } = useMutation("autotuner_resume");

  const handleRevert = async () => {
    await revert({} as Record<string, unknown>);
    onRefetch();
  };

  const handlePauseResume = async () => {
    if (paused) {
      await resume({} as Record<string, unknown>);
    } else {
      await pause({} as Record<string, unknown>);
    }
    onRefetch();
  };

  return (
    <div className="island p-4 flex flex-col gap-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-ui font-medium text-fg-secondary flex items-center gap-2">
          <Trophy className="size-3.5 text-brand" />
          Current Champion
        </h2>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handlePauseResume}
            title={paused ? "Resume autotuner" : "Pause autotuner"}
            className="size-6 rounded-md flex items-center justify-center text-fg-secondary
              hover:text-fg hover:bg-control-hover transition-colors"
          >
            {paused ? <Play className="size-3" /> : <Pause className="size-3" />}
          </button>
          {champion.trial_id && (
            <button
              type="button"
              onClick={handleRevert}
              disabled={reverting}
              title="Revert to baseline"
              className="size-6 rounded-md flex items-center justify-center text-fg-secondary
                hover:text-status-danger hover:bg-control-hover transition-colors disabled:opacity-40"
            >
              <RotateCcw className="size-3" />
            </button>
          )}
        </div>
      </div>

      {/* Champion info */}
      <div className="flex flex-col gap-1.5">
        <p className="text-ui-sm text-fg font-light leading-relaxed">{champion.description}</p>
        {champion.impact && <p className="text-ui-xs text-brand font-light">{champion.impact}</p>}
        <p className="text-ui-xs text-fg-dim font-light">
          Active {champion.days_active === 1 ? "1 day" : `${champion.days_active} days`}
          {paused && (
            <span className="ml-2 px-1.5 py-0.5 rounded bg-status-warning/10 text-status-warning">
              paused
            </span>
          )}
        </p>
      </div>

      {/* Brain health */}
      {brainGrowth && (
        <div className="mt-1 pt-3 border-t border-separator flex flex-col gap-2">
          <BrainHealthBadge />
          <p className="text-ui-xs font-light text-fg-dim">
            {brainGrowth.correctionsCaptured7d} corrections &middot; {brainGrowth.trialsEvaluated7d}{" "}
            evaluated &middot; {brainGrowth.promotedThisWeek} promoted
          </p>
          {metricsHealth && <MetricsHealthDots health={metricsHealth} />}
        </div>
      )}

      {/* Active experiment */}
      {activeExperiment && (
        <div className="mt-1 pt-3 border-t border-separator flex flex-col gap-1">
          <p className="text-ui-xs font-medium text-fg-secondary flex items-center gap-1.5">
            <FlaskConical className="size-3 text-brand" />
            Running Experiment
          </p>
          <p className="text-ui-xs font-light text-fg leading-relaxed">
            {activeExperiment.hypothesis}
          </p>
          <p className="text-ui-xs text-fg-dim font-light">
            {activeExperiment.variant_count} variants &middot; {activeExperiment.messages_scored}{" "}
            messages scored
          </p>
        </div>
      )}
    </div>
  );
}

function MetricsHealthDots({ health }: { health: MetricsHealth }) {
  return (
    <div className="flex items-center gap-3">
      <MetricDot available={health.correctionRateAvailable} label="Corrections" />
      <MetricDot available={health.tokenRateAvailable} label="Tokens" />
      <MetricDot available={health.stabilityAvailable} label="Stability" />
    </div>
  );
}
