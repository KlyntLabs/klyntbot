import { useEffect, useState } from "react";
import { fetchRecallOverlay } from "@/api/endpoints/codingMemory";

interface Props {
  sessionId: string;
}

export function RecallOverlay({ sessionId }: Props) {
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    fetchRecallOverlay(sessionId).then((data) => {
      if (Array.isArray(data)) setRows(data as Array<Record<string, unknown>>);
    });
  }, [sessionId]);
  if (rows.length === 0) return null;
  return (
    <section className="cm-recall-overlay" aria-label="Recall events">
      <h3 className="cm-recall-overlay__title">Recall ({rows.length})</h3>
      <ul className="cm-recall-overlay__list">
        {rows.map((r) => (
          <li key={JSON.stringify(r)} className="cm-recall-overlay__row">
            <span className="cm-event-chip cm-event-chip--indigo">recall</span>
            <span className="cm-recall-overlay__layer">{String(r.layer ?? "?")}</span>
            <span className="cm-recall-overlay__query">{String(r.query ?? "")}</span>
            {r.coverage_score != null && (
              <span className="cm-recall-overlay__score">
                {Number(r.coverage_score).toFixed(2)}
              </span>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
