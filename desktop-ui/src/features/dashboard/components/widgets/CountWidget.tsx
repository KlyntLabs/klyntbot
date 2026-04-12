import { useEntities } from "@features/database/hooks/useEntities";
import type { FilterRule } from "@shared/types";

interface CountWidgetProps {
  databaseId: string;
  title: string;
  filters?: FilterRule[];
}

export function CountWidget({ databaseId, title, filters }: CountWidgetProps) {
  const { data, loading } = useEntities(databaseId, { filters, limit: 0 });

  return (
    <div className="flex flex-col items-center justify-center h-full p-4">
      <span className="text-3xl font-bold tabular-nums">
        {loading ? "..." : (data?.total ?? 0)}
      </span>
      <span className="text-sm text-muted mt-1">{title}</span>
    </div>
  );
}
