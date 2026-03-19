import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";

interface AmbientIndicatorProps {
  onClick?: () => void;
}

export function AmbientIndicator({ onClick }: AmbientIndicatorProps) {
  const { data: status } = useAutoTunerStatus();

  if (!status?.enabled) return null;
  if (!status.champion.impact) return null;

  return (
    <button
      type="button"
      onClick={onClick}
      className="text-[10px] font-light text-dim hover:text-muted-foreground transition-colors cursor-pointer"
    >
      Getting to know you better &mdash; {status.champion.impact}
    </button>
  );
}
