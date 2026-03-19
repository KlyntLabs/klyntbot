// desktop-ui/src/features/projects/components/ProjectHeader.tsx

import { ProgressRing } from "@shared/ui";
import { ArrowLeft } from "lucide-react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../contexts/ProjectContext";
import { useHealthScore } from "../hooks/useHealthScore";
import { useProjectTasks } from "../hooks/useProjectTasks";

export function ProjectHeader() {
  const navigate = useNavigate();
  const { project, objectives } = useProjectContext();
  const { data: tasks } = useProjectTasks(project?.id ?? "");
  const health = useHealthScore(objectives, tasks);

  if (!project) return null;

  return (
    <div className="flex items-center gap-3 px-6 py-3 border-b border-border">
      <button
        type="button"
        onClick={() => navigate(-1)}
        className="text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="w-4 h-4" />
      </button>
      <div
        className="w-2.5 h-2.5 rounded-full flex-shrink-0"
        style={{ backgroundColor: project.color }}
      />
      <h1 className="text-base font-semibold text-foreground truncate">{project.name}</h1>
      {project.areaId && (
        <span className="text-[11px] px-2 py-0.5 rounded-full bg-brand/10 text-brand">
          {project.areaId}
        </span>
      )}
      <div className="ml-auto flex items-center gap-3">
        <button
          type="button"
          onClick={() => navigate(`/project/${project.id}/okr`)}
          title={`Health: ${health.score}%`}
          className="cursor-pointer"
        >
          <ProgressRing progress={health.score} size="sm" gradient />
        </button>
        {/* TODO: "Ask AI about this project" button — wires to SidebarChat */}
      </div>
    </div>
  );
}
