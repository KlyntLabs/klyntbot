import { useEffect } from "react";
import { useLauncherApi, useLauncherState } from "../store";
import { ipc } from "../tauri-bridge";
import type { DashboardData } from "../types";

export function useDashboardData() {
	const setDashboard = useLauncherApi().setDashboard;
	const mode = useLauncherState((s) => s.mode);

	useEffect(() => {
		if (mode !== "dashboard") return;
		const fetchDashboard = async () => {
			try {
				const data = await ipc<DashboardData>("launcher_dashboard");
				setDashboard(data);
			} catch (e) {
				console.error("Dashboard fetch failed:", e);
			}
		};
		fetchDashboard();
		const interval = setInterval(fetchDashboard, 30000);
		return () => clearInterval(interval);
	}, [mode, setDashboard]);
}
