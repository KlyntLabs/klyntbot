import { qk, useTauriQuery } from "@/lib/query";
import type { FocusSession } from "../types";

export interface DndActiveResult {
	data: FocusSession | null;
	refetch: () => void;
}

export function useDndActive(): DndActiveResult {
	const query = useTauriQuery<FocusSession | null>({
		queryKey: qk.launcher.dndActive(),
		command: "focus_active",
		args: { mode: "dnd" },
		fallback: null,
	});
	return {
		data: query.data,
		refetch: () => {
			query.refetch();
		},
	};
}
