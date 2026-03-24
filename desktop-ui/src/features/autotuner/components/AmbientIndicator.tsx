import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";
import { BrainHealthBadge } from "./BrainHealthBadge";

interface AmbientIndicatorProps {
  onClick?: () => void;
}

export function AmbientIndicator({ onClick }: AmbientIndicatorProps) {
  const { data: status } = useAutoTunerStatus();

  if (!status?.enabled) return null;

  const brainStatus = status.brainGrowth?.status;

  const text = (() => {
    switch (brainStatus) {
      case "needs_feedback":
        return "Help me learn \u2014 correct me when I'm wrong";
      case "adapting":
        return `Learning from ${status.brainGrowth?.correctionsCaptured7d ?? 0} corrections this week`;
      case "growing":
        return status.champion.impact || "Getting to know you better";
      default:
        if (status.champion.impact) return status.champion.impact;
        return null;
    }
  })();

  if (!text) return null;

  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1.5 text-2xs font-light text-dim
        hover:text-muted-foreground transition-colors cursor-pointer"
    >
      <BrainHealthBadge compact />
      {text}
    </button>
  );
}
