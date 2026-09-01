import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { TimelineSource } from "@/bindings";

export type LayerKey = "activity" | "calendar" | "timeEntries" | "tasks" | "transactions" | "notes";

export interface LayerConfig {
  key: LayerKey;
  label: string;
  /** TimelineSource values this layer maps to */
  sources: TimelineSource[];
  defaultOn: boolean;
  color: string;
}

export const LAYERS: LayerConfig[] = [
  {
    key: "activity",
    label: "Activity",
    sources: ["productivity", "focus"],
    defaultOn: true,
    color: "var(--timeline-app-productive)",
  },
  {
    key: "calendar",
    label: "Calendar",
    sources: ["calendar"],
    defaultOn: true,
    color: "var(--timeline-calendar)",
  },
  {
    key: "timeEntries",
    label: "Time Entries",
    sources: ["task"],
    defaultOn: false,
    color: "var(--timeline-task)",
  },
  {
    key: "tasks",
    label: "Tasks",
    sources: ["todo"],
    defaultOn: true,
    color: "var(--timeline-todo)",
  },
  {
    key: "transactions",
    label: "Transactions",
    sources: ["finance"],
    defaultOn: true,
    color: "var(--timeline-finance)",
  },
  {
    key: "notes",
    label: "Notes",
    sources: ["note"],
    defaultOn: true,
    color: "var(--timeline-note)",
  },
];

const STORAGE_KEY = "dashboard-layers";

function defaultEnabled(): Set<LayerKey> {
  return new Set(LAYERS.filter((l) => l.defaultOn).map((l) => l.key));
}

function loadEnabled(): Set<LayerKey> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultEnabled();
    const arr = JSON.parse(raw) as LayerKey[];
    return new Set(arr);
  } catch {
    return defaultEnabled();
  }
}

export function useLayerToggle() {
  const [enabled, setEnabled] = useState<Set<LayerKey>>(loadEnabled);

  const toggle = useCallback((key: LayerKey) => {
    setEnabled((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      localStorage.setItem(STORAGE_KEY, JSON.stringify([...next]));
      return next;
    });
  }, []);

  const reset = useCallback(() => {
    const defaults = defaultEnabled();
    setEnabled(defaults);
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...defaults]));
  }, []);

  /** Flat list of TimelineSource values for enabled layers, to pass to timeline_query */
  const enabledSources = useMemo(
    () => LAYERS.filter((l) => enabled.has(l.key)).flatMap((l) => l.sources),
    [enabled],
  );

  return { enabled, toggle, reset, enabledSources };
}

export interface LayerContextValue {
  enabled: Set<LayerKey>;
  enabledSources: TimelineSource[];
  toggle: (key: LayerKey) => void;
  reset: () => void;
}

export const LayerContext = createContext<LayerContextValue>({
  enabled: new Set(),
  enabledSources: [],
  toggle: () => {},
  reset: () => {},
});

export function useEnabledLayers() {
  return useContext(LayerContext);
}

export interface SidebarContextValue {
  sidebarOpen: boolean;
  toggleSidebar: () => void;
}

export const SidebarContext = createContext<SidebarContextValue>({
  sidebarOpen: true,
  toggleSidebar: () => {},
});

export function useSidebarOpen() {
  return useContext(SidebarContext);
}

export function useSidebarToggle() {
  const [open, setOpen] = useState(() => {
    try {
      return localStorage.getItem("dashboard-sidebar") !== "closed";
    } catch {
      return true;
    }
  });

  const toggle = () => {
    setOpen((prev) => {
      const next = !prev;
      localStorage.setItem("dashboard-sidebar", next ? "open" : "closed");
      return next;
    });
  };

  return { sidebarOpen: open, toggleSidebar: toggle };
}

// ── Data Mode ────────────────────────────────────────────────
export type DataMode = "productivity" | "contexts";

const DATA_MODE_KEY = "dashboard-data-mode";

export function useDataMode() {
  const [mode, setMode] = useState<DataMode>(() => {
    try {
      return (localStorage.getItem(DATA_MODE_KEY) as DataMode) || "productivity";
    } catch {
      return "productivity";
    }
  });

  const setDataMode = useCallback((next: DataMode) => {
    setMode(next);
    localStorage.setItem(DATA_MODE_KEY, next);
  }, []);

  return { dataMode: mode, setDataMode };
}

export interface DataModeContextValue {
  dataMode: DataMode;
  setDataMode: (next: DataMode) => void;
}

export const DataModeContext = createContext<DataModeContextValue>({
  dataMode: "productivity",
  setDataMode: () => {},
});

export function useDataModeValue() {
  return useContext(DataModeContext);
}
