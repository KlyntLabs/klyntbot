import { createContext, useContext, useMemo, useState } from "react";
import type { TimelineSource } from "../../lib/types";

export type LayerKey = "focus" | "tasks" | "apps" | "events" | "calendar";

export interface LayerConfig {
  key: LayerKey;
  label: string;
  /** TimelineSource values this layer maps to */
  sources: TimelineSource[];
  defaultOn: boolean;
  color: string;
  /** If true, the layer is not yet implemented */
  comingSoon?: boolean;
}

export const LAYERS: LayerConfig[] = [
  {
    key: "focus",
    label: "Focus Sessions",
    sources: ["focus"],
    defaultOn: true,
    color: "var(--timeline-focus)",
  },
  {
    key: "tasks",
    label: "Task Time Entries",
    sources: ["task"],
    defaultOn: true,
    color: "var(--timeline-task)",
  },
  {
    key: "apps",
    label: "App Activity",
    sources: ["productivity"],
    defaultOn: true,
    color: "var(--timeline-app-neutral)",
  },
  {
    key: "events",
    label: "Point Events",
    sources: ["note", "finance", "system"],
    defaultOn: true,
    color: "var(--timeline-note)",
  },
  {
    key: "calendar",
    label: "Calendar Events",
    sources: [],
    defaultOn: false,
    color: "var(--timeline-system)",
    comingSoon: true,
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

  const toggle = (key: LayerKey) => {
    setEnabled((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      localStorage.setItem(STORAGE_KEY, JSON.stringify([...next]));
      return next;
    });
  };

  const reset = () => {
    const defaults = defaultEnabled();
    setEnabled(defaults);
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...defaults]));
  };

  /** Flat list of TimelineSource values for enabled layers, to pass to timeline_query */
  const enabledSources = useMemo(
    () => LAYERS.filter((l) => enabled.has(l.key) && !l.comingSoon).flatMap((l) => l.sources),
    [enabled],
  );

  return { enabled, toggle, reset, enabledSources };
}

export const LayerContext = createContext<{
  enabled: Set<LayerKey>;
  enabledSources: TimelineSource[];
}>({ enabled: new Set(), enabledSources: [] });

export function useEnabledLayers() {
  return useContext(LayerContext);
}

export const SidebarContext = createContext<boolean>(true);

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
