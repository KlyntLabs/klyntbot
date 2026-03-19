import { useMutation } from "@shared/hooks/useMutation";
import type { KeyResult, Objective } from "@shared/types";
import { ProgressRing } from "@shared/ui";
import {
  ChevronDown,
  ChevronRight,
  MoreHorizontal,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
} from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";
import { useProjectDetailStore } from "../../store/project-detail-store";
import { KeyResultCreateForm } from "./KeyResultCreateForm";
import { KeyResultRow } from "./KeyResultRow";

interface ObjectiveCardProps {
  objective: Objective;
  onEdit: (objective: Objective) => void;
}

function computeStatus(objective: Objective): { label: string; color: string } {
  if (objective.progress >= 100)
    return { label: "Achieved", color: "bg-emerald-500/20 text-emerald-400" };
  if (objective.status === "at_risk")
    return { label: "At Risk", color: "bg-amber-500/20 text-amber-400" };

  // Heuristic: if progress is significantly behind, show at risk
  const krs = objective.keyResults ?? [];
  const avgProgress =
    krs.length > 0 ? krs.reduce((s, kr) => s + kr.progress, 0) / krs.length : objective.progress;
  if (avgProgress < 30 && krs.length > 0)
    return { label: "At Risk", color: "bg-amber-500/20 text-amber-400" };

  return { label: "On Track", color: "bg-emerald-500/20 text-emerald-400" };
}

function computeAiConfidence(objective: Objective): number {
  const krs = objective.keyResults ?? [];
  if (krs.length === 0) return 0;

  // Simple heuristic: average KR progress as confidence proxy
  const avgProgress = krs.reduce((s, kr) => s + kr.progress, 0) / krs.length;
  return Math.round(Math.max(0, Math.min(100, avgProgress)));
}

export function ObjectiveCard({ objective, onEdit }: ObjectiveCardProps) {
  const { refetchObjectives } = useProjectContext();
  const expanded = useProjectDetailStore((s) => s.expandedObjectives.has(objective.id));
  const toggleObjective = useProjectDetailStore((s) => s.toggleObjective);

  const [showKrForm, setShowKrForm] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const { mutate: deleteObjective } = useMutation<void, { id: string }>("objective_delete");

  const status = useMemo(() => computeStatus(objective), [objective]);
  const confidence = useMemo(() => computeAiConfidence(objective), [objective]);
  const krs = objective.keyResults ?? [];

  const handleDelete = useCallback(async () => {
    setMenuOpen(false);
    await deleteObjective({ id: objective.id });
    refetchObjectives();
  }, [objective.id, deleteObjective, refetchObjectives]);

  return (
    <div className="glass-card rounded-lg border border-border">
      {/* Objective header */}
      <div className="flex items-center gap-3 px-4 py-3">
        {/* Expand/collapse */}
        <button
          type="button"
          onClick={() => toggleObjective(objective.id)}
          className="text-muted-foreground hover:text-foreground transition-colors"
        >
          {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
        </button>

        {/* Progress ring */}
        <ProgressRing progress={objective.progress} size="md" />

        {/* Title + metadata */}
        <div className="flex-1 min-w-0">
          <h3 className="text-sm font-medium text-foreground truncate">{objective.title}</h3>
          <div className="flex items-center gap-2 mt-0.5">
            <span className="text-[10px] text-muted-foreground">
              {krs.length} Key Result{krs.length !== 1 ? "s" : ""}
            </span>
          </div>
        </div>

        {/* Status badge */}
        <span className={`text-[10px] px-2 py-0.5 rounded-full font-medium ${status.color}`}>
          {status.label}
        </span>

        {/* AI Confidence badge */}
        <span
          className="text-[10px] px-2 py-0.5 rounded-full bg-purple-500/15 text-purple-400 font-medium cursor-help"
          title={`AI Confidence: ${confidence}% — based on KR velocity and progress`}
        >
          AI {confidence}%
        </span>

        {/* "Suggest next KR" placeholder */}
        <button
          type="button"
          className="text-[10px] px-2 py-1 rounded bg-accent text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1"
          title="Ask AI to suggest next Key Result (coming soon)"
        >
          <Sparkles className="w-3 h-3" />
          Suggest KR
        </button>

        {/* Context menu */}
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen(!menuOpen)}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <MoreHorizontal className="w-4 h-4" />
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-full mt-1 z-20 glass-panel rounded-lg py-1 min-w-[140px] bg-card border border-border shadow-lg">
              <button
                type="button"
                onClick={() => {
                  setMenuOpen(false);
                  onEdit(objective);
                }}
                className="w-full px-3 py-1.5 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors flex items-center gap-2"
              >
                <Pencil className="w-3 h-3" /> Edit
              </button>
              <button
                type="button"
                onClick={handleDelete}
                className="w-full px-3 py-1.5 text-left text-xs text-red-400 hover:text-red-300 hover:bg-accent transition-colors flex items-center gap-2"
              >
                <Trash2 className="w-3 h-3" /> Delete
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Expanded KR list */}
      {expanded && (
        <div className="px-4 pb-3 space-y-1">
          {krs.map((kr: KeyResult) => (
            <KeyResultRow key={kr.id} keyResult={kr} projectId={objective.projectId} />
          ))}

          {/* Add Key Result form or button */}
          {showKrForm ? (
            <KeyResultCreateForm
              objectiveId={objective.id}
              onCreated={() => {
                setShowKrForm(false);
                refetchObjectives();
              }}
              onCancel={() => setShowKrForm(false)}
            />
          ) : (
            <button
              type="button"
              onClick={() => setShowKrForm(true)}
              className="flex items-center gap-1.5 ml-4 px-3 py-1.5 text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent rounded transition-colors"
            >
              <Plus className="w-3 h-3" /> Add Key Result
            </button>
          )}
        </div>
      )}
    </div>
  );
}
