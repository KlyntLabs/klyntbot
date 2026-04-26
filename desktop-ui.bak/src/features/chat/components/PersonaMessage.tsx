interface PersonaMessageProps {
  personaName: string;
  personaIcon?: string;
  personaRole?: string;
  content: string;
}

export function PersonaMessage({
  personaName,
  personaIcon,
  personaRole,
  content,
}: PersonaMessageProps) {
  return (
    <div className="flex gap-2 py-2">
      <div className="shrink-0 size-7 rounded-full bg-purple/10 flex items-center justify-center text-sm">
        {personaIcon || "🤖"}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-1.5 mb-1">
          <span className="text-[11px] font-medium text-foreground">{personaName}</span>
          {personaRole && <span className="text-[9px] text-dim">{personaRole}</span>}
        </div>
        <div className="text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    </div>
  );
}
