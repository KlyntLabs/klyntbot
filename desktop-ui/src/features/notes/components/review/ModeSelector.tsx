import type { AnswerMode } from "@shared/types/notes";

interface ModeSelectorProps {
  current: AnswerMode;
  onChange: (mode: AnswerMode) => void;
}

const MODES: { mode: AnswerMode; label: string }[] = [
  { mode: "typed", label: "Typed" },
  { mode: "self_grade", label: "Self-grade" },
  { mode: "multiple_choice", label: "Multiple choice" },
  { mode: "voice", label: "Voice" },
];

export function ModeSelector({ current, onChange }: ModeSelectorProps) {
  return (
    <div className="flex items-center gap-1 flex-wrap">
      {MODES.map(({ mode, label }) => {
        const isActive = current === mode || (current === "auto" && mode === "typed");
        return (
          <button
            key={mode}
            type="button"
            onClick={() => onChange(mode)}
            className={[
              "text-[9px] px-2 py-0.5 rounded-full border transition-colors",
              isActive
                ? "border-brand bg-brand/15 text-brand"
                : "border-separator bg-white/[0.03] text-fg-dim hover:text-fg-secondary hover:bg-white/[0.06]",
            ].join(" ")}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
