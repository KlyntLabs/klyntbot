import { useState } from "react";
export function useDetail() {
  const [expandedSeq, setExpandedSeq] = useState<number | null>(null);
  const [selectedToolId, setSelectedToolId] = useState<string | null>(null);
  const [filterKind, setFilterKind] = useState<string>("");

  const toggleEvent = (seq: number) => {
    setExpandedSeq((prev) => (prev === seq ? null : seq));
  };

  return {
    expandedSeq,
    toggleEvent,
    selectedToolId,
    selectToolId: setSelectedToolId,
    filterKind,
    setFilterKind,
  };
}
