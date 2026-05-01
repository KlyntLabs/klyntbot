import {
  ChevronsDownUp,
  ChevronsUpDown,
  Search,
  ChevronUp,
  ChevronDown,
  X,
  AlertCircle,
} from "lucide-react";
import { eventChipColor } from "./eventHelpers";

interface FilterPreset {
  label: string;
  types: Set<string>;
  errorsOnly: boolean;
}

const FILTER_PRESETS: FilterPreset[] = [
  { label: "All Events", types: new Set(), errorsOnly: false },
  { label: "Errors Only", types: new Set(), errorsOnly: true },
  { label: "Tool Calls", types: new Set(["toolCall", "toolResult", "toolCallPart"]), errorsOnly: false },
  { label: "Thinking", types: new Set(["thinkPart", "textPart"]), errorsOnly: false },
  { label: "Approvals", types: new Set(["approvalRequest", "approvalResponse"]), errorsOnly: false },
  { label: "Sub-agents", types: new Set(["subagentEvent"]), errorsOnly: false },
];

function setsEqual(a: Set<string>, b: Set<string>): boolean {
  if (a.size !== b.size) return false;
  for (const item of a) {
    if (!b.has(item)) return false;
  }
  return true;
}

interface WireFiltersProps {
  allTypes: string[];
  selectedTypes: Set<string>;
  onToggle: (kind: string) => void;
  total: number;
  filteredCount: number;
  allExpanded: boolean;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  searchQuery: string;
  onSearchChange: (s: string) => void;
  searchMatchCount: number;
  errorCount: number;
  errorsOnly: boolean;
  onToggleErrorsOnly: () => void;
  onNextError: () => void;
  onPrevError: () => void;
  onApplyPreset?: (types: Set<string>, errorsOnly: boolean) => void;
}

export function WireFilters({
  allTypes,
  selectedTypes,
  onToggle,
  total,
  filteredCount,
  allExpanded,
  onExpandAll,
  onCollapseAll,
  searchQuery,
  onSearchChange,
  searchMatchCount,
  errorCount,
  errorsOnly,
  onToggleErrorsOnly,
  onNextError,
  onPrevError,
  onApplyPreset,
}: WireFiltersProps) {
  const activePresetIndex = FILTER_PRESETS.findIndex(
    (p) => setsEqual(p.types, selectedTypes) && p.errorsOnly === errorsOnly,
  );

  const allActive = selectedTypes.size === 0 && !errorsOnly && !searchQuery;

  return (
    <div className="cm-filters">
      {/* Row 0: Presets */}
      {onApplyPreset && (
        <div className="cm-filters__presets">
          {FILTER_PRESETS.map((preset, i) => (
            <button
              key={preset.label}
              type="button"
              onClick={() => onApplyPreset(preset.types, preset.errorsOnly)}
              className={
                "cm-preset" +
                (activePresetIndex === i ? " cm-preset--active" : "")
              }
            >
              {preset.label}
            </button>
          ))}
        </div>
      )}

      {/* Row 1: Search + controls */}
      <div className="cm-filters__row">
        <div className="cm-search">
          <Search size={13} className="cm-search__icon" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Search events..."
            data-wire-search
            className="cm-search__input"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => onSearchChange("")}
              className="cm-search__clear"
            >
              <X size={12} />
            </button>
          )}
        </div>
        {searchQuery && (
          <span className="cm-filters__matches">{searchMatchCount} matches</span>
        )}

        <div className="cm-filters__divider" />

        <button
          type="button"
          onClick={onToggleErrorsOnly}
          className={
            "cm-filters__errors" +
            (errorsOnly ? " cm-filters__errors--active" : "")
          }
          title="Show errors only"
        >
          <AlertCircle size={12} />
          {errorCount} errors
        </button>
        {errorCount > 0 && (
          <>
            <button type="button" onClick={onPrevError} className="cm-filters__nav-btn" title="Previous error">
              <ChevronUp size={14} />
            </button>
            <button type="button" onClick={onNextError} className="cm-filters__nav-btn" title="Next error">
              <ChevronDown size={14} />
            </button>
          </>
        )}

        <div className="cm-filters__divider" />

        <span className="cm-filters__count">
          {allActive ? `${total}` : `${filteredCount} / ${total}`}
        </span>
        <button
          type="button"
          onClick={allExpanded ? onCollapseAll : onExpandAll}
          className="cm-filters__expand"
          title={allExpanded ? "Collapse all" : "Expand all"}
        >
          {allExpanded ? <ChevronsDownUp size={12} /> : <ChevronsUpDown size={12} />}
          {allExpanded ? "Collapse" : "Expand"}
        </button>
      </div>

      {/* Row 2: Type chips */}
      <div className="cm-filters__chips">
        {allTypes.map((type) => {
          const active = selectedTypes.has(type);
          const color = eventChipColor(type);
          const noneSelected = selectedTypes.size === 0;
          return (
            <button
              key={type}
              type="button"
              onClick={() => onToggle(type)}
              className={`cm-event-chip cm-event-chip--${color}`}
              aria-pressed={active || noneSelected}
            >
              {type}
            </button>
          );
        })}
      </div>
    </div>
  );
}
