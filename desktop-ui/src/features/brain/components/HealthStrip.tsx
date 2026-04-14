import { useQuery } from "@shared/hooks/useQuery";

interface MemoryStats {
  activeFacts: number;
  archivedFacts: number;
  episodicCount: number;
  rulesCount: number;
}

interface MirrorState {
  latestBrainVersion: { version: number; promotedAt: string } | null;
  recentTrialPreviews: unknown[];
}

interface CoachingSituation {
  energyLevel?: number;
  focusState?: number;
}

export function HealthStrip() {
  const { data: memStats } = useQuery<MemoryStats>("cognitive_memory_stats", undefined, {
    activeFacts: 0,
    archivedFacts: 0,
    episodicCount: 0,
    rulesCount: 0,
  });

  const { data: mirrorState } = useQuery<MirrorState>("get_mirror_state", undefined, {
    latestBrainVersion: null,
    recentTrialPreviews: [],
  });

  const { data: situation } = useQuery<CoachingSituation>("coaching_situation", undefined, {});

  const { data: interventions } = useQuery<unknown[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );

  const brainVersion = mirrorState.latestBrainVersion;
  const trialCount = mirrorState.recentTrialPreviews?.length ?? 0;
  const pendingCount = interventions?.length ?? 0;

  return (
    <div className="grid grid-cols-4 gap-3">
      <MetricCard
        label="Knowledge Trust"
        value={`${memStats.activeFacts}`}
        sub={`${memStats.activeFacts} facts · ${memStats.episodicCount} episodic`}
        valueClass="text-success"
      />
      <MetricCard
        label="Brain Version"
        value={brainVersion ? `v${brainVersion.version}` : "v1"}
        sub={brainVersion ? new Date(brainVersion.promotedAt).toLocaleDateString() : "Initial"}
        valueClass="text-foreground"
      />
      <MetricCard
        label="Coaching"
        value={situation.focusState !== undefined ? "Active" : "Idle"}
        sub={`${pendingCount} pending`}
        valueClass="text-info"
      />
      <MetricCard
        label="Experiments"
        value={`${trialCount}`}
        sub={trialCount === 0 ? "No active trials" : `${trialCount} active`}
        valueClass="text-purple"
      />
    </div>
  );
}

function MetricCard({
  label,
  value,
  sub,
  valueClass,
}: {
  label: string;
  value: string;
  sub: string;
  valueClass: string;
}) {
  return (
    <div className="bg-surface-lowest border border-border rounded-xl px-4 py-3">
      <p className="text-2xs uppercase tracking-wide text-dim mb-1">{label}</p>
      <p className={`text-xl font-semibold ${valueClass}`}>{value}</p>
      <p className="text-2xs text-dim mt-0.5">{sub}</p>
    </div>
  );
}
