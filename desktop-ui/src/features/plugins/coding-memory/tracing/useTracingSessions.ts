import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SessionSummary } from "./types";

interface State {
  sessions: SessionSummary[];
  loading: boolean;
  error: string | null;
}

export function useTracingSessions(providerId: string, refreshKey: number = 0): State {
  const [state, setState] = useState<State>({ sessions: [], loading: true, error: null });

  useEffect(() => {
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    invoke<SessionSummary[]>("tracing_list_sessions", { providerId })
      .then((rows) => {
        if (cancelled) return;
        setState({ sessions: rows, loading: false, error: null });
      })
      .catch((err) => {
        if (cancelled) return;
        setState({ sessions: [], loading: false, error: String(err?.message ?? err) });
      });
    return () => { cancelled = true; };
  }, [providerId, refreshKey]);

  return state;
}
