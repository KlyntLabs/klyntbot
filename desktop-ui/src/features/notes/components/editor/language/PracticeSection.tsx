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
  return GRADE_COLORS[letter] ?? "text-fg-secondary";
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
        <div className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-1">
          Your Translation
        </div>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Write your translation here..."
          className="w-full rounded-md border border-separator bg-control-hover/50 px-3 py-2 text-sm text-brand placeholder:text-fg-dim resize-none focus:border-fg-secondary/40 focus:outline-none"
          rows={3}
          disabled={evaluating}
        />
        <div className="flex gap-2 mt-2">
          <button
            type="button"
            onClick={handleSubmit}
            disabled={evaluating || !input.trim()}
            className="rounded-md bg-brand px-3 py-1.5 text-ui-sm font-semibold text-black hover:bg-brand/90 disabled:opacity-50"
          >
            {evaluating ? "Evaluating..." : "Check Translation"}
          </button>
          {evaluation && (
            <button
              type="button"
              onClick={handleReset}
              className="rounded-md border border-separator px-3 py-1.5 text-ui-sm text-fg-secondary hover:bg-control-hover"
            >
              Try Again
            </button>
          )}
        </div>
      </div>

      {error && <div className="text-ui-sm text-red-400">{error}</div>}

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
      <div className="flex gap-2 rounded-md bg-control-hover p-2">
        <GradeCell label="Meaning" grade={grades.meaning} />
        <GradeCell label="Grammar" grade={grades.grammar} />
        <GradeCell label="Natural" grade={grades.naturalness} />
        <GradeCell label="Choice" grade={grades.wordChoice} />
      </div>

      {/* Corrections */}
      {corrections.length > 0 && (
        <div className="space-y-0.5">
          {corrections.map((c, i) => (
            <div key={`${c.original}-${c.suggested}`} className="border-b border-separator/50 py-1.5">
              <div className="flex items-center justify-between">
                <div className="text-ui-sm">
                  <span className="text-red-400">✗</span>{" "}
                  <span className="text-fg-secondary line-through">{c.original}</span> →{" "}
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
                  className="text-ui-xs text-fg-secondary hover:text-brand"
                >
                  {expandedSet.has(i) ? "▾ Hide" : "▸ Why?"}
                </button>
              </div>
              {expandedSet.has(i) && (
                <div className="mt-1 ml-4 border-l-2 border-blue-500/30 pl-2 text-ui-xs text-fg-secondary">
                  {c.explanation}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Model translation */}
      <div className="rounded-md bg-control-hover/50 border border-separator/50 p-2">
        <div className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-1">
          Model Translation
        </div>
        <p className="text-ui-sm text-brand">{modelTranslation}</p>
      </div>
    </div>
  );
}

function GradeCell({ label, grade }: { label: string; grade: string }) {
  return (
    <div className="flex-1 text-center">
      <div className={`text-lg font-bold ${gradeColor(grade)}`}>{grade}</div>
      <div className="text-[9px] text-fg-secondary">{label}</div>
    </div>
  );
}
