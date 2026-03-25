// desktop-ui/src/features/projects/components/ProjectHeader.tsx

import { ProgressRing } from "@shared/ui";
import { ArrowLeft, MessageSquare } from "lucide-react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../contexts/ProjectContext";
import { useHealthScore } from "../hooks/useHealthScore";

export function ProjectHeader() {
  const navigate = useNavigate();
  const { project, objectives, tasks } = useProjectContext();
  const health = useHealthScore(objectives, tasks, project?.id);

  if (!project) return null;

  return (
    <div className="flex items-center gap-3 px-6 py-3 border-b border-border">
      <button
        type="button"
        onClick={() => navigate(-1)}
        className="text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="size-4" />
      </button>
      <div
        className="size-2.5 rounded-full flex-shrink-0"
        style={{ backgroundColor: project.color }}
      />
      <h1 className="text-base font-semibold text-foreground truncate">{project.name}</h1>
      <div className="ml-auto flex items-center gap-3">
        <button
          type="button"
          onClick={() => navigate(`/project/${project.id}/okr`)}
          title={`Health: ${health.score}%`}
          className="cursor-pointer"
        >
          <ProgressRing progress={health.score} size="sm" gradient />
        </button>
        <button
          type="button"
          onClick={() => navigate(`/chat?project=${project.id}`)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-accent text-muted-foreground hover:text-foreground text-[11px] transition-colors"
          title="Ask AI about this project"
        >
          <MessageSquare className="size-3.5" />
          Ask AI
        </button>
      </div>
    </div>
  );
}
