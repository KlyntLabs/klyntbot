import { ChevronDown, ChevronRight } from 'lucide-react';
import { useNavigate } from 'react-router';
import type { Project, Task, Objective } from '../../lib/types';

interface ProjectHeaderProps {
  project: Project;
  tasks: Task[];
  objectives: Objective[];
  isCollapsed: boolean;
  onToggle: () => void;
}

export function ProjectHeader({ project, tasks, objectives, isCollapsed, onToggle }: ProjectHeaderProps) {
  const navigate = useNavigate();

  return (
    <button
      onClick={onToggle}
      className="w-full flex items-center gap-3 px-6 py-3 bg-[rgba(0,0,0,0.15)] hover:bg-[rgba(0,0,0,0.2)] transition-colors text-left border-b border-[rgba(255,255,255,0.04)]"
    >
      {isCollapsed ? (
        <ChevronRight className="w-[14px] h-[14px] text-[#8B949E] flex-shrink-0" strokeWidth={1.5} />
      ) : (
        <ChevronDown className="w-[14px] h-[14px] text-[#8B949E] flex-shrink-0" strokeWidth={1.5} />
      )}
      <div
        className="w-2 h-2 rounded-full flex-shrink-0"
        style={{ backgroundColor: project.color }}
      />
      <span className="text-[12px] font-light text-[#C9D1D9] flex-shrink-0">{project.name}</span>
      <span className="text-[11px] text-[#8B949E] font-light flex-shrink-0">({tasks.length})</span>

      {/* Inline Objectives */}
      {objectives.length > 0 && (
        <>
          <span className="text-[#6E7681] text-[11px] flex-shrink-0">&middot;</span>
          <div className="flex items-center gap-2 overflow-hidden flex-1 min-w-0">
            {objectives.map((objective, idx) => (
              <div key={objective.id} className="flex items-center gap-1.5 flex-shrink-0">
                <span className="text-[10px] text-[#8B949E] font-light truncate">{objective.title}</span>
                <span className="text-[9px] text-[#6E7681] font-light">{objective.progress}%</span>
                {idx < objectives.length - 1 && (
                  <span className="text-[#6E7681] text-[10px] ml-1">&middot;</span>
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {/* Details Link */}
      <div
        onClick={(e) => {
          e.stopPropagation();
          navigate(`/project/${project.id}`);
        }}
        className="text-[11px] text-[#8B949E] hover:text-[#F97316] font-light transition-colors flex-shrink-0 ml-auto cursor-pointer"
      >
        Details &gt;
      </div>
    </button>
  );
}
