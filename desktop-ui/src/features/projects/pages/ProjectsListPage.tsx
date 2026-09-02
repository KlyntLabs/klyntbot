import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { Project, ProjectCreateParams } from "@shared/types";
import { ProgressRing } from "@shared/ui";
import { Plus } from "lucide-react";
import { useCallback, useState } from "react";
import { useNavigate } from "react-router";
import { useHealthScore } from "../hooks/useHealthScore";
import { useProjectObjectives } from "../hooks/useProjectObjectives";

function ProjectCard({ project }: { project: Project }) {
  const navigate = useNavigate();
  const { data: objectives } = useProjectObjectives(project.id);
  const tasks: never[] = []; // Lightweight — don't fetch tasks per card
  const health = useHealthScore(objectives, tasks, project.id);

  return (
    <button
      type="button"
      onClick={() => navigate(`/project/${project.id}`)}
      className="island rounded-xl p-5 text-left hover:bg-control-hover/30 transition-colors group"
    >
      <div className="flex items-center gap-3 mb-3">
        <div
          className="size-3 rounded-full flex-shrink-0"
          style={{ backgroundColor: project.color }}
        />
        <h3 className="text-sm font-medium text-fg truncate flex-1">{project.name}</h3>
        <ProgressRing progress={health.score} size="sm" gradient />
      </div>
      <div className="flex items-center gap-4 text-ui-xs text-fg-secondary">
        <span>{project.taskCount - project.completedCount} active tasks</span>
        <span>{objectives.length} objectives</span>
      </div>
      {project.description && (
        <p className="text-ui-xs text-fg-secondary mt-2 line-clamp-2">{project.description}</p>
      )}
    </button>
  );
}

export function ProjectsListPage() {
  const navigate = useNavigate();
  const { data: projects, refetch } = useQuery<Project[]>("project_list", undefined, []);
  const { mutate: createProject } = useMutation<Project, ProjectCreateParams>(
    "project_create",
    "params",
  );
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  const handleCreate = useCallback(async () => {
    if (!newName.trim()) return;
    const result = await createProject({ name: newName.trim(), areaId: "" });
    setCreating(false);
    setNewName("");
    if (result) {
      refetch();
      navigate(`/project/${result.id}`);
    }
  }, [newName, createProject, refetch, navigate]);

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold text-fg">Projects</h1>
        <button
          type="button"
          onClick={() => setCreating(true)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand text-white text-ui-sm font-medium hover:bg-brand/90 transition-colors"
        >
          <Plus className="size-3.5" /> New Project
        </button>
      </div>

      {creating && (
        <div className="island rounded-xl p-4 mb-4 flex items-center gap-3">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreate();
              if (e.key === "Escape") setCreating(false);
            }}
            placeholder="Project name..."
            className="flex-1 bg-transparent text-sm text-fg placeholder:text-fg-secondary focus:outline-none"
          />
          <button
            type="button"
            onClick={handleCreate}
            className="text-ui-sm px-3 py-1 rounded bg-brand text-white"
          >
            Create
          </button>
          <button
            type="button"
            onClick={() => setCreating(false)}
            className="text-ui-sm px-3 py-1 rounded bg-control-hover text-fg-secondary"
          >
            Cancel
          </button>
        </div>
      )}

      {projects.length === 0 && !creating ? (
        <div className="island rounded-xl p-12 text-center">
          <p className="text-sm text-fg-secondary mb-4">
            No projects yet. Create your first project to get started.
          </p>
          <button
            type="button"
            onClick={() => setCreating(true)}
            className="px-4 py-2 rounded-lg bg-brand text-white text-ui-sm font-medium"
          >
            Create Project
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          {projects.map((p) => (
            <ProjectCard key={p.id} project={p} />
          ))}
        </div>
      )}
    </div>
  );
}
