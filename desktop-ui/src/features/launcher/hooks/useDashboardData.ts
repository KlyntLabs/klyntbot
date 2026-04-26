import { qk, useTauriQuery } from "@/lib/query";
import { useLauncherState } from "../store";
import type { DashboardData } from "../types";

export function useDashboardData() {
	const mode = useLauncherState((s) => s.mode);
	return useTauriQuery<DashboardData | null>({
		queryKey: qk.launcher.dashboard(),
		command: "launcher_dashboard",
		fallback: null,
		enabled: mode === "dashboard",
	});
}
