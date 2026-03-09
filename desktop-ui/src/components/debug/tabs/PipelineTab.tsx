import { GitBranch } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useEvent } from "../../../hooks/useEvent";
import { ipc } from "../../../hooks/useIpc";

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

/** Shape returned by the `cognitive_pipeline_log` backend command. */
interface PipelineEventRow {
  id: string;
  event_kind: string;
  observation: string | null;
  facts_extracted: number | null;
  operation: string | null;
  fact_triple: string | null;
  timestamp: string;
}

const opColors: Record<string, string> = {
  ADD: "bg-green-500/20 text-green-300",
  add: "bg-green-500/20 text-green-300",
  UPDATE: "bg-blue-500/20 text-blue-300",
  update: "bg-blue-500/20 text-blue-300",
  DELETE: "bg-red-500/20 text-red-300",
  delete: "bg-red-500/20 text-red-300",
  NOOP: "bg-white/[0.06] text-muted",
};

export function PipelineTab() {
  const [extractions, setExtractions] = useState<TimestampedExtraction[]>([]);
  const [consolidations, setConsolidations] = useState<TimestampedConsolidation[]>([]);

  // Load historical pipeline events on mount
  useEffect(() => {
    ipc<PipelineEventRow[]>("cognitive_pipeline_log", { limit: 100 })
      .then((rows) => {
        const ext: TimestampedExtraction[] = [];
        const con: TimestampedConsolidation[] = [];
        for (const r of rows) {
          if (r.event_kind === "extraction") {
            ext.push({
              observation: r.observation ?? "",
              factsExtracted: r.facts_extracted ?? 0,
              ts: r.timestamp,
            });
          } else if (r.event_kind === "consolidation") {
            con.push({
              operation: (r.operation ?? "NOOP").toUpperCase(),
              fact: r.fact_triple ?? "",
              ts: r.timestamp,
            });
          }
        }
        setExtractions(ext);
        setConsolidations(con);
      })
      .catch(() => {
        // Endpoint may not exist on older backends — silently ignore.
      });
  }, []);

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
          const key = c.operation.toUpperCase();
          if (key in acc) acc[key as keyof typeof acc]++;
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
            {extractions.map((e) => (
              <div
                key={`ext-${e.ts}-${e.observation.slice(0, 20)}`}
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
              <p className="text-[12px] text-muted text-center py-4">No extraction events yet</p>
            )}
          </div>
        </div>

        {/* Consolidation Log */}
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3">Consolidation Log</h2>
          <div className="space-y-2">
            {consolidations.map((c) => (
              <div
                key={`con-${c.ts}-${c.fact.slice(0, 20)}`}
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
              <p className="text-[12px] text-muted text-center py-4">No consolidation events yet</p>
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
