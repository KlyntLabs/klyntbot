import {
	type QueryKey,
	useQuery,
	type UseQueryResult,
} from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";

export interface TauriQueryOptions<TData> {
	queryKey: QueryKey;
	command: string;
	args?: Record<string, unknown>;
	/** Returned as `data` until the first successful fetch. */
	fallback?: TData;
	/** Disable the query (e.g. wait for a prerequisite). */
	enabled?: boolean;
	/** Override the cache stale time for this query. Default 30s (client.ts). */
	staleTime?: number;
}

export function useTauriQuery<TData>(
	opts: TauriQueryOptions<TData>,
): UseQueryResult<TData> & { data: TData } {
	const result = useQuery<TData>({
		queryKey: opts.queryKey,
		queryFn: () => ipc<TData>(opts.command, opts.args),
		enabled: opts.enabled,
		staleTime: opts.staleTime,
		placeholderData: opts.fallback as any,
	});

	return {
		...result,
		// `placeholderData` keeps the fallback as `data` until the query
		// succeeds, so the cast is safe.
		data: (result.data ?? opts.fallback) as TData,
	} as UseQueryResult<TData> & { data: TData };
}
