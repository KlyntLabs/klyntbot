import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { GitBranch } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

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
  ADD: "bg-success/20 text-success",
  add: "bg-success/20 text-success",
  UPDATE: "bg-info/20 text-info",
  update: "bg-info/20 text-info",
  DELETE: "bg-destructive/20 text-destructive",
  delete: "bg-destructive/20 text-destructive",
  NOOP: "bg-surface-base text-muted",
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
                className="p-3 bg-surface-low rounded-lg border border-border"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">
                    {new Date(e.ts).toLocaleTimeString(undefined, {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      hour12: false,
                    })}
                  </span>
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
                className="p-3 bg-surface-low rounded-lg border border-border"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] text-muted font-mono">
                    {new Date(c.ts).toLocaleTimeString(undefined, {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      hour12: false,
                    })}
                  </span>
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
      <div className="flex items-center gap-4 p-3 bg-surface-low rounded-lg border border-border">
        <span className="text-[11px] text-muted">
          Extractions: <span className="text-secondary">{extractions.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          Consolidations: <span className="text-secondary">{consolidations.length}</span>
        </span>
        <span className="text-[11px] text-muted">
          ADDs: <span className="text-success">{opCounts.ADD}</span>
        </span>
        <span className="text-[11px] text-muted">
          UPDATEs: <span className="text-info">{opCounts.UPDATE}</span>
        </span>
        <span className="text-[11px] text-muted">
          DELETEs: <span className="text-destructive">{opCounts.DELETE}</span>
        </span>
      </div>
    </div>
  );
}
