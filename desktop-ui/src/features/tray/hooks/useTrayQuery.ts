import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "@/utils/tauri-bridge";

interface QueryOpts {
	// Re-run the query whenever any of these Tauri events fire.
	invalidateOn?: string[];
}

// Lightweight stand-in for the .bak's @shared/hooks/useQuery. No global cache:
// each caller has its own state. `invalidateOn` is omitted in favour of letting
// the parent component listen for events and call refetch() — this hook keeps
// just the basics the tray actually relies on.
export function useTrayQuery<T>(
	cmd: string,
	args: Record<string, unknown> | undefined,
	initial: T,
	_opts?: QueryOpts,
): { data: T; refetch: () => void; loading: boolean } {
	const [data, setData] = useState<T>(initial);
	const [loading, setLoading] = useState(true);
	const aliveRef = useRef(true);
	const argsKey = JSON.stringify(args ?? null);

	const refetch = useCallback(async () => {
		setLoading(true);
		try {
			const result = await ipc<T>(cmd, args);
			if (aliveRef.current) {
				setData(result ?? initial);
			}
		} catch (err) {
			// Swallow — UI keeps initial state. Helpful in dev where ipc may not
			// be available, or when a backend command isn't implemented yet.
			console.warn(`[useTrayQuery] ${cmd} failed:`, err);
		} finally {
			if (aliveRef.current) setLoading(false);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [cmd, argsKey]);

	useEffect(() => {
		aliveRef.current = true;
		refetch();
		return () => {
			aliveRef.current = false;
		};
	}, [refetch]);

	return { data, refetch, loading };
}
