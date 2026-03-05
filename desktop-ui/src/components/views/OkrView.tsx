import { ChevronDown, ChevronRight, Plus, Target } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import { useSetToggle } from "../../hooks/useSetToggle";
import type { Objective, ObjectiveCreateParams, Project } from "../../lib/types";
import { Badge } from "../ui/Badge";
import { Progress } from "../ui/Progress";

export function OkrView() {
  const navigate = useNavigate();

  const { data: objectives, refetch } = useQuery<Objective[]>("objective_list", undefined, []);
  const { data: projects } = useQuery<Project[]>("project_list", undefined, []);
  const createObjective = useMutation<Objective, ObjectiveCreateParams>(
    "objective_create",
    "params",
  );

  const [expandedProjects, toggleProject] = useSetToggle(new Set(projects.map((p) => p.id)));
  const [addingTo, setAddingTo] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState("");

  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetch();
  });

  const projectMap = useMemo(() => new Map(projects.map((p) => [p.id, p])), [projects]);

  const objectivesByProject = useMemo(() => {
    const grouped: Record<string, Objective[]> = {};
    for (const obj of objectives) {
      const key = obj.projectId;
      if (!grouped[key]) grouped[key] = [];
      grouped[key].push(obj);
    }
    return grouped;
  }, [objectives]);

  const handleAddObjective = useCallback(
    async (projectId: string) => {
      if (!newTitle.trim()) return;
      await createObjective.mutate({ title: newTitle.trim(), projectId });
      setNewTitle("");
      setAddingTo(null);
    },
    [newTitle, createObjective],
  );

  // Show projects that have objectives, plus any with objectiveIds
  const relevantProjectIds = useMemo(() => {
    const ids = new Set(Object.keys(objectivesByProject));
    for (const p of projects) {
      if (p.objectiveIds && p.objectiveIds.length > 0) ids.add(p.id);
    }
    return Array.from(ids);
  }, [objectivesByProject, projects]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-[14px] font-light text-primary">Objectives & Key Results</h2>
      </div>

      {relevantProjectIds.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <Target className="w-8 h-8 text-muted mb-3" strokeWidth={1} />
          <p className="text-muted text-sm font-light">No objectives yet</p>
          <p className="text-dim text-xs font-light mt-1">
            Create objectives from a project detail page
          </p>
        </div>
      )}

      {relevantProjectIds.map((projectId) => {
        const project = projectMap.get(projectId);
        if (!project) return null;
        const projectObjectives = objectivesByProject[projectId] ?? [];
        const isExpanded = expandedProjects.has(projectId);
        const avgProgress =
          projectObjectives.length > 0
            ? Math.round(
                projectObjectives.reduce((s, o) => s + o.progress, 0) / projectObjectives.length,
              )
            : 0;

        return (
          <div key={projectId} className="bg-surface-low rounded-xl overflow-hidden">
            {/* Project Header */}
            <button
              onClick={() => toggleProject(projectId)}
              className="w-full flex items-center gap-3 px-4 py-3 hover:bg-surface-base transition-colors text-left"
            >
              {isExpanded ? (
                <ChevronDown className="w-3.5 h-3.5 text-muted flex-shrink-0" strokeWidth={1.5} />
              ) : (
                <ChevronRight className="w-3.5 h-3.5 text-muted flex-shrink-0" strokeWidth={1.5} />
              )}
              <div
                className="w-2 h-2 rounded-full flex-shrink-0"
                style={{ backgroundColor: project.color }}
              />
              <span className="text-[13px] font-light text-secondary flex-1">{project.name}</span>
              <span className="text-[11px] text-muted font-light">
                {projectObjectives.length} objectives
              </span>
              <span className="text-[11px] text-muted font-light ml-2">{avgProgress}%</span>
            </button>

            {/* Objectives */}
            {isExpanded && (
              <div className="px-4 pb-3 space-y-2">
                {projectObjectives.map((obj) => (
                  <button
                    key={obj.id}
                    onClick={() => navigate(`/objective/${obj.id}`)}
                    className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-surface-base transition-colors text-left"
                  >
                    <Target className="w-3.5 h-3.5 text-brand flex-shrink-0" strokeWidth={1.5} />
                    <span className="text-[13px] font-light text-secondary flex-1 truncate">
                      {obj.title}
                    </span>
                    <Badge variant="status" value={obj.status} />
                    <div className="w-20">
                      <Progress value={obj.progress} />
                    </div>
                    <span className="text-[11px] text-muted font-light w-10 text-right">
                      {obj.progress}%
                    </span>
                  </button>
                ))}

                {/* Add Objective inline */}
                {addingTo === projectId ? (
                  <div className="px-3 py-2.5">
                    <input
                      value={newTitle}
                      onChange={(e) => setNewTitle(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleAddObjective(projectId);
                        if (e.key === "Escape") {
                          setAddingTo(null);
                          setNewTitle("");
                        }
                      }}
                      onBlur={() => {
                        if (!newTitle.trim()) {
                          setAddingTo(null);
                          setNewTitle("");
                        }
                      }}
                      placeholder="Objective title..."
                      className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
                    />
                  </div>
                ) : (
                  <button
                    onClick={() => setAddingTo(projectId)}
                    className="flex items-center gap-2 px-3 py-2 text-[12px] text-muted hover:text-brand transition-colors font-light"
                  >
                    <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                    New Objective
                  </button>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
