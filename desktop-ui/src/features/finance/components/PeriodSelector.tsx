import type { PeriodMode } from "../hooks/usePeriodState";
import { ChevronLeft, ChevronRight } from "lucide-react";

const MODES: PeriodMode[] = ["day", "week", "month", "year"];

export function PeriodSelector({
  label,
  mode,
  onPrev,
  onNext,
  onSetMode,
}: {
  label: string;
  mode: PeriodMode;
  onPrev: () => void;
  onNext: () => void;
  onSetMode: (mode: PeriodMode) => void;
}) {
  return (
    <div className="flex items-center justify-between mb-4">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onPrev}
          aria-label="Previous period"
          className="p-1.5 rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
        >
          <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
        </button>
        <span className="text-[14px] font-medium text-primary min-w-[160px] text-center">
          {label}
        </span>
        <button
          type="button"
          onClick={onNext}
          aria-label="Next period"
          className="p-1.5 rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
        >
          <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
        </button>
      </div>
      <div
        className="flex gap-1 bg-white/[0.04] rounded-xl p-1 border border-white/[0.06]"
        role="tablist"
      >
        {MODES.map((m) => (
          <button
            key={m}
            type="button"
            role="tab"
            aria-selected={m === mode}
            onClick={() => onSetMode(m)}
            className={`px-3 py-1.5 rounded-lg text-[11px] font-light transition-all capitalize ${
              m === mode
                ? "glass-button-active text-primary"
                : "text-muted hover:text-secondary"
            }`}
          >
            {m}
          </button>
        ))}
      </div>
    </div>
  );
}
