import { useQuery } from "@shared/hooks/useQuery";
import { PatternCard } from "../components/PatternCard";
import type { DetectedPattern } from "../types";

export function PatternsPage() {
  const { data: patterns, loading } = useQuery<DetectedPattern[]>(
    "coaching_patterns",
    undefined,
    [],
  );

  if (loading) {
    return <div className="text-[11px] text-muted-foreground">Loading patterns...</div>;
  }

  if (!patterns || patterns.length === 0) {
    return (
      <div className="flex items-center justify-center h-48">
        <p className="text-[11px] text-muted-foreground">
          No patterns detected yet. Patterns emerge as the coaching system observes your work habits
          over time.
        </p>
      </div>
    );
  }

  const sorted = [...patterns].sort((a, b) => b.confidence - a.confidence);

  return (
    <div className="grid grid-cols-2 gap-3">
      {sorted.map((p) => (
        <PatternCard key={p.name} {...p} />
      ))}
    </div>
  );
}
