import { useMemo, useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import type { ProductivitySummary, ScorePayload } from "../../lib/types";
import { ProductivityScoreRing } from "./ProductivityScoreRing";

interface LiveScoreRingProps {
  summary: ProductivitySummary | null;
}

export function LiveScoreRing({ summary }: LiveScoreRingProps) {
  const [liveScore, setLiveScore] = useState<ScorePayload | null>(null);

  useEvent<ScorePayload>("score:updated", (payload) => {
    setLiveScore(payload);
  });

  const score = liveScore?.score ?? summary?.productivityScore ?? 0;

  const mergedSummary = useMemo(() => {
    if (!summary) return null;
    return {
      ...summary,
      productiveSecs: liveScore?.productiveSecs ?? summary.productiveSecs,
      distractingSecs: liveScore?.distractingSecs ?? summary.distractingSecs,
    };
  }, [summary, liveScore]);

  return <ProductivityScoreRing score={score} summary={mergedSummary} />;
}
