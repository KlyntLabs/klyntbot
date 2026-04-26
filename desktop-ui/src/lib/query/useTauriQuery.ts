import { type QueryKey, type UseQueryResult, useQuery } from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";

export interface TauriQueryOptions<TData> {
  queryKey: QueryKey;
  /** Tauri command name. Mutually exclusive with `queryFn`. */
  command?: string;
  args?: Record<string, unknown>;
  /**
   * Custom fetch function. Use this when the data source is a typed wrapper
   * (e.g. `@services/tauri`'s `getGitStatus(workspaceId)`) rather than a
   * string-named ipc command. Mutually exclusive with `command`.
   */
  queryFn?: () => Promise<TData>;
  /** Returned as `data` until the first successful fetch. */
  fallback?: TData;
  enabled?: boolean;
  staleTime?: number;
}

export function useTauriQuery<TData>(
  opts: TauriQueryOptions<TData>,
): UseQueryResult<TData> & { data: TData } {
  if (!opts.command && !opts.queryFn) {
    throw new Error("useTauriQuery: either `command` or `queryFn` must be provided");
  }

  const result = useQuery<TData>({
    queryKey: opts.queryKey,
    queryFn: opts.queryFn ?? (() => ipc<TData>(opts.command!, opts.args)),
    enabled: opts.enabled,
    staleTime: opts.staleTime,
    placeholderData: opts.fallback as never,
  });

  return {
    ...result,
    data: (result.data ?? opts.fallback) as TData,
  } as UseQueryResult<TData> & { data: TData };
}
