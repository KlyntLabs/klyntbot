import { useTauriMutation } from "@/lib/query";
import { formatRemaining } from "../lib/formatRemaining";

interface Props {
  endsAt: string;
  onDone: () => void;
}

export function FocusActiveChip({ endsAt, onDone }: Props) {
  const focusExtend = useTauriMutation<void, { mode: "dnd"; newEndsAt: string }>({
    command: "focus_extend",
  });
  const focusDeactivate = useTauriMutation<void, { mode: "dnd" }>({
    command: "focus_deactivate",
  });

  const handleExtend = (extraMs: number) => {
    const base = Math.max(new Date(endsAt).getTime(), Date.now());
    const newEndsAt = new Date(base + extraMs).toISOString();
    focusExtend
      .mutate({ mode: "dnd", newEndsAt })
      .then(() => onDone())
      .catch((err) => console.error("focus_extend failed:", err));
  };

  const handleTurnOff = () => {
    focusDeactivate
      .mutate({ mode: "dnd" })
      .then(() => onDone())
      .catch((err) => console.error("focus_deactivate failed:", err));
  };

  return (
    <div className="lc-arg-bar">
      <span className="lc-muted-sm">DND on — {formatRemaining(endsAt)} left</span>
      <button type="button" className="lc-chip-btn" onClick={() => handleExtend(30 * 60_000)}>
        +30m
      </button>
      <button type="button" className="lc-chip-btn" onClick={() => handleExtend(2 * 3_600_000)}>
        +2h
      </button>
      <button type="button" className="lc-chip-btn is-danger" onClick={handleTurnOff}>
        Turn off
      </button>
    </div>
  );
}
