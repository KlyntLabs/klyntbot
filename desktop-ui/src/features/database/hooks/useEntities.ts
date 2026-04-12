import { DATABASE_UPDATED_EVENT } from "@features/database/lib/schema-utils";
import { useQuery } from "@shared/hooks/useQuery";
import type { QueryParams, QueryResult } from "@shared/types";

export function useEntities(databaseId: string | undefined, params?: QueryParams) {
  return useQuery<QueryResult>(
    "db_query",
    databaseId ? { database_id: databaseId, params: params ?? {} } : null,
    { invalidateOn: [DATABASE_UPDATED_EVENT] },
  );
}
