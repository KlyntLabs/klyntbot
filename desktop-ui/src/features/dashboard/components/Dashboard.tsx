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
import { DayView } from "./views/DayView";

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
    case "month":
    case "year":
      view = (
        <div className="dashboard__placeholder">
          {state.mode.charAt(0).toUpperCase() + state.mode.slice(1)} view — coming in next phase
        </div>
      );
      break;
  }

  return (
    <DashboardStateContext.Provider value={state}>
      <DataModeContext.Provider value={{ dataMode, setDataMode }}>
        <LayerContext.Provider value={{ enabled, enabledSources, toggle, reset }}>
          <SidebarContext.Provider value={{ sidebarOpen, toggleSidebar }}>
            <div className="dashboard">
              <DashboardTopbar />
              <div className="dashboard__content">{view}</div>
            </div>
          </SidebarContext.Provider>
        </LayerContext.Provider>
      </DataModeContext.Provider>
    </DashboardStateContext.Provider>
  );
}
