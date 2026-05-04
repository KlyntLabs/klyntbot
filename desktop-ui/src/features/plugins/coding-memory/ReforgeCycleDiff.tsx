import { useEffect, useState } from "react";
import { fetchReforgeCycleDiff, listReforgeCycles } from "@/api/endpoints/codingMemory";

interface Cycle {
  cycleId: string;
  ranAt: string;
  repos: string[];
  artifactsWritten: number;
}

interface Props {
  repoId?: string;
}

export function ReforgeCycleDiff({ repoId }: Props) {
  const [cycles, setCycles] = useState<Cycle[]>([]);
  const [selectedBefore, setSelectedBefore] = useState<string | null>(null);
  const [selectedAfter, setSelectedAfter] = useState<string | null>(null);
  const [diff, setDiff] = useState<{ beforeBody: string; afterBody: string } | null>(null);

  useEffect(() => {
    listReforgeCycles().then((data) => {
      if (Array.isArray(data)) setCycles(data as Cycle[]);
    });
  }, []);

  const loadDiff = async () => {
    if (!repoId || !selectedBefore || !selectedAfter) return;
    const result = await fetchReforgeCycleDiff({
      repoId,
      artifact: "claude_md",
      beforeCycleId: selectedBefore,
      afterCycleId: selectedAfter,
    });
    setDiff(result as { beforeBody: string; afterBody: string });
  };

  return (
    <div className="cm-reforge">
      <div className="cm-reforge__cycles">
        <h3 className="cm-reforge__title">Reforge Cycles</h3>
        <ul className="cm-reforge__list">
          {cycles.map((c) => (
            <li key={c.cycleId} className="cm-reforge__item">
              <button
                type="button"
                className={
                  "cm-reforge__cycle" +
                  (selectedBefore === c.cycleId
                    ? " cm-reforge__cycle--before"
                    : selectedAfter === c.cycleId
                      ? " cm-reforge__cycle--after"
                      : "")
                }
                onClick={() => {
                  if (!selectedBefore || selectedAfter) {
                    setSelectedBefore(c.cycleId);
                    setSelectedAfter(null);
                    setDiff(null);
                  } else {
                    setSelectedAfter(c.cycleId);
                  }
                }}
              >
                {c.cycleId.slice(0, 8)}… ({c.artifactsWritten} artifacts)
              </button>
              <span className="cm-reforge__when">{new Date(c.ranAt).toLocaleDateString()}</span>
            </li>
          ))}
        </ul>
        {selectedBefore && selectedAfter && (
          <button type="button" className="cm-reforge__diff-btn" onClick={loadDiff}>
            Show Diff
          </button>
        )}
      </div>
      {diff && (
        <div className="cm-reforge__diff">
          <div className="cm-reforge__col">
            <div className="cm-reforge__col-title">Before</div>
            <pre>{diff.beforeBody}</pre>
          </div>
          <div className="cm-reforge__col">
            <div className="cm-reforge__col-title">After</div>
            <pre>{diff.afterBody}</pre>
          </div>
        </div>
      )}
    </div>
  );
}
