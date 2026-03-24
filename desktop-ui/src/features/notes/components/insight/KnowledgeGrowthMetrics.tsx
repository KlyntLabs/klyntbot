import { useKnowledgeAtoms } from "../../hooks/useKnowledgeAtoms";

interface KnowledgeGrowthMetricsProps {
  noteId: string | null;
}

export function KnowledgeGrowthMetrics({ noteId }: KnowledgeGrowthMetricsProps) {
  const { activeAtoms, suggestedAtoms } = useKnowledgeAtoms(noteId);
  const active = activeAtoms.length;
  const suggested = suggestedAtoms.length;

  if (active === 0 && suggested === 0) return null;

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-2xs text-muted-foreground bg-accent/30 rounded-md mx-3">
      <span className="font-medium text-foreground">{active}</span> atoms
      {suggested > 0 && (
        <>
          <span>·</span>
          <span className="font-medium text-amber-400">{suggested}</span> suggested
        </>
      )}
    </div>
  );
}
