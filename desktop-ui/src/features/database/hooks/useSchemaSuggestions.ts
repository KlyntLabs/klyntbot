import { useQuery } from "@shared/hooks/useQuery";
import type { SchemaEvolution } from "@shared/types";

export function useSchemaSuggestions(databaseId: string | undefined) {
  return useQuery<SchemaEvolution[]>(
    "db_get_suggestions",
    databaseId ? { database_id: databaseId } : null,
    { invalidateOn: ["database:updated"], staleTime: 60_000 },
  );
}
