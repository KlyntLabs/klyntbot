import { ipc } from "@shared/hooks/useIpc";
import { useEffect } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { DashboardData } from "../types";

export function useDashboardData() {
  const setDashboard = useLauncherStore((s) => s.setDashboard);
  const mode = useLauncherStore((s) => s.mode);

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
