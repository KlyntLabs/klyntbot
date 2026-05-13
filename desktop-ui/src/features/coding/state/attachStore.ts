import { create } from "zustand";

interface ActiveAttach {
  jobId: string;
}

interface AttachState {
  activeAttach: ActiveAttach | null;
  setActiveAttach: (a: ActiveAttach | null) => void;
}

export const useAttachStore = create<AttachState>((set) => ({
  activeAttach: null,
  setActiveAttach: (a) => set({ activeAttach: a }),
}));
