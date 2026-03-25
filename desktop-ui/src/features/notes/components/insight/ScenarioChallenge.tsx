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
    <div className="rounded-lg bg-card border border-border-subtle p-4 space-y-3">
      <div className="flex items-center gap-2">
        <BookOpen size={14} className="text-brand" />
        <span className="text-[13px] font-medium text-foreground">{scenario.title}</span>
        <span className="text-[9px] px-1.5 py-0.5 rounded bg-brand/10 text-brand ml-auto">
          Scenario
        </span>
      </div>

      <p className="text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap">
        {scenario.situation}
      </p>

      <div className="space-y-2">
        <span className="text-2xs font-medium text-muted-foreground uppercase tracking-wider">
          Decision Points
        </span>
        {scenario.questions.map((q, i) => (
          <div key={q} className="flex items-start gap-2 text-[11px] text-foreground">
            <span className="text-brand font-medium shrink-0">{i + 1}.</span>
            <span>{q}</span>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={() => setShowAnswer((p) => !p)}
        className="flex items-center gap-1 text-2xs text-brand hover:text-brand/80 transition-colors"
      >
        <ChevronDown
          size={10}
          className={`transition-transform ${showAnswer ? "rotate-180" : ""}`}
        />
        {showAnswer ? "Hide" : "Show"} Model Answer
      </button>

      {showAnswer && (
        <div className="p-3 rounded-md bg-accent border border-border-subtle">
          <p className="text-[11px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
            {scenario.modelAnswer}
          </p>
          {scenario.sourceNotes.length > 0 && (
            <div className="mt-2 flex items-center gap-1 flex-wrap">
              <span className="text-[9px] text-dim">Sources:</span>
              {scenario.sourceNotes.map((note) => (
                <span
                  key={note}
                  className="text-[9px] px-1.5 py-0.5 rounded bg-card text-muted-foreground border border-border-subtle"
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
