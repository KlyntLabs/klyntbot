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
            <div className="dashboard">
              <h1 className="sr-only">Dashboard</h1>
              <DashboardTopbar />
              <FocusStateIndicator />
              <div className="dashboard__content">{view}</div>
            </div>
          </SidebarContext.Provider>
        </LayerContext.Provider>
      </DataModeContext.Provider>
    </DashboardStateContext.Provider>
  );
}
