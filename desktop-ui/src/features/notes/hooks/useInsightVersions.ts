import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface InsightVersion {
  id: string;
  version: number;
  generatedAt: string;
  inputHash: string;
  hasParent: boolean;
}

interface VersionsState {
  loading: boolean;
  versions: InsightVersion[];
  selectedId: string | null;
}

export function useInsightVersions() {
  const [state, setState] = useState<VersionsState>({
    loading: false,
    versions: [],
    selectedId: null,
  });

  const fetch = useCallback(async (noteId: string) => {
    setState((prev) => ({ ...prev, loading: true }));
    try {
      const versions = await ipc<InsightVersion[]>("note_insight_list_versions", { noteId });
      setState({ loading: false, versions, selectedId: null });
    } catch {
      setState({ loading: false, versions: [], selectedId: null });
    }
  }, []);

  const select = useCallback((id: string | null) => {
    setState((prev) => ({ ...prev, selectedId: id }));
  }, []);

  return { ...state, fetch, select };
}
