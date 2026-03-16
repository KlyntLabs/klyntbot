import { Search } from "lucide-react";
import type { SmartView } from "../hooks/useGraphData";

interface GraphToolbarProps {
  view: SmartView;
  onViewChange: (view: SmartView) => void;
  hopRadius: number;
  onHopRadiusChange: (r: number) => void;
  searchQuery: string;
  onSearchChange: (q: string) => void;
}

const VIEW_OPTIONS: { value: SmartView; label: string }[] = [
  { value: "local", label: "Local" },
  { value: "full", label: "Full" },
  { value: "by-tag", label: "By Tag" },
  { value: "by-notebook", label: "By Notebook" },
  { value: "orphans", label: "Orphans" },
];

export function GraphToolbar({
  view,
  onViewChange,
  hopRadius,
  onHopRadiusChange,
  searchQuery,
  onSearchChange,
}: GraphToolbarProps) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 shrink-0">
      {/* Smart view pills */}
      <div className="flex items-center gap-0.5 bg-white/[0.04] rounded-lg p-0.5">
        {VIEW_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onViewChange(opt.value)}
            className={`px-2.5 py-1 text-xs rounded-md transition-all ${
              view === opt.value
                ? "bg-brand/20 text-brand font-medium shadow-sm"
                : "text-muted hover:text-secondary hover:bg-white/[0.06]"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Hop radius selector (only for local view) */}
      {view === "local" && (
        <div className="flex items-center gap-1 text-xs text-muted">
          <span>Hops:</span>
          <div className="flex items-center gap-0.5 bg-white/[0.04] rounded-lg p-0.5">
            {[1, 2, 3].map((r) => (
              <button
                key={r}
                type="button"
                onClick={() => onHopRadiusChange(r)}
                className={`w-6 h-6 rounded-md text-xs flex items-center justify-center transition-all ${
                  hopRadius === r
                    ? "bg-brand/20 text-brand font-medium"
                    : "text-muted hover:text-secondary hover:bg-white/[0.06]"
                }`}
              >
                {r}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Search input */}
      <div className="relative">
        <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-dim pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Filter nodes..."
          className="w-40 pl-7 pr-2 py-1 text-xs rounded-lg bg-white/[0.04] border border-white/[0.06] text-primary placeholder:text-dim outline-none focus:border-brand/40 transition-colors"
        />
      </div>
    </div>
  );
}
