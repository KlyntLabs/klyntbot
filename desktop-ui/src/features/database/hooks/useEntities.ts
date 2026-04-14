import { DATABASE_UPDATED_EVENT } from "@features/database/lib/schema-utils";
import { useQuery } from "@shared/hooks/useQuery";
import type { QueryParams, QueryResult } from "@shared/types";

export function useEntities(databaseId: string | undefined, params?: QueryParams) {
  return useQuery<QueryResult>(
    "db_query",
    databaseId
      ? {
          databaseId,
          filters: params?.filters,
          filter: params?.filter,
          sorts: params?.sorts,
          limit: params?.limit,
          offset: params?.offset,
        }
      : null,
    { invalidateOn: [DATABASE_UPDATED_EVENT] },
  );
}
