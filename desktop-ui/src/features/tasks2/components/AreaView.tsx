import { useMemo } from "react";
import { getAreaById } from "../mock-data/areas";
import { projects } from "../mock-data/projects";
import { useIssuesStore } from "../store/issues-store";
import { useTabStore } from "../store/tab-store";

interface AreaViewProps {
  areaId: string;
}

export function AreaView({ areaId }: AreaViewProps) {
  const area = getAreaById(areaId);
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const issues = useIssuesStore((s) => s.issues);

  const areaProjects = useMemo(() => {
    if (!area) return [];
    return projects.filter((p) => area.projectIds.includes(p.id));
  }, [area]);

  const projectIssueCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const issue of issues) {
      if (issue.project) {
        counts[issue.project.id] = (counts[issue.project.id] ?? 0) + 1;
      }
    }
    return counts;
  }, [issues]);

  if (!area) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Area not found
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {areaProjects.map((project) => {
        const Icon = project.icon;
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
            className="flex items-center gap-3 px-4 py-3 text-left hover:bg-[hsl(var(--accent))] transition-colors border-b border-[hsl(var(--border))]"
          >
            <Icon className="h-4 w-4 text-[hsl(var(--muted-foreground))]" />
            <span className="text-sm text-[hsl(var(--foreground))] flex-1">{project.name}</span>
            <span className="text-xs text-[hsl(var(--muted-foreground))]">{count} issues</span>
          </button>
        );
      })}
    </div>
  );
}
