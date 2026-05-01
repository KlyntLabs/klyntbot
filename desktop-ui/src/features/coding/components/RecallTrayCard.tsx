import { useState } from "react";

export type RecallSnippet = {
  kind: string;
  summary: string;
  source: string;
};

export function RecallTrayCard({
  memoryIds,
  coverageScore,
  snippets,
}: {
  memoryIds: string[];
  coverageScore: number;
  snippets: RecallSnippet[];
}) {
  const [expanded, setExpanded] = useState(false);
  const pct = Math.round(coverageScore * 100);
  return (
    <div className="recall-tray-card">
      <button
        type="button"
        className="recall-tray-card__header"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <span className="recall-tray-card__icon">★</span>
        <span>
          {snippets.length} snippet{snippets.length === 1 ? "" : "s"} injected
        </span>
        <span className="recall-tray-card__coverage">{pct}% coverage</span>
      </button>
      {expanded && (
        <ul className="recall-tray-card__list">
          {snippets.map((s, i) => (
            <li key={memoryIds[i] ?? i}>
              <span className="recall-tray-card__kind">{s.kind}</span>
              <span className="recall-tray-card__summary">{s.summary}</span>
              <span className="recall-tray-card__source">{s.source}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
