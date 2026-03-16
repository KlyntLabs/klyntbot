import { create } from "zustand";
import { persist } from "zustand/middleware";

export type ViewType = "list" | "grid";

interface ViewState {
  viewType: ViewType;
  setViewType: (viewType: ViewType) => void;
}

const VALID_VIEW_TYPES: ViewType[] = ["list", "grid"];

export const useViewStore = create<ViewState>()(
  persist(
    (set) => ({
      viewType: "list",
      setViewType: (viewType) => set({ viewType }),
    }),
    {
      name: "tasks-view-storage",
      onRehydrateStorage: () => (state) => {
        if (state && !VALID_VIEW_TYPES.includes(state.viewType)) {
          state.viewType = "list";
        }
      },
    },
  ),
);
