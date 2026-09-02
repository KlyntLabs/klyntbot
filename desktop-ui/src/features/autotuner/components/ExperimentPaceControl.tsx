import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";

const PACES = [
  { value: "conservative", label: "Conservative" },
  { value: "balanced", label: "Balanced" },
  { value: "bold", label: "Bold" },
] as const;

export function ExperimentPaceControl() {
  const { data: status } = useAutoTunerStatus();
  const { mutate: setPace } = useMutation<void, { pace: string }>("autotuner_set_pace");

  const currentPace = status?.experimentPace ?? "balanced";

  const handleSetPace = async (pace: string) => {
    if (pace === currentPace) return;
    await setPace({ pace });
    invalidateQueries("autotuner_");
  };

  return (
    <div className="glass-card p-4 flex flex-col gap-2">
      <p className="text-ui-sm font-medium text-fg-secondary">Experiment Pace</p>
      <div className="flex rounded-lg border border-separator overflow-hidden">
        {PACES.map((p) => (
          <button
            key={p.value}
            type="button"
            onClick={() => handleSetPace(p.value)}
            className={`flex-1 px-3 py-1.5 text-ui-xs font-medium transition-colors ${
              currentPace === p.value
                ? "bg-brand/15 text-brand"
                : "text-fg-secondary hover:text-fg hover:bg-white/[0.03]"
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>
    </div>
  );
}
