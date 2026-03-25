import { useClickOutside } from "@shared/hooks/useClickOutside";
import { useMutation } from "@shared/hooks/useMutation";
import type { KeyResult, Task } from "@shared/types";
import { ProgressRing } from "@shared/ui";
import { ChevronDown, ChevronRight, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";
import { useProjectDetailStore } from "../../store/project-detail-store";
import { LinkedTasksList } from "./LinkedTasksList";

interface KeyResultRowProps {
  keyResult: KeyResult;
  projectId: string;
  tasks?: Task[];
  onEdit?: (kr: KeyResult) => void;
  onDelete?: (kr: KeyResult) => void;
}

export function KeyResultRow({
  keyResult,
  projectId: _projectId,
  tasks = [],
  onEdit,
  onDelete,
}: KeyResultRowProps) {
  const { refetchObjectives } = useProjectContext();
  const expanded = useProjectDetailStore((s) => s.expandedKrs.has(keyResult.id));
  const toggleKr = useProjectDetailStore((s) => s.toggleKr);

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(String(keyResult.current));
  const [menuOpen, setMenuOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const { mutate: updateMetric } = useMutation<void, { id: string; currentValue: number }>(
    "key_result_update_metric",
  );

  useClickOutside(menuRef, () => setMenuOpen(false), menuOpen);

  const linkedTaskCount = useMemo(
    () =>
      tasks.filter((t) => {
        const meta = t as Task & { metadata?: Record<string, unknown> };
        return meta.metadata?.keyResultId === keyResult.id;
      }).length,
    [tasks, keyResult.id],
  );

  const handleMetricSave = useCallback(async () => {
    const parsed = Number.parseFloat(editValue);
    if (Number.isNaN(parsed)) {
      setEditValue(String(keyResult.current));
      setEditing(false);
      return;
    }
    setEditing(false);
    await updateMetric({ id: keyResult.id, currentValue: parsed });
    refetchObjectives();
  }, [editValue, keyResult.id, keyResult.current, updateMetric, refetchObjectives]);

  const handleMetricKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleMetricSave();
    if (e.key === "Escape") {
      setEditValue(String(keyResult.current));
      setEditing(false);
    }
  };

  const startEditing = () => {
    setEditValue(String(keyResult.current));
    setEditing(true);
    setTimeout(() => inputRef.current?.select(), 0);
  };

  return (
    <div className="border-l-2 border-border ml-4">
      <div className="flex items-center gap-2 px-3 py-2 hover:bg-accent/50 transition-colors rounded-r-md group">
        {/* Expand toggle */}
        <button
          type="button"
          onClick={() => toggleKr(keyResult.id)}
          className="text-muted-foreground hover:text-foreground"
        >
          {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
        </button>

        {/* Mini progress ring */}
        <ProgressRing progress={keyResult.progress} size="sm" />

        {/* Title */}
        <span className="text-xs text-foreground flex-1 truncate">{keyResult.title}</span>

        {/* Current / Target metric */}
        <div className="flex items-center gap-1 text-[11px] text-muted-foreground">
          {editing ? (
            <input
              ref={inputRef}
              type="number"
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              onBlur={handleMetricSave}
              onKeyDown={handleMetricKeyDown}
              className="w-16 px-1.5 py-0.5 text-[11px] bg-accent border border-border rounded text-foreground focus:outline-none focus:ring-1 focus:ring-brand"
            />
          ) : (
            <button
              type="button"
              onClick={startEditing}
              className="px-1.5 py-0.5 rounded hover:bg-accent transition-colors cursor-text"
              title="Click to edit metric"
            >
              {keyResult.current}
            </button>
          )}
          <span>/</span>
          <span>
            {keyResult.target} {keyResult.unit}
          </span>
        </div>

        {/* Linked tasks badge */}
        <button
          type="button"
          onClick={() => toggleKr(keyResult.id)}
          className="text-2xs px-1.5 py-0.5 rounded-full bg-brand/10 text-brand opacity-0 group-hover:opacity-100 transition-opacity"
        >
          {linkedTaskCount > 0
            ? `${linkedTaskCount} task${linkedTaskCount !== 1 ? "s" : ""}`
            : "Tasks"}
        </button>

        {/* Context menu */}
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen(!menuOpen)}
            className="text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity"
          >
            <MoreHorizontal className="size-3.5" />
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-full mt-1 z-20 glass-panel rounded-lg py-1 min-w-[120px] bg-card border border-border shadow-lg">
              <button
                type="button"
                onClick={() => {
                  setMenuOpen(false);
                  onEdit?.(keyResult);
                }}
                className="w-full px-3 py-1.5 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors flex items-center gap-2"
              >
                <Pencil className="size-3" /> Edit
              </button>
              <button
                type="button"
                onClick={() => {
                  setMenuOpen(false);
                  onDelete?.(keyResult);
                }}
                className="w-full px-3 py-1.5 text-left text-xs text-red-400 hover:text-red-300 hover:bg-accent transition-colors flex items-center gap-2"
              >
                <Trash2 className="size-3" /> Delete
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Expanded linked tasks */}
      {expanded && <LinkedTasksList keyResultId={keyResult.id} />}
    </div>
  );
}
