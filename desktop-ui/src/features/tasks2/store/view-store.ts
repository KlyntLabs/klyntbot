import { create } from "zustand";
import { persist } from "zustand/middleware";

export type ViewType = "list" | "grid";

interface ViewState {
  viewType: ViewType;
  setViewType: (viewType: ViewType) => void;
}

export const useViewStore = create<ViewState>()(
  persist(
    (set) => ({
      viewType: "list",
      setViewType: (viewType) => set({ viewType }),
    }),
    {
      name: "tasks2-view-storage",
    },
  ),
);
