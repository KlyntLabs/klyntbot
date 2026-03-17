export interface ScopeConfig {
  scopeType: "backlinks" | "semantic" | "project" | "manual";
  radius: number;
  includeCognitive: boolean;
  deepDive: boolean;
}

export const DEFAULT_SCOPE: ScopeConfig = {
  scopeType: "backlinks",
  radius: 0.72,
  includeCognitive: true,
  deepDive: false,
};

interface Props {
  value: ScopeConfig;
  onChange: (config: ScopeConfig) => void;
  onClose: () => void;
}

const SCOPE_TYPES = [
  {
    id: "backlinks" as const,
    label: "Backlinks",
    desc: "Wikilink references",
  },
  {
    id: "semantic" as const,
    label: "Semantic",
    desc: "Similar by embedding",
  },
  { id: "project" as const, label: "Project", desc: "Same notebook" },
  { id: "manual" as const, label: "Manual", desc: "Selected notes" },
];

export function InsightScopePopover({ value, onChange, onClose }: Props) {
  return (
    <div className="absolute right-0 top-full mt-1 z-50 w-64 glass-panel rounded-xl p-3 flex flex-col gap-3 shadow-xl">
      {/* Scope type */}
      <div className="flex flex-col gap-1.5">
        <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Scope
        </label>
        <div className="grid grid-cols-2 gap-1">
          {SCOPE_TYPES.map((st) => (
            <button
              key={st.id}
              type="button"
              onClick={() => onChange({ ...value, scopeType: st.id })}
              className={`px-2 py-1.5 rounded-md text-[10px] text-left transition-colors ${
                value.scopeType === st.id
                  ? "bg-purple/20 text-purple-300 border border-purple/30"
                  : "bg-white/[0.04] text-muted-foreground hover:bg-white/[0.06] border border-transparent"
              }`}
            >
              <div className="font-medium">{st.label}</div>
              <div className="text-[9px] text-dim">{st.desc}</div>
            </button>
          ))}
        </div>
      </div>

      {/* Radius slider (only for semantic) */}
      {value.scopeType === "semantic" && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <label className="text-[10px] font-medium text-muted-foreground">
              Similarity Radius
            </label>
            <span className="text-[10px] text-dim">{value.radius.toFixed(2)}</span>
          </div>
          <input
            type="range"
            min={0.5}
            max={0.95}
            step={0.01}
            value={value.radius}
            onChange={(e) =>
              onChange({
                ...value,
                radius: Number.parseFloat(e.target.value),
              })
            }
            className="w-full accent-purple h-1"
          />
        </div>
      )}

      {/* Toggles */}
      <div className="flex flex-col gap-2 pt-1 border-t border-border">
        <Toggle
          label="Cognitive Context"
          description="Include facts, memories, rules"
          checked={value.includeCognitive}
          onChange={(c) => onChange({ ...value, includeCognitive: c })}
        />
        <Toggle
          label="Deep Dive"
          description="User model + entity graph + history"
          checked={value.deepDive}
          onChange={(d) => onChange({ ...value, deepDive: d })}
        />
      </div>

      {/* Close */}
      <button
        type="button"
        onClick={onClose}
        className="self-end text-[10px] text-dim hover:text-muted-foreground transition-colors"
      >
        Done
      </button>
    </div>
  );
}

function Toggle({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="flex items-center gap-2 text-left"
      role="switch"
      aria-checked={checked}
    >
      <div
        className={`w-7 h-4 rounded-full transition-colors flex items-center px-0.5 ${
          checked ? "bg-purple" : "bg-accent"
        }`}
      >
        <div
          className={`w-3 h-3 rounded-full bg-white transition-transform ${
            checked ? "translate-x-3" : "translate-x-0"
          }`}
        />
      </div>
      <div className="flex flex-col">
        <span className="text-[10px] font-medium text-foreground">{label}</span>
        <span className="text-[9px] text-dim">{description}</span>
      </div>
    </button>
  );
}
