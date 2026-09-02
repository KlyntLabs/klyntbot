import { Clock, GitBranch } from "lucide-react";
import type { InsightVersion } from "../../hooks/useInsightVersions";

interface Props {
  versions: InsightVersion[];
  selectedId: string | null;
  currentId: string | null;
  onSelect: (id: string | null) => void;
}

export function InsightVersionList({ versions, selectedId, currentId, onSelect }: Props) {
  if (versions.length === 0) {
    return <p className="text-ui-xs text-fg-dim italic px-3 py-4">No version history yet.</p>;
  }

  return (
    <div className="flex flex-col">
      {versions.map((v) => {
        const isActive = selectedId ? selectedId === v.id : currentId === v.id;
        const date = new Date(v.generatedAt);
        const dateStr = date.toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        });
        const timeStr = date.toLocaleTimeString(undefined, {
          hour: "2-digit",
          minute: "2-digit",
        });

        return (
          <button
            key={v.id}
            type="button"
            onClick={() => onSelect(isActive && selectedId ? null : v.id)}
            className={`flex items-start gap-2 px-3 py-2 text-left transition-colors border-l-2 ${
              isActive
                ? "border-purple bg-white/[0.04]"
                : "border-transparent hover:bg-white/[0.02]"
            }`}
          >
            <div className="flex flex-col gap-0.5 min-w-0">
              <div className="flex items-center gap-1.5">
                <span className="text-ui-xs font-medium text-fg">v{v.version}</span>
                {v.hasParent && (
                  <span title="Merged from related insight">
                    <GitBranch size={10} className="text-fg-secondary" />
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1 text-ui-xs text-fg-dim">
                <Clock size={9} />
                <span>
                  {dateStr} {timeStr}
                </span>
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
