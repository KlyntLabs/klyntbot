import { create } from "zustand";
import type { DashboardData, LauncherItem, LauncherMode } from "../types";

interface LauncherState {
  mode: LauncherMode;
  query: string;
  results: LauncherItem[];
  selectedIndex: number;
  isSearching: boolean;
  dashboard: DashboardData | null;
  queryHistory: string[];
  historyIndex: number;
  detailItem: LauncherItem | null;
  actionMenuOpen: boolean;
  argModeItem: LauncherItem | null;

  setMode: (mode: LauncherMode) => void;
  setQuery: (query: string) => void;
  setResults: (results: LauncherItem[]) => void;
  setSelectedIndex: (index: number) => void;
  setIsSearching: (loading: boolean) => void;
  setDashboard: (data: DashboardData) => void;
  moveSelection: (delta: number) => void;
  pushHistory: (query: string) => void;
  navigateHistory: (direction: "up" | "down") => void;
  setDetailItem: (item: LauncherItem | null) => void;
  setActionMenuOpen: (open: boolean) => void;
  setArgModeItem: (item: LauncherItem | null) => void;
  reset: () => void;
}

export const useLauncherStore = create<LauncherState>((set, get) => ({
  mode: "dashboard",
  query: "",
  results: [],
  selectedIndex: 0,
  isSearching: false,
  dashboard: null,
  queryHistory: [],
  historyIndex: -1,
  detailItem: null,
  actionMenuOpen: false,
  argModeItem: null,

  setMode: (mode) => set({ mode }),
  setQuery: (query) => {
    const mode = query.length > 0 ? "search" : "dashboard";
    set({ query, mode, selectedIndex: 0, historyIndex: -1 });
  },
  setResults: (results) => set({ results, isSearching: false }),
  setSelectedIndex: (index) => set({ selectedIndex: index }),
  setIsSearching: (isSearching) => set({ isSearching }),
  setDashboard: (data) => set({ dashboard: data }),
  moveSelection: (delta) => {
    const { results, selectedIndex } = get();
    const next = Math.max(0, Math.min(results.length - 1, selectedIndex + delta));
    set({ selectedIndex: next });
  },
  pushHistory: (query) => {
    if (!query.trim()) return;
    const { queryHistory } = get();
    const filtered = queryHistory.filter((q) => q !== query);
    set({ queryHistory: [query, ...filtered].slice(0, 50) });
  },
  setDetailItem: (item) => set({ detailItem: item }),
  setActionMenuOpen: (open) => set({ actionMenuOpen: open }),
  setArgModeItem: (item) => set({ argModeItem: item }),
  navigateHistory: (direction) => {
    const { queryHistory, historyIndex } = get();
    if (queryHistory.length === 0) return;
    const next =
      direction === "up"
        ? Math.min(historyIndex + 1, queryHistory.length - 1)
        : Math.max(-1, historyIndex - 1);
    if (next === -1) {
      set({ historyIndex: -1, query: "" });
    } else {
      set({ historyIndex: next, query: queryHistory[next] });
    }
  },
  reset: () =>
    set({
      mode: "dashboard",
      query: "",
      results: [],
      selectedIndex: 0,
      isSearching: false,
      historyIndex: -1,
      detailItem: null,
      actionMenuOpen: false,
      argModeItem: null,
    }),
}));
