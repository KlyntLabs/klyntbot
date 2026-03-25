import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";
import type { BrainGrowth } from "../types";

interface BrainHealthBadgeProps {
  compact?: boolean;
}

function dotConfig(status: BrainGrowth["status"]) {
  switch (status) {
    case "needs_feedback":
      return { color: "bg-dim", pulse: false, label: "Waiting for feedback" };
    case "adapting":
      return { color: "bg-warning", pulse: true, label: "Learning from your corrections" };
    case "growing":
      return { color: "bg-success", pulse: true, label: "Actively improving" };
    default:
      return { color: "bg-dim", pulse: false, label: status };
  }
}

export function BrainHealthBadge({ compact = false }: BrainHealthBadgeProps) {
  const { data: status } = useAutoTunerStatus();

  if (!status?.enabled) return null;
  if (!status.brainGrowth) return null;

  const { color, pulse, label } = dotConfig(status.brainGrowth.status);

  return (
    <span className="inline-flex items-center gap-1.5">
      <span className="relative flex h-2 w-2">
        {pulse && (
          <span
            className={`absolute inset-0 rounded-full ${color} opacity-75 animate-ping`}
            style={{ animationDuration: "2s" }}
          />
        )}
        <span className={`relative inline-flex h-2 w-2 rounded-full ${color}`} />
      </span>
      {!compact && <span className="text-2xs font-light text-muted-foreground">{label}</span>}
    </span>
  );
}
