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
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-muted">{label}</span>
        <span className="text-[10px] text-dim tabular-nums">
          {value}
          {unit}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1 bg-surface-raised rounded-full appearance-none cursor-pointer accent-brand [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-brand"
      />
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
    <label className="flex items-center justify-between cursor-pointer">
      <span className="text-[10px] text-muted">{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative w-7 h-4 rounded-full transition-colors ${
          checked ? "bg-brand" : "bg-surface-raised"
        }`}
      >
        <span
          className={`absolute top-0.5 w-3 h-3 rounded-full bg-white transition-transform ${
            checked ? "translate-x-3.5" : "translate-x-0.5"
          }`}
        />
      </button>
    </label>
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
    <div className="w-[220px] space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium text-primary">Graph Settings</span>
        {!isDefault && (
          <button
            type="button"
            onClick={onReset}
            className="flex items-center gap-1 text-[10px] text-muted hover:text-secondary transition-colors"
          >
            <RotateCcw size={10} />
            Reset
          </button>
        )}
      </div>

      <div className="space-y-2.5">
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
          label="Label Zoom Threshold"
          value={settings.labelThreshold}
          min={0.1}
          max={1.5}
          step={0.1}
          unit="×"
          onChange={(v) => onChange({ labelThreshold: v })}
        />
      </div>

      <div className="border-t border-border-subtle pt-2 space-y-2">
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
      </div>
    </div>
  );
}
