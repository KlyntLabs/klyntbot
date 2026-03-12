import { create } from "zustand";

interface SearchState {
  isSearchOpen: boolean;
  searchQuery: string;
  toggleSearch: () => void;
  closeSearch: () => void;
  setSearchQuery: (query: string) => void;
}

export const useSearchStore = create<SearchState>()((set) => ({
  isSearchOpen: false,
  searchQuery: "",
  toggleSearch: () => set((state) => ({ isSearchOpen: !state.isSearchOpen })),
  closeSearch: () => set({ isSearchOpen: false, searchQuery: "" }),
  setSearchQuery: (query) => set({ searchQuery: query }),
}));
