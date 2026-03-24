import { useCallback, useState } from "react";
import type { TranslationEvalResponse } from "../../../hooks/useTranslationPractice";
import { useTranslationPractice } from "../../../hooks/useTranslationPractice";

interface PracticeSectionProps {
  sourceText: string;
  sourceLang: string;
  targetLang: string;
}

const GRADE_COLORS: Record<string, string> = {
  A: "text-green-400",
  B: "text-blue-400",
  C: "text-orange-400",
  D: "text-red-400",
  F: "text-red-500",
};

function gradeColor(grade: string): string {
  const letter = grade.charAt(0).toUpperCase();
  return GRADE_COLORS[letter] ?? "text-muted";
}

export function PracticeSection({ sourceText, sourceLang, targetLang }: PracticeSectionProps) {
  const { evaluation, evaluating, error, evaluate, reset } = useTranslationPractice();
  const [input, setInput] = useState("");

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    evaluate(sourceText, input, sourceLang, targetLang);
  }, [input, sourceText, sourceLang, targetLang, evaluate]);

  const handleReset = useCallback(() => {
    setInput("");
    reset();
  }, [reset]);

  return (
    <div className="space-y-3">
      {/* Input area */}
      <div>
        <div className="text-2xs text-muted-foreground uppercase tracking-wider mb-1">
          Your Translation
        </div>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Write your translation here..."
          className="w-full rounded-md border border-border bg-surface-hover/50 px-3 py-2 text-sm text-primary placeholder:text-dim resize-none focus:border-brand focus:outline-none"
          rows={3}
          disabled={evaluating}
        />
        <div className="flex gap-2 mt-2">
          <button
            type="button"
            onClick={handleSubmit}
            disabled={evaluating || !input.trim()}
            className="rounded-md bg-brand px-3 py-1.5 text-xs font-semibold text-black hover:bg-brand/90 disabled:opacity-50"
          >
            {evaluating ? "Evaluating..." : "Check Translation"}
          </button>
          {evaluation && (
            <button
              type="button"
              onClick={handleReset}
              className="rounded-md border border-border px-3 py-1.5 text-xs text-muted-foreground hover:bg-surface-hover"
            >
              Try Again
            </button>
          )}
        </div>
      </div>

      {error && <div className="text-xs text-red-400">{error}</div>}

      {/* Evaluation results (Hybrid C format) */}
      {evaluation && <EvaluationResults evaluation={evaluation} />}
    </div>
  );
}

function EvaluationResults({ evaluation }: { evaluation: TranslationEvalResponse }) {
  const [expandedSet, setExpandedSet] = useState<Set<number>>(new Set());

  const { grades, corrections, modelTranslation } = evaluation;

  return (
    <div className="space-y-3">
      {/* Letter grades bar */}
      <div className="flex gap-2 rounded-md bg-surface-hover p-2">
        <GradeCell label="Meaning" grade={grades.meaning} />
        <GradeCell label="Grammar" grade={grades.grammar} />
        <GradeCell label="Natural" grade={grades.naturalness} />
        <GradeCell label="Choice" grade={grades.wordChoice} />
      </div>

      {/* Corrections */}
      {corrections.length > 0 && (
        <div className="space-y-0.5">
          {corrections.map((c, i) => (
            <div key={`${c.original}-${i}`} className="border-b border-border/50 py-1.5">
              <div className="flex items-center justify-between">
                <div className="text-xs">
                  <span className="text-red-400">✗</span>{" "}
                  <span className="text-muted line-through">{c.original}</span> →{" "}
                  <span className="text-green-400">{c.suggested}</span>
                </div>
                <button
                  type="button"
                  onClick={() =>
                    setExpandedSet((prev) => {
                      const next = new Set(prev);
                      if (next.has(i)) next.delete(i);
                      else next.add(i);
                      return next;
                    })
                  }
                  className="text-2xs text-muted-foreground hover:text-primary"
                >
                  {expandedSet.has(i) ? "▾ Hide" : "▸ Why?"}
                </button>
              </div>
              {expandedSet.has(i) && (
                <div className="mt-1 ml-4 border-l-2 border-blue-500/30 pl-2 text-[11px] text-muted">
                  {c.explanation}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Model translation */}
      <div className="rounded-md bg-surface-hover/50 border border-border/50 p-2">
        <div className="text-2xs text-muted-foreground uppercase tracking-wider mb-1">
          Model Translation
        </div>
        <p className="text-xs text-primary">{modelTranslation}</p>
      </div>
    </div>
  );
}

function GradeCell({ label, grade }: { label: string; grade: string }) {
  return (
    <div className="flex-1 text-center">
      <div className={`text-lg font-bold ${gradeColor(grade)}`}>{grade}</div>
      <div className="text-[9px] text-muted-foreground">{label}</div>
    </div>
  );
}
