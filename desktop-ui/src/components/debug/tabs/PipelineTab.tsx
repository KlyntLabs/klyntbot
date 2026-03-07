import { GitBranch } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useEvent } from "../../../hooks/useEvent";

interface ExtractionEvent {
  observation: string;
  factsExtracted: number;
}

interface ConsolidationEvent {
  operation: string;
  fact: string;
}

interface TimestampedExtraction extends ExtractionEvent {
  ts: string;
}

interface TimestampedConsolidation extends ConsolidationEvent {
  ts: string;
}

const opColors: Record<string, string> = {
  ADD: "bg-green-500/20 text-green-300",
  UPDATE: "bg-blue-500/20 text-blue-300",
  DELETE: "bg-red-500/20 text-red-300",
  NOOP: "bg-white/[0.06] text-muted",
};

export function PipelineTab() {
  const [extractions, setExtractions] = useState<TimestampedExtraction[]>([]);
  const [consolidations, setConsolidations] = useState<TimestampedConsolidation[]>([]);

  useEvent<ExtractionEvent>(
    "cognitive:extraction",
    useCallback((e: ExtractionEvent) => {
      setExtractions((prev) => [{ ...e, ts: new Date().toISOString() }, ...prev].slice(0, 50));
    }, []),
  );

  useEvent<ConsolidationEvent>(
    "cognitive:consolidation",
    useCallback((e: ConsolidationEvent) => {
      setConsolidations((prev) => [{ ...e, ts: new Date().toISOString() }, ...prev].slice(0, 50));
    }, []),
  );

  const opCounts = useMemo(
    () =>
      consolidations.reduce(
        (acc, c) => {
          if (c.operation in acc) acc[c.operation as keyof typeof acc]++;
          return acc;
        },
        { ADD: 0, UPDATE: 0, DELETE: 0 },
      ),
    [consolidations],
  );

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-6">
        {/* Extraction Log */}
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3 flex items-center gap-1.5">
            <GitBranch className="w-3.5 h-3.5" /> Extraction Log
          </h2>
          <div className="space-y-2">
            {extractions.map((e, i) => (
              <div
                key={`ext-${e.ts}-${i}`}
                className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">{e.ts.slice(11, 19)}</span>
                  <span className="text-[10px] bg-brand/20 text-brand px-1 py-0.5 rounded">
                    {e.factsExtracted} facts
                  </span>
                </div>
                <p className="text-[11px] text-secondary">{e.observation}</p>
              </div>
            ))}
            {extractions.length === 0 && (
              <p className="text-[12px] text-muted text-center py-4">
                Waiting for extraction events...
              </p>
            )}
          </div>
        </div>

        {/* Consolidation Log */}
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3">Consolidation Log</h2>
          <div className="space-y-2">
            {consolidations.map((c, i) => (
              <div
                key={`con-${c.ts}-${i}`}
                className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">{c.ts.slice(11, 19)}</span>
                  <span
                    className={`text-[10px] px-1 py-0.5 rounded ${opColors[c.operation] ?? opColors.NOOP}`}
                  >
                    {c.operation}
                  </span>
                </div>
                <p className="text-[11px] text-secondary">{c.fact}</p>
              </div>
            ))}
            {consolidations.length === 0 && (
              <p className="text-[12px] text-muted text-center py-4">
                Waiting for consolidation events...
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Pipeline stats summary */}
      <div className="flex items-center gap-4 p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
        <span className="text-[11px] text-muted">
          Extractions: <span className="text-secondary">{extractions.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          Consolidations: <span className="text-secondary">{consolidations.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          ADDs: <span className="text-green-400">{opCounts.ADD}</span>
        </span>
        <span className="text-[11px] text-muted">
          UPDATEs: <span className="text-blue-400">{opCounts.UPDATE}</span>
        </span>
        <span className="text-[11px] text-muted">
          DELETEs: <span className="text-red-400">{opCounts.DELETE}</span>
        </span>
      </div>
    </div>
  );
}
