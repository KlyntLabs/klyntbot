import { useMutation } from "@shared/hooks/useMutation";
import type { Project, ProjectUpdateParams } from "@shared/types";
import { Archive, ArrowLeft } from "lucide-react";
import { useCallback, useState } from "react";
import { useNavigate } from "react-router";

const PROJECT_COLORS = [
  "#3b82f6",
  "#ef4444",
  "#f97316",
  "#eab308",
  "#22c55e",
  "#a855f7",
  "#6b7280",
];

interface ProjectDetailHeaderProps {
  project: Project;
}

export function ProjectDetailHeader({ project }: ProjectDetailHeaderProps) {
  const navigate = useNavigate();
  const updateProject = useMutation<Project, ProjectUpdateParams>("project_update", "params");
  const archiveProject = useMutation<Project, { id: string }>("project_archive");

  const [editingName, setEditingName] = useState(false);
  const [nameDraft, setNameDraft] = useState("");
  const [showColorPicker, setShowColorPicker] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);

  const handleUpdateProject = useCallback(
    async (params: Partial<ProjectUpdateParams>) => {
      await updateProject.mutate({ id: project.id, ...params });
    },
    [project.id, updateProject],
  );

  const handleArchive = useCallback(async () => {
    if (!confirmArchive) {
      setConfirmArchive(true);
      return;
    }
    await archiveProject.mutate({ id: project.id });
    navigate("/");
  }, [project.id, confirmArchive, archiveProject, navigate]);

  return (
    <div className="h-12 flex items-center px-6 gap-3 shrink-0 border-b border-white/[0.06]">
      <button
        type="button"
        onClick={() => navigate("/")}
        className="text-muted hover:text-secondary transition-colors"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>

      {/* Color dot */}
      <div className="relative">
        <button
          type="button"
          onClick={() => setShowColorPicker(!showColorPicker)}
          className="w-2.5 h-2.5 rounded-full cursor-pointer hover:ring-2 hover:ring-brand/30 transition-shadow"
          style={{ backgroundColor: project.color }}
        />
        {showColorPicker && (
          <div className="absolute top-6 left-0 z-50 glass-dropdown flex gap-1.5">
            {PROJECT_COLORS.map((c) => (
              <button
                type="button"
                key={c}
                onClick={() => {
                  handleUpdateProject({ color: c });
                  setShowColorPicker(false);
                }}
                className={`w-5 h-5 rounded-full hover:ring-2 hover:ring-brand/30 transition-shadow ${project.color === c ? "ring-2 ring-brand" : ""}`}
                style={{ backgroundColor: c }}
              />
            ))}
          </div>
        )}
      </div>

      {/* Project name — click to edit */}
      {editingName ? (
        <input
          value={nameDraft}
          onChange={(e) => setNameDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              handleUpdateProject({ name: nameDraft });
              setEditingName(false);
            }
            if (e.key === "Escape") setEditingName(false);
          }}
          onBlur={() => {
            if (nameDraft !== project.name) handleUpdateProject({ name: nameDraft });
            setEditingName(false);
          }}
          className="text-[14px] font-light text-primary bg-transparent border-b border-brand outline-none"
        />
      ) : (
        <button
          type="button"
          onClick={() => {
            setNameDraft(project.name);
            setEditingName(true);
          }}
          className="text-[14px] font-light text-primary cursor-text hover:text-secondary transition-colors"
        >
          {project.name}
        </button>
      )}

      {project.userRole && (
        <span className="text-[11px] text-dim font-light px-2 py-0.5 rounded bg-white/[0.04]">
          {project.userRole}
        </span>
      )}

      <div className="flex-1" />

      {/* Archive */}
      <button
        type="button"
        onClick={handleArchive}
        onBlur={() => setConfirmArchive(false)}
        className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[11px] font-light transition-colors ${
          confirmArchive
            ? "bg-destructive text-white"
            : "text-muted hover:text-secondary hover:bg-white/[0.04]"
        }`}
      >
        <Archive className="w-3.5 h-3.5" strokeWidth={1.5} />
        {confirmArchive ? "Click again" : "Archive"}
      </button>
    </div>
  );
}
