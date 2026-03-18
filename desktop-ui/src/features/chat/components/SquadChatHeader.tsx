import { useSquads } from "../../notes/hooks/useSquads";

interface SquadChatHeaderProps {
  squadId: string;
}

export function SquadChatHeader({ squadId }: SquadChatHeaderProps) {
  const { squads } = useSquads();
  const squad = squads.find((s) => s.id === squadId);

  if (!squad) return null;

  return (
    <div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-white/[0.02]">
      <span className="text-sm">{squad.icon}</span>
      <div className="flex-1 min-w-0">
        <div className="text-[11px] font-medium text-foreground">{squad.name}</div>
        <div className="text-[9px] text-dim">
          {squad.members.length} {squad.members.length === 1 ? "member" : "members"}
        </div>
      </div>
      <div className="flex -space-x-1">
        {squad.members.slice(0, 5).map((m) => (
          <span key={m.personaId} className="text-xs" title={m.personaName}>
            {m.personaIcon}
          </span>
        ))}
      </div>
    </div>
  );
}
