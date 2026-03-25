import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useState } from "react";
import type { WordBreakdown } from "../../../hooks/useLanguageBreakdown";
import { CollapsibleSection } from "./CollapsibleSection";

interface ConfusableAlert {
  word: string;
  confusableWord: string;
  confusableMeaning: string;
  explanation: string | null;
}

interface ConfusableResponse {
  hasConfusable: boolean;
  confusableWord: string | null;
  confusableMeaning: string | null;
  explanation: string | null;
}

interface ConfusableSectionProps {
  words: WordBreakdown[];
  sourceLang: string;
}

export function ConfusableSection({ words, sourceLang }: ConfusableSectionProps) {
  const [alerts, setAlerts] = useState<ConfusableAlert[]>([]);

  useEffect(() => {
    const newWords = words.filter((w) => w.isNew);
    if (newWords.length === 0) {
      setAlerts([]);
      return;
    }

    let cancelled = false;

    // Check each new word for confusables (sequentially to avoid thundering herd)
    async function checkWords() {
      const found: ConfusableAlert[] = [];
      for (const w of newWords) {
        if (cancelled) break;
        try {
          const resp = await ipc<ConfusableResponse>("language_detect_confusables", {
            params: { word: w.word, sourceLang },
          });
          if (resp.hasConfusable && resp.confusableWord) {
            found.push({
              word: w.word,
              confusableWord: resp.confusableWord,
              confusableMeaning: resp.confusableMeaning ?? "",
              explanation: resp.explanation,
            });
          }
        } catch {
          // Skip failed checks
        }
      }
      if (!cancelled) setAlerts(found);
    }

    checkWords();
    return () => {
      cancelled = true;
    };
  }, [words, sourceLang]);

  if (alerts.length === 0) return null;

  return (
    <CollapsibleSection title={`Confusable Words (${alerts.length})`}>
      <div className="space-y-2">
        {alerts.map((a) => (
          <div
            key={`${a.word}-${a.confusableWord}`}
            className="rounded-md border border-amber-500/20 bg-amber-500/5 p-2"
          >
            <div className="flex items-center gap-2 mb-1">
              <span className="text-amber-400 text-xs">⚠</span>
              <span className="text-xs text-primary font-medium">
                {a.word} vs {a.confusableWord}
              </span>
              <span className="text-2xs text-muted">({a.confusableMeaning})</span>
            </div>
            {a.explanation && <p className="text-[11px] text-muted ml-5">{a.explanation}</p>}
          </div>
        ))}
      </div>
    </CollapsibleSection>
  );
}
