import { ipc } from "@shared/hooks/useIpc";
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

interface SocraticPanelProps {
  cardId: string;
  userAnswer: string;
  gradeExplanation: string;
}

export function SocraticPanel({ cardId, userAnswer, gradeExplanation }: SocraticPanelProps) {
  const [explanation, setExplanation] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    ipc<{ explanation: string }>("flashcard_explain_answer", {
      cardId,
      userAnswer,
      gradeExplanation,
    })
      .then((res) => {
        if (!cancelled) {
          setExplanation(res.explanation);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "Failed to load explanation");
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [cardId, userAnswer, gradeExplanation]);

  if (loading) {
    return (
      <div className="rounded-lg bg-white/[0.03] border border-accent/20 p-3 flex items-center gap-2">
        <Loader2 size={12} className="animate-spin text-accent" />
        <span className="text-[10px] text-dim">Thinking deeper...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg bg-red-500/10 border border-red-500/20 p-3">
        <p className="text-[10px] text-red-400">{error}</p>
      </div>
    );
  }

  return (
    <div className="rounded-lg bg-white/[0.03] border border-accent/20 p-3">
      <p className="text-[9px] text-accent font-medium mb-1.5">Socratic follow-up</p>
      <p className="text-[10px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
        {explanation}
      </p>
    </div>
  );
}
