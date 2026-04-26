import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { formatRemaining } from "../lib/formatRemaining";

interface Props {
  endsAt: string;
  onDone: () => void;
}

export function FocusActiveChip({ endsAt, onDone }: Props) {
  const handleExtend = (extraMs: number) => {
    const base = Math.max(new Date(endsAt).getTime(), Date.now());
    const newEndsAt = new Date(base + extraMs).toISOString();
    ipc("focus_extend", { mode: "dnd", newEndsAt })
      .then(() => {
        invalidateQueries("focus_active");
        onDone();
      })
      .catch((err) => console.error("focus_extend failed:", err));
  };

  const handleTurnOff = () => {
    ipc("focus_deactivate", { mode: "dnd" })
      .then(() => {
        invalidateQueries("focus_active");
        onDone();
      })
      .catch((err) => console.error("focus_deactivate failed:", err));
  };

  return (
    <div className="flex items-center gap-2 p-2">
      <span className="text-xs text-muted-foreground shrink-0">
        DND on — {formatRemaining(endsAt)} left
      </span>
      <button
        type="button"
        className="px-2 py-1 rounded bg-surface-base border border-border text-xs text-fg hover:bg-muted transition-colors"
        onClick={() => handleExtend(30 * 60_000)}
      >
        +30m
      </button>
      <button
        type="button"
        className="px-2 py-1 rounded bg-surface-base border border-border text-xs text-fg hover:bg-muted transition-colors"
        onClick={() => handleExtend(2 * 3_600_000)}
      >
        +2h
      </button>
      <button
        type="button"
        className="px-2 py-1 rounded bg-surface-base border border-border text-xs text-destructive hover:bg-muted transition-colors"
        onClick={handleTurnOff}
      >
        Turn off
      </button>
    </div>
  );
}
