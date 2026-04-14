import { DATABASE_UPDATED_EVENT } from "@features/database/lib/schema-utils";
import { useQuery } from "@shared/hooks/useQuery";
import type { DatabaseSchema } from "@shared/types";

export function useDatabases() {
  return useQuery<DatabaseSchema[]>("db_list", undefined, undefined, {
    invalidateOn: [DATABASE_UPDATED_EVENT],
  });
}

export function useDatabase(databaseId: string | undefined) {
  return useQuery<DatabaseSchema>("db_get_schema", databaseId ? { databaseId } : null, undefined, {
    invalidateOn: [DATABASE_UPDATED_EVENT],
  });
}
