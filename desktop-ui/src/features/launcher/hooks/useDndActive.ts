import { useQuery } from "@shared/hooks/useQuery";
import type { FocusSession } from "../types";

/** Polls focus_active for the DND session. 30s stale-time matches shared default. */
export function useDndActive() {
  return useQuery<FocusSession | null>("focus_active", { mode: "dnd" }, null);
}
