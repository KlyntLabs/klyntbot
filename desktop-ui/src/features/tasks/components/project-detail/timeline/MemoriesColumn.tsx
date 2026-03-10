import { useProjectMemories } from "@shared/hooks";
import type { SemanticFactSummary } from "@shared/hooks/useProjectMemories";

interface MemoriesColumnProps {
  projectId: string;
}

const TYPE_COLORS: Record<string, string> = {
  decision: "#f97316",
  milestone: "#22c55e",
  pattern: "#a855f7",
  insight: "#3b82f6",
  fact: "#6b7280",
};

export function MemoriesColumn({ projectId }: MemoriesColumnProps) {
  const { data: memories } = useProjectMemories(projectId);

  if (memories.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-[11px] text-dim font-light">
        No memories
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5 p-2">
      {memories.slice(0, 20).map((mem: SemanticFactSummary) => (
        <div key={mem.id} className="flex items-start gap-2 px-2.5 py-1.5 rounded-md">
          <div
            className="w-1.5 h-1.5 rounded-full mt-1 shrink-0"
            style={{ backgroundColor: TYPE_COLORS[mem.domain] ?? "#6b7280" }}
          />
          <div className="flex-1 min-w-0">
            <p className="text-[11px] font-light text-secondary truncate">
              {mem.subject} {mem.predicate} {mem.object}
            </p>
            <p className="text-[9px] text-dim">{mem.domain}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
