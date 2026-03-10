import { LAYERS, type LayerKey } from "@features/dashboard/lib/layers";
import { MiniCalendar } from "@features/tasks/components/editors/MiniCalendar";
import { useClickOutside } from "@shared/hooks/useClickOutside";
import { useMutation } from "@shared/hooks/useMutation";
import { toLocalISO } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { Project, ProjectUpdateParams } from "@shared/types";
import {
  Archive,
  ArrowLeft,
  Calendar,
  ChevronLeft,
  ChevronRight,
  Layers,
  PanelRight,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
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

function formatDateDisplay(date: string): string {
  const d = new Date(`${date}T00:00:00`);
  return d.toLocaleDateString("en-US", {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

interface ProjectDetailHeaderProps {
  project: Project;
  date: string;
  onDateChange: (date: string) => void;
  layersEnabled: Set<LayerKey>;
  onToggleLayer: (key: LayerKey) => void;
  onResetLayers: () => void;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
}

export function ProjectDetailHeader({
  project,
  date,
  onDateChange,
  layersEnabled,
  onToggleLayer,
  onResetLayers,
  sidebarOpen,
  onToggleSidebar,
}: ProjectDetailHeaderProps) {
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

  const navigateBy = (dir: 1 | -1) => {
    const d = new Date(`${date}T00:00:00`);
    d.setDate(d.getDate() + dir);
    onDateChange(toLocalISO(d));
  };

  // Layers dropdown
  const [layersOpen, setLayersOpen] = useState(false);
  const layersTriggerRef = useRef<HTMLButtonElement>(null);
  const layersDropdownRef = useRef<HTMLDivElement>(null);
  const [layersPos, setLayersPos] = useState({ top: 0, right: 0 });
  useClickOutside(layersDropdownRef, () => setLayersOpen(false), layersOpen);

  const updateLayersPos = useCallback(() => {
    if (!layersTriggerRef.current) return;
    const rect = layersTriggerRef.current.getBoundingClientRect();
    setLayersPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!layersOpen) return;
    updateLayersPos();
    window.addEventListener("resize", updateLayersPos);
    return () => window.removeEventListener("resize", updateLayersPos);
  }, [layersOpen, updateLayersPos]);

  // Calendar picker
  const [calOpen, setCalOpen] = useState(false);
  const calTriggerRef = useRef<HTMLButtonElement>(null);
  const calDropdownRef = useRef<HTMLDivElement>(null);
  const [calPos, setCalPos] = useState({ top: 0, right: 0 });
  useClickOutside(calDropdownRef, () => setCalOpen(false), calOpen);

  const updateCalPos = useCallback(() => {
    if (!calTriggerRef.current) return;
    const rect = calTriggerRef.current.getBoundingClientRect();
    setCalPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!calOpen) return;
    updateCalPos();
    window.addEventListener("resize", updateCalPos);
    return () => window.removeEventListener("resize", updateCalPos);
  }, [calOpen, updateCalPos]);

  return (
    <div className="glass-card px-4 py-2 flex items-center gap-3 shrink-0">
      {/* Back + project identity */}
      <button
        type="button"
        onClick={() => navigate("/")}
        className="text-muted hover:text-secondary transition-colors"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>

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
          className="text-sm font-medium text-primary bg-transparent border-b border-brand outline-none"
        />
      ) : (
        <button
          type="button"
          onClick={() => {
            setNameDraft(project.name);
            setEditingName(true);
          }}
          className="text-sm font-medium text-primary cursor-text hover:text-secondary transition-colors"
        >
          {project.name}
        </button>
      )}

      {/* Separator */}
      <div className="w-px h-4 bg-white/[0.08]" />

      {/* Date label */}
      <span className="text-sm font-medium text-primary whitespace-nowrap">
        {formatDateDisplay(date)}
      </span>

      {/* Layers toggle */}
      <button
        ref={layersTriggerRef}
        type="button"
        onClick={() => setLayersOpen(!layersOpen)}
        className={cn(
          "p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors",
          layersOpen && "bg-white/[0.08] text-secondary",
        )}
        title="Toggle layers"
      >
        <Layers className="w-4 h-4" />
      </button>

      {/* Date nav */}
      <div className="flex items-center rounded-full bg-white/[0.06] p-0.5 ml-auto">
        <button
          type="button"
          onClick={() => navigateBy(-1)}
          className="p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <button
          ref={calTriggerRef}
          type="button"
          onClick={() => setCalOpen(!calOpen)}
          className={cn(
            "p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors",
            calOpen && "bg-white/[0.08] text-secondary",
          )}
          title="Pick date"
        >
          <Calendar className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={() => navigateBy(1)}
          className="p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>

      {/* Sidebar toggle */}
      <button
        type="button"
        onClick={onToggleSidebar}
        className={cn(
          "p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors",
          sidebarOpen && "bg-white/[0.08] text-secondary",
        )}
        title={sidebarOpen ? "Hide summary" : "Show summary"}
      >
        <PanelRight className="w-4 h-4" />
      </button>

      {/* Archive */}
      <button
        type="button"
        onClick={handleArchive}
        onBlur={() => setConfirmArchive(false)}
        className={cn(
          "flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[11px] font-light transition-colors",
          confirmArchive
            ? "bg-destructive text-white"
            : "text-muted hover:text-secondary hover:bg-white/[0.04]",
        )}
      >
        <Archive className="w-3.5 h-3.5" strokeWidth={1.5} />
        {confirmArchive ? "Click again" : "Archive"}
      </button>

      {/* Layers dropdown portal */}
      {layersOpen &&
        createPortal(
          <div
            ref={layersDropdownRef}
            className="fixed z-[9999] glass-dropdown py-2 min-w-[180px]"
            style={{ top: layersPos.top, right: layersPos.right }}
          >
            {LAYERS.map((layer) => (
              <label
                key={layer.key}
                className="flex items-center gap-2 px-3 py-1.5 text-xs cursor-pointer rounded-lg transition-colors text-secondary hover:bg-white/[0.06]"
              >
                <input
                  type="checkbox"
                  checked={layersEnabled.has(layer.key)}
                  onChange={() => onToggleLayer(layer.key)}
                  className="accent-brand w-3 h-3"
                />
                <span className="w-2 h-2 rounded-full" style={{ backgroundColor: layer.color }} />
                {layer.label}
              </label>
            ))}
            <button
              type="button"
              onClick={onResetLayers}
              className="w-full text-left mt-1 px-3 py-1.5 text-[11px] text-muted hover:text-secondary rounded-lg hover:bg-white/[0.06] transition-colors"
            >
              Reset to defaults
            </button>
          </div>,
          document.body,
        )}

      {/* Calendar picker portal */}
      {calOpen &&
        createPortal(
          <div
            ref={calDropdownRef}
            className="fixed z-[9999] glass-dropdown"
            style={{ top: calPos.top, right: calPos.right }}
          >
            <MiniCalendar
              value={date}
              onSelect={(iso) => {
                onDateChange(iso);
                setCalOpen(false);
              }}
              showShortcuts={false}
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
