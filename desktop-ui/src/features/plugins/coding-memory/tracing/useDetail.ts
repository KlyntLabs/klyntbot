import { useCallback, useState } from "react";
export function useDetail() {
  const [expandedSeq, setExpandedSeq] = useState<number | null>(null);
  const [selectedToolId, setSelectedToolId] = useState<string | null>(null);
  const [filterKind, setFilterKind] = useState<string>("");

  const toggleEvent = useCallback((seq: number) => {
    setExpandedSeq((prev) => (prev === seq ? null : seq));
  }, []);

  const selectToolId = useCallback((id: string | null) => {
    setSelectedToolId(id);
  }, []);

  return {
    expandedSeq,
    toggleEvent,
    selectedToolId,
    selectToolId,
    filterKind,
    setFilterKind,
  };
}
