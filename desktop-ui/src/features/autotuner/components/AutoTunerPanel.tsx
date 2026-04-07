import { useAutoTunerHistory } from "../hooks/useAutoTunerHistory";
import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";
import { ChampionCard } from "./ChampionCard";
import { ExperimentPaceControl } from "./ExperimentPaceControl";
import { ExperimentTimeline } from "./ExperimentTimeline";

export function AutoTunerPanel() {
  const { data: status, loading: statusLoading, refetch } = useAutoTunerStatus();
  const { data: history, loading: historyLoading } = useAutoTunerHistory(20);

  if (statusLoading) {
    return (
      <div className="flex flex-col gap-3">
        <div className="glass-card p-4 h-28 animate-pulse" />
        <div className="glass-card p-4 h-40 animate-pulse" />
      </div>
    );
  }

  if (!status) {
    return (
      <div className="glass-card p-4">
        <p className="text-xs font-light text-dim">AutoTuner unavailable</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <ChampionCard status={status} onRefetch={refetch} />
      {!status.paused && <ExperimentPaceControl />}
      <ExperimentTimeline experiments={history} loading={historyLoading} />
    </div>
  );
}
