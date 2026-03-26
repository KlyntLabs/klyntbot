import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cn } from "@shared/lib/utils";
import { Link, NotebookText, Sliders } from "lucide-react";

export interface ScopeConfig {
  scopeType: "backlinks" | "notebook" | "semantic" | "project" | "manual";
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
    label: "Linked",
    desc: "Notes that link to this one",
    icon: Link,
  },
  {
    id: "notebook" as const,
    label: "Notebook",
    desc: "All notes in this notebook tree",
    icon: NotebookText,
  },
];

export function InsightScopePopover({ value, onChange }: Props) {
  const activeScope = SCOPE_TYPES.find((s) => s.id === value.scopeType) ?? SCOPE_TYPES[0];

  return (
    <PopoverPrimitive.Root>
      <PopoverPrimitive.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 px-1.5 py-0.5 rounded-md text-2xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          title="Scope Config"
        >
          <Sliders size={10} />
          <span>{activeScope.label}</span>
        </button>
      </PopoverPrimitive.Trigger>

      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          align="start"
          sideOffset={6}
          className={cn(
            "z-50 w-56 rounded-lg border border-border bg-popover p-3 text-foreground shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
            "data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2",
          )}
        >
          <div className="flex flex-col gap-3">
            {/* Scope type */}
            <div className="flex flex-col gap-1.5">
              <span className="text-2xs font-medium text-muted-foreground uppercase tracking-wider">
                Context scope
              </span>
              <div className="flex flex-col gap-1">
                {SCOPE_TYPES.map((st) => {
                  const Icon = st.icon;
                  return (
                    <button
                      key={st.id}
                      type="button"
                      onClick={() => onChange({ ...value, scopeType: st.id })}
                      className={cn(
                        "flex items-center gap-2.5 px-2.5 py-2 rounded-md text-2xs text-left transition-colors border",
                        value.scopeType === st.id
                          ? "bg-accent text-foreground border-border"
                          : "bg-transparent text-muted-foreground hover:bg-accent/50 border-transparent",
                      )}
                    >
                      <Icon size={14} className="shrink-0 text-muted-foreground" />
                      <div>
                        <div className="font-medium">{st.label}</div>
                        <div className="text-[9px] text-muted">{st.desc}</div>
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>

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
            "size-3 rounded-full bg-white transition-transform",
            checked ? "translate-x-3" : "translate-x-0",
          )}
        />
      </div>
      <div className="flex flex-col">
        <span className="text-2xs font-medium text-foreground">{label}</span>
        <span className="text-[9px] text-muted">{description}</span>
      </div>
    </button>
  );
}
