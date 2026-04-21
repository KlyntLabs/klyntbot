import { useQuery } from "@shared/hooks/useQuery";
import { useEffect } from "react";
import type { FocusSession } from "../types";

/**
 * Polls focus_active for the DND session. Uses a short 2s stale time and a
 * 2s background refetch so the UI reflects natural expiration and external
 * activations without needing bus-event wiring.
 */
export function useDndActive() {
  const query = useQuery<FocusSession | null>(
    "focus_active",
    { mode: "dnd" },
    { staleTime: 2_000 },
  );

  useEffect(() => {
    const id = setInterval(() => query.refetch(), 2_000);
    return () => clearInterval(id);
  }, [query.refetch]);

  return query;
}
