import { useEffect, useState } from "react";
import { fetchRecallFacts } from "@/api/endpoints/codingMemory";

interface Props {
  factIds: string[];
}

export function CausalGraphInspector({ factIds }: Props) {
  const [facts, setFacts] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    if (factIds.length === 0) return;
    fetchRecallFacts(factIds).then((data) => {
      if (Array.isArray(data)) setFacts(data as any[]);
    });
  }, [factIds]);

  if (factIds.length === 0) return null;
  if (facts.length === 0)
    return <div className="cm-state cm-state--loading">Loading causal graph…</div>;

  return (
    <section className="cm-causal" aria-label="Causal graph">
      <h4 className="cm-causal__title">Causal Graph ({facts.length} facts)</h4>
      <ul className="cm-causal__list">
        {facts.map((f, i) => (
          <li key={i} className="cm-causal__row">
            <span className="cm-causal__subject">{String(f.subject ?? "?")}</span>
            <span className="cm-causal__arrow">→</span>
            <span className="cm-causal__predicate">{String(f.predicate ?? "?")}</span>
            <span className="cm-causal__arrow">→</span>
            <span className="cm-causal__object">{String(f.object ?? "?")}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
