import { Filter, ChevronDown, Plus, LayoutGrid, List, GitBranch } from 'lucide-react';
import type { ViewMode } from '../../lib/types';

interface ToolbarProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
}

function FilterButton({ label }: { label: string }) {
  return (
    <button className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md bg-[rgba(255,255,255,0.03)] hover:bg-[rgba(255,255,255,0.05)] text-[#8B949E] hover:text-[#C9D1D9] transition-colors">
      <span className="text-[11px] font-light">{label}</span>
      <ChevronDown className="w-[12px] h-[12px]" strokeWidth={1.5} />
    </button>
  );
}

export function Toolbar({ viewMode, onViewModeChange }: ToolbarProps) {
  return (
    <div className="flex items-center justify-between mb-3.5">
      {/* Left: Filters */}
      <div className="flex items-center gap-2">
        <button className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-[rgba(255,255,255,0.03)] hover:bg-[rgba(255,255,255,0.05)] text-[#8B949E] hover:text-[#C9D1D9] transition-colors">
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
        <div className="flex items-center bg-[rgba(255,255,255,0.03)] rounded-md p-0.5">
          {([
            { mode: 'table' as const, icon: List },
            { mode: 'board' as const, icon: LayoutGrid },
            { mode: 'tree' as const, icon: GitBranch },
          ]).map(({ mode, icon: Icon }) => (
            <button
              key={mode}
              onClick={() => onViewModeChange(mode)}
              className={`p-1.5 rounded transition-colors ${
                viewMode === mode
                  ? 'bg-[rgba(255,255,255,0.08)] text-[#F97316]'
                  : 'text-[#8B949E] hover:text-[#C9D1D9]'
              }`}
            >
              <Icon className="w-[14px] h-[14px]" strokeWidth={1.5} />
            </button>
          ))}
        </div>

        <button className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-[#F97316] hover:bg-[#ea6a0f] text-white transition-colors">
          <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} />
          <span className="text-[12px] font-light">Add task</span>
        </button>
      </div>
    </div>
  );
}
