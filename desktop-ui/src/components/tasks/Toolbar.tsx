import { ChevronDown, Columns3, Filter, GitBranch, List, Plus } from "lucide-react";
import type { ViewMode } from "../../lib/types";

interface ToolbarProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  onAddTask?: () => void;
}

function FilterButton({ label }: { label: string }) {
  return (
    <button
      type="button"
      className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md bg-surface-low hover:bg-surface-base text-muted hover:text-secondary transition-colors"
    >
      <span className="text-[11px] font-light">{label}</span>
      <ChevronDown className="w-[12px] h-[12px]" strokeWidth={1.5} />
    </button>
  );
}

export function Toolbar({ viewMode, onViewModeChange, onAddTask }: ToolbarProps) {
  return (
    <div className="flex items-center justify-between mb-3.5">
      {/* Left: Filters */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-surface-low hover:bg-surface-base text-muted hover:text-secondary transition-colors"
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

      {/* Right: View toggles + Add task */}
      <div className="flex items-center gap-2">
        <div className="flex items-center bg-surface-low rounded-md p-0.5">
          {[
            { mode: "table" as const, icon: List },
            { mode: "board" as const, icon: Columns3 },
            { mode: "tree" as const, icon: GitBranch },
          ].map(({ mode, icon: Icon }) => (
            <button
              type="button"
              key={mode}
              onClick={() => onViewModeChange(mode)}
              aria-label={`${mode} view`}
              className={`p-1.5 rounded transition-colors ${
                viewMode === mode
                  ? "bg-surface-highest text-brand"
                  : "text-muted hover:text-secondary"
              }`}
            >
              <Icon className="w-[14px] h-[14px]" strokeWidth={1.5} />
            </button>
          ))}
        </div>

        <button
          type="button"
          onClick={onAddTask}
          className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-brand hover:bg-brand-hover text-white transition-colors"
        >
          <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} />
          <span className="text-[12px] font-light">Add task</span>
        </button>
      </div>
    </div>
  );
}
