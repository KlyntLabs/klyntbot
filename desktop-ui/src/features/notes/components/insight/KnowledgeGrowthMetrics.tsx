import { ipc } from "@shared/hooks/useIpc";
import { TrendingUp } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

interface KnowledgeGrowth {
  newFactsCount: number;
  updatedFactsCount: number;
  supersededFactsCount: number;
  byDomain: { domain: string; count: number }[];
  periodDays: number;
}

interface Props {
  isOpen: boolean;
}

export function KnowledgeGrowthMetrics({ isOpen }: Props) {
  const [data, setData] = useState<KnowledgeGrowth | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const result = await ipc<KnowledgeGrowth>("note_insight_knowledge_growth", { days: 7 });
      if (result.newFactsCount > 0 || result.updatedFactsCount > 0) {
        setData(result);
      }
    } catch {
      // Supplementary metric — silent fail
    }
  }, []);

  useEffect(() => {
    if (isOpen) fetchData();
  }, [isOpen, fetchData]);

  if (!data) return null;

  return (
    <div className="px-3 py-1.5 border-b border-border flex items-center gap-2 text-[10px] text-dim">
      <TrendingUp size={10} className="text-success shrink-0" />
      <span>
        <span className="text-muted-foreground font-medium">{data.newFactsCount}</span> new facts
      </span>
      {data.updatedFactsCount > 0 && (
        <span>
          ·{" "}
          <span className="text-muted-foreground font-medium">{data.updatedFactsCount}</span>{" "}
          updated
        </span>
      )}
      {data.byDomain.length > 0 && (
        <span className="ml-auto text-[9px]">
          {data.byDomain
            .slice(0, 3)
            .map((d) => d.domain)
            .join(", ")}
        </span>
      )}
    </div>
  );
}
