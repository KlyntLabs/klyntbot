import { DashboardStateContext, useDashboardStateImpl } from "../hooks/useDashboardState";
import {
  DataModeContext,
  LayerContext,
  SidebarContext,
  useDataMode,
  useLayerToggle,
  useSidebarToggle,
} from "../lib/layers";
import { DashboardTopbar } from "./DashboardTopbar";
import { FocusStateIndicator } from "./productivity/FocusStateIndicator";
import { DayView } from "./views/DayView";
import { MonthView } from "./views/MonthView";
import { WeekView } from "./views/WeekView";
import { YearView } from "./views/YearView";

export function Dashboard() {
  const state = useDashboardStateImpl();
  const { enabled, enabledSources, toggle, reset } = useLayerToggle();
  const { sidebarOpen, toggleSidebar } = useSidebarToggle();
  const { dataMode, setDataMode } = useDataMode();

  let view: React.ReactNode;
  switch (state.mode) {
    case "day":
      view = <DayView />;
      break;
    case "week":
      view = <WeekView />;
      break;
    case "month":
      view = <MonthView />;
      break;
    case "year":
      view = <YearView />;
      break;
  }

  return (
    <DashboardStateContext.Provider value={state}>
      <DataModeContext.Provider value={{ dataMode, setDataMode }}>
        <LayerContext.Provider value={{ enabled, enabledSources, toggle, reset }}>
          <SidebarContext.Provider value={{ sidebarOpen, toggleSidebar }}>
            <div
              className="flex flex-col gap-2 min-w-0 flex-1 h-full overflow-hidden"
              style={{
                background:
                  "linear-gradient(180deg, color-mix(in srgb, var(--surface-messages) 94%, transparent) 0%, color-mix(in srgb, var(--surface-messages) 100%, transparent) 100%)",
              }}
            >
              <h1 className="sr-only">Dashboard</h1>
              <DashboardTopbar />
              <FocusStateIndicator />
              <div className="flex-1 overflow-hidden relative">{view}</div>
            </div>
          </SidebarContext.Provider>
        </LayerContext.Provider>
      </DataModeContext.Provider>
    </DashboardStateContext.Provider>
  );
}
