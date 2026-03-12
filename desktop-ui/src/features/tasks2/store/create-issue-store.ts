import { create } from "zustand";
import type { Status } from "../mock-data/status";

interface CreateIssueState {
  isOpen: boolean;
  defaultStatus: Status | null;
  openModal: (status?: Status) => void;
  closeModal: () => void;
}

export const useCreateIssueStore = create<CreateIssueState>()((set) => ({
  isOpen: false,
  defaultStatus: null,
  openModal: (status) => set({ isOpen: true, defaultStatus: status ?? null }),
  closeModal: () => set({ isOpen: false, defaultStatus: null }),
}));
