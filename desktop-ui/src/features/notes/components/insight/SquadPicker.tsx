import { useClickOutside } from "@shared/hooks/useClickOutside";
import { ChevronDown, Settings } from "lucide-react";
import { useRef, useState } from "react";
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
  const [open, setOpen] = useState(!!inline);
  const containerRef = useRef<HTMLDivElement>(null);

  useClickOutside(containerRef, () => setOpen(false), open && !inline);

  const selected = squads.find((s) => s.id === selectedSquadId);

  // Inline mode: render dropdown directly, no trigger button
  if (inline) {
    return (
      <div ref={containerRef} className="w-64 glass-dropdown rounded-xl p-2 flex flex-col gap-1">
        {squads.length === 0 && !loading && (
          <div className="px-2 py-3 text-[11px] text-dim italic text-center">
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
              className="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] text-dim hover:text-muted-foreground hover:bg-white/[0.04] transition-colors"
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
    <div ref={containerRef} className="relative">
      {/* Trigger */}
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-white/[0.04] hover:bg-white/[0.06] border border-transparent hover:border-border transition-colors text-[11px]"
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
          className={`text-dim transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {/* Dropdown */}
      {open && (
        <div className="absolute left-0 top-full mt-1 z-50 w-64 glass-dropdown rounded-xl p-2 flex flex-col gap-1">
          {squads.length === 0 && !loading && (
            <div className="px-2 py-3 text-[11px] text-dim italic text-center">
              No squads created yet
            </div>
          )}

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

          {/* Manage squads button */}
          {onManage && (
            <>
              <div className="border-t border-border my-1" />
              <button
                type="button"
                onClick={() => {
                  onManage();
                  setOpen(false);
                }}
                className="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] text-dim hover:text-muted-foreground hover:bg-white/[0.04] transition-colors"
              >
                <Settings size={10} />
                Manage Squads
              </button>
            </>
          )}
        </div>
      )}
    </div>
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
      className={`flex items-start gap-2 px-2 py-1.5 rounded-md text-left transition-colors w-full ${
        isSelected
          ? "bg-purple/20 text-purple-300 border border-purple/30"
          : "bg-transparent text-muted-foreground hover:bg-white/[0.06] border border-transparent"
      }`}
    >
      <span className="text-sm leading-none mt-0.5 shrink-0">{squad.icon}</span>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium truncate">{squad.name}</div>
        <div className="text-[9px] text-dim">{memberLabel(squad.members.length)}</div>
        {/* Member icon chips */}
        {squad.members.length > 0 && (
          <div className="flex items-center gap-0.5 mt-1 flex-wrap">
            {squad.members.slice(0, 5).map((m) => (
              <span
                key={m.personaId}
                className="w-4 h-4 rounded-full bg-white/[0.06] flex items-center justify-center text-[8px]"
                title={m.personaName}
              >
                {m.personaIcon}
              </span>
            ))}
            {squad.members.length > 5 && (
              <span className="text-[8px] text-dim ml-0.5">+{squad.members.length - 5}</span>
            )}
          </div>
        )}
      </div>
    </button>
  );
}
