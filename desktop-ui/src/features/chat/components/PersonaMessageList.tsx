import type { PersonaSegment } from "@shared/types";
import { PersonaMessage } from "./PersonaMessage";

interface PersonaMessageListProps {
  personaMessages: PersonaSegment[];
  compact?: boolean;
}

export function PersonaMessageList({ personaMessages, compact }: PersonaMessageListProps) {
  if (personaMessages.length === 0) return null;

  return (
    <div className={compact ? "space-y-1 px-2 py-1" : "glass-card rounded-xl p-3 space-y-1"}>
      {!compact && (
        <div className="text-[9px] text-fg-dim uppercase tracking-wider mb-1">
          Individual Perspectives
        </div>
      )}
      {personaMessages.map((pm) => (
        <PersonaMessage
          key={pm.personaId}
          personaName={pm.personaName}
          personaIcon={pm.personaIcon}
          personaRole={pm.personaRole}
          content={pm.content}
        />
      ))}
    </div>
  );
}
