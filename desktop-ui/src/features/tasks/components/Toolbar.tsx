import type { ViewMode } from "@shared/types";
import { ChevronDown, Filter, GitBranch, List, Plus } from "lucide-react";
import type { ColumnId } from "../hooks/useColumnVisibility";
import { ColumnPicker } from "./ColumnPicker";

interface ToolbarProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  onAddTask?: () => void;
  visibleColumns: Set<ColumnId>;
  onToggleColumn: (id: ColumnId) => void;
  onResetColumns: () => void;
}

function FilterButton({ label }: { label: string }) {
  return (
    <button
      type="button"
      className="glass-button flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-muted hover:text-secondary"
    >
      <span className="text-[11px] font-light">{label}</span>
      <ChevronDown className="w-[12px] h-[12px]" strokeWidth={1.5} />
    </button>
  );
}

export function Toolbar({
  viewMode,
  onViewModeChange,
  onAddTask,
  visibleColumns,
  onToggleColumn,
  onResetColumns,
}: ToolbarProps) {
  return (
    <div className="flex items-center justify-between mb-3.5">
      {/* Left: Filters */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="glass-button flex items-center gap-2 px-3 py-1.5 rounded-lg text-muted hover:text-secondary"
        >
          <Filter className="w-[14px] h-[14px]" strokeWidth={1.5} />
          <span className="text-[12px] font-light">Filter</span>
        </button>
        <FilterButton label="Project" />
        <FilterButton label="Priority" />
        <FilterButton label="Status" />
        <FilterButton label="Tag" />
        <FilterButton label="OKR" />
      </div>

      {/* Right: View toggles + Column picker + Add task */}
      <div className="flex items-center gap-2">
        <div className="flex items-center glass-button rounded-lg p-0.5">
          {[
            { mode: "table" as const, icon: List },
            {
              mode: "board" as const,
              icon: () => (
                <svg
                  className="w-[14px] h-[14px]"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={1.5}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  role="img"
                >
                  <title>Board view</title>
                  <rect x="3" y="3" width="7" height="18" rx="1" />
                  <rect x="14" y="3" width="7" height="12" rx="1" />
                </svg>
              ),
            },
            { mode: "tree" as const, icon: GitBranch },
          ].map(({ mode, icon: Icon }) => (
            <button
              type="button"
              key={mode}
              onClick={() => onViewModeChange(mode)}
              aria-label={`${mode} view`}
              className={`p-1.5 rounded-md transition-all ${
                viewMode === mode
                  ? "glass-button-active text-brand"
                  : "text-muted hover:text-secondary"
              }`}
            >
              <Icon className="w-[14px] h-[14px]" strokeWidth={1.5} />
            </button>
          ))}
        </div>

        {viewMode === "table" && (
          <ColumnPicker
            visibleColumns={visibleColumns}
            onToggle={onToggleColumn}
            onReset={onResetColumns}
          />
        )}

        <button
          type="button"
          onClick={onAddTask}
          className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-brand hover:bg-brand-hover text-white transition-colors"
        >
          <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} />
          <span className="text-[12px] font-light">Add task</span>
        </button>
      </div>
    </div>
  );
}
