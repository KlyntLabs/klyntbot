import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cn } from "@shared/lib/utils";
import { ChevronDown, Settings } from "lucide-react";
import { useState } from "react";
import type { Squad } from "../../hooks/useSquads";
import { useSquads } from "../../hooks/useSquads";

const memberLabel = (n: number) => `${n} ${n === 1 ? "member" : "members"}`;

interface SquadPickerProps {
  selectedSquadId: string | null;
  onSelect: (squadId: string) => void;
  onManage?: () => void;
  /** When true, renders the dropdown directly without a trigger button.
   *  Useful when the parent already provides the trigger (e.g. "New squad chat" button). */
  inline?: boolean;
}

export function SquadPicker({ selectedSquadId, onSelect, onManage, inline }: SquadPickerProps) {
  const { squads, loading } = useSquads();
  const [open, setOpen] = useState(false);

  const selected = squads.find((s) => s.id === selectedSquadId);

  // Inline mode: render dropdown directly, no trigger button
  if (inline) {
    return (
      <div className="w-64 rounded-lg border border-border bg-popover p-2 flex flex-col gap-1">
        {squads.length === 0 && !loading && (
          <div className="px-2 py-3 text-[11px] text-muted italic text-center">
            No squads created yet
          </div>
        )}
        {squads.map((squad) => (
          <SquadOption
            key={squad.id}
            squad={squad}
            isSelected={squad.id === selectedSquadId}
            onSelect={() => onSelect(squad.id)}
          />
        ))}
        {onManage && (
          <>
            <div className="border-t border-border my-1" />
            <button
              type="button"
              onClick={onManage}
              className="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] text-muted hover:text-foreground hover:bg-accent transition-colors"
            >
              <Settings size={10} />
              Manage Squads
            </button>
          </>
        )}
      </div>
    );
  }

  return (
    <PopoverPrimitive.Root open={open} onOpenChange={setOpen}>
      <PopoverPrimitive.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1.5 px-2 py-1 rounded-md hover:bg-accent border border-transparent hover:border-border transition-colors text-[11px]"
        >
          {loading && selectedSquadId ? (
            <span className="text-[10px] text-muted-foreground">Loading...</span>
          ) : selected ? (
            <>
              <span>{selected.icon}</span>
              <span className="text-muted-foreground truncate max-w-[120px]">{selected.name}</span>
            </>
          ) : (
            <span className="text-[10px] text-muted-foreground">Select squad</span>
          )}
          <ChevronDown
            size={10}
            className={cn("text-muted transition-transform", open && "rotate-180")}
          />
        </button>
      </PopoverPrimitive.Trigger>

      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          align="end"
          sideOffset={6}
          collisionPadding={8}
          className={cn(
            "z-50 w-64 rounded-lg border border-border bg-popover p-2 shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
            "data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2",
          )}
        >
          {squads.length === 0 && !loading && (
            <div className="px-2 py-3 text-[11px] text-muted italic text-center">
              No squads created yet
            </div>
          )}

          <div className="flex flex-col gap-1 max-h-[280px] overflow-y-auto">
            {squads.map((squad) => (
              <SquadOption
                key={squad.id}
                squad={squad}
                isSelected={squad.id === selectedSquadId}
                onSelect={() => {
                  onSelect(squad.id);
                  setOpen(false);
                }}
              />
            ))}
          </div>

          {onManage && (
            <>
              <div className="border-t border-border my-1" />
              <button
                type="button"
                onClick={() => {
                  onManage();
                  setOpen(false);
                }}
                className="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] text-muted hover:text-foreground hover:bg-accent transition-colors w-full"
              >
                <Settings size={10} />
                Manage Squads
              </button>
            </>
          )}
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}

function SquadOption({
  squad,
  isSelected,
  onSelect,
}: {
  squad: Squad;
  isSelected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "flex items-start gap-2 px-2 py-1.5 rounded-md text-left transition-colors w-full border",
        isSelected
          ? "bg-accent text-foreground border-border"
          : "bg-transparent text-muted-foreground hover:bg-accent/50 border-transparent",
      )}
    >
      <span className="text-sm leading-none mt-0.5 shrink-0">{squad.icon}</span>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium truncate">{squad.name}</div>
        <div className="text-[9px] text-muted">{memberLabel(squad.members.length)}</div>
        {squad.members.length > 0 && (
          <div className="flex items-center gap-0.5 mt-1 flex-wrap">
            {squad.members.slice(0, 5).map((m) => (
              <span
                key={m.personaId}
                className="w-4 h-4 rounded-full bg-accent flex items-center justify-center text-[8px]"
                title={m.personaName}
              >
                {m.personaIcon}
              </span>
            ))}
            {squad.members.length > 5 && (
              <span className="text-[8px] text-muted ml-0.5">+{squad.members.length - 5}</span>
            )}
          </div>
        )}
      </div>
    </button>
  );
}
