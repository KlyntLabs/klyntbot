import { create } from "zustand";

export interface FilterState {
  filters: {
    status: string[];
    assignee: string[];
    priority: string[];
    labels: string[];
    project: string[];
  };
  setFilter: (type: keyof FilterState["filters"], values: string[]) => void;
  toggleFilter: (type: keyof FilterState["filters"], value: string) => void;
  clearFilters: () => void;
  clearFilterType: (type: keyof FilterState["filters"]) => void;
  hasActiveFilters: () => boolean;
  getActiveFiltersCount: () => number;
}

const emptyFilters = {
  status: [],
  assignee: [],
  priority: [],
  labels: [],
  project: [],
};

export const useFilterStore = create<FilterState>()((set, get) => ({
  filters: { ...emptyFilters },

  setFilter: (type, values) =>
    set((state) => ({
      filters: { ...state.filters, [type]: values },
    })),

  toggleFilter: (type, value) =>
    set((state) => {
      const current = state.filters[type];
      const updated = current.includes(value)
        ? current.filter((v) => v !== value)
        : [...current, value];
      return { filters: { ...state.filters, [type]: updated } };
    }),

  clearFilters: () => set({ filters: { ...emptyFilters } }),

  clearFilterType: (type) =>
    set((state) => ({
      filters: { ...state.filters, [type]: [] },
    })),

  hasActiveFilters: () => {
    const { filters } = get();
    return Object.values(filters).some((arr) => arr.length > 0);
  },

  getActiveFiltersCount: () => {
    const { filters } = get();
    return Object.values(filters).reduce((sum, arr) => sum + arr.length, 0);
  },
}));
