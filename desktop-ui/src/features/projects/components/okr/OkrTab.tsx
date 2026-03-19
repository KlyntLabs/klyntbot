import type { Objective } from "@shared/types";
import { Plus, Target } from "lucide-react";
import { useMemo, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";
import { ObjectiveCard } from "./ObjectiveCard";
import { ObjectiveCreateModal } from "./ObjectiveCreateModal";

type StatusFilter = "all" | "on_track" | "at_risk" | "achieved";

function classifyObjective(objective: Objective): StatusFilter {
  if (objective.progress >= 100) return "achieved";
  if (objective.status === "at_risk") return "at_risk";
  const krs = objective.keyResults ?? [];
  const avgProgress =
    krs.length > 0 ? krs.reduce((s, kr) => s + kr.progress, 0) / krs.length : objective.progress;
  if (avgProgress < 30 && krs.length > 0) return "at_risk";
  return "on_track";
}

const FILTER_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "on_track", label: "On Track" },
  { value: "at_risk", label: "At Risk" },
  { value: "achieved", label: "Achieved" },
];

export function OkrTab() {
  const { objectives } = useProjectContext();
  const [filter, setFilter] = useState<StatusFilter>("all");
  const [modalOpen, setModalOpen] = useState(false);
  const [editingObjective, setEditingObjective] = useState<Objective | undefined>();

  const filtered = useMemo(() => {
    if (filter === "all") return objectives;
    return objectives.filter((o) => classifyObjective(o) === filter);
  }, [objectives, filter]);

  const overallProgress = useMemo(() => {
    if (objectives.length === 0) return 0;
    return Math.round(objectives.reduce((s, o) => s + o.progress, 0) / objectives.length);
  }, [objectives]);

  const handleEdit = (objective: Objective) => {
    setEditingObjective(objective);
    setModalOpen(true);
  };

  const handleCloseModal = () => {
    setModalOpen(false);
    setEditingObjective(undefined);
  };

  return (
    <div className="p-6 space-y-4">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Target className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-sm font-semibold text-foreground">Objectives</h2>
        <span className="text-[10px] px-2 py-0.5 rounded-full bg-brand/10 text-brand font-medium">
          {overallProgress}%
        </span>

        {/* Filter bar */}
        <div className="flex items-center gap-1 ml-4 p-0.5 rounded-md bg-accent/50">
          {FILTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setFilter(opt.value)}
              className={`px-2.5 py-1 text-[10px] font-medium rounded transition-colors ${
                filter === opt.value
                  ? "bg-brand/20 text-brand"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Create button */}
        <button
          type="button"
          onClick={() => setModalOpen(true)}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md bg-brand text-white hover:bg-brand/90 transition-colors"
        >
          <Plus className="w-3.5 h-3.5" /> New Objective
        </button>
      </div>

      {/* Objective list */}
      {filtered.length > 0 ? (
        <div className="space-y-3">
          {filtered.map((objective) => (
            <ObjectiveCard key={objective.id} objective={objective} onEdit={handleEdit} />
          ))}
        </div>
      ) : (
        /* Empty state */
        <div className="border-2 border-dashed border-border rounded-lg p-8 text-center">
          <Target className="w-8 h-8 text-muted-foreground mx-auto mb-3" />
          <p className="text-sm text-muted-foreground mb-1">
            {filter === "all"
              ? "No objectives defined."
              : `No ${FILTER_OPTIONS.find((o) => o.value === filter)?.label ?? ""} objectives.`}
          </p>
          {filter === "all" && (
            <>
              <p className="text-xs text-muted-foreground mb-4">
                Create one or ask AI: "Suggest objectives for this project."
              </p>
              <button
                type="button"
                onClick={() => setModalOpen(true)}
                className="inline-flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-md bg-brand text-white hover:bg-brand/90 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" /> New Objective
              </button>
            </>
          )}
        </div>
      )}

      {/* Create/Edit modal */}
      <ObjectiveCreateModal
        open={modalOpen}
        onClose={handleCloseModal}
        editingObjective={editingObjective}
      />
    </div>
  );
}
