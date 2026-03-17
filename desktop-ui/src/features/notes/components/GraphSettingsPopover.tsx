import { RotateCcw } from "lucide-react";
import type { GraphSettings } from "../hooks/useGraphSettings";

interface GraphSettingsPopoverProps {
  settings: GraphSettings;
  defaults: GraphSettings;
  onChange: (partial: Partial<GraphSettings>) => void;
  onReset: () => void;
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  unit,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;

  return (
    <div className="flex items-center gap-3 h-7">
      <span className="text-[11px] text-secondary w-[90px] shrink-0">{label}</span>
      <div className="flex-1 relative flex items-center h-5">
        {/* Custom track with progress fill */}
        <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 h-[4px] rounded-full bg-surface-raised overflow-hidden">
          <div
            className="h-full rounded-full bg-brand/50"
            style={{ width: `${pct}%` }}
          />
        </div>
        {/* Native range input (invisible track, visible thumb) */}
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          style={{ WebkitAppearance: "none", appearance: "none", background: "transparent" }}
          className="relative z-10 w-full h-5 cursor-pointer outline-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-brand [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-background"
        />
      </div>
      <span className="text-[10px] text-muted tabular-nums w-[36px] text-right shrink-0">
        {value}{unit}
      </span>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-3 h-7">
      <span className="text-[11px] text-secondary flex-1">{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative w-[34px] h-[18px] rounded-full transition-colors shrink-0 ${
          checked ? "bg-brand" : "bg-surface-raised"
        }`}
      >
        <span
          className={`absolute top-[3px] w-3 h-3 rounded-full bg-background transition-all ${
            checked ? "left-[19px]" : "left-[3px]"
          }`}
        />
      </button>
    </div>
  );
}

export function GraphSettingsPopover({
  settings,
  defaults,
  onChange,
  onReset,
}: GraphSettingsPopoverProps) {
  const isDefault =
    settings.linkDistance === defaults.linkDistance &&
    settings.repulsion === defaults.repulsion &&
    settings.centerForce === defaults.centerForce &&
    settings.nodeScale === defaults.nodeScale;

  return (
    <div className="w-[280px]">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] font-semibold text-muted uppercase tracking-wider">
          Settings
        </span>
        {!isDefault && (
          <button
            type="button"
            onClick={onReset}
            className="flex items-center gap-1 text-[10px] text-muted hover:text-secondary transition-colors"
          >
            <RotateCcw size={9} />
            Reset
          </button>
        )}
      </div>

      {/* Sliders */}
      <div className="space-y-0.5">
        <Slider
          label="Link Distance"
          value={settings.linkDistance}
          min={40}
          max={300}
          step={10}
          unit="px"
          onChange={(v) => onChange({ linkDistance: v })}
        />
        <Slider
          label="Repulsion"
          value={settings.repulsion}
          min={1000}
          max={30000}
          step={500}
          onChange={(v) => onChange({ repulsion: v })}
        />
        <Slider
          label="Center Force"
          value={settings.centerForce}
          min={0}
          max={1}
          step={0.05}
          onChange={(v) => onChange({ centerForce: v })}
        />
        <Slider
          label="Node Size"
          value={settings.nodeScale}
          min={0.5}
          max={2}
          step={0.1}
          unit="×"
          onChange={(v) => onChange({ nodeScale: v })}
        />
        <Slider
          label="Label Threshold"
          value={settings.labelThreshold}
          min={0.1}
          max={1.5}
          step={0.1}
          unit="×"
          onChange={(v) => onChange({ labelThreshold: v })}
        />
      </div>

      {/* Toggles */}
      <div className="mt-2 pt-2 border-t border-border-subtle space-y-0.5">
        <Toggle
          label="Show Arrows"
          checked={settings.showArrows}
          onChange={(v) => onChange({ showArrows: v })}
        />
        <Toggle
          label="Show Orphan Nodes"
          checked={settings.showOrphans}
          onChange={(v) => onChange({ showOrphans: v })}
        />
        <Toggle
          label="Instant Load"
          checked={settings.instantLoad}
          onChange={(v) => onChange({ instantLoad: v })}
        />
      </div>
    </div>
  );
}
