import { useState } from "react";

interface MultipleChoiceInputProps {
  correctAnswer: string;
  distractors: string[];
  onSelect: (answer: string) => void;
}

const LABELS = ["A", "B", "C", "D"];

function shuffleOnce(correct: string, distractors: string[]): string[] {
  const all = [correct, ...distractors].slice(0, 4);
  // Fisher-Yates shuffle
  for (let i = all.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [all[i], all[j]] = [all[j], all[i]];
  }
  return all;
}

export function MultipleChoiceInput({
  correctAnswer,
  distractors,
  onSelect,
}: MultipleChoiceInputProps) {
  const [options] = useState<string[]>(() => shuffleOnce(correctAnswer, distractors));
  const [selected, setSelected] = useState<string | null>(null);

  const handleSelect = (option: string) => {
    if (selected !== null) return;
    setSelected(option);
    onSelect(option);
  };

  return (
    <div className="flex flex-col gap-2">
      {options.map((option, idx) => {
        const label = LABELS[idx] ?? String(idx + 1);
        const isSelected = selected === option;

        return (
          <button
            key={option}
            type="button"
            onClick={() => handleSelect(option)}
            disabled={selected !== null}
            className={[
              "flex items-start gap-2.5 w-full text-left px-3 py-2 rounded-lg border text-ui-xs transition-colors",
              isSelected
                ? "border-brand bg-brand/10 text-fg"
                : "border-separator bg-white/[0.03] text-fg-secondary hover:bg-white/[0.06] hover:text-fg",
              selected !== null && !isSelected ? "opacity-50" : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            <span
              className={[
                "shrink-0 size-4 rounded text-[9px] font-semibold flex items-center justify-center",
                isSelected ? "bg-control-hover text-white" : "bg-white/[0.08] text-fg-dim",
              ].join(" ")}
            >
              {label}
            </span>
            <span className="flex-1 whitespace-pre-wrap">{option}</span>
          </button>
        );
      })}
    </div>
  );
}
