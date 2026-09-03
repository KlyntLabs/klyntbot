import { Folder } from "lucide-react";
import { useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { useTabStore } from "../store/tab-store";

interface AreaViewProps {
  areaId: string;
  tasksData: UseTasksResult;
}

export function AreaView({ areaId, tasksData }: AreaViewProps) {
  const area = tasksData.areaMap.get(areaId);
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);

  const areaProjects = useMemo(() => {
    if (!area) return [];
    return tasksData.projects.filter((p) => p.areaId === areaId);
  }, [area, tasksData.projects, areaId]);

  const projectIssueCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const issue of tasksData.issues) {
      if (issue.project) {
        counts[issue.project.id] = (counts[issue.project.id] ?? 0) + 1;
      }
    }
    return counts;
  }, [tasksData.issues]);

  if (!area) {
    return <div className="px-6 py-8 text-center text-sm text-fg-secondary">Area not found</div>;
  }

  return (
    <div className="flex flex-col">
      {areaProjects.map((project) => {
        const count = projectIssueCounts[project.id] ?? 0;
        return (
          <button
            key={project.id}
            type="button"
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey) {
                useTabStore.getState().openTab("project", project.id, project.name);
              } else {
                navigateInPlace("project", project.id, project.name);
              }
            }}
            className="flex items-center gap-3 px-4 py-3 text-left hover:bg-control-hover transition-colors border-b border-separator"
          >
            <Folder className="h-4 w-4 text-fg-secondary" />
            <span className="text-sm text-fg flex-1">{project.name}</span>
            <span className="text-ui-sm text-fg-secondary">{count} issues</span>
          </button>
        );
      })}
    </div>
  );
}
