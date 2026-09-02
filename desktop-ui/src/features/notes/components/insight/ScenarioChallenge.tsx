import { BookOpen, ChevronDown } from "lucide-react";
import { useState } from "react";

export interface ScenarioData {
  title: string;
  situation: string;
  questions: string[];
  modelAnswer: string;
  sourceNotes: string[];
  difficultyScore: number;
}

interface Props {
  scenario: ScenarioData;
}

export function ScenarioChallenge({ scenario }: Props) {
  const [showAnswer, setShowAnswer] = useState(false);

  return (
    <div className="island p-4 space-y-3">
      <div className="flex items-center gap-2">
        <BookOpen size={14} className="text-brand" />
        <span className="text-ui font-medium text-fg">{scenario.title}</span>
        <span className="text-[9px] px-1.5 py-0.5 rounded bg-brand/10 text-brand ml-auto">
          Scenario
        </span>
      </div>

      <p className="text-ui-sm text-fg-secondary leading-relaxed whitespace-pre-wrap">
        {scenario.situation}
      </p>

      <div className="space-y-2">
        <span className="text-ui-xs font-medium text-fg-secondary uppercase tracking-wider">
          Decision Points
        </span>
        {scenario.questions.map((q, i) => (
          <div key={q} className="flex items-start gap-2 text-ui-xs text-fg">
            <span className="text-brand font-medium shrink-0">{i + 1}.</span>
            <span>{q}</span>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={() => setShowAnswer((p) => !p)}
        className="flex items-center gap-1 text-ui-xs text-brand hover:text-brand/80 transition-colors"
      >
        <ChevronDown
          size={10}
          className={`transition-transform ${showAnswer ? "rotate-180" : ""}`}
        />
        {showAnswer ? "Hide" : "Show"} Model Answer
      </button>

      {showAnswer && (
        <div className="p-3 rounded-md bg-control-hover border border-separator">
          <p className="text-ui-xs text-fg-secondary leading-relaxed whitespace-pre-wrap">
            {scenario.modelAnswer}
          </p>
          {scenario.sourceNotes.length > 0 && (
            <div className="mt-2 flex items-center gap-1 flex-wrap">
              <span className="text-[9px] text-fg-dim">Sources:</span>
              {scenario.sourceNotes.map((note) => (
                <span
                  key={note}
                  className="text-[9px] px-1.5 py-0.5 rounded bg-bg-elevated text-fg-secondary border border-separator"
                >
                  {note}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
