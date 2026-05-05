import { useEffect, useState } from "react";
import { fetchEffectivenessTrends } from "@/api/endpoints/codingMemory";

interface Props {
  patternId: string;
}

export function EffectivenessTrendsChart({ patternId }: Props) {
  const [buckets, setBuckets] = useState<Array<{ at: string; score: number }>>([]);
  useEffect(() => {
    fetchEffectivenessTrends(patternId).then((data: unknown) => {
      const trends = data as { buckets?: Array<{ at: string; score: number }> } | undefined;
      if (trends?.buckets) setBuckets(trends.buckets);
    });
  }, [patternId]);

  if (buckets.length === 0) return null;

  const w = 240;
  const h = 60;
  const minScore = Math.min(...buckets.map((b) => b.score));
  const maxScore = Math.max(...buckets.map((b) => b.score));
  const range = maxScore - minScore || 1;

  const points = buckets
    .map((b, i) => {
      const x = (i / (buckets.length - 1 || 1)) * w;
      const y = h - ((b.score - minScore) / range) * h;
      return `${x},${y}`;
    })
    .join(" ");

  return (
    <div className="cm-sparkline">
      <svg viewBox={`0 0 ${w} ${h}`} width={w} height={h} aria-label="Effectiveness trend">
        <title>Effectiveness trend</title>
        <polyline
          fill="none"
          stroke="var(--ds-border-accent-soft)"
          strokeWidth={2}
          points={points}
        />
      </svg>
    </div>
  );
}
