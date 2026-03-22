import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cn } from "@shared/lib/cn";
import { Sliders } from "lucide-react";

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

export function InsightScopePopover({ value, onChange }: Props) {
  return (
    <PopoverPrimitive.Root>
      <PopoverPrimitive.Trigger asChild>
        <button
          type="button"
          className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          title="Scope Config"
        >
          <Sliders size={12} />
        </button>
      </PopoverPrimitive.Trigger>

      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          align="end"
          sideOffset={6}
          className={cn(
            "z-50 w-64 rounded-lg border border-border bg-popover p-3 text-foreground shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
            "data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2",
          )}
        >
          <div className="flex flex-col gap-3">
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
                    className={cn(
                      "px-2 py-1.5 rounded-md text-[10px] text-left transition-colors border",
                      value.scopeType === st.id
                        ? "bg-accent text-foreground border-border"
                        : "bg-transparent text-muted-foreground hover:bg-accent/50 border-transparent",
                    )}
                  >
                    <div className="font-medium">{st.label}</div>
                    <div className="text-[9px] text-muted">{st.desc}</div>
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
                  <span className="text-[10px] text-muted">{value.radius.toFixed(2)}</span>
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
                  className="w-full accent-brand h-1"
                />
              </div>
            )}

            {/* Toggles */}
            <div className="flex flex-col gap-2 pt-2 border-t border-border">
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
          </div>
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
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
      className="flex items-center gap-2 text-left group"
      role="switch"
      aria-checked={checked}
    >
      <div
        className={cn(
          "w-7 h-4 rounded-full transition-colors flex items-center px-0.5",
          checked ? "bg-brand" : "bg-accent",
        )}
      >
        <div
          className={cn(
            "w-3 h-3 rounded-full bg-white transition-transform",
            checked ? "translate-x-3" : "translate-x-0",
          )}
        />
      </div>
      <div className="flex flex-col">
        <span className="text-[10px] font-medium text-foreground">{label}</span>
        <span className="text-[9px] text-muted">{description}</span>
      </div>
    </button>
  );
}
