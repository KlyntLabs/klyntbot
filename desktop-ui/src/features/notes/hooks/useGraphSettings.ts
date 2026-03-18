import { useCallback, useState } from "react";

export interface GraphSettings {
  /** Link distance between connected nodes (px) */
  linkDistance: number;
  /** Node repulsion strength (higher = more spread) */
  repulsion: number;
  /** Center gravity (higher = tighter cluster) */
  centerForce: number;
  /** Node size multiplier (1 = default) */
  nodeScale: number;
  /** Zoom level below which labels hide */
  labelThreshold: number;
  /** Show directional arrows on edges */
  showArrows: boolean;
  /** Show orphan (unlinked) nodes */
  showOrphans: boolean;
  /** Enable continuous physics simulation (CPU-intensive) */
  livePhysics: boolean;
  /** Skip progressive reveal animation on graph load */
  instantLoad: boolean;
}

const DEFAULT_SETTINGS: GraphSettings = {
  linkDistance: 120,
  repulsion: 8000,
  centerForce: 0.2,
  nodeScale: 1,
  labelThreshold: 0.5,
  showArrows: true,
  showOrphans: true,
  livePhysics: false,
  instantLoad: false,
};

const STORAGE_KEY = "klynt-graph-settings";

function loadSettings(): GraphSettings {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return { ...DEFAULT_SETTINGS, ...JSON.parse(stored) };
  } catch {
    // ignore
  }
  return DEFAULT_SETTINGS;
}

function saveSettings(settings: GraphSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}

export function useGraphSettings() {
  const [settings, setSettingsState] = useState<GraphSettings>(loadSettings);

  const setSettings = useCallback((partial: Partial<GraphSettings>) => {
    setSettingsState((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  const resetSettings = useCallback(() => {
    setSettingsState(DEFAULT_SETTINGS);
    saveSettings(DEFAULT_SETTINGS);
  }, []);

  return { settings, setSettings, resetSettings, defaults: DEFAULT_SETTINGS };
}
