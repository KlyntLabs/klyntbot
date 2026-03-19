import { create } from "zustand";

interface ProjectDetailState {
  expandedObjectives: Set<string>;
  expandedKrs: Set<string>;
  toggleObjective: (id: string) => void;
  toggleKr: (id: string) => void;
}

export const useProjectDetailStore = create<ProjectDetailState>((set) => ({
  expandedObjectives: new Set(),
  expandedKrs: new Set(),
  toggleObjective: (id) =>
    set((s) => {
      const next = new Set(s.expandedObjectives);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { expandedObjectives: next };
    }),
  toggleKr: (id) =>
    set((s) => {
      const next = new Set(s.expandedKrs);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { expandedKrs: next };
    }),
}));
