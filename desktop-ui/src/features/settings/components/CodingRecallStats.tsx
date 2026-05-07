import { useEffect, useState } from "react";
import { fetchCodingRecallStats } from "@/api/endpoints/recall";
import type { RecallStats } from "@/bindings";

type Props = { workspaceId: string };

export function CodingRecallStats({ workspaceId }: Props) {
  const [stats, setStats] = useState<RecallStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    fetchCodingRecallStats(workspaceId, 7)
      .then(setStats)
      .finally(() => setLoading(false));
  }, [workspaceId]);

  if (loading) return <div className="recall-stats recall-stats--loading">Loading…</div>;
  if (!stats) return <div className="recall-stats recall-stats--empty">No recall data</div>;

  return (
    <section className="recall-stats" aria-label="Recall stats">
      <header>
        <h3>Recall — last {stats.daysWindow} days</h3>
      </header>
      <dl className="recall-stats__summary">
        <dt>Invocations</dt>
        <dd>{stats.totalInvocations}</dd>
        <dt>Mean latency</dt>
        <dd>{stats.meanLatencyMs.toFixed(1)} ms</dd>
      </dl>
      {stats.topFacts.length > 0 && (
        <>
          <h4>Top recalled facts</h4>
          <ol className="recall-stats__top">
            {stats.topFacts.map((f) => (
              <li key={f.factId}>
                <code>
                  {f.subject}.{f.predicate}
                </code>
                <span className="count">×{f.recallCount}</span>
              </li>
            ))}
          </ol>
        </>
      )}
    </section>
  );
}
