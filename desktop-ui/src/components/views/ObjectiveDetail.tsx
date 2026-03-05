import { ArrowLeft, Plus, Trash2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type {
  KeyResult,
  KeyResultCreateParams,
  Objective,
  ObjectiveUpdateParams,
  Project,
} from "../../lib/types";
import { Progress } from "../ui/Progress";

const STATUSES = ["active", "paused", "completed", "abandoned"] as const;

export function ObjectiveDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data: objectives, refetch } = useQuery<Objective[]>("objective_list", undefined, []);
  const { data: projects } = useQuery<Project[]>("project_list", undefined, []);
  const updateObjective = useMutation<Objective, ObjectiveUpdateParams>(
    "objective_update",
    "params",
  );
  const deleteObjective = useMutation<boolean, { id: string }>("objective_delete");
  const createKr = useMutation<KeyResult, KeyResultCreateParams>("key_result_create", "params");
  const updateKrMetric = useMutation<KeyResult, { id: string; currentValue: number }>(
    "key_result_update_metric",
  );
  const deleteKr = useMutation<boolean, { id: string }>("key_result_delete");

  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [newKrTitle, setNewKrTitle] = useState("");
  const [addingKr, setAddingKr] = useState(false);

  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetch();
  });

  const objective = useMemo(() => objectives.find((o) => o.id === id), [objectives, id]);
  const project = useMemo(
    () => (objective ? projects.find((p) => p.id === objective.projectId) : undefined),
    [objective, projects],
  );

  const handleUpdate = useCallback(
    async (params: Partial<ObjectiveUpdateParams>) => {
      if (!id) return;
      await updateObjective.mutate({ id, ...params });
    },
    [id, updateObjective],
  );

  const handleDelete = useCallback(async () => {
    if (!id) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await deleteObjective.mutate({ id });
    navigate(-1);
  }, [id, confirmDelete, deleteObjective, navigate]);

  const handleAddKr = useCallback(async () => {
    if (!id || !newKrTitle.trim()) return;
    await createKr.mutate({
      objectiveId: id,
      title: newKrTitle.trim(),
      targetValue: 100,
      unit: "%",
    });
    setNewKrTitle("");
    setAddingKr(false);
  }, [id, newKrTitle, createKr]);

  const handleUpdateMetric = useCallback(
    async (krId: string, value: number) => {
      await updateKrMetric.mutate({ id: krId, currentValue: value });
    },
    [updateKrMetric],
  );

  const handleDeleteKr = useCallback(
    async (krId: string) => {
      await deleteKr.mutate({ id: krId });
    },
    [deleteKr],
  );

  if (!objective) {
    return (
      <div className="h-screen w-screen bg-background text-primary flex items-center justify-center">
        <p className="text-muted text-sm font-light">Objective not found</p>
      </div>
    );
  }

  const krs = objective.keyResults ?? [];

  return (
    <div className="h-screen w-screen bg-background text-primary flex flex-col overflow-hidden">
      {/* Header */}
      <div className="h-14 flex items-center px-6 gap-3 border-b border-border flex-shrink-0">
        <button
          type="button"
          onClick={() => navigate(-1)}
          className="text-muted hover:text-secondary transition-colors"
        >
          <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
        </button>
        {project && (
          <button
            type="button"
            onClick={() => navigate(`/project/${project.id}`)}
            className="flex items-center gap-2 text-muted hover:text-secondary transition-colors"
          >
            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: project.color }} />
            <span className="text-[12px] font-light">{project.name}</span>
          </button>
        )}
        <span className="text-dim text-[12px]">/</span>
        <span className="text-[12px] text-muted font-light">Objective</span>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6 max-w-2xl">
        {/* Title */}
        {editingTitle ? (
          <input
            value={titleDraft}
            onChange={(e) => setTitleDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleUpdate({ title: titleDraft });
                setEditingTitle(false);
              }
              if (e.key === "Escape") setEditingTitle(false);
            }}
            onBlur={() => {
              if (titleDraft !== objective.title) handleUpdate({ title: titleDraft });
              setEditingTitle(false);
            }}
            className="text-[22px] font-light text-primary bg-transparent border-b border-brand outline-none w-full mb-4"
          />
        ) : (
          <button
            type="button"
            onClick={() => {
              setTitleDraft(objective.title);
              setEditingTitle(true);
            }}
            className="text-[22px] font-light text-primary cursor-text mb-4 hover:text-secondary transition-colors text-left"
          >
            {objective.title}
          </button>
        )}

        {/* Overall Progress */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[12px] text-muted font-light">Overall Progress</span>
            <span className="text-[14px] font-light text-primary">{objective.progress}%</span>
          </div>
          <Progress value={objective.progress} />
        </div>

        {/* Status */}
        <div className="grid grid-cols-[120px_1fr] gap-y-4 gap-x-4 mb-6">
          <span className="text-[12px] text-muted font-light self-center">Status</span>
          <div className="flex gap-2">
            {STATUSES.map((s) => (
              <button
                type="button"
                key={s}
                onClick={() => handleUpdate({ status: s })}
                className={`px-3 py-1 rounded-md text-[12px] font-light transition-colors ${
                  objective.status === s
                    ? "bg-brand text-white"
                    : "bg-surface-low text-muted hover:bg-surface-base"
                }`}
              >
                {s}
              </button>
            ))}
          </div>
        </div>

        {/* Key Results */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="text-[12px] text-muted font-light uppercase tracking-wider">
              Key Results
            </span>
            <button
              type="button"
              onClick={() => setAddingKr(true)}
              className="flex items-center gap-1 text-[12px] text-brand hover:text-brand-hover transition-colors font-light"
            >
              <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
              Add
            </button>
          </div>

          <div className="space-y-2">
            {krs.map((kr) => (
              <div key={kr.id} className="bg-surface-low rounded-xl px-4 py-3">
                <div className="flex items-center gap-3 mb-2">
                  <span className="text-[13px] font-light text-secondary flex-1">{kr.title}</span>
                  <button
                    type="button"
                    onClick={() => handleDeleteKr(kr.id)}
                    className="text-muted hover:text-destructive transition-colors"
                  >
                    <Trash2 className="w-3 h-3" strokeWidth={1.5} />
                  </button>
                </div>
                <div className="flex items-center gap-3">
                  <input
                    type="number"
                    defaultValue={kr.current}
                    onBlur={(e) => {
                      const v = parseFloat(e.target.value) || 0;
                      if (v !== kr.current) handleUpdateMetric(kr.id, v);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") e.currentTarget.blur();
                    }}
                    className="w-20 bg-surface-base rounded-md px-2 py-1 text-[12px] font-light text-primary border border-border-subtle outline-none text-center"
                  />
                  <span className="text-[11px] text-dim font-light">
                    / {kr.target} {kr.unit}
                  </span>
                  <div className="flex-1">
                    <Progress value={kr.progress} />
                  </div>
                  <span className="text-[11px] text-muted font-light w-10 text-right">
                    {kr.progress}%
                  </span>
                </div>
              </div>
            ))}

            {krs.length === 0 && !addingKr && (
              <div className="text-center py-6">
                <p className="text-[13px] text-muted font-light">No key results yet</p>
                <p className="text-[11px] text-dim font-light mt-1">
                  Add key results to track progress
                </p>
              </div>
            )}

            {/* Add KR inline row */}
            {addingKr && (
              <div className="bg-surface-low rounded-xl px-4 py-3">
                <input
                  value={newKrTitle}
                  onChange={(e) => setNewKrTitle(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleAddKr();
                    if (e.key === "Escape") {
                      setAddingKr(false);
                      setNewKrTitle("");
                    }
                  }}
                  onBlur={() => {
                    if (!newKrTitle.trim()) {
                      setAddingKr(false);
                      setNewKrTitle("");
                    }
                  }}
                  placeholder="Key result title\u2026"
                  className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
                />
              </div>
            )}
          </div>
        </div>

        {/* Delete */}
        <div className="pt-4 border-t border-border">
          <button
            type="button"
            onClick={handleDelete}
            onBlur={() => setConfirmDelete(false)}
            className={`flex items-center gap-2 px-3 py-2 rounded-md text-[12px] font-light transition-colors ${
              confirmDelete
                ? "bg-destructive text-white"
                : "text-muted hover:text-destructive hover:bg-surface-low"
            }`}
          >
            <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
            {confirmDelete ? "Click again to delete" : "Delete objective"}
          </button>
        </div>
      </div>
    </div>
  );
}
