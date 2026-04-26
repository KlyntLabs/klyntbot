import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "../tauri-bridge";
import type { FocusSession } from "../types";

export interface DndActiveResult {
	data: FocusSession | null;
	refetch: () => void;
}

// Replaces the .bak's useQuery-based hook. 2-second polling matches the original
// stale-time so the FocusActiveChip and DND-related rows stay accurate without
// a global query cache.
export function useDndActive(): DndActiveResult {
	const [data, setData] = useState<FocusSession | null>(null);
	const aliveRef = useRef(true);

	const refetch = useCallback(async () => {
		try {
			const result = await ipc<FocusSession | null>("focus_active", {
				mode: "dnd",
			});
			if (aliveRef.current) setData(result);
		} catch {
			// silent — focus may not be wired in dev
		}
	}, []);

	useEffect(() => {
		aliveRef.current = true;
		refetch();
		const id = setInterval(refetch, 2_000);
		return () => {
			aliveRef.current = false;
			clearInterval(id);
		};
	}, [refetch]);

	return { data, refetch };
}
