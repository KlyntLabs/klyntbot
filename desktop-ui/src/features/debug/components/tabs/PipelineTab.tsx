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
  ADD: "bg-status-success/20 text-status-success",
  add: "bg-status-success/20 text-status-success",
  UPDATE: "bg-status-info/20 text-status-info",
  update: "bg-status-info/20 text-status-info",
  DELETE: "bg-status-danger/20 text-status-danger",
  delete: "bg-status-danger/20 text-status-danger",
  NOOP: "bg-control-hover text-fg-secondary",
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
          <h2 className="text-ui font-medium text-fg-secondary mb-3 flex items-center gap-1.5">
            <GitBranch className="size-3.5" /> Extraction Log
          </h2>
          <div className="space-y-2">
            {extractions.map((e) => (
              <div
                key={`ext-${e.ts}-${e.observation.slice(0, 20)}`}
                className="p-3 bg-bg-elevated rounded-panel border border-separator"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-ui-xs text-fg-secondary font-mono">
                    {new Date(e.ts).toLocaleTimeString(undefined, {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      hour12: false,
                    })}
                  </span>
                  <span className="text-ui-xs bg-brand/20 text-brand px-1 py-0.5 rounded">
                    {e.factsExtracted} facts
                  </span>
                </div>
                <p className="text-ui-xs text-fg-secondary">{e.observation}</p>
              </div>
            ))}
            {extractions.length === 0 && (
              <p className="text-ui-sm text-fg-secondary text-center py-4">
                No extraction events yet
              </p>
            )}
          </div>
        </div>

        {/* Consolidation Log */}
        <div>
          <h2 className="text-ui font-medium text-fg-secondary mb-3">Consolidation Log</h2>
          <div className="space-y-2">
            {consolidations.map((c) => (
              <div
                key={`con-${c.ts}-${c.fact.slice(0, 20)}`}
                className="p-3 bg-bg-elevated rounded-panel border border-separator"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-ui-xs text-fg-secondary font-mono">
                    {new Date(c.ts).toLocaleTimeString(undefined, {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      hour12: false,
                    })}
                  </span>
                  <span
                    className={`text-ui-xs px-1 py-0.5 rounded ${opColors[c.operation] ?? opColors.NOOP}`}
                  >
                    {c.operation}
                  </span>
                </div>
                <p className="text-ui-xs text-fg-secondary">{c.fact}</p>
              </div>
            ))}
            {consolidations.length === 0 && (
              <p className="text-ui-sm text-fg-secondary text-center py-4">
                No consolidation events yet
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Pipeline stats summary */}
      <div className="flex items-center gap-4 p-3 bg-bg-elevated rounded-panel border border-separator">
        <span className="text-ui-xs text-fg-secondary">
          Extractions: <span className="text-fg-secondary">{extractions.length}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          Consolidations: <span className="text-fg-secondary">{consolidations.length}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          ADDs: <span className="text-status-success">{opCounts.ADD}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          UPDATEs: <span className="text-status-info">{opCounts.UPDATE}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          DELETEs: <span className="text-status-danger">{opCounts.DELETE}</span>
        </span>
      </div>
    </div>
  );
}
