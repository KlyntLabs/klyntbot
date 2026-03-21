import { useQuery } from "@shared/hooks/useQuery";
import { InterventionRow } from "../components/InterventionRow";
import type { InterventionLog } from "../types";

export function HistoryPage() {
  const { data: history, loading } = useQuery<InterventionLog[]>(
    "coaching_intervention_log",
    { limit: 100 },
    [],
  );

  if (loading) {
    return <div className="text-[11px] text-muted-foreground">Loading history...</div>;
  }

  if (!history || history.length === 0) {
    return (
      <div className="flex items-center justify-center h-48">
        <p className="text-[11px] text-muted-foreground">
          No coaching interventions yet. The system will start offering suggestions as it learns
          your patterns.
        </p>
      </div>
    );
  }

  return (
    <div className="glass-card rounded-xl p-5">
        {history.map((h) => (
          <InterventionRow key={h.id} {...h} />
        ))}
    </div>
  );
}
