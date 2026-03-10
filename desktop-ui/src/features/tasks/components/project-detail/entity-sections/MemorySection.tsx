import { CollapsibleSection } from "@shared/components";
import { useProjectMemories } from "@shared/hooks";
import type { SemanticFactSummary } from "@shared/hooks/useProjectMemories";
import { Brain } from "lucide-react";
import { useState } from "react";

interface MemorySectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

const MEMORY_TYPES = ["All", "decision", "insight", "milestone", "pattern"] as const;

const TYPE_COLORS: Record<string, string> = {
  decision: "bg-orange-500/20 text-orange-400",
  milestone: "bg-green-500/20 text-green-400",
  pattern: "bg-purple-500/20 text-purple-400",
  insight: "bg-blue-500/20 text-blue-400",
  fact: "bg-gray-500/20 text-gray-400",
};

export function MemorySection({ projectId, defaultOpen }: MemorySectionProps) {
  const [filter, setFilter] = useState<string>("All");
  const { data: memories } = useProjectMemories(projectId, filter === "All" ? undefined : filter);

  return (
    <CollapsibleSection
      title="Memories"
      icon={<Brain className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      count={memories.length || null}
      defaultOpen={defaultOpen}
    >
      {/* Filter tabs */}
      <div className="flex gap-1 mb-2 flex-wrap">
        {MEMORY_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            onClick={() => setFilter(type)}
            className={`px-2 py-0.5 rounded text-[10px] font-light transition-colors ${
              filter === type
                ? "bg-brand/20 text-brand"
                : "text-muted hover:text-secondary hover:bg-white/[0.04]"
            }`}
          >
            {type === "All" ? "All" : type.charAt(0).toUpperCase() + type.slice(1)}
          </button>
        ))}
      </div>

      {memories.length === 0 ? (
        <p className="text-[11px] text-dim font-light py-2">No memories</p>
      ) : (
        <div className="space-y-1 max-h-48 overflow-y-auto">
          {memories.map((mem: SemanticFactSummary) => (
            <div key={mem.id} className="px-2 py-1.5 rounded-md">
              <div className="flex items-center gap-1.5 mb-0.5">
                <span
                  className={`text-[9px] px-1.5 py-0.5 rounded ${TYPE_COLORS[mem.domain] ?? TYPE_COLORS.fact}`}
                >
                  {mem.domain}
                </span>
                <span className="text-[9px] text-dim tabular-nums">
                  {Math.round(mem.confidence * 100)}%
                </span>
              </div>
              <p className="text-[11px] font-light text-secondary">
                {mem.subject} {mem.predicate} {mem.object}
              </p>
            </div>
          ))}
        </div>
      )}
    </CollapsibleSection>
  );
}
