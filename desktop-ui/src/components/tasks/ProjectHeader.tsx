import { ChevronDown, ChevronRight } from "lucide-react";
import { useNavigate } from "react-router";
import type { Objective, Project, Task } from "../../lib/types";

interface ProjectHeaderProps {
  project: Project;
  tasks: Task[];
  objectives: Objective[];
  isCollapsed: boolean;
  onToggle: () => void;
}

export function ProjectHeader({
  project,
  tasks,
  objectives,
  isCollapsed,
  onToggle,
}: ProjectHeaderProps) {
  const navigate = useNavigate();

  return (
    <div className="w-full flex items-center gap-3 px-6 py-3 bg-overlay hover:bg-overlay-heavy transition-colors text-left border-b border-border-subtle">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={!isCollapsed}
        className="flex items-center gap-3 flex-1 min-w-0"
      >
        {isCollapsed ? (
          <ChevronRight className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
        ) : (
          <ChevronDown className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
        )}
        <div
          className="w-2 h-2 rounded-full flex-shrink-0"
          style={{ backgroundColor: project.color }}
        />
        <span className="text-[12px] font-light text-secondary flex-shrink-0">{project.name}</span>
        <span className="text-[11px] text-muted font-light flex-shrink-0">({tasks.length})</span>

        {/* Inline Objectives */}
        {objectives.length > 0 && (
          <>
            <span className="text-dim text-[11px] flex-shrink-0">&middot;</span>
            <div className="flex items-center gap-2 overflow-hidden flex-1 min-w-0">
              {objectives.map((objective, idx) => (
                <div key={objective.id} className="flex items-center gap-1.5 flex-shrink-0">
                  <span className="text-[10px] text-muted font-light truncate">
                    {objective.title}
                  </span>
                  <span className="text-[9px] text-dim font-light">{objective.progress}%</span>
                  {idx < objectives.length - 1 && (
                    <span className="text-dim text-[10px] ml-1">&middot;</span>
                  )}
                </div>
              ))}
            </div>
          </>
        )}
      </button>

      {/* Details Link */}
      <button
        type="button"
        onClick={() => navigate(`/project/${project.id}`)}
        className="text-[11px] text-muted hover:text-brand font-light transition-colors flex-shrink-0 ml-auto"
      >
        Details &gt;
      </button>
    </div>
  );
}
